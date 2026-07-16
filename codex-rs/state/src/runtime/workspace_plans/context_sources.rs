use super::PlanResult;
use super::validation;
use crate::runtime::workspace_agent::MAX_AGENT_CONTEXT_SNAPSHOT_BYTES;
use crate::runtime::workspace_agent::MAX_AGENT_DISPLAY_LABEL_BYTES;
use crate::runtime::workspace_agent::MAX_AGENT_NOTE_BODY_BYTES;
use crate::runtime::workspace_agent::redact_local_path_tokens;
use crate::runtime::workspace_agent::truncate_utf8;
use sha2::Digest;
use sha2::Sha256;

mod history;
mod patient_chart;
mod selected_context;

pub(super) async fn read_category(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    category: &str,
    max_records: u32,
) -> PlanResult<Vec<crate::WorkspacePlanningContextSource>> {
    let sources = match category {
        "visit_history" => {
            Box::pin(history::read_visit_history(
                tx,
                &execution.client_id,
                max_records,
            ))
            .await?
        }
        "progress_notes" => {
            Box::pin(history::read_progress_notes(
                tx,
                &execution.client_id,
                max_records,
            ))
            .await?
        }
        "patient_chart" => {
            Box::pin(patient_chart::read_patient_chart(
                tx,
                execution,
                max_records,
            ))
            .await?
        }
        "selected_context" => {
            Box::pin(selected_context::read_selected_context(
                tx,
                execution,
                max_records,
            ))
            .await?
        }
        _ => unreachable!("planning context category was validated"),
    };
    let returned_bytes = sources
        .iter()
        .map(|source| source.snapshot_json.len())
        .sum::<usize>();
    if returned_bytes > MAX_AGENT_CONTEXT_SNAPSHOT_BYTES {
        return Err(validation(format!(
            "workspace planning context snapshot exceeds the {MAX_AGENT_CONTEXT_SNAPSHOT_BYTES} byte limit"
        )));
    }
    Ok(sources)
}

pub(super) fn context_source(
    entity_type: &str,
    entity_id: String,
    source_revision: Option<i64>,
    display_label: &str,
    snapshot_json: String,
) -> crate::WorkspacePlanningContextSource {
    let content_sha256 = format!("{:x}", Sha256::digest(snapshot_json.as_bytes()));
    crate::WorkspacePlanningContextSource {
        source_entity_type: entity_type.to_string(),
        source_entity_id: entity_id,
        source_revision,
        display_label: display_label.to_string(),
        snapshot_json,
        content_sha256,
    }
}

pub(super) fn safe_label(value: &str) -> (String, bool, bool) {
    let (value, paths_redacted) = redact_local_path_tokens(value);
    let (value, truncated) = truncate_utf8(&value, MAX_AGENT_DISPLAY_LABEL_BYTES);
    (value.to_string(), paths_redacted, truncated)
}

pub(super) fn safe_body(value: &str) -> (String, bool, bool) {
    let (value, paths_redacted) = redact_local_path_tokens(value);
    let (value, truncated) = truncate_utf8(&value, MAX_AGENT_NOTE_BODY_BYTES);
    (value.to_string(), paths_redacted, truncated)
}
