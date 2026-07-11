use super::*;
use crate::model::WorkspaceNoteProposalDecisionRow;
use crate::model::WorkspaceNoteProposalRow;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;
use super::workspace_agent_queries::workspace_note_proposal_decision_row_by_id;
use super::workspace_agent_queries::workspace_note_proposal_row_by_id;

impl WorkspaceStore {
    pub async fn resolve_note_proposal_with(
        &self,
        input: crate::WorkspaceNoteProposalResolve,
    ) -> anyhow::Result<Option<crate::WorkspaceNoteProposal>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        // Serialize proposal decisions before reading their terminal state so
        // concurrent retries observe the canonical first decision instead of
        // racing through the optimistic note update.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(proposal_row) =
            workspace_note_proposal_row_by_id(&mut tx, &input.proposal_id).await?
        else {
            tx.rollback().await?;
            return Ok(None);
        };
        let proposal: crate::WorkspaceNoteProposal =
            WorkspaceNoteProposalRow::try_from_row(&proposal_row)?.try_into()?;
        if proposal.status != crate::WorkspaceNoteProposalStatus::Pending {
            let retry_result =
                validate_resolved_proposal_retry(&mut tx, &proposal, &input.resolution).await;
            tx.rollback().await?;
            retry_result?;
            return Ok(Some(proposal));
        }

        let note: Option<(String, Option<String>, String, i64)> = sqlx::query_as(
            "SELECT client_id, encounter_id, status, current_revision FROM workspace_notes WHERE id = ?",
        )
        .bind(&proposal.note_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, note_status, current_revision)) = note else {
            tx.rollback().await?;
            return Ok(None);
        };

        let (proposal_status, decision_kind, applied_text, resulting_note_revision) =
            match input.resolution {
                crate::WorkspaceNoteProposalResolution::Decline => (
                    crate::WorkspaceNoteProposalStatus::Declined,
                    crate::WorkspaceNoteProposalDecisionKind::RejectedAll,
                    None,
                    None,
                ),
                crate::WorkspaceNoteProposalResolution::Accept => {
                    resolve_accepted_body(
                        &mut tx,
                        AcceptedProposal {
                            proposal: &proposal,
                            note_status: &note_status,
                            current_revision,
                            body: proposal.proposed_body.clone(),
                            actor: &input.actor,
                        },
                        crate::WorkspaceNoteProposalDecisionKind::AcceptedAll,
                        now_ms,
                    )
                    .await?
                }
                crate::WorkspaceNoteProposalResolution::AcceptEdited { body } => {
                    if body.trim().is_empty() {
                        anyhow::bail!("edited workspace note proposal body must not be empty");
                    }
                    resolve_accepted_body(
                        &mut tx,
                        AcceptedProposal {
                            proposal: &proposal,
                            note_status: &note_status,
                            current_revision,
                            body,
                            actor: &input.actor,
                        },
                        crate::WorkspaceNoteProposalDecisionKind::AcceptedEdited,
                        now_ms,
                    )
                    .await?
                }
            };

        sqlx::query(
            "UPDATE workspace_note_proposals SET status = ?, resolved_at_ms = ? WHERE id = ? AND status = 'pending'",
        )
        .bind(proposal_status.as_str())
        .bind(now_ms)
        .bind(&proposal.id)
        .execute(&mut *tx)
        .await?;
        insert_proposal_decision(
            &mut tx,
            ProposalDecisionInsert {
                proposal_id: &proposal.id,
                agent_result_id: proposal.agent_result_id.as_deref(),
                note_id: &proposal.note_id,
                base_revision: proposal.base_revision,
                decision_kind,
                change_id: None,
                applied_text: applied_text.as_deref(),
                resulting_note_revision,
                actor: &input.actor,
                reason: &input.reason,
            },
            now_ms,
        )
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "note_proposal".to_string(),
                entity_id: proposal.id.clone(),
                action: proposal_status.as_str().to_string(),
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(client_id),
                encounter_id,
                note_id: Some(proposal.note_id),
                source_thread_id: proposal.source_thread_id,
                source_turn_id: proposal.source_turn_id,
                success: true,
                summary: proposal.summary,
                metadata_json: resulting_note_revision
                    .map(|revision| format!(r#"{{"resulting_note_revision":{revision}}}"#)),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = workspace_note_proposal_row_by_id(&mut tx, &proposal.id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "resolved workspace note proposal `{}` was not found",
                    proposal.id
                )
            })?;
        tx.commit().await?;
        WorkspaceNoteProposalRow::try_from_row(&row)
            .and_then(TryInto::try_into)
            .map(Some)
    }

    pub async fn record_note_proposal_change_decision(
        &self,
        input: crate::WorkspaceNoteProposalChangeDecisionCreate,
    ) -> anyhow::Result<crate::WorkspaceNoteProposalDecision> {
        if !matches!(
            input.decision_kind,
            crate::WorkspaceNoteProposalDecisionKind::CopiedChange
                | crate::WorkspaceNoteProposalDecisionKind::RejectedChange
        ) {
            anyhow::bail!("change decisions must be copied_change or rejected_change");
        }
        if input.change_id.trim().is_empty() {
            anyhow::bail!("workspace note proposal change id must not be empty");
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let proposal = workspace_note_proposal_row_by_id(&mut tx, &input.proposal_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace note proposal `{}` was not found",
                    input.proposal_id
                )
            })?;
        let proposal: crate::WorkspaceNoteProposal =
            WorkspaceNoteProposalRow::try_from_row(&proposal)?.try_into()?;
        if proposal.status != crate::WorkspaceNoteProposalStatus::Pending {
            anyhow::bail!(
                "workspace note proposal `{}` is already resolved",
                proposal.id
            );
        }
        let id = insert_proposal_decision(
            &mut tx,
            ProposalDecisionInsert {
                proposal_id: &proposal.id,
                agent_result_id: proposal.agent_result_id.as_deref(),
                note_id: &proposal.note_id,
                base_revision: proposal.base_revision,
                decision_kind: input.decision_kind,
                change_id: Some(input.change_id.trim()),
                applied_text: input.applied_text.as_deref(),
                resulting_note_revision: None,
                actor: &input.actor,
                reason: &input.reason,
            },
            now_ms,
        )
        .await?;
        let row = workspace_note_proposal_decision_row_by_id(&mut tx, &id).await?;
        tx.commit().await?;
        WorkspaceNoteProposalDecisionRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_note_proposal_decisions(
        &self,
        proposal_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceNoteProposalDecision>> {
        let rows = sqlx::query(
            r#"
SELECT
    id, proposal_id, agent_result_id, note_id, base_revision, decision_kind,
    change_id, applied_text, applied_text_sha256, resulting_note_revision,
    actor, reason, created_at_ms
FROM workspace_note_proposal_decisions
WHERE proposal_id = ?
ORDER BY created_at_ms ASC, rowid ASC
            "#,
        )
        .bind(proposal_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| {
                WorkspaceNoteProposalDecisionRow::try_from_row(&row).and_then(TryInto::try_into)
            })
            .collect()
    }
}

