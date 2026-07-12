//! Typed, local-only recovery state for the medical workspace.
//!
//! This module deliberately stops at JSON-RPC request construction and pure
//! state transitions. It never commits canonical chart data or starts agent
//! work.

mod model;
mod state;

pub(crate) use model::MEDICAL_WORKSPACE_DRAFT_ACTOR;
pub(crate) use model::MedicalWorkspaceWorkingDraftInput;
pub(crate) use model::MedicalWorkspaceWorkingDraftV1;
pub(crate) use model::RecoverableMedicalWorkspaceDraft;
pub(crate) use model::WORKSPACE_DRAFT_AUTOSAVE_DELAY;
pub(crate) use model::WorkspaceDraftCheckpointTrigger;
pub(crate) use model::WorkspaceDraftCloseDisposition;
pub(crate) use model::WorkspaceDraftError;
pub(crate) use state::WorkspaceDraftCheckpointStart;
pub(crate) use state::WorkspaceDraftGenerationToken;
pub(crate) use state::WorkspaceDraftState;

// These complete the private facade for callers that want to render persistence
// state without reaching into the implementation modules.
#[allow(unused_imports)]
pub(crate) use model::{
    MEDICAL_WORKSPACE_DRAFT_KIND, MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION,
    WorkspaceDraftCheckpointMetadata,
};
#[allow(unused_imports)]
pub(crate) use state::{WorkspaceDraftAutosaveSchedule, WorkspaceDraftPersistenceStatus};

#[cfg(test)]
#[path = "workspace_draft_tests.rs"]
mod tests;
