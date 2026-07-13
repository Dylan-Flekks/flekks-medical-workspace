use super::*;
use crate::StateRuntime;
use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartCommitRequest;
use crate::WorkspaceChartExpectedVersions;
use crate::WorkspaceChartNoteChange;
use crate::WorkspaceClientUpsert;
use crate::WorkspaceDocumentUpsert;
use crate::WorkspaceNoteUpsert;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sqlx::sqlite::SqlitePoolOptions;
use std::borrow::Cow;

async fn runtime() -> std::sync::Arc<StateRuntime> {
    StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize")
}

fn patient(id: Option<String>, last_name: &str, phone: &str) -> WorkspaceClientUpsert {
    WorkspaceClientUpsert {
        id,
        display_name: format!("Avery {last_name}"),
        legal_first_name: Some("Avery".to_string()),
        legal_middle_name: Some("Q".to_string()),
        legal_last_name: Some(last_name.to_string()),
        date_of_birth: Some("1980-04-11".to_string()),
        administrative_sex: Some("female".to_string()),
        preferred_language: Some("English".to_string()),
        primary_phone: Some(phone.to_string()),
        primary_phone_use: Some("mobile".to_string()),
        email: Some("avery@example.test".to_string()),
        primary_email: Some("avery@example.test".to_string()),
        address_line_1: Some("100 Test Street".to_string()),
        city: Some("Testville".to_string()),
        state_or_province: Some("IL".to_string()),
        postal_code: Some("60601".to_string()),
        country: Some("US".to_string()),
        address_use: Some("home_and_mailing".to_string()),
        payer_name: Some("Medicare".to_string()),
        member_id: Some("1EG4TE5MK73".to_string()),
        coverage_type: Some("medicare".to_string()),
        coverage_status: Some("active".to_string()),
        summary: "synthetic patient".to_string(),
        ..Default::default()
    }
}

fn coverage_commit(
    key: &str,
    client_id: &str,
    coverage: WorkspaceCoverageUpsert,
    expected_version: Option<String>,
) -> WorkspaceChartCommitRequest {
    WorkspaceChartCommitRequest {
        idempotency_key: key.to_string(),
        actor: "Dr. Test".to_string(),
        reason: "coverage edit".to_string(),
        source_thread_id: None,
        source_turn_id: None,
        client_id: Some(client_id.to_string()),
        client: None,
        coverage: Some(coverage),
        expected_versions: WorkspaceChartExpectedVersions {
            coverage: expected_version,
            ..Default::default()
        },
        safety_item: None,
        encounter: None,
        note: None,
        document: None,
        artifact_derivative: None,
        context_clip: None,
        task: None,
    }
}

async fn card_document(runtime: &StateRuntime, client_id: &str) -> crate::WorkspaceDocument {
    runtime
        .workspace()
        .upsert_document(WorkspaceDocumentUpsert {
            client_id: client_id.to_string(),
            title: "Synthetic Medicare card".to_string(),
            kind: "insurance_card".to_string(),
            local_path: "/synthetic/medicare-card.png".to_string(),
            scope: "patient".to_string(),
            detected_kind: "insurance_card".to_string(),
            existence_status: "present".to_string(),
            metadata_json: "{}".to_string(),
            original_path: "/synthetic/medicare-card.png".to_string(),
            reference_kind: "local_reference".to_string(),
            content_sha256: Some("a".repeat(64)),
            thumbnail_status: "none".to_string(),
            ..Default::default()
        })
        .await
        .expect("card document should save")
}

