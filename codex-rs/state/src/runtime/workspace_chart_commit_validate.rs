use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartCommitRequest;
use uuid::Uuid;

pub(super) fn normalize_before_hash(
    request: &mut WorkspaceChartCommitRequest,
) -> Result<(), WorkspaceChartCommitError> {
    request.idempotency_key = request.idempotency_key.trim().to_string();
    request.actor = request.actor.trim().to_string();
    request.reason = request.reason.trim().to_string();
    normalize_optional(&mut request.source_thread_id);
    normalize_optional(&mut request.source_turn_id);
    normalize_optional(&mut request.client_id);
    normalize_optional(&mut request.expected_versions.client);
    normalize_optional(&mut request.expected_versions.coverage);
    normalize_optional(&mut request.expected_versions.safety_item);
    normalize_optional(&mut request.expected_versions.encounter);
    normalize_optional(&mut request.expected_versions.document);
    normalize_optional(&mut request.expected_versions.artifact_derivative);
    normalize_optional(&mut request.expected_versions.context_clip);
    normalize_optional(&mut request.expected_versions.task);

    if request.idempotency_key.is_empty() {
        return validation("workspace chart commit idempotency key must not be empty");
    }
    if request.actor.is_empty() {
        return validation("workspace chart commit actor must not be empty");
    }
    if request.reason.is_empty() {
        return validation("workspace chart commit reason must not be empty");
    }

    if let Some(client) = request.client.as_mut() {
        normalize_optional(&mut client.id);
        client.display_name = required_text(&client.display_name, "client display name")?;
        normalize_optional(&mut client.legal_first_name);
        normalize_optional(&mut client.legal_middle_name);
        normalize_optional(&mut client.legal_last_name);
        normalize_optional(&mut client.legal_suffix);
        normalize_optional(&mut client.preferred_name);
        normalize_optional(&mut client.previous_name);
        normalize_optional(&mut client.date_of_birth);
        normalize_optional(&mut client.sex_or_gender);
        normalize_optional(&mut client.administrative_sex);
        normalize_optional(&mut client.preferred_language);
        normalize_optional(&mut client.external_id);
        normalize_optional(&mut client.record_start_date);
        normalize_optional(&mut client.record_end_date);
        normalize_optional(&mut client.primary_phone);
        normalize_optional(&mut client.primary_phone_use);
        normalize_optional(&mut client.secondary_phone);
        normalize_optional(&mut client.secondary_phone_use);
        normalize_optional(&mut client.email);
        normalize_optional(&mut client.primary_email);
        normalize_optional(&mut client.secondary_email);
        match (&client.email, &client.primary_email) {
            (Some(legacy), Some(primary)) if legacy != primary => {
                return validation("workspace client email and primaryEmail must match");
            }
            (Some(legacy), None) => client.primary_email = Some(legacy.clone()),
            (None, Some(primary)) => client.email = Some(primary.clone()),
            (Some(_), Some(_)) | (None, None) => {}
        }
        normalize_optional(&mut client.preferred_contact_method);
        normalize_optional(&mut client.address_line_1);
        normalize_optional(&mut client.address_line_2);
        normalize_optional(&mut client.city);
        normalize_optional(&mut client.state_or_province);
        normalize_optional(&mut client.postal_code);
        normalize_optional(&mut client.country);
        normalize_optional(&mut client.address_use);
        normalize_optional(&mut client.emergency_contact_name);
        normalize_optional(&mut client.emergency_contact_relationship);
        normalize_optional(&mut client.emergency_contact_phone);
        normalize_optional(&mut client.emergency_contact_email);
        normalize_optional(&mut client.contact_notes);
        normalize_optional(&mut client.payer_name);
        normalize_optional(&mut client.plan_name);
        normalize_optional(&mut client.member_id);
        normalize_optional(&mut client.group_number);
        normalize_optional(&mut client.coverage_type);
        normalize_optional(&mut client.coverage_status);
        normalize_optional(&mut client.coverage_notes);
    }
    if let Some(coverage) = request.coverage.as_mut() {
        normalize_optional(&mut coverage.id);
        coverage.client_id = coverage.client_id.trim().to_string();
        if !(1..=3).contains(&coverage.priority) {
            return validation("workspace coverage priority must be 1, 2, or 3");
        }
        normalize_optional(&mut coverage.payer_name);
        normalize_optional(&mut coverage.plan_name);
        normalize_optional(&mut coverage.member_id);
        normalize_optional(&mut coverage.group_number);
        normalize_optional(&mut coverage.coverage_type);
        normalize_optional(&mut coverage.coverage_status);
        normalize_optional(&mut coverage.effective_date);
        normalize_optional(&mut coverage.termination_date);
        normalize_optional(&mut coverage.patient_relationship_to_subscriber);
        normalize_optional(&mut coverage.subscriber_first_name);
        normalize_optional(&mut coverage.subscriber_middle_name);
        normalize_optional(&mut coverage.subscriber_last_name);
        normalize_optional(&mut coverage.subscriber_suffix);
        normalize_optional(&mut coverage.subscriber_date_of_birth);
        normalize_optional(&mut coverage.subscriber_administrative_sex);
        normalize_optional(&mut coverage.subscriber_address_line_1);
        normalize_optional(&mut coverage.subscriber_address_line_2);
        normalize_optional(&mut coverage.subscriber_city);
        normalize_optional(&mut coverage.subscriber_state_or_province);
        normalize_optional(&mut coverage.subscriber_postal_code);
        normalize_optional(&mut coverage.subscriber_country);
        normalize_optional(&mut coverage.coverage_notes);
    }
    normalize_root_shape(request)?;

    if let Some(input) = request.safety_item.as_mut() {
        normalize_optional(&mut input.id);
        input.client_id = input.client_id.trim().to_string();
        input.category = normalize_safety_category(&input.category)?;
        input.name = required_text(&input.name, "patient safety item name")?;
        normalize_optional(&mut input.reaction);
        normalize_optional(&mut input.severity);
        normalize_optional(&mut input.dose);
        normalize_optional(&mut input.route);
        normalize_optional(&mut input.frequency);
        normalize_optional(&mut input.status);
        normalize_optional(&mut input.recorded_date);
    }
    if let Some(input) = request.encounter.as_mut() {
        normalize_optional(&mut input.id);
        input.client_id = input.client_id.trim().to_string();
        input.kind = nonempty(&input.kind, "encounter");
        input.title = required_text(&input.title, "encounter title")?;
        input.status = nonempty(&input.status, "open");
        if !matches!(input.status.as_str(), "open" | "completed" | "cancelled") {
            return validation("workspace encounter status must be open, completed, or cancelled");
        }
        if input
            .started_at
            .zip(input.ended_at)
            .is_some_and(|(started_at, ended_at)| ended_at < started_at)
        {
            return validation("workspace encounter endedAt must not precede startedAt");
        }
    }
    if let Some(change) = request.note.as_mut() {
        let input = &mut change.upsert;
        normalize_optional(&mut input.id);
        input.client_id = input.client_id.trim().to_string();
        normalize_optional(&mut input.encounter_id);
        input.title = required_text(&input.title, "note title")?;
        input.kind = nonempty(&input.kind, "note");
        input.status = nonempty(&input.status, "draft");
        normalize_optional(&mut input.source_thread_id);
        normalize_optional(&mut input.source_turn_id);
        normalize_optional(&mut input.summary);
        if input.status != "draft" {
            return validation("workspace atomic note status must be draft");
        }
    }
    if let Some(input) = request.document.as_mut() {
        normalize_optional(&mut input.id);
        input.client_id = input.client_id.trim().to_string();
        normalize_optional(&mut input.encounter_id);
        input.title = required_text(&input.title, "document title")?;
        input.kind = nonempty(&input.kind, "document");
        input.local_path = required_text(&input.local_path, "document local path")?;
        input.scope = nonempty(&input.scope, "patient");
        input.detected_kind = input.detected_kind.trim().to_string();
        if input.file_size_bytes.is_some_and(|size| size < 0) {
            return validation("workspace document fileSizeBytes must not be negative");
        }
        normalize_optional(&mut input.mime_type);
        normalize_optional(&mut input.sha256);
        input.existence_status = nonempty(&input.existence_status, "unknown");
        input.metadata_json = nonempty(&input.metadata_json, "{}");
        input.original_path = nonempty(&input.original_path, &input.local_path);
        input.reference_kind = nonempty(&input.reference_kind, "local_reference");
        normalize_optional(&mut input.content_sha256);
        input.thumbnail_status = nonempty(&input.thumbnail_status, "none");
        normalize_optional(&mut input.thumbnail_mime_type);
    }
    if let Some(input) = request.artifact_derivative.as_mut() {
        normalize_optional(&mut input.id);
        input.document_id = input.document_id.trim().to_string();
        input.client_id = input.client_id.trim().to_string();
        normalize_optional(&mut input.encounter_id);
        normalize_optional(&mut input.note_id);
        input.kind = nonempty(&input.kind, "human annotation");
        input.title = required_text(&input.title, "artifact derivative title")?;
        require_nonempty(&input.body, "artifact derivative body")?;
        input.review_status =
            normalize_review_status(&input.review_status, "artifact derivative review status")?;
        if input.review_status == "archived" {
            return validation(
                "workspace atomic artifact derivative review status cannot archive records",
            );
        }
        input.source_method = nonempty(&input.source_method, "human_typed");
        input.metadata_json = nonempty(&input.metadata_json, "{}");
    }
    if let Some(input) = request.context_clip.as_mut() {
        normalize_optional(&mut input.id);
        input.derivative_id = input.derivative_id.trim().to_string();
        input.document_id = input.document_id.trim().to_string();
        input.client_id = input.client_id.trim().to_string();
        normalize_optional(&mut input.encounter_id);
        normalize_optional(&mut input.note_id);
        input.kind = nonempty(&input.kind, "generic excerpt");
        input.title = required_text(&input.title, "context clip title")?;
        require_nonempty(&input.body, "context clip body")?;
        input.review_status =
            normalize_review_status(&input.review_status, "context clip review status")?;
        if input.review_status == "archived" {
            return validation(
                "workspace atomic context clip review status cannot archive records",
            );
        }
        input.source_method = nonempty(&input.source_method, "human_selected");
        input.metadata_json = nonempty(&input.metadata_json, "{}");
    }
    if let Some(input) = request.task.as_mut() {
        normalize_optional(&mut input.id);
        input.client_id = input.client_id.trim().to_string();
        normalize_optional(&mut input.encounter_id);
        normalize_optional(&mut input.note_id);
        normalize_optional(&mut input.document_id);
        input.title = required_text(&input.title, "task title")?;
        input.kind = nonempty(&input.kind, "task");
        normalize_optional(&mut input.due_date);
        normalize_optional(&mut input.assigned_to);
    }
    Ok(())
}

