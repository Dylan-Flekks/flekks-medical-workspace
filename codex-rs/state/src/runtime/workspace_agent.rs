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
use super::workspace_policy::require_synthetic_workspace;

const HANDOFF_PROMPT_SOURCE_TYPE: &str = "handoff_prompt";
const HANDOFF_PROMPT_RENDERER: &str = "render_workspace_agent_handoff_prompt";
const HANDOFF_PROMPT_RENDERER_VERSION: i64 = 1;

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
        let run_kind = nonempty_or(&input.run_kind, "agent");
        let provider = input.provider.trim().to_string();
        let model = input.model.trim().to_string();
        let source_thread_id = input
            .source_thread_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let source_turn_id = input
            .source_turn_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        match run_kind.as_str() {
            "agent" => {
                if input.source_turn_id.is_some() {
                    anyhow::bail!(
                        "workspace agent run source turn is server-owned and must be claimed after run start"
                    );
                }
                if source_thread_id.is_none() {
                    anyhow::bail!("workspace agent run source thread must not be empty");
                }
                if provider.is_empty() {
                    anyhow::bail!("workspace agent run provider must not be empty");
                }
                if model.is_empty() {
                    anyhow::bail!("workspace agent run model must not be empty");
                }
            }
            "manual_import" => {
                if source_thread_id.is_some() != source_turn_id.is_some() {
                    anyhow::bail!(
                        "workspace manual import source thread and source turn must be provided together"
                    );
                }
            }
            other => anyhow::bail!("unsupported workspace agent run kind `{other}`"),
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let policy = require_synthetic_workspace(&mut tx).await?;
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
        .bind(&provider)
        .bind(&model)
        .bind(&source_thread_id)
        .bind(&source_turn_id)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        let packet_contract_snapshot_json = serde_json::json!({
            "schema": "workspace-agent-packet-contract-v1",
            "packetId": &packet.id,
            "clientId": &packet.client_id,
            "encounterId": &packet.encounter_id,
            "noteId": &packet.note_id,
            "baseNoteRevision": packet.base_note_revision,
            "contextEnvelopeSha256": &packet.context_envelope_sha256,
            "contextEnvelope": serde_json::from_str::<serde_json::Value>(
                &packet.context_envelope_json,
            )?,
            "authorizedScope": serde_json::from_str::<serde_json::Value>(
                &packet.authorized_scope_json,
            )?,
            "expectedOutputKind": &packet.expected_output_kind,
            "safety": {
                "dataClassification": policy.data_classification.as_str(),
            },
        })
        .to_string();
        let packet_contract_sha256 = format!(
            "{:x}",
            Sha256::digest(packet_contract_snapshot_json.as_bytes())
        );
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
        sqlx::query(
            r#"
INSERT INTO workspace_agent_run_sources (
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
) VALUES (?, ?, 'packet_contract', ?, ?, 'Hashed packet authorization contract', ?, ?, 'bind scope and expected output', ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&id)
        .bind(&packet.id)
        .bind(packet.base_note_revision)
        .bind(&packet_contract_snapshot_json)
        .bind(&packet_contract_sha256)
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
                actor_kind: "clinician".to_string(),
                source: "state".to_string(),
                client_id: Some(packet.client_id),
                encounter_id: packet.encounter_id,
                note_id: packet.note_id,
                source_thread_id,
                source_turn_id,
                success: true,
                summary: format!("{run_kind} run started"),
                metadata_json: Some(
                    serde_json::json!({
                        "packet_id": &packet.id,
                        "base_note_revision": packet.base_note_revision,
                        "context_envelope_sha256": &packet.context_envelope_sha256,
                        "packet_contract_sha256": packet_contract_sha256,
                    })
                    .to_string(),
                ),
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
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
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
        if input.source_entity_type.trim() == HANDOFF_PROMPT_SOURCE_TYPE {
            anyhow::bail!(
                "workspace handoff prompt sources are server-owned and may only be recorded by the turn claim"
            );
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
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
        let source = insert_agent_run_source(&mut tx, &run, input, now_ms).await?;
        tx.commit().await?;
        Ok(source)
    }

    /// Atomically claims one running medical agent run for one exact restricted model turn.
    ///
    /// The claim is the security boundary below the TUI: direct app-server clients must present
    /// the canonical packet prompt and must execute on the thread, provider, and model recorded
    /// when the clinician created the run. Setting `source_turn_id` consumes the run capability so
    /// it cannot be sampled a second time.
    pub async fn claim_agent_turn(
        &self,
        input: crate::WorkspaceAgentTurnClaim,
    ) -> anyhow::Result<crate::WorkspaceAgentExecutionBinding> {
        let execution = normalized_execution_binding(input.execution)?;
        if input.prompt.is_empty() {
            anyhow::bail!("workspace agent turn prompt must not be empty");
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
        let run = workspace_agent_run_row_by_id(&mut tx, &execution.run_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("workspace agent run `{}` was not found", execution.run_id)
            })?;
        if run.status != "running" {
            anyhow::bail!(
                "workspace agent run `{}` is `{}` and cannot claim a model turn",
                run.id,
                run.status
            );
        }
        if run.run_kind != "agent" {
            anyhow::bail!(
                "workspace agent run `{}` is `{}` and cannot claim a model turn",
                run.id,
                run.run_kind
            );
        }
        validate_unclaimed_execution_identity(&run, &execution)?;
        let packet = workspace_context_packet_row_by_id(&mut tx, &run.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` for run `{}` was not found",
                    run.packet_id,
                    run.id
                )
            })?;
        validate_run_packet_binding(&run, &packet)?;
        if packet.status != "submitted" {
            anyhow::bail!(
                "workspace context packet `{}` is `{}` and cannot authorize a model turn",
                packet.id,
                packet.status
            );
        }
        let expected_prompt = crate::render_workspace_agent_handoff_prompt(
            &crate::WorkspaceAgentHandoffPromptInput {
                packet_id: packet.id.clone(),
                client_id: packet.client_id.clone(),
                encounter_id: packet.encounter_id.clone(),
                note_id: packet.note_id.clone(),
                human_request: packet.human_request.clone(),
                chart_context_summary: packet.chart_context_summary.clone(),
                context_envelope_json: packet.context_envelope_json.clone(),
                context_envelope_sha256: packet.context_envelope_sha256.clone(),
                authorized_scope_json: packet.authorized_scope_json.clone(),
            },
            Some(run.id.as_str()),
        );
        if input.prompt != expected_prompt {
            anyhow::bail!(
                "workspace agent turn prompt does not match the canonical packet handoff"
            );
        }

        let claimed = sqlx::query(
            "UPDATE workspace_agent_runs SET source_turn_id = ?, updated_at_ms = ? WHERE id = ? AND status = 'running' AND source_turn_id IS NULL",
        )
        .bind(&execution.source_turn_id)
        .bind(now_ms)
        .bind(&run.id)
        .execute(&mut *tx)
        .await?;
        if claimed.rows_affected() != 1 {
            anyhow::bail!(
                "workspace agent run `{}` was already claimed or changed concurrently",
                run.id
            );
        }

        let prompt_sha256 = format!("{:x}", Sha256::digest(input.prompt.as_bytes()));
        let prompt_source_id = Uuid::new_v4().to_string();
        let prompt_snapshot_json = serde_json::json!({
            "schema": "workspace-agent-handoff-prompt-v1",
            "renderer": HANDOFF_PROMPT_RENDERER,
            "rendererVersion": HANDOFF_PROMPT_RENDERER_VERSION,
            "runId": &run.id,
            "packetId": &run.packet_id,
            "clientId": &run.client_id,
            "sourceThreadId": &execution.source_thread_id,
            "sourceTurnId": &execution.source_turn_id,
            "prompt": &input.prompt,
            "promptSha256": &prompt_sha256,
        })
        .to_string();
        let prompt_snapshot_sha256 =
            format!("{:x}", Sha256::digest(prompt_snapshot_json.as_bytes()));
        sqlx::query(
            r#"
INSERT INTO workspace_agent_run_sources (
    id, run_id, source_entity_type, source_entity_id, source_revision,
    display_label, snapshot_json, content_sha256, access_purpose, accessed_at_ms
) VALUES (?, ?, ?, ?, ?, 'Canonical agent handoff prompt', ?, ?, 'authorize one restricted model turn', ?)
            "#,
        )
        .bind(&prompt_source_id)
        .bind(&run.id)
        .bind(HANDOFF_PROMPT_SOURCE_TYPE)
        .bind(&execution.source_turn_id)
        .bind(HANDOFF_PROMPT_RENDERER_VERSION)
        .bind(&prompt_snapshot_json)
        .bind(&prompt_snapshot_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: run.id,
                action: "turn_claimed".to_string(),
                actor: "agent".to_string(),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(run.client_id),
                note_id: run.note_id,
                source_thread_id: Some(execution.source_thread_id.clone()),
                source_turn_id: Some(execution.source_turn_id.clone()),
                success: true,
                summary: "restricted medical context turn claimed".to_string(),
                metadata_json: Some(
                    serde_json::json!({
                        "provider": &execution.provider,
                        "model": &execution.model,
                        "prompt_sha256": prompt_sha256,
                        "prompt_source_id": prompt_source_id,
                        "prompt_snapshot_sha256": prompt_snapshot_sha256,
                        "renderer": HANDOFF_PROMPT_RENDERER,
                        "renderer_version": HANDOFF_PROMPT_RENDERER_VERSION,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        Ok(execution)
    }

    pub async fn read_authorized_agent_context_for_execution(
        &self,
        input: crate::WorkspaceAgentContextReadRequest,
        execution: crate::WorkspaceAgentExecutionBinding,
    ) -> anyhow::Result<crate::WorkspaceAgentContextRead> {
        self.read_authorized_agent_context_inner(input, execution)
            .await
    }

    async fn read_authorized_agent_context_inner(
        &self,
        input: crate::WorkspaceAgentContextReadRequest,
        execution: crate::WorkspaceAgentExecutionBinding,
    ) -> anyhow::Result<crate::WorkspaceAgentContextRead> {
        let category = input.category.trim();
        if !matches!(category, "visit_history" | "progress_notes") {
            anyhow::bail!("unsupported workspace agent context category `{category}`");
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
        let run = workspace_agent_run_row_by_id(&mut tx, input.run_id.trim())
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("workspace agent run `{}` was not found", input.run_id)
            })?;
        if run.status != "running" {
            anyhow::bail!(
                "workspace agent run `{}` is `{}` and cannot read additional context",
                run.id,
                run.status
            );
        }
        let execution = normalized_execution_binding(execution)?;
        validate_claimed_execution_identity(&run, &execution)?;
        let packet = workspace_context_packet_row_by_id(&mut tx, &run.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` for run `{}` was not found",
                    run.packet_id,
                    run.id
                )
            })?;
        validate_run_packet_binding(&run, &packet)?;
        if packet.status != "submitted" {
            anyhow::bail!(
                "workspace context packet `{}` is `{}` and does not authorize agent context reads",
                packet.id,
                packet.status
            );
        }
        let max_records = authorized_context_read_limit(
            &packet.authorized_scope_json,
            category,
            input.max_records,
        )?;

        let sources = match category {
            "visit_history" => {
                read_visit_history_sources(&mut tx, &run, max_records, now_ms).await?
            }
            "progress_notes" => {
                read_progress_note_sources(&mut tx, &run, max_records, now_ms).await?
            }
            _ => unreachable!("category was validated above"),
        };
        let result = crate::WorkspaceAgentContextRead {
            run_id: run.id,
            packet_id: packet.id,
            client_id: run.client_id,
            note_id: run.note_id,
            category: category.to_string(),
            max_records,
            sources,
        };
        tx.commit().await?;
        Ok(result)
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
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
        let mut run = workspace_agent_run_row_by_id(&mut tx, run_id)
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
        let packet = workspace_context_packet_row_by_id(&mut tx, &run.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` for run `{run_id}` was not found",
                    run.packet_id
                )
            })?;
        let result_kind = nonempty_or(&input.result_kind, "recommendation");
        let expected_output_kind = nonempty_or(&packet.expected_output_kind, "recommendation");
        if result_kind != expected_output_kind {
            anyhow::bail!(
                "workspace agent result kind `{result_kind}` does not match packet expected output kind `{expected_output_kind}`"
            );
        }
        let source_thread_id = input
            .source_thread_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let source_turn_id = input
            .source_turn_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if run.run_kind == "manual_import" && source_thread_id.is_some() != source_turn_id.is_some()
        {
            anyhow::bail!(
                "workspace manual import source thread and source turn must be provided together"
            );
        }
        if let (Some(existing), Some(requested)) =
            (run.source_thread_id.as_deref(), source_thread_id)
            && existing != requested
        {
            anyhow::bail!(
                "workspace agent result source thread `{requested}` does not match run source thread `{existing}`"
            );
        }
        if let (Some(existing), Some(requested)) = (run.source_turn_id.as_deref(), source_turn_id)
            && existing != requested
        {
            anyhow::bail!(
                "workspace agent result source turn `{requested}` does not match run source turn `{existing}`"
            );
        }
        if source_turn_id.is_some() && source_thread_id.is_none() && run.source_thread_id.is_none()
        {
            anyhow::bail!("workspace agent result source turn requires a source thread");
        }
        if run.run_kind != "manual_import" {
            if run
                .source_thread_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                anyhow::bail!(
                    "workspace agent run `{run_id}` is missing its claimed source thread"
                );
            }
            let claimed_source_turn_id = run
                .source_turn_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "workspace agent run `{run_id}` must claim a model turn before result completion"
                    )
                })?;
            let prompt_source_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM workspace_agent_run_sources WHERE run_id = ? AND source_entity_type = ? AND source_entity_id = ?)",
            )
            .bind(&run.id)
            .bind(HANDOFF_PROMPT_SOURCE_TYPE)
            .bind(claimed_source_turn_id)
            .fetch_one(&mut *tx)
            .await?;
            if !prompt_source_exists {
                anyhow::bail!(
                    "workspace agent run `{run_id}` does not have a durable claimed handoff prompt"
                );
            }
        } else {
            let bound_source_thread_id = run
                .source_thread_id
                .clone()
                .or_else(|| source_thread_id.map(ToString::to_string));
            let bound_source_turn_id = run
                .source_turn_id
                .clone()
                .or_else(|| source_turn_id.map(ToString::to_string));
            if bound_source_thread_id != run.source_thread_id
                || bound_source_turn_id != run.source_turn_id
            {
                sqlx::query(
                    "UPDATE workspace_agent_runs SET source_thread_id = ?, source_turn_id = ?, updated_at_ms = ? WHERE id = ? AND status = 'running'",
                )
                .bind(&bound_source_thread_id)
                .bind(&bound_source_turn_id)
                .bind(now_ms)
                .bind(&run.id)
                .execute(&mut *tx)
                .await?;
                run.source_thread_id = bound_source_thread_id;
                run.source_turn_id = bound_source_turn_id;
            }
        }

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
        let (result_actor, result_actor_kind, result_source) = if run.run_kind == "manual_import" {
            (
                nonempty_or(&input.actor, "clinician"),
                "clinician",
                "manual_import",
            )
        } else {
            ("agent".to_string(), "agent", "agent_harness")
        };
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_result".to_string(),
                entity_id: id.clone(),
                action: "saved".to_string(),
                actor: result_actor.clone(),
                actor_kind: result_actor_kind.to_string(),
                source: result_source.to_string(),
                client_id: Some(run.client_id.clone()),
                note_id: run.note_id.clone(),
                source_thread_id: run.source_thread_id.clone(),
                source_turn_id: run.source_turn_id.clone(),
                success: true,
                summary: input.summary,
                metadata_json: Some(
                    serde_json::json!({
                        "packet_id": run.packet_id,
                        "run_id": run.id,
                        "base_note_revision": run.base_note_revision,
                        "context_envelope_sha256": run.context_envelope_sha256,
                        "result_kind": result_kind,
                    })
                    .to_string(),
                ),
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
                actor: result_actor,
                actor_kind: result_actor_kind.to_string(),
                source: result_source.to_string(),
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

fn validate_run_packet_binding(
    run: &WorkspaceAgentRunRow,
    packet: &WorkspaceContextPacketRow,
) -> anyhow::Result<()> {
    if packet.client_id != run.client_id
        || packet.note_id != run.note_id
        || packet.context_envelope_sha256 != run.context_envelope_sha256
    {
        anyhow::bail!(
            "workspace agent run `{}` no longer matches its authoritative context packet `{}`",
            run.id,
            packet.id
        );
    }
    Ok(())
}

fn normalized_execution_binding(
    mut execution: crate::WorkspaceAgentExecutionBinding,
) -> anyhow::Result<crate::WorkspaceAgentExecutionBinding> {
    execution.run_id = execution.run_id.trim().to_string();
    execution.source_thread_id = execution.source_thread_id.trim().to_string();
    execution.source_turn_id = execution.source_turn_id.trim().to_string();
    execution.provider = execution.provider.trim().to_string();
    execution.model = execution.model.trim().to_string();
    for (label, value) in [
        ("run id", execution.run_id.as_str()),
        ("source thread", execution.source_thread_id.as_str()),
        ("source turn", execution.source_turn_id.as_str()),
        ("provider", execution.provider.as_str()),
        ("model", execution.model.as_str()),
    ] {
        if value.is_empty() {
            anyhow::bail!("workspace agent execution {label} must not be empty");
        }
    }
    Ok(execution)
}

fn validate_unclaimed_execution_identity(
    run: &WorkspaceAgentRunRow,
    execution: &crate::WorkspaceAgentExecutionBinding,
) -> anyhow::Result<()> {
    if run.id != execution.run_id {
        anyhow::bail!("workspace agent execution run does not match the stored run");
    }
    let Some(source_thread_id) = run
        .source_thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        anyhow::bail!("workspace agent run is missing its required source thread binding");
    };
    if source_thread_id != execution.source_thread_id {
        anyhow::bail!("workspace agent execution source thread does not match the stored run");
    }
    let provider = run.provider.trim();
    if provider.is_empty() {
        anyhow::bail!("workspace agent run is missing its required provider binding");
    }
    if provider != execution.provider {
        anyhow::bail!("workspace agent execution provider does not match the stored run");
    }
    let model = run.model.trim();
    if model.is_empty() {
        anyhow::bail!("workspace agent run is missing its required model binding");
    }
    if model != execution.model {
        anyhow::bail!("workspace agent execution model does not match the stored run");
    }
    if run.source_turn_id.is_some() {
        anyhow::bail!("workspace agent run was already claimed by a model turn");
    }
    Ok(())
}

fn validate_claimed_execution_identity(
    run: &WorkspaceAgentRunRow,
    execution: &crate::WorkspaceAgentExecutionBinding,
) -> anyhow::Result<()> {
    if run.id != execution.run_id
        || run.source_thread_id.as_deref().map(str::trim)
            != Some(execution.source_thread_id.as_str())
        || run.source_turn_id.as_deref().map(str::trim) != Some(execution.source_turn_id.as_str())
        || run.provider.trim() != execution.provider
        || run.model.trim() != execution.model
    {
        anyhow::bail!(
            "workspace agent context read does not match the claimed turn execution identity"
        );
    }
    Ok(())
}

fn authorized_context_read_limit(
    authorized_scope_json: &str,
    category: &str,
    requested_max_records: Option<u32>,
) -> anyhow::Result<u32> {
    let scope: serde_json::Value = serde_json::from_str(authorized_scope_json).map_err(|err| {
        anyhow::anyhow!("workspace context packet authorized scope is invalid JSON: {err}")
    })?;
    let categories = scope
        .get("categories")
        .or_else(|| scope.pointer("/read/categories"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "workspace context packet does not explicitly authorize context category `{category}`"
            )
        })?;
    if !categories
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|authorized| authorized == category)
    {
        anyhow::bail!(
            "workspace context packet does not explicitly authorize context category `{category}`"
        );
    }

    let scope_max_records = scope
        .get("maxRecords")
        .or_else(|| scope.pointer("/read/maxRecords"))
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                anyhow::anyhow!("workspace context packet maxRecords must be an unsigned integer")
            })
        })
        .transpose()?
        .unwrap_or(20)
        .clamp(1, 100) as u32;
    let requested_max_records = requested_max_records.unwrap_or(20).clamp(1, 100);
    Ok(requested_max_records.min(scope_max_records))
}

const MAX_AGENT_NOTE_BODY_BYTES: usize = 32 * 1024;
const MAX_AGENT_CONTEXT_SNAPSHOT_BYTES: usize = 512 * 1024;
const MAX_AGENT_DISPLAY_LABEL_BYTES: usize = 512;

fn truncate_utf8(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut end = max_bytes.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (&value[..end], true)
}

fn token_looks_like_local_path(token: &str) -> bool {
    let token = token
        .rsplit_once('=')
        .map_or(token, |(_, candidate)| candidate)
        .trim_matches(|character: char| {
            matches!(
                character,
                '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });
    token.contains("file://")
        || token.as_bytes().windows(3).any(|window| {
            window[0].is_ascii_alphabetic()
                && window[1] == b':'
                && matches!(window[2], b'/' | b'\\')
        })
        || token.starts_with("~/")
        || token.starts_with("\\\\")
        || token.starts_with("//")
        || (token.starts_with('/') && token != "/workspacemedical")
}

fn redact_local_path_tokens(value: &str) -> (String, bool) {
    let mut redacted = String::with_capacity(value.len());
    let mut changed = false;
    for segment in value.split_inclusive(char::is_whitespace) {
        let token_len = segment.trim_end_matches(char::is_whitespace).len();
        let (token, whitespace) = segment.split_at(token_len);
        if token_looks_like_local_path(token) {
            redacted.push_str("[local path omitted]");
            changed = true;
        } else {
            redacted.push_str(token);
        }
        redacted.push_str(whitespace);
    }
    if value.is_empty() {
        return (String::new(), false);
    }
    (redacted, changed)
}

async fn read_visit_history_sources(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run: &WorkspaceAgentRunRow,
    max_records: u32,
    now_ms: i64,
) -> anyhow::Result<Vec<crate::WorkspaceAgentRunSource>> {
    let rows = sqlx::query(
        r#"
SELECT
    id, client_id, kind, title, status, started_at_ms, ended_at_ms,
    archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_encounters
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY COALESCE(started_at_ms, updated_at_ms) DESC, title ASC, id ASC
LIMIT ?
        "#,
    )
    .bind(&run.client_id)
    .bind(i64::from(max_records))
    .fetch_all(&mut **tx)
    .await?;

    let mut sources = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let client_id: String = row.try_get("client_id")?;
        let kind: String = row.try_get("kind")?;
        let title: String = row.try_get("title")?;
        let status: String = row.try_get("status")?;
        let started_at_ms: Option<i64> = row.try_get("started_at_ms")?;
        let ended_at_ms: Option<i64> = row.try_get("ended_at_ms")?;
        let archived_at_ms: Option<i64> = row.try_get("archived_at_ms")?;
        let created_at_ms: i64 = row.try_get("created_at_ms")?;
        let updated_at_ms: i64 = row.try_get("updated_at_ms")?;
        let (safe_title, title_paths_redacted) = redact_local_path_tokens(&title);
        let (safe_title, title_truncated) =
            truncate_utf8(&safe_title, MAX_AGENT_DISPLAY_LABEL_BYTES);
        let snapshot_json = serde_json::json!({
            "id": id,
            "client_id": client_id,
            "kind": kind,
            "title": safe_title,
            "title_truncated": title_truncated,
            "title_local_paths_redacted": title_paths_redacted,
            "status": status,
            "started_at_ms": started_at_ms,
            "ended_at_ms": ended_at_ms,
            "archived_at_ms": archived_at_ms,
            "created_at_ms": created_at_ms,
            "updated_at_ms": updated_at_ms,
        })
        .to_string();
        sources.push(
            insert_agent_run_source(
                tx,
                run,
                crate::WorkspaceAgentRunSourceCreate {
                    run_id: run.id.clone(),
                    source_entity_type: "encounter".to_string(),
                    source_entity_id: id,
                    source_revision: None,
                    display_label: safe_title.to_string(),
                    snapshot_json,
                    access_purpose: "authorized visit_history read".to_string(),
                },
                now_ms,
            )
            .await?,
        );
    }
    Ok(sources)
}

async fn read_progress_note_sources(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run: &WorkspaceAgentRunRow,
    max_records: u32,
    now_ms: i64,
) -> anyhow::Result<Vec<crate::WorkspaceAgentRunSource>> {
    let rows = sqlx::query(
        r#"
SELECT
    id, client_id, encounter_id, title, kind, body, status,
    current_revision, archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_notes
WHERE client_id = ?
  AND archived_at_ms IS NULL
  AND LOWER(kind) IN ('progress', 'progress_note', 'daily', 'daily_note')
ORDER BY updated_at_ms DESC, title ASC, id ASC
LIMIT ?
        "#,
    )
    .bind(&run.client_id)
    .bind(i64::from(max_records))
    .fetch_all(&mut **tx)
    .await?;

    let mut sources = Vec::with_capacity(rows.len());
    let mut returned_snapshot_bytes = 0usize;
    for row in rows {
        let id: String = row.try_get("id")?;
        let client_id: String = row.try_get("client_id")?;
        let encounter_id: Option<String> = row.try_get("encounter_id")?;
        let title: String = row.try_get("title")?;
        let kind: String = row.try_get("kind")?;
        let body: String = row.try_get("body")?;
        let status: String = row.try_get("status")?;
        let current_revision: i64 = row.try_get("current_revision")?;
        let archived_at_ms: Option<i64> = row.try_get("archived_at_ms")?;
        let created_at_ms: i64 = row.try_get("created_at_ms")?;
        let updated_at_ms: i64 = row.try_get("updated_at_ms")?;
        let original_body_bytes = body.len();
        let original_body_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
        let (safe_body, body_paths_redacted) = redact_local_path_tokens(&body);
        let (safe_body, body_truncated) = truncate_utf8(&safe_body, MAX_AGENT_NOTE_BODY_BYTES);
        let (safe_title, title_paths_redacted) = redact_local_path_tokens(&title);
        let (safe_title, title_truncated) =
            truncate_utf8(&safe_title, MAX_AGENT_DISPLAY_LABEL_BYTES);
        let snapshot_json = serde_json::json!({
            "id": id,
            "client_id": client_id,
            "encounter_id": encounter_id,
            "title": safe_title,
            "title_truncated": title_truncated,
            "title_local_paths_redacted": title_paths_redacted,
            "kind": kind,
            "body": safe_body,
            "body_truncated": body_truncated,
            "body_local_paths_redacted": body_paths_redacted,
            "body_original_bytes": original_body_bytes,
            "body_original_sha256": original_body_sha256,
            "status": status,
            "current_revision": current_revision,
            "archived_at_ms": archived_at_ms,
            "created_at_ms": created_at_ms,
            "updated_at_ms": updated_at_ms,
        })
        .to_string();
        if returned_snapshot_bytes.saturating_add(snapshot_json.len())
            > MAX_AGENT_CONTEXT_SNAPSHOT_BYTES
        {
            break;
        }
        returned_snapshot_bytes += snapshot_json.len();
        sources.push(
            insert_agent_run_source(
                tx,
                run,
                crate::WorkspaceAgentRunSourceCreate {
                    run_id: run.id.clone(),
                    source_entity_type: "note_revision".to_string(),
                    source_entity_id: id,
                    source_revision: Some(current_revision),
                    display_label: safe_title.to_string(),
                    snapshot_json,
                    access_purpose: "authorized progress_notes read".to_string(),
                },
                now_ms,
            )
            .await?,
        );
    }
    Ok(sources)
}

async fn insert_agent_run_source(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run: &WorkspaceAgentRunRow,
    input: crate::WorkspaceAgentRunSourceCreate,
    now_ms: i64,
) -> anyhow::Result<crate::WorkspaceAgentRunSource> {
    if input.run_id.trim() != run.id {
        anyhow::bail!(
            "workspace agent run source requested run `{}` but loaded run `{}`",
            input.run_id,
            run.id
        );
    }
    let source_entity_type = input.source_entity_type.trim();
    let source_entity_id = input.source_entity_id.trim();
    if source_entity_type.is_empty() || source_entity_id.is_empty() {
        anyhow::bail!("workspace agent run source type and id must not be empty");
    }
    let snapshot_json = input.snapshot_json.trim();
    let snapshot: serde_json::Value = serde_json::from_str(snapshot_json).map_err(|err| {
        anyhow::anyhow!("workspace agent run source snapshot must be valid JSON: {err}")
    })?;
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
        tx,
        run,
        source_entity_type,
        source_entity_id,
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
    .bind(source_entity_type)
    .bind(source_entity_id)
    .bind(input.source_revision)
    .bind(input.display_label.trim())
    .bind(snapshot_json)
    .bind(&content_sha256)
    .bind(input.access_purpose.trim())
    .bind(now_ms)
    .fetch_one(&mut **tx)
    .await?;
    insert_audit_event(
        tx,
        crate::WorkspaceAuditEventCreate {
            entity_type: "agent_run".to_string(),
            entity_id: run.id.clone(),
            action: "source_read".to_string(),
            actor: "agent".to_string(),
            actor_kind: "agent".to_string(),
            source: "state".to_string(),
            client_id: Some(run.client_id.clone()),
            note_id: run.note_id.clone(),
            source_thread_id: run.source_thread_id.clone(),
            source_turn_id: run.source_turn_id.clone(),
            success: true,
            summary: input.display_label,
            metadata_json: Some(
                serde_json::json!({
                    "source_entity_type": source_entity_type,
                    "source_entity_id": source_entity_id,
                    "source_revision": input.source_revision,
                    "content_sha256": content_sha256,
                })
                .to_string(),
            ),
            ..Default::default()
        },
        now_ms,
    )
    .await?;
    WorkspaceAgentRunSourceRow::try_from_row(&row).and_then(TryInto::try_into)
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
