use super::PlanResult;
use super::not_found;
use super::required;
use super::stale;
use super::transition;
use super::validation;
use crate::model::WorkspacePlanRevisionRow;
use crate::model::WorkspacePlanSubmissionReceiptRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use crate::runtime::workspace_agent_queries::workspace_agent_run_row_by_id;
use crate::runtime::workspace_agent_queries::workspace_context_packet_row_by_id;
use chrono::Utc;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use std::collections::BTreeSet;

macro_rules! revision_query {
    ($suffix:literal) => {
        concat!(
            "SELECT id, plan_session_id, client_id, guide_run_id, revision, plan_markdown, ",
            "decisions_json, open_questions_json, content_sha256, idempotency_key, ",
            "evidence_manifest_json, evidence_manifest_sha256, evidence_read_count, ",
            "status, source_checkpoint_id, source_checkpoint_revision, ",
            "source_checkpoint_sha256, encounter_id, note_id, source_thread_id, ",
            "source_turn_id, created_at_ms, submitted_at_ms ",
            "FROM workspace_plan_revisions ",
            $suffix
        )
    };
}

struct PlanRevisionTransition<'a> {
    revision_id: &'a str,
    session_id: &'a str,
    client_id: &'a str,
    content_sha256: &'a str,
    next: crate::WorkspacePlanRevisionStatus,
    actor: &'a str,
    summary: &'a str,
    expected_checkpoint: Option<(&'a str, i64, &'a str)>,
    expected_handoff: Option<(&'a str, &'a str)>,
}