async fn validate_resolved_proposal_retry(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    proposal: &crate::WorkspaceNoteProposal,
    resolution: &crate::WorkspaceNoteProposalResolution,
) -> anyhow::Result<()> {
    use crate::WorkspaceNoteProposalResolution;
    use crate::WorkspaceNoteProposalStatus;

    match (proposal.status, resolution) {
        (WorkspaceNoteProposalStatus::Declined, WorkspaceNoteProposalResolution::Decline) => Ok(()),
        (WorkspaceNoteProposalStatus::Accepted, WorkspaceNoteProposalResolution::Accept) => {
            let decision = stored_acceptance_decision(tx, &proposal.id).await?;
            if decision.decision_kind == crate::WorkspaceNoteProposalDecisionKind::AcceptedAll {
                return Ok(());
            }
            anyhow::bail!(
                "cannot retry unedited acceptance for workspace note proposal `{}` because the stored acceptance was edited",
                proposal.id
            );
        }
        (
            WorkspaceNoteProposalStatus::Accepted,
            WorkspaceNoteProposalResolution::AcceptEdited { body },
        ) => {
            if body.trim().is_empty() {
                anyhow::bail!("edited workspace note proposal body must not be empty");
            }
            let decision = stored_acceptance_decision(tx, &proposal.id).await?;
            if decision.decision_kind == crate::WorkspaceNoteProposalDecisionKind::AcceptedEdited
                && decision.applied_text.as_deref() == Some(body.as_str())
            {
                return Ok(());
            }
            anyhow::bail!(
                "cannot retry edited acceptance for workspace note proposal `{}` because the stored acceptance differs",
                proposal.id
            );
        }
        (WorkspaceNoteProposalStatus::Accepted, WorkspaceNoteProposalResolution::Decline) => {
            anyhow::bail!(
                "cannot decline workspace note proposal `{}` because it is already accepted",
                proposal.id
            );
        }
        (
            WorkspaceNoteProposalStatus::Declined,
            WorkspaceNoteProposalResolution::Accept
            | WorkspaceNoteProposalResolution::AcceptEdited { .. },
        ) => {
            anyhow::bail!(
                "cannot accept workspace note proposal `{}` because it is already declined",
                proposal.id
            );
        }
        (WorkspaceNoteProposalStatus::Pending, _) => {
            anyhow::bail!("workspace note proposal `{}` is still pending", proposal.id);
        }
    }
}