#[tokio::test]
async fn card_match_stays_current_for_contact_edits_and_stales_for_identity_edits() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(patient(None, "Test", "312-555-0100"))
        .await
        .expect("patient should save");
    let coverage = runtime
        .workspace()
        .list_coverages(&client.id, None, 3)
        .await
        .expect("coverage should list")
        .into_iter()
        .next()
        .expect("legacy primary projection should exist");
    assert_eq!(
        runtime
            .workspace()
            .coverage_billing_readiness(&coverage)
            .await
            .expect("readiness should derive"),
        WorkspaceBillingReadiness::Unverified
    );
    let document = card_document(&runtime, &client.id).await;
    let verification = runtime
        .workspace()
        .create_coverage_verification(WorkspaceCoverageVerificationCreate {
            coverage_id: coverage.id.clone(),
            source_document_id: document.id.clone(),
            expected_patient_version: client.record_version().expect("patient version"),
            expected_coverage_version: coverage.record_version().expect("coverage version"),
            expected_document_version: document.record_version().expect("document version"),
            compared_subject: WorkspaceCoverageVerificationSubject::Beneficiary,
            observed_first_name: Some("avery".to_string()),
            observed_middle_name: Some("Q".to_string()),
            observed_last_name: Some("TEST".to_string()),
            observed_suffix: None,
            observed_member_id: Some("1EG4-TE5-MK73".to_string()),
            actor: "Dr. Test".to_string(),
        })
        .await
        .expect("matching verification should save")
        .verification;
    assert_eq!(
        verification.match_result,
        WorkspaceCoverageMatchResult::Match
    );
    assert_eq!(
        verification.observed_member_id.as_deref(),
        Some("1EG4TE5MK73")
    );

    runtime
        .workspace()
        .upsert_client(patient(Some(client.id.clone()), "Test", "312-555-0199"))
        .await
        .expect("contact-only edit should save");
    let after_contact = runtime
        .workspace()
        .list_coverage_verifications(&coverage.id, None, 1)
        .await
        .expect("verification should list")
        .remove(0);
    assert!(!after_contact.is_stale);

    runtime
        .workspace()
        .upsert_client(patient(Some(client.id.clone()), "Changed", "312-555-0199"))
        .await
        .expect("identity edit should save");
    let after_identity = runtime
        .workspace()
        .list_coverage_verifications(&coverage.id, None, 1)
        .await
        .expect("verification should list")
        .remove(0);
    assert!(after_identity.is_stale);
    assert_eq!(
        runtime
            .workspace()
            .coverage_billing_readiness(&coverage)
            .await
            .expect("readiness should derive"),
        WorkspaceBillingReadiness::Stale
    );

    let changed_client = runtime
        .workspace()
        .get_client(&client.id)
        .await
        .expect("changed patient read")
        .expect("changed patient");
    let mismatch = runtime
        .workspace()
        .create_coverage_verification(WorkspaceCoverageVerificationCreate {
            coverage_id: coverage.id.clone(),
            source_document_id: document.id.clone(),
            expected_patient_version: changed_client
                .record_version()
                .expect("changed patient version"),
            expected_coverage_version: coverage.record_version().expect("coverage version"),
            expected_document_version: document.record_version().expect("document version"),
            compared_subject: WorkspaceCoverageVerificationSubject::Beneficiary,
            observed_first_name: Some("Avery".to_string()),
            observed_middle_name: Some("Q".to_string()),
            observed_last_name: Some("Test".to_string()),
            observed_suffix: None,
            observed_member_id: Some("1EG4TE5MK73".to_string()),
            actor: "Dr. Test".to_string(),
        })
        .await
        .expect("mismatch should remain auditable")
        .verification;
    assert_eq!(
        mismatch.match_result,
        WorkspaceCoverageMatchResult::Mismatch
    );
    assert_eq!(mismatch.mismatch_fields, vec!["lastName"]);

    let note_result = runtime
        .workspace()
        .commit_chart(WorkspaceChartCommitRequest {
            idempotency_key: "note-after-mismatch".to_string(),
            actor: "Dr. Test".to_string(),
            reason: "clinical note remains independent".to_string(),
            source_thread_id: None,
            source_turn_id: None,
            client_id: Some(client.id),
            client: None,
            coverage: None,
            expected_versions: Default::default(),
            safety_item: None,
            encounter: None,
            note: Some(WorkspaceChartNoteChange {
                upsert: WorkspaceNoteUpsert {
                    title: "Clinical note".to_string(),
                    body: "Billing mismatch does not block care.".to_string(),
                    status: "draft".to_string(),
                    ..Default::default()
                },
                expected_base_revision: None,
            }),
            document: None,
            artifact_derivative: None,
            context_clip: None,
            task: None,
        })
        .await
        .expect("clinical save must not be blocked by billing mismatch");
    assert!(note_result.note.is_some());
}

