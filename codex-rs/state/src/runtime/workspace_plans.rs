use super::workspace::WorkspaceStore;
use crate::model::WorkspacePlanSessionRow;
use sqlx::Sqlite;

mod completion;
mod completion_integrity;
mod context;
mod context_sources;
mod messages;
mod proposals;
mod recovery;
mod revisions;
mod sessions;

pub(super) type PlanResult<T> = Result<T, crate::WorkspacePlanError>;

pub(super) const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub(super) const MAX_PLAN_BYTES: usize = 128 * 1024;
pub(super) const MAX_PROPOSAL_BYTES: usize = 128 * 1024;

#[derive(sqlx::FromRow)]
pub(super) struct PlanRunBinding {
    pub client_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub model_tool_mode: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub is_stale: i64,
}

impl WorkspaceStore {
    pub(super) async fn plan_session_row(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<Option<WorkspacePlanSessionRow>> {
        Ok(sqlx::query_as(
            r#"
SELECT id, client_id, source_thread_id, status, latest_revision, created_by,
       created_at_ms, updated_at_ms, closed_at_ms
FROM workspace_plan_sessions
WHERE id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await?)
    }

    pub(super) async fn require_active_plan_session(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        session_id: &str,
        client_id: &str,
    ) -> PlanResult<WorkspacePlanSessionRow> {
        let session = self
            .plan_session_row(tx, session_id)
            .await?
            .ok_or_else(|| {
                not_found(format!(
                    "workspace plan session `{session_id}` was not found"
                ))
            })?;
        if session.client_id != client_id {
            return Err(validation(format!(
                "workspace plan session `{session_id}` belongs to client `{}` not `{client_id}`",
                session.client_id
            )));
        }
        if session.status != "active" {
            return Err(transition(format!(
                "workspace plan session `{session_id}` is `{}` and cannot be changed",
                session.status
            )));
        }
        Ok(session)
    }

    pub(super) async fn plan_run_binding(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        guide_run_id: &str,
        client_id: &str,
    ) -> PlanResult<PlanRunBinding> {
        let run = sqlx::query_as::<_, PlanRunBinding>(
            r#"
SELECT
    run.client_id,
    run.source_checkpoint_id,
    run.source_checkpoint_revision,
    run.source_checkpoint_sha256,
    checkpoint.encounter_id,
    checkpoint.note_id,
    run.model_tool_mode,
    run.provider,
    run.model,
    run.status,
    run.source_thread_id,
    run.source_turn_id,
    CASE WHEN current.id = run.source_checkpoint_id
         AND current.revision = run.source_checkpoint_revision
         AND current.content_sha256 = run.source_checkpoint_sha256
         THEN 0 ELSE 1 END AS is_stale
FROM workspace_guide_runs AS run
JOIN workspace_draft_sessions AS draft_session ON draft_session.id = run.session_id
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.id = run.source_checkpoint_id
 AND checkpoint.session_id = run.session_id
 AND checkpoint.client_id = run.client_id
JOIN workspace_draft_checkpoints AS current
  ON current.session_id = draft_session.id
 AND current.revision = draft_session.current_revision
WHERE run.id = ?
            "#,
        )
        .bind(guide_run_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            not_found(format!(
                "workspace planning guide run `{guide_run_id}` was not found"
            ))
        })?;
        if run.client_id != client_id {
            return Err(validation(format!(
                "workspace planning guide run `{guide_run_id}` belongs to client `{}` not `{client_id}`",
                run.client_id
            )));
        }
        if run.model_tool_mode != crate::WorkspaceGuideModelToolMode::WorkspacePlanningOnly.as_str()
        {
            return Err(validation(format!(
                "workspace guide run `{guide_run_id}` is not a planning-only run"
            )));
        }
        Ok(run)
    }
}

pub(super) fn required<'a>(label: &str, value: &'a str) -> PlanResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(validation(format!("workspace plan {label} is required")));
    }
    Ok(value)
}

pub(super) fn source_pair<'a>(
    thread_id: Option<&'a str>,
    turn_id: Option<&'a str>,
) -> PlanResult<(Option<&'a str>, Option<&'a str>)> {
    let thread_id = thread_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_id = turn_id.map(str::trim).filter(|value| !value.is_empty());
    if thread_id.is_some() != turn_id.is_some() {
        return Err(validation(
            "workspace plan source thread and turn ids must be provided together",
        ));
    }
    Ok((thread_id, turn_id))
}

pub(super) fn validate_bound_thread(
    session: &WorkspacePlanSessionRow,
    thread_id: &str,
) -> PlanResult<()> {
    match session.source_thread_id.as_deref() {
        Some(bound) if bound == thread_id => Ok(()),
        Some(bound) => Err(validation(format!(
            "workspace plan session `{}` is bound to thread `{bound}` not `{thread_id}`",
            session.id
        ))),
        None => Err(validation(format!(
            "workspace plan session `{}` must bind its dedicated thread before persisting model work",
            session.id
        ))),
    }
}

pub(super) fn validation(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::Validation {
        message: message.into(),
    }
}

pub(super) fn not_found(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::NotFound {
        message: message.into(),
    }
}

pub(super) fn idempotency(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::IdempotencyConflict {
        message: message.into(),
    }
}

pub(super) fn terminal_conflict(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::TerminalConflict {
        message: message.into(),
    }
}

pub(super) fn stale(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::Stale {
        message: message.into(),
    }
}

pub(super) fn transition(message: impl Into<String>) -> crate::WorkspacePlanError {
    crate::WorkspacePlanError::Transition {
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "workspace_plans_tests.rs"]
mod tests;
