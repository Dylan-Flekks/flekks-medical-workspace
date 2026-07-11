use super::*;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use pretty_assertions::assert_eq;

#[test]
fn schema_v1_round_trip_restores_new_note_without_touching_canonical_data() {
    let mut dashboard = saved_patient_dashboard();
    dashboard.draft_note.title = "Daily note".to_string();
    dashboard.draft_note.body = "Human draft body".to_string();
    dashboard.focus = WorkspaceFocus::NoteBody;
    let input = dashboard
        .draft_checkpoint_input()
        .expect("saved patient draft should checkpoint");
    let recovery = recovery_session(input, /*revision*/ 3);

    let snapshot = dashboard
        .validate_recovery_snapshot(&recovery)
        .expect("new-note baseline should restore");
    dashboard.draft_note = NoteDraft::default();
    dashboard.apply_recovery_snapshot(snapshot);

    assert_eq!(dashboard.draft_note.title, "Daily note");
    assert_eq!(dashboard.draft_note.body, "Human draft body");
    assert_eq!(dashboard.note_index, dashboard.notes.len());
    assert!(dashboard.dirty);
}

#[test]
fn stale_note_revision_is_blocked_without_merging_draft() {
    let mut dashboard = saved_patient_dashboard();
    dashboard.notes = vec![saved_note(/*revision*/ 4)];
    dashboard.note_index = 0;
    dashboard.draft_note = NoteDraft::from_note(&dashboard.notes[0]);
    dashboard.draft_note.body = "Checkpoint text".to_string();
    let input = dashboard
        .draft_checkpoint_input()
        .expect("saved note should checkpoint");
    let recovery = recovery_session(input, /*revision*/ 2);
    dashboard.notes[0].current_revision = 5;

    let error = dashboard
        .validate_recovery_snapshot(&recovery)
        .expect_err("changed canonical note must block restore");

    assert_eq!(
        error,
        "Restore blocked: canonical note revision changed; no draft was merged."
    );
}

#[test]
fn recovery_prompt_explains_restore_discard_and_canonical_boundary() {
    let mut dashboard = saved_patient_dashboard();
    dashboard.draft_note.title = "Daily note".to_string();
    let input = dashboard
        .draft_checkpoint_input()
        .expect("saved patient draft should checkpoint");
    let recovery = recovery_session(input, /*revision*/ 7);
    dashboard.draft_coordinator.set_recovery_for_tests(recovery);

    insta::assert_snapshot!(
        "medical_workspace_draft_recovery_120x32",
        render_dashboard(&dashboard, 120, 32)
    );
}

fn saved_patient_dashboard() -> WorkspaceDashboard {
    let client = saved_client();
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.clients = vec![client.clone()];
    dashboard.client_index = 0;
    dashboard.draft_client = ClientDraft::from_client(&client);
    dashboard.status = "Loaded.".to_string();
    dashboard
}

fn saved_client() -> WorkspaceClient {
    WorkspaceClient {
        id: "patient-1".to_string(),
        version: "patient-v1".to_string(),
        display_name: "Jordan Test".to_string(),
        preferred_name: Some("Jordan".to_string()),
        date_of_birth: Some("1980-01-02".to_string()),
        sex_or_gender: None,
        external_id: Some("MRN-TEST-1".to_string()),
        record_start_date: None,
        record_end_date: None,
        summary: String::new(),
        primary_phone: None,
        secondary_phone: None,
        email: None,
        preferred_contact_method: None,
        emergency_contact_name: None,
        emergency_contact_relationship: None,
        emergency_contact_phone: None,
        emergency_contact_email: None,
        contact_notes: None,
        payer_name: None,
        plan_name: None,
        member_id: None,
        group_number: None,
        coverage_type: None,
        coverage_status: None,
        coverage_notes: None,
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn saved_note(revision: i64) -> WorkspaceNote {
    WorkspaceNote {
        id: "note-1".to_string(),
        client_id: "patient-1".to_string(),
        encounter_id: Some("encounter-1".to_string()),
        title: "Progress note".to_string(),
        kind: "daily".to_string(),
        body: "Canonical body".to_string(),
        status: "draft".to_string(),
        current_revision: revision,
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn recovery_session(input: WorkspaceDraftCheckpointInput, revision: i64) -> WorkspaceDraftSession {
    WorkspaceDraftSession {
        id: "draft-session-1".to_string(),
        client_id: input.client_id.clone(),
        status: WorkspaceDraftSessionStatus::Active,
        current_revision: revision,
        current_checkpoint: WorkspaceDraftCheckpoint {
            id: "checkpoint-1".to_string(),
            session_id: "draft-session-1".to_string(),
            client_id: input.client_id,
            encounter_id: input.encounter_id,
            note_id: input.note_id,
            base_note_revision: input.base_note_revision,
            schema_version: DRAFT_SCHEMA_VERSION,
            revision,
            draft: input.draft,
            content_sha256: "0".repeat(64),
            trigger: "idle_typing".to_string(),
            actor: "medical workspace TUI".to_string(),
            created_at: 1,
        },
        created_by: "medical workspace TUI".to_string(),
        created_at: 1,
        updated_at: 1,
        closed_at: None,
    }
}

fn render_dashboard(dashboard: &WorkspaceDashboard, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buffer = Buffer::empty(area);
    dashboard.render(area, &mut buffer);
    (0..height)
        .map(|row| {
            let line = (0..width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>();
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
