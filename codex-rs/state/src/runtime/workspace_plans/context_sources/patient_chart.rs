use super::context_source;
use super::safe_body;
use super::safe_label;
use serde_json::Value;
use sqlx::Row;

pub(super) async fn read_patient_chart(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    max_records: u32,
) -> super::super::PlanResult<Vec<crate::WorkspacePlanningContextSource>> {
    let patient = sqlx::query(
        r#"
SELECT c.id, c.display_name, c.preferred_name, c.date_of_birth, c.sex_or_gender,
       c.external_id, c.summary, c.record_start_date, c.record_end_date,
       c.legal_first_name, c.legal_middle_name, c.legal_last_name, c.legal_suffix,
       c.previous_name, c.administrative_sex, c.preferred_language,
       c.interpreter_required, c.created_at_ms, c.updated_at_ms,
       contact.primary_phone, contact.primary_phone_use, contact.secondary_phone,
       contact.secondary_phone_use, contact.email, contact.secondary_email,
       contact.preferred_contact_method, contact.emergency_contact_name,
       contact.emergency_contact_relationship, contact.emergency_contact_phone,
       contact.emergency_contact_email, contact.contact_notes,
       contact.address_line_1, contact.address_line_2, contact.city,
       contact.state_or_province, contact.postal_code, contact.country,
       contact.address_use
FROM workspace_clients AS c
LEFT JOIN workspace_client_contacts AS contact ON contact.client_id = c.id
WHERE c.id = ? AND c.archived_at_ms IS NULL
        "#,
    )
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        super::super::validation(format!(
            "workspace planning patient `{}` was not found",
            execution.client_id
        ))
    })?;

    let display_name: String = patient.try_get("display_name")?;
    let (display_name, name_paths_redacted, name_truncated) = safe_label(&display_name);
    let summary: String = patient.try_get("summary")?;
    let (summary, summary_paths_redacted, summary_truncated) = safe_body(&summary);
    let contact_notes: Option<String> = patient.try_get("contact_notes")?;
    let (contact_notes, contact_notes_paths_redacted, contact_notes_truncated) =
        safe_optional_body(contact_notes);

    let coverages = coverage_rows(tx, &execution.client_id, max_records).await?;
    let safety = safety_rows(tx, &execution.client_id, max_records).await?;
    let tasks = task_rows(tx, &execution.client_id, max_records).await?;
    let encounters = encounter_rows(tx, &execution.client_id, max_records).await?;
    let current_note = current_note(tx, execution).await?;
    let snapshot_json = serde_json::json!({
        "snapshot_kind": "patient_chart",
        "source_checkpoint": {
            "id": execution.source_checkpoint_id,
            "revision": execution.source_checkpoint_revision,
            "sha256": execution.source_checkpoint_sha256,
        },
        "demographics": {
            "id": execution.client_id,
            "display_name": display_name,
            "display_name_truncated": name_truncated,
            "display_name_local_paths_redacted": name_paths_redacted,
            "preferred_name": patient.try_get::<Option<String>, _>("preferred_name")?,
            "legal_first_name": patient.try_get::<Option<String>, _>("legal_first_name")?,
            "legal_middle_name": patient.try_get::<Option<String>, _>("legal_middle_name")?,
            "legal_last_name": patient.try_get::<Option<String>, _>("legal_last_name")?,
            "legal_suffix": patient.try_get::<Option<String>, _>("legal_suffix")?,
            "previous_name": patient.try_get::<Option<String>, _>("previous_name")?,
            "date_of_birth": patient.try_get::<Option<String>, _>("date_of_birth")?,
            "administrative_sex": patient.try_get::<Option<String>, _>("administrative_sex")?,
            "sex_or_gender": patient.try_get::<Option<String>, _>("sex_or_gender")?,
            "preferred_language": patient.try_get::<Option<String>, _>("preferred_language")?,
            "interpreter_required": patient.try_get::<i64, _>("interpreter_required")? != 0,
            "external_id": patient.try_get::<Option<String>, _>("external_id")?,
            "record_start_date": patient.try_get::<Option<String>, _>("record_start_date")?,
            "record_end_date": patient.try_get::<Option<String>, _>("record_end_date")?,
            "summary": summary,
            "summary_truncated": summary_truncated,
            "summary_local_paths_redacted": summary_paths_redacted,
            "created_at_ms": patient.try_get::<i64, _>("created_at_ms")?,
            "updated_at_ms": patient.try_get::<i64, _>("updated_at_ms")?,
        },
        "contact": {
            "primary_phone": patient.try_get::<Option<String>, _>("primary_phone")?,
            "primary_phone_use": patient.try_get::<Option<String>, _>("primary_phone_use")?,
            "secondary_phone": patient.try_get::<Option<String>, _>("secondary_phone")?,
            "secondary_phone_use": patient.try_get::<Option<String>, _>("secondary_phone_use")?,
            "email": patient.try_get::<Option<String>, _>("email")?,
            "secondary_email": patient.try_get::<Option<String>, _>("secondary_email")?,
            "preferred_contact_method": patient.try_get::<Option<String>, _>("preferred_contact_method")?,
            "emergency_contact_name": patient.try_get::<Option<String>, _>("emergency_contact_name")?,
            "emergency_contact_relationship": patient.try_get::<Option<String>, _>("emergency_contact_relationship")?,
            "emergency_contact_phone": patient.try_get::<Option<String>, _>("emergency_contact_phone")?,
            "emergency_contact_email": patient.try_get::<Option<String>, _>("emergency_contact_email")?,
            "address_line_1": patient.try_get::<Option<String>, _>("address_line_1")?,
            "address_line_2": patient.try_get::<Option<String>, _>("address_line_2")?,
            "city": patient.try_get::<Option<String>, _>("city")?,
            "state_or_province": patient.try_get::<Option<String>, _>("state_or_province")?,
            "postal_code": patient.try_get::<Option<String>, _>("postal_code")?,
            "country": patient.try_get::<Option<String>, _>("country")?,
            "address_use": patient.try_get::<Option<String>, _>("address_use")?,
            "notes": contact_notes,
            "notes_truncated": contact_notes_truncated,
            "notes_local_paths_redacted": contact_notes_paths_redacted,
        },
        "coverages": coverages,
        "safety_items": safety,
        "open_tasks": tasks,
        "current_note": current_note,
        "encounters": encounters,
    })
    .to_string();
    Ok(vec![context_source(
        "patient_chart",
        execution.client_id.clone(),
        Some(execution.source_checkpoint_revision),
        &display_name,
        snapshot_json,
    )])
}

