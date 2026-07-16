use super::MAX_PROPOSAL_BYTES;
use super::PlanResult;
use super::idempotency;
use super::not_found;
use super::required;
use super::transition;
use super::validate_bound_thread;
use super::validation;
use crate::model::WorkspacePlanProposalRow;
use crate::model::WorkspacePlanRevisionRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use chrono::Utc;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

macro_rules! proposal_query {
    ($suffix:literal) => {
        concat!(
            "SELECT id, plan_session_id, plan_revision_id, client_id, guide_run_id, ",
            "proposal_kind, payload_json, payload_sha256, summary, rationale, ",
            "idempotency_key, status, source_checkpoint_id, source_checkpoint_revision, ",
            "source_checkpoint_sha256, source_thread_id, source_turn_id, created_at_ms, ",
            "resolved_at_ms, resolved_by FROM workspace_plan_proposals ",
            $suffix
        )
    };
}

macro_rules! revision_query {
    ($suffix:literal) => {
        concat!(
            "SELECT id, plan_session_id, client_id, guide_run_id, revision, plan_markdown, ",
            "decisions_json, open_questions_json, content_sha256, evidence_manifest_json, ",
            "evidence_manifest_sha256, evidence_read_count, idempotency_key, ",
            "status, source_checkpoint_id, source_checkpoint_revision, ",
            "source_checkpoint_sha256, encounter_id, note_id, source_thread_id, ",
            "source_turn_id, created_at_ms, submitted_at_ms ",
            "FROM workspace_plan_revisions ",
            $suffix
        )
    };
}

