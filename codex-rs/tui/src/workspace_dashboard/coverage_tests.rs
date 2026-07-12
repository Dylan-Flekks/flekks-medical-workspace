use super::*;
use codex_app_server_protocol::WorkspaceCoverage;
use codex_app_server_protocol::WorkspaceCoverageUpsertParams;
use pretty_assertions::assert_eq;

fn stored_coverage() -> WorkspaceCoverage {
    WorkspaceCoverage {
        id: "coverage-2".to_string(),
        version: "coverage-version-7".to_string(),
        client_id: "patient-1".to_string(),
        priority: 2,
        payer_name: Some("Example Health".to_string()),
        plan_name: Some("Community PPO".to_string()),
        member_id: Some("ABC123456".to_string()),
        group_number: Some("GROUP-7".to_string()),
        coverage_type: Some("commercial".to_string()),
        coverage_status: Some("active".to_string()),
        effective_date: Some("2026-01-01".to_string()),
        termination_date: None,
        patient_relationship_to_subscriber: Some("child".to_string()),
        subscriber_first_name: Some("Alex".to_string()),
        subscriber_middle_name: Some("Q".to_string()),
        subscriber_last_name: Some("Example".to_string()),
        subscriber_suffix: Some("Jr".to_string()),
        subscriber_date_of_birth: Some("1982-03-04".to_string()),
        subscriber_administrative_sex: Some("male".to_string()),
        subscriber_address_same_as_patient: false,
        subscriber_address_line_1: Some("100 Main Street".to_string()),
        subscriber_address_line_2: Some("Unit 2".to_string()),
        subscriber_city: Some("Madison".to_string()),
        subscriber_state_or_province: Some("WI".to_string()),
        subscriber_postal_code: Some("53703".to_string()),
        subscriber_country: Some("US".to_string()),
        coverage_notes: Some("Confirm coordination of benefits.".to_string()),
        billing_readiness: WorkspaceBillingReadiness::Match,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_100,
    }
}

#[test]
fn coverage_priorities_wrap_in_both_directions() {
    let forward = CoveragePriority::ALL
        .into_iter()
        .map(CoveragePriority::next_wrapping)
        .collect::<Vec<_>>();
    let backward = CoveragePriority::ALL
        .into_iter()
        .map(CoveragePriority::previous_wrapping)
        .collect::<Vec<_>>();

    assert_eq!(
        forward,
        vec![
            CoveragePriority::Secondary,
            CoveragePriority::Tertiary,
            CoveragePriority::Primary,
        ]
    );
    assert_eq!(
        backward,
        vec![
            CoveragePriority::Tertiary,
            CoveragePriority::Primary,
            CoveragePriority::Secondary,
        ]
    );
}

#[test]
fn coverage_field_navigation_preserves_the_form_order() {
    let traversed = std::iter::successors(Some(CoverageField::PayerName), |field| {
        Some(field.next_wrapping())
    })
    .take(CoverageField::ALL.len())
    .collect::<Vec<_>>();

    assert_eq!(traversed, CoverageField::ALL);
    assert_eq!(
        CoverageField::PayerName.previous_wrapping(),
        CoverageField::CoverageNotes
    );
    assert_eq!(
        CoverageField::CoverageNotes.next_wrapping(),
        CoverageField::PayerName
    );
}

#[test]
fn stored_coverage_round_trips_as_an_upsert_without_server_owned_fields() {
    let coverage = stored_coverage();

    let draft = CoverageDraft::try_from(&coverage).expect("valid coverage priority");
    let upsert = WorkspaceCoverageUpsertParams::from(&draft);

    assert_eq!(
        upsert,
        WorkspaceCoverageUpsertParams {
            id: Some("coverage-2".to_string()),
            client_id: "patient-1".to_string(),
            priority: 2,
            payer_name: Some("Example Health".to_string()),
            plan_name: Some("Community PPO".to_string()),
            member_id: Some("ABC123456".to_string()),
            group_number: Some("GROUP-7".to_string()),
            coverage_type: Some("commercial".to_string()),
            coverage_status: Some("active".to_string()),
            effective_date: Some("2026-01-01".to_string()),
            termination_date: None,
            patient_relationship_to_subscriber: Some("child".to_string()),
            subscriber_first_name: Some("Alex".to_string()),
            subscriber_middle_name: Some("Q".to_string()),
            subscriber_last_name: Some("Example".to_string()),
            subscriber_suffix: Some("Jr".to_string()),
            subscriber_date_of_birth: Some("1982-03-04".to_string()),
            subscriber_administrative_sex: Some("male".to_string()),
            subscriber_address_same_as_patient: false,
            subscriber_address_line_1: Some("100 Main Street".to_string()),
            subscriber_address_line_2: Some("Unit 2".to_string()),
            subscriber_city: Some("Madison".to_string()),
            subscriber_state_or_province: Some("WI".to_string()),
            subscriber_postal_code: Some("53703".to_string()),
            subscriber_country: Some("US".to_string()),
            coverage_notes: Some("Confirm coordination of benefits.".to_string()),
        }
    );
    assert_eq!(draft.version.as_deref(), Some("coverage-version-7"));
    assert_eq!(draft.billing_readiness, WorkspaceBillingReadiness::Match);

    let restored = CoverageDraft::try_from(&upsert).expect("valid upsert priority");
    assert_eq!(WorkspaceCoverageUpsertParams::from(restored), upsert);
}

