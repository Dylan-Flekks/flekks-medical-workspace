use super::*;
use crate::workspace_draft::WorkspaceDraftCheckpointInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const DRAFT_SCHEMA_VERSION_V2: i64 = 2;
const MAX_DRAFT_SNAPSHOT_BYTES: usize = 1024 * 1024;
type SelectedContextIds = (Vec<String>, Vec<String>, Vec<String>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum DraftFocusV1 {
    Demographics,
    NoteTitle,
    NoteBody,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WorkspaceDraftSnapshotV1 {
    pub(super) schema_version: i64,
    pub(super) base_client_version: String,
    pub(super) client: ClientDraft,
    pub(super) note: NoteDraft,
    pub(super) focus: DraftFocusV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WorkspaceDraftSnapshotV2 {
    pub(super) schema_version: i64,
    pub(super) base_client_version: String,
    pub(super) client: ClientDraft,
    pub(super) note: NoteDraft,
    pub(super) focus: DraftFocusV1,
    pub(super) active_encounter_id: Option<String>,
    pub(super) agent_request_body: String,
    #[serde(default)]
    pub(super) context_submitted: bool,
    pub(super) selected_artifact_ids: Vec<String>,
    pub(super) selected_derivative_ids: Vec<String>,
    pub(super) selected_clip_ids: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) enum DecodedWorkspaceDraftSnapshot {
    V1(WorkspaceDraftSnapshotV1),
    V2(WorkspaceDraftSnapshotV2),
}

impl WorkspaceDashboard {
    pub(super) fn draft_checkpoint_input(
        &self,
    ) -> std::result::Result<WorkspaceDraftCheckpointInput, String> {
        let Some(client_id) = self.draft_client.id.clone() else {
            return Err(
                "Save this new patient before local draft checkpointing is available; canonical chart unchanged."
                    .to_string(),
            );
        };
        let canonical_client = self
            .clients
            .iter()
            .find(|client| client.id == client_id)
            .ok_or_else(|| {
                "Reload the saved patient before checkpointing this draft.".to_string()
            })?;
        let active_encounter_id = self.checkpoint_active_encounter_id(&client_id)?;
        let (selected_artifact_ids, selected_derivative_ids, selected_clip_ids) =
            self.checkpoint_selected_context_ids(&client_id)?;
        let snapshot = WorkspaceDraftSnapshotV2 {
            schema_version: DRAFT_SCHEMA_VERSION_V2,
            base_client_version: canonical_client.version.clone(),
            client: self.draft_client.clone(),
            note: self.draft_note.clone(),
            focus: DraftFocusV1::from_dashboard(self),
            active_encounter_id: active_encounter_id.clone(),
            agent_request_body: self.agent_request.body.clone(),
            context_submitted: self.draft_coordinator.context_is_submitted(),
            selected_artifact_ids,
            selected_derivative_ids,
            selected_clip_ids,
        };
        let encoded = serde_json::to_vec(&snapshot)
            .map_err(|error| format!("Could not encode local draft checkpoint: {error}"))?;
        if encoded.len() > MAX_DRAFT_SNAPSHOT_BYTES {
            return Err(format!(
                "Local draft checkpoint exceeds the {MAX_DRAFT_SNAPSHOT_BYTES} byte limit."
            ));
        }
        let draft = serde_json::from_slice(&encoded)
            .map_err(|error| format!("Could not finalize local draft checkpoint: {error}"))?;
        Ok(WorkspaceDraftCheckpointInput {
            client_id,
            encounter_id: active_encounter_id,
            note_id: self.draft_note.id.clone(),
            base_note_revision: self
                .draft_note
                .id
                .as_ref()
                .map(|_| self.draft_note.current_revision),
            draft,
        })
    }

    fn checkpoint_active_encounter_id(
        &self,
        client_id: &str,
    ) -> std::result::Result<Option<String>, String> {
        let selected = self.encounters.get(self.encounter_index);
        if self.draft_note.id.is_some()
            && let Some(note_encounter_id) = self.draft_note.encounter_id.as_deref()
        {
            let note_encounter = self
                .encounters
                .iter()
                .find(|encounter| encounter.id == note_encounter_id)
                .ok_or_else(|| {
                    "The note encounter is not loaded; reload before checkpointing.".to_string()
                })?;
            if note_encounter.client_id != client_id {
                return Err(
                    "The note encounter does not belong to the active patient; reload the chart."
                        .to_string(),
                );
            }
            if selected.map(|encounter| encounter.id.as_str()) != Some(note_encounter_id) {
                return Err(
                    "The selected encounter does not match this note; reselect the note or reload the chart."
                        .to_string(),
                );
            }
            return Ok(Some(note_encounter_id.to_string()));
        }
        let Some(encounter) = selected else {
            return Ok(None);
        };
        if encounter.client_id != client_id {
            return Err(
                "The selected encounter does not belong to the active patient; reload the chart."
                    .to_string(),
            );
        }
        Ok(Some(encounter.id.clone()))
    }

    fn checkpoint_selected_context_ids(
        &self,
        client_id: &str,
    ) -> std::result::Result<SelectedContextIds, String> {
        let artifact_ids = normalized_selected_ids(&self.selected_artifact_ids);
        let derivative_ids = normalized_selected_ids(&self.selected_derivative_ids);
        let clip_ids = normalized_selected_ids(&self.selected_clip_ids);
        if artifact_ids.iter().any(|id| {
            !self.documents.iter().any(|document| {
                document.id == *id
                    && document.client_id == client_id
                    && artifact_scope_label(document) == "patient"
            })
        }) {
            return Err(
                "A selected file reference is outside the active patient; reload context selections."
                    .to_string(),
            );
        }
        if derivative_ids.iter().any(|id| {
            !self
                .derivatives
                .iter()
                .any(|derivative| derivative.id == *id && derivative.client_id == client_id)
        }) {
            return Err(
                "Selected reviewed text is outside the active patient; reload context selections."
                    .to_string(),
            );
        }
        if clip_ids.iter().any(|id| {
            !self
                .clips
                .iter()
                .any(|clip| clip.id == *id && clip.client_id == client_id)
        }) {
            return Err(
                "A selected context clip is outside the active patient; reload context selections."
                    .to_string(),
            );
        }
        Ok((artifact_ids, derivative_ids, clip_ids))
    }
}

impl DraftFocusV1 {
    fn from_dashboard(dashboard: &WorkspaceDashboard) -> Self {
        match dashboard.focus {
            WorkspaceFocus::Demographics => Self::Demographics,
            WorkspaceFocus::NoteTitle => Self::NoteTitle,
            WorkspaceFocus::NoteBody => Self::NoteBody,
            WorkspaceFocus::Clients
            | WorkspaceFocus::Notes
            | WorkspaceFocus::Workflow
            | WorkspaceFocus::Agent
            | WorkspaceFocus::PatientFiles => Self::Workflow,
        }
    }
}

fn normalized_selected_ids(ids: &BTreeSet<String>) -> Vec<String> {
    let mut normalized = ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[allow(dead_code)]
pub(super) fn decode_workspace_draft_snapshot(
    draft: Value,
) -> std::result::Result<DecodedWorkspaceDraftSnapshot, String> {
    match draft.get("schemaVersion").and_then(Value::as_i64) {
        Some(1) => serde_json::from_value(draft)
            .map(DecodedWorkspaceDraftSnapshot::V1)
            .map_err(|error| format!("Could not decode schemaVersion 1 draft checkpoint: {error}")),
        Some(2) => serde_json::from_value(draft)
            .map(DecodedWorkspaceDraftSnapshot::V2)
            .map_err(|error| format!("Could not decode schemaVersion 2 draft checkpoint: {error}")),
        Some(version) => Err(format!(
            "Unsupported workspace draft checkpoint schemaVersion {version}."
        )),
        None => Err("Workspace draft checkpoint is missing schemaVersion.".to_string()),
    }
}

#[cfg(test)]
#[path = "draft_snapshot_tests.rs"]
mod tests;
