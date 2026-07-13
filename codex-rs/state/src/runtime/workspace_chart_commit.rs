use super::WorkspaceStore;
use super::workspace_chart_commit_compare as compare;
use super::workspace_chart_commit_ops::*;
use super::workspace_chart_commit_sql as chart_sql;
use super::workspace_chart_commit_validate as validate;
use super::workspace_chart_commit_versions;
use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartCommitRequest;
use crate::WorkspaceChartCommitResult;
use crate::WorkspaceChartEntityKind;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Row;
use uuid::Uuid;

const LEGACY_CHART_COMMIT_SCHEMA_VERSION: i64 = 1;
const CHART_COMMIT_SCHEMA_VERSION: i64 = 2;
const LEGACY_CHART_COMMIT_HASH_PREFIX: &[u8] = b"workspace-chart-commit:v1\0";
const CHART_COMMIT_HASH_PREFIX: &[u8] = b"workspace-chart-commit:v2\0";

#[derive(Serialize)]
struct LegacyChartCommitRequestV1<'a> {
    idempotency_key: &'a str,
    actor: &'a str,
    reason: &'a str,
    source_thread_id: &'a Option<String>,
    source_turn_id: &'a Option<String>,
    client_id: &'a Option<String>,
    client: Option<LegacyClientUpsertV1<'a>>,
    expected_versions: LegacyExpectedVersionsV1<'a>,
    safety_item: &'a Option<crate::WorkspacePatientSafetyItemUpsert>,
    encounter: &'a Option<crate::WorkspaceEncounterUpsert>,
    note: &'a Option<crate::WorkspaceChartNoteChange>,
    document: &'a Option<crate::WorkspaceDocumentUpsert>,
    artifact_derivative: &'a Option<crate::WorkspaceArtifactDerivativeUpsert>,
    context_clip: &'a Option<crate::WorkspaceContextClipUpsert>,
    task: &'a Option<crate::WorkspaceTaskUpsert>,
}

#[derive(Serialize)]
struct LegacyExpectedVersionsV1<'a> {
    client: &'a Option<String>,
    safety_item: &'a Option<String>,
    encounter: &'a Option<String>,
    document: &'a Option<String>,
    artifact_derivative: &'a Option<String>,
    context_clip: &'a Option<String>,
    task: &'a Option<String>,
}

#[derive(Serialize)]
struct LegacyClientUpsertV1<'a> {
    id: &'a Option<String>,
    display_name: &'a str,
    preferred_name: &'a Option<String>,
    date_of_birth: &'a Option<String>,
    sex_or_gender: &'a Option<String>,
    external_id: &'a Option<String>,
    record_start_date: &'a Option<String>,
    record_end_date: &'a Option<String>,
    summary: &'a str,
    primary_phone: &'a Option<String>,
    secondary_phone: &'a Option<String>,
    email: &'a Option<String>,
    preferred_contact_method: &'a Option<String>,
    emergency_contact_name: &'a Option<String>,
    emergency_contact_relationship: &'a Option<String>,
    emergency_contact_phone: &'a Option<String>,
    emergency_contact_email: &'a Option<String>,
    contact_notes: &'a Option<String>,
    payer_name: &'a Option<String>,
    plan_name: &'a Option<String>,
    member_id: &'a Option<String>,
    group_number: &'a Option<String>,
    coverage_type: &'a Option<String>,
    coverage_status: &'a Option<String>,
    coverage_notes: &'a Option<String>,
}

pub(super) struct ExistingRecords {
    pub(super) client: Option<crate::WorkspaceClient>,
    pub(super) coverage: Option<crate::WorkspaceCoverage>,
    pub(super) safety_item: Option<crate::WorkspacePatientSafetyItem>,
    pub(super) encounter: Option<crate::WorkspaceEncounter>,
    pub(super) note: Option<crate::WorkspaceNote>,
    pub(super) document: Option<crate::WorkspaceDocument>,
    pub(super) derivative: Option<crate::WorkspaceArtifactDerivative>,
    pub(super) clip: Option<crate::WorkspaceContextClip>,
    pub(super) task: Option<crate::WorkspaceTask>,
}

