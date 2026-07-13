use super::*;
use codex_app_server_protocol::WorkspaceBillingReadiness;
use codex_app_server_protocol::WorkspaceCoverage;
use codex_app_server_protocol::WorkspaceCoverageListParams;
use codex_app_server_protocol::WorkspaceCoverageListResponse;
use codex_app_server_protocol::WorkspaceCoverageMatchResult;
use codex_app_server_protocol::WorkspaceCoverageUpsertParams;
use codex_app_server_protocol::WorkspaceCoverageVerification;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateParams;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateResponse;
use codex_app_server_protocol::WorkspaceCoverageVerificationListParams;
use codex_app_server_protocol::WorkspaceCoverageVerificationListResponse;
use codex_app_server_protocol::WorkspaceCoverageVerificationSubject;

impl WorkspaceRequestProcessor {
    pub(crate) async fn coverage_list(
        &self,
        params: WorkspaceCoverageListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace coverage clientId must not be empty",
            ));
        }
        let limit = params.limit.unwrap_or(50).clamp(1, 100);
        let state_db = self.state_db()?;
        let mut coverages = state_db
            .workspace()
            .list_coverages_with_billing_readiness(
                &params.client_id,
                params.cursor.as_deref(),
                limit.saturating_add(1),
            )
            .await
            .map_err(|err| invalid_request(format!("failed to list workspace coverage: {err}")))?;
        let next_cursor =
            (coverages.len() > limit as usize).then(|| coverages[limit as usize - 1].0.id.clone());
        coverages.truncate(limit as usize);
        let mut data = Vec::with_capacity(coverages.len());
        for (coverage, readiness) in coverages {
            data.push(api_coverage_from_state(coverage, readiness)?);
        }
        Ok(Some(
            WorkspaceCoverageListResponse { data, next_cursor }.into(),
        ))
    }

    pub(crate) async fn coverage_verification_list(
        &self,
        params: WorkspaceCoverageVerificationListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.coverage_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace coverage verification coverageId must not be empty",
            ));
        }
        let limit = params.limit.unwrap_or(50).clamp(1, 100);
        let state_db = self.state_db()?;
        let mut verifications = state_db
            .workspace()
            .list_coverage_verifications(
                &params.coverage_id,
                params.cursor.as_deref(),
                limit.saturating_add(1),
            )
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to list workspace coverage verifications: {err}"
                ))
            })?;
        let next_cursor = (verifications.len() > limit as usize)
            .then(|| verifications[limit as usize - 1].id.clone());
        verifications.truncate(limit as usize);
        let data = verifications
            .into_iter()
            .map(api_verification_from_state)
            .collect();
        Ok(Some(
            WorkspaceCoverageVerificationListResponse { data, next_cursor }.into(),
        ))
    }

    pub(crate) async fn coverage_verification_create(
        &self,
        params: WorkspaceCoverageVerificationCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let result = state_db
            .workspace()
            .create_coverage_verification(codex_state::WorkspaceCoverageVerificationCreate {
                coverage_id: params.coverage_id,
                source_document_id: params.source_document_id,
                expected_patient_version: params.expected_patient_version,
                expected_coverage_version: params.expected_coverage_version,
                expected_document_version: params.expected_document_version,
                compared_subject: state_subject(params.compared_subject),
                observed_first_name: empty_to_none(params.observed_first_name),
                observed_middle_name: empty_to_none(params.observed_middle_name),
                observed_last_name: empty_to_none(params.observed_last_name),
                observed_suffix: empty_to_none(params.observed_suffix),
                observed_member_id: empty_to_none(params.observed_member_id),
                actor: params.actor,
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to verify workspace coverage card: {err}"))
            })?;
        Ok(Some(
            WorkspaceCoverageVerificationCreateResponse {
                verification: api_verification_from_state(result.verification),
                billing_readiness: api_readiness(result.billing_readiness),
            }
            .into(),
        ))
    }
}

pub(super) fn state_coverage_upsert(
    value: WorkspaceCoverageUpsertParams,
) -> codex_state::WorkspaceCoverageUpsert {
    codex_state::WorkspaceCoverageUpsert {
        id: value.id,
        client_id: value.client_id,
        priority: value.priority,
        payer_name: value.payer_name,
        plan_name: value.plan_name,
        member_id: value.member_id,
        group_number: value.group_number,
        coverage_type: value.coverage_type,
        coverage_status: value.coverage_status,
        effective_date: value.effective_date,
        termination_date: value.termination_date,
        patient_relationship_to_subscriber: value.patient_relationship_to_subscriber,
        subscriber_first_name: value.subscriber_first_name,
        subscriber_middle_name: value.subscriber_middle_name,
        subscriber_last_name: value.subscriber_last_name,
        subscriber_suffix: value.subscriber_suffix,
        subscriber_date_of_birth: value.subscriber_date_of_birth,
        subscriber_administrative_sex: value.subscriber_administrative_sex,
        subscriber_address_same_as_patient: value.subscriber_address_same_as_patient,
        subscriber_address_line_1: value.subscriber_address_line_1,
        subscriber_address_line_2: value.subscriber_address_line_2,
        subscriber_city: value.subscriber_city,
        subscriber_state_or_province: value.subscriber_state_or_province,
        subscriber_postal_code: value.subscriber_postal_code,
        subscriber_country: value.subscriber_country,
        coverage_notes: value.coverage_notes,
    }
}

