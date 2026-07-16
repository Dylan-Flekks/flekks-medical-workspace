use chrono::DateTime;
use chrono::Utc;

use super::workspace_chart_commit::ExistingRecords;

pub(super) fn preserve_existing_timestamp_precision(
    request: &mut crate::WorkspaceChartCommitRequest,
    existing: &ExistingRecords,
) {
    if let (Some(input), Some(existing)) = (request.encounter.as_mut(), existing.encounter.as_ref())
    {
        preserve_timestamp(&mut input.started_at, &existing.started_at);
        preserve_timestamp(&mut input.ended_at, &existing.ended_at);
    }
    if let (Some(input), Some(existing)) = (request.document.as_mut(), existing.document.as_ref()) {
        preserve_timestamp(&mut input.modified_at, &existing.modified_at);
        preserve_timestamp(&mut input.imported_at, &existing.imported_at);
    }
}

fn preserve_timestamp(submitted: &mut Option<DateTime<Utc>>, existing: &Option<DateTime<Utc>>) {
    if submitted.as_ref().map(DateTime::timestamp) == existing.as_ref().map(DateTime::timestamp) {
        submitted.clone_from(existing);
    }
}

pub(super) fn client(
    existing: &crate::WorkspaceClient,
    input: &crate::WorkspaceClientUpsert,
) -> bool {
    existing.display_name == input.display_name
        && existing.legal_first_name == input.legal_first_name
        && existing.legal_middle_name == input.legal_middle_name
        && existing.legal_last_name == input.legal_last_name
        && existing.legal_suffix == input.legal_suffix
        && existing.preferred_name == input.preferred_name
        && existing.previous_name == input.previous_name
        && existing.date_of_birth == input.date_of_birth
        && existing.sex_or_gender == input.sex_or_gender
        && existing.administrative_sex == input.administrative_sex
        && existing.preferred_language == input.preferred_language
        && existing.interpreter_required == input.interpreter_required
        && existing.external_id == input.external_id
        && existing.record_start_date == input.record_start_date
        && existing.record_end_date == input.record_end_date
        && existing.summary == input.summary
        && existing.primary_phone == input.primary_phone
        && existing.primary_phone_use == input.primary_phone_use
        && existing.secondary_phone == input.secondary_phone
        && existing.secondary_phone_use == input.secondary_phone_use
        && existing.primary_email == input.primary_email
        && existing.secondary_email == input.secondary_email
        && existing.preferred_contact_method == input.preferred_contact_method
        && existing.address_line_1 == input.address_line_1
        && existing.address_line_2 == input.address_line_2
        && existing.city == input.city
        && existing.state_or_province == input.state_or_province
        && existing.postal_code == input.postal_code
        && existing.country == input.country
        && existing.address_use == input.address_use
        && existing.emergency_contact_name == input.emergency_contact_name
        && existing.emergency_contact_relationship == input.emergency_contact_relationship
        && existing.emergency_contact_phone == input.emergency_contact_phone
        && existing.emergency_contact_email == input.emergency_contact_email
        && existing.contact_notes == input.contact_notes
        && existing.archived_at.is_none()
}

pub(super) fn coverage(
    existing: &crate::WorkspaceCoverage,
    input: &crate::WorkspaceCoverageUpsert,
) -> bool {
    existing.source_kind == "structured"
        && existing.client_id == input.client_id
        && existing.priority == input.priority
        && existing.payer_name == input.payer_name
        && existing.plan_name == input.plan_name
        && existing.member_id == input.member_id
        && existing.group_number == input.group_number
        && existing.coverage_type == input.coverage_type
        && existing.coverage_status == input.coverage_status
        && existing.effective_date == input.effective_date
        && existing.termination_date == input.termination_date
        && existing.patient_relationship_to_subscriber == input.patient_relationship_to_subscriber
        && existing.subscriber_first_name == input.subscriber_first_name
        && existing.subscriber_middle_name == input.subscriber_middle_name
        && existing.subscriber_last_name == input.subscriber_last_name
        && existing.subscriber_suffix == input.subscriber_suffix
        && existing.subscriber_date_of_birth == input.subscriber_date_of_birth
        && existing.subscriber_administrative_sex == input.subscriber_administrative_sex
        && existing.subscriber_address_same_as_patient == input.subscriber_address_same_as_patient
        && existing.subscriber_address_line_1 == input.subscriber_address_line_1
        && existing.subscriber_address_line_2 == input.subscriber_address_line_2
        && existing.subscriber_city == input.subscriber_city
        && existing.subscriber_state_or_province == input.subscriber_state_or_province
        && existing.subscriber_postal_code == input.subscriber_postal_code
        && existing.subscriber_country == input.subscriber_country
        && existing.coverage_notes == input.coverage_notes
}

