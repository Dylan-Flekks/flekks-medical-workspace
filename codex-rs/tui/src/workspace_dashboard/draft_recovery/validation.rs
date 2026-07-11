use super::*;
use crate::workspace_draft::RecoveredContextSubmission;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;

use super::super::draft_snapshot::DecodedWorkspaceDraftSnapshot;
use super::super::draft_snapshot::DraftFocusV1;
use super::super::draft_snapshot::decode_workspace_draft_snapshot;

#[path = "validation_selected_context.rs"]
mod selected_context;
use selected_context::validate_selected_context;

#[derive(Debug, Clone)]
struct RecoveredSnapshot {
    schema_version: i64,
    base_client_version: String,
    client: ClientDraft,
    note: NoteDraft,
    focus: DraftFocusV1,
    active_encounter_id: Option<String>,
    agent_request_body: String,
    context_submitted: bool,
    selected_artifact_ids: Vec<String>,
    selected_derivative_ids: Vec<String>,
    selected_clip_ids: Vec<String>,
}

impl RecoveredSnapshot {
    fn decode(checkpoint: &WorkspaceDraftCheckpoint) -> Result<Self> {
        let decoded = decode_workspace_draft_snapshot(checkpoint.draft.clone())
            .map_err(color_eyre::eyre::Report::msg)?;
        let snapshot = match decoded {
            DecodedWorkspaceDraftSnapshot::V1(snapshot) => Self {
                schema_version: snapshot.schema_version,
                base_client_version: snapshot.base_client_version,
                client: snapshot.client,
                note: snapshot.note,
                focus: snapshot.focus,
                active_encounter_id: checkpoint.encounter_id.clone(),
                agent_request_body: String::new(),
                context_submitted: true,
                selected_artifact_ids: Vec::new(),
                selected_derivative_ids: Vec::new(),
                selected_clip_ids: Vec::new(),
            },
            DecodedWorkspaceDraftSnapshot::V2(snapshot) => Self {
                schema_version: snapshot.schema_version,
                base_client_version: snapshot.base_client_version,
                client: snapshot.client,
                note: snapshot.note,
                focus: snapshot.focus,
                active_encounter_id: snapshot.active_encounter_id,
                agent_request_body: snapshot.agent_request_body,
                context_submitted: snapshot.context_submitted,
                selected_artifact_ids: snapshot.selected_artifact_ids,
                selected_derivative_ids: snapshot.selected_derivative_ids,
                selected_clip_ids: snapshot.selected_clip_ids,
            },
        };
        snapshot.validate_envelope(checkpoint)?;
        Ok(snapshot)
    }

    fn validate_envelope(&self, checkpoint: &WorkspaceDraftCheckpoint) -> Result<()> {
        if self.schema_version != checkpoint.schema_version {
            color_eyre::eyre::bail!(
                "draft schema identity changed between checkpoint metadata and content"
            );
        }
        if self.client.id.as_deref() != Some(checkpoint.client_id.as_str())
            || checkpoint.note_id != self.note.id
            || checkpoint.encounter_id != self.active_encounter_id
        {
            color_eyre::eyre::bail!(
                "recovered draft outer and inner scope identities do not match"
            );
        }
        if !is_normalized_nonempty(&self.base_client_version) {
            color_eyre::eyre::bail!("recovered patient baseline identity is empty");
        }
        if self
            .active_encounter_id
            .as_deref()
            .is_some_and(|id| !is_normalized_nonempty(id))
            || self
                .note
                .encounter_id
                .as_deref()
                .is_some_and(|id| !is_normalized_nonempty(id))
        {
            color_eyre::eyre::bail!("recovered encounter identity is not normalized");
        }
        validate_normalized_ids(&self.selected_artifact_ids, "file")?;
        validate_normalized_ids(&self.selected_derivative_ids, "reviewed text")?;
        validate_normalized_ids(&self.selected_clip_ids, "context clip")?;
        if self.context_submitted && !self.agent_request_body.is_empty() {
            color_eyre::eyre::bail!(
                "recovered agent request cannot be both submitted and non-empty"
            );
        }
        match self.note.id.as_deref() {
            Some(note_id)
                if !is_normalized_nonempty(note_id)
                    || self.note.current_revision < 1
                    || checkpoint.base_note_revision != Some(self.note.current_revision)
                    || !is_normalized_nonempty(&self.note.status) =>
            {
                color_eyre::eyre::bail!("recovered note identity is inconsistent");
            }
            Some(_) => {
                if let Some(note_encounter_id) = self.note.encounter_id.as_deref()
                    && self.active_encounter_id.as_deref() != Some(note_encounter_id)
                {
                    color_eyre::eyre::bail!("recovered note and active encounter do not match");
                }
            }
            None if checkpoint.base_note_revision.is_some()
                || self.note.current_revision != 0
                || self.note.encounter_id.is_some()
                || self.note.status.trim() != self.note.status
                || !matches!(self.note.status.trim(), "" | "draft") =>
            {
                color_eyre::eyre::bail!("recovered new-note identity is inconsistent");
            }
            None => {}
        }
        Ok(())
    }