#[tokio::test]
async fn readiness_snapshot_remains_consistent_across_a_concurrent_coverage_commit() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(patient(None, "Snapshot", "312-555-0110"))
        .await
        .expect("patient should save");
    let coverage = runtime
        .workspace()
        .list_coverages(&client.id, None, 3)
        .await
        .expect("coverage should list")
        .remove(0);
    let document = card_document(&runtime, &client.id).await;
    runtime
        .workspace()
        .create_coverage_verification(WorkspaceCoverageVerificationCreate {
            coverage_id: coverage.id.clone(),
            source_document_id: document.id.clone(),
            expected_patient_version: client.record_version().expect("patient version"),
            expected_coverage_version: coverage.record_version().expect("coverage version"),
            expected_document_version: document.record_version().expect("document version"),
            compared_subject: WorkspaceCoverageVerificationSubject::Beneficiary,
            observed_first_name: Some("Avery".to_string()),
            observed_middle_name: Some("Q".to_string()),
            observed_last_name: Some("Snapshot".to_string()),
            observed_suffix: None,
            observed_member_id: Some("1EG4TE5MK73".to_string()),
            actor: "Dr. Test".to_string(),
        })
        .await
        .expect("matching verification should save");

    let mut snapshot_tx = runtime
        .workspace()
        .pool
        .begin()
        .await
        .expect("snapshot transaction should begin");
    let snapshot_coverage = coverage_in_tx(&mut snapshot_tx, &coverage.id)
        .await
        .expect("snapshot coverage read")
        .expect("snapshot coverage");
    let snapshot_client =
        super::super::workspace_chart_commit_sql::client(&mut snapshot_tx, &client.id)
            .await
            .expect("snapshot patient read")
            .expect("snapshot patient");

    let update = WorkspaceCoverageUpsert {
        id: Some(coverage.id.clone()),
        client_id: client.id.clone(),
        priority: 1,
        payer_name: Some("Medicare".to_string()),
        member_id: Some("1EG4TE5MK73".to_string()),
        coverage_type: Some("medicare".to_string()),
        coverage_status: Some("active".to_string()),
        patient_relationship_to_subscriber: Some("self".to_string()),
        subscriber_address_same_as_patient: true,
        coverage_notes: Some("concurrent structured update".to_string()),
        ..Default::default()
    };
    let updated_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        runtime.workspace().commit_chart(coverage_commit(
            "snapshot-concurrent-update",
            &client.id,
            update,
            Some(coverage.record_version().expect("coverage version")),
        )),
    )
    .await
    .expect("concurrent commit should not block behind a WAL reader")
    .expect("concurrent coverage update should save");
    assert_eq!(
        updated_result.coverage_billing_readiness,
        Some(WorkspaceBillingReadiness::Stale)
    );

    let snapshot_readiness =
        coverage_billing_readiness_in_tx(&mut snapshot_tx, &snapshot_client, &snapshot_coverage)
            .await
            .expect("snapshot readiness should derive");
    assert_eq!(snapshot_readiness, WorkspaceBillingReadiness::Match);
    snapshot_tx
        .commit()
        .await
        .expect("snapshot transaction should close");

    let current = runtime
        .workspace()
        .list_coverages_with_billing_readiness(&client.id, None, 3)
        .await
        .expect("current coverage snapshot should list");
    let updated_coverage = updated_result.coverage.expect("updated coverage");
    assert_eq!(
        current,
        vec![(updated_coverage, WorkspaceBillingReadiness::Stale)]
    );
}

#[tokio::test]
async fn chart_commit_enforces_coverage_priority_and_version() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(patient(None, "Dependent", "312-555-0101"))
        .await
        .expect("patient should save");
    let secondary = WorkspaceCoverageUpsert {
        client_id: client.id.clone(),
        priority: 2,
        payer_name: Some("Synthetic Secondary".to_string()),
        member_id: Some("SYN-200".to_string()),
        coverage_status: Some("active".to_string()),
        patient_relationship_to_subscriber: Some("child".to_string()),
        subscriber_first_name: Some("Jordan".to_string()),
        subscriber_last_name: Some("Subscriber".to_string()),
        subscriber_date_of_birth: Some("1970-01-01".to_string()),
        subscriber_administrative_sex: Some("male".to_string()),
        subscriber_address_same_as_patient: true,
        ..Default::default()
    };
    let created = runtime
        .workspace()
        .commit_chart(coverage_commit(
            "secondary-create",
            &client.id,
            secondary,
            None,
        ))
        .await
        .expect("secondary coverage should save")
        .coverage
        .expect("coverage result");
    assert_eq!(created.priority, 2);

    let duplicate = WorkspaceCoverageUpsert {
        client_id: client.id.clone(),
        priority: 2,
        payer_name: Some("Duplicate".to_string()),
        ..Default::default()
    };
    assert!(matches!(
        runtime
            .workspace()
            .commit_chart(coverage_commit(
                "secondary-duplicate",
                &client.id,
                duplicate,
                None,
            ))
            .await,
        Err(WorkspaceChartCommitError::Validation { .. })
    ));

    let mut update = WorkspaceCoverageUpsert {
        id: Some(created.id.clone()),
        client_id: client.id.clone(),
        priority: 2,
        payer_name: Some("Updated Secondary".to_string()),
        ..Default::default()
    };
    let stale = runtime
        .workspace()
        .commit_chart(coverage_commit(
            "secondary-stale",
            &client.id,
            update.clone(),
            Some("stale-version".to_string()),
        ))
        .await;
    assert!(matches!(
        stale,
        Err(WorkspaceChartCommitError::StaleEntityVersion { .. })
    ));
    update.coverage_notes = Some("human-reviewed".to_string());
    let updated = runtime
        .workspace()
        .commit_chart(coverage_commit(
            "secondary-update",
            &client.id,
            update,
            Some(created.record_version().expect("coverage version")),
        ))
        .await
        .expect("version-pinned coverage edit should save");
    assert_eq!(updated.coverage.expect("coverage").priority, 2);
}

