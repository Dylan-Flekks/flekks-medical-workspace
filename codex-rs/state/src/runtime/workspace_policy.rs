use super::workspace::WorkspaceStore;
use crate::model::datetime_to_epoch_millis;
use crate::model::epoch_millis_to_datetime;
use chrono::Utc;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::SqliteConnection;
use sqlx::Transaction;
use std::collections::BTreeSet;

pub(super) const WORKSPACE_DOMAIN_TABLES: [&str; 24] = [
    "workspace_clients",
    "workspace_notes",
    "workspace_note_revisions",
    "workspace_note_proposals",
    "workspace_audit_events",
    "workspace_documents",
    "workspace_encounters",
    "workspace_note_signatures",
    "workspace_note_addenda",
    "workspace_tasks",
    "workspace_context_packets",
    "workspace_agent_results",
    "workspace_artifact_derivatives",
    "workspace_context_clips",
    "workspace_client_contacts",
    "workspace_client_coverages",
    "workspace_patient_safety_items",
    "workspace_agent_runs",
    "workspace_agent_run_sources",
    "workspace_note_proposal_decisions",
    "workspace_chart_commits",
    "workspace_draft_sessions",
    "workspace_draft_checkpoints",
    "workspace_guide_runs",
];

const POLICY_TABLE: &str = "workspace_data_policy";
const MAX_CLASSIFIED_BY_BYTES: usize = 256;

impl WorkspaceStore {
    pub async fn workspace_data_policy_status(
        &self,
    ) -> anyhow::Result<crate::WorkspaceDataPolicyStatus> {
        let mut connection = self.pool.acquire().await?;
        read_policy(&mut connection).await
    }

    pub async fn provision_synthetic_workspace(
        &self,
        classified_by: &str,
    ) -> anyhow::Result<crate::WorkspaceSyntheticProvisionOutcome> {
        let classified_by = classified_by.trim();
        if classified_by.is_empty() {
            anyhow::bail!("workspace synthetic data classification requires an attestation source");
        }
        if classified_by.len() > MAX_CLASSIFIED_BY_BYTES {
            anyhow::bail!(
                "workspace synthetic data attestation source exceeds the {MAX_CLASSIFIED_BY_BYTES} byte limit"
            );
        }

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = read_policy(&mut tx).await?;
        match current.data_classification {
            crate::WorkspaceDataClassification::Synthetic => {
                tx.rollback().await?;
                return Ok(crate::WorkspaceSyntheticProvisionOutcome::AlreadySynthetic(
                    current,
                ));
            }
            crate::WorkspaceDataClassification::Unclassified => {}
        }
        if workspace_has_domain_data(&mut tx).await? {
            anyhow::bail!(
                "workspace data classification cannot change after workspace records exist"
            );
        }

        let updated = sqlx::query(
            "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = ?, classified_by = ? WHERE singleton_id = 1 AND schema_version = 1 AND data_classification = 'unclassified' AND classified_at_ms IS NULL AND classified_by IS NULL",
        )
        .bind(datetime_to_epoch_millis(Utc::now()))
        .bind(classified_by)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            anyhow::bail!("workspace data policy changed concurrently or is not canonical");
        }
        let status = read_policy(&mut tx).await?;
        tx.commit().await?;
        Ok(crate::WorkspaceSyntheticProvisionOutcome::Provisioned(
            status,
        ))
    }
}

pub(super) async fn require_synthetic_workspace(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<crate::WorkspaceDataPolicyStatus, WorkspacePolicyRequirementError> {
    let status = read_policy(tx)
        .await
        .map_err(WorkspacePolicyRequirementError::Integrity)?;
    if status.data_classification != crate::WorkspaceDataClassification::Synthetic {
        return Err(WorkspacePolicyRequirementError::NotSynthetic);
    }
    Ok(status)
}

#[derive(Debug)]
pub(super) enum WorkspacePolicyRequirementError {
    NotSynthetic,
    Integrity(anyhow::Error),
}

impl std::fmt::Display for WorkspacePolicyRequirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSynthetic => formatter.write_str(
                "workspace model runs require an explicit synthetic data classification before any workspace records are created",
            ),
            Self::Integrity(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for WorkspacePolicyRequirementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotSynthetic => None,
            Self::Integrity(error) => Some(error.as_ref()),
        }
    }
}
async fn read_policy(
    connection: &mut SqliteConnection,
) -> anyhow::Result<crate::WorkspaceDataPolicyStatus> {
    validate_known_workspace_schema(connection).await?;
    let rows = sqlx::query(
        "SELECT singleton_id, schema_version, data_classification, classified_at_ms, classified_by FROM workspace_data_policy",
    )
    .fetch_all(&mut *connection)
    .await?;
    if rows.len() != 1 {
        anyhow::bail!(
            "workspace data policy must contain exactly one row; found {}",
            rows.len()
        );
    }
    use sqlx::Row;
    let row = &rows[0];
    let singleton_id: i64 = row.try_get("singleton_id")?;
    let schema_version: i64 = row.try_get("schema_version")?;
    if singleton_id != 1 || schema_version != 1 {
        anyhow::bail!(
            "workspace data policy has noncanonical identity or schema: id={singleton_id}, schema={schema_version}"
        );
    }
    let data_classification = crate::WorkspaceDataClassification::from_stored(
        row.try_get::<&str, _>("data_classification")?,
    )?;
    let classified_at_ms: Option<i64> = row.try_get("classified_at_ms")?;
    let classified_by: Option<String> = row.try_get("classified_by")?;
    match (
        data_classification,
        classified_at_ms,
        classified_by.as_deref(),
    ) {
        (crate::WorkspaceDataClassification::Unclassified, None, None) => {}
        (crate::WorkspaceDataClassification::Synthetic, Some(at_ms), Some(source))
            if at_ms >= 0
                && !source.is_empty()
                && source == source.trim()
                && source.len() <= MAX_CLASSIFIED_BY_BYTES => {}
        _ => anyhow::bail!("workspace data policy row is internally inconsistent"),
    }
    Ok(crate::WorkspaceDataPolicyStatus {
        schema_version,
        data_classification,
        classified_at: classified_at_ms.map(epoch_millis_to_datetime).transpose()?,
        classified_by,
    })
}

async fn validate_known_workspace_schema(connection: &mut SqliteConnection) -> anyhow::Result<()> {
    let actual = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name LIKE 'workspace_%' ORDER BY name",
    )
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut expected = WORKSPACE_DOMAIN_TABLES
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    expected.insert(POLICY_TABLE.to_string());
    if actual != expected {
        anyhow::bail!(
            "workspace schema does not match the data-classification safety boundary; expected {expected:?}, found {actual:?}"
        );
    }
    Ok(())
}

async fn workspace_has_domain_data(connection: &mut SqliteConnection) -> anyhow::Result<bool> {
    for table in WORKSPACE_DOMAIN_TABLES {
        let mut query = QueryBuilder::<Sqlite>::new("SELECT EXISTS(SELECT 1 FROM ");
        query.push(table).push(" LIMIT 1)");
        if query
            .build_query_scalar::<bool>()
            .fetch_one(&mut *connection)
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "workspace_policy_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_policy_gating_tests.rs"]
mod gating_tests;
