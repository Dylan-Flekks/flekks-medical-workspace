use super::*;
use crate::StateRuntime;
use crate::WorkspaceArtifactDerivativeUpsert;
use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartEntityKind;
use crate::WorkspaceChartNoteChange;
use crate::WorkspaceClient;
use crate::WorkspaceClientUpsert;
use crate::WorkspaceContextClipUpsert;
use crate::WorkspaceDocumentUpsert;
use crate::WorkspaceEncounterUpsert;
use crate::WorkspaceNoteSign;
use crate::WorkspaceNoteUpsert;
use crate::WorkspacePatientSafetyItemUpsert;
use crate::WorkspaceTaskUpsert;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use std::sync::Arc;

async fn runtime() -> Arc<StateRuntime> {
    StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize")
}

fn request(key: &str, client: WorkspaceClientUpsert) -> WorkspaceChartCommitRequest {
    let client_id = client.id.clone();
    WorkspaceChartCommitRequest {
        idempotency_key: key.to_string(),
        actor: "Dr. Rivera".to_string(),
        reason: "daily note save".to_string(),
        source_thread_id: Some("thread-1".to_string()),
        source_turn_id: Some("turn-1".to_string()),
        client_id,
        client: Some(client),
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

fn new_client(name: &str) -> WorkspaceClientUpsert {
    WorkspaceClientUpsert {
        display_name: name.to_string(),
        summary: "synthetic patient".to_string(),
        ..Default::default()
    }
}

fn existing_client(client: &WorkspaceClient) -> WorkspaceClientUpsert {
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
    }
}

fn note_change(id: Option<String>, body: &str, expected: Option<i64>) -> WorkspaceChartNoteChange {
    WorkspaceChartNoteChange {
        upsert: WorkspaceNoteUpsert {
            id,
            title: "Daily note".to_string(),
            kind: "daily_note".to_string(),
            body: body.to_string(),
            status: "draft".to_string(),
            ..Default::default()
        },
        expected_base_revision: expected,
    }
}

#[tokio::test]
async fn new_chart_commit_allocates_and_links_every_record() {
    let runtime = runtime().await;
    let mut input = request("full-chart", new_client("Synthetic Ada"));
    input.safety_item = Some(WorkspacePatientSafetyItemUpsert {
        client_id: "caller value for new root is overwritten".to_string(),
        category: "allergies".to_string(),
        name: "Synthetic latex".to_string(),
        ..Default::default()
    });
    input.encounter = Some(WorkspaceEncounterUpsert {
        kind: "visit".to_string(),
        title: "Daily visit".to_string(),
        status: "open".to_string(),
        ..Default::default()
    });
    input.note = Some(note_change(None, "Initial synthetic note", None));
    input.document = Some(WorkspaceDocumentUpsert {
        title: "Synthetic scan".to_string(),
        kind: "image".to_string(),
        local_path: "/synthetic/scan.png".to_string(),
        ..Default::default()
    });
    input.artifact_derivative = Some(WorkspaceArtifactDerivativeUpsert {
        title: "Reviewed text".to_string(),
        body: "Synthetic extracted text".to_string(),
        ..Default::default()
    });
    input.context_clip = Some(WorkspaceContextClipUpsert {
        title: "Relevant excerpt".to_string(),
        body: "Synthetic context".to_string(),
        ..Default::default()
    });
    input.task = Some(WorkspaceTaskUpsert {
        title: "Synthetic follow-up".to_string(),
        ..Default::default()
    });

    let result = runtime
        .workspace()
        .commit_chart(input)
        .await
        .expect("full chart should commit");

    assert_eq!(
        result.changed_entity_kinds,
        vec![
            WorkspaceChartEntityKind::Client,
            WorkspaceChartEntityKind::SafetyItem,
            WorkspaceChartEntityKind::Encounter,
            WorkspaceChartEntityKind::Note,
            WorkspaceChartEntityKind::Document,
            WorkspaceChartEntityKind::ArtifactDerivative,
            WorkspaceChartEntityKind::ContextClip,
            WorkspaceChartEntityKind::Task,
        ]
    );
    let encounter = result.encounter.as_ref().expect("encounter");
    let note = result.note.as_ref().expect("note");
    let document = result.document.as_ref().expect("document");
    let derivative = result.artifact_derivative.as_ref().expect("derivative");
    let clip = result.context_clip.as_ref().expect("clip");
    let task = result.task.as_ref().expect("task");
    assert_eq!(
        result.safety_item.as_ref().expect("safety item").client_id,
        result.client.id
    );
    assert_eq!(note.encounter_id.as_deref(), Some(encounter.id.as_str()));
    assert_eq!(
        document.encounter_id.as_deref(),
        Some(encounter.id.as_str())
    );
    assert_eq!(derivative.document_id, document.id);
    assert_eq!(derivative.note_id.as_deref(), Some(note.id.as_str()));
    assert_eq!(clip.derivative_id, derivative.id);
    assert_eq!(clip.document_id, document.id);
    assert_eq!(task.note_id.as_deref(), Some(note.id.as_str()));
    assert_eq!(result.resulting_note_revision, Some(1));

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE source = 'workspace_chart_commit'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("audit count");
    assert_eq!(audit_count, 9);
}

#[tokio::test]
async fn note_update_replays_exactly_and_rejects_key_reuse() {
    let runtime = runtime().await;
    let mut create = request("create-note", new_client("Synthetic Grace"));
    create.note = Some(note_change(None, "Revision one", None));
    let created = runtime
        .workspace()
        .commit_chart(create)
        .await
        .expect("create");
    let note = created.note.expect("note");

    let mut update = request(" update-note ", existing_client(&created.client));
    update.client = None;
    update.note = Some(note_change(Some(note.id.clone()), "Revision two", Some(1)));
    let updated = runtime
        .workspace()
        .commit_chart(update.clone())
        .await
        .expect("update");
    assert_eq!(
        updated.changed_entity_kinds,
        vec![WorkspaceChartEntityKind::Note]
    );
    assert_eq!(updated.resulting_note_revision, Some(2));

    let mut replay_request = update.clone();
    replay_request.idempotency_key = "update-note".to_string();
    let replay = runtime
        .workspace()
        .commit_chart(replay_request)
        .await
        .expect("replay");
    assert!(replay.replayed);
    let mut expected = updated.clone();
    expected.replayed = true;
    assert_eq!(replay, expected);

    update.note.as_mut().expect("note input").upsert.body = "Different body".to_string();
    let error = runtime
        .workspace()
        .commit_chart(update)
        .await
        .expect_err("changed request must conflict");
    assert!(matches!(
        error,
        WorkspaceChartCommitError::IdempotencyConflict { .. }
    ));

    let audit_before_no_op: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE source = 'workspace_chart_commit'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("audit count");
    let mut no_op = request("note-no-op", existing_client(&created.client));
    no_op.client = None;
    no_op.note = Some(note_change(Some(note.id.clone()), "Revision two", Some(2)));
    let no_op_result = runtime
        .workspace()
        .commit_chart(no_op)
        .await
        .expect("unchanged note should produce receipt");
    assert!(no_op_result.changed_entity_kinds.is_empty());
    let audit_after_no_op: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE source = 'workspace_chart_commit'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("audit count");
    assert_eq!(audit_after_no_op, audit_before_no_op);
    let revision_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_note_revisions WHERE note_id = ?")
            .bind(note.id)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("revision count");
    assert_eq!(revision_count, 2);
}

#[tokio::test]
async fn no_op_and_demographic_only_commits_do_not_create_unrelated_rows() {
    let runtime = runtime().await;
    let created = runtime
        .workspace()
        .commit_chart(request("create-client", new_client("Synthetic Lin")))
        .await
        .expect("create client");
    let audit_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_audit_events")
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .expect("audit count");

    let mut no_op_request = request("no-op", existing_client(&created.client));
    no_op_request.client = None;
    let no_op = runtime
        .workspace()
        .commit_chart(no_op_request)
        .await
        .expect("no-op receipt");
    assert!(no_op.changed_entity_kinds.is_empty());
    let audit_after_no_op: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspace_audit_events")
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .expect("audit count");
    assert_eq!(audit_after_no_op, audit_before);

    let mut demographics = existing_client(&created.client);
    demographics.preferred_name = Some("Lin".to_string());
    let mut demographics_request = request("demographics", demographics);
    demographics_request.expected_versions.client = Some(
        created
            .client
            .record_version()
            .expect("client record version"),
    );
    let result = runtime
        .workspace()
        .commit_chart(demographics_request)
        .await
        .expect("demographic update");
    assert_eq!(
        result.changed_entity_kinds,
        vec![WorkspaceChartEntityKind::Client]
    );
    for (table, query) in [
        (
            "workspace_encounters",
            "SELECT COUNT(*) FROM workspace_encounters",
        ),
        ("workspace_notes", "SELECT COUNT(*) FROM workspace_notes"),
        (
            "workspace_note_revisions",
            "SELECT COUNT(*) FROM workspace_note_revisions",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("row count");
        assert_eq!(count, 0, "unexpected rows in {table}");
    }
}

#[tokio::test]
async fn stale_locked_and_cross_patient_changes_fail_closed() {
    let runtime = runtime().await;
    let mut first = request("patient-a", new_client("Synthetic Patient A"));
    first.note = Some(note_change(None, "Patient A note", None));
    let patient_a = runtime
        .workspace()
        .commit_chart(first)
        .await
        .expect("patient A");
    let note = patient_a.note.as_ref().expect("note");
    let patient_b = runtime
        .workspace()
        .commit_chart(request("patient-b", new_client("Synthetic Patient B")))
        .await
        .expect("patient B");

    let mut stale = request("stale", existing_client(&patient_a.client));
    stale.client = None;
    stale.note = Some(note_change(Some(note.id.clone()), "stale edit", Some(0)));
    assert!(matches!(
        runtime.workspace().commit_chart(stale).await,
        Err(WorkspaceChartCommitError::StaleNoteRevision { .. })
    ));

    runtime
        .workspace()
        .sign_note(WorkspaceNoteSign {
            note_id: note.id.clone(),
            signer: "Dr. Rivera".to_string(),
        })
        .await
        .expect("sign note");
    let mut locked = request("locked", existing_client(&patient_a.client));
    locked.client = None;
    locked.note = Some(note_change(Some(note.id.clone()), "locked edit", Some(1)));
    assert!(matches!(
        runtime.workspace().commit_chart(locked).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut cross_patient = request("cross-patient", existing_client(&patient_a.client));
    cross_patient.client = None;
    cross_patient.safety_item = Some(WorkspacePatientSafetyItemUpsert {
        client_id: patient_b.client.id,
        category: "allergy".to_string(),
        name: "Synthetic conflict".to_string(),
        ..Default::default()
    });
    assert!(matches!(
        runtime.workspace().commit_chart(cross_patient).await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));
}

#[tokio::test]
async fn sqlite_abort_rolls_back_records_audits_and_receipt() {
    let runtime = runtime().await;
    sqlx::query(
        r#"
CREATE TRIGGER fail_chart_audit
BEFORE INSERT ON workspace_audit_events
WHEN NEW.source = 'workspace_chart_commit'
BEGIN
    SELECT RAISE(ABORT, 'injected chart audit failure');
END
        "#,
    )
    .execute(runtime.workspace().pool.as_ref())
    .await
    .expect("create failure trigger");

    let mut input = request("rollback", new_client("Synthetic Rollback"));
    input.encounter = Some(WorkspaceEncounterUpsert {
        kind: "visit".to_string(),
        title: "Must roll back".to_string(),
        status: "open".to_string(),
        ..Default::default()
    });
    assert!(matches!(
        runtime.workspace().commit_chart(input).await,
        Err(WorkspaceChartCommitError::Storage { .. })
    ));

    for (table, query) in [
        (
            "workspace_clients",
            "SELECT COUNT(*) FROM workspace_clients",
        ),
        (
            "workspace_encounters",
            "SELECT COUNT(*) FROM workspace_encounters",
        ),
        (
            "workspace_audit_events",
            "SELECT COUNT(*) FROM workspace_audit_events",
        ),
        (
            "workspace_chart_commits",
            "SELECT COUNT(*) FROM workspace_chart_commits",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("row count");
        assert_eq!(count, 0, "partial rows remained in {table}");
    }
}

#[tokio::test]
async fn receipt_failure_rolls_back_the_complete_changeset() {
    let runtime = runtime().await;
    sqlx::query(
        r#"
CREATE TRIGGER fail_chart_receipt
BEFORE INSERT ON workspace_chart_commits
BEGIN
    SELECT RAISE(ABORT, 'injected chart receipt failure');
END
        "#,
    )
    .execute(runtime.workspace().pool.as_ref())
    .await
    .expect("create receipt failure trigger");

    let mut input = request("receipt-rollback", new_client("Synthetic Receipt Rollback"));
    input.encounter = Some(WorkspaceEncounterUpsert {
        kind: "visit".to_string(),
        title: "Late rollback".to_string(),
        status: "open".to_string(),
        ..Default::default()
    });
    input.note = Some(note_change(None, "Must roll back", None));
    assert!(matches!(
        runtime.workspace().commit_chart(input).await,
        Err(WorkspaceChartCommitError::Storage { .. })
    ));

    let remaining_rows: i64 = sqlx::query_scalar(
        r#"
SELECT
    (SELECT COUNT(*) FROM workspace_clients)
  + (SELECT COUNT(*) FROM workspace_encounters)
  + (SELECT COUNT(*) FROM workspace_notes)
  + (SELECT COUNT(*) FROM workspace_note_revisions)
  + (SELECT COUNT(*) FROM workspace_audit_events)
  + (SELECT COUNT(*) FROM workspace_chart_commits)
        "#,
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("remaining row count");
    assert_eq!(remaining_rows, 0);
}