#[tokio::test]
async fn chart_commit_creates_new_patient_and_structured_primary_atomically() {
    let runtime = runtime().await;
    let mut client = patient(None, "Atomic", "312-555-0102");
    client.payer_name = Some("Legacy value must not preempt structured coverage".to_string());
    let request = WorkspaceChartCommitRequest {
        idempotency_key: "atomic-client-primary".to_string(),
        actor: "Dr. Test".to_string(),
        reason: "create patient and primary coverage".to_string(),
        source_thread_id: None,
        source_turn_id: None,
        client_id: None,
        client: Some(client),
        coverage: Some(WorkspaceCoverageUpsert {
            client_id: String::new(),
            priority: 1,
            payer_name: Some("Structured Primary".to_string()),
            member_id: Some("SYN-PRIMARY-1".to_string()),
            coverage_status: Some("active".to_string()),
            patient_relationship_to_subscriber: Some("self".to_string()),
            subscriber_address_same_as_patient: true,
            ..Default::default()
        }),
        expected_versions: Default::default(),
        safety_item: None,
        encounter: None,
        note: None,
        document: None,
        artifact_derivative: None,
        context_clip: None,
        task: None,
    };
    let result = runtime
        .workspace()
        .commit_chart(request)
        .await
        .expect("new patient and structured primary should commit together");
    let coverage = result.coverage.expect("primary coverage");
    assert_eq!(
        result.coverage_billing_readiness,
        Some(WorkspaceBillingReadiness::Unverified)
    );
    assert_eq!(coverage.client_id, result.client.id);
    assert_eq!(coverage.payer_name.as_deref(), Some("Structured Primary"));
    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_coverages WHERE client_id = ? AND priority = 1",
    )
    .bind(&result.client.id)
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("primary row count");
    assert_eq!(rows, 1);
    let source_kind: String =
        sqlx::query_scalar("SELECT source_kind FROM workspace_coverages WHERE id = ?")
            .bind(&coverage.id)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("coverage source kind");
    assert_eq!(source_kind, "structured");
}

#[tokio::test]
async fn legacy_client_update_cannot_rewrite_structured_primary() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(patient(None, "Projection", "312-555-0103"))
        .await
        .expect("patient should save");
    let original = runtime
        .workspace()
        .list_coverages(&client.id, None, 3)
        .await
        .expect("coverage list")
        .remove(0);
    let structured = runtime
        .workspace()
        .commit_chart(coverage_commit(
            "structured-primary",
            &client.id,
            WorkspaceCoverageUpsert {
                id: Some(original.id.clone()),
                client_id: client.id.clone(),
                priority: 1,
                payer_name: Some("Structured Authority".to_string()),
                member_id: Some("STRUCTURED-1".to_string()),
                coverage_status: Some("active".to_string()),
                effective_date: Some("2026-01-01".to_string()),
                patient_relationship_to_subscriber: Some("self".to_string()),
                subscriber_address_same_as_patient: true,
                ..Default::default()
            },
            Some(original.record_version().expect("coverage version")),
        ))
        .await
        .expect("structured coverage update")
        .coverage
        .expect("structured coverage");
    let source_kind: String =
        sqlx::query_scalar("SELECT source_kind FROM workspace_coverages WHERE id = ?")
            .bind(&structured.id)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("structured source kind");
    assert_eq!(source_kind, "structured");
    let mut legacy_update = patient(Some(client.id.clone()), "Projection", "312-555-0199");
    legacy_update.payer_name = Some("Stale Legacy Payer".to_string());
    legacy_update.member_id = Some("STALE-LEGACY".to_string());
    runtime
        .workspace()
        .upsert_client(legacy_update)
        .await
        .expect("legacy demographic update");
    let after = runtime
        .workspace()
        .get_coverage(&structured.id)
        .await
        .expect("coverage read")
        .expect("coverage retained");
    assert_eq!(after, structured);
}