pub(super) fn api_coverage_from_state(
    value: codex_state::WorkspaceCoverage,
    readiness: codex_state::WorkspaceBillingReadiness,
) -> Result<WorkspaceCoverage, JSONRPCErrorError> {
    let version = value
        .record_version()
        .map_err(|err| internal_error(format!("failed to version workspace coverage: {err}")))?;
    Ok(WorkspaceCoverage {
        id: value.id,
        version,
        client_id: value.client_id,
        priority: value.priority,
        payer_name: value.payer_name,
        plan_name: value.plan_name,
        member_id: value.member_id,
        group_number: value.group_number,
        coverage_type: value.coverage_type,
        coverage_status: value.coverage_status,
        effective_date: value.effective_date,
        termination_date: value.termination_date,
        patient_relationship_to_subscriber: value.patient_relationship_to_subscriber,
        subscriber_first_name: value.subscriber_first_name,
        subscriber_middle_name: value.subscriber_middle_name,
        subscriber_last_name: value.subscriber_last_name,
        subscriber_suffix: value.subscriber_suffix,
        subscriber_date_of_birth: value.subscriber_date_of_birth,
        subscriber_administrative_sex: value.subscriber_administrative_sex,
        subscriber_address_same_as_patient: value.subscriber_address_same_as_patient,
        subscriber_address_line_1: value.subscriber_address_line_1,
        subscriber_address_line_2: value.subscriber_address_line_2,
        subscriber_city: value.subscriber_city,
        subscriber_state_or_province: value.subscriber_state_or_province,
        subscriber_postal_code: value.subscriber_postal_code,
        subscriber_country: value.subscriber_country,
        coverage_notes: value.coverage_notes,
        billing_readiness: api_readiness(readiness),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_verification_from_state(
    value: codex_state::WorkspaceCoverageVerification,
) -> WorkspaceCoverageVerification {
    WorkspaceCoverageVerification {
        id: value.id,
        coverage_id: value.coverage_id,
        client_id: value.client_id,
        source_document_id: value.source_document_id,
        source_document_version: value.source_document_version,
        source_document_content_sha256: value.source_document_content_sha256,
        compared_subject: match value.compared_subject {
            codex_state::WorkspaceCoverageVerificationSubject::Beneficiary => {
                WorkspaceCoverageVerificationSubject::Beneficiary
            }
            codex_state::WorkspaceCoverageVerificationSubject::Subscriber => {
                WorkspaceCoverageVerificationSubject::Subscriber
            }
        },
        observed_first_name: value.observed_first_name,
        observed_middle_name: value.observed_middle_name,
        observed_last_name: value.observed_last_name,
        observed_suffix: value.observed_suffix,
        observed_member_id: value.observed_member_id,
        patient_record_version: value.patient_record_version,
        patient_version: value.patient_version,
        coverage_version: value.coverage_version,
        match_result: match value.match_result {
            codex_state::WorkspaceCoverageMatchResult::Match => WorkspaceCoverageMatchResult::Match,
            codex_state::WorkspaceCoverageMatchResult::Mismatch => {
                WorkspaceCoverageMatchResult::Mismatch
            }
        },
        mismatch_fields: value.mismatch_fields,
        actor: value.actor,
        content_sha256: value.content_sha256,
        is_stale: value.is_stale,
        created_at: value.created_at.timestamp(),
    }
}

fn state_subject(
    value: WorkspaceCoverageVerificationSubject,
) -> codex_state::WorkspaceCoverageVerificationSubject {
    match value {
        WorkspaceCoverageVerificationSubject::Beneficiary => {
            codex_state::WorkspaceCoverageVerificationSubject::Beneficiary
        }
        WorkspaceCoverageVerificationSubject::Subscriber => {
            codex_state::WorkspaceCoverageVerificationSubject::Subscriber
        }
    }
}

fn api_readiness(value: codex_state::WorkspaceBillingReadiness) -> WorkspaceBillingReadiness {
    match value {
        codex_state::WorkspaceBillingReadiness::Match => WorkspaceBillingReadiness::Match,
        codex_state::WorkspaceBillingReadiness::Mismatch => WorkspaceBillingReadiness::Mismatch,
        codex_state::WorkspaceBillingReadiness::Unverified => WorkspaceBillingReadiness::Unverified,
        codex_state::WorkspaceBillingReadiness::Stale => WorkspaceBillingReadiness::Stale,
        codex_state::WorkspaceBillingReadiness::Incomplete => WorkspaceBillingReadiness::Incomplete,
    }
}