impl WorkspaceStore {
    #[deprecated(
        note = "use complete_plan_turn so revision publication is atomic and evidence-bound"
    )]
    pub async fn create_plan_revision(
        &self,
        _input: crate::WorkspacePlanRevisionCreate,
    ) -> PlanResult<crate::WorkspacePlanRevision> {
        Err(validation(
            "workspace plan revisions must be published through complete_plan_turn",
        ))
    }
    pub async fn get_plan_revision(
        &self,
        revision_id: &str,
        session_id: &str,
        client_id: &str,
    ) -> PlanResult<Option<crate::WorkspacePlanRevision>> {
        let revision_id = required("revision id", revision_id)?;
        let session_id = required("revision session id", session_id)?;
        let client_id = required("revision client id", client_id)?;
        let row = sqlx::query_as::<_, WorkspacePlanRevisionRow>(revision_query!(
            "WHERE id = ? AND plan_session_id = ? AND client_id = ?"
        ))
        .bind(revision_id)
        .bind(session_id)
        .bind(client_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| row.try_into_model(false))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn list_plan_revisions(
        &self,
        filter: crate::WorkspacePlanRevisionFilter,
    ) -> PlanResult<Vec<crate::WorkspacePlanRevision>> {
        let session_id = required("revision session id", &filter.plan_session_id)?;
        let client_id = required("revision client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(20).clamp(1, 100);
        let rows = sqlx::query_as::<_, WorkspacePlanRevisionRow>(revision_query!(
            "WHERE plan_session_id = ? AND client_id = ? AND (? IS NULL OR revision < ?) ORDER BY revision DESC LIMIT ?"
        ))
        .bind(session_id)
        .bind(client_id)
        .bind(filter.before_revision)
        .bind(filter.before_revision)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| row.try_into_model(false).map_err(Into::into))
            .collect()
    }

    /// Loads the exact immutable submission receipt for each requested submitted revision.
    ///
    /// The returned order matches `plan_revision_ids`. A missing or scope-mismatched receipt is
    /// treated as a storage-integrity failure rather than silently returning a partial handoff.
    pub async fn list_plan_submission_receipts(
        &self,
        plan_session_id: &str,
        client_id: &str,
        plan_revision_ids: &[String],
    ) -> PlanResult<Vec<crate::WorkspacePlanSubmissionReceipt>> {
        let plan_session_id = required("submission receipt session id", plan_session_id)?;
        let client_id = required("submission receipt client id", client_id)?;
        if plan_revision_ids.len() > 100 {
            return Err(validation(
                "workspace plan submission receipt lookup is limited to 100 revisions",
            ));
        }
        if plan_revision_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut unique_ids = BTreeSet::new();
        let mut normalized_ids = Vec::with_capacity(plan_revision_ids.len());
        for revision_id in plan_revision_ids {
            let revision_id = required("submission receipt revision id", revision_id)?;
            if !unique_ids.insert(revision_id) {
                return Err(validation(format!(
                    "workspace plan submission receipt lookup repeats revision `{revision_id}`"
                )));
            }
            normalized_ids.push(revision_id);
        }

        let mut query =
            QueryBuilder::<Sqlite>::new("WITH requested(plan_revision_id, ordinal) AS (VALUES ");
        for (ordinal, revision_id) in normalized_ids.iter().enumerate() {
            if ordinal > 0 {
                query.push(", ");
            }
            query
                .push("(")
                .push_bind(revision_id)
                .push(", ")
                .push_bind(i64::try_from(ordinal).map_err(|error| {
                    crate::WorkspacePlanError::Storage {
                        message: error.to_string(),
                    }
                })?)
                .push(")");
        }
        query.push(
            r#")
SELECT
    receipt.plan_revision_id,
    receipt.packet_id,
    receipt.agent_run_id,
    receipt.plan_session_id,
    receipt.client_id,
    receipt.plan_content_sha256,
    receipt.evidence_manifest_sha256,
    receipt.submitted_by,
    receipt.submitted_at_ms
FROM requested
JOIN workspace_plan_submission_receipts AS receipt
  ON receipt.plan_revision_id = requested.plan_revision_id
JOIN workspace_plan_revisions AS revision
  ON revision.id = receipt.plan_revision_id
WHERE receipt.plan_session_id = "#,
        );
        query
            .push_bind(plan_session_id)
            .push(" AND receipt.client_id = ")
            .push_bind(client_id)
            .push(
                r#"
  AND revision.plan_session_id = receipt.plan_session_id
  AND revision.client_id = receipt.client_id
  AND revision.status = 'submitted'
  AND revision.content_sha256 = receipt.plan_content_sha256
  AND revision.evidence_manifest_sha256 = receipt.evidence_manifest_sha256
  AND revision.submitted_at_ms = receipt.submitted_at_ms
ORDER BY requested.ordinal
                "#,
            );
        let rows = query
            .build_query_as::<WorkspacePlanSubmissionReceiptRow>()
            .fetch_all(self.pool.as_ref())
            .await?;
        let exact = rows.len() == normalized_ids.len()
            && rows
                .iter()
                .zip(normalized_ids)
                .all(|(row, revision_id)| row.plan_revision_id == revision_id);
        if !exact {
            return Err(crate::WorkspacePlanError::Storage {
                message: "one or more submitted workspace plan revisions are missing their exact immutable submission receipt"
                    .to_string(),
            });
        }
        rows.into_iter()
            .map(|row| row.try_into_model().map_err(Into::into))
            .collect()
    }

    pub async fn outdate_plan_revision(
        &self,
        input: crate::WorkspacePlanRevisionOutdate,
    ) -> PlanResult<crate::WorkspacePlanRevision> {
        let reason = required("revision outdate reason", &input.reason)?;
        let actor = required("revision outdate actor", &input.actor)?;
        self.transition_plan_revision(PlanRevisionTransition {
            revision_id: &input.revision_id,
            session_id: &input.plan_session_id,
            client_id: &input.client_id,
            content_sha256: &input.content_sha256,
            next: crate::WorkspacePlanRevisionStatus::Outdated,
            actor,
            summary: reason,
            expected_checkpoint: None,
            expected_handoff: None,
        })
        .await
    }

    pub async fn submit_plan_revision(
        &self,
        input: crate::WorkspacePlanRevisionSubmit,
    ) -> PlanResult<crate::WorkspacePlanRevision> {
        let actor = required("revision submit actor", &input.actor)?;
        let checkpoint_id = required("revision source checkpoint id", &input.source_checkpoint_id)?;
        let checkpoint_hash = required(
            "revision source checkpoint SHA-256",
            &input.source_checkpoint_sha256,
        )?;
        let packet_id = required("revision submit packet id", &input.packet_id)?;
        let agent_run_id = required("revision submit agent run id", &input.agent_run_id)?;
        self.transition_plan_revision(PlanRevisionTransition {
            revision_id: &input.revision_id,
            session_id: &input.plan_session_id,
            client_id: &input.client_id,
            content_sha256: &input.content_sha256,
            next: crate::WorkspacePlanRevisionStatus::Submitted,
            actor,
            summary: "decision-complete plan submitted",
            expected_checkpoint: Some((
                checkpoint_id,
                input.source_checkpoint_revision,
                checkpoint_hash,
            )),
            expected_handoff: Some((packet_id, agent_run_id)),
        })
        .await
    }

    async fn transition_plan_revision(
        &self,
        input: PlanRevisionTransition<'_>,
    ) -> PlanResult<crate::WorkspacePlanRevision> {
        let revision_id = required("revision id", input.revision_id)?;
        let session_id = required("revision session id", input.session_id)?;
        let client_id = required("revision client id", input.client_id)?;
        let content_sha256 = required("revision content SHA-256", input.content_sha256)?;
        let next = input.next;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        let row = revision_by_id(&mut tx, revision_id).await?.ok_or_else(|| {
            not_found(format!(
                "workspace plan revision `{revision_id}` was not found"
            ))
        })?;
        if row.plan_session_id != session_id
            || row.client_id != client_id
            || row.content_sha256 != content_sha256
        {
            return Err(validation(
                "workspace plan revision transition identity does not match the persisted revision",
            ));
        }
        let current = row.status.as_str();
        if current != next.as_str()
            && current != crate::WorkspacePlanRevisionStatus::Current.as_str()
        {
            return Err(transition(format!(
                "workspace plan revision `{revision_id}` cannot transition from `{current}` to `{}`",
                next.as_str()
            )));
        }
        if next == crate::WorkspacePlanRevisionStatus::Submitted {
            let (checkpoint_id, checkpoint_revision, checkpoint_hash) =
                input
                    .expected_checkpoint
                    .ok_or_else(|| validation("submit checkpoint is required"))?;
            if row.source_checkpoint_id != checkpoint_id
                || row.source_checkpoint_revision != checkpoint_revision
                || row.source_checkpoint_sha256 != checkpoint_hash
            {
                return Err(stale(
                    "workspace plan submission checkpoint does not match the plan revision",
                ));
            }
            let (packet_id, agent_run_id) = input
                .expected_handoff
                .ok_or_else(|| validation("submit packet and agent run are required"))?;
            validate_submission_handoff(&mut tx, &row, packet_id, agent_run_id).await?;
            if current == crate::WorkspacePlanRevisionStatus::Current.as_str() {
                let still_current = sqlx::query_scalar::<_, i64>(
                    r#"
SELECT 1
FROM workspace_guide_runs AS run
JOIN workspace_draft_sessions AS session ON session.id = run.session_id
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.session_id = session.id AND checkpoint.revision = session.current_revision
WHERE run.id = ? AND checkpoint.id = ? AND checkpoint.revision = ?
  AND checkpoint.content_sha256 = ?
                    "#,
                )
                .bind(&row.guide_run_id)
                .bind(checkpoint_id)
                .bind(checkpoint_revision)
                .bind(checkpoint_hash)
                .fetch_optional(&mut *tx)
                .await?
                .is_some();
                if !still_current {
                    return Err(stale(
                        "workspace plan source checkpoint changed before submission",
                    ));
                }
            }
        }
        if current == next.as_str() {
            if next == crate::WorkspacePlanRevisionStatus::Submitted {
                let (packet_id, agent_run_id) = input
                    .expected_handoff
                    .ok_or_else(|| validation("submit packet and agent run are required"))?;
                validate_submission_receipt(&mut tx, &row, packet_id, agent_run_id).await?;
            }
            tx.rollback().await?;
            return Ok(row.try_into_model(true)?);
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let updated = sqlx::query(
            "UPDATE workspace_plan_revisions SET status = ?, submitted_at_ms = ? WHERE id = ? AND status = 'current'",
        )
        .bind(next.as_str())
        .bind((next == crate::WorkspacePlanRevisionStatus::Submitted).then_some(now_ms))
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(transition(format!(
                "workspace plan revision `{revision_id}` changed before its `{}` transition completed",
                next.as_str()
            )));
        }
        if next == crate::WorkspacePlanRevisionStatus::Submitted {
            let (packet_id, agent_run_id) = input
                .expected_handoff
                .ok_or_else(|| validation("submit packet and agent run are required"))?;
            insert_submission_receipt(&mut tx, &row, packet_id, agent_run_id, input.actor, now_ms)
                .await?;
        }
        if next == crate::WorkspacePlanRevisionStatus::Outdated {
            sqlx::query("UPDATE workspace_plan_proposals SET status = 'outdated' WHERE plan_revision_id = ? AND status = 'pending'")
                .bind(revision_id)
                .execute(&mut *tx)
                .await?;
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_revision".to_string(),
                entity_id: revision_id.to_string(),
                action: next.as_str().to_string(),
                actor: input.actor.to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                encounter_id: row.encounter_id,
                note_id: row.note_id,
                source_thread_id: Some(row.source_thread_id),
                source_turn_id: Some(row.source_turn_id),
                success: true,
                summary: input.summary.to_string(),
                metadata_json: Some(
                    serde_json::json!({
                        "contentSha256": content_sha256,
                        "packetId": input.expected_handoff.map(|(packet_id, _)| packet_id),
                        "agentRunId": input.expected_handoff.map(|(_, agent_run_id)| agent_run_id),
                        "from": current,
                        "to": next.as_str(),
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let updated = revision_by_id(&mut tx, revision_id)
            .await?
            .ok_or_else(|| not_found("updated workspace plan revision was not found"))?;
        tx.commit().await?;
        Ok(updated.try_into_model(false)?)
    }
}

async fn validate_submission_handoff(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    revision: &WorkspacePlanRevisionRow,
    packet_id: &str,
    agent_run_id: &str,
) -> PlanResult<()> {
    let packet = workspace_context_packet_row_by_id(tx, packet_id)
        .await?
        .ok_or_else(|| {
            not_found(format!(
                "workspace context packet `{packet_id}` was not found for plan submission"
            ))
        })?;
    if packet.client_id != revision.client_id
        || packet.encounter_id != revision.encounter_id
        || packet.note_id != revision.note_id
        || packet.source_checkpoint_id.as_deref() != Some(revision.source_checkpoint_id.as_str())
        || packet.source_checkpoint_sha256.as_deref()
            != Some(revision.source_checkpoint_sha256.as_str())
        || packet.workspace_plan_revision_id.as_deref() != Some(revision.id.as_str())
        || packet.workspace_plan_content_sha256.as_deref() != Some(revision.content_sha256.as_str())
        || packet.workspace_plan_evidence_manifest_sha256.as_deref()
            != Some(revision.evidence_manifest_sha256.as_str())
    {
        return Err(validation(
            "workspace plan submission packet does not carry the exact revision binding",
        ));
    }
    if !matches!(
        packet.status.as_str(),
        "submitted" | "sent" | "result_saved"
    ) {
        return Err(transition(format!(
            "workspace context packet `{packet_id}` is `{}` and is not submitted",
            packet.status
        )));
    }

    let run = workspace_agent_run_row_by_id(tx, agent_run_id)
        .await?
        .ok_or_else(|| {
            not_found(format!(
                "workspace agent run `{agent_run_id}` was not found for plan submission"
            ))
        })?;
    if run.packet_id != packet.id
        || run.client_id != revision.client_id
        || run.note_id != revision.note_id
        || run.workspace_plan_revision_id.as_deref() != Some(revision.id.as_str())
        || run.workspace_plan_content_sha256.as_deref() != Some(revision.content_sha256.as_str())
        || run.workspace_plan_evidence_manifest_sha256.as_deref()
            != Some(revision.evidence_manifest_sha256.as_str())
    {
        return Err(validation(
            "workspace plan submission agent run does not carry the exact packet revision binding",
        ));
    }
    if run.run_kind != "agent" || !matches!(run.status.as_str(), "running" | "completed") {
        return Err(transition(format!(
            "workspace agent run `{agent_run_id}` is kind `{}` with status `{}` and cannot submit a plan",
            run.run_kind, run.status
        )));
    }
    Ok(())
}

async fn insert_submission_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    revision: &WorkspacePlanRevisionRow,
    packet_id: &str,
    agent_run_id: &str,
    actor: &str,
    submitted_at_ms: i64,
) -> PlanResult<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_plan_submission_receipts (
    plan_revision_id, packet_id, agent_run_id, plan_session_id, client_id,
    plan_content_sha256, evidence_manifest_sha256, submitted_by, submitted_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&revision.id)
    .bind(packet_id)
    .bind(agent_run_id)
    .bind(&revision.plan_session_id)
    .bind(&revision.client_id)
    .bind(&revision.content_sha256)
    .bind(&revision.evidence_manifest_sha256)
    .bind(actor)
    .bind(submitted_at_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn validate_submission_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    revision: &WorkspacePlanRevisionRow,
    packet_id: &str,
    agent_run_id: &str,
) -> PlanResult<()> {
    let receipt_matches = sqlx::query_scalar::<_, i64>(
        r#"
SELECT 1
FROM workspace_plan_submission_receipts
WHERE plan_revision_id = ?
  AND packet_id = ?
  AND agent_run_id = ?
  AND plan_session_id = ?
  AND client_id = ?
  AND plan_content_sha256 = ?
  AND evidence_manifest_sha256 = ?
  AND submitted_at_ms = ?
LIMIT 1
        "#,
    )
    .bind(&revision.id)
    .bind(packet_id)
    .bind(agent_run_id)
    .bind(&revision.plan_session_id)
    .bind(&revision.client_id)
    .bind(&revision.content_sha256)
    .bind(&revision.evidence_manifest_sha256)
    .bind(revision.submitted_at_ms)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if !receipt_matches {
        return Err(validation(format!(
            "workspace plan revision `{}` was submitted with a different context packet or agent run",
            revision.id
        )));
    }
    Ok(())
}

pub(super) async fn revision_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> PlanResult<Option<WorkspacePlanRevisionRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanRevisionRow>(revision_query!("WHERE id = ?"))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?,
    )
}