fn normalize_root_shape(
    request: &mut WorkspaceChartCommitRequest,
) -> Result<(), WorkspaceChartCommitError> {
    let embedded_id = request.client.as_ref().and_then(|client| client.id.clone());
    match (request.client_id.as_ref(), embedded_id.as_ref()) {
        (Some(root_id), Some(embedded_id)) if root_id != embedded_id => {
            return validation(format!(
                "workspace chart client id `{embedded_id}` conflicts with root `{root_id}`"
            ));
        }
        (None, Some(embedded_id)) => request.client_id = Some(embedded_id.clone()),
        (Some(_), Some(_)) | (Some(_), None) | (None, None) => {}
    }
    if request.client_id.is_none() && request.client.is_none() {
        return validation("workspace chart commit requires clientId or client");
    }
    if let (Some(root_id), Some(client)) = (request.client_id.as_ref(), request.client.as_mut()) {
        client.id = Some(root_id.clone());
    }
    Ok(())
}

pub(super) fn allocate_and_bind(
    request: &mut WorkspaceChartCommitRequest,
    client_id: &str,
    root_exists: bool,
) -> Result<(), WorkspaceChartCommitError> {
    if let Some(client) = request.client.as_mut() {
        client.id = Some(client_id.to_string());
    }
    if let Some(coverage) = request.coverage.as_mut() {
        allocate(&mut coverage.id);
        bind_client("coverage", &mut coverage.client_id, client_id, root_exists)?;
    }
    let encounter_id = request
        .encounter
        .as_mut()
        .map(|input| allocate(&mut input.id));
    let note_id = request
        .note
        .as_mut()
        .map(|change| allocate(&mut change.upsert.id));
    let document_id = request
        .document
        .as_mut()
        .map(|input| allocate(&mut input.id));
    let derivative_id = request
        .artifact_derivative
        .as_mut()
        .map(|input| allocate(&mut input.id));
    if let Some(input) = request.safety_item.as_mut() {
        allocate(&mut input.id);
        bind_client("safety item", &mut input.client_id, client_id, root_exists)?;
    }
    if let Some(input) = request.encounter.as_mut() {
        bind_client("encounter", &mut input.client_id, client_id, root_exists)?;
    }
    if let Some(change) = request.note.as_mut() {
        bind_client("note", &mut change.upsert.client_id, client_id, root_exists)?;
        change.upsert.actor = request.actor.clone();
        change.upsert.source_thread_id = request.source_thread_id.clone();
        change.upsert.source_turn_id = request.source_turn_id.clone();
        change.upsert.summary = Some(request.reason.clone());
        if change.upsert.encounter_id.is_none() {
            change.upsert.encounter_id.clone_from(&encounter_id);
        }
    }
    if let Some(input) = request.document.as_mut() {
        bind_client("document", &mut input.client_id, client_id, root_exists)?;
        if input.encounter_id.is_none() {
            input.encounter_id.clone_from(&encounter_id);
        }
    }
    if let Some(input) = request.artifact_derivative.as_mut() {
        bind_client(
            "artifact derivative",
            &mut input.client_id,
            client_id,
            root_exists,
        )?;
        input.actor = request.actor.clone();
        if input.document_id.is_empty() {
            input.document_id = document_id.clone().unwrap_or_default();
        }
        if input.encounter_id.is_none() {
            input.encounter_id.clone_from(&encounter_id);
        }
        if input.note_id.is_none() {
            input.note_id.clone_from(&note_id);
        }
        if input.document_id.is_empty() {
            return validation("workspace artifact derivative document link must not be empty");
        }
    }
    if let Some(input) = request.context_clip.as_mut() {
        allocate(&mut input.id);
        bind_client("context clip", &mut input.client_id, client_id, root_exists)?;
        input.actor = request.actor.clone();
        if input.derivative_id.is_empty() {
            input.derivative_id = derivative_id.unwrap_or_default();
        }
        if input.document_id.is_empty() {
            input.document_id = document_id.clone().unwrap_or_default();
        }
        if input.encounter_id.is_none() {
            input.encounter_id.clone_from(&encounter_id);
        }
        if input.note_id.is_none() {
            input.note_id.clone_from(&note_id);
        }
        if input.document_id.is_empty() {
            return validation("workspace context clip document link must not be empty");
        }
        if input.derivative_id.is_empty() {
            return validation("workspace context clip derivative link must not be empty");
        }
    }
    if let Some(input) = request.task.as_mut() {
        allocate(&mut input.id);
        bind_client("task", &mut input.client_id, client_id, root_exists)?;
        input.actor = request.actor.clone();
        if input.encounter_id.is_none() {
            input.encounter_id.clone_from(&encounter_id);
        }
        if input.note_id.is_none() {
            input.note_id.clone_from(&note_id);
        }
        if input.document_id.is_none() {
            input.document_id.clone_from(&document_id);
        }
    }
    Ok(())
}