    fn has_context(&self) -> bool {
        !self.agent_request_body.trim().is_empty()
            || !self.selected_artifact_ids.is_empty()
            || !self.selected_derivative_ids.is_empty()
            || !self.selected_clip_ids.is_empty()
    }

    fn context_submission(&self) -> RecoveredContextSubmission {
        if !self.has_context() {
            RecoveredContextSubmission::Empty
        } else if self.context_submitted {
            RecoveredContextSubmission::Submitted
        } else {
            RecoveredContextSubmission::Unsubmitted
        }
    }
}

pub(super) fn validate_recovery_session_envelope(
    session: &WorkspaceDraftSession,
    require_active: bool,
) -> Result<()> {
    if !is_normalized_nonempty(&session.id)
        || !is_normalized_nonempty(&session.client_id)
        || !is_normalized_nonempty(&session.current_checkpoint.id)
        || !is_normalized_nonempty(&session.current_checkpoint.content_sha256)
    {
        color_eyre::eyre::bail!("draft recovery session identity is incomplete");
    }
    if require_active && session.status != WorkspaceDraftSessionStatus::Active {
        color_eyre::eyre::bail!("global draft recovery returned a non-active session");
    }
    let checkpoint = &session.current_checkpoint;
    if checkpoint.session_id != session.id
        || checkpoint.client_id != session.client_id
        || checkpoint.revision != session.current_revision
        || checkpoint.revision < 1
    {
        color_eyre::eyre::bail!("draft recovery session/checkpoint identity is inconsistent");
    }
    let normalized = serde_json::to_string(&checkpoint.draft)?;
    let actual_hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    if actual_hash != checkpoint.content_sha256 {
        color_eyre::eyre::bail!("draft recovery checkpoint content hash is invalid");
    }
    RecoveredSnapshot::decode(checkpoint)?;
    Ok(())
}

pub(super) fn validate_recovery_session_unchanged(
    expected: &WorkspaceDraftSession,
    refreshed: &WorkspaceDraftSession,
) -> Result<()> {
    validate_recovery_session_envelope(refreshed, /*require_active*/ true)?;
    if refreshed.id != expected.id
        || refreshed.client_id != expected.client_id
        || refreshed.current_checkpoint.id != expected.current_checkpoint.id
        || refreshed.current_checkpoint.revision != expected.current_checkpoint.revision
        || refreshed.current_checkpoint.content_sha256 != expected.current_checkpoint.content_sha256
    {
        color_eyre::eyre::bail!(
            "draft changed while recovery data was loading; review the refreshed revision"
        );
    }
    Ok(())
}

pub(super) async fn stage_recovered_dashboard(
    dashboard: &WorkspaceDashboard,
    app_server: &mut AppServerSession,
    session: &WorkspaceDraftSession,
) -> Result<WorkspaceDashboard> {
    validate_recovery_session_envelope(session, /*require_active*/ true)?;
    let snapshot = RecoveredSnapshot::decode(&session.current_checkpoint)?;
    let mut staged = dashboard.clone();
    load_required_baseline(&mut staged, app_server, session, &snapshot).await?;
    validate_snapshot_against_baseline(&staged, session, &snapshot)?;
    apply_recovered_snapshot(&mut staged, session, snapshot)?;
    let degraded = load_optional_enrichment(&mut staged, app_server).await;
    staged.finish_recovery_adoption(&session.id);
    staged.status = if degraded == 0 {
        format!(
            "Recovered local draft r{}; canonical chart unchanged and draft session active.",
            session.current_revision
        )
    } else {
        format!(
            "Recovered local draft r{} with {degraded} optional history view(s) unavailable; draft session active.",
            session.current_revision
        )
    };
    Ok(staged)
}

