use super::WorkspaceStore;
use super::workspace::insert_audit_event;
use super::workspace_coverage_identity::compare_card_identity;
use super::workspace_coverage_identity::coverage_incomplete;
use super::workspace_coverage_identity::coverage_is_medicare;
use super::workspace_coverage_identity::normalize_mbi;
use super::workspace_coverage_identity::patient_identity_version;
use super::workspace_coverage_identity::valid_mbi;
use crate::WorkspaceAuditEventCreate;
use crate::WorkspaceBillingReadiness;
use crate::WorkspaceCoverage;
use crate::WorkspaceCoverageMatchResult;
use crate::WorkspaceCoverageUpsert;
use crate::WorkspaceCoverageVerification;
use crate::WorkspaceCoverageVerificationCreate;
use crate::WorkspaceCoverageVerificationCreateResult;
use crate::WorkspaceCoverageVerificationSubject;
use crate::model::datetime_to_epoch_millis;
use crate::model::epoch_millis_to_datetime;
use chrono::Utc;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::Transaction;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

const COVERAGE_COLUMNS: &str = r#"
id, client_id, priority, payer_name, plan_name, member_id, group_number,
coverage_type, coverage_status, effective_date, termination_date,
patient_relationship_to_subscriber, subscriber_first_name,
subscriber_middle_name, subscriber_last_name, subscriber_suffix,
subscriber_date_of_birth, subscriber_administrative_sex,
subscriber_address_same_as_patient, subscriber_address_line_1,
subscriber_address_line_2, subscriber_city, subscriber_state_or_province,
subscriber_postal_code, subscriber_country, coverage_notes, created_at_ms,
source_kind, updated_at_ms
"#;