impl WorkspaceStore {
    pub async fn create_plan_proposal(
        &self,
        input: crate::WorkspacePlanProposalCreate,
    ) -> PlanResult<crate::WorkspacePlanProposal> {
        let session_id = required("proposal session id", &input.plan_session_id)?;
        let revision_id = required("proposal plan revision id", &input.plan_revision_id)?;
        let client_id = required("proposal client id", &input.client_id)?;
        let guide_run_id = required("proposal guide run id", &input.guide_run_id)?;
        let summary = required("proposal summary", &input.summary)?;
        let rationale = required("proposal rationale", &input.rationale)?;
        let key = required("proposal idempotency key", &input.idempotency_key)?;
        let thread_id = required("proposal source thread id", &input.source_thread_id)?;
        let turn_id = required("proposal source turn id", &input.source_turn_id)?;
        validate_payload(&input.payload)?;
        let payload_json = serde_json::to_string(&input.payload)?;
        if payload_json.len() > MAX_PROPOSAL_BYTES {
            return Err(validation(format!(
                "workspace plan proposal exceeds the {MAX_PROPOSAL_BYTES} byte limit"
            )));
        }
        let payload_sha256 = format!("{:x}", Sha256::digest(payload_json.as_bytes()));
        let (target_note_id, base_revision) = input.payload.target_note();
        let kind = input.payload.kind();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let session = self
            .require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        validate_bound_thread(&session, thread_id)?;
        let run = self
            .plan_run_binding(&mut tx, guide_run_id, client_id)
            .await?;
        if run.status != "completed"
            || run.source_thread_id.as_deref() != Some(thread_id)
            || run.source_turn_id.as_deref() != Some(turn_id)
        {
            return Err(validation(format!(
                "workspace plan proposal requires completed guide run `{guide_run_id}` with matching thread and turn"
            )));
        }
        let revision = revision_by_id(&mut tx, revision_id).await?.ok_or_else(|| {
            not_found(format!(
                "workspace plan revision `{revision_id}` was not found"
            ))
        })?;
        if revision.plan_session_id != session_id
            || revision.client_id != client_id
            || revision.guide_run_id != guide_run_id
            || revision.source_thread_id != thread_id
            || revision.source_turn_id != turn_id
        {
            return Err(validation(
                "workspace plan proposal does not match its plan revision provenance",
            ));
        }
        if revision.status != crate::WorkspacePlanRevisionStatus::Current.as_str() {
            return Err(transition(format!(
                "workspace plan proposal requires a current plan revision, found `{}`",
                revision.status
            )));
        }
        if let Some(existing) = proposal_by_key(&mut tx, session_id, key).await? {
            if existing.plan_revision_id != revision_id
                || existing.guide_run_id != guide_run_id
                || existing.proposal_kind != kind.as_str()
                || existing.payload_json != payload_json
                || existing.payload_sha256 != payload_sha256
                || existing.summary != summary
                || existing.rationale != rationale
                || existing.source_thread_id != thread_id
                || existing.source_turn_id != turn_id
            {
                return Err(idempotency(format!(
                    "workspace plan proposal key `{key}` was reused with different content"
                )));
            }
            tx.rollback().await?;
            return Ok(existing.try_into_model(true)?);
        }

        let status = match (target_note_id, base_revision) {
            (Some(note_id), Some(base_revision)) => {
                let note = sqlx::query_as::<_, (String, i64)>(
                    "SELECT client_id, current_revision FROM workspace_notes WHERE id = ? AND archived_at_ms IS NULL",
                )
                .bind(note_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| {
                    not_found(format!(
                        "workspace plan proposal note `{note_id}` was not found or is archived"
                    ))
                })?;
                if note.0 != client_id {
                    return Err(validation(format!(
                        "workspace plan proposal note `{note_id}` belongs to client `{}` not `{client_id}`",
                        note.0
                    )));
                }
                if note.1 == base_revision {
                    crate::WorkspacePlanProposalStatus::Pending
                } else {
                    crate::WorkspacePlanProposalStatus::Outdated
                }
            }
            (None, None) => crate::WorkspacePlanProposalStatus::Pending,
            (Some(_), None) | (None, Some(_)) => {
                return Err(validation(
                    "workspace plan proposal target note and base revision must be provided together",
                ));
            }
        };
        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO workspace_plan_proposals (
    id, plan_session_id, plan_revision_id, client_id, guide_run_id,
    proposal_kind, target_note_id, base_revision, payload_json, payload_sha256,
    summary, rationale, idempotency_key, status, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, created_at_ms, resolved_at_ms, resolved_by
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL)
            "#,
        )
        .bind(&id)
        .bind(session_id)
        .bind(revision_id)
        .bind(client_id)
        .bind(guide_run_id)
        .bind(kind.as_str())
        .bind(target_note_id)
        .bind(base_revision)
        .bind(&payload_json)
        .bind(&payload_sha256)
        .bind(summary)
        .bind(rationale)
        .bind(key)
        .bind(status.as_str())
        .bind(&revision.source_checkpoint_id)
        .bind(revision.source_checkpoint_revision)
        .bind(&revision.source_checkpoint_sha256)
        .bind(thread_id)
        .bind(turn_id)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_proposal".to_string(),
                entity_id: id.clone(),
                action: "created".to_string(),
                actor: "workspace planner".to_string(),
                actor_kind: "agent".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                note_id: target_note_id.map(str::to_string),
                source_thread_id: Some(thread_id.to_string()),
                source_turn_id: Some(turn_id.to_string()),
                success: true,
                summary: format!("{} proposal stored as {}", kind.as_str(), status.as_str()),
                metadata_json: Some(
                    serde_json::json!({
                        "payloadSha256": payload_sha256,
                        "planRevisionId": revision_id,
                        "planSessionId": session_id,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = proposal_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| not_found("inserted workspace plan proposal was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }

    pub async fn list_plan_proposals(
        &self,
        filter: crate::WorkspacePlanProposalFilter,
    ) -> PlanResult<Vec<crate::WorkspacePlanProposal>> {
        let session_id = required("proposal session id", &filter.plan_session_id)?;
        let client_id = required("proposal client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let rows = sqlx::query_as::<_, WorkspacePlanProposalRow>(proposal_query!(
            "WHERE plan_session_id = ? AND client_id = ? AND (? IS NULL OR status = ?) ORDER BY created_at_ms DESC, id DESC LIMIT ?"
        ))
        .bind(session_id)
        .bind(client_id)
        .bind(filter.status.map(crate::WorkspacePlanProposalStatus::as_str))
        .bind(filter.status.map(crate::WorkspacePlanProposalStatus::as_str))
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| row.try_into_model(false).map_err(Into::into))
            .collect()
    }

    pub async fn resolve_plan_proposal(
        &self,
        input: crate::WorkspacePlanProposalResolve,
    ) -> PlanResult<crate::WorkspacePlanProposal> {
        let proposal_id = required("proposal id", &input.proposal_id)?;
        let session_id = required("proposal session id", &input.plan_session_id)?;
        let client_id = required("proposal client id", &input.client_id)?;
        let actor = required("proposal decision actor", &input.actor)?;
        let next = input.resolution.status();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        let proposal = proposal_by_id(&mut tx, proposal_id).await?.ok_or_else(|| {
            not_found(format!(
                "workspace plan proposal `{proposal_id}` was not found"
            ))
        })?;
        if proposal.plan_session_id != session_id || proposal.client_id != client_id {
            return Err(validation(
                "workspace plan proposal decision identity does not match the persisted proposal",
            ));
        }
        if proposal.status == next.as_str() {
            tx.rollback().await?;
            return Ok(proposal.try_into_model(true)?);
        }
        if proposal.status != crate::WorkspacePlanProposalStatus::Pending.as_str() {
            return Err(transition(format!(
                "workspace plan proposal `{proposal_id}` cannot transition from `{}` to `{}`",
                proposal.status,
                next.as_str()
            )));
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            "UPDATE workspace_plan_proposals SET status = ?, resolved_at_ms = ?, resolved_by = ? WHERE id = ? AND status = 'pending'",
        )
        .bind(next.as_str())
        .bind(now_ms)
        .bind(actor)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_proposal".to_string(),
                entity_id: proposal_id.to_string(),
                action: next.as_str().to_string(),
                actor: actor.to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                source_thread_id: Some(proposal.source_thread_id),
                source_turn_id: Some(proposal.source_turn_id),
                success: true,
                summary: format!(
                    "proposal decision recorded as {}; canonical chart unchanged",
                    next.as_str()
                ),
                metadata_json: Some(
                    serde_json::json!({
                        "payloadSha256": proposal.payload_sha256,
                        "planRevisionId": proposal.plan_revision_id,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let updated = proposal_by_id(&mut tx, proposal_id)
            .await?
            .ok_or_else(|| not_found("updated workspace plan proposal was not found"))?;
        tx.commit().await?;
        Ok(updated.try_into_model(false)?)
    }
}

async fn proposal_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> PlanResult<Option<WorkspacePlanProposalRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanProposalRow>(proposal_query!("WHERE id = ?"))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?,
    )
}

async fn proposal_by_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
    key: &str,
) -> PlanResult<Option<WorkspacePlanProposalRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanProposalRow>(proposal_query!(
            "WHERE plan_session_id = ? AND idempotency_key = ?"
        ))
        .bind(session_id)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?,
    )
}

async fn revision_by_id(
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

fn validate_payload(payload: &crate::WorkspacePlanProposalPayload) -> PlanResult<()> {
    match payload {
        crate::WorkspacePlanProposalPayload::NoteRevision {
            note_id,
            base_revision,
            proposed_body,
        } => {
            required("proposal note id", note_id)?;
            required("proposal note body", proposed_body)?;
            if *base_revision < 1 {
                return Err(validation(
                    "workspace plan proposal base revision must be positive",
                ));
            }
        }
        crate::WorkspacePlanProposalPayload::NoteAddendum {
            note_id,
            base_revision,
            body,
        } => {
            required("proposal note id", note_id)?;
            required("proposal addendum body", body)?;
            if *base_revision < 1 {
                return Err(validation(
                    "workspace plan proposal base revision must be positive",
                ));
            }
        }
        crate::WorkspacePlanProposalPayload::TaskDraft {
            title,
            details: _,
            task_kind,
            priority: _,
            due_date: _,
            assigned_to: _,
        } => {
            required("proposal task title", title)?;
            required("proposal task kind", task_kind)?;
        }
    }
    Ok(())
}
