use super::*;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use pretty_assertions::assert_eq;
use std::time::Duration;
use std::time::Instant;

fn working_draft(body: &str) -> MedicalWorkspaceWorkingDraftV1 {
    MedicalWorkspaceWorkingDraftV1::new(MedicalWorkspaceWorkingDraftInput {
        client_id: " client-1 ".to_string(),
        note_id: Some(" note-1 ".to_string()),
        working_note_id: " note-1 ".to_string(),
        encounter_id: Some(" encounter-1 ".to_string()),
        base_note_revision: Some(4),
        note_title: "Daily note".to_string(),
        note_body: body.to_string(),
        agent_request_body: "Suggest a reliable context packet.".to_string(),
        selected_file_ids: vec![
            "file-b".to_string(),
            " file-a ".to_string(),
            "file-a".to_string(),
            String::new(),
        ],
        selected_reviewed_text_ids: vec!["text-1".to_string()],
        selected_clip_ids: vec![" clip-1 ".to_string()],
    })
    .expect("valid test working draft")
}

fn checkpoint(
    draft: &MedicalWorkspaceWorkingDraftV1,
    checkpoint_id: &str,
    revision: i64,
) -> WorkspaceDraftCheckpoint {
    let content_sha256 = draft.content_sha256().expect("valid checkpoint hash");
    WorkspaceDraftCheckpoint {
        id: checkpoint_id.to_string(),
        session_id: "session-1".to_string(),
        client_id: draft.client_id.clone(),
        encounter_id: draft.note.encounter_id.clone(),
        note_id: draft.note.note_id.clone(),
        base_note_revision: draft.note.base_revision,
        schema_version: MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION,
        revision,
        draft: draft.encode().expect("valid checkpoint JSON"),
        content_sha256,
        trigger: "idle_typing".to_string(),
        actor: MEDICAL_WORKSPACE_DRAFT_ACTOR.to_string(),
        created_at: 10,
    }
}

fn active_session(checkpoint: WorkspaceDraftCheckpoint) -> WorkspaceDraftSession {
    WorkspaceDraftSession {
        id: checkpoint.session_id.clone(),
        client_id: checkpoint.client_id.clone(),
        status: WorkspaceDraftSessionStatus::Active,
        current_revision: checkpoint.revision,
        current_checkpoint: checkpoint,
        created_by: MEDICAL_WORKSPACE_DRAFT_ACTOR.to_string(),
        created_at: 10,
        updated_at: 20,
        closed_at: None,
    }
}

fn terminal_session(
    checkpoint: WorkspaceDraftCheckpoint,
    status: WorkspaceDraftSessionStatus,
) -> WorkspaceDraftSession {
    WorkspaceDraftSession {
        id: checkpoint.session_id.clone(),
        client_id: checkpoint.client_id.clone(),
        status,
        current_revision: checkpoint.revision,
        current_checkpoint: checkpoint,
        created_by: MEDICAL_WORKSPACE_DRAFT_ACTOR.to_string(),
        created_at: 10,
        updated_at: 30,
        closed_at: Some(30),
    }
}

#[test]
fn schema_v1_round_trip_has_fixed_kind_and_normalized_context_ids() {
    let draft = working_draft("Line one\nLine two");
    let encoded = draft.encode().expect("schema V1 should encode");

    assert_eq!(
        encoded,
        serde_json::json!({
            "schemaVersion": 1,
            "kind": "medicalWorkspaceWorkingDraft",
            "clientId": "client-1",
            "note": {
                "noteId": "note-1",
                "workingNoteId": "note-1",
                "encounterId": "encounter-1",
                "baseRevision": 4,
                "title": "Daily note",
                "body": "Line one\nLine two"
            },
            "agentRequestBody": "Suggest a reliable context packet.",
            "selectedFileIds": ["file-a", "file-b"],
            "selectedReviewedTextIds": ["text-1"],
            "selectedClipIds": ["clip-1"]
        })
    );
    assert_eq!(
        MedicalWorkspaceWorkingDraftV1::decode(encoded).expect("schema V1 should decode"),
        draft
    );
}

