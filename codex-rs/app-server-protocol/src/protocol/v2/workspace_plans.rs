use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::workspace::WorkspaceTaskPriority;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanSessionStatus {
    Active,
    Closed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSession {
    pub id: String,
    pub client_id: String,
    pub source_thread_id: Option<String>,
    pub status: WorkspacePlanSessionStatus,
    #[ts(type = "number")]
    pub latest_revision: i64,
    pub created_by: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub closed_at: Option<i64>,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionOpenParams {
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionOpenResponse {
    pub session: WorkspacePlanSession,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionGetParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionGetResponse {
    pub session: Option<WorkspacePlanSession>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionBindThreadParams {
    pub session_id: String,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub expected_thread_id: Option<String>,
    pub source_thread_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSessionBindThreadResponse {
    pub session: WorkspacePlanSession,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanMessageRole {
    Human,
    Assistant,
    Question,
    Answer,
    Error,
    SystemStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanMessage {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    #[ts(type = "number")]
    pub sequence: i64,
    pub role: WorkspacePlanMessageRole,
    pub content: String,
    pub content_sha256: String,
    pub idempotency_key: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanMessageAppendParams {
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub content: String,
    pub idempotency_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanMessageAppendResponse {
    pub message: WorkspacePlanMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSnapshotGetParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub plan_session_id: Option<String>,
    #[ts(optional = nullable)]
    #[ts(type = "number | null")]
    pub after_message_sequence: Option<i64>,
    #[ts(optional = nullable)]
    pub message_limit: Option<u32>,
    #[ts(optional = nullable)]
    pub revision_limit: Option<u32>,
    #[ts(optional = nullable)]
    pub proposal_limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanRevisionStatus {
    Current,
    Outdated,
    Submitted,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRevision {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    pub plan_markdown: String,
    pub decisions_json: String,
    pub open_questions_json: String,
    pub content_sha256: String,
    pub evidence_manifest_json: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: u32,
    pub idempotency_key: String,
    pub status: WorkspacePlanRevisionStatus,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: String,
    pub source_turn_id: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub submitted_at: Option<i64>,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSubmissionReceipt {
    pub plan_revision_id: String,
    pub packet_id: String,
    pub agent_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub plan_content_sha256: String,
    pub evidence_manifest_sha256: String,
    pub submitted_by: String,
    #[ts(type = "number")]
    pub submitted_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanProposalStatus {
    Pending,
    Accepted,
    Declined,
    Outdated,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanProposalPayload {
    NoteRevision {
        note_id: String,
        #[ts(type = "number")]
        base_revision: i64,
        proposed_body: String,
    },
    NoteAddendum {
        note_id: String,
        #[ts(type = "number")]
        base_revision: i64,
        body: String,
    },
    TaskDraft {
        title: String,
        details: String,
        task_kind: String,
        priority: WorkspaceTaskPriority,
        due_date: Option<String>,
        assigned_to: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanProposal {
    pub id: String,
    pub plan_session_id: String,
    pub plan_revision_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub payload: WorkspacePlanProposalPayload,
    pub payload_sha256: String,
    pub summary: String,
    pub rationale: String,
    pub idempotency_key: String,
    pub status: WorkspacePlanProposalStatus,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanSnapshotGetResponse {
    pub session: Option<WorkspacePlanSession>,
    pub messages: Vec<WorkspacePlanMessage>,
    pub revisions: Vec<WorkspacePlanRevision>,
    #[serde(default)]
    pub submission_receipts: Vec<WorkspacePlanSubmissionReceipt>,
    pub proposals: Vec<WorkspacePlanProposal>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanActiveRun {
    pub run: WorkspacePlanGuideRun,
    pub plan_session_id: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub context_read_count: u32,
    #[ts(type = "number")]
    pub claimed_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanTurnCompletionReceipt {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub idempotency_key: String,
    pub assistant_message_id: String,
    pub plan_revision_id: Option<String>,
    pub completion_input_sha256: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: u32,
    pub terminal_envelope_sha256: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    #[ts(type = "number")]
    pub completed_at: i64,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRecoveryGetParams {
    pub plan_session_id: String,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRecoveryState {
    pub session: WorkspacePlanSession,
    pub active_runs: Vec<WorkspacePlanActiveRun>,
    pub pending_questions: Vec<WorkspacePlanMessage>,
    pub current_revision: Option<WorkspacePlanRevision>,
    pub last_completion: Option<WorkspacePlanTurnCompletionReceipt>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRecoveryGetResponse {
    pub recovery: WorkspacePlanRecoveryState,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanGuideRunStatus {
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanGuideRun {
    pub id: String,
    pub client_id: String,
    pub draft_session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub request_envelope_sha256: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub provider: String,
    pub model: String,
    pub status: WorkspacePlanGuideRunStatus,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub terminal_at: Option<i64>,
    pub is_stale: bool,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanGuideRunStartParams {
    pub client_id: String,
    pub draft_session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_json: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub provider: String,
    pub model: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanGuideRunStartResponse {
    pub run: WorkspacePlanGuideRun,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanGuideRunOutcome {
    Failed { error_summary: String },
    Canceled { reason: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanGuideRunFinishParams {
    pub run_id: String,
    pub client_id: String,
    pub draft_session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_envelope_sha256: String,
    #[ts(optional = nullable)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_turn_id: Option<String>,
    pub outcome: WorkspacePlanGuideRunOutcome,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanGuideRunFinishResponse {
    pub run: WorkspacePlanGuideRun,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRevisionOutdateParams {
    pub revision_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub content_sha256: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRevisionOutdateResponse {
    pub revision: WorkspacePlanRevision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRevisionSubmitParams {
    pub revision_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub packet_id: String,
    pub agent_run_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub content_sha256: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanRevisionSubmitResponse {
    pub revision: WorkspacePlanRevision,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspacePlanProposalResolution {
    Accept,
    Decline,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanProposalResolveParams {
    pub proposal_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub resolution: WorkspacePlanProposalResolution,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspacePlanProposalResolveResponse {
    pub proposal: WorkspacePlanProposal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completed_guide_run_finish_is_not_a_public_request_outcome() {
        let hash = "a".repeat(64);
        let error = serde_json::from_value::<WorkspacePlanGuideRunFinishParams>(json!({
            "runId": "run",
            "clientId": "client",
            "draftSessionId": "draft-session",
            "sourceCheckpointId": "checkpoint",
            "sourceCheckpointRevision": 1,
            "sourceCheckpointSha256": hash,
            "requestEnvelopeSha256": "b".repeat(64),
            "sourceThreadId": "thread",
            "sourceTurnId": "turn",
            "outcome": {
                "status": "completed",
                "result_json": r#"{"schemaVersion":1}"#,
            },
        }))
        .expect_err("successful completion is core-owned and absent from the public protocol");
        assert!(error.to_string().contains("unknown variant `completed`"));
    }

    #[test]
    fn revision_submission_requires_explicit_packet_and_agent_run_ids() {
        let hash = "a".repeat(64);
        let params: WorkspacePlanRevisionSubmitParams = serde_json::from_value(json!({
            "revisionId": "revision",
            "planSessionId": "session",
            "clientId": "client",
            "packetId": "packet",
            "agentRunId": "run",
            "sourceCheckpointId": "checkpoint",
            "sourceCheckpointRevision": 2,
            "sourceCheckpointSha256": hash,
            "contentSha256": hash,
        }))
        .expect("revision submit params");
        assert_eq!(params.packet_id, "packet");
        assert_eq!(params.agent_run_id, "run");

        let encoded = serde_json::to_value(params).expect("revision submit JSON");
        assert_eq!(encoded["packetId"], "packet");
        assert_eq!(encoded["agentRunId"], "run");
        assert!(encoded.get("packet_id").is_none());
        assert!(encoded.get("agent_run_id").is_none());
    }

    #[test]
    fn snapshot_exposes_the_exact_immutable_submission_receipt() {
        let content_hash = "a".repeat(64);
        let evidence_hash = "b".repeat(64);
        let response: WorkspacePlanSnapshotGetResponse = serde_json::from_value(json!({
            "session": null,
            "messages": [],
            "revisions": [],
            "submissionReceipts": [{
                "planRevisionId": "revision",
                "packetId": "packet",
                "agentRunId": "run",
                "planSessionId": "session",
                "clientId": "client",
                "planContentSha256": content_hash,
                "evidenceManifestSha256": evidence_hash,
                "submittedBy": "Clinician Example",
                "submittedAt": 1_754_000_000_123_i64,
            }],
            "proposals": [],
        }))
        .expect("snapshot submission receipt");

        assert_eq!(
            response.submission_receipts,
            vec![WorkspacePlanSubmissionReceipt {
                plan_revision_id: "revision".to_string(),
                packet_id: "packet".to_string(),
                agent_run_id: "run".to_string(),
                plan_session_id: "session".to_string(),
                client_id: "client".to_string(),
                plan_content_sha256: "a".repeat(64),
                evidence_manifest_sha256: "b".repeat(64),
                submitted_by: "Clinician Example".to_string(),
                submitted_at: 1_754_000_000_123,
            }]
        );
        let encoded = serde_json::to_value(response).expect("snapshot receipt JSON");
        assert_eq!(encoded["submissionReceipts"][0]["agentRunId"], "run");
        assert_eq!(encoded["submissionReceipts"][0]["packetId"], "packet");
        assert!(encoded.get("submission_receipts").is_none());
    }

    #[test]
    fn snapshot_defaults_missing_receipts_for_older_readers() {
        let response: WorkspacePlanSnapshotGetResponse = serde_json::from_value(json!({
            "session": null,
            "messages": [],
            "revisions": [],
            "proposals": [],
        }))
        .expect("legacy snapshot");
        assert!(response.submission_receipts.is_empty());
    }
}
