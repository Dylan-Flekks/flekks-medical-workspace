//! Typed, local-only recovery state for the medical workspace.
//!
//! This module deliberately stops at JSON-RPC request construction and pure
//! state transitions. It never commits canonical chart data or starts agent
//! work.

mod context_plan;
mod model;
mod state;

pub(crate) use context_plan::MEDICAL_CONTEXT_PLAN_SCHEMA_VERSION;
#[allow(unused_imports)]
pub(crate) use context_plan::MedicalContextPlanAcknowledgementV2;
#[allow(unused_imports)]
pub(crate) use context_plan::MedicalContextPlanAuthorizedCategoryV2;
pub(crate) use context_plan::MedicalContextPlanAuthorizedScopeV2;
pub(crate) use context_plan::MedicalContextPlanInput;
pub(crate) use context_plan::MedicalContextPlanNoteKindV2;
pub(crate) use context_plan::MedicalContextPlanWarningV2;
pub(crate) use context_plan::MedicalContextPlanWorkflowV2;
pub(crate) use model::MEDICAL_WORKSPACE_DRAFT_ACTOR;
pub(crate) use model::MedicalWorkspaceWorkingDraftInput;
pub(crate) use model::MedicalWorkspaceWorkingDraftV1;
pub(crate) use model::RecoverableMedicalWorkspaceDraft;
#[cfg(test)]
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
pub(crate) use model::MEDICAL_WORKSPACE_DRAFT_KIND;
#[allow(unused_imports)]
pub(crate) use model::MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION;
#[allow(unused_imports)]
pub(crate) use model::WorkspaceDraftCheckpointMetadata;
#[allow(unused_imports)]
pub(crate) use state::WorkspaceDraftAutosaveSchedule;
#[allow(unused_imports)]
pub(crate) use state::WorkspaceDraftPersistenceStatus;

#[cfg(test)]
#[path = "workspace_draft_tests.rs"]
mod tests;
