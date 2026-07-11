use crate::WorkspaceGuideError;

pub(super) type GuideResult<T> = Result<T, WorkspaceGuideError>;

pub(super) fn required<'a>(label: &str, value: &'a str) -> GuideResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return validation(format!("workspace guide {label} must not be empty"));
    }
    Ok(value)
}

pub(super) fn validation<T>(message: impl Into<String>) -> GuideResult<T> {
    Err(WorkspaceGuideError::Validation {
        message: message.into(),
    })
}

pub(super) fn stale_checkpoint<T>(message: impl Into<String>) -> GuideResult<T> {
    Err(WorkspaceGuideError::StaleCheckpoint {
        message: message.into(),
    })
}

pub(super) fn idempotency_conflict<T>(message: impl Into<String>) -> GuideResult<T> {
    Err(WorkspaceGuideError::IdempotencyConflict {
        message: message.into(),
    })
}

pub(super) fn active_run_conflict<T>(message: impl Into<String>) -> GuideResult<T> {
    Err(WorkspaceGuideError::ActiveRunConflict {
        message: message.into(),
    })
}

pub(super) fn terminal_conflict<T>(message: impl Into<String>) -> GuideResult<T> {
    Err(WorkspaceGuideError::TerminalConflict {
        message: message.into(),
    })
}

pub(super) fn storage(message: impl Into<String>) -> WorkspaceGuideError {
    WorkspaceGuideError::Storage {
        message: message.into(),
    }
}
