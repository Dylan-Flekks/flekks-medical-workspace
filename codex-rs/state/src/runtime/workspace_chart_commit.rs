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
use sha2::Digest;
use sha2::Sha256;
use sqlx::Row;
use uuid::Uuid;

const CHART_COMMIT_SCHEMA_VERSION: i64 = 1;
const CHART_COMMIT_HASH_PREFIX: &[u8] = b"workspace-chart-commit:v1\0";

pub(super) struct ExistingRecords {
    pub(super) client: Option<crate::WorkspaceClient>,
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
        let mut request_hasher = Sha256::new();
        request_hasher.update(CHART_COMMIT_HASH_PREFIX);
        request_hasher.update(request_json.as_bytes());
        let request_sha256 = format!("{:x}", request_hasher.finalize());
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
            if schema_version != CHART_COMMIT_SCHEMA_VERSION {
                return Err(WorkspaceChartCommitError::Storage {
                    message: format!(
                        "unsupported workspace chart commit receipt schema version {schema_version}"
                    ),
                });
            }
            let stored_hash: String = row.try_get("request_sha256")?;
            let stored_request_json: String = row.try_get("request_json")?;
            if stored_hash != request_sha256 || stored_request_json != request_json {
                return Err(WorkspaceChartCommitError::IdempotencyConflict {
                    idempotency_key: request.idempotency_key.clone(),
                });
            }
            let result_json: String = row.try_get("result_json")?;
            let mut result: WorkspaceChartCommitResult = serde_json::from_str(&result_json)?;
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

#[cfg(test)]
#[path = "workspace_chart_commit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_chart_commit_adversarial_tests.rs"]
mod adversarial_tests;