async fn load_required_baseline(
    staged: &mut WorkspaceDashboard,
    app_server: &mut AppServerSession,
    session: &WorkspaceDraftSession,
    snapshot: &RecoveredSnapshot,
) -> Result<()> {
    staged.clients = app_server.workspace_client_list().await?.clients;
    staged.client_index = staged
        .clients
        .iter()
        .position(|client| client.id == session.client_id)
        .ok_or_else(|| color_eyre::eyre::eyre!("recovered draft patient is not available"))?;
    staged.draft_client = ClientDraft::from_client(&staged.clients[staged.client_index]);

    staged.notes = app_server
        .workspace_note_list(session.client_id.clone())
        .await?
        .notes;
    staged.note_index = match snapshot.note.id.as_deref() {
        Some(note_id) => staged
            .notes
            .iter()
            .position(|note| note.id == note_id)
            .ok_or_else(|| color_eyre::eyre::eyre!("recovered draft note is not available"))?,
        None => staged.notes.len(),
    };
    staged.draft_note = staged
        .notes
        .get(staged.note_index)
        .map(NoteDraft::from_note)
        .unwrap_or_default();

    staged.encounters = app_server
        .workspace_encounter_list(session.client_id.clone())
        .await?
        .encounters;
    staged.encounter_index = match snapshot.active_encounter_id.as_deref() {
        Some(encounter_id) => staged
            .encounters
            .iter()
            .position(|encounter| encounter.id == encounter_id)
            .ok_or_else(|| color_eyre::eyre::eyre!("recovered draft encounter is not available"))?,
        None if !staged.encounters.is_empty() => {
            color_eyre::eyre::bail!(
                "recovered draft had no active encounter, but canonical encounters now exist"
            );
        }
        None => staged.encounters.len(),
    };
    staged.documents = app_server
        .workspace_document_list(session.client_id.clone())
        .await?
        .documents;
    staged.derivatives = app_server
        .workspace_artifact_derivative_list(WorkspaceArtifactDerivativeListParams {
            client_id: session.client_id.clone(),
            document_id: None,
            note_id: None,
            limit: Some(200),
        })
        .await?
        .derivatives;
    staged.clips = app_server
        .workspace_context_clip_list(WorkspaceContextClipListParams {
            client_id: session.client_id.clone(),
            derivative_id: None,
            document_id: None,
            note_id: None,
            limit: Some(200),
        })
        .await?
        .clips;

    staged.selected_artifact_ids.clear();
    staged.selected_derivative_ids.clear();
    staged.selected_clip_ids.clear();
    staged.context_packets.clear();
    staged.agent_results.clear();
    staged.patient_safety_items.clear();
    staged.practice_library_items.clear();
    staged.tasks.clear();
    staged.signatures.clear();
    staged.addenda.clear();
    staged.proposals.clear();
    staged.audit_events.clear();
    staged.practice_library_index = 0;
    staged.proposal_index = 0;
    staged.task_index = 0;
    staged.patient_files_tree_index = 0;
    staged.selected_agent_result_id = None;
    staged.agent_result_inspect = false;
    staged.packet_replay_inspect = false;
    staged.practice_library_inspect = false;
    staged.inspected_artifact_id = None;
    staged.inspected_derivative_id = None;
    staged.inspected_clip_id = None;
    Ok(())
}

fn validate_snapshot_against_baseline(
    staged: &WorkspaceDashboard,
    session: &WorkspaceDraftSession,
    snapshot: &RecoveredSnapshot,
) -> Result<()> {
    let canonical_client = staged
        .clients
        .get(staged.client_index)
        .ok_or_else(|| color_eyre::eyre::eyre!("recovered canonical patient is missing"))?;
    if canonical_client.version != snapshot.base_client_version {
        color_eyre::eyre::bail!(
            "recovered patient baseline is stale; canonical demographics were not changed"
        );
    }

    if let Some(note_id) = snapshot.note.id.as_deref() {
        let canonical_note = staged
            .notes
            .iter()
            .find(|note| note.id == note_id && note.client_id == session.client_id)
            .ok_or_else(|| {
                color_eyre::eyre::eyre!("recovered note ownership could not be verified")
            })?;
        if canonical_note.current_revision != snapshot.note.current_revision
            || canonical_note.status != snapshot.note.status
            || canonical_note.encounter_id != snapshot.note.encounter_id
        {
            color_eyre::eyre::bail!(
                "recovered note baseline is stale or inconsistent; canonical note unchanged"
            );
        }
    }
    if let Some(encounter_id) = snapshot.active_encounter_id.as_deref()
        && !staged.encounters.iter().any(|encounter| {
            encounter.id == encounter_id && encounter.client_id == session.client_id
        })
    {
        color_eyre::eyre::bail!("recovered encounter ownership could not be verified");
    }
    validate_selected_context(staged, session, snapshot)
}

