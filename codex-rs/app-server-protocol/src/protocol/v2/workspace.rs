use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClient {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub legal_suffix: Option<String>,
    pub preferred_name: Option<String>,
    pub previous_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub sex_or_gender: Option<String>,
    pub administrative_sex: Option<String>,
    pub preferred_language: Option<String>,
    #[serde(default)]
    pub interpreter_required: bool,
    pub external_id: Option<String>,
    pub record_start_date: Option<String>,
    pub record_end_date: Option<String>,
    pub summary: String,
    pub primary_phone: Option<String>,
    pub primary_phone_use: Option<String>,
    pub secondary_phone: Option<String>,
    pub secondary_phone_use: Option<String>,
    pub email: Option<String>,
    pub primary_email: Option<String>,
    pub secondary_email: Option<String>,
    pub preferred_contact_method: Option<String>,
    pub address_line_1: Option<String>,
    pub address_line_2: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub address_use: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientListParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientListResponse {
    pub clients: Vec<WorkspaceClient>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientGetParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientGetResponse {
    pub client: Option<WorkspaceClient>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientUpsertParams {
    #[ts(optional = nullable)]
    pub id: Option<String>,
    #[ts(optional = nullable)]
    pub expected_version: Option<String>,
    pub display_name: String,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub legal_first_name: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub legal_middle_name: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub legal_last_name: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub legal_suffix: Option<Option<String>>,
    pub preferred_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub previous_name: Option<Option<String>>,
    pub date_of_birth: Option<String>,
    pub sex_or_gender: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub administrative_sex: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub preferred_language: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "boolean | null", optional = nullable)]
    pub interpreter_required: Option<Option<bool>>,
    pub external_id: Option<String>,
    pub record_start_date: Option<String>,
    pub record_end_date: Option<String>,
    pub summary: String,
    pub primary_phone: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub primary_phone_use: Option<Option<String>>,
    pub secondary_phone: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub secondary_phone_use: Option<Option<String>>,
    pub email: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub primary_email: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub secondary_email: Option<Option<String>>,
    pub preferred_contact_method: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub address_line_1: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub address_line_2: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub city: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub state_or_province: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub postal_code: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub country: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional = nullable)]
    pub address_use: Option<Option<String>>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientUpsertResponse {
    pub client: WorkspaceClient,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientArchiveParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceClientArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocument {
    pub id: String,
    pub version: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub local_path: String,
    pub notes: String,
    pub scope: String,
    pub detected_kind: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    #[ts(type = "number | null")]
    pub modified_at: Option<i64>,
    pub sha256: Option<String>,
    pub tags: String,
    pub source_label: String,
    pub existence_status: String,
    pub metadata_json: String,
    pub original_path: String,
    pub reference_kind: String,
    pub vault_path: String,
    pub content_sha256: Option<String>,
    pub thumbnail_path: String,
    pub thumbnail_status: String,
    pub thumbnail_mime_type: Option<String>,
    pub intake_source: String,
    #[ts(type = "number | null")]
    pub imported_at: Option<i64>,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentListParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentListResponse {
    pub documents: Vec<WorkspaceDocument>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePracticeLibraryListParams {
    #[ts(optional = nullable)]
    pub active_client_id: Option<String>,
    #[ts(optional = nullable)]
    pub query: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePracticeLibraryItem {
    pub document: WorkspaceDocument,
    pub owner_client_id: String,
    pub owner_display_name: String,
    pub linked_to_active_client: bool,
    pub linked_document_id: Option<String>,
    pub scope_reason: String,
    pub reviewed_text_count: i64,
    pub clip_count: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePracticeLibraryListResponse {
    pub items: Vec<WorkspacePracticeLibraryItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentGetParams {
    pub document_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentGetResponse {
    pub document: Option<WorkspaceDocument>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentUpsertParams {
    pub id: Option<String>,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub local_path: String,
    pub notes: String,
    pub scope: String,
    pub detected_kind: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    #[ts(optional = nullable)]
    pub modified_at: Option<i64>,
    pub sha256: Option<String>,
    pub tags: String,
    pub source_label: String,
    pub existence_status: String,
    pub metadata_json: String,
    #[serde(default)]
    pub original_path: String,
    #[serde(default)]
    pub reference_kind: String,
    #[serde(default)]
    pub vault_path: String,
    #[serde(default)]
    pub content_sha256: Option<String>,
    #[serde(default)]
    pub thumbnail_path: String,
    #[serde(default)]
    pub thumbnail_status: String,
    #[serde(default)]
    pub thumbnail_mime_type: Option<String>,
    #[serde(default)]
    pub intake_source: String,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub imported_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentUpsertResponse {
    pub document: WorkspaceDocument,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentArchiveParams {
    pub document_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDocumentArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItem {
    pub id: String,
    pub version: String,
    pub client_id: String,
    pub category: String,
    pub name: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub dose: Option<String>,
    pub route: Option<String>,
    pub frequency: Option<String>,
    pub status: Option<String>,
    pub recorded_date: Option<String>,
    pub notes: String,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemListParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemListResponse {
    pub items: Vec<WorkspacePatientSafetyItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemUpsertParams {
    pub id: Option<String>,
    pub client_id: String,
    pub category: String,
    pub name: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub dose: Option<String>,
    pub route: Option<String>,
    pub frequency: Option<String>,
    pub status: Option<String>,
    pub recorded_date: Option<String>,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemUpsertResponse {
    pub item: WorkspacePatientSafetyItem,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemArchiveParams {
    pub item_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePatientSafetyItemArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivative {
    pub id: String,
    pub version: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub document_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeListResponse {
    pub derivatives: Vec<WorkspaceArtifactDerivative>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeUpsertParams {
    pub id: Option<String>,
    pub document_id: String,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeUpsertResponse {
    pub derivative: WorkspaceArtifactDerivative,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeStatusUpdateParams {
    pub derivative_id: String,
    pub review_status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceArtifactDerivativeStatusUpdateResponse {
    pub derivative: Option<WorkspaceArtifactDerivative>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClip {
    pub id: String,
    pub version: String,
    pub derivative_id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub line_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub derivative_id: Option<String>,
    #[ts(optional = nullable)]
    pub document_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipListResponse {
    pub clips: Vec<WorkspaceContextClip>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipUpsertParams {
    pub id: Option<String>,
    pub derivative_id: String,
    pub document_id: String,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub line_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipUpsertResponse {
    pub clip: WorkspaceContextClip,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipStatusUpdateParams {
    pub clip_id: String,
    pub review_status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextClipStatusUpdateResponse {
    pub clip: Option<WorkspaceContextClip>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceTaskStatus {
    Open,
    InProgress,
    Blocked,
    Done,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceTaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTask {
    pub id: String,
    pub version: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub details: String,
    pub kind: String,
    pub status: WorkspaceTaskStatus,
    pub priority: WorkspaceTaskPriority,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskSummary {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub status: WorkspaceTaskStatus,
    pub priority: WorkspaceTaskPriority,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskListParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskListResponse {
    pub tasks: Vec<WorkspaceTask>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskUpsertParams {
    #[ts(optional = nullable)]
    pub id: Option<String>,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub document_id: Option<String>,
    pub title: String,
    pub details: String,
    pub kind: String,
    pub status: WorkspaceTaskStatus,
    pub priority: WorkspaceTaskPriority,
    #[ts(optional = nullable)]
    pub due_date: Option<String>,
    #[ts(optional = nullable)]
    pub assigned_to: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskUpsertResponse {
    pub task: WorkspaceTask,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskStatusUpdateParams {
    pub client_id: String,
    pub task_id: String,
    pub status: WorkspaceTaskStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceTaskStatusUpdateResponse {
    pub task: Option<WorkspaceTask>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceEncounter {
    pub id: String,
    pub version: String,
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    #[ts(type = "number | null")]
    pub started_at: Option<i64>,
    #[ts(type = "number | null")]
    pub ended_at: Option<i64>,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceEncounterListParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceEncounterListResponse {
    pub encounters: Vec<WorkspaceEncounter>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceEncounterUpsertParams {
    #[ts(optional = nullable)]
    pub id: Option<String>,
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    #[ts(type = "number | null", optional = nullable)]
    pub started_at: Option<i64>,
    #[ts(type = "number | null", optional = nullable)]
    pub ended_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceEncounterUpsertResponse {
    pub encounter: WorkspaceEncounter,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNote {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    #[ts(type = "number")]
    pub current_revision: i64,
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSummary {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub status: String,
    #[ts(type = "number")]
    pub current_revision: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteListParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteListResponse {
    pub notes: Vec<WorkspaceNote>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteGetParams {
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteGetResponse {
    pub note: Option<WorkspaceNote>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteUpsertParams {
    pub id: Option<String>,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteUpsertResponse {
    pub note: WorkspaceNote,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteArchiveParams {
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteArchiveResponse {
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSignature {
    pub id: String,
    pub note_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    pub signer: String,
    pub body_sha256: String,
    #[ts(type = "number")]
    pub signed_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSignParams {
    pub note_id: String,
    pub signer: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSignResponse {
    pub signature: WorkspaceNoteSignature,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSignatureListParams {
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteSignatureListResponse {
    pub signatures: Vec<WorkspaceNoteSignature>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteAddendum {
    pub id: String,
    pub note_id: String,
    #[ts(type = "number")]
    pub base_revision: i64,
    pub body: String,
    pub author: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteAddendumCreateParams {
    pub note_id: String,
    #[ts(type = "number")]
    pub base_revision: i64,
    pub body: String,
    pub author: String,
    #[ts(optional = nullable)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_turn_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteAddendumCreateResponse {
    pub addendum: WorkspaceNoteAddendum,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteAddendumListParams {
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteAddendumListResponse {
    pub addenda: Vec<WorkspaceNoteAddendum>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContext {
    pub client: WorkspaceClient,
    pub active_note: Option<WorkspaceNote>,
    pub recent_notes: Vec<WorkspaceNoteSummary>,
    pub documents: Vec<WorkspaceDocument>,
    pub tasks: Vec<WorkspaceTaskSummary>,
}

/// Broad human/dashboard workspace context. This is not an agent-visible context contract.
/// Agent-visible medical context must be scoped to an explicit context packet replay.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextGetParams {
    pub client_id: String,
    pub note_id: Option<String>,
    pub include_documents: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextGetResponse {
    pub context: Option<WorkspaceContext>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacket {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub human_request: String,
    pub selected_artifact_ids_json: String,
    pub selected_derivative_ids_json: String,
    pub selected_clip_ids_json: String,
    pub artifact_summary: String,
    pub derivative_summary: String,
    pub clip_summary: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    pub context_envelope_sha256: String,
    pub clinician_actor: String,
    #[ts(type = "number | null")]
    pub base_note_revision: Option<i64>,
    pub authorized_scope_json: String,
    pub expected_output_kind: String,
    pub status: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub sent_at: i64,
    #[ts(type = "number | null")]
    pub submitted_at: Option<i64>,
    #[ts(type = "number | null")]
    pub canceled_at: Option<i64>,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketListResponse {
    pub packets: Vec<WorkspaceContextPacket>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketCreateParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    pub human_request: String,
    pub selected_artifact_ids_json: String,
    pub selected_derivative_ids_json: String,
    pub selected_clip_ids_json: String,
    pub artifact_summary: String,
    pub derivative_summary: String,
    pub clip_summary: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    #[ts(optional = nullable)]
    pub clinician_actor: Option<String>,
    #[ts(optional = nullable)]
    pub base_note_revision: Option<i64>,
    #[ts(optional = nullable)]
    pub authorized_scope_json: Option<String>,
    #[ts(optional = nullable)]
    pub expected_output_kind: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketCreateResponse {
    pub packet: WorkspaceContextPacket,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketReplayParams {
    pub client_id: String,
    pub packet_id: String,
    #[ts(optional = nullable)]
    pub context_envelope_sha256: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketReplay {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub human_request: String,
    pub context_envelope_json: String,
    pub context_envelope_sha256: String,
    pub clinician_actor: String,
    #[ts(type = "number | null")]
    pub base_note_revision: Option<i64>,
    pub authorized_scope_json: String,
    pub expected_output_kind: String,
    pub read_only_safety_constraints: Vec<String>,
    pub status: String,
    #[ts(type = "number")]
    pub sent_at: i64,
    #[ts(type = "number | null")]
    pub submitted_at: Option<i64>,
}

/// One execution attempt against an immutable context packet. Retries create a
/// new run; repeated starts with the same packet/idempotency key return the
/// existing run.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRun {
    pub id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    #[ts(type = "number | null")]
    pub base_note_revision: Option<i64>,
    pub context_envelope_sha256: String,
    pub run_kind: String,
    pub idempotency_key: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub status: String,
    pub error_summary: Option<String>,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunStartParams {
    pub packet_id: String,
    pub idempotency_key: String,
    #[ts(optional = nullable)]
    pub client_id: Option<String>,
    #[ts(optional = nullable)]
    pub context_envelope_sha256: Option<String>,
    #[ts(optional = nullable)]
    pub provider: Option<String>,
    #[ts(optional = nullable)]
    pub model: Option<String>,
    #[ts(optional = nullable)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_turn_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunStartResponse {
    pub run: WorkspaceAgentRun,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub packet_id: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunListResponse {
    pub runs: Vec<WorkspaceAgentRun>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunStatusUpdateParams {
    pub run_id: String,
    pub status: String,
    #[ts(optional = nullable)]
    pub error_summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunStatusUpdateResponse {
    pub run: Option<WorkspaceAgentRun>,
}

/// Exact, immutable record returned to one agent run. The snapshot omits local
/// filesystem paths and is hashed server-side for later audit replay.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunSource {
    pub id: String,
    pub run_id: String,
    pub source_entity_type: String,
    pub source_entity_id: String,
    #[ts(type = "number | null")]
    pub source_revision: Option<i64>,
    pub display_label: String,
    pub snapshot_json: String,
    pub content_sha256: String,
    pub access_purpose: String,
    #[ts(type = "number")]
    pub accessed_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunSourceListParams {
    pub run_id: String,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunSourceListResponse {
    pub sources: Vec<WorkspaceAgentRunSource>,
}

/// Packet-authorized database categories that an active agent run may read.
/// The server derives patient ownership from the run and records every returned
/// record as an immutable source snapshot.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceAgentContextCategory {
    VisitHistory,
    ProgressNotes,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunContextReadParams {
    pub run_id: String,
    pub category: WorkspaceAgentContextCategory,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentRunContextReadResponse {
    pub category: WorkspaceAgentContextCategory,
    pub sources: Vec<WorkspaceAgentRunSource>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceContextPacketReplayResponse {
    pub replay: Option<WorkspaceContextPacketReplay>,
}

/// Agent results are review-pending outputs bound to one context packet. The
/// packet id/hash are provenance; clients must not treat result payloads as
/// write, signing, submission, payer-contact, or workspace-wide read authority.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResult {
    pub id: String,
    pub run_id: Option<String>,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub context_envelope_sha256: String,
    #[ts(type = "number | null")]
    pub base_note_revision: Option<i64>,
    pub packet_context_sha256: String,
    pub result_kind: String,
    pub structured_changes_json: String,
    pub rationale_summary: String,
    pub body: String,
    pub summary: String,
    pub status: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub packet_id: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultListResponse {
    pub results: Vec<WorkspaceAgentResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultCreateParams {
    pub packet_id: String,
    #[ts(optional = nullable)]
    pub run_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_turn_id: Option<String>,
    pub body: String,
    #[ts(optional = nullable)]
    pub summary: Option<String>,
    #[ts(optional = nullable)]
    pub client_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub context_envelope_sha256: Option<String>,
    #[ts(optional = nullable)]
    pub result_kind: Option<String>,
    #[ts(optional = nullable)]
    pub structured_changes_json: Option<String>,
    #[ts(optional = nullable)]
    pub rationale_summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultCreateResponse {
    pub result: WorkspaceAgentResult,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultStatusUpdateParams {
    pub result_id: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAgentResultStatusUpdateResponse {
    pub result: Option<WorkspaceAgentResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceNoteProposalStatus {
    Pending,
    Accepted,
    Declined,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposal {
    pub id: String,
    pub note_id: String,
    #[ts(type = "number")]
    pub base_revision: i64,
    pub proposed_body: String,
    pub summary: String,
    pub status: WorkspaceNoteProposalStatus,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub agent_result_id: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub resolved_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalListParams {
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalListResponse {
    pub proposals: Vec<WorkspaceNoteProposal>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalCreateParams {
    pub note_id: String,
    #[ts(type = "number")]
    pub base_revision: i64,
    pub proposed_body: String,
    pub summary: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    #[ts(optional = nullable)]
    pub agent_result_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalCreateResponse {
    pub proposal: WorkspaceNoteProposal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalResolveParams {
    pub proposal_id: String,
    pub accept: bool,
    #[ts(optional = nullable)]
    pub edited_body: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalResolveResponse {
    pub proposal: Option<WorkspaceNoteProposal>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceNoteProposalDecisionKind {
    AcceptedAll,
    AcceptedEdited,
    RejectedAll,
    CopiedChange,
    RejectedChange,
}

/// Append-only record of what a clinician did with proposed agent work.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalDecision {
    pub id: String,
    pub proposal_id: String,
    pub agent_result_id: Option<String>,
    pub note_id: String,
    #[ts(type = "number")]
    pub base_revision: i64,
    pub decision_kind: WorkspaceNoteProposalDecisionKind,
    pub change_id: Option<String>,
    pub applied_text: Option<String>,
    pub applied_text_sha256: Option<String>,
    #[ts(type = "number | null")]
    pub resulting_note_revision: Option<i64>,
    pub actor: String,
    pub reason: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalDecisionListParams {
    pub proposal_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceNoteProposalDecisionListResponse {
    pub decisions: Vec<WorkspaceNoteProposalDecision>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAuditEvent {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub actor: String,
    pub actor_kind: String,
    pub source: String,
    pub client_id: Option<String>,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub success: bool,
    pub summary: String,
    pub metadata_json: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAuditListParams {
    #[ts(optional = nullable)]
    pub entity_type: Option<String>,
    #[ts(optional = nullable)]
    pub entity_id: Option<String>,
    #[ts(optional = nullable)]
    pub client_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceAuditListResponse {
    pub data: Vec<WorkspaceAuditEvent>,
    pub next_cursor: Option<String>,
}
