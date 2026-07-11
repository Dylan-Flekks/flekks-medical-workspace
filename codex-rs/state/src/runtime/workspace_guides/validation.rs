use super::errors::GuideResult;
use super::errors::required;
use super::errors::validation;

/// Limits keep caller-controlled provenance compact enough for durable SQLite rows and lists.
pub(super) const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub(super) const MAX_TRIGGER_BYTES: usize = 128;
pub(super) const MAX_ACTOR_BYTES: usize = 256;
pub(super) const MAX_PROVIDER_BYTES: usize = 128;
pub(super) const MAX_MODEL_BYTES: usize = 256;
pub(super) const MAX_SOURCE_PROVENANCE_ID_BYTES: usize = 256;

pub(super) fn validate_start(input: &crate::WorkspaceGuideRunStart) -> GuideResult<()> {
    required("client id", &input.client_id)?;
    required("session id", &input.session_id)?;
    required("source checkpoint id", &input.source_checkpoint_id)?;
    required_bounded(
        "idempotency key",
        &input.idempotency_key,
        MAX_IDEMPOTENCY_KEY_BYTES,
    )?;
    required_bounded("trigger", &input.trigger, MAX_TRIGGER_BYTES)?;
    required_bounded("actor", &input.actor, MAX_ACTOR_BYTES)?;
    required_bounded("provider", &input.provider, MAX_PROVIDER_BYTES)?;
    required_bounded("model", &input.model, MAX_MODEL_BYTES)?;
    if input.source_checkpoint_revision < 1 {
        return validation("workspace guide source checkpoint revision must be positive");
    }
    Ok(())
}

pub(super) fn validate_finish_metadata(
    input: &crate::WorkspaceGuideRunFinish,
) -> GuideResult<(Option<&str>, Option<&str>)> {
    required_bounded("actor", &input.actor, MAX_ACTOR_BYTES)?;
    let thread_id = bounded_optional(
        "source thread id",
        input.source_thread_id.as_deref(),
        MAX_SOURCE_PROVENANCE_ID_BYTES,
    )?;
    let turn_id = bounded_optional(
        "source turn id",
        input.source_turn_id.as_deref(),
        MAX_SOURCE_PROVENANCE_ID_BYTES,
    )?;
    if thread_id.is_some() != turn_id.is_some() {
        return validation("workspace guide source thread and turn must be supplied together");
    }
    Ok((thread_id, turn_id))
}

pub(super) fn normalized_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn required_bounded<'a>(label: &str, value: &'a str, max_bytes: usize) -> GuideResult<&'a str> {
    let value = required(label, value)?;
    ensure_bounded(label, value, max_bytes)?;
    Ok(value)
}

fn bounded_optional<'a>(
    label: &str,
    value: Option<&'a str>,
    max_bytes: usize,
) -> GuideResult<Option<&'a str>> {
    let Some(value) = normalized_optional(value) else {
        return Ok(None);
    };
    ensure_bounded(label, value, max_bytes)?;
    Ok(Some(value))
}

fn ensure_bounded(label: &str, value: &str, max_bytes: usize) -> GuideResult<()> {
    if value.len() > max_bytes {
        return validation(format!(
            "workspace guide {label} exceeds the {max_bytes} byte limit"
        ));
    }
    Ok(())
}