async fn stored_acceptance_decision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    proposal_id: &str,
) -> anyhow::Result<crate::WorkspaceNoteProposalDecision> {
    let row = sqlx::query(
        r#"
SELECT
    id, proposal_id, agent_result_id, note_id, base_revision, decision_kind,
    change_id, applied_text, applied_text_sha256, resulting_note_revision,
    actor, reason, created_at_ms
FROM workspace_note_proposal_decisions
WHERE proposal_id = ? AND decision_kind IN ('accepted_all', 'accepted_edited')
ORDER BY created_at_ms DESC, rowid DESC
LIMIT 1
        "#,
    )
    .bind(proposal_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "accepted workspace note proposal `{proposal_id}` has no stored acceptance decision"
        )
    })?;
    WorkspaceNoteProposalDecisionRow::try_from_row(&row).and_then(TryInto::try_into)
}

struct AcceptedProposal<'a> {
    proposal: &'a crate::WorkspaceNoteProposal,
    note_status: &'a str,
    current_revision: i64,
    body: String,
    actor: &'a str,
}

async fn resolve_accepted_body(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: AcceptedProposal<'_>,
    decision_kind: crate::WorkspaceNoteProposalDecisionKind,
    now_ms: i64,
) -> anyhow::Result<(
    crate::WorkspaceNoteProposalStatus,
    crate::WorkspaceNoteProposalDecisionKind,
    Option<String>,
    Option<i64>,
)> {
    if matches!(input.note_status.trim(), "signed" | "addended") {
        anyhow::bail!(
            "signed workspace notes require an addendum instead of replacement proposals"
        );
    }
    if input.current_revision != input.proposal.base_revision {
        anyhow::bail!(
            "cannot accept proposal based on revision {} because note is now at revision {}",
            input.proposal.base_revision,
            input.current_revision
        );
    }
    let next_revision = input.current_revision + 1;
    let updated = sqlx::query(
        "UPDATE workspace_notes SET body = ?, current_revision = ?, updated_at_ms = ? WHERE id = ? AND current_revision = ?",
    )
    .bind(&input.body)
    .bind(next_revision)
    .bind(now_ms)
    .bind(&input.proposal.note_id)
    .bind(input.current_revision)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        anyhow::bail!("workspace note revision changed while accepting proposal");
    }
    sqlx::query(
        r#"
INSERT INTO workspace_note_revisions (
    note_id, revision, body, actor, source_thread_id, source_turn_id, summary, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.proposal.note_id)
    .bind(next_revision)
    .bind(&input.body)
    .bind(input.actor)
    .bind(&input.proposal.source_thread_id)
    .bind(&input.proposal.source_turn_id)
    .bind(&input.proposal.summary)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok((
        crate::WorkspaceNoteProposalStatus::Accepted,
        decision_kind,
        Some(input.body),
        Some(next_revision),
    ))
}

struct ProposalDecisionInsert<'a> {
    proposal_id: &'a str,
    agent_result_id: Option<&'a str>,
    note_id: &'a str,
    base_revision: i64,
    decision_kind: crate::WorkspaceNoteProposalDecisionKind,
    change_id: Option<&'a str>,
    applied_text: Option<&'a str>,
    resulting_note_revision: Option<i64>,
    actor: &'a str,
    reason: &'a str,
}

async fn insert_proposal_decision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: ProposalDecisionInsert<'_>,
    now_ms: i64,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let applied_text_sha256 = input
        .applied_text
        .map(|text| format!("{:x}", Sha256::digest(text.as_bytes())));
    sqlx::query(
        r#"
INSERT INTO workspace_note_proposal_decisions (
    id, proposal_id, agent_result_id, note_id, base_revision, decision_kind,
    change_id, applied_text, applied_text_sha256, resulting_note_revision,
    actor, reason, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(input.proposal_id)
    .bind(input.agent_result_id)
    .bind(input.note_id)
    .bind(input.base_revision)
    .bind(input.decision_kind.as_str())
    .bind(input.change_id)
    .bind(input.applied_text)
    .bind(applied_text_sha256)
    .bind(input.resulting_note_revision)
    .bind(input.actor)
    .bind(input.reason)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}
