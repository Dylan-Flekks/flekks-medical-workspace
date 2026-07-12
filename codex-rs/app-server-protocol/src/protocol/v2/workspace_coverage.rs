use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceBillingReadiness {
    Match,
    Mismatch,
    Unverified,
    Stale,
    Incomplete,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceCoverageVerificationSubject {
    Beneficiary,
    Subscriber,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceCoverageMatchResult {
    Match,
    Mismatch,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverage {
    pub id: String,
    pub version: String,
    pub client_id: String,
    #[ts(type = "number")]
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
    pub billing_readiness: WorkspaceBillingReadiness,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageUpsertParams {
    #[ts(optional = nullable)]
    pub id: Option<String>,
    pub client_id: String,
    #[ts(type = "number")]
    pub priority: i64,
    #[ts(optional = nullable)]
    pub payer_name: Option<String>,
    #[ts(optional = nullable)]
    pub plan_name: Option<String>,
    #[ts(optional = nullable)]
    pub member_id: Option<String>,
    #[ts(optional = nullable)]
    pub group_number: Option<String>,
    #[ts(optional = nullable)]
    pub coverage_type: Option<String>,
    #[ts(optional = nullable)]
    pub coverage_status: Option<String>,
    #[ts(optional = nullable)]
    pub effective_date: Option<String>,
    #[ts(optional = nullable)]
    pub termination_date: Option<String>,
    #[ts(optional = nullable)]
    pub patient_relationship_to_subscriber: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_first_name: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_middle_name: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_last_name: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_suffix: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_date_of_birth: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_administrative_sex: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subscriber_address_same_as_patient: bool,
    #[ts(optional = nullable)]
    pub subscriber_address_line_1: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_address_line_2: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_city: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_state_or_province: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_postal_code: Option<String>,
    #[ts(optional = nullable)]
    pub subscriber_country: Option<String>,
    #[ts(optional = nullable)]
    pub coverage_notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageListResponse {
    pub data: Vec<WorkspaceCoverage>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
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
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageVerificationListParams {
    pub coverage_id: String,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageVerificationListResponse {
    pub data: Vec<WorkspaceCoverageVerification>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageVerificationCreateParams {
    pub coverage_id: String,
    pub source_document_id: String,
    pub expected_patient_version: String,
    pub expected_coverage_version: String,
    pub expected_document_version: String,
    pub compared_subject: WorkspaceCoverageVerificationSubject,
    #[ts(optional = nullable)]
    pub observed_first_name: Option<String>,
    #[ts(optional = nullable)]
    pub observed_middle_name: Option<String>,
    #[ts(optional = nullable)]
    pub observed_last_name: Option<String>,
    #[ts(optional = nullable)]
    pub observed_suffix: Option<String>,
    #[ts(optional = nullable)]
    pub observed_member_id: Option<String>,
    pub actor: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceCoverageVerificationCreateResponse {
    pub verification: WorkspaceCoverageVerification,
    pub billing_readiness: WorkspaceBillingReadiness,
}
