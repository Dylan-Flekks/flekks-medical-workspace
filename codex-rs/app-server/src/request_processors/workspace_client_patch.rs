use super::*;

fn patched_text(
    patch: Option<Option<String>>,
    existing: Option<&String>,
) -> Option<String> {
    match patch {
        Some(value) => empty_to_none(value),
        None => existing.cloned(),
    }
}

pub(super) fn state_client_upsert(
    value: WorkspaceClientUpsertParams,
    existing: Option<&codex_state::WorkspaceClient>,
) -> Result<codex_state::WorkspaceClientUpsert, JSONRPCErrorError> {
    let legacy_email = empty_to_none(value.email);
    let primary_email = match value.primary_email {
        Some(primary) => {
            let primary = empty_to_none(primary);
            if legacy_email
                .as_ref()
                .is_some_and(|legacy| primary.as_ref() != Some(legacy))
            {
                return Err(invalid_request(
                    "workspace client email and primaryEmail must match",
                ));
            }
            primary
        }
        None => legacy_email.clone().or_else(|| {
            existing.and_then(|client| {
                client
                    .primary_email
                    .clone()
                    .or_else(|| client.email.clone())
            })
        }),
    };
    Ok(codex_state::WorkspaceClientUpsert {
        id: empty_to_none(value.id),
        display_name: value.display_name.trim().to_string(),
        legal_first_name: patched_text(
            value.legal_first_name,
            existing.and_then(|client| client.legal_first_name.as_ref()),
        ),
        legal_middle_name: patched_text(
            value.legal_middle_name,
            existing.and_then(|client| client.legal_middle_name.as_ref()),
        ),
        legal_last_name: patched_text(
            value.legal_last_name,
            existing.and_then(|client| client.legal_last_name.as_ref()),
        ),
        legal_suffix: patched_text(
            value.legal_suffix,
            existing.and_then(|client| client.legal_suffix.as_ref()),
        ),
        preferred_name: empty_to_none(value.preferred_name),
        previous_name: patched_text(
            value.previous_name,
            existing.and_then(|client| client.previous_name.as_ref()),
        ),
        date_of_birth: empty_to_none(value.date_of_birth),
        sex_or_gender: empty_to_none(value.sex_or_gender),
        administrative_sex: patched_text(
            value.administrative_sex,
            existing.and_then(|client| client.administrative_sex.as_ref()),
        ),
        preferred_language: patched_text(
            value.preferred_language,
            existing.and_then(|client| client.preferred_language.as_ref()),
        ),
        interpreter_required: match value.interpreter_required {
            Some(Some(value)) => value,
            Some(None) => false,
            None => existing.is_some_and(|client| client.interpreter_required),
        },
        external_id: empty_to_none(value.external_id),
        record_start_date: empty_to_none(value.record_start_date),
        record_end_date: empty_to_none(value.record_end_date),
        summary: value.summary,
        primary_phone: empty_to_none(value.primary_phone),
        primary_phone_use: patched_text(
            value.primary_phone_use,
            existing.and_then(|client| client.primary_phone_use.as_ref()),
        ),
        secondary_phone: empty_to_none(value.secondary_phone),
        secondary_phone_use: patched_text(
            value.secondary_phone_use,
            existing.and_then(|client| client.secondary_phone_use.as_ref()),
        ),
        email: primary_email.clone(),
        primary_email,
        secondary_email: patched_text(
            value.secondary_email,
            existing.and_then(|client| client.secondary_email.as_ref()),
        ),
        preferred_contact_method: empty_to_none(value.preferred_contact_method),
        address_line_1: patched_text(
            value.address_line_1,
            existing.and_then(|client| client.address_line_1.as_ref()),
        ),
        address_line_2: patched_text(
            value.address_line_2,
            existing.and_then(|client| client.address_line_2.as_ref()),
        ),
        city: patched_text(
            value.city,
            existing.and_then(|client| client.city.as_ref()),
        ),
        state_or_province: patched_text(
            value.state_or_province,
            existing.and_then(|client| client.state_or_province.as_ref()),
        ),
        postal_code: patched_text(
            value.postal_code,
            existing.and_then(|client| client.postal_code.as_ref()),
        ),
        country: patched_text(
            value.country,
            existing.and_then(|client| client.country.as_ref()),
        ),
        address_use: patched_text(
            value.address_use,
            existing.and_then(|client| client.address_use.as_ref()),
        ),
        emergency_contact_name: empty_to_none(value.emergency_contact_name),
        emergency_contact_relationship: empty_to_none(value.emergency_contact_relationship),
        emergency_contact_phone: empty_to_none(value.emergency_contact_phone),
        emergency_contact_email: empty_to_none(value.emergency_contact_email),
        contact_notes: empty_to_none(value.contact_notes),
        payer_name: empty_to_none(value.payer_name),
        plan_name: empty_to_none(value.plan_name),
        member_id: empty_to_none(value.member_id),
        group_number: empty_to_none(value.group_number),
        coverage_type: empty_to_none(value.coverage_type),
        coverage_status: empty_to_none(value.coverage_status),
        coverage_notes: empty_to_none(value.coverage_notes),
    })
}
