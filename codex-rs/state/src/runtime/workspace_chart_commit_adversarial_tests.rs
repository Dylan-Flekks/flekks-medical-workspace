use super::*;
use crate::StateRuntime;
use crate::WorkspaceArtifactDerivativeFilter;
use crate::WorkspaceArtifactDerivativeUpsert;
use crate::WorkspaceAuditEvent;
use crate::WorkspaceAuditEventFilter;
use crate::WorkspaceChartNoteChange;
use crate::WorkspaceClient;
use crate::WorkspaceClientUpsert;
use crate::WorkspaceContextClipFilter;
use crate::WorkspaceContextClipUpsert;
use crate::WorkspaceCoverageUpsert;
use crate::WorkspaceDocumentUpsert;
use crate::WorkspaceEncounterUpsert;
use crate::WorkspaceNoteUpsert;
use crate::WorkspacePatientSafetyItemUpsert;
use crate::WorkspaceTaskUpsert;
use crate::runtime::test_support::unique_temp_dir;
use chrono::DateTime;
use chrono::Utc;
use pretty_assertions::assert_eq;
use sqlx::sqlite::SqlitePoolOptions;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

type RevisionSnapshot = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

async fn runtime_at(path: PathBuf) -> Arc<StateRuntime> {
    StateRuntime::init(path, "test-provider".to_string())
        .await
        .expect("state runtime should initialize")
}

async fn runtime() -> Arc<StateRuntime> {
    runtime_at(unique_temp_dir()).await
}

fn new_request(key: &str, display_name: &str) -> WorkspaceChartCommitRequest {
    WorkspaceChartCommitRequest {
        idempotency_key: key.to_string(),
        actor: "Dr. Rivera".to_string(),
        reason: "atomic chart save".to_string(),
        source_thread_id: Some("thread-atomic".to_string()),
        source_turn_id: Some("turn-atomic".to_string()),
        client_id: None,
        client: Some(WorkspaceClientUpsert {
            display_name: display_name.to_string(),
            summary: "synthetic patient".to_string(),
            ..Default::default()
        }),
        coverage: None,
        expected_versions: Default::default(),
        safety_item: None,
        encounter: None,
        note: None,
        document: None,
        artifact_derivative: None,
        context_clip: None,
        task: None,
    }
}

fn identity_request(key: &str, client_id: &str) -> WorkspaceChartCommitRequest {
    let mut request = new_request(key, "unused");
    request.client_id = Some(client_id.to_string());
    request.client = None;
    request
}

fn full_graph_request(key: &str) -> WorkspaceChartCommitRequest {
    let mut request = new_request(key, "Synthetic Full Graph");
    request.safety_item = Some(WorkspacePatientSafetyItemUpsert {
        category: "allergy".to_string(),
        name: "Synthetic latex".to_string(),
        notes: "Synthetic only".to_string(),
        ..Default::default()
    });
    request.encounter = Some(WorkspaceEncounterUpsert {
        kind: "visit".to_string(),
        title: "Daily visit".to_string(),
        status: "open".to_string(),
        ..Default::default()
    });
    request.note = Some(WorkspaceChartNoteChange {
        upsert: WorkspaceNoteUpsert {
            title: "Daily note".to_string(),
            kind: "daily_note".to_string(),
            body: "Initial synthetic note".to_string(),
            status: "draft".to_string(),
            ..Default::default()
        },
        expected_base_revision: None,
    });
    request.document = Some(WorkspaceDocumentUpsert {
        title: "Synthetic scan".to_string(),
        kind: "image".to_string(),
        local_path: "/synthetic/scan.png".to_string(),
        ..Default::default()
    });
    request.artifact_derivative = Some(WorkspaceArtifactDerivativeUpsert {
        title: "Reviewed text".to_string(),
        body: "Synthetic extracted text".to_string(),
        ..Default::default()
    });
    request.context_clip = Some(WorkspaceContextClipUpsert {
        title: "Relevant excerpt".to_string(),
        body: "Synthetic context".to_string(),
        ..Default::default()
    });
    request.task = Some(WorkspaceTaskUpsert {
        title: "Synthetic follow-up".to_string(),
        ..Default::default()
    });
    request
}

fn client_upsert(client: &WorkspaceClient) -> WorkspaceClientUpsert {
    WorkspaceClientUpsert {
        id: Some(client.id.clone()),
        display_name: client.display_name.clone(),
        preferred_name: client.preferred_name.clone(),
        date_of_birth: client.date_of_birth.clone(),
        sex_or_gender: client.sex_or_gender.clone(),
        external_id: client.external_id.clone(),
        record_start_date: client.record_start_date.clone(),
        record_end_date: client.record_end_date.clone(),
        summary: client.summary.clone(),
        primary_phone: client.primary_phone.clone(),
        secondary_phone: client.secondary_phone.clone(),
        email: client.email.clone(),
        preferred_contact_method: client.preferred_contact_method.clone(),
        emergency_contact_name: client.emergency_contact_name.clone(),
        emergency_contact_relationship: client.emergency_contact_relationship.clone(),
        emergency_contact_phone: client.emergency_contact_phone.clone(),
        emergency_contact_email: client.emergency_contact_email.clone(),
        contact_notes: client.contact_notes.clone(),
        payer_name: client.payer_name.clone(),
        plan_name: client.plan_name.clone(),
        member_id: client.member_id.clone(),
        group_number: client.group_number.clone(),
        coverage_type: client.coverage_type.clone(),
        coverage_status: client.coverage_status.clone(),
        coverage_notes: client.coverage_notes.clone(),
        ..Default::default()
    }
}

fn task_upsert(task: &crate::WorkspaceTask) -> WorkspaceTaskUpsert {
    WorkspaceTaskUpsert {
        id: Some(task.id.clone()),
        client_id: task.client_id.clone(),
        encounter_id: task.encounter_id.clone(),
        note_id: task.note_id.clone(),
        document_id: task.document_id.clone(),
        title: task.title.clone(),
        details: task.details.clone(),
        kind: task.kind.clone(),
        status: task.status,
        priority: task.priority,
        due_date: task.due_date.clone(),
        assigned_to: task.assigned_to.clone(),
        actor: String::new(),
    }
}