impl WorkspaceStore {
    /// Atomically applies one patient-rooted chart changeset.
    ///
    /// Missing child IDs are allocated before writes. Blank links on included
    /// children resolve to the corresponding record in the same commit. A
    /// successful no-op stores only its idempotency receipt.
    pub async fn commit_chart(
        &self,
        mut request: WorkspaceChartCommitRequest,
    ) -> Result<WorkspaceChartCommitResult, WorkspaceChartCommitError> {
        validate::normalize_before_hash(&mut request)?;
        let request_json = serde_json::to_string(&request)?;
        let request_sha256 = chart_commit_hash(CHART_COMMIT_HASH_PREFIX, &request_json);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        if let Some(row) = sqlx::query(
            r#"
SELECT schema_version, request_sha256, request_json, result_json
FROM workspace_chart_commits
WHERE idempotency_key = ?
            "#,
        )
        .bind(&request.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            let schema_version: i64 = row.try_get("schema_version")?;
            let stored_hash: String = row.try_get("request_sha256")?;
            let stored_request_json: String = row.try_get("request_json")?;
            let matches = match schema_version {
                LEGACY_CHART_COMMIT_SCHEMA_VERSION => {
                    let legacy_json = legacy_request_json(&request)?;
                    let pre_upgrade_matches = legacy_json.as_ref().is_some_and(|legacy_json| {
                        stored_hash
                            == chart_commit_hash(LEGACY_CHART_COMMIT_HASH_PREFIX, legacy_json)
                            && stored_request_json == *legacy_json
                    });
                    let transitional_matches = stored_hash
                        == chart_commit_hash(LEGACY_CHART_COMMIT_HASH_PREFIX, &request_json)
                        && stored_request_json == request_json;
                    pre_upgrade_matches || transitional_matches
                }
                CHART_COMMIT_SCHEMA_VERSION => {
                    stored_hash == request_sha256 && stored_request_json == request_json
                }
                _ => {
                    return Err(WorkspaceChartCommitError::Storage {
                        message: format!(
                            "unsupported workspace chart commit receipt schema version {schema_version}"
                        ),
                    });
                }
            };
            if !matches {
                return Err(WorkspaceChartCommitError::IdempotencyConflict {
                    idempotency_key: request.idempotency_key.clone(),
                });
            }
            let result_json: String = row.try_get("result_json")?;
            let mut result: WorkspaceChartCommitResult = serde_json::from_str(&result_json)?;
            if schema_version == LEGACY_CHART_COMMIT_SCHEMA_VERSION
                && result.client.primary_email.is_none()
            {
                result.client.primary_email.clone_from(&result.client.email);
            }
            result.replayed = true;
            tx.rollback().await?;
            return Ok(result);
        }

        let (client_id, existing_client) = match request.client_id.as_deref() {
            Some(client_id) => {
                let existing = chart_sql::client(&mut tx, client_id).await?;
                if existing.is_none() {
                    return validation(format!("workspace client `{client_id}` was not found"));
                }
                (client_id.to_string(), existing)
            }
            None => {
                let client_id = loop {
                    let candidate = Uuid::new_v4().to_string();
                    if chart_sql::client(&mut tx, &candidate).await?.is_none() {
                        break candidate;
                    }
                };
                (client_id, None)
            }
        };
        if existing_client
            .as_ref()
            .is_some_and(|client| client.archived_at.is_some())
        {
            return validation(format!(
                "workspace client `{client_id}` is archived and cannot be committed"
            ));
        }
        let root_exists = existing_client.is_some();
        validate::allocate_and_bind(&mut request, &client_id, root_exists)?;

        let existing = fetch_existing(&mut tx, &request, existing_client).await?;
        validate_existing_ownership(&existing, &client_id)?;
        compare::preserve_existing_timestamp_precision(&mut request, &existing);
        validate_note_revision(&request, existing.note.as_ref())?;
        validate_relations(&mut tx, &request, &client_id).await?;
        workspace_chart_commit_versions::validate(&request, &existing, &client_id)?;

        let mut changed = Vec::new();
        if request.client.as_ref().is_some_and(|input| {
            existing
                .client
                .as_ref()
                .is_none_or(|value| !compare::client(value, input))
        }) {
            changed.push(WorkspaceChartEntityKind::Client);
        }
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::Coverage,
            request.coverage.as_ref(),
            existing.coverage.as_ref(),
            compare::coverage,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::SafetyItem,
            request.safety_item.as_ref(),
            existing.safety_item.as_ref(),
            compare::safety_item,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::Encounter,
            request.encounter.as_ref(),
            existing.encounter.as_ref(),
            compare::encounter,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::Note,
            request.note.as_ref().map(|change| &change.upsert),
            existing.note.as_ref(),
            compare::note,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::Document,
            request.document.as_ref(),
            existing.document.as_ref(),
            compare::document,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::ArtifactDerivative,
            request.artifact_derivative.as_ref(),
            existing.derivative.as_ref(),
            compare::derivative,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::ContextClip,
            request.context_clip.as_ref(),
            existing.clip.as_ref(),
            compare::clip,
        );
        push_if_changed(
            &mut changed,
            WorkspaceChartEntityKind::Task,
            request.task.as_ref(),
            existing.task.as_ref(),
            compare::task,
        );

        let commit_id = Uuid::new_v4().to_string();
        let committed_at = Utc::now();
        let now_ms = datetime_to_epoch_millis(committed_at);
        apply_changes(
            &mut tx, &request, &existing, &changed, &commit_id, &client_id, now_ms,
        )
        .await?;

