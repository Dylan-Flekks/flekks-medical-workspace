use crate::model::WorkspaceDraftCheckpointRow;
use crate::model::WorkspaceDraftSessionRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use chrono::Utc;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use sqlx::SqliteConnection;
use sqlx::Transaction;
use uuid::Uuid;

const MIN_DRAFT_SCHEMA_VERSION: i64 = 1;
const MAX_DRAFT_SCHEMA_VERSION: i64 = 2;
const MAX_NORMALIZED_DRAFT_BYTES: usize = 1024 * 1024;
const MAX_SESSION_CREATION_KEY_BYTES: usize = 256;

impl WorkspaceStore {
    pub async fn create_draft_checkpoint(
        &self,
        input: crate::WorkspaceDraftCheckpointCreate,
    ) -> Result<crate::WorkspaceDraftCheckpoint, crate::WorkspaceDraftError> {
        let (draft_json, schema_version, content_sha256) = normalize_draft(&input.draft_json)?;
        let client_id = required("draft checkpoint client id", &input.client_id)?.to_string();
        let session_creation_key = normalize_session_creation_key(&input)?;
        if input.session_id.is_some() && session_creation_key.is_some() {
            return validation(
                "workspace draft checkpoint accepts either a session id or session creation key, not both",
            );
        }
        let actor = nonempty_or(&input.actor, "local human");
        let trigger = nonempty_or(&input.trigger, "manual");
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        // Capture ordering metadata only after the immediate transaction has serialized writers.
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = create_checkpoint_in_transaction(
            &mut transaction,
            &input,
            &client_id,
            session_creation_key.as_deref(),
            &draft_json,
            schema_version,
            &content_sha256,
            &actor,
            &trigger,
            now_ms,
        )
        .await;
        finish_transaction(transaction, result).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_checkpoint_in_transaction(
    connection: &mut SqliteConnection,
    input: &crate::WorkspaceDraftCheckpointCreate,
    client_id: &str,
    session_creation_key: Option<&str>,
    draft_json: &str,
    schema_version: i64,
    content_sha256: &str,
    actor: &str,
    trigger: &str,
    now_ms: i64,
) -> Result<crate::WorkspaceDraftCheckpoint, crate::WorkspaceDraftError> {
    let client_exists: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM workspace_clients WHERE id = ? AND archived_at_ms IS NULL",
    )
    .bind(client_id)
    .fetch_optional(&mut *connection)
    .await?;
    if client_exists.is_none() {
        return validation(format!(
            "workspace draft client `{client_id}` was not found or is archived"
        ));
    }

    let session_id = resolve_session(
        connection,
        input.session_id.as_deref(),
        session_creation_key,
        client_id,
        actor,
        now_ms,
    )
    .await?;
    if let Some(existing) = checkpoint_by_hash(connection, &session_id, content_sha256).await? {
        validate_replay(&existing, input, schema_version, draft_json)?;
        sqlx::query(
            "UPDATE workspace_draft_sessions SET current_revision = ?, updated_at_ms = ? WHERE id = ?",
        )
        .bind(existing.revision)
        .bind(now_ms)
        .bind(&session_id)
        .execute(&mut *connection)
        .await?;
        return existing.try_into_model(true).map_err(Into::into);
    }

    let revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(revision), 0) + 1 FROM workspace_draft_checkpoints WHERE session_id = ?",
    )
    .bind(&session_id)
    .fetch_one(&mut *connection)
    .await?;
    let checkpoint_id = Uuid::new_v4().to_string();
    let checkpoint = sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
        r#"
INSERT INTO workspace_draft_checkpoints (
    id, session_id, client_id, encounter_id, note_id, base_note_revision,
    schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
RETURNING id, session_id, client_id, encounter_id, note_id, base_note_revision,
          schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
        "#,
    )
    .bind(&checkpoint_id)
    .bind(&session_id)
    .bind(client_id)
    .bind(normalize_optional(input.encounter_id.as_deref()))
    .bind(normalize_optional(input.note_id.as_deref()))
    .bind(input.base_note_revision)
    .bind(schema_version)
    .bind(revision)
    .bind(draft_json)
    .bind(content_sha256)
    .bind(trigger)
    .bind(actor)
    .bind(now_ms)
    .fetch_one(&mut *connection)
    .await?;
    sqlx::query(
        "UPDATE workspace_draft_sessions SET current_revision = ?, updated_at_ms = ? WHERE id = ?",
    )
    .bind(revision)
    .bind(now_ms)
    .bind(&session_id)
    .execute(&mut *connection)
    .await?;
    checkpoint.try_into_model(false).map_err(Into::into)
}