#[tokio::test]
async fn verification_requires_current_snapshot_and_history_is_append_only() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(patient(None, "Van   Test", "312-555-0104"))
        .await
        .expect("patient should save");
    let coverage = runtime
        .workspace()
        .list_coverages(&client.id, None, 3)
        .await
        .expect("coverage list")
        .remove(0);
    let ineligible_document = runtime
        .workspace()
        .upsert_document(WorkspaceDocumentUpsert {
            client_id: client.id.clone(),
            title: "Not an insurance card".to_string(),
            kind: "document".to_string(),
            local_path: "/synthetic/not-a-card.txt".to_string(),
            scope: "patient".to_string(),
            existence_status: "present".to_string(),
            reference_kind: "local_reference".to_string(),
            ..Default::default()
        })
        .await
        .expect("ineligible document");
    let ineligible = WorkspaceCoverageVerificationCreate {
        coverage_id: coverage.id.clone(),
        source_document_id: ineligible_document.id.clone(),
        expected_patient_version: client.record_version().expect("patient version"),
        expected_coverage_version: coverage.record_version().expect("coverage version"),
        expected_document_version: ineligible_document
            .record_version()
            .expect("document version"),
        compared_subject: WorkspaceCoverageVerificationSubject::Beneficiary,
        observed_first_name: Some("Avery".to_string()),
        observed_middle_name: Some("Q".to_string()),
        observed_last_name: Some("Van Test".to_string()),
        observed_suffix: None,
        observed_member_id: Some("1EG4TE5MK73".to_string()),
        actor: "Dr. Test".to_string(),
    };
    assert!(
        runtime
            .workspace()
            .create_coverage_verification(ineligible)
            .await
            .expect_err("ineligible card document must fail")
            .to_string()
            .contains("must be a present patient-scoped local reference")
    );
    let document = card_document(&runtime, &client.id).await;
    let input = WorkspaceCoverageVerificationCreate {
        coverage_id: coverage.id.clone(),
        source_document_id: document.id.clone(),
        expected_patient_version: client.record_version().expect("patient version"),
        expected_coverage_version: coverage.record_version().expect("coverage version"),
        expected_document_version: document.record_version().expect("document version"),
        compared_subject: WorkspaceCoverageVerificationSubject::Beneficiary,
        observed_first_name: Some("Avery".to_string()),
        observed_middle_name: Some("Q".to_string()),
        observed_last_name: Some("Van Test".to_string()),
        observed_suffix: None,
        observed_member_id: Some("1EG4TE5MK73".to_string()),
        actor: "Dr. Test".to_string(),
    };
    let mut stale = input.clone();
    stale.expected_document_version = "0".repeat(64);
    assert!(
        runtime
            .workspace()
            .create_coverage_verification(stale)
            .await
            .expect_err("stale document pin must fail")
            .to_string()
            .contains("card document changed")
    );
    let result = runtime
        .workspace()
        .create_coverage_verification(input)
        .await
        .expect("current card snapshot should verify");
    assert_eq!(result.billing_readiness, WorkspaceBillingReadiness::Match);
    assert_eq!(
        result.verification.match_result,
        WorkspaceCoverageMatchResult::Match
    );
    assert_eq!(
        result.verification.source_document_content_sha256,
        "a".repeat(64)
    );
    assert_eq!(
        result.verification.source_document_version,
        document.record_version().expect("document version")
    );

    let update = sqlx::query(
        "UPDATE workspace_coverage_card_verifications SET actor = 'mutated' WHERE id = ?",
    )
    .bind(&result.verification.id)
    .execute(runtime.workspace().pool.as_ref())
    .await;
    assert!(update.is_err());
    let delete = sqlx::query("DELETE FROM workspace_coverage_card_verifications WHERE id = ?")
        .bind(&result.verification.id)
        .execute(runtime.workspace().pool.as_ref())
        .await;
    assert!(delete.is_err());

    sqlx::query("DELETE FROM workspace_client_coverages WHERE client_id = ?")
        .bind(&client.id)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("legacy projection deletion should deactivate, not delete normalized coverage");
    let retained = runtime
        .workspace()
        .get_coverage(&coverage.id)
        .await
        .expect("coverage read")
        .expect("normalized coverage retained");
    assert_eq!(retained.coverage_status.as_deref(), Some("inactive"));
    assert_eq!(
        runtime
            .workspace()
            .list_coverage_verifications(&coverage.id, None, 10)
            .await
            .expect("verification history")
            .len(),
        1
    );
    let coverage_delete = sqlx::query("DELETE FROM workspace_coverages WHERE id = ?")
        .bind(&coverage.id)
        .execute(runtime.workspace().pool.as_ref())
        .await;
    assert!(coverage_delete.is_err());
}

