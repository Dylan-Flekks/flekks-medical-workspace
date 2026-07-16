use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCoverage {
    pub id: String,
    pub client_id: String,
    pub priority: i64,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub effective_date: Option<String>,
    pub termination_date: Option<String>,
    pub patient_relationship_to_subscriber: Option<String>,
    pub subscriber_first_name: Option<String>,
    pub subscriber_middle_name: Option<String>,
    pub subscriber_last_name: Option<String>,
    pub subscriber_suffix: Option<String>,
    pub subscriber_date_of_birth: Option<String>,
    pub subscriber_administrative_sex: Option<String>,
    pub subscriber_address_same_as_patient: bool,
    pub subscriber_address_line_1: Option<String>,
    pub subscriber_address_line_2: Option<String>,
    pub subscriber_city: Option<String>,
    pub subscriber_state_or_province: Option<String>,
    pub subscriber_postal_code: Option<String>,
    pub subscriber_country: Option<String>,
    pub coverage_notes: Option<String>,
    pub(crate) source_kind: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCoverageUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub priority: i64,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub effective_date: Option<String>,
    pub termination_date: Option<String>,
    pub patient_relationship_to_subscriber: Option<String>,
    pub subscriber_first_name: Option<String>,
    pub subscriber_middle_name: Option<String>,
    pub subscriber_last_name: Option<String>,
    pub subscriber_suffix: Option<String>,
    pub subscriber_date_of_birth: Option<String>,
    pub subscriber_administrative_sex: Option<String>,
    pub subscriber_address_same_as_patient: bool,
    pub subscriber_address_line_1: Option<String>,
    pub subscriber_address_line_2: Option<String>,
    pub subscriber_city: Option<String>,
    pub subscriber_state_or_province: Option<String>,
    pub subscriber_postal_code: Option<String>,
    pub subscriber_country: Option<String>,
    pub coverage_notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCoverageVerificationSubject {
    Beneficiary,
    Subscriber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCoverageMatchResult {
    Match,
    Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceBillingReadiness {
    Match,
    Mismatch,
    Unverified,
    Stale,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCoverageVerificationCreate {
    pub coverage_id: String,
    pub source_document_id: String,
    pub expected_patient_version: String,
    pub expected_coverage_version: String,
    pub expected_document_version: String,
    pub compared_subject: WorkspaceCoverageVerificationSubject,
    pub observed_first_name: Option<String>,
    pub observed_middle_name: Option<String>,
    pub observed_last_name: Option<String>,
    pub observed_suffix: Option<String>,
    pub observed_member_id: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCoverageVerification {
    pub id: String,
    pub coverage_id: String,
    pub client_id: String,
    pub source_document_id: String,
    pub source_document_version: String,
    pub source_document_content_sha256: String,
    pub compared_subject: WorkspaceCoverageVerificationSubject,
    pub observed_first_name: Option<String>,
    pub observed_middle_name: Option<String>,
    pub observed_last_name: Option<String>,
    pub observed_suffix: Option<String>,
    pub observed_member_id: Option<String>,
    pub patient_record_version: String,
    pub patient_version: String,
    pub coverage_version: String,
    pub match_result: WorkspaceCoverageMatchResult,
    pub mismatch_fields: Vec<String>,
    pub actor: String,
    pub content_sha256: String,
    pub is_stale: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCoverageVerificationCreateResult {
    pub verification: WorkspaceCoverageVerification,
    pub billing_readiness: WorkspaceBillingReadiness,
}