async fn resolve_session(
    connection: &mut SqliteConnection,
    session_id: Option<&str>,
    session_creation_key: Option<&str>,
    client_id: &str,
    actor: &str,
    now_ms: i64,
) -> Result<String, crate::WorkspaceDraftError> {
    if let Some(session_id) = session_id {
        let session_id = required("draft session id", session_id)?;
        let session = session_by_id(connection, session_id)
            .await?
            .ok_or_else(|| crate::WorkspaceDraftError::Validation {
                message: format!("workspace draft session `{session_id}` was not found"),
            })?;
        validate_active_session(&session, client_id)?;
        return Ok(session_id.to_string());
    }
    if let Some(session_creation_key) = session_creation_key
        && let Some(session) =
            session_by_creation_key(connection, client_id, session_creation_key).await?
    {
        validate_active_session(&session, client_id)?;
        return Ok(session.id);
    }
    create_session(connection, client_id, session_creation_key, actor, now_ms).await
}

fn validate_active_session(
    session: &WorkspaceDraftSessionRow,
    client_id: &str,
) -> Result<(), crate::WorkspaceDraftError> {
    if session.client_id != client_id {
        return validation(format!(
            "workspace draft session `{}` belongs to client `{}` not `{client_id}`",
            session.id, session.client_id
        ));
    }
    if session.status != "active" {
        return validation(format!(
            "workspace draft session `{}` is `{}` and cannot checkpoint",
            session.id, session.status
        ));
    }
    Ok(())
}

async fn create_session(
    connection: &mut SqliteConnection,
    client_id: &str,
    session_creation_key: Option<&str>,
    actor: &str,
    now_ms: i64,
) -> Result<String, crate::WorkspaceDraftError> {
    let session_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
INSERT INTO workspace_draft_sessions (
    id, client_id, status, current_revision, created_by,
    created_at_ms, updated_at_ms, closed_at_ms, session_creation_key
) VALUES (?, ?, 'active', 0, ?, ?, ?, NULL, ?)
        "#,
    )
    .bind(&session_id)
    .bind(client_id)
    .bind(actor)
    .bind(now_ms)
    .bind(now_ms)
    .bind(session_creation_key)
    .execute(&mut *connection)
    .await?;
    Ok(session_id)
}

async fn session_by_id(
    connection: &mut SqliteConnection,
    id: &str,
) -> anyhow::Result<Option<WorkspaceDraftSessionRow>> {
    sqlx::query_as::<_, WorkspaceDraftSessionRow>(
        "SELECT id, client_id, status, current_revision, created_by, created_at_ms, updated_at_ms, closed_at_ms FROM workspace_draft_sessions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
    .await
    .map_err(Into::into)
}

async fn session_by_creation_key(
    connection: &mut SqliteConnection,
    client_id: &str,
    session_creation_key: &str,
) -> anyhow::Result<Option<WorkspaceDraftSessionRow>> {
    sqlx::query_as::<_, WorkspaceDraftSessionRow>(
        "SELECT id, client_id, status, current_revision, created_by, created_at_ms, updated_at_ms, closed_at_ms FROM workspace_draft_sessions WHERE client_id = ? AND session_creation_key = ?",
    )
    .bind(client_id)
    .bind(session_creation_key)
    .fetch_optional(&mut *connection)
    .await
    .map_err(Into::into)
}

async fn checkpoint_by_hash(
    connection: &mut SqliteConnection,
    session_id: &str,
    hash: &str,
) -> anyhow::Result<Option<WorkspaceDraftCheckpointRow>> {
    sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
        r#"
SELECT id, session_id, client_id, encounter_id, note_id, base_note_revision,
       schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
FROM workspace_draft_checkpoints
WHERE session_id = ? AND content_sha256 = ?
        "#,
    )
    .bind(session_id)
    .bind(hash)
    .fetch_optional(&mut *connection)
    .await
    .map_err(Into::into)
}