impl WorkspaceStore {
    pub async fn list_coverages(
        &self,
        client_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<WorkspaceCoverage>> {
        let mut tx = self.pool.begin().await?;
        let coverages = list_coverages_in_tx(&mut tx, client_id, cursor, limit).await?;
        tx.commit().await?;
        Ok(coverages)
    }

    pub async fn list_coverages_with_billing_readiness(
        &self,
        client_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<(WorkspaceCoverage, WorkspaceBillingReadiness)>> {
        let mut tx = self.pool.begin().await?;
        let coverages = list_coverages_in_tx(&mut tx, client_id, cursor, limit).await?;
        if coverages.is_empty() {
            tx.commit().await?;
            return Ok(Vec::new());
        }
        let client = super::workspace_chart_commit_sql::client(&mut tx, client_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace coverage patient was not found"))?;
        let mut snapshots = Vec::with_capacity(coverages.len());
        for coverage in coverages {
            let readiness = coverage_billing_readiness_in_tx(&mut tx, &client, &coverage).await?;
            snapshots.push((coverage, readiness));
        }
        tx.commit().await?;
        Ok(snapshots)
    }

    pub async fn get_coverage(&self, id: &str) -> anyhow::Result<Option<WorkspaceCoverage>> {
        let query = format!("SELECT {COVERAGE_COLUMNS} FROM workspace_coverages WHERE id = ?");
        sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(id)
            .fetch_optional(self.pool.as_ref())
            .await?
            .as_ref()
            .map(coverage_from_row)
            .transpose()
    }

    pub async fn list_coverage_verifications(
        &self,
        coverage_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> anyhow::Result<Vec<WorkspaceCoverageVerification>> {
        let mut tx = self.pool.begin().await?;
        let verifications =
            list_coverage_verifications_in_tx(&mut tx, coverage_id, cursor, limit).await?;
        tx.commit().await?;
        Ok(verifications)
    }

    pub async fn create_coverage_verification(
        &self,
        mut input: WorkspaceCoverageVerificationCreate,
    ) -> anyhow::Result<WorkspaceCoverageVerificationCreateResult> {
        normalize_optional(&mut input.observed_first_name);
        normalize_optional(&mut input.observed_middle_name);
        normalize_optional(&mut input.observed_last_name);
        normalize_optional(&mut input.observed_suffix);
        normalize_optional(&mut input.observed_member_id);
        input.expected_patient_version = input.expected_patient_version.trim().to_string();
        input.expected_coverage_version = input.expected_coverage_version.trim().to_string();
        input.expected_document_version = input.expected_document_version.trim().to_string();
        input.actor = input.actor.trim().to_string();
        if input.expected_patient_version.is_empty()
            || input.expected_coverage_version.is_empty()
            || input.expected_document_version.is_empty()
        {
            anyhow::bail!(
                "workspace coverage verification expected patient, coverage, and document versions must not be empty"
            );
        }
        if input.actor.is_empty() {
            anyhow::bail!("workspace coverage verification actor must not be empty");
        }

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let coverage = coverage_in_tx(&mut tx, &input.coverage_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace coverage was not found"))?;
        let client = super::workspace_chart_commit_sql::client(&mut tx, &coverage.client_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace coverage patient was not found"))?;
        let document =
            super::workspace_chart_commit_sql::document(&mut tx, &input.source_document_id)
                .await?
                .filter(|document| {
                    document.client_id == coverage.client_id && document.archived_at.is_none()
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "workspace insurance-card document was not found for this patient"
                    )
                })?;
        if !eligible_card_document(&document) {
            anyhow::bail!(
                "workspace insurance-card document must be a present patient-scoped local reference with insurance-card kind and a SHA-256 content snapshot"
            );
        }
        let source_document_content_sha256 = document_content_sha256(&document)
            .ok_or_else(|| anyhow::anyhow!("workspace insurance-card content hash is invalid"))?;
        let patient_record_version = client.record_version()?;
        let patient_version = patient_identity_version(&client)?;
        let coverage_version = coverage.record_version()?;
        let source_document_version = document.record_version()?;
        if input.expected_patient_version != patient_record_version {
            anyhow::bail!("workspace coverage verification patient changed after form open");
        }
        if input.expected_coverage_version != coverage_version {
            anyhow::bail!("workspace coverage verification coverage changed after form open");
        }
        if input.expected_document_version != source_document_version {
            anyhow::bail!("workspace coverage verification card document changed after form open");
        }
        let medicare = coverage_is_medicare(&coverage);
        if medicare && input.compared_subject != WorkspaceCoverageVerificationSubject::Beneficiary {
            anyhow::bail!("Medicare card identity must be compared with the beneficiary");
        }
        if !medicare
            && coverage
                .patient_relationship_to_subscriber
                .as_deref()
                .is_some_and(|relationship| relationship.eq_ignore_ascii_case("self"))
            && input.compared_subject != WorkspaceCoverageVerificationSubject::Beneficiary
        {
            anyhow::bail!("self coverage card identity must be compared with the beneficiary");
        }
        if !medicare
            && coverage
                .patient_relationship_to_subscriber
                .as_deref()
                .is_some_and(|relationship| !relationship.eq_ignore_ascii_case("self"))
            && input.compared_subject != WorkspaceCoverageVerificationSubject::Subscriber
        {
            anyhow::bail!("dependent coverage card identity must be compared with the subscriber");
        }
        if medicare {
            input.observed_member_id = input.observed_member_id.map(normalize_mbi);
            if input
                .observed_member_id
                .as_deref()
                .is_none_or(|value| !valid_mbi(value))
            {
                anyhow::bail!(
                    "Medicare Beneficiary Identifier must use the 11-character CMS format"
                );
            }
        }

        let mismatch_fields = compare_card_identity(&client, &coverage, &input, medicare);
        let match_result = if mismatch_fields.is_empty() {
            WorkspaceCoverageMatchResult::Match
        } else {
            WorkspaceCoverageMatchResult::Mismatch
        };
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let billing_readiness = if coverage_incomplete(&client, &coverage) {
            WorkspaceBillingReadiness::Incomplete
        } else if match_result == WorkspaceCoverageMatchResult::Mismatch {
            WorkspaceBillingReadiness::Mismatch
        } else {
            WorkspaceBillingReadiness::Match
        };
        let canonical = serde_json::to_vec(&json!({
            "coverageId": input.coverage_id,
            "documentId": input.source_document_id,
            "documentVersion": source_document_version,
            "documentContentSha256": source_document_content_sha256,
            "subject": input.compared_subject,
            "firstName": input.observed_first_name,
            "middleName": input.observed_middle_name,
            "lastName": input.observed_last_name,
            "suffix": input.observed_suffix,
            "memberId": input.observed_member_id,
            "patientRecordVersion": patient_record_version,
            "patientVersion": patient_version,
            "coverageVersion": coverage_version,
            "result": match_result,
            "mismatchFields": mismatch_fields,
            "actor": input.actor,
            "createdAtMs": now_ms,
        }))?;
        let mut hasher = Sha256::new();
        hasher.update(b"workspace-coverage-card-verification:v2\0");
        hasher.update(canonical);
        let content_sha256 = format!("{:x}", hasher.finalize());
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
INSERT INTO workspace_coverage_card_verifications (
    id, coverage_id, client_id, source_document_id, source_document_version,
    source_document_content_sha256, compared_subject,
    observed_first_name, observed_middle_name, observed_last_name,
    observed_suffix, observed_member_id, patient_record_version, patient_version,
    coverage_version, match_result, mismatch_fields_json, actor, content_sha256,
    created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&coverage.id)
        .bind(&coverage.client_id)
        .bind(&input.source_document_id)
        .bind(&source_document_version)
        .bind(&source_document_content_sha256)
        .bind(subject_str(input.compared_subject))
        .bind(&input.observed_first_name)
        .bind(&input.observed_middle_name)
        .bind(&input.observed_last_name)
        .bind(&input.observed_suffix)
        .bind(&input.observed_member_id)
        .bind(&patient_record_version)
        .bind(&patient_version)
        .bind(&coverage_version)
        .bind(match_result_str(match_result))
        .bind(serde_json::to_string(&mismatch_fields)?)
        .bind(&input.actor)
        .bind(&content_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            WorkspaceAuditEventCreate {
                entity_type: "coverage_verification".to_string(),
                entity_id: id.clone(),
                action: "created".to_string(),
                actor: input.actor.clone(),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(coverage.client_id.clone()),
                document_id: Some(input.source_document_id.clone()),
                success: true,
                summary: format!("insurance card identity {}", match_result_str(match_result)),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        Ok(WorkspaceCoverageVerificationCreateResult {
            verification: WorkspaceCoverageVerification {
                id,
                coverage_id: coverage.id,
                client_id: coverage.client_id,
                source_document_id: input.source_document_id,
                source_document_version,
                source_document_content_sha256,
                compared_subject: input.compared_subject,
                observed_first_name: input.observed_first_name,
                observed_middle_name: input.observed_middle_name,
                observed_last_name: input.observed_last_name,
                observed_suffix: input.observed_suffix,
                observed_member_id: input.observed_member_id,
                patient_record_version,
                patient_version,
                coverage_version,
                match_result,
                mismatch_fields,
                actor: input.actor,
                content_sha256,
                is_stale: false,
                created_at: epoch_millis_to_datetime(now_ms)?,
            },
            billing_readiness,
        })
    }

    pub async fn coverage_billing_readiness(
        &self,
        coverage: &WorkspaceCoverage,
    ) -> anyhow::Result<WorkspaceBillingReadiness> {
        let mut tx = self.pool.begin().await?;
        let current = coverage_in_tx(&mut tx, &coverage.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace coverage was not found"))?;
        let readiness = if current.record_version()? != coverage.record_version()? {
            WorkspaceBillingReadiness::Stale
        } else {
            let client = super::workspace_chart_commit_sql::client(&mut tx, &coverage.client_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("workspace coverage patient was not found"))?;
            coverage_billing_readiness_in_tx(&mut tx, &client, &current).await?
        };
        tx.commit().await?;
        Ok(readiness)
    }
}

async fn list_coverages_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    client_id: &str,
    cursor: Option<&str>,
    limit: u32,
) -> anyhow::Result<Vec<WorkspaceCoverage>> {
    let after_priority = match cursor {
        Some(cursor) => Some(
            sqlx::query_scalar::<_, i64>(
                "SELECT priority FROM workspace_coverages WHERE id = ? AND client_id = ?",
            )
            .bind(cursor)
            .bind(client_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace coverage cursor was not found"))?,
        ),
        None => None,
    };
    let query = format!(
        "SELECT {COVERAGE_COLUMNS} FROM workspace_coverages \
         WHERE client_id = ? AND priority > ? ORDER BY priority ASC LIMIT ?"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(client_id)
        .bind(after_priority.unwrap_or(0))
        .bind(i64::from(limit.clamp(1, 101)))
        .fetch_all(&mut **tx)
        .await?;
    rows.iter().map(coverage_from_row).collect()
}

async fn list_coverage_verifications_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    coverage_id: &str,
    cursor: Option<&str>,
    limit: u32,
) -> anyhow::Result<Vec<WorkspaceCoverageVerification>> {
    let coverage = coverage_in_tx(tx, coverage_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace coverage was not found"))?;
    let client = super::workspace_chart_commit_sql::client(tx, &coverage.client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace coverage patient was not found"))?;
    let patient_version = patient_identity_version(&client)?;
    let coverage_version = coverage.record_version()?;
    let cursor_created_at = match cursor {
        Some(cursor) => Some(
            sqlx::query_scalar::<_, i64>(
                "SELECT created_at_ms FROM workspace_coverage_card_verifications WHERE id = ? AND coverage_id = ?",
            )
            .bind(cursor)
            .bind(coverage_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace verification cursor was not found"))?,
        ),
        None => None,
    };
    let rows = sqlx::query(
        r#"
SELECT id, coverage_id, client_id, source_document_id, compared_subject,
       source_document_version, source_document_content_sha256,
       observed_first_name, observed_middle_name, observed_last_name,
       observed_suffix, observed_member_id, patient_record_version,
       patient_version, coverage_version, match_result, mismatch_fields_json,
       actor, content_sha256, created_at_ms
FROM workspace_coverage_card_verifications
WHERE coverage_id = ?
  AND (
      ? IS NULL
      OR created_at_ms < ?
      OR (created_at_ms = ? AND id < ?)
  )
ORDER BY created_at_ms DESC, id DESC
LIMIT ?
        "#,
    )
    .bind(coverage_id)
    .bind(cursor_created_at)
    .bind(cursor_created_at)
    .bind(cursor_created_at)
    .bind(cursor)
    .bind(i64::from(limit.clamp(1, 101)))
    .fetch_all(&mut **tx)
    .await?;
    rows.iter()
        .map(|row| verification_from_row(row, &patient_version, &coverage_version))
        .collect()
}

pub(super) async fn coverage_billing_readiness_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    client: &crate::WorkspaceClient,
    coverage: &WorkspaceCoverage,
) -> anyhow::Result<WorkspaceBillingReadiness> {
    if coverage_incomplete(client, coverage) {
        return Ok(WorkspaceBillingReadiness::Incomplete);
    }
    let latest = sqlx::query(
        r#"
SELECT patient_version, coverage_version, match_result
FROM workspace_coverage_card_verifications
WHERE coverage_id = ?
ORDER BY created_at_ms DESC, id DESC
LIMIT 1
        "#,
    )
    .bind(&coverage.id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(latest) = latest else {
        return Ok(WorkspaceBillingReadiness::Unverified);
    };
    let patient_version: String = latest.try_get("patient_version")?;
    let coverage_version: String = latest.try_get("coverage_version")?;
    if patient_version != patient_identity_version(client)?
        || coverage_version != coverage.record_version()?
    {
        return Ok(WorkspaceBillingReadiness::Stale);
    }
    let match_result: String = latest.try_get("match_result")?;
    if parse_match_result(&match_result)? == WorkspaceCoverageMatchResult::Mismatch {
        Ok(WorkspaceBillingReadiness::Mismatch)
    } else {
        Ok(WorkspaceBillingReadiness::Match)
    }
}

pub(super) async fn coverage_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceCoverage>> {
    let query = format!("SELECT {COVERAGE_COLUMNS} FROM workspace_coverages WHERE id = ?");
    sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .as_ref()
        .map(coverage_from_row)
        .transpose()
}

pub(super) async fn put_coverage(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &WorkspaceCoverageUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_coverages (
    id, client_id, priority, payer_name, plan_name, member_id, group_number,
    coverage_type, coverage_status, effective_date, termination_date,
    patient_relationship_to_subscriber, subscriber_first_name,
    subscriber_middle_name, subscriber_last_name, subscriber_suffix,
    subscriber_date_of_birth, subscriber_administrative_sex,
    subscriber_address_same_as_patient, subscriber_address_line_1,
    subscriber_address_line_2, subscriber_city, subscriber_state_or_province,
    subscriber_postal_code, subscriber_country, coverage_notes, source_kind,
    created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'structured', ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id, priority = excluded.priority,
    payer_name = excluded.payer_name, plan_name = excluded.plan_name,
    member_id = excluded.member_id, group_number = excluded.group_number,
    coverage_type = excluded.coverage_type, coverage_status = excluded.coverage_status,
    effective_date = excluded.effective_date, termination_date = excluded.termination_date,
    patient_relationship_to_subscriber = excluded.patient_relationship_to_subscriber,
    subscriber_first_name = excluded.subscriber_first_name,
    subscriber_middle_name = excluded.subscriber_middle_name,
    subscriber_last_name = excluded.subscriber_last_name,
    subscriber_suffix = excluded.subscriber_suffix,
    subscriber_date_of_birth = excluded.subscriber_date_of_birth,
    subscriber_administrative_sex = excluded.subscriber_administrative_sex,
    subscriber_address_same_as_patient = excluded.subscriber_address_same_as_patient,
    subscriber_address_line_1 = excluded.subscriber_address_line_1,
    subscriber_address_line_2 = excluded.subscriber_address_line_2,
    subscriber_city = excluded.subscriber_city,
    subscriber_state_or_province = excluded.subscriber_state_or_province,
    subscriber_postal_code = excluded.subscriber_postal_code,
    subscriber_country = excluded.subscriber_country,
    coverage_notes = excluded.coverage_notes, source_kind = 'structured',
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(input.priority)
    .bind(&input.payer_name)
    .bind(&input.plan_name)
    .bind(&input.member_id)
    .bind(&input.group_number)
    .bind(&input.coverage_type)
    .bind(&input.coverage_status)
    .bind(&input.effective_date)
    .bind(&input.termination_date)
    .bind(&input.patient_relationship_to_subscriber)
    .bind(&input.subscriber_first_name)
    .bind(&input.subscriber_middle_name)
    .bind(&input.subscriber_last_name)
    .bind(&input.subscriber_suffix)
    .bind(&input.subscriber_date_of_birth)
    .bind(&input.subscriber_administrative_sex)
    .bind(input.subscriber_address_same_as_patient)
    .bind(&input.subscriber_address_line_1)
    .bind(&input.subscriber_address_line_2)
    .bind(&input.subscriber_city)
    .bind(&input.subscriber_state_or_province)
    .bind(&input.subscriber_postal_code)
    .bind(&input.subscriber_country)
    .bind(&input.coverage_notes)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    if input.priority == 1 {
        sqlx::query(
            r#"
INSERT INTO workspace_client_coverages (
    client_id, payer_name, plan_name, member_id, group_number, coverage_type,
    coverage_status, coverage_notes, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO UPDATE SET
    payer_name = excluded.payer_name,
    plan_name = excluded.plan_name,
    member_id = excluded.member_id,
    group_number = excluded.group_number,
    coverage_type = excluded.coverage_type,
    coverage_status = excluded.coverage_status,
    coverage_notes = excluded.coverage_notes,
    updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(&input.client_id)
        .bind(&input.payer_name)
        .bind(&input.plan_name)
        .bind(&input.member_id)
        .bind(&input.group_number)
        .bind(&input.coverage_type)
        .bind(&input.coverage_status)
        .bind(&input.coverage_notes)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(super) async fn put_legacy_primary_coverage_if_present(
    tx: &mut Transaction<'_, Sqlite>,
    client_id: &str,
    input: &crate::WorkspaceClientUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    let values = [
        input.payer_name.as_deref(),
        input.plan_name.as_deref(),
        input.member_id.as_deref(),
        input.group_number.as_deref(),
        input.coverage_type.as_deref(),
        input.coverage_status.as_deref(),
        input.coverage_notes.as_deref(),
    ];
    if values
        .into_iter()
        .flatten()
        .all(|value| value.trim().is_empty())
    {
        return Ok(());
    }
    sqlx::query(
        r#"
INSERT INTO workspace_client_coverages (
    client_id, payer_name, plan_name, member_id, group_number, coverage_type,
    coverage_status, coverage_notes, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO NOTHING
        "#,
    )
    .bind(client_id)
    .bind(&input.payer_name)
    .bind(&input.plan_name)
    .bind(&input.member_id)
    .bind(&input.group_number)
    .bind(&input.coverage_type)
    .bind(&input.coverage_status)
    .bind(&input.coverage_notes)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn coverage_from_row(row: &SqliteRow) -> anyhow::Result<WorkspaceCoverage> {
    Ok(WorkspaceCoverage {
        id: row.try_get("id")?,
        client_id: row.try_get("client_id")?,
        priority: row.try_get("priority")?,
        payer_name: row.try_get("payer_name")?,
        plan_name: row.try_get("plan_name")?,
        member_id: row.try_get("member_id")?,
        group_number: row.try_get("group_number")?,
        coverage_type: row.try_get("coverage_type")?,
        coverage_status: row.try_get("coverage_status")?,
        effective_date: row.try_get("effective_date")?,
        termination_date: row.try_get("termination_date")?,
        patient_relationship_to_subscriber: row.try_get("patient_relationship_to_subscriber")?,
        subscriber_first_name: row.try_get("subscriber_first_name")?,
        subscriber_middle_name: row.try_get("subscriber_middle_name")?,
        subscriber_last_name: row.try_get("subscriber_last_name")?,
        subscriber_suffix: row.try_get("subscriber_suffix")?,
        subscriber_date_of_birth: row.try_get("subscriber_date_of_birth")?,
        subscriber_administrative_sex: row.try_get("subscriber_administrative_sex")?,
        subscriber_address_same_as_patient: row
            .try_get::<i64, _>("subscriber_address_same_as_patient")?
            != 0,
        subscriber_address_line_1: row.try_get("subscriber_address_line_1")?,
        subscriber_address_line_2: row.try_get("subscriber_address_line_2")?,
        subscriber_city: row.try_get("subscriber_city")?,
        subscriber_state_or_province: row.try_get("subscriber_state_or_province")?,
        subscriber_postal_code: row.try_get("subscriber_postal_code")?,
        subscriber_country: row.try_get("subscriber_country")?,
        coverage_notes: row.try_get("coverage_notes")?,
        source_kind: row.try_get("source_kind")?,
        created_at: epoch_millis_to_datetime(row.try_get("created_at_ms")?)?,
        updated_at: epoch_millis_to_datetime(row.try_get("updated_at_ms")?)?,
    })
}

fn verification_from_row(
    row: &SqliteRow,
    current_patient_version: &str,
    current_coverage_version: &str,
) -> anyhow::Result<WorkspaceCoverageVerification> {
    let patient_version: String = row.try_get("patient_version")?;
    let coverage_version: String = row.try_get("coverage_version")?;
    Ok(WorkspaceCoverageVerification {
        id: row.try_get("id")?,
        coverage_id: row.try_get("coverage_id")?,
        client_id: row.try_get("client_id")?,
        source_document_id: row.try_get("source_document_id")?,
        source_document_version: row.try_get("source_document_version")?,
        source_document_content_sha256: row.try_get("source_document_content_sha256")?,
        compared_subject: parse_subject(row.try_get::<String, _>("compared_subject")?.as_str())?,
        observed_first_name: row.try_get("observed_first_name")?,
        observed_middle_name: row.try_get("observed_middle_name")?,
        observed_last_name: row.try_get("observed_last_name")?,
        observed_suffix: row.try_get("observed_suffix")?,
        observed_member_id: row.try_get("observed_member_id")?,
        patient_record_version: row.try_get("patient_record_version")?,
        is_stale: patient_version != current_patient_version
            || coverage_version != current_coverage_version,
        patient_version,
        coverage_version,
        match_result: parse_match_result(row.try_get::<String, _>("match_result")?.as_str())?,
        mismatch_fields: serde_json::from_str(&row.try_get::<String, _>("mismatch_fields_json")?)?,
        actor: row.try_get("actor")?,
        content_sha256: row.try_get("content_sha256")?,
        created_at: epoch_millis_to_datetime(row.try_get("created_at_ms")?)?,
    })
}

fn normalize_optional(value: &mut Option<String>) {
    *value = value.take().and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    });
}

fn eligible_card_document(document: &crate::WorkspaceDocument) -> bool {
    document.scope.trim().eq_ignore_ascii_case("patient")
        && [document.kind.as_str(), document.detected_kind.as_str()]
            .into_iter()
            .any(is_insurance_card_kind)
        && document
            .reference_kind
            .trim()
            .eq_ignore_ascii_case("local_reference")
        && document
            .existence_status
            .trim()
            .eq_ignore_ascii_case("present")
        && !document.local_path.trim().is_empty()
        && document_content_sha256(document).is_some()
}

fn is_insurance_card_kind(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "insurance_card" | "insurance-card" | "insurance card" | "medicare insurance card image"
    )
}

fn document_content_sha256(document: &crate::WorkspaceDocument) -> Option<String> {
    document
        .content_sha256
        .as_deref()
        .or(document.sha256.as_deref())
        .map(str::trim)
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

fn subject_str(value: WorkspaceCoverageVerificationSubject) -> &'static str {
    match value {
        WorkspaceCoverageVerificationSubject::Beneficiary => "beneficiary",
        WorkspaceCoverageVerificationSubject::Subscriber => "subscriber",
    }
}

fn parse_subject(value: &str) -> anyhow::Result<WorkspaceCoverageVerificationSubject> {
    match value {
        "beneficiary" => Ok(WorkspaceCoverageVerificationSubject::Beneficiary),
        "subscriber" => Ok(WorkspaceCoverageVerificationSubject::Subscriber),
        other => anyhow::bail!("unsupported workspace verification subject `{other}`"),
    }
}

fn match_result_str(value: WorkspaceCoverageMatchResult) -> &'static str {
    match value {
        WorkspaceCoverageMatchResult::Match => "match",
        WorkspaceCoverageMatchResult::Mismatch => "mismatch",
    }
}

fn parse_match_result(value: &str) -> anyhow::Result<WorkspaceCoverageMatchResult> {
    match value {
        "match" => Ok(WorkspaceCoverageMatchResult::Match),
        "mismatch" => Ok(WorkspaceCoverageMatchResult::Mismatch),
        other => anyhow::bail!("unsupported workspace coverage match result `{other}`"),
    }
}

#[cfg(test)]
#[path = "workspace_coverage_tests.rs"]
mod tests;