#[test]
fn decoder_rejects_wrong_schema_kind_and_inconsistent_note_baseline() {
    let encoded = working_draft("Body")
        .encode()
        .expect("valid checkpoint JSON");

    let mut wrong_schema = encoded.clone();
    wrong_schema["schemaVersion"] = 2.into();
    assert!(matches!(
        MedicalWorkspaceWorkingDraftV1::decode(wrong_schema),
        Err(WorkspaceDraftError::UnsupportedSchemaVersion(2))
    ));

    let mut wrong_kind = encoded;
    wrong_kind["kind"] = "someOtherDraft".into();
    assert!(matches!(
        MedicalWorkspaceWorkingDraftV1::decode(wrong_kind),
        Err(WorkspaceDraftError::UnsupportedKind(kind)) if kind == "someOtherDraft"
    ));

    let error = MedicalWorkspaceWorkingDraftV1::new(MedicalWorkspaceWorkingDraftInput {
        client_id: "client-1".to_string(),
        note_id: Some("note-1".to_string()),
        working_note_id: "note-1".to_string(),
        encounter_id: None,
        base_note_revision: None,
        note_title: String::new(),
        note_body: String::new(),
        agent_request_body: String::new(),
        selected_file_ids: Vec::new(),
        selected_reviewed_text_ids: Vec::new(),
        selected_clip_ids: Vec::new(),
    })
    .expect_err("saved note ID requires its base revision");
    assert!(
        error
            .to_string()
            .contains("saved note ID and base revision must be present together")
    );
}

#[test]
fn checkpoint_metadata_rejects_tampered_draft_content_hash() {
    let draft = working_draft("Original body");
    let mut persisted = checkpoint(&draft, "checkpoint-tampered", 1);
    persisted.draft["note"]["body"] = "Tampered body".into();

    let error = WorkspaceDraftCheckpointMetadata::from_checkpoint(&persisted)
        .expect_err("tampered draft content must not verify");

    assert!(matches!(
        error,
        WorkspaceDraftError::InvalidCheckpoint(ref message)
            if message.contains("content hash does not match")
    ));
}

#[test]
fn autosave_uses_750ms_generation_tokens_and_invalidates_stale_events() {
    let start = Instant::now();
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");

    let first = state.mark_changed_at(start);
    let second_edit = start + Duration::from_millis(300);
    let second = state.mark_changed_at(second_edit);

    assert_eq!(first.delay, Duration::from_millis(750));
    assert_eq!(second.delay, Duration::from_millis(750));
    assert_eq!(second.token.edit_generation(), 2);
    assert_eq!(
        state.autosave_remaining_at(second.token, second_edit + Duration::from_millis(250)),
        Some(Duration::from_millis(500))
    );
    assert_eq!(
        state.autosave_remaining_at(first.token, second_edit + Duration::from_millis(250)),
        None
    );
    assert!(!state.autosave_is_due_at(first.token, start + Duration::from_secs(2)));
    assert!(!state.autosave_is_due_at(second.token, second_edit + Duration::from_millis(749)));
    assert!(state.autosave_is_due_at(second.token, second_edit + Duration::from_millis(750)));

    state.reset_for_client("client-2");
    assert_ne!(
        second.token.scope_generation(),
        state.current_token().scope_generation()
    );
    assert!(!state.autosave_is_due_at(second.token, start + Duration::from_secs(2)));
}