async fn coverage_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    limit: u32,
) -> super::super::PlanResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, priority, payer_name, plan_name, member_id, group_number, coverage_type, coverage_status, effective_date, termination_date, patient_relationship_to_subscriber, updated_at_ms FROM workspace_coverages WHERE client_id = ? ORDER BY priority ASC LIMIT ?",
    )
    .bind(client_id)
    .bind(i64::from(limit))
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| Ok(serde_json::json!({
            "id": row.try_get::<String, _>("id")?,
            "priority": row.try_get::<i64, _>("priority")?,
            "payer_name": row.try_get::<Option<String>, _>("payer_name")?,
            "plan_name": row.try_get::<Option<String>, _>("plan_name")?,
            "member_id": row.try_get::<Option<String>, _>("member_id")?,
            "group_number": row.try_get::<Option<String>, _>("group_number")?,
            "coverage_type": row.try_get::<Option<String>, _>("coverage_type")?,
            "coverage_status": row.try_get::<Option<String>, _>("coverage_status")?,
            "effective_date": row.try_get::<Option<String>, _>("effective_date")?,
            "termination_date": row.try_get::<Option<String>, _>("termination_date")?,
            "patient_relationship_to_subscriber": row.try_get::<Option<String>, _>("patient_relationship_to_subscriber")?,
            "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        })))
        .collect()
}

async fn safety_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    limit: u32,
) -> super::super::PlanResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, category, name, reaction, severity, dose, route, frequency, status, recorded_date, notes, updated_at_ms FROM workspace_patient_safety_items WHERE client_id = ? AND archived_at_ms IS NULL ORDER BY category, updated_at_ms DESC, id LIMIT ?",
    )
    .bind(client_id)
    .bind(i64::from(limit))
    .fetch_all(&mut **tx)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let notes: String = row.try_get("notes")?;
        let (notes, paths_redacted, truncated) = safe_body(&notes);
        values.push(serde_json::json!({
            "id": row.try_get::<String, _>("id")?,
            "category": row.try_get::<String, _>("category")?,
            "name": row.try_get::<String, _>("name")?,
            "reaction": row.try_get::<Option<String>, _>("reaction")?,
            "severity": row.try_get::<Option<String>, _>("severity")?,
            "dose": row.try_get::<Option<String>, _>("dose")?,
            "route": row.try_get::<Option<String>, _>("route")?,
            "frequency": row.try_get::<Option<String>, _>("frequency")?,
            "status": row.try_get::<Option<String>, _>("status")?,
            "recorded_date": row.try_get::<Option<String>, _>("recorded_date")?,
            "notes": notes,
            "notes_truncated": truncated,
            "notes_local_paths_redacted": paths_redacted,
            "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        }));
    }
    Ok(values)
}

async fn task_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    limit: u32,
) -> super::super::PlanResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, encounter_id, note_id, document_id, title, details, kind, status, priority, due_date, assigned_to, updated_at_ms FROM workspace_tasks WHERE client_id = ? AND archived_at_ms IS NULL AND status != 'completed' ORDER BY updated_at_ms DESC, id LIMIT ?",
    )
    .bind(client_id)
    .bind(i64::from(limit))
    .fetch_all(&mut **tx)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let title: String = row.try_get("title")?;
        let details: String = row.try_get("details")?;
        let (title, _, title_truncated) = safe_label(&title);
        let (details, details_paths_redacted, details_truncated) = safe_body(&details);
        values.push(serde_json::json!({
            "id": row.try_get::<String, _>("id")?,
            "encounter_id": row.try_get::<Option<String>, _>("encounter_id")?,
            "note_id": row.try_get::<Option<String>, _>("note_id")?,
            "document_id": row.try_get::<Option<String>, _>("document_id")?,
            "title": title,
            "title_truncated": title_truncated,
            "details": details,
            "details_truncated": details_truncated,
            "details_local_paths_redacted": details_paths_redacted,
            "kind": row.try_get::<String, _>("kind")?,
            "status": row.try_get::<String, _>("status")?,
            "priority": row.try_get::<String, _>("priority")?,
            "due_date": row.try_get::<Option<String>, _>("due_date")?,
            "assigned_to": row.try_get::<Option<String>, _>("assigned_to")?,
            "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        }));
    }
    Ok(values)
}

async fn encounter_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    limit: u32,
) -> super::super::PlanResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, kind, title, status, started_at_ms, ended_at_ms, updated_at_ms FROM workspace_encounters WHERE client_id = ? AND archived_at_ms IS NULL ORDER BY COALESCE(started_at_ms, updated_at_ms) DESC, id LIMIT ?",
    )
    .bind(client_id)
    .bind(i64::from(limit))
    .fetch_all(&mut **tx)
    .await?;
    let mut values = Vec::with_capacity(rows.len());
    for row in rows {
        let title: String = row.try_get("title")?;
        let (title, paths_redacted, truncated) = safe_label(&title);
        values.push(serde_json::json!({
            "id": row.try_get::<String, _>("id")?,
            "kind": row.try_get::<String, _>("kind")?,
            "title": title,
            "title_truncated": truncated,
            "title_local_paths_redacted": paths_redacted,
            "status": row.try_get::<String, _>("status")?,
            "started_at_ms": row.try_get::<Option<i64>, _>("started_at_ms")?,
            "ended_at_ms": row.try_get::<Option<i64>, _>("ended_at_ms")?,
            "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        }));
    }
    Ok(values)
}

async fn current_note(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
) -> super::super::PlanResult<Option<Value>> {
    let row = sqlx::query(
        r#"
SELECT note.id, note.encounter_id, note.title, note.kind, note.body, note.status,
       note.current_revision, note.updated_at_ms
FROM workspace_draft_checkpoints AS checkpoint
JOIN workspace_notes AS note ON note.id = checkpoint.note_id
WHERE checkpoint.id = ? AND checkpoint.client_id = ? AND note.client_id = ?
        "#,
    )
    .bind(&execution.source_checkpoint_id)
    .bind(&execution.client_id)
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let title: String = row.try_get("title")?;
    let body: String = row.try_get("body")?;
    let (title, title_paths_redacted, title_truncated) = safe_label(&title);
    let (body, body_paths_redacted, body_truncated) = safe_body(&body);
    Ok(Some(serde_json::json!({
        "id": row.try_get::<String, _>("id")?,
        "encounter_id": row.try_get::<Option<String>, _>("encounter_id")?,
        "title": title,
        "title_truncated": title_truncated,
        "title_local_paths_redacted": title_paths_redacted,
        "kind": row.try_get::<String, _>("kind")?,
        "body": body,
        "body_truncated": body_truncated,
        "body_local_paths_redacted": body_paths_redacted,
        "status": row.try_get::<String, _>("status")?,
        "current_revision": row.try_get::<i64, _>("current_revision")?,
        "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
    })))
}

fn safe_optional_body(value: Option<String>) -> (Option<String>, bool, bool) {
    match value {
        Some(value) => {
            let (value, paths_redacted, truncated) = safe_body(&value);
            (Some(value), paths_redacted, truncated)
        }
        None => (None, false, false),
    }
}