fn bind_client(
    label: &str,
    child_client_id: &mut String,
    client_id: &str,
    root_exists: bool,
) -> Result<(), WorkspaceChartCommitError> {
    if root_exists && !child_client_id.is_empty() && child_client_id != client_id {
        return validation(format!(
            "workspace {label} belongs to client `{child_client_id}`, not `{client_id}`"
        ));
    }
    *child_client_id = client_id.to_string();
    Ok(())
}

fn allocate(id: &mut Option<String>) -> String {
    id.get_or_insert_with(|| Uuid::new_v4().to_string()).clone()
}

fn normalize_optional(value: &mut Option<String>) {
    *value = value
        .take()
        .and_then(|value| (!value.trim().is_empty()).then(|| value.trim().to_string()));
}

fn required_text(value: &str, label: &str) -> Result<String, WorkspaceChartCommitError> {
    let value = value.trim();
    if value.is_empty() {
        return validation(format!("workspace {label} must not be empty"));
    }
    Ok(value.to_string())
}

fn require_nonempty(value: &str, label: &str) -> Result<(), WorkspaceChartCommitError> {
    if value.trim().is_empty() {
        return validation(format!("workspace {label} must not be empty"));
    }
    Ok(())
}

fn nonempty(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_safety_category(value: &str) -> Result<String, WorkspaceChartCommitError> {
    let category = match value.trim().to_ascii_lowercase().as_str() {
        "allergy" | "allergies" => "allergy",
        "medication" | "medications" | "med" | "meds" => "medication",
        "condition" | "conditions" | "problem" | "problems" => "condition",
        "precaution" | "precautions" | "restriction" | "restrictions" => "precaution",
        other => {
            return validation(format!(
                "unsupported workspace patient safety category `{other}`"
            ));
        }
    };
    Ok(category.to_string())
}

fn normalize_review_status(value: &str, label: &str) -> Result<String, WorkspaceChartCommitError> {
    let value = nonempty(value, "draft");
    if !matches!(
        value.as_str(),
        "draft" | "human_reviewed" | "superseded" | "archived"
    ) {
        return validation(format!(
            "workspace {label} must be draft, human_reviewed, superseded, or archived"
        ));
    }
    Ok(value)
}

pub(super) fn note_status_is_locked(status: &str) -> bool {
    matches!(status.trim(), "signed" | "addended")
}

fn validation<T>(message: impl Into<String>) -> Result<T, WorkspaceChartCommitError> {
    Err(WorkspaceChartCommitError::Validation {
        message: message.into(),
    })
}