#[test]
fn upsert_conversion_trims_edges_and_uses_none_for_blank_optional_fields() {
    let mut draft = CoverageDraft::new(" patient-1 ", CoveragePriority::Tertiary);
    draft.payer_name = " Example Health ".to_string();
    draft.plan_name = "   ".to_string();
    draft.member_id = " M-123 ".to_string();
    draft.subscriber_address_same_as_patient = true;

    let upsert = WorkspaceCoverageUpsertParams::from(draft);

    assert_eq!(upsert.client_id, "patient-1");
    assert_eq!(upsert.priority, 3);
    assert_eq!(upsert.payer_name.as_deref(), Some("Example Health"));
    assert_eq!(upsert.plan_name, None);
    assert_eq!(upsert.member_id.as_deref(), Some("M-123"));
    assert!(upsert.subscriber_address_same_as_patient);
}

#[test]
fn same_as_patient_toggle_does_not_destroy_a_manually_entered_address() {
    let mut draft = CoverageDraft::new("patient-1", CoveragePriority::Primary);
    draft.subscriber_address_line_1 = "200 Oak Avenue".to_string();

    draft.toggle_subscriber_address_same_as_patient();
    draft.toggle_subscriber_address_same_as_patient();

    assert!(!draft.subscriber_address_same_as_patient);
    assert_eq!(draft.subscriber_address_line_1, "200 Oak Avenue");
}

#[test]
fn card_verification_conversion_is_explicit_human_entered_data() {
    let mut draft = CardVerificationDraft::new(
        " coverage-2 ",
        WorkspaceCoverageVerificationSubject::Subscriber,
    );
    draft.source_document_id = " card-document-8 ".to_string();
    draft.expected_patient_version = " patient-version-3 ".to_string();
    draft.expected_coverage_version = " coverage-version-4 ".to_string();
    draft.expected_document_version = " document-version-5 ".to_string();
    draft.observed_first_name = " Alex ".to_string();
    draft.observed_middle_name = " ".to_string();
    draft.observed_last_name = " Example ".to_string();
    draft.observed_suffix = " Jr ".to_string();
    draft.observed_member_id = " ABC123456 ".to_string();
    draft.actor = " clinician@example.test ".to_string();

    assert_eq!(draft.submission_issue(), None);
    assert_eq!(
        WorkspaceCoverageVerificationCreateParams::from(draft),
        WorkspaceCoverageVerificationCreateParams {
            coverage_id: "coverage-2".to_string(),
            source_document_id: "card-document-8".to_string(),
            expected_patient_version: "patient-version-3".to_string(),
            expected_coverage_version: "coverage-version-4".to_string(),
            expected_document_version: "document-version-5".to_string(),
            compared_subject: WorkspaceCoverageVerificationSubject::Subscriber,
            observed_first_name: Some("Alex".to_string()),
            observed_middle_name: None,
            observed_last_name: Some("Example".to_string()),
            observed_suffix: Some("Jr".to_string()),
            observed_member_id: Some("ABC123456".to_string()),
            actor: "clinician@example.test".to_string(),
        }
    );
}

#[test]
fn card_verification_requires_a_saved_coverage_source_and_printed_identity() {
    let mut draft =
        CardVerificationDraft::new("", WorkspaceCoverageVerificationSubject::Beneficiary);
    assert_eq!(
        draft.submission_issue(),
        Some("Save the coverage before recording a card verification.")
    );

    draft.coverage_id = "coverage-1".to_string();
    assert_eq!(
        draft.submission_issue(),
        Some("Select the source card document.")
    );

    draft.source_document_id = "card-document-1".to_string();
    assert_eq!(
        draft.submission_issue(),
        Some("Reload the patient, coverage, and source card before verification.")
    );
    draft.expected_patient_version = "patient-version-1".to_string();
    draft.expected_coverage_version = "coverage-version-1".to_string();
    draft.expected_document_version = "document-version-1".to_string();
    assert_eq!(
        draft.submission_issue(),
        Some("Enter the first and last name exactly as printed on the card.")
    );
}

#[test]
fn summaries_mask_member_identity_and_keep_clinical_save_semantics_clear() {
    let draft = CoverageDraft::try_from(stored_coverage()).expect("valid coverage priority");

    assert_eq!(
        draft.concise_summary(),
        "Secondary · Example Health / Community PPO · member ending 3456 · MATCH · billing ready"
    );
    assert_eq!(
        billing_readiness_summary(WorkspaceBillingReadiness::Mismatch),
        "Printed card identity does not match; clinical chart saves remain available."
    );
    assert!(billing_export_is_blocked(
        WorkspaceBillingReadiness::Mismatch
    ));
    assert!(!billing_export_is_blocked(WorkspaceBillingReadiness::Match));
}
