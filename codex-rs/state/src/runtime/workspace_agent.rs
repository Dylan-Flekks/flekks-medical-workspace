use super::workspace_agent_queries::validate_agent_source_ownership;
use super::workspace_agent_queries::workspace_agent_result_row_by_id;
use super::workspace_agent_queries::workspace_agent_result_row_by_run;
use super::workspace_agent_queries::workspace_agent_run_row_by_id;
use super::workspace_agent_queries::workspace_agent_run_row_by_key;
use super::workspace_agent_queries::workspace_context_packet_row_by_id;
use super::*;
use crate::model::WorkspaceAgentRunRow;
use crate::model::WorkspaceAgentRunSourceRow;
use crate::model::WorkspaceContextPacketRow;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;

impl WorkspaceStore {
    pub async fn prepare_context_packet(
        &self,
        mut input: crate::WorkspaceContextPacketCreate,
    ) -> anyhow::Result<crate::WorkspaceContextPacket> {
        input.status = "prepared".to_string();
        self.create_context_packet(input).await
    }

    pub async fn submit_context_packet(
        &self,
        input: crate::WorkspaceContextPacketLifecycleUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceContextPacket>> {
        self.update_context_packet_lifecycle(input, "submitted")
            .await
    }

    pub async fn cancel_context_packet(
        &self,
        input: crate::WorkspaceContextPacketLifecycleUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceContextPacket>> {
        self.update_context_packet_lifecycle(input, "canceled")
            .await
    }

    async fn update_context_packet_lifecycle(
        &self,
        input: crate::WorkspaceContextPacketLifecycleUpdate,
        target_status: &str,
    ) -> anyhow::Result<Option<crate::WorkspaceContextPacket>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let Some(packet) = workspace_context_packet_row_by_id(&mut tx, &input.packet_id).await?
        else {
            tx.rollback().await?;
            return Ok(None);
        };
        validate_packet_identity(
            &packet,
            &input.client_id,
            &input.expected_context_envelope_sha256,
        )?;

        if packet.status == target_status {
            tx.rollback().await?;
            return crate::WorkspaceContextPacket::try_from(packet).map(Some);
        }
        if packet.status != "prepared" {
            anyhow::bail!(
                "workspace context packet `{}` cannot transition from `{}` to `{target_status}`",
                packet.id,
                packet.status
            );
        }

        let actor = nonempty_or(&input.actor, &packet.clinician_actor);
        let updated = match target_status {
            "submitted" => sqlx::query(
                "UPDATE workspace_context_packets SET status = 'submitted', submitted_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'prepared'",
            )
            .bind(now_ms)
            .bind(now_ms)
            .bind(&packet.id)
            .execute(&mut *tx)
            .await?,
            "canceled" => sqlx::query(
                "UPDATE workspace_context_packets SET status = 'canceled', canceled_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'prepared'",
            )
            .bind(now_ms)
            .bind(now_ms)
            .bind(&packet.id)
            .execute(&mut *tx)
            .await?,
            other => anyhow::bail!("unsupported workspace context packet lifecycle `{other}`"),
        };
        if updated.rows_affected() != 1 {
            anyhow::bail!(
                "workspace context packet `{}` lifecycle changed concurrently",
                packet.id
            );
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "context_packet".to_string(),
                entity_id: packet.id.clone(),
                action: target_status.to_string(),
                actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(packet.client_id),
                encounter_id: packet.encounter_id,
                note_id: packet.note_id,
                success: true,
                summary: packet.human_request,
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let packet = workspace_context_packet_row_by_id(&mut tx, &input.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` disappeared after lifecycle update",
                    input.packet_id
                )
            })?;
        tx.commit().await?;
        crate::WorkspaceContextPacket::try_from(packet).map(Some)
    }

    pub async fn start_agent_run(
        &self,
        input: crate::WorkspaceAgentRunStart,
    ) -> anyhow::Result<crate::WorkspaceAgentRun> {
        if input.idempotency_key.trim().is_empty() {
            anyhow::bail!("workspace agent run idempotency key must not be empty");
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let packet = workspace_context_packet_row_by_id(&mut tx, &input.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` was not found",
                    input.packet_id
                )
            })?;
        validate_packet_identity(
            &packet,
            &input.expected_client_id,
            &input.expected_context_envelope_sha256,
        )?;
        if packet.status == "canceled" {
            anyhow::bail!(
                "workspace context packet `{}` was canceled and cannot start a run",
                packet.id
            );
        }

        if let Some(existing) =
            workspace_agent_run_row_by_key(&mut tx, &packet.id, input.idempotency_key.trim())
                .await?
        {
            tx.rollback().await?;
            return existing.try_into();
        }

        if packet.status == "prepared" {
            sqlx::query(
                "UPDATE workspace_context_packets SET status = 'submitted', submitted_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'prepared'",
            )
            .bind(now_ms)
            .bind(now_ms)
            .bind(&packet.id)
            .execute(&mut *tx)
            .await?;
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "context_packet".to_string(),
                    entity_id: packet.id.clone(),
                    action: "submitted".to_string(),
                    actor: nonempty_or(&input.actor, &packet.clinician_actor),
                    actor_kind: "human".to_string(),
                    source: "state".to_string(),
                    client_id: Some(packet.client_id.clone()),
                    encounter_id: packet.encounter_id.clone(),
                    note_id: packet.note_id.clone(),
                    success: true,
                    summary: packet.human_request.clone(),
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        } else if !matches!(
            packet.status.as_str(),
            "submitted" | "sent" | "result_saved"
        ) {
            anyhow::bail!(
                "workspace context packet `{}` has unsupported lifecycle `{}`",
                packet.id,
                packet.status
            );
        }

        let id = Uuid::new_v4().to_string();
        let run_kind = nonempty_or(&input.run_kind, "agent");
        sqlx::query(
            r#"
INSERT INTO workspace_agent_runs (
    id, packet_id, client_id, note_id, base_note_revision,
    context_envelope_sha256, run_kind, idempotency_key, provider, model,
    source_thread_id, source_turn_id, status, error_summary,
    started_at_ms, completed_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'running', '', ?, NULL, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&packet.id)
        .bind(&packet.client_id)
        .bind(&packet.note_id)
        .bind(packet.base_note_revision)
        .bind(&packet.context_envelope_sha256)
        .bind(&run_kind)
        .bind(input.idempotency_key.trim())
        .bind(input.provider.trim())
        .bind(input.model.trim())
        .bind(&input.source_thread_id)
        .bind(&input.source_turn_id)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        let packet_source_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
INSERT INTO workspace_agent_run_sources (
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
) VALUES (?, ?, 'context_packet', ?, ?, 'Authoritative context packet', ?, ?, 'authorized packet handoff', ?)
            "#,
        )
        .bind(packet_source_id)
        .bind(&id)
        .bind(&packet.id)
        .bind(packet.base_note_revision)
        .bind(&packet.context_envelope_json)
        .bind(&packet.context_envelope_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: id.clone(),
                action: "started".to_string(),
                actor: nonempty_or(&input.actor, "agent"),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(packet.client_id),
                encounter_id: packet.encounter_id,
                note_id: packet.note_id,
                source_thread_id: input.source_thread_id,
                source_turn_id: input.source_turn_id,
                success: true,
                summary: format!("{run_kind} run started"),
                metadata_json: Some(format!(
                    r#"{{"packet_id":"{}","base_note_revision":{},"context_envelope_sha256":"{}"}}"#,
                    packet.id,
                    packet
                        .base_note_revision
                        .map_or_else(|| "null".to_string(), |revision| revision.to_string()),
                    packet.context_envelope_sha256
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = workspace_agent_run_row_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("inserted workspace agent run `{id}` was not found"))?;
        tx.commit().await?;
        row.try_into()
    }

    pub async fn list_agent_runs(
        &self,
        filter: crate::WorkspaceAgentRunFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceAgentRun>> {
        let limit = filter.limit.unwrap_or(20).clamp(1, 100);
        let rows = sqlx::query(
            r#"
SELECT
    id, packet_id, client_id, note_id, base_note_revision,
    context_envelope_sha256, run_kind, idempotency_key, provider, model,
    source_thread_id, source_turn_id, status, error_summary,
    started_at_ms, completed_at_ms, created_at_ms, updated_at_ms
FROM workspace_agent_runs
WHERE client_id = ?
  AND (? IS NULL OR note_id = ?)
  AND (? IS NULL OR packet_id = ?)
ORDER BY created_at_ms DESC
LIMIT ?
            "#,
        )
        .bind(filter.client_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(&filter.packet_id)
        .bind(&filter.packet_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| WorkspaceAgentRunRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn update_agent_run_status(
        &self,
        input: crate::WorkspaceAgentRunStatusUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceAgentRun>> {
        let status = input.status.trim();
        if !matches!(status, "failed" | "canceled") {
            anyhow::bail!(
                "workspace agent run status update must be failed or canceled; completion is result-owned"
            );
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let Some(existing) = workspace_agent_run_row_by_id(&mut tx, &input.run_id).await? else {
            tx.rollback().await?;
            return Ok(None);
        };
        if existing.status == status {
            tx.rollback().await?;
            return existing.try_into().map(Some);
        }
        if existing.status != "running" {
            anyhow::bail!(
                "workspace agent run `{}` cannot transition from `{}` to `{status}`",
                existing.id,
                existing.status
            );
        }
        sqlx::query(
            "UPDATE workspace_agent_runs SET status = ?, error_summary = ?, completed_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'running'",
        )
        .bind(status)
        .bind(input.error_summary.trim())
        .bind(now_ms)
        .bind(now_ms)
        .bind(&existing.id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: existing.id.clone(),
                action: status.to_string(),
                actor: nonempty_or(&input.actor, "agent"),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(existing.client_id),
                note_id: existing.note_id,
                source_thread_id: existing.source_thread_id,
                source_turn_id: existing.source_turn_id,
                success: status != "failed",
                summary: input.error_summary,
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = workspace_agent_run_row_by_id(&mut tx, &input.run_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "updated workspace agent run `{}` was not found",
                    input.run_id
                )
            })?;
        tx.commit().await?;
        row.try_into().map(Some)
    }

    pub async fn record_agent_run_source(
        &self,
        input: crate::WorkspaceAgentRunSourceCreate,
    ) -> anyhow::Result<crate::WorkspaceAgentRunSource> {
        if input.source_entity_type.trim().is_empty() || input.source_entity_id.trim().is_empty() {
            anyhow::bail!("workspace agent run source type and id must not be empty");
        }
        let snapshot_json = input.snapshot_json.trim();
        let snapshot: serde_json::Value = serde_json::from_str(snapshot_json).map_err(|err| {
            anyhow::anyhow!("workspace agent run source snapshot must be valid JSON: {err}")
        })?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let run = workspace_agent_run_row_by_id(&mut tx, &input.run_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("workspace agent run `{}` was not found", input.run_id)
            })?;
        if run.status != "running" {
            anyhow::bail!(
                "workspace agent run `{}` is `{}` and cannot record new source reads",
                run.id,
                run.status
            );
        }
        if let Some(snapshot_client_id) = snapshot
            .get("clientId")
            .or_else(|| snapshot.get("client_id"))
            .and_then(serde_json::Value::as_str)
            && snapshot_client_id != run.client_id
        {
            anyhow::bail!(
                "workspace agent run source snapshot belongs to client `{snapshot_client_id}` not `{}`",
                run.client_id
            );
        }
        validate_agent_source_ownership(
            &mut tx,
            &run,
            input.source_entity_type.trim(),
            input.source_entity_id.trim(),
            input.source_revision,
        )
        .await?;

        let id = Uuid::new_v4().to_string();
        let content_sha256 = format!("{:x}", Sha256::digest(snapshot_json.as_bytes()));
        let row = sqlx::query(
            r#"
INSERT INTO workspace_agent_run_sources (
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
RETURNING
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
            "#,
        )
        .bind(&id)
        .bind(&run.id)
        .bind(input.source_entity_type.trim())
        .bind(input.source_entity_id.trim())
        .bind(input.source_revision)
        .bind(input.display_label.trim())
        .bind(snapshot_json)
        .bind(&content_sha256)
        .bind(input.access_purpose.trim())
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: run.id,
                action: "source_read".to_string(),
                actor: "agent".to_string(),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(run.client_id),
                note_id: run.note_id,
                source_thread_id: run.source_thread_id,
                source_turn_id: run.source_turn_id,
                success: true,
                summary: input.display_label,
                metadata_json: Some(format!(
                    r#"{{"source_entity_type":"{}","source_entity_id":"{}","source_revision":{},"content_sha256":"{}"}}"#,
                    input.source_entity_type,
                    input.source_entity_id,
                    input
                        .source_revision
                        .map_or_else(|| "null".to_string(), |revision| revision.to_string()),
                    content_sha256
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceAgentRunSourceRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_agent_run_sources(
        &self,
        run_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceAgentRunSource>> {
        let rows = sqlx::query(
            r#"
SELECT
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
FROM workspace_agent_run_sources
WHERE run_id = ?
ORDER BY accessed_at_ms ASC, id ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter()
            .map(|row| WorkspaceAgentRunSourceRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn complete_agent_run_with_result(
        &self,
        input: crate::WorkspaceAgentResultCreate,
    ) -> anyhow::Result<crate::WorkspaceAgentResult> {
        let run_id = input
            .run_id
            .as_deref()
            .map(str::trim)
            .filter(|run_id| !run_id.is_empty())
            .ok_or_else(|| anyhow::anyhow!("workspace agent result run id must not be empty"))?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let run = workspace_agent_run_row_by_id(&mut tx, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace agent run `{run_id}` was not found"))?;
        if run.packet_id != input.packet_id {
            anyhow::bail!(
                "workspace agent run `{run_id}` belongs to packet `{}` not `{}`",
                run.packet_id,
                input.packet_id
            );
        }
        validate_result_identity(
            &run,
            input.expected_client_id.as_deref(),
            input.expected_note_id.as_deref(),
            &input.expected_context_envelope_sha256,
        )?;

        if let Some(existing) = workspace_agent_result_row_by_run(&mut tx, run_id).await? {
            if existing.body == input.body {
                tx.rollback().await?;
                return existing.try_into();
            }
            anyhow::bail!("workspace agent run `{run_id}` already has a different result");
        }
        if run.status != "running" {
            anyhow::bail!(
                "workspace agent run `{run_id}` is `{}` and cannot complete with a result",
                run.status
            );
        }
        if input.body.trim().is_empty() {
            anyhow::bail!("workspace agent result body must not be empty");
        }
        let structured_changes_json = nonempty_or(&input.structured_changes_json, "[]");
        let _: serde_json::Value =
            serde_json::from_str(&structured_changes_json).map_err(|err| {
                anyhow::anyhow!(
                    "workspace agent result structured changes must be valid JSON: {err}"
                )
            })?;
        let id = Uuid::new_v4().to_string();
        let status = nonempty_or(&input.status, "review_pending");
        let result_kind = nonempty_or(&input.result_kind, "recommendation");
        sqlx::query(
            r#"
INSERT INTO workspace_agent_results (
    id, packet_id, client_id, note_id, run_id, base_note_revision,
    packet_context_sha256, body, summary, result_kind,
    structured_changes_json, rationale_summary, status, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&run.packet_id)
        .bind(&run.client_id)
        .bind(&run.note_id)
        .bind(&run.id)
        .bind(run.base_note_revision)
        .bind(&run.context_envelope_sha256)
        .bind(&input.body)
        .bind(&input.summary)
        .bind(&result_kind)
        .bind(&structured_changes_json)
        .bind(&input.rationale_summary)
        .bind(&status)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query(
            "UPDATE workspace_agent_runs SET status = 'completed', completed_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'running'",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(&run.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            anyhow::bail!("workspace agent run `{run_id}` completion raced");
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_result".to_string(),
                entity_id: id.clone(),
                action: "saved".to_string(),
                actor: nonempty_or(&input.actor, "agent"),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(run.client_id.clone()),
                note_id: run.note_id.clone(),
                source_thread_id: run.source_thread_id.clone(),
                source_turn_id: run.source_turn_id.clone(),
                success: true,
                summary: input.summary,
                metadata_json: Some(format!(
                    r#"{{"packet_id":"{}","run_id":"{}","base_note_revision":{},"context_envelope_sha256":"{}","result_kind":"{}"}}"#,
                    run.packet_id,
                    run.id,
                    run.base_note_revision
                        .map_or_else(|| "null".to_string(), |revision| revision.to_string()),
                    run.context_envelope_sha256,
                    result_kind
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: run.id.clone(),
                action: "completed".to_string(),
                actor: nonempty_or(&input.actor, "agent"),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(run.client_id),
                note_id: run.note_id,
                source_thread_id: run.source_thread_id,
                source_turn_id: run.source_turn_id,
                success: true,
                summary: format!("result {id} saved"),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = workspace_agent_result_row_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("inserted workspace agent result `{id}` was not found")
            })?;
        tx.commit().await?;
        row.try_into()
    }
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn validate_packet_identity(
    packet: &WorkspaceContextPacketRow,
    expected_client_id: &str,
    expected_hash: &str,
) -> anyhow::Result<()> {
    if !expected_client_id.trim().is_empty() && packet.client_id != expected_client_id.trim() {
        anyhow::bail!(
            "workspace context packet `{}` belongs to client `{}` not `{}`",
            packet.id,
            packet.client_id,
            expected_client_id
        );
    }
    if !expected_hash.trim().is_empty() && packet.context_envelope_sha256 != expected_hash.trim() {
        anyhow::bail!(
            "workspace context packet `{}` envelope hash does not match",
            packet.id
        );
    }
    Ok(())
}

fn validate_result_identity(
    run: &WorkspaceAgentRunRow,
    expected_client_id: Option<&str>,
    expected_note_id: Option<&str>,
    expected_hash: &str,
) -> anyhow::Result<()> {
    if let Some(expected_client_id) = expected_client_id
        && run.client_id != expected_client_id
    {
        anyhow::bail!(
            "workspace agent run `{}` belongs to client `{}` not `{expected_client_id}`",
            run.id,
            run.client_id
        );
    }
    if let Some(expected_note_id) = expected_note_id
        && run.note_id.as_deref() != Some(expected_note_id)
    {
        anyhow::bail!(
            "workspace agent run `{}` belongs to note `{:?}` not `{expected_note_id}`",
            run.id,
            run.note_id
        );
    }
    if !expected_hash.trim().is_empty() && run.context_envelope_sha256 != expected_hash.trim() {
        anyhow::bail!(
            "workspace agent run `{}` envelope hash does not match",
            run.id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