fn validate_normalized_ids(ids: &[String], label: &str) -> Result<()> {
    if ids.iter().any(|id| id.trim().is_empty() || id.trim() != id)
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        color_eyre::eyre::bail!("recovered selected {label} IDs are not normalized and unique");
    }
    Ok(())
}

fn is_normalized_nonempty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn apply_recovered_snapshot(
    staged: &mut WorkspaceDashboard,
    session: &WorkspaceDraftSession,
    snapshot: RecoveredSnapshot,
) -> Result<()> {
    let context_submission = snapshot.context_submission();
    staged.draft_client = snapshot.client;
    staged.draft_note = snapshot.note;
    staged.focus = match snapshot.focus {
        DraftFocusV1::Demographics => WorkspaceFocus::Demographics,
        DraftFocusV1::NoteTitle => WorkspaceFocus::NoteTitle,
        DraftFocusV1::NoteBody => WorkspaceFocus::NoteBody,
        DraftFocusV1::Workflow => WorkspaceFocus::Workflow,
    };
    staged.agent_request.active = !snapshot.agent_request_body.trim().is_empty();
    staged.agent_request.body = snapshot.agent_request_body;
    staged.selected_artifact_ids = snapshot.selected_artifact_ids.into_iter().collect();
    staged.selected_derivative_ids = snapshot.selected_derivative_ids.into_iter().collect();
    staged.selected_clip_ids = snapshot.selected_clip_ids.into_iter().collect();
    staged.stale_context_notice = None;
    staged.draft_document.clear();
    staged.draft_safety.clear();
    staged.derivative_draft.clear();
    staged.clip_draft.clear();
    staged.draft_task.clear();
    staged.addendum_draft.clear();
    staged.agent_result.clear();
    staged.pending_chart_changeset = None;
    staged.next_chart_save_purpose = ChartChangesetPurpose::General;
    staged.dirty = staged.has_checkpointable_patient_or_note_changes();
    staged
        .draft_coordinator
        .adopt_recovered_session(session, context_submission)?;
    Ok(())
}

async fn load_optional_enrichment(
    staged: &mut WorkspaceDashboard,
    app_server: &mut AppServerSession,
) -> usize {
    let Some(client_id) = staged.draft_client.id.clone() else {
        return 0;
    };
    let mut degraded = 0;
    macro_rules! optional_load {
        ($target:ident = $future:expr => $field:ident) => {
            match $future.await {
                Ok(response) => staged.$target = response.$field,
                Err(_) => degraded += 1,
            }
        };
    }
    optional_load!(patient_safety_items = app_server
        .workspace_patient_safety_item_list(client_id.clone()) => items);
    optional_load!(
        practice_library_items = app_server.workspace_practice_library_list(
            WorkspacePracticeLibraryListParams {
            active_client_id: Some(client_id.clone()),
            query: None,
            limit: Some(100),
        }) => items
    );
    optional_load!(tasks = app_server.workspace_task_list(client_id.clone()) => tasks);
    let note_id = staged.draft_note.id.clone();
    optional_load!(
        context_packets = app_server.workspace_context_packet_list(
            WorkspaceContextPacketListParams {
            client_id: client_id.clone(),
            note_id: note_id.clone(),
            limit: Some(8),
        }) => packets
    );
    optional_load!(
        agent_results = app_server.workspace_agent_result_list(WorkspaceAgentResultListParams {
            client_id: client_id.clone(),
            note_id: note_id.clone(),
            packet_id: None,
            limit: Some(8),
        }) => results
    );
    let Some(note_id) = note_id else {
        return degraded;
    };
    optional_load!(signatures = app_server.workspace_note_signature_list(note_id.clone()) => signatures);
    optional_load!(addenda = app_server.workspace_note_addendum_list(note_id.clone()) => addenda);
    optional_load!(proposals = app_server.workspace_note_proposal_list(note_id.clone()) => proposals);
    optional_load!(
        audit_events = app_server.workspace_audit_list(WorkspaceAuditListParams {
            entity_type: None,
            entity_id: None,
            client_id: Some(client_id),
            note_id: Some(note_id),
            cursor: None,
            limit: Some(8),
        }) => data
    );
    staged.selected_agent_result_id = staged.agent_results.first().map(|result| result.id.clone());
    staged.proposal_index = staged
        .proposal_index
        .min(staged.proposals.len().saturating_sub(1));
    staged.task_index = staged.task_index.min(staged.tasks.len().saturating_sub(1));
    degraded
}