#[test]
fn failed_idle_checkpoint_pauses_automatic_retry_until_new_activity() {
    let start = Instant::now();
    let draft = working_draft("Body that still needs a checkpoint");
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    let schedule = state.mark_changed_at(start);
    assert!(matches!(
        state
            .begin_checkpoint(
                schedule.token,
                &draft,
                WorkspaceDraftCheckpointTrigger::IdleTyping,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("idle checkpoint should start"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));
    let failed_at = start + WORKSPACE_DRAFT_AUTOSAVE_DELAY;
    state
        .fail_checkpoint_at(
            schedule.token,
            "database unavailable".to_string(),
            failed_at,
        )
        .expect("failed request should return the generation to a retryable state");

    assert!(matches!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::Failed { token, .. } if token == schedule.token
    ));
    assert_eq!(
        state.autosave_remaining_at(schedule.token, failed_at + Duration::from_secs(60)),
        None
    );
    assert!(!state.autosave_is_due_at(schedule.token, failed_at + Duration::from_secs(60)));

    let mut explicit_retry = state.clone();
    assert!(matches!(
        explicit_retry
            .begin_checkpoint(
                schedule.token,
                &draft,
                WorkspaceDraftCheckpointTrigger::FocusChange,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("focus change should still permit an explicit retry"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));

    let next_edit_at = failed_at + Duration::from_secs(1);
    let next = state.mark_changed_at(next_edit_at);
    assert_ne!(next.token, schedule.token);
    assert!(state.autosave_is_due_at(next.token, next_edit_at + WORKSPACE_DRAFT_AUTOSAVE_DELAY));
}

#[test]
fn clean_state_only_forces_a_first_checkpoint_at_packet_boundaries() {
    let draft = working_draft("Body");
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    let token = state.current_token();

    assert_eq!(
        state
            .begin_checkpoint(
                token,
                &draft,
                WorkspaceDraftCheckpointTrigger::PatientNavigation,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("clean navigation should be current"),
        WorkspaceDraftCheckpointStart::AlreadyCurrent
    );
    assert!(matches!(
        state
            .begin_checkpoint(
                token,
                &draft,
                WorkspaceDraftCheckpointTrigger::PacketPreview,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("first packet boundary needs an exact checkpoint"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));
}

#[test]
fn checkpoint_completion_only_acknowledges_its_generation_and_reuses_session() {
    let start = Instant::now();
    let first_draft = working_draft("First body");
    let first_draft_hash = first_draft.content_sha256().expect("first draft hash");
    let second_draft = working_draft("Second body");
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    let first = state.mark_changed_at(start);

    let WorkspaceDraftCheckpointStart::Request(first_request) = state
        .begin_checkpoint(
            first.token,
            &first_draft,
            WorkspaceDraftCheckpointTrigger::IdleTyping,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        )
        .expect("current generation should start")
    else {
        panic!("dirty generation must create a checkpoint request");
    };
    assert_eq!(
        (
            first_request.session_id,
            first_request.expected_current_checkpoint_id,
            first_request.expected_current_checkpoint_revision,
            first_request.expected_current_checkpoint_sha256,
        ),
        (None, None, None, None)
    );
    assert_eq!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::Saving(first.token)
    );

    let second = state.mark_changed_at(start + Duration::from_millis(10));
    let response = WorkspaceDraftCheckpointCreateResponse {
        checkpoint: checkpoint(&first_draft, "checkpoint-1", 1),
        replayed: false,
    };
    let metadata = state
        .complete_checkpoint(first.token, &response)
        .expect("older in-flight generation should remain a valid receipt");

    assert_eq!(metadata.checkpoint_id, "checkpoint-1");
    assert_eq!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::Pending(second.token)
    );
    assert!(state.has_uncheckpointed_changes());

    let WorkspaceDraftCheckpointStart::Request(second_request) = state
        .begin_checkpoint(
            second.token,
            &second_draft,
            WorkspaceDraftCheckpointTrigger::IdleTyping,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        )
        .expect("newer generation should checkpoint")
    else {
        panic!("newer generation must create a checkpoint request");
    };
    assert_eq!(second_request.session_id.as_deref(), Some("session-1"));
    assert_eq!(
        (
            second_request.expected_current_checkpoint_id.as_deref(),
            second_request.expected_current_checkpoint_revision,
            second_request.expected_current_checkpoint_sha256.as_deref(),
        ),
        (
            Some("checkpoint-1"),
            Some(1),
            Some(first_draft_hash.as_str()),
        )
    );
    assert_eq!(second_request.draft["note"]["body"], "Second body");
}

#[test]
fn exact_close_echoes_checkpoint_cas_and_verifies_terminal_response() {
    let draft = working_draft("Saved body");
    let draft_hash = draft.content_sha256().expect("saved draft hash");
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    let schedule = state.mark_changed();
    let WorkspaceDraftCheckpointStart::Request(_) = state
        .begin_checkpoint(
            schedule.token,
            &draft,
            WorkspaceDraftCheckpointTrigger::ExplicitSave,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        )
        .expect("checkpoint should start")
    else {
        panic!("dirty state should create a checkpoint request");
    };
    let persisted = checkpoint(&draft, "checkpoint-7", 7);
    state
        .complete_checkpoint(
            schedule.token,
            &WorkspaceDraftCheckpointCreateResponse {
                checkpoint: persisted.clone(),
                replayed: false,
            },
        )
        .expect("checkpoint should be confirmed");

    let mut changed_session_state = state.clone();
    let changed_draft = working_draft("Changed body");
    let changed_schedule = changed_session_state.mark_changed();
    assert!(matches!(
        changed_session_state
            .begin_checkpoint(
                changed_schedule.token,
                &changed_draft,
                WorkspaceDraftCheckpointTrigger::IdleTyping,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("new generation should start in the confirmed session"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));
    let mut changed_session_checkpoint = checkpoint(&changed_draft, "checkpoint-other", 8);
    changed_session_checkpoint.session_id = "session-other".to_string();
    assert!(
        changed_session_state
            .complete_checkpoint(
                changed_schedule.token,
                &WorkspaceDraftCheckpointCreateResponse {
                    checkpoint: changed_session_checkpoint,
                    replayed: false,
                },
            )
            .expect_err("checkpoint response cannot change the active session")
            .to_string()
            .contains("changed the active draft session")
    );
    assert_eq!(
        changed_session_state
            .confirmed_checkpoint()
            .map(|checkpoint| checkpoint.checkpoint_id.as_str()),
        Some("checkpoint-7")
    );

    let params = state
        .exact_close_params(
            WorkspaceDraftCloseDisposition::Closed,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
            "agent handoff completed",
        )
        .expect("clean confirmed checkpoint should close exactly");
    assert_eq!(
        params,
        WorkspaceDraftSessionCloseParams {
            session_id: "session-1".to_string(),
            client_id: "client-1".to_string(),
            status: WorkspaceDraftSessionCloseStatus::Closed,
            expected_current_checkpoint_id: Some("checkpoint-7".to_string()),
            expected_current_checkpoint_revision: Some(7),
            expected_current_checkpoint_sha256: Some(draft_hash),
            actor: MEDICAL_WORKSPACE_DRAFT_ACTOR.to_string(),
            reason: "agent handoff completed".to_string(),
        }
    );

    let mut wrong_scope = terminal_session(persisted.clone(), WorkspaceDraftSessionStatus::Closed);
    wrong_scope.current_checkpoint.note_id = Some("note-other".to_string());
    assert!(
        state
            .clone()
            .confirm_closed(&wrong_scope, WorkspaceDraftCloseDisposition::Closed)
            .is_err()
    );

    state
        .confirm_closed(
            &terminal_session(persisted, WorkspaceDraftSessionStatus::Closed),
            WorkspaceDraftCloseDisposition::Closed,
        )
        .expect("terminal response should echo exact checkpoint identity");
    assert_eq!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::Idle
    );
}

#[test]
fn invalid_checkpoint_response_stays_failed_and_retryable() {
    let requested = working_draft("Requested body");
    let unexpected = working_draft("Unexpected body");
    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    let schedule = state.mark_changed();
    assert!(matches!(
        state
            .begin_checkpoint(
                schedule.token,
                &requested,
                WorkspaceDraftCheckpointTrigger::IdleTyping,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("checkpoint should start"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));

    let error = state
        .complete_checkpoint(
            schedule.token,
            &WorkspaceDraftCheckpointCreateResponse {
                checkpoint: checkpoint(&unexpected, "checkpoint-wrong", 1),
                replayed: false,
            },
        )
        .expect_err("different persisted content must fail closed");

    assert!(
        error
            .to_string()
            .contains("did not contain the requested working draft")
    );
    assert!(matches!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::Failed { token, .. } if token == schedule.token
    ));
    assert!(matches!(
        state
            .begin_checkpoint(
                schedule.token,
                &requested,
                WorkspaceDraftCheckpointTrigger::ExplicitSave,
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
            )
            .expect("failed response must leave the generation retryable"),
        WorkspaceDraftCheckpointStart::Request(_)
    ));
}

#[test]
fn recovery_is_typed_note_scoped_and_discard_is_exact_cas() {
    let draft = working_draft("Recovered body");
    let draft_hash = draft.content_sha256().expect("recovered draft hash");
    let persisted = checkpoint(&draft, "checkpoint-recovery", 3);
    let recovery = RecoverableMedicalWorkspaceDraft::try_from(active_session(persisted.clone()))
        .expect("active consistent session should decode");

    assert!(recovery.matches_note_scope(Some("note-1"), Some("encounter-1")));
    assert!(!recovery.matches_note_scope(Some("note-other"), Some("encounter-1")));
    assert!(!recovery.matches_note_scope(Some("note-1"), Some("encounter-other")));
    assert!(recovery.matches_working_note_scope(Some("note-1"), Some("encounter-1"), "note-1"));
    assert!(!recovery.matches_working_note_scope(
        Some("note-1"),
        Some("encounter-1"),
        "working-note-other"
    ));
    let discard = recovery
        .discard_params(
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
            "clinician discarded local draft",
        )
        .expect("recovery discard should use exact checkpoint CAS");
    assert_eq!(
        (
            discard.status,
            discard.expected_current_checkpoint_id.as_deref(),
            discard.expected_current_checkpoint_revision,
            discard.expected_current_checkpoint_sha256.as_deref(),
        ),
        (
            WorkspaceDraftSessionCloseStatus::Discarded,
            Some("checkpoint-recovery"),
            Some(3),
            Some(draft_hash.as_str()),
        )
    );

    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    state
        .offer_recovery(recovery.clone())
        .expect("clean matching patient scope should offer recovery");
    assert_eq!(
        state.persistence_status(),
        WorkspaceDraftPersistenceStatus::RecoveryAvailable(recovery.checkpoint.clone())
    );
    let recovery_token = state.current_token();
    assert!(matches!(
        state.begin_checkpoint(
            recovery_token,
            &draft,
            WorkspaceDraftCheckpointTrigger::PacketPreview,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        ),
        Err(WorkspaceDraftError::InvalidRecovery(message))
            if message.contains("resolve the offered recovery")
    ));
    let mut discarded_state = state.clone();
    discarded_state
        .confirm_recovery_discarded(&terminal_session(
            persisted,
            WorkspaceDraftSessionStatus::Discarded,
        ))
        .expect("discard response should match the recovery checkpoint exactly");
    assert_eq!(discarded_state.pending_recovery(), None);
    assert_eq!(
        state
            .adopt_recovery()
            .expect("offered recovery should adopt"),
        draft
    );
    assert_eq!(state.confirmed_checkpoint(), Some(&recovery.checkpoint));

    let updated = working_draft("Updated after recovery");
    let schedule = state.mark_changed();
    let WorkspaceDraftCheckpointStart::Request(request) = state
        .begin_checkpoint(
            schedule.token,
            &updated,
            WorkspaceDraftCheckpointTrigger::IdleTyping,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        )
        .expect("adopted recovery should append with its exact head CAS")
    else {
        panic!("updated recovery must create a checkpoint request");
    };
    assert_eq!(
        (
            request.session_id.as_deref(),
            request.expected_current_checkpoint_id.as_deref(),
            request.expected_current_checkpoint_revision,
            request.expected_current_checkpoint_sha256.as_deref(),
        ),
        (
            Some("session-1"),
            Some("checkpoint-recovery"),
            Some(3),
            Some(draft_hash.as_str()),
        )
    );
}

#[test]
fn malformed_recovery_and_dirty_close_fail_closed() {
    let draft = working_draft("Body");
    let persisted = checkpoint(&draft, "checkpoint-1", 1);
    let mut malformed = active_session(persisted);
    malformed.current_revision = 2;
    assert!(matches!(
        RecoverableMedicalWorkspaceDraft::try_from(malformed),
        Err(WorkspaceDraftError::InvalidRecovery(_))
    ));

    let mut state = WorkspaceDraftState::default();
    state.reset_for_client("client-1");
    state.mark_changed();
    assert!(matches!(
        state.exact_close_params(
            WorkspaceDraftCloseDisposition::Discarded,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
            "discard after checkpoint flush"
        ),
        Err(WorkspaceDraftError::UncheckpointedClose)
    ));
}
