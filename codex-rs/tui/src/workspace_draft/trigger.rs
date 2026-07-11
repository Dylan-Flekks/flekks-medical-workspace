#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceDraftCheckpointTrigger {
    IdleTyping,
    FocusChange,
    ExplicitSave,
    PostCanonicalSave,
    Close,
    Handoff,
    HandoffCleared,
}

impl WorkspaceDraftCheckpointTrigger {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::IdleTyping => "idle_typing",
            Self::FocusChange => "focus_change",
            Self::ExplicitSave => "explicit_save",
            Self::PostCanonicalSave => "post_canonical_save",
            Self::Close => "workspace_close",
            Self::Handoff => "agent_handoff",
            Self::HandoffCleared => "agent_handoff_cleared",
        }
    }

    pub(crate) fn forces_checkpoint(self) -> bool {
        matches!(
            self,
            Self::PostCanonicalSave | Self::Handoff | Self::HandoffCleared
        )
    }
}
