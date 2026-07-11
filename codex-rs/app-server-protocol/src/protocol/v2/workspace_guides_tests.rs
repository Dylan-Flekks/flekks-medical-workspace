use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn guide_run_uses_camel_case_and_exposes_read_only_provenance() {
    let run = WorkspaceGuideRun {
        id: "guide-1".to_string(),
        client_id: "client-1".to_string(),
        session_id: "session-1".to_string(),
        source_checkpoint_id: "checkpoint-1".to_string(),
        source_checkpoint_revision: 2,
        source_checkpoint_sha256: "a".repeat(64),
        encounter_id: Some("encounter-1".to_string()),
        note_id: Some("note-1".to_string()),
        request_schema_version: 1,
        request_envelope: json!({"safety": {"modelToolMode": "disabled"}}),
        request_envelope_sha256: "b".repeat(64),
        idempotency_key: "guide-key".to_string(),
        trigger: "focusChange".to_string(),
        actor: "Clinician Example".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
        model_tool_mode: ModelToolMode::Disabled,
        status: WorkspaceGuideRunStatus::Completed,
        source_thread_id: Some("thread-1".to_string()),
        source_turn_id: Some("turn-1".to_string()),
        terminal_envelope: Some(json!({"type": "completed"})),
        terminal_envelope_sha256: Some("c".repeat(64)),
        created_at: 10,
        updated_at: 11,
        terminal_at: Some(11),
        is_stale: true,
    };

    assert_eq!(
        serde_json::to_value(run).expect("guide run should serialize"),
        json!({
            "id": "guide-1",
            "clientId": "client-1",
            "sessionId": "session-1",
            "sourceCheckpointId": "checkpoint-1",
            "sourceCheckpointRevision": 2,
            "sourceCheckpointSha256": "a".repeat(64),
            "encounterId": "encounter-1",
            "noteId": "note-1",
            "requestSchemaVersion": 1,
            "requestEnvelope": {"safety": {"modelToolMode": "disabled"}},
            "requestEnvelopeSha256": "b".repeat(64),
            "idempotencyKey": "guide-key",
            "trigger": "focusChange",
            "actor": "Clinician Example",
            "provider": "test-provider",
            "model": "test-model",
            "modelToolMode": "disabled",
            "status": "completed",
            "sourceThreadId": "thread-1",
            "sourceTurnId": "turn-1",
            "terminalEnvelope": {"type": "completed"},
            "terminalEnvelopeSha256": "c".repeat(64),
            "createdAt": 10,
            "updatedAt": 11,
            "terminalAt": 11,
            "isStale": true
        })
    );
}

#[test]
fn guide_finish_outcome_is_a_camel_case_tagged_union() {
    let completed: WorkspaceGuideRunFinishOutcome = serde_json::from_value(json!({
        "type": "completed",
        "result": {"schemaVersion": 1, "summary": "Ready"}
    }))
    .expect("completed outcome should deserialize");
    assert_eq!(
        completed,
        WorkspaceGuideRunFinishOutcome::Completed {
            result: json!({"schemaVersion": 1, "summary": "Ready"})
        }
    );

    assert_eq!(
        serde_json::to_value(WorkspaceGuideRunFinishOutcome::Failed {
            error_summary: "provider unavailable".to_string(),
        })
        .expect("failed outcome should serialize"),
        json!({"type": "failed", "errorSummary": "provider unavailable"})
    );
    assert_eq!(
        serde_json::to_value(WorkspaceGuideRunFinishOutcome::Canceled {
            reason: "superseded".to_string(),
        })
        .expect("canceled outcome should serialize"),
        json!({"type": "canceled", "reason": "superseded"})
    );
}

#[test]
fn guide_list_and_finish_params_allow_only_documented_optional_fields() {
    let list: WorkspaceGuideRunListParams = serde_json::from_value(json!({
        "clientId": "client-1"
    }))
    .expect("guide list params should deserialize");
    assert_eq!(
        list,
        WorkspaceGuideRunListParams {
            client_id: "client-1".to_string(),
            session_id: None,
            cursor: None,
            limit: None,
        }
    );

    let finish: WorkspaceGuideRunFinishParams = serde_json::from_value(json!({
        "runId": "guide-1",
        "clientId": "client-1",
        "sessionId": "session-1",
        "sourceCheckpointId": "checkpoint-1",
        "sourceCheckpointRevision": 1,
        "sourceCheckpointSha256": "a",
        "requestEnvelopeSha256": "b",
        "outcome": {"type": "canceled", "reason": "superseded"},
        "actor": "Workspace Guide"
    }))
    .expect("guide finish params should deserialize");
    assert_eq!(finish.source_thread_id, None);
    assert_eq!(finish.source_turn_id, None);
}

#[test]
fn guide_error_kinds_serialize_as_stable_tagged_data() {
    assert_eq!(
        serde_json::to_value(WorkspaceGuideRunErrorData::StaleCheckpoint)
            .expect("guide error data should serialize"),
        json!({"kind": "staleCheckpoint"})
    );
    assert_eq!(
        serde_json::to_value(WorkspaceGuideRunErrorData::TerminalConflict)
            .expect("guide error data should serialize"),
        json!({"kind": "terminalConflict"})
    );
}