#[tokio::test]
async fn legacy_single_coverage_migrates_to_deterministic_primary() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database should open");
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
        .expect("legacy workspace schema should apply");
    sqlx::query(
        "INSERT INTO workspace_clients (id, display_name, summary, created_at_ms, updated_at_ms) VALUES ('patient-1', 'Synthetic', '', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy patient should insert");
    sqlx::query(
        "INSERT INTO workspace_client_coverages (client_id, payer_name, member_id, created_at_ms, updated_at_ms) VALUES ('patient-1', 'Legacy Payer', 'LEGACY-1', 1, 2)",
    )
    .execute(&pool)
    .await
    .expect("legacy coverage should insert");
    base.run(&pool)
        .await
        .expect("current workspace migration should apply");
    let migrated = sqlx::query(
        "SELECT id, client_id, priority, payer_name, member_id FROM workspace_coverages",
    )
    .fetch_one(&pool)
    .await
    .expect("migrated coverage should load");
    assert_eq!(migrated.get::<String, _>("id"), "legacy-primary:patient-1");
    assert_eq!(migrated.get::<String, _>("client_id"), "patient-1");
    assert_eq!(migrated.get::<i64, _>("priority"), 1);
    assert_eq!(migrated.get::<String, _>("payer_name"), "Legacy Payer");
    assert_eq!(migrated.get::<String, _>("member_id"), "LEGACY-1");

    sqlx::query(
        "UPDATE workspace_client_coverages SET payer_name = 'Legacy Updated', updated_at_ms = 3 WHERE client_id = 'patient-1'",
    )
    .execute(&pool)
    .await
    .expect("legacy projection update should remain supported");
    let synchronized: String = sqlx::query_scalar(
        "SELECT payer_name FROM workspace_coverages WHERE client_id = 'patient-1' AND priority = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("normalized primary should follow the legacy projection");
    assert_eq!(synchronized, "Legacy Updated");

    sqlx::query(
        "UPDATE workspace_coverages SET payer_name = 'Structured Authority', source_kind = 'structured', updated_at_ms = 4 WHERE client_id = 'patient-1' AND priority = 1",
    )
    .execute(&pool)
    .await
    .expect("promote normalized coverage");
    sqlx::query(
        "UPDATE workspace_client_coverages SET payer_name = 'Stale Legacy Rewrite', updated_at_ms = 5 WHERE client_id = 'patient-1'",
    )
    .execute(&pool)
    .await
    .expect("legacy writer update");
    sqlx::query("DELETE FROM workspace_client_coverages WHERE client_id = 'patient-1'")
        .execute(&pool)
        .await
        .expect("legacy writer delete");
    let protected: (String, String) = sqlx::query_as(
        "SELECT payer_name, source_kind FROM workspace_coverages WHERE client_id = 'patient-1' AND priority = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("structured coverage survives legacy writes");
    assert_eq!(
        protected,
        ("Structured Authority".to_string(), "structured".to_string())
    );
}

#[test]
fn medicare_beneficiary_identifier_uses_cms_position_rules() {
    assert!(valid_mbi("1EG4TE5MK73"));
    assert!(!valid_mbi("0EG4TE5MK73"));
    assert!(!valid_mbi("11G4TE5MK73"));
    assert!(!valid_mbi("1SG4TE5MK73"));
    assert!(!valid_mbi("1EGATE5MK73"));
}