        let client = required(chart_sql::client(&mut tx, &client_id).await?, "client")?;
        let coverage = fetch_requested_coverage(&mut tx, &request).await?;
        let coverage_billing_readiness = match coverage.as_ref() {
            Some(coverage) => Some(
                super::workspace_coverage::coverage_billing_readiness_in_tx(
                    &mut tx, &client, coverage,
                )
                .await?,
            ),
            None => None,
        };
        let safety_item = fetch_requested_safety(&mut tx, &request).await?;
        let encounter = fetch_requested_encounter(&mut tx, &request).await?;
        let note = fetch_requested_note(&mut tx, &request).await?;
        let document = fetch_requested_document(&mut tx, &request).await?;
        let artifact_derivative = fetch_requested_derivative(&mut tx, &request).await?;
        let context_clip = fetch_requested_clip(&mut tx, &request).await?;
        let task = fetch_requested_task(&mut tx, &request).await?;
        let result = WorkspaceChartCommitResult {
            commit_id: commit_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            replayed: false,
            changed_entity_kinds: changed.clone(),
            client,
            coverage,
            coverage_billing_readiness,
            safety_item,
            encounter,
            resulting_note_revision: note.as_ref().map(|value| value.current_revision),
            note,
            document,
            artifact_derivative,
            context_clip,
            task,
            committed_at,
        };
        let changed_json = serde_json::to_string(&changed)?;
        let result_json = serde_json::to_string(&result)?;
        sqlx::query(
            r#"
INSERT INTO workspace_chart_commits (
    id, idempotency_key, schema_version, request_sha256, request_json,
    client_id, actor, reason, changed_entity_kinds_json, result_json, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&commit_id)
        .bind(&request.idempotency_key)
        .bind(CHART_COMMIT_SCHEMA_VERSION)
        .bind(&request_sha256)
        .bind(&request_json)
        .bind(&client_id)
        .bind(&request.actor)
        .bind(&request.reason)
        .bind(changed_json)
        .bind(result_json)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result)
    }
}

fn chart_commit_hash(prefix: &[u8], request_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(request_json.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn legacy_request_json(
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<String>, WorkspaceChartCommitError> {
    if request.coverage.is_some() || request.expected_versions.coverage.is_some() {
        return Ok(None);
    }
    let client = match request.client.as_ref() {
        Some(client) if legacy_client_compatible(client) => Some(LegacyClientUpsertV1 {
            id: &client.id,
            display_name: &client.display_name,
            preferred_name: &client.preferred_name,
            date_of_birth: &client.date_of_birth,
            sex_or_gender: &client.sex_or_gender,
            external_id: &client.external_id,
            record_start_date: &client.record_start_date,
            record_end_date: &client.record_end_date,
            summary: &client.summary,
            primary_phone: &client.primary_phone,
            secondary_phone: &client.secondary_phone,
            email: &client.email,
            preferred_contact_method: &client.preferred_contact_method,
            emergency_contact_name: &client.emergency_contact_name,
            emergency_contact_relationship: &client.emergency_contact_relationship,
            emergency_contact_phone: &client.emergency_contact_phone,
            emergency_contact_email: &client.emergency_contact_email,
            contact_notes: &client.contact_notes,
            payer_name: &client.payer_name,
            plan_name: &client.plan_name,
            member_id: &client.member_id,
            group_number: &client.group_number,
            coverage_type: &client.coverage_type,
            coverage_status: &client.coverage_status,
            coverage_notes: &client.coverage_notes,
        }),
        Some(_) => return Ok(None),
        None => None,
    };
    Ok(Some(serde_json::to_string(&LegacyChartCommitRequestV1 {
        idempotency_key: &request.idempotency_key,
        actor: &request.actor,
        reason: &request.reason,
        source_thread_id: &request.source_thread_id,
        source_turn_id: &request.source_turn_id,
        client_id: &request.client_id,
        client,
        expected_versions: LegacyExpectedVersionsV1 {
            client: &request.expected_versions.client,
            safety_item: &request.expected_versions.safety_item,
            encounter: &request.expected_versions.encounter,
            document: &request.expected_versions.document,
            artifact_derivative: &request.expected_versions.artifact_derivative,
            context_clip: &request.expected_versions.context_clip,
            task: &request.expected_versions.task,
        },
        safety_item: &request.safety_item,
        encounter: &request.encounter,
        note: &request.note,
        document: &request.document,
        artifact_derivative: &request.artifact_derivative,
        context_clip: &request.context_clip,
        task: &request.task,
    })?))
}

fn legacy_client_compatible(client: &crate::WorkspaceClientUpsert) -> bool {
    client.legal_first_name.is_none()
        && client.legal_middle_name.is_none()
        && client.legal_last_name.is_none()
        && client.legal_suffix.is_none()
        && client.previous_name.is_none()
        && client.administrative_sex.is_none()
        && client.preferred_language.is_none()
        && !client.interpreter_required
        && client.primary_phone_use.is_none()
        && client.secondary_phone_use.is_none()
        && client
            .primary_email
            .as_ref()
            .is_none_or(|primary| Some(primary) == client.email.as_ref())
        && client.secondary_email.is_none()
        && client.address_line_1.is_none()
        && client.address_line_2.is_none()
        && client.city.is_none()
        && client.state_or_province.is_none()
        && client.postal_code.is_none()
        && client.country.is_none()
        && client.address_use.is_none()
}

#[cfg(test)]
#[path = "workspace_chart_commit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_chart_commit_adversarial_tests.rs"]
mod adversarial_tests;
