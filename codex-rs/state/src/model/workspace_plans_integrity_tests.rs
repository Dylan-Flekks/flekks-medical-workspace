use super::*;

fn message_row(content: &str, content_hash: String) -> WorkspacePlanMessageRow {
    WorkspacePlanMessageRow {
        id: "message".to_string(),
        plan_session_id: "session".to_string(),
        client_id: "client".to_string(),
        guide_run_id: "guide".to_string(),
        sequence: 1,
        role: "assistant".to_string(),
        content: content.to_string(),
        content_sha256: content_hash,
        idempotency_key: "message-key".to_string(),
        source_checkpoint_id: "checkpoint".to_string(),
        source_checkpoint_revision: 1,
        source_checkpoint_sha256: "a".repeat(64),
        encounter_id: None,
        note_id: None,
        source_thread_id: Some("thread".to_string()),
        source_turn_id: Some("turn".to_string()),
        created_at_ms: 1,
    }
}

fn revision_row(content_hash: String, evidence_hash: String) -> WorkspacePlanRevisionRow {
    WorkspacePlanRevisionRow {
        id: "revision".to_string(),
        plan_session_id: "session".to_string(),
        client_id: "client".to_string(),
        guide_run_id: "guide".to_string(),
        revision: 1,
        plan_markdown: "# Plan".to_string(),
        decisions_json: "[\"Proceed\"]".to_string(),
        open_questions_json: "[]".to_string(),
        content_sha256: content_hash,
        evidence_manifest_json: "[{}]".to_string(),
        evidence_manifest_sha256: evidence_hash,
        evidence_read_count: 1,
        idempotency_key: "revision-key".to_string(),
        status: "current".to_string(),
        source_checkpoint_id: "checkpoint".to_string(),
        source_checkpoint_revision: 1,
        source_checkpoint_sha256: "a".repeat(64),
        encounter_id: None,
        note_id: None,
        source_thread_id: "thread".to_string(),
        source_turn_id: "turn".to_string(),
        created_at_ms: 1,
        submitted_at_ms: None,
    }
}

fn proposal_row(payload_json: String, payload_hash: String) -> WorkspacePlanProposalRow {
    WorkspacePlanProposalRow {
        id: "proposal".to_string(),
        plan_session_id: "session".to_string(),
        plan_revision_id: "revision".to_string(),
        client_id: "client".to_string(),
        guide_run_id: "guide".to_string(),
        proposal_kind: "task_draft".to_string(),
        payload_json,
        payload_sha256: payload_hash,
        summary: "Summary".to_string(),
        rationale: "Rationale".to_string(),
        idempotency_key: "proposal-key".to_string(),
        status: "pending".to_string(),
        source_checkpoint_id: "checkpoint".to_string(),
        source_checkpoint_revision: 1,
        source_checkpoint_sha256: "a".repeat(64),
        source_thread_id: "thread".to_string(),
        source_turn_id: "turn".to_string(),
        created_at_ms: 1,
        resolved_at_ms: None,
        resolved_by: None,
    }
}

#[test]
fn audited_plan_models_recompute_message_revision_and_proposal_hashes() {
    let message = "Assistant message";
    message_row(message, content_sha256(message))
        .try_into_model(false)
        .expect("valid message hash");
    let message_error = message_row(message, "b".repeat(64))
        .try_into_model(false)
        .expect_err("substituted message hash must fail");
    assert!(message_error.to_string().contains("content hash check"));

    let canonical_revision = serde_json::to_string(&serde_json::json!({
        "planMarkdown": "# Plan",
        "decisions": ["Proceed"],
        "openQuestions": Vec::<String>::new(),
    }))
    .expect("canonical revision");
    revision_row(content_sha256(&canonical_revision), content_sha256("[{}]"))
        .try_into_model(false)
        .expect("valid revision hashes");
    let revision_error = revision_row("b".repeat(64), content_sha256("[{}]"))
        .try_into_model(false)
        .expect_err("substituted revision hash must fail");
    assert!(revision_error.to_string().contains("content hash checks"));

    let payload = WorkspacePlanProposalPayload::TaskDraft {
        title: "Follow up".to_string(),
        details: "Review progress".to_string(),
        task_kind: "clinical_review".to_string(),
        priority: WorkspaceTaskPriority::Normal,
        due_date: None,
        assigned_to: None,
    };
    let payload_json = serde_json::to_string(&payload).expect("proposal payload");
    proposal_row(payload_json.clone(), content_sha256(&payload_json))
        .try_into_model(false)
        .expect("valid proposal hash");
    let proposal_error = proposal_row(payload_json, "b".repeat(64))
        .try_into_model(false)
        .expect_err("substituted proposal hash must fail");
    assert!(proposal_error.to_string().contains("payload hash check"));
}