fn full_graph_existing_request(
    key: &str,
    result: &WorkspaceChartCommitResult,
) -> WorkspaceChartCommitRequest {
    let safety = result.safety_item.as_ref().expect("safety item");
    let encounter = result.encounter.as_ref().expect("encounter");
    let note = result.note.as_ref().expect("note");
    let document = result.document.as_ref().expect("document");
    let derivative = result.artifact_derivative.as_ref().expect("derivative");
    let clip = result.context_clip.as_ref().expect("clip");
    let task = result.task.as_ref().expect("task");
    WorkspaceChartCommitRequest {
        idempotency_key: key.to_string(),
        actor: "Dr. Rivera".to_string(),
        reason: "atomic chart save".to_string(),
        source_thread_id: Some("thread-atomic".to_string()),
        source_turn_id: Some("turn-atomic".to_string()),
        client_id: Some(result.client.id.clone()),
        client: Some(client_upsert(&result.client)),
        coverage: None,
        expected_versions: crate::WorkspaceChartExpectedVersions {
            client: Some(result.client.record_version().expect("client version")),
            coverage: None,
            safety_item: Some(safety.record_version().expect("safety version")),
            encounter: Some(encounter.record_version().expect("encounter version")),
            document: Some(document.record_version().expect("document version")),
            artifact_derivative: Some(derivative.record_version().expect("derivative version")),
            context_clip: Some(clip.record_version().expect("clip version")),
            task: Some(task.record_version().expect("task version")),
        },
        safety_item: Some(WorkspacePatientSafetyItemUpsert {
            id: Some(safety.id.clone()),
            client_id: safety.client_id.clone(),
            category: safety.category.clone(),
            name: safety.name.clone(),
            reaction: safety.reaction.clone(),
            severity: safety.severity.clone(),
            dose: safety.dose.clone(),
            route: safety.route.clone(),
            frequency: safety.frequency.clone(),
            status: safety.status.clone(),
            recorded_date: safety.recorded_date.clone(),
            notes: safety.notes.clone(),
        }),
        encounter: Some(WorkspaceEncounterUpsert {
            id: Some(encounter.id.clone()),
            client_id: encounter.client_id.clone(),
            kind: encounter.kind.clone(),
            title: encounter.title.clone(),
            status: encounter.status.clone(),
            started_at: encounter.started_at,
            ended_at: encounter.ended_at,
        }),
        note: Some(WorkspaceChartNoteChange {
            upsert: WorkspaceNoteUpsert {
                id: Some(note.id.clone()),
                client_id: note.client_id.clone(),
                encounter_id: note.encounter_id.clone(),
                title: note.title.clone(),
                kind: note.kind.clone(),
                body: note.body.clone(),
                status: note.status.clone(),
                ..Default::default()
            },
            expected_base_revision: Some(note.current_revision),
        }),
        document: Some(WorkspaceDocumentUpsert {
            id: Some(document.id.clone()),
            client_id: document.client_id.clone(),
            encounter_id: document.encounter_id.clone(),
            title: document.title.clone(),
            kind: document.kind.clone(),
            local_path: document.local_path.clone(),
            notes: document.notes.clone(),
            scope: document.scope.clone(),
            detected_kind: document.detected_kind.clone(),
            mime_type: document.mime_type.clone(),
            file_size_bytes: document.file_size_bytes,
            modified_at: document.modified_at,
            sha256: document.sha256.clone(),
            tags: document.tags.clone(),
            source_label: document.source_label.clone(),
            existence_status: document.existence_status.clone(),
            metadata_json: document.metadata_json.clone(),
            original_path: document.original_path.clone(),
            reference_kind: document.reference_kind.clone(),
            vault_path: document.vault_path.clone(),
            content_sha256: document.content_sha256.clone(),
            thumbnail_path: document.thumbnail_path.clone(),
            thumbnail_status: document.thumbnail_status.clone(),
            thumbnail_mime_type: document.thumbnail_mime_type.clone(),
            intake_source: document.intake_source.clone(),
            imported_at: document.imported_at,
        }),
        artifact_derivative: Some(WorkspaceArtifactDerivativeUpsert {
            id: Some(derivative.id.clone()),
            document_id: derivative.document_id.clone(),
            client_id: derivative.client_id.clone(),
            encounter_id: derivative.encounter_id.clone(),
            note_id: derivative.note_id.clone(),
            kind: derivative.kind.clone(),
            title: derivative.title.clone(),
            body: derivative.body.clone(),
            review_status: derivative.review_status.clone(),
            source_method: derivative.source_method.clone(),
            page_range: derivative.page_range.clone(),
            timestamp_range: derivative.timestamp_range.clone(),
            segment_label: derivative.segment_label.clone(),
            tags: derivative.tags.clone(),
            metadata_json: derivative.metadata_json.clone(),
            actor: String::new(),
        }),
        context_clip: Some(WorkspaceContextClipUpsert {
            id: Some(clip.id.clone()),
            derivative_id: clip.derivative_id.clone(),
            document_id: clip.document_id.clone(),
            client_id: clip.client_id.clone(),
            encounter_id: clip.encounter_id.clone(),
            note_id: clip.note_id.clone(),
            kind: clip.kind.clone(),
            title: clip.title.clone(),
            body: clip.body.clone(),
            review_status: clip.review_status.clone(),
            source_method: clip.source_method.clone(),
            page_range: clip.page_range.clone(),
            timestamp_range: clip.timestamp_range.clone(),
            line_range: clip.line_range.clone(),
            segment_label: clip.segment_label.clone(),
            tags: clip.tags.clone(),
            metadata_json: clip.metadata_json.clone(),
            actor: String::new(),
        }),
        task: Some(WorkspaceTaskUpsert {
            id: Some(task.id.clone()),
            client_id: task.client_id.clone(),
            encounter_id: task.encounter_id.clone(),
            note_id: task.note_id.clone(),
            document_id: task.document_id.clone(),
            title: task.title.clone(),
            details: task.details.clone(),
            kind: task.kind.clone(),
            status: task.status,
            priority: task.priority,
            due_date: task.due_date.clone(),
            assigned_to: task.assigned_to.clone(),
            actor: String::new(),
        }),
    }
}

