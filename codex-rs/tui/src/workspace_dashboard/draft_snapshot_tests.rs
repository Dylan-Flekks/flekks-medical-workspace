use super::*;

fn test_client(id: &str) -> WorkspaceClient {
    WorkspaceClient {
        id: id.to_string(),
        version: format!("version-{id}"),
        display_name: "Jordan Patient".to_string(),
        preferred_name: None,
        date_of_birth: None,
        sex_or_gender: None,
        external_id: None,
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

fn test_encounter(id: &str, client_id: &str) -> WorkspaceEncounter {
    WorkspaceEncounter {
        id: id.to_string(),
        version: format!("version-{id}"),
        client_id: client_id.to_string(),
        kind: "visit".to_string(),
        title: "Daily visit".to_string(),
        status: "open".to_string(),
        started_at: Some(1),
        ended_at: None,
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn test_document(id: &str, client_id: &str) -> WorkspaceDocument {
    WorkspaceDocument {
        id: id.to_string(),
        version: format!("version-{id}"),
        client_id: client_id.to_string(),
        encounter_id: Some("encounter-1".to_string()),
        title: "PRIVATE_DOCUMENT_SUMMARY".to_string(),
        kind: "pdf".to_string(),
        local_path: "/private/PATH_MUST_NOT_LEAK.pdf".to_string(),
        notes: "PRIVATE_DOCUMENT_NOTES".to_string(),
        scope: "patient".to_string(),
        detected_kind: "pdf".to_string(),
        mime_type: Some("application/pdf".to_string()),
        file_size_bytes: Some(42),
        modified_at: Some(1),
        sha256: None,
        tags: String::new(),
        source_label: String::new(),
        existence_status: "available".to_string(),
        metadata_json: "{}".to_string(),
        original_path: "/private/ORIGINAL_MUST_NOT_LEAK.pdf".to_string(),
        reference_kind: "local_reference".to_string(),
        vault_path: "/private/VAULT_MUST_NOT_LEAK.pdf".to_string(),
        content_sha256: None,
        thumbnail_path: "/private/THUMB_MUST_NOT_LEAK.png".to_string(),
        thumbnail_status: "ready".to_string(),
        thumbnail_mime_type: Some("image/png".to_string()),
        intake_source: "test".to_string(),
        imported_at: Some(1),
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn test_derivative(id: &str, document_id: &str, client_id: &str) -> WorkspaceArtifactDerivative {
    WorkspaceArtifactDerivative {
        id: id.to_string(),
        version: format!("version-{id}"),
        document_id: document_id.to_string(),
        client_id: client_id.to_string(),
        encounter_id: Some("encounter-1".to_string()),
        note_id: None,
        kind: "reviewed_text".to_string(),
        title: "PRIVATE_DERIVATIVE_SUMMARY".to_string(),
        body: "PRIVATE_DERIVATIVE_BODY".to_string(),
        review_status: "reviewed".to_string(),
        source_method: "human_pasted".to_string(),
        page_range: String::new(),
        timestamp_range: String::new(),
        segment_label: String::new(),
        tags: String::new(),
        metadata_json: "{}".to_string(),
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn test_clip(
    id: &str,
    derivative_id: &str,
    document_id: &str,
    client_id: &str,
) -> WorkspaceContextClip {
    WorkspaceContextClip {
        id: id.to_string(),
        version: format!("version-{id}"),
        derivative_id: derivative_id.to_string(),
        document_id: document_id.to_string(),
        client_id: client_id.to_string(),
        encounter_id: Some("encounter-1".to_string()),
        note_id: None,
        kind: "excerpt".to_string(),
        title: "PRIVATE_CLIP_SUMMARY".to_string(),
        body: "PRIVATE_CLIP_BODY".to_string(),
        review_status: "reviewed".to_string(),
        source_method: "human_selected".to_string(),
        page_range: String::new(),
        timestamp_range: String::new(),
        line_range: String::new(),
        segment_label: String::new(),
        tags: String::new(),
        metadata_json: "{}".to_string(),
        archived_at: None,
        created_at: 1,
        updated_at: 1,
    }
}

fn snapshot_dashboard() -> WorkspaceDashboard {
    let client = test_client("client-1");
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.draft_client = ClientDraft::from_client(&client);
    dashboard.clients = vec![client];
    dashboard.encounters = vec![test_encounter("encounter-1", "client-1")];
    dashboard.encounter_index = 0;
    dashboard.draft_note = NoteDraft {
        id: None,
        encounter_id: None,
        title: "New daily note".to_string(),
        body: "Human canonical note draft".to_string(),
        status: "draft".to_string(),
        current_revision: 0,
    };
    dashboard.documents = vec![
        test_document("artifact-b", "client-1"),
        test_document("artifact-a", "client-1"),
    ];
    dashboard.derivatives = vec![test_derivative("derivative-a", "artifact-a", "client-1")];
    dashboard.clips = vec![test_clip(
        "clip-a",
        "derivative-a",
        "artifact-a",
        "client-1",
    )];
    dashboard.agent_request.body = "Draft a similar daily note template.".to_string();
    dashboard.selected_artifact_ids = [
        "artifact-b".to_string(),
        " artifact-a ".to_string(),
        "artifact-a".to_string(),
    ]
    .into_iter()
    .collect();
    dashboard.selected_derivative_ids = [" derivative-a ".to_string()].into_iter().collect();
    dashboard.selected_clip_ids = ["clip-a".to_string()].into_iter().collect();
    dashboard
}

#[test]
fn v2_snapshot_is_deterministic_id_only_and_encounter_consistent() {
    let dashboard = snapshot_dashboard();

    let first = dashboard
        .draft_checkpoint_input()
        .expect("valid V2 snapshot");
    let second = dashboard
        .draft_checkpoint_input()
        .expect("deterministic V2 snapshot");

    assert_eq!(first.draft, second.draft);
    assert_eq!(first.encounter_id.as_deref(), Some("encounter-1"));
    assert_eq!(first.draft["schemaVersion"], 2);
    assert_eq!(first.draft["activeEncounterId"], "encounter-1");
    assert_eq!(
        first.draft["agentRequestBody"],
        "Draft a similar daily note template."
    );
    assert_eq!(
        first.draft["selectedArtifactIds"],
        serde_json::json!(["artifact-a", "artifact-b"])
    );
    assert_eq!(
        first.draft["selectedDerivativeIds"],
        serde_json::json!(["derivative-a"])
    );
    assert_eq!(
        first.draft["selectedClipIds"],
        serde_json::json!(["clip-a"])
    );
    let encoded = serde_json::to_string(&first.draft).expect("snapshot JSON");
    for private_value in [
        "PATH_MUST_NOT_LEAK",
        "ORIGINAL_MUST_NOT_LEAK",
        "VAULT_MUST_NOT_LEAK",
        "THUMB_MUST_NOT_LEAK",
        "PRIVATE_DOCUMENT_SUMMARY",
        "PRIVATE_DOCUMENT_NOTES",
        "PRIVATE_DERIVATIVE_SUMMARY",
        "PRIVATE_DERIVATIVE_BODY",
        "PRIVATE_CLIP_SUMMARY",
        "PRIVATE_CLIP_BODY",
    ] {
        assert!(!encoded.contains(private_value), "leaked {private_value}");
    }
    assert!(matches!(
        decode_workspace_draft_snapshot(first.draft),
        Ok(DecodedWorkspaceDraftSnapshot::V2(_))
    ));
}

#[test]
fn legacy_v1_snapshot_still_decodes() {
    let dashboard = snapshot_dashboard();
    let legacy = serde_json::to_value(WorkspaceDraftSnapshotV1 {
        schema_version: 1,
        base_client_version: "version-client-1".to_string(),
        client: dashboard.draft_client,
        note: dashboard.draft_note,
        focus: DraftFocusV1::Workflow,
    })
    .expect("legacy snapshot JSON");

    assert!(matches!(
        decode_workspace_draft_snapshot(legacy),
        Ok(DecodedWorkspaceDraftSnapshot::V1(_))
    ));
}

#[test]
fn new_note_without_selected_encounter_encodes_null_consistently() {
    let mut dashboard = snapshot_dashboard();
    dashboard.encounters.clear();

    let input = dashboard
        .draft_checkpoint_input()
        .expect("encounterless V2 snapshot");

    assert_eq!(input.encounter_id, None);
    assert_eq!(input.draft["activeEncounterId"], serde_json::Value::Null);
}

#[test]
fn existing_note_requires_its_loaded_owned_selected_encounter() {
    let mut dashboard = snapshot_dashboard();
    dashboard.draft_note.id = Some("note-1".to_string());
    dashboard.draft_note.encounter_id = Some("encounter-note".to_string());
    dashboard
        .encounters
        .push(test_encounter("encounter-note", "client-1"));

    let mismatch = dashboard
        .draft_checkpoint_input()
        .expect_err("selected encounter must match an existing note");
    assert!(mismatch.contains("selected encounter does not match"));

    dashboard.encounter_index = 1;
    dashboard.encounters[1].client_id = "client-other".to_string();
    let wrong_patient = dashboard
        .draft_checkpoint_input()
        .expect_err("note encounter must belong to the active patient");
    assert!(wrong_patient.contains("does not belong to the active patient"));
}

#[test]
fn v2_snapshot_rejects_oversized_agent_request() {
    let mut dashboard = snapshot_dashboard();
    dashboard.agent_request.body = "x".repeat(MAX_DRAFT_SNAPSHOT_BYTES + 1);

    let error = dashboard
        .draft_checkpoint_input()
        .expect_err("oversized snapshot must fail closed");

    assert!(error.contains("exceeds the 1048576 byte limit"));
}
