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
        && existing.preferred_name == input.preferred_name
        && existing.date_of_birth == input.date_of_birth
        && existing.sex_or_gender == input.sex_or_gender
        && existing.external_id == input.external_id
        && existing.record_start_date == input.record_start_date
        && existing.record_end_date == input.record_end_date
        && existing.summary == input.summary
        && existing.primary_phone == input.primary_phone
        && existing.secondary_phone == input.secondary_phone
        && existing.email == input.email
        && existing.preferred_contact_method == input.preferred_contact_method
        && existing.emergency_contact_name == input.emergency_contact_name
        && existing.emergency_contact_relationship == input.emergency_contact_relationship
        && existing.emergency_contact_phone == input.emergency_contact_phone
        && existing.emergency_contact_email == input.emergency_contact_email
        && existing.contact_notes == input.contact_notes
        && existing.payer_name == input.payer_name
        && existing.plan_name == input.plan_name
        && existing.member_id == input.member_id
        && existing.group_number == input.group_number
        && existing.coverage_type == input.coverage_type
        && existing.coverage_status == input.coverage_status
        && existing.coverage_notes == input.coverage_notes
        && existing.archived_at.is_none()
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