async fn chart_row_count(runtime: &StateRuntime) -> i64 {
    sqlx::query_scalar(
        r#"
SELECT
    (SELECT COUNT(*) FROM workspace_clients)
  + (SELECT COUNT(*) FROM workspace_client_contacts)
  + (SELECT COUNT(*) FROM workspace_client_coverages)
  + (SELECT COUNT(*) FROM workspace_coverages)
  + (SELECT COUNT(*) FROM workspace_coverage_card_verifications)
  + (SELECT COUNT(*) FROM workspace_patient_safety_items)
  + (SELECT COUNT(*) FROM workspace_encounters)
  + (SELECT COUNT(*) FROM workspace_notes)
  + (SELECT COUNT(*) FROM workspace_note_revisions)
  + (SELECT COUNT(*) FROM workspace_documents)
  + (SELECT COUNT(*) FROM workspace_artifact_derivatives)
  + (SELECT COUNT(*) FROM workspace_context_clips)
  + (SELECT COUNT(*) FROM workspace_tasks)
  + (SELECT COUNT(*) FROM workspace_audit_events)
  + (SELECT COUNT(*) FROM workspace_chart_commits)
        "#,
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("chart row count")
}

#[tokio::test]
async fn blank_entity_ids_allocate_and_blank_links_infer_full_graph() {
    let runtime = runtime().await;
    let mut request = full_graph_request("blank-ids");
    request.client.as_mut().expect("client").id = Some("   ".to_string());
    request.safety_item.as_mut().expect("safety").id = Some(" ".to_string());
    request.encounter.as_mut().expect("encounter").id = Some(" ".to_string());
    let note = &mut request.note.as_mut().expect("note").upsert;
    note.id = Some(" ".to_string());
    note.encounter_id = Some(" ".to_string());
    let document = request.document.as_mut().expect("document");
    document.id = Some(" ".to_string());
    document.encounter_id = Some(" ".to_string());
    let derivative = request.artifact_derivative.as_mut().expect("derivative");
    derivative.id = Some(" ".to_string());
    derivative.encounter_id = Some(" ".to_string());
    derivative.note_id = Some(" ".to_string());
    let clip = request.context_clip.as_mut().expect("clip");
    clip.id = Some(" ".to_string());
    clip.encounter_id = Some(" ".to_string());
    clip.note_id = Some(" ".to_string());
    let task = request.task.as_mut().expect("task");
    task.id = Some(" ".to_string());
    task.encounter_id = Some(" ".to_string());
    task.note_id = Some(" ".to_string());
    task.document_id = Some(" ".to_string());

    let result = runtime
        .workspace()
        .commit_chart(request)
        .await
        .expect("blank IDs should allocate");
    let ids = [
        result.client.id.as_str(),
        result.safety_item.as_ref().expect("safety").id.as_str(),
        result.encounter.as_ref().expect("encounter").id.as_str(),
        result.note.as_ref().expect("note").id.as_str(),
        result.document.as_ref().expect("document").id.as_str(),
        result
            .artifact_derivative
            .as_ref()
            .expect("derivative")
            .id
            .as_str(),
        result.context_clip.as_ref().expect("clip").id.as_str(),
        result.task.as_ref().expect("task").id.as_str(),
    ];
    assert!(ids.iter().all(|id| !id.trim().is_empty()));
    let encounter_id = result.encounter.as_ref().expect("encounter").id.as_str();
    assert_eq!(
        result.note.as_ref().expect("note").encounter_id.as_deref(),
        Some(encounter_id)
    );
    assert_eq!(
        result.task.as_ref().expect("task").note_id,
        result.note.as_ref().map(|note| note.id.clone())
    );
}

#[tokio::test]
async fn empty_optional_fields_are_no_op_equivalent() {
    let runtime = runtime().await;
    let created = runtime
        .workspace()
        .commit_chart(full_graph_request("optional-create"))
        .await
        .expect("create full graph");
    let audit_count_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE source = 'workspace_chart_commit'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("audit count");
    let mut request = full_graph_existing_request("optional-no-op", &created);
    let client = request.client.as_mut().expect("client");
    client.preferred_name = Some("   ".to_string());
    client.primary_phone = Some(" ".to_string());
    client.coverage_notes = Some("\t".to_string());
    let safety = request.safety_item.as_mut().expect("safety");
    safety.reaction = Some(" ".to_string());
    safety.status = Some("\n".to_string());
    let document = request.document.as_mut().expect("document");
    document.mime_type = Some(" ".to_string());
    document.sha256 = Some(" ".to_string());
    document.content_sha256 = Some(" ".to_string());
    document.thumbnail_mime_type = Some(" ".to_string());
    let task = request.task.as_mut().expect("task");
    task.due_date = Some(" ".to_string());
    task.assigned_to = Some(" ".to_string());

    let result = runtime
        .workspace()
        .commit_chart(request)
        .await
        .expect("normalized optional values should no-op");
    assert_eq!(result.changed_entity_kinds, Vec::new());
    assert_eq!(result.resulting_note_revision, Some(1));
    let audit_count_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE source = 'workspace_chart_commit'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("audit count");
    assert_eq!(audit_count_after, audit_count_before);
}

#[tokio::test]
async fn encounter_and_document_times_compare_at_wire_second_precision() {
    let runtime = runtime().await;
    let mut create = full_graph_request("timestamp-create");
    let encounter = create.encounter.as_mut().expect("encounter");
    encounter.started_at = DateTime::<Utc>::from_timestamp(1_800_000_000, 100_000_000);
    encounter.ended_at = DateTime::<Utc>::from_timestamp(1_800_000_100, 100_000_000);
    let document = create.document.as_mut().expect("document");
    document.modified_at = DateTime::<Utc>::from_timestamp(1_800_000_200, 100_000_000);
    document.imported_at = DateTime::<Utc>::from_timestamp(1_800_000_300, 100_000_000);
    let created = runtime
        .workspace()
        .commit_chart(create)
        .await
        .expect("create timestamp graph");
    let versions_before = (
        created
            .encounter
            .as_ref()
            .expect("encounter")
            .record_version()
            .expect("encounter version"),
        created
            .document
            .as_ref()
            .expect("document")
            .record_version()
            .expect("document version"),
    );

    let mut no_op = full_graph_existing_request("timestamp-no-op", &created);
    let encounter = no_op.encounter.as_mut().expect("encounter");
    encounter.started_at = DateTime::<Utc>::from_timestamp(1_800_000_000, 900_000_000);
    encounter.ended_at = DateTime::<Utc>::from_timestamp(1_800_000_100, 900_000_000);
    let document = no_op.document.as_mut().expect("document");
    document.modified_at = DateTime::<Utc>::from_timestamp(1_800_000_200, 900_000_000);
    document.imported_at = DateTime::<Utc>::from_timestamp(1_800_000_300, 900_000_000);
    assert!(super::compare::encounter(
        created.encounter.as_ref().expect("created encounter"),
        no_op.encounter.as_ref().expect("encounter input")
    ));
    assert!(super::compare::document(
        created.document.as_ref().expect("created document"),
        no_op.document.as_ref().expect("document input")
    ));
    let result = runtime
        .workspace()
        .commit_chart(no_op)
        .await
        .expect("subsecond-only differences should no-op");
    assert_eq!(result.changed_entity_kinds, Vec::new());
    assert_eq!(
        result
            .encounter
            .as_ref()
            .expect("encounter")
            .record_version()
            .expect("encounter version"),
        versions_before.0
    );
    assert_eq!(
        result
            .document
            .as_ref()
            .expect("document")
            .record_version()
            .expect("document version"),
        versions_before.1
    );
}

#[tokio::test]
async fn mixed_edits_preserve_exact_existing_timestamp_precision() {
    let runtime = runtime().await;
    let mut create = full_graph_request("mixed-timestamp-create");
    let encounter = create.encounter.as_mut().expect("encounter");
    encounter.started_at = DateTime::<Utc>::from_timestamp(1_800_001_000, 123_000_000);
    encounter.ended_at = DateTime::<Utc>::from_timestamp(1_800_001_100, 456_000_000);
    let document = create.document.as_mut().expect("document");
    document.modified_at = DateTime::<Utc>::from_timestamp(1_800_001_200, 234_000_000);
    document.imported_at = DateTime::<Utc>::from_timestamp(1_800_001_300, 567_000_000);
    let created = runtime
        .workspace()
        .commit_chart(create)
        .await
        .expect("create timestamp graph");
    let original_encounter = created.encounter.clone().expect("encounter");
    let original_document = created.document.clone().expect("document");
    let original_encounter_version = original_encounter
        .record_version()
        .expect("encounter version");
    let original_document_version = original_document
        .record_version()
        .expect("document version");

    let mut update = full_graph_existing_request("mixed-timestamp-update", &created);
    update.client = None;
    update.safety_item = None;
    update.note = None;
    update.artifact_derivative = None;
    update.context_clip = None;
    update.task = None;
    let encounter_version = update.expected_versions.encounter.take();
    let document_version = update.expected_versions.document.take();
    update.expected_versions = Default::default();
    update.expected_versions.encounter = encounter_version;
    update.expected_versions.document = document_version;
    let encounter = update.encounter.as_mut().expect("encounter");
    encounter.title = "Mixed edit encounter".to_string();
    encounter.started_at = DateTime::<Utc>::from_timestamp(
        original_encounter
            .started_at
            .expect("started at")
            .timestamp(),
        0,
    );
    encounter.ended_at = DateTime::<Utc>::from_timestamp(
        original_encounter.ended_at.expect("ended at").timestamp(),
        0,
    );
    let document = update.document.as_mut().expect("document");
    document.notes = "Mixed edit document".to_string();
    document.modified_at = DateTime::<Utc>::from_timestamp(
        original_document
            .modified_at
            .expect("modified at")
            .timestamp(),
        0,
    );
    document.imported_at = DateTime::<Utc>::from_timestamp(
        original_document
            .imported_at
            .expect("imported at")
            .timestamp(),
        0,
    );

    let result = runtime
        .workspace()
        .commit_chart(update)
        .await
        .expect("mixed edit should commit");
    assert_eq!(
        result.changed_entity_kinds,
        vec![
            WorkspaceChartEntityKind::Encounter,
            WorkspaceChartEntityKind::Document,
        ]
    );
    let actual_encounter = result.encounter.expect("encounter");
    let actual_document = result.document.expect("document");
    let mut expected_encounter = original_encounter;
    expected_encounter.title = "Mixed edit encounter".to_string();
    expected_encounter.updated_at = actual_encounter.updated_at;
    let mut expected_document = original_document;
    expected_document.notes = "Mixed edit document".to_string();
    expected_document.updated_at = actual_document.updated_at;
    assert_eq!(actual_encounter, expected_encounter);
    assert_eq!(actual_document, expected_document);
    assert_eq!(
        actual_encounter
            .record_version()
            .expect("encounter version"),
        expected_encounter
            .record_version()
            .expect("expected encounter version")
    );
    assert_eq!(
        actual_document.record_version().expect("document version"),
        expected_document
            .record_version()
            .expect("expected document version")
    );
    assert_ne!(
        actual_encounter
            .record_version()
            .expect("encounter version"),
        original_encounter_version
    );
    assert_ne!(
        actual_document.record_version().expect("document version"),
        original_document_version
    );
}

#[tokio::test]
async fn invalid_child_payloads_fail_validation_before_any_write() {
    let runtime = runtime().await;
    let mut invalid = Vec::new();

    let mut safety = full_graph_request("invalid-safety");
    safety.safety_item.as_mut().expect("safety").name = "  ".to_string();
    invalid.push(safety);

    let mut encounter = full_graph_request("invalid-encounter");
    encounter.encounter.as_mut().expect("encounter").title = " ".to_string();
    invalid.push(encounter);

    let mut encounter_status = full_graph_request("invalid-encounter-status");
    encounter_status
        .encounter
        .as_mut()
        .expect("encounter")
        .status = "closed".to_string();
    invalid.push(encounter_status);

    let mut encounter_dates = full_graph_request("invalid-encounter-dates");
    let encounter = encounter_dates.encounter.as_mut().expect("encounter");
    encounter.started_at = DateTime::<Utc>::from_timestamp(200, 0);
    encounter.ended_at = DateTime::<Utc>::from_timestamp(100, 0);
    invalid.push(encounter_dates);

    let mut note = full_graph_request("invalid-note");
    note.note.as_mut().expect("note").upsert.title = " ".to_string();
    invalid.push(note);

    let mut note_status = full_graph_request("invalid-note-status");
    note_status.note.as_mut().expect("note").upsert.status = "completed".to_string();
    invalid.push(note_status);

    let mut document = full_graph_request("invalid-document");
    document.document.as_mut().expect("document").local_path = " ".to_string();
    invalid.push(document);

    let mut document_size = full_graph_request("invalid-document-size");
    document_size
        .document
        .as_mut()
        .expect("document")
        .file_size_bytes = Some(-1);
    invalid.push(document_size);

    let mut derivative_status = full_graph_request("invalid-derivative-status");
    derivative_status
        .artifact_derivative
        .as_mut()
        .expect("derivative")
        .review_status = "machine_approved".to_string();
    invalid.push(derivative_status);

    let mut derivative_archived = full_graph_request("invalid-derivative-archived");
    derivative_archived
        .artifact_derivative
        .as_mut()
        .expect("derivative")
        .review_status = "archived".to_string();
    invalid.push(derivative_archived);

    let mut derivative_body = full_graph_request("invalid-derivative-body");
    derivative_body
        .artifact_derivative
        .as_mut()
        .expect("derivative")
        .body = " ".to_string();
    invalid.push(derivative_body);

    let mut missing_document = new_request("invalid-derivative-link", "Invalid Link");
    missing_document.artifact_derivative = Some(WorkspaceArtifactDerivativeUpsert {
        title: "Derivative".to_string(),
        body: "Body".to_string(),
        ..Default::default()
    });
    invalid.push(missing_document);

    let mut missing_derivative = new_request("invalid-clip-link", "Invalid Clip");
    missing_derivative.document = Some(WorkspaceDocumentUpsert {
        title: "Document".to_string(),
        local_path: "/synthetic/document".to_string(),
        ..Default::default()
    });
    missing_derivative.context_clip = Some(WorkspaceContextClipUpsert {
        title: "Clip".to_string(),
        body: "Body".to_string(),
        ..Default::default()
    });
    invalid.push(missing_derivative);

    let mut clip_archived = full_graph_request("invalid-clip-archived");
    clip_archived
        .context_clip
        .as_mut()
        .expect("clip")
        .review_status = "archived".to_string();
    invalid.push(clip_archived);

    let mut task = full_graph_request("invalid-task");
    task.task.as_mut().expect("task").title = " ".to_string();
    invalid.push(task);

    for request in invalid {
        assert!(matches!(
            runtime.workspace().commit_chart(request).await,
            Err(WorkspaceChartCommitError::Validation { .. })
        ));
        assert_eq!(chart_row_count(&runtime).await, 0);
    }
}

#[tokio::test]
async fn root_identity_and_edit_combinations_are_explicit() {
    let runtime = runtime().await;
    let mut neither = new_request("neither", "unused");
    neither.client = None;
    assert!(matches!(
        runtime.workspace().commit_chart(neither).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let missing = identity_request("missing-root", "missing-client");
    assert!(matches!(
        runtime.workspace().commit_chart(missing).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let created = runtime
        .workspace()
        .commit_chart(new_request("new-root", "Synthetic Root"))
        .await
        .expect("new client payload should allocate root");

    let mut edit = identity_request("root-edit", &created.client.id);
    let mut edited_client = client_upsert(&created.client);
    edited_client.id = None;
    edited_client.preferred_name = Some("Root".to_string());
    edit.client = Some(edited_client);
    edit.expected_versions.client = Some(
        created
            .client
            .record_version()
            .expect("client record version"),
    );
    let edited = runtime
        .workspace()
        .commit_chart(edit)
        .await
        .expect("root id plus client edit should normalize");
    assert_eq!(edited.client.preferred_name.as_deref(), Some("Root"));

    let mut conflicting = identity_request("root-conflict", &created.client.id);
    let mut conflict_client = client_upsert(&edited.client);
    conflict_client.id = Some("different-client".to_string());
    conflicting.client = Some(conflict_client);
    assert!(matches!(
        runtime.workspace().commit_chart(conflicting).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut derived = identity_request("derived-root", &edited.client.id);
    derived.client_id = None;
    let mut derived_client = client_upsert(&edited.client);
    derived_client.contact_notes = Some("derived from embedded id".to_string());
    derived.client = Some(derived_client);
    derived.expected_versions.client = Some(
        edited
            .client
            .record_version()
            .expect("edited client record version"),
    );
    let mut explicit = derived.clone();
    explicit.client_id = Some(edited.client.id.clone());
    let derived_result = runtime
        .workspace()
        .commit_chart(derived)
        .await
        .expect("embedded client id should derive root");
    let replay = runtime
        .workspace()
        .commit_chart(explicit)
        .await
        .expect("explicit root should canonicalize to same receipt");
    assert!(replay.replayed);
    let mut expected_replay = derived_result;
    expected_replay.replayed = true;
    assert_eq!(replay, expected_replay);

    let mut explicit_new_id = new_request("new-explicit-id", "Invalid New Root");
    explicit_new_id.client.as_mut().expect("client").id = Some("explicit".to_string());
    assert!(matches!(
        runtime.workspace().commit_chart(explicit_new_id).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));
}

#[tokio::test]
async fn identity_only_note_commit_preserves_newer_client_metadata() {
    let runtime = runtime().await;
    let mut create = new_request("identity-create", "Synthetic Identity");
    create.client.as_mut().expect("client").primary_phone = Some("555-0100".to_string());
    create.client.as_mut().expect("client").payer_name = Some("Synthetic Plan".to_string());
    create.note = Some(WorkspaceChartNoteChange {
        upsert: WorkspaceNoteUpsert {
            title: "Daily note".to_string(),
            body: "Revision one".to_string(),
            ..Default::default()
        },
        expected_base_revision: None,
    });
    let created = runtime
        .workspace()
        .commit_chart(create)
        .await
        .expect("create chart");
    let stale_client_snapshot = created.client.clone();
    let mut concurrent_client = client_upsert(&created.client);
    concurrent_client.preferred_name = Some("Newer Name".to_string());
    concurrent_client.primary_phone = Some("555-0199".to_string());
    concurrent_client.coverage_notes = Some("newer coverage".to_string());
    let concurrent_client = runtime
        .workspace()
        .upsert_client(concurrent_client)
        .await
        .expect("concurrent client update");

    let note = created.note.as_ref().expect("note");
    let mut note_only = identity_request("identity-note-only", &created.client.id);
    note_only.note = Some(WorkspaceChartNoteChange {
        upsert: WorkspaceNoteUpsert {
            id: Some(note.id.clone()),
            title: note.title.clone(),
            kind: note.kind.clone(),
            body: "Revision two".to_string(),
            status: note.status.clone(),
            ..Default::default()
        },
        expected_base_revision: Some(1),
    });
    let committed = runtime
        .workspace()
        .commit_chart(note_only)
        .await
        .expect("identity-only note commit");

    assert_eq!(
        committed.changed_entity_kinds,
        vec![WorkspaceChartEntityKind::Note]
    );
    assert_eq!(committed.client, concurrent_client);
    assert_ne!(committed.client, stale_client_snapshot);
}

#[tokio::test]
async fn cross_patient_entity_and_relationship_ids_fail_closed() {
    let runtime = runtime().await;
    let patient_a = runtime
        .workspace()
        .commit_chart(full_graph_request("cross-a"))
        .await
        .expect("patient A");
    let patient_b = runtime
        .workspace()
        .commit_chart(full_graph_request("cross-b"))
        .await
        .expect("patient B");
    let receipts_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_chart_commits")
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .expect("receipt count");

    let safety_b = patient_b.safety_item.as_ref().expect("B safety");
    runtime
        .workspace()
        .archive_patient_safety_item(&safety_b.id)
        .await
        .expect("archive B safety");
    let mut safety_entity = identity_request("cross-safety-entity", &patient_a.client.id);
    safety_entity.safety_item = Some(WorkspacePatientSafetyItemUpsert {
        id: Some(safety_b.id.clone()),
        client_id: patient_a.client.id.clone(),
        category: safety_b.category.clone(),
        name: "Cross-patient edit".to_string(),
        ..Default::default()
    });
    let safety_error = runtime
        .workspace()
        .commit_chart(safety_entity)
        .await
        .expect_err("cross-patient archived entity must fail");
    assert!(matches!(
        &safety_error,
        WorkspaceChartCommitError::Validation { .. }
    ));
    assert!(
        safety_error
            .to_string()
            .contains("was not found for client")
    );
    assert!(!safety_error.to_string().contains("archived"));

    let task_b = patient_b.task.as_ref().expect("B task");
    let mut task_entity = identity_request("cross-task-entity", &patient_a.client.id);
    task_entity.task = Some(WorkspaceTaskUpsert {
        id: Some(task_b.id.clone()),
        client_id: patient_a.client.id.clone(),
        title: "Cross-patient task edit".to_string(),
        ..Default::default()
    });
    assert!(matches!(
        runtime.workspace().commit_chart(task_entity).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut relationship = identity_request("cross-relationships", &patient_a.client.id);
    relationship.task = Some(WorkspaceTaskUpsert {
        client_id: patient_a.client.id.clone(),
        encounter_id: patient_b.encounter.as_ref().map(|value| value.id.clone()),
        note_id: patient_b.note.as_ref().map(|value| value.id.clone()),
        document_id: patient_b.document.as_ref().map(|value| value.id.clone()),
        title: "Cross-patient links".to_string(),
        ..Default::default()
    });
    assert!(matches!(
        runtime.workspace().commit_chart(relationship).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let document_b = patient_b.document.as_ref().expect("B document");
    let derivative_b = patient_b
        .artifact_derivative
        .as_ref()
        .expect("B derivative");
    let mut clip_relationship = identity_request("cross-clip-links", &patient_a.client.id);
    clip_relationship.context_clip = Some(WorkspaceContextClipUpsert {
        derivative_id: derivative_b.id.clone(),
        document_id: document_b.id.clone(),
        client_id: patient_a.client.id.clone(),
        title: "Cross clip".to_string(),
        body: "Cross body".to_string(),
        ..Default::default()
    });
    assert!(matches!(
        runtime.workspace().commit_chart(clip_relationship).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let receipts_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_chart_commits")
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .expect("receipt count");
    assert_eq!(receipts_after, receipts_before);
}

#[tokio::test]
async fn exact_replay_survives_restart_and_rejects_unknown_receipt_schema() {
    let path = unique_temp_dir();
    let runtime = runtime_at(path.clone()).await;
    let request = full_graph_request("restart-replay");
    let committed = runtime
        .workspace()
        .commit_chart(request.clone())
        .await
        .expect("initial commit");
    let stored: (i64, String) = sqlx::query_as(
        "SELECT schema_version, request_json FROM workspace_chart_commits WHERE idempotency_key = ?",
    )
    .bind("restart-replay")
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("receipt metadata");
    assert_eq!(stored.0, CHART_COMMIT_SCHEMA_VERSION);
    let normalized: WorkspaceChartCommitRequest =
        serde_json::from_str(&stored.1).expect("normalized request JSON");
    assert!(normalized.client_id.is_none());
    assert!(
        normalized
            .client
            .as_ref()
            .is_some_and(|client| client.id.is_none())
    );
    drop(runtime);

    let reopened = runtime_at(path).await;
    let replayed = reopened
        .workspace()
        .commit_chart(request.clone())
        .await
        .expect("replay after reopen");
    let mut expected = committed;
    expected.replayed = true;
    assert_eq!(replayed, expected);

    sqlx::query(
        "UPDATE workspace_chart_commits SET schema_version = 1, request_sha256 = ? WHERE idempotency_key = ?",
    )
    .bind(chart_commit_hash(
        LEGACY_CHART_COMMIT_HASH_PREFIX,
        &stored.1,
    ))
    .bind("restart-replay")
    .execute(reopened.workspace().pool.as_ref())
    .await
    .expect("mark transitional v1 receipt");
    reopened
        .workspace()
        .commit_chart(request.clone())
        .await
        .expect("transitional v1 receipt replay");

    sqlx::query("UPDATE workspace_chart_commits SET schema_version = 99 WHERE idempotency_key = ?")
        .bind("restart-replay")
        .execute(reopened.workspace().pool.as_ref())
        .await
        .expect("corrupt receipt schema version");
    assert!(matches!(
        reopened.workspace().commit_chart(request).await,
        Err(WorkspaceChartCommitError::Storage { .. })
    ));
}

#[tokio::test]
async fn pre_demographics_receipt_replays_after_workspace_migration() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory workspace database");
    let base = &crate::migrations::WORKSPACE_MIGRATOR;
    let through_seventeen = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            base.migrations
                .iter()
                .filter(|migration| migration.version <= 17)
                .cloned()
                .collect(),
        ),
        ignore_missing: base.ignore_missing,
        locking: base.locking,
        table_name: base.table_name.clone(),
        create_schemas: base.create_schemas.clone(),
        no_tx: base.no_tx,
    };
    through_seventeen
        .run(&pool)
        .await
        .expect("pre-demographics migrations");
    sqlx::query(
        "INSERT INTO workspace_clients (id, display_name, summary, created_at_ms, updated_at_ms) VALUES ('legacy-client', 'Legacy Retry', 'synthetic', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy client");

    let request = new_request("legacy-retry", "Legacy Retry");
    let mut normalized = request.clone();
    validate::normalize_before_hash(&mut normalized).expect("normalize legacy retry");
    let request_json = legacy_request_json(&normalized)
        .expect("legacy request projection")
        .expect("v1-compatible request");
    let result_json = serde_json::json!({
        "commit_id": "legacy-commit",
        "idempotency_key": "legacy-retry",
        "replayed": false,
        "changed_entity_kinds": ["client"],
        "client": {
            "id": "legacy-client",
            "display_name": "Legacy Retry",
            "preferred_name": null,
            "date_of_birth": null,
            "sex_or_gender": null,
            "external_id": null,
            "record_start_date": null,
            "record_end_date": null,
            "summary": "synthetic",
            "primary_phone": null,
            "secondary_phone": null,
            "email": "legacy@example.test",
            "preferred_contact_method": null,
            "emergency_contact_name": null,
            "emergency_contact_relationship": null,
            "emergency_contact_phone": null,
            "emergency_contact_email": null,
            "contact_notes": null,
            "payer_name": null,
            "plan_name": null,
            "member_id": null,
            "group_number": null,
            "coverage_type": null,
            "coverage_status": null,
            "coverage_notes": null,
            "archived_at": null,
            "created_at": "1970-01-01T00:00:00.001Z",
            "updated_at": "1970-01-01T00:00:00.001Z"
        },
        "safety_item": null,
        "encounter": null,
        "note": null,
        "document": null,
        "artifact_derivative": null,
        "context_clip": null,
        "task": null,
        "resulting_note_revision": null,
        "committed_at": "1970-01-01T00:00:00.001Z"
    })
    .to_string();
    sqlx::query(
        r#"
INSERT INTO workspace_chart_commits (
    id, idempotency_key, schema_version, request_sha256, request_json,
    client_id, actor, reason, changed_entity_kinds_json, result_json, created_at_ms
) VALUES ('legacy-commit', 'legacy-retry', 1, ?, ?, 'legacy-client',
          'Dr. Rivera', 'atomic chart save', '["client"]', ?, 1)
        "#,
    )
    .bind(chart_commit_hash(
        LEGACY_CHART_COMMIT_HASH_PREFIX,
        &request_json,
    ))
    .bind(&request_json)
    .bind(result_json)
    .execute(&pool)
    .await
    .expect("legacy receipt");

    base.run(&pool)
        .await
        .expect("demographics migration after legacy receipt");
    let store = WorkspaceStore::new(Arc::new(pool));
    let replayed = store
        .commit_chart(request)
        .await
        .expect("legacy retry should replay");
    assert!(replayed.replayed);
    assert_eq!(replayed.commit_id, "legacy-commit");
    assert_eq!(
        replayed.client.primary_email.as_deref(),
        Some("legacy@example.test")
    );
    assert!(!replayed.client.interpreter_required);
    assert_eq!(replayed.coverage, None);
}

#[tokio::test]
async fn optimistic_versions_serialize_task_and_client_edit_races() {
    let runtime = runtime().await;
    let created = runtime
        .workspace()
        .commit_chart(full_graph_request("version-race-seed"))
        .await
        .expect("seed version race");
    let task = created.task.as_ref().expect("task");
    let task_version = task.record_version().expect("task record version");
    let mut left_task = identity_request("task-race-left", &created.client.id);
    left_task.task = Some(task_upsert(task));
    left_task.task.as_mut().expect("task").title = "Left task edit".to_string();
    left_task.expected_versions.task = Some(task_version.clone());
    let mut right_task = left_task.clone();
    right_task.idempotency_key = "task-race-right".to_string();
    right_task.task.as_mut().expect("task").title = "Right task edit".to_string();
    let (left_task_result, right_task_result) = tokio::join!(
        runtime.workspace().commit_chart(left_task),
        runtime.workspace().commit_chart(right_task)
    );
    let task_results = [left_task_result, right_task_result];
    assert_eq!(
        task_results.iter().filter(|result| result.is_ok()).count(),
        1
    );
    assert_eq!(
        task_results
            .iter()
            .filter(|result| matches!(
                result,
                Err(WorkspaceChartCommitError::StaleEntityVersion {
                    entity_kind: WorkspaceChartEntityKind::Task,
                    expected,
                    ..
                }) if expected == &task_version
            ))
            .count(),
        1
    );
    let task_winner = task_results
        .into_iter()
        .find_map(Result::ok)
        .expect("one task winner");
    assert_deep_entity_audit(
        &runtime,
        &task_winner,
        WorkspaceChartEntityKind::Task,
        task.id.as_str(),
        task.encounter_id.as_deref(),
        task.note_id.as_deref(),
        task.document_id.as_deref(),
    )
    .await;

    let client = task_winner.client.clone();
    let client_version = client.record_version().expect("client record version");
    let mut left_client = identity_request("client-race-left", &client.id);
    left_client.client = Some(client_upsert(&client));
    left_client.client.as_mut().expect("client").preferred_name = Some("Left".to_string());
    left_client.expected_versions.client = Some(client_version.clone());
    let mut right_client = left_client.clone();
    right_client.idempotency_key = "client-race-right".to_string();
    right_client.client.as_mut().expect("client").preferred_name = Some("Right".to_string());
    let (left_client_result, right_client_result) = tokio::join!(
        runtime.workspace().commit_chart(left_client),
        runtime.workspace().commit_chart(right_client)
    );
    let client_results = [left_client_result, right_client_result];
    assert_eq!(
        client_results
            .iter()
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert_eq!(
        client_results
            .iter()
            .filter(|result| matches!(
                result,
                Err(WorkspaceChartCommitError::StaleEntityVersion {
                    entity_kind: WorkspaceChartEntityKind::Client,
                    expected,
                    ..
                }) if expected == &client_version
            ))
            .count(),
        1
    );
    let client_winner = client_results
        .into_iter()
        .find_map(Result::ok)
        .expect("one client winner");
    assert_deep_entity_audit(
        &runtime,
        &client_winner,
        WorkspaceChartEntityKind::Client,
        client.id.as_str(),
        None,
        None,
        None,
    )
    .await;
}

async fn assert_deep_entity_audit(
    runtime: &StateRuntime,
    result: &WorkspaceChartCommitResult,
    kind: WorkspaceChartEntityKind,
    entity_id: &str,
    encounter_id: Option<&str>,
    note_id: Option<&str>,
    document_id: Option<&str>,
) {
    let entity_type = kind.as_str();
    let audit = runtime
        .workspace()
        .list_audit_events(entity_type, entity_id)
        .await
        .expect("audit read")
        .into_iter()
        .find(|audit| {
            audit.action == "updated"
                && audit
                    .metadata_json
                    .as_deref()
                    .is_some_and(|metadata| metadata.contains(&result.commit_id))
        })
        .expect("updated entity audit");
    let expected = WorkspaceAuditEvent {
        id: audit.id.clone(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        action: "updated".to_string(),
        actor: "Dr. Rivera".to_string(),
        actor_kind: "human".to_string(),
        source: "workspace_chart_commit".to_string(),
        client_id: Some(result.client.id.clone()),
        encounter_id: encounter_id.map(str::to_string),
        note_id: note_id.map(str::to_string),
        document_id: document_id.map(str::to_string),
        source_thread_id: Some("thread-atomic".to_string()),
        source_turn_id: Some("turn-atomic".to_string()),
        success: true,
        summary: "atomic chart save".to_string(),
        metadata_json: Some(serde_json::json!({ "commit_id": result.commit_id }).to_string()),
        created_at: audit.created_at,
    };
    assert_eq!(audit, expected);
}

#[tokio::test]
async fn optimistic_version_requirements_reject_missing_wrong_and_unexpected_tokens() {
    let runtime = runtime().await;
    let created = runtime
        .workspace()
        .commit_chart(full_graph_request("version-requirements-seed"))
        .await
        .expect("seed version requirements");
    let task = created.task.as_ref().expect("task");

    let mut missing = identity_request("version-missing", &created.client.id);
    missing.task = Some(task_upsert(task));
    assert!(matches!(
        runtime.workspace().commit_chart(missing).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut wrong = identity_request("version-wrong", &created.client.id);
    wrong.task = Some(task_upsert(task));
    wrong.expected_versions.task = Some("wrong-version".to_string());
    assert!(matches!(
        runtime.workspace().commit_chart(wrong).await,
        Err(WorkspaceChartCommitError::StaleEntityVersion {
            entity_kind: WorkspaceChartEntityKind::Task,
            ..
        })
    ));

    let mut unexpected_new = identity_request("version-unexpected-new", &created.client.id);
    unexpected_new.task = Some(WorkspaceTaskUpsert {
        title: "New task".to_string(),
        ..Default::default()
    });
    unexpected_new.expected_versions.task = Some("not-allowed".to_string());
    assert!(matches!(
        runtime.workspace().commit_chart(unexpected_new).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut unexpected_absent = identity_request("version-unexpected-absent", &created.client.id);
    unexpected_absent.expected_versions.document = Some("not-included".to_string());
    assert!(matches!(
        runtime.workspace().commit_chart(unexpected_absent).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut unexpected_new_client = new_request("version-new-client", "Version New Client");
    unexpected_new_client.expected_versions.client = Some("not-allowed".to_string());
    assert!(matches!(
        runtime
            .workspace()
            .commit_chart(unexpected_new_client)
            .await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));
}

#[tokio::test]
async fn begin_immediate_serializes_idempotency_and_note_revision_races() {
    let runtime = runtime().await;
    let request = full_graph_request("concurrent-identical");
    let (first, second) = tokio::join!(
        runtime.workspace().commit_chart(request.clone()),
        runtime.workspace().commit_chart(request)
    );
    let first = first.expect("first identical request");
    let second = second.expect("second identical request");
    assert_ne!(first.replayed, second.replayed);
    let mut normalized_first = first.clone();
    normalized_first.replayed = true;
    let mut normalized_second = second.clone();
    normalized_second.replayed = true;
    assert_eq!(normalized_first, normalized_second);

    let note = first.note.as_ref().expect("note");
    let mut left = identity_request("concurrent-note-left", &first.client.id);
    left.note = Some(WorkspaceChartNoteChange {
        upsert: WorkspaceNoteUpsert {
            id: Some(note.id.clone()),
            title: note.title.clone(),
            kind: note.kind.clone(),
            body: "Concurrent left".to_string(),
            status: note.status.clone(),
            ..Default::default()
        },
        expected_base_revision: Some(1),
    });
    let mut right = left.clone();
    right.idempotency_key = "concurrent-note-right".to_string();
    right.note.as_mut().expect("note").upsert.body = "Concurrent right".to_string();
    let (left_result, right_result) = tokio::join!(
        runtime.workspace().commit_chart(left),
        runtime.workspace().commit_chart(right)
    );
    let results = [left_result, right_result];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(WorkspaceChartCommitError::StaleNoteRevision { actual: 2, .. })
            ))
            .count(),
        1
    );
}

#[tokio::test]
async fn failed_existing_row_update_restores_deep_snapshot() {
    let runtime = runtime().await;
    let before = runtime
        .workspace()
        .commit_chart(full_graph_request("existing-before"))
        .await
        .expect("seed existing chart");
    let audits_before = runtime
        .workspace()
        .list_audit_events_filtered(WorkspaceAuditEventFilter {
            client_id: Some(before.client.id.clone()),
            limit: Some(200),
            ..Default::default()
        })
        .await
        .expect("audit snapshot");
    let revisions_before: Vec<RevisionSnapshot> = sqlx::query_as(
        r#"
SELECT revision, body, actor, source_thread_id, source_turn_id, summary
FROM workspace_note_revisions
WHERE note_id = ?
ORDER BY revision
            "#,
    )
    .bind(before.note.as_ref().expect("note").id.as_str())
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("revision snapshot");
    let receipts_before: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, request_sha256, result_json FROM workspace_chart_commits ORDER BY id",
    )
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("receipt snapshot");

    sqlx::query(
        r#"
CREATE TRIGGER fail_existing_update_receipt
BEFORE INSERT ON workspace_chart_commits
BEGIN
    SELECT RAISE(ABORT, 'injected existing update receipt failure');
END
        "#,
    )
    .execute(runtime.workspace().pool.as_ref())
    .await
    .expect("create update failure trigger");
    let mut update = full_graph_existing_request("existing-update-fails", &before);
    update.client.as_mut().expect("client").preferred_name = Some("Must roll back".to_string());
    update.encounter.as_mut().expect("encounter").title = "Changed encounter".to_string();
    update.note.as_mut().expect("note").upsert.body = "Changed note".to_string();
    update.document.as_mut().expect("document").notes = "Changed document".to_string();
    update
        .artifact_derivative
        .as_mut()
        .expect("derivative")
        .body = "Changed derivative".to_string();
    update.context_clip.as_mut().expect("clip").body = "Changed clip".to_string();
    update.task.as_mut().expect("task").details = "Changed task".to_string();
    assert!(matches!(
        runtime.workspace().commit_chart(update).await,
        Err(WorkspaceChartCommitError::Storage { .. })
    ));

    assert_eq!(
        runtime
            .workspace()
            .get_client(&before.client.id)
            .await
            .expect("client read"),
        Some(before.client.clone())
    );
    assert_eq!(
        runtime
            .workspace()
            .list_patient_safety_items(&before.client.id)
            .await
            .expect("safety read"),
        vec![before.safety_item.clone().expect("safety")]
    );
    assert_eq!(
        runtime
            .workspace()
            .list_encounters(&before.client.id)
            .await
            .expect("encounter read"),
        vec![before.encounter.clone().expect("encounter")]
    );
    assert_eq!(
        runtime
            .workspace()
            .get_note(&before.note.as_ref().expect("note").id)
            .await
            .expect("note read"),
        before.note.clone()
    );
    assert_eq!(
        runtime
            .workspace()
            .get_document(&before.document.as_ref().expect("document").id)
            .await
            .expect("document read"),
        before.document.clone()
    );
    assert_eq!(
        runtime
            .workspace()
            .list_artifact_derivatives(WorkspaceArtifactDerivativeFilter {
                client_id: before.client.id.clone(),
                limit: Some(10),
                ..Default::default()
            })
            .await
            .expect("derivative read"),
        vec![before.artifact_derivative.clone().expect("derivative")]
    );
    assert_eq!(
        runtime
            .workspace()
            .list_context_clips(WorkspaceContextClipFilter {
                client_id: before.client.id.clone(),
                limit: Some(10),
                ..Default::default()
            })
            .await
            .expect("clip read"),
        vec![before.context_clip.clone().expect("clip")]
    );
    assert_eq!(
        runtime
            .workspace()
            .list_tasks(&before.client.id)
            .await
            .expect("task read"),
        vec![before.task.clone().expect("task")]
    );
    assert_eq!(
        runtime
            .workspace()
            .list_audit_events_filtered(WorkspaceAuditEventFilter {
                client_id: Some(before.client.id.clone()),
                limit: Some(200),
                ..Default::default()
            })
            .await
            .expect("audit read"),
        audits_before
    );
    let revisions_after: Vec<RevisionSnapshot> = sqlx::query_as(
        r#"
SELECT revision, body, actor, source_thread_id, source_turn_id, summary
FROM workspace_note_revisions
WHERE note_id = ?
ORDER BY revision
        "#,
    )
    .bind(before.note.as_ref().expect("note").id.as_str())
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("revision read");
    assert_eq!(revisions_after, revisions_before);
    let receipts_after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, request_sha256, result_json FROM workspace_chart_commits ORDER BY id",
    )
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("receipt read");
    assert_eq!(receipts_after, receipts_before);
}

#[tokio::test]
async fn every_full_graph_write_and_audit_stage_rolls_back_atomically() {
    let stages = [
        (
            "client",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_clients BEGIN SELECT RAISE(ABORT, 'client'); END",
        ),
        (
            "client contact",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_client_contacts BEGIN SELECT RAISE(ABORT, 'contact'); END",
        ),
        (
            "client coverage",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_client_coverages BEGIN SELECT RAISE(ABORT, 'coverage'); END",
        ),
        (
            "normalized coverage",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_coverages BEGIN SELECT RAISE(ABORT, 'normalized coverage'); END",
        ),
        (
            "safety item",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_patient_safety_items BEGIN SELECT RAISE(ABORT, 'safety'); END",
        ),
        (
            "encounter",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_encounters BEGIN SELECT RAISE(ABORT, 'encounter'); END",
        ),
        (
            "note",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_notes BEGIN SELECT RAISE(ABORT, 'note'); END",
        ),
        (
            "note revision",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_note_revisions BEGIN SELECT RAISE(ABORT, 'revision'); END",
        ),
        (
            "document",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_documents BEGIN SELECT RAISE(ABORT, 'document'); END",
        ),
        (
            "derivative",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_artifact_derivatives BEGIN SELECT RAISE(ABORT, 'derivative'); END",
        ),
        (
            "clip",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_context_clips BEGIN SELECT RAISE(ABORT, 'clip'); END",
        ),
        (
            "task",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_tasks BEGIN SELECT RAISE(ABORT, 'task'); END",
        ),
        (
            "receipt",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_chart_commits BEGIN SELECT RAISE(ABORT, 'receipt'); END",
        ),
        (
            "client audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'client' BEGIN SELECT RAISE(ABORT, 'client audit'); END",
        ),
        (
            "coverage audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'coverage' BEGIN SELECT RAISE(ABORT, 'coverage audit'); END",
        ),
        (
            "safety audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'patient_safety_item' BEGIN SELECT RAISE(ABORT, 'safety audit'); END",
        ),
        (
            "encounter audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'encounter' BEGIN SELECT RAISE(ABORT, 'encounter audit'); END",
        ),
        (
            "note audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'note' BEGIN SELECT RAISE(ABORT, 'note audit'); END",
        ),
        (
            "document audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'document' BEGIN SELECT RAISE(ABORT, 'document audit'); END",
        ),
        (
            "derivative audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'artifact_derivative' BEGIN SELECT RAISE(ABORT, 'derivative audit'); END",
        ),
        (
            "clip audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'context_clip' BEGIN SELECT RAISE(ABORT, 'clip audit'); END",
        ),
        (
            "task audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'task' BEGIN SELECT RAISE(ABORT, 'task audit'); END",
        ),
        (
            "commit summary audit",
            "CREATE TRIGGER fail_stage BEFORE INSERT ON workspace_audit_events WHEN NEW.source = 'workspace_chart_commit' AND NEW.entity_type = 'chart_commit' BEGIN SELECT RAISE(ABORT, 'commit audit'); END",
        ),
    ];

    for (label, trigger) in stages {
        let runtime = runtime().await;
        sqlx::query(trigger)
            .execute(runtime.workspace().pool.as_ref())
            .await
            .unwrap_or_else(|error| panic!("create {label} trigger: {error}"));
        let mut request = full_graph_request(&format!("fail-{label}"));
        if label == "client coverage" {
            request.client.as_mut().expect("client").payer_name =
                Some("Synthetic legacy payer".to_string());
        }
        if matches!(label, "normalized coverage" | "coverage audit") {
            request.coverage = Some(WorkspaceCoverageUpsert {
                client_id: String::new(),
                priority: 1,
                payer_name: Some("Synthetic structured payer".to_string()),
                member_id: Some("SYN-ROLLBACK".to_string()),
                coverage_status: Some("active".to_string()),
                patient_relationship_to_subscriber: Some("self".to_string()),
                subscriber_address_same_as_patient: true,
                ..Default::default()
            });
        }
        let result = runtime.workspace().commit_chart(request).await;
        assert!(
            matches!(result, Err(WorkspaceChartCommitError::Storage { .. })),
            "stage {label} returned {result:?}"
        );
        assert_eq!(
            chart_row_count(&runtime).await,
            0,
            "partial rows at {label}"
        );
    }
}
