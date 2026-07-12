use crate::WorkspaceCoverage;
use crate::WorkspaceCoverageVerificationCreate;
use crate::WorkspaceCoverageVerificationSubject;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;

pub(super) fn compare_card_identity(
    client: &crate::WorkspaceClient,
    coverage: &WorkspaceCoverage,
    input: &WorkspaceCoverageVerificationCreate,
    medicare: bool,
) -> Vec<String> {
    let (first, middle, last, suffix) = match input.compared_subject {
        WorkspaceCoverageVerificationSubject::Beneficiary => (
            &client.legal_first_name,
            &client.legal_middle_name,
            &client.legal_last_name,
            &client.legal_suffix,
        ),
        WorkspaceCoverageVerificationSubject::Subscriber => (
            &coverage.subscriber_first_name,
            &coverage.subscriber_middle_name,
            &coverage.subscriber_last_name,
            &coverage.subscriber_suffix,
        ),
    };
    let mut mismatches = Vec::new();
    push_mismatch(&mut mismatches, "firstName", first, &input.observed_first_name);
    push_mismatch(&mut mismatches, "middleName", middle, &input.observed_middle_name);
    push_mismatch(&mut mismatches, "lastName", last, &input.observed_last_name);
    push_mismatch(&mut mismatches, "suffix", suffix, &input.observed_suffix);
    let expected_member = if medicare {
        coverage.member_id.as_deref().map(normalize_mbi)
    } else {
        coverage.member_id.clone()
    };
    push_mismatch(
        &mut mismatches,
        "memberId",
        &expected_member,
        &input.observed_member_id,
    );
    mismatches
}

pub(super) fn coverage_incomplete(
    client: &crate::WorkspaceClient,
    coverage: &WorkspaceCoverage,
) -> bool {
    let required = [
        coverage.payer_name.as_deref(),
        coverage.member_id.as_deref(),
        coverage.coverage_status.as_deref(),
        client.legal_first_name.as_deref(),
        client.legal_last_name.as_deref(),
        client.date_of_birth.as_deref(),
        client.administrative_sex.as_deref(),
        client.primary_phone.as_deref(),
        client.address_line_1.as_deref(),
        client.city.as_deref(),
        client.state_or_province.as_deref(),
        client.postal_code.as_deref(),
        client.country.as_deref(),
    ];
    if required.iter().any(|value| value.is_none_or(str::is_empty)) {
        return true;
    }
    if coverage
        .coverage_status
        .as_deref()
        .is_none_or(|status| !status.eq_ignore_ascii_case("active"))
    {
        return true;
    }
    if coverage_is_medicare(coverage)
        && coverage
            .member_id
            .as_deref()
            .map(normalize_mbi)
            .is_none_or(|id| !valid_mbi(&id))
    {
        return true;
    }
    if !coverage_is_medicare(coverage)
        && coverage.patient_relationship_to_subscriber.is_none()
    {
        return true;
    }
    let dependent_incomplete = coverage
        .patient_relationship_to_subscriber
        .as_deref()
        .is_some_and(|relationship| {
            !relationship.eq_ignore_ascii_case("self")
                && [
                    coverage.subscriber_first_name.as_deref(),
                    coverage.subscriber_last_name.as_deref(),
                    coverage.subscriber_date_of_birth.as_deref(),
                    coverage.subscriber_administrative_sex.as_deref(),
                ]
                .iter()
                .any(|value| value.is_none_or(str::is_empty))
        });
    dependent_incomplete
        || (!coverage.subscriber_address_same_as_patient
            && [
                coverage.subscriber_address_line_1.as_deref(),
                coverage.subscriber_city.as_deref(),
                coverage.subscriber_state_or_province.as_deref(),
                coverage.subscriber_postal_code.as_deref(),
                coverage.subscriber_country.as_deref(),
            ]
            .iter()
            .any(|value| value.is_none_or(str::is_empty)))
}

pub(super) fn coverage_is_medicare(coverage: &WorkspaceCoverage) -> bool {
    [
        coverage.payer_name.as_deref(),
        coverage.coverage_type.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.to_ascii_lowercase().contains("medicare"))
}

pub(super) fn patient_identity_version(
    client: &crate::WorkspaceClient,
) -> anyhow::Result<String> {
    let identity = serde_json::to_vec(&json!({
        "legalFirstName": &client.legal_first_name,
        "legalMiddleName": &client.legal_middle_name,
        "legalLastName": &client.legal_last_name,
        "legalSuffix": &client.legal_suffix,
        "dateOfBirth": &client.date_of_birth,
        "administrativeSex": &client.administrative_sex,
    }))?;
    let mut hasher = Sha256::new();
    hasher.update(b"workspace-patient-billing-identity:v1\0");
    hasher.update(identity);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn valid_mbi(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 11 || !matches!(bytes[0], b'1'..=b'9') {
        return false;
    }
    let alphabetic = |byte: u8| b"ACDEFGHJKMNPQRTUVWXY".contains(&byte);
    let alphanumeric = |byte: u8| byte.is_ascii_digit() || alphabetic(byte);
    let alphabetic_positions = [1, 4, 7, 8];
    let alphanumeric_positions = [2, 5];
    let numeric = [3, 6, 9, 10];
    alphabetic_positions
        .into_iter()
        .all(|index| alphabetic(bytes[index]))
        && alphanumeric_positions
            .into_iter()
            .all(|index| alphanumeric(bytes[index]))
        && numeric
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
}

pub(super) fn normalize_mbi(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .chars()
        .filter(|character| !matches!(character, ' ' | '-'))
        .flat_map(char::to_uppercase)
        .collect()
}

fn push_mismatch(
    mismatches: &mut Vec<String>,
    field: &str,
    expected: &Option<String>,
    observed: &Option<String>,
) {
    if expected.as_deref().map(normalize_comparison)
        != observed.as_deref().map(normalize_comparison)
    {
        mismatches.push(field.to_string());
    }
}

fn normalize_comparison(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}