async fn finish_transaction<T>(
    transaction: Transaction<'_, Sqlite>,
    result: Result<T, crate::WorkspaceDraftError>,
) -> Result<T, crate::WorkspaceDraftError> {
    match result {
        Ok(value) => {
            transaction.commit().await?;
            Ok(value)
        }
        Err(error) => rollback_after_error(transaction, error).await,
    }
}

async fn rollback_after_error<T>(
    transaction: Transaction<'_, Sqlite>,
    error: crate::WorkspaceDraftError,
) -> Result<T, crate::WorkspaceDraftError> {
    match transaction.rollback().await {
        Ok(_) => Err(error),
        Err(rollback_error) => Err(crate::WorkspaceDraftError::Storage {
            message: format!(
                "{error}; failed to roll back workspace draft checkpoint transaction: {rollback_error}"
            ),
        }),
    }
}

fn normalize_session_creation_key(
    input: &crate::WorkspaceDraftCheckpointCreate,
) -> Result<Option<String>, crate::WorkspaceDraftError> {
    let Some(key) = input.session_creation_key.as_deref() else {
        return Ok(None);
    };
    let key = required("draft session creation key", key)?;
    if key.len() > MAX_SESSION_CREATION_KEY_BYTES {
        return validation(format!(
            "workspace draft session creation key must not exceed {MAX_SESSION_CREATION_KEY_BYTES} bytes"
        ));
    }
    Ok(Some(key.to_string()))
}

fn normalize_draft(draft_json: &str) -> Result<(String, i64, String), crate::WorkspaceDraftError> {
    let value: Value = serde_json::from_str(draft_json.trim()).map_err(|error| {
        crate::WorkspaceDraftError::Validation {
            message: format!("workspace draft checkpoint must be valid JSON: {error}"),
        }
    })?;
    if !value.is_object() {
        return validation("workspace draft checkpoint must be a JSON object");
    }
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or_else(|| crate::WorkspaceDraftError::Validation {
            message: "workspace draft checkpoint schemaVersion is required".to_string(),
        })?;
    if !(MIN_DRAFT_SCHEMA_VERSION..=MAX_DRAFT_SCHEMA_VERSION).contains(&schema_version) {
        return validation(format!(
            "unsupported workspace draft checkpoint schemaVersion {schema_version}"
        ));
    }
    let normalized = serde_json::to_string(&value)?;
    if normalized.len() > MAX_NORMALIZED_DRAFT_BYTES {
        return validation(format!(
            "workspace draft checkpoint exceeds the {MAX_NORMALIZED_DRAFT_BYTES} byte normalized limit"
        ));
    }
    let hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    Ok((normalized, schema_version, hash))
}

fn validate_replay(
    checkpoint: &WorkspaceDraftCheckpointRow,
    input: &crate::WorkspaceDraftCheckpointCreate,
    schema_version: i64,
    draft_json: &str,
) -> Result<(), crate::WorkspaceDraftError> {
    if checkpoint.client_id != input.client_id.trim()
        || checkpoint.encounter_id != normalized_owned(input.encounter_id.as_deref())
        || checkpoint.note_id != normalized_owned(input.note_id.as_deref())
        || checkpoint.base_note_revision != input.base_note_revision
        || checkpoint.schema_version != schema_version
        || checkpoint.draft_json != draft_json
    {
        return validation(
            "workspace draft checkpoint content hash was reused with different metadata",
        );
    }
    Ok(())
}

fn required<'a>(label: &str, value: &'a str) -> Result<&'a str, crate::WorkspaceDraftError> {
    let value = value.trim();
    if value.is_empty() {
        return validation(format!("workspace {label} must not be empty"));
    }
    Ok(value)
}

fn validation<T>(message: impl Into<String>) -> Result<T, crate::WorkspaceDraftError> {
    Err(crate::WorkspaceDraftError::Validation {
        message: message.into(),
    })
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_owned(value: Option<&str>) -> Option<String> {
    normalize_optional(value).map(str::to_string)
}