pub(super) fn safety_item(
    existing: &crate::WorkspacePatientSafetyItem,
    input: &crate::WorkspacePatientSafetyItemUpsert,
) -> bool {
    existing.client_id == input.client_id
        && existing.category == input.category
        && existing.name == input.name
        && existing.reaction == input.reaction
        && existing.severity == input.severity
        && existing.dose == input.dose
        && existing.route == input.route
        && existing.frequency == input.frequency
        && existing.status == input.status
        && existing.recorded_date == input.recorded_date
        && existing.notes == input.notes
        && existing.archived_at.is_none()
}

pub(super) fn encounter(
    existing: &crate::WorkspaceEncounter,
    input: &crate::WorkspaceEncounterUpsert,
) -> bool {
    existing.client_id == input.client_id
        && existing.kind == input.kind
        && existing.title == input.title
        && existing.status == input.status
        && timestamps_equal(&existing.started_at, &input.started_at)
        && timestamps_equal(&existing.ended_at, &input.ended_at)
        && existing.archived_at.is_none()
}

pub(super) fn note(existing: &crate::WorkspaceNote, input: &crate::WorkspaceNoteUpsert) -> bool {
    existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.title == input.title
        && existing.kind == input.kind
        && existing.body == input.body
        && existing.status == input.status
        && existing.archived_at.is_none()
}

pub(super) fn document(
    existing: &crate::WorkspaceDocument,
    input: &crate::WorkspaceDocumentUpsert,
) -> bool {
    existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.title == input.title
        && existing.kind == input.kind
        && existing.local_path == input.local_path
        && existing.notes == input.notes
        && existing.scope == input.scope
        && existing.detected_kind == input.detected_kind
        && existing.mime_type == input.mime_type
        && existing.file_size_bytes == input.file_size_bytes
        && timestamps_equal(&existing.modified_at, &input.modified_at)
        && existing.sha256 == input.sha256
        && existing.tags == input.tags
        && existing.source_label == input.source_label
        && existing.existence_status == input.existence_status
        && existing.metadata_json == input.metadata_json
        && existing.original_path == input.original_path
        && existing.reference_kind == input.reference_kind
        && existing.vault_path == input.vault_path
        && existing.content_sha256 == input.content_sha256
        && existing.thumbnail_path == input.thumbnail_path
        && existing.thumbnail_status == input.thumbnail_status
        && existing.thumbnail_mime_type == input.thumbnail_mime_type
        && existing.intake_source == input.intake_source
        && timestamps_equal(&existing.imported_at, &input.imported_at)
        && existing.archived_at.is_none()
}

fn timestamps_equal(left: &Option<DateTime<Utc>>, right: &Option<DateTime<Utc>>) -> bool {
    left.as_ref().map(DateTime::timestamp) == right.as_ref().map(DateTime::timestamp)
}

pub(super) fn derivative(
    existing: &crate::WorkspaceArtifactDerivative,
    input: &crate::WorkspaceArtifactDerivativeUpsert,
) -> bool {
    existing.document_id == input.document_id
        && existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.note_id == input.note_id
        && existing.kind == input.kind
        && existing.title == input.title
        && existing.body == input.body
        && existing.review_status == input.review_status
        && existing.source_method == input.source_method
        && existing.page_range == input.page_range
        && existing.timestamp_range == input.timestamp_range
        && existing.segment_label == input.segment_label
        && existing.tags == input.tags
        && existing.metadata_json == input.metadata_json
        && existing.archived_at.is_none()
}

pub(super) fn clip(
    existing: &crate::WorkspaceContextClip,
    input: &crate::WorkspaceContextClipUpsert,
) -> bool {
    existing.derivative_id == input.derivative_id
        && existing.document_id == input.document_id
        && existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.note_id == input.note_id
        && existing.kind == input.kind
        && existing.title == input.title
        && existing.body == input.body
        && existing.review_status == input.review_status
        && existing.source_method == input.source_method
        && existing.page_range == input.page_range
        && existing.timestamp_range == input.timestamp_range
        && existing.line_range == input.line_range
        && existing.segment_label == input.segment_label
        && existing.tags == input.tags
        && existing.metadata_json == input.metadata_json
        && existing.archived_at.is_none()
}

pub(super) fn task(existing: &crate::WorkspaceTask, input: &crate::WorkspaceTaskUpsert) -> bool {
    existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.note_id == input.note_id
        && existing.document_id == input.document_id
        && existing.title == input.title
        && existing.details == input.details
        && existing.kind == input.kind
        && existing.status == input.status
        && existing.priority == input.priority
        && existing.due_date == input.due_date
        && existing.assigned_to == input.assigned_to
        && existing.archived_at.is_none()
}
