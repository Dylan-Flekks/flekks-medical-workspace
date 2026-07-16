use chrono::DateTime;
use chrono::Utc;

/// Atomic terminal write for one capability-bound master medical-agent turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentTurnComplete {
    pub execution: crate::WorkspaceAgentExecutionBinding,
    pub assistant_message_id: String,
    pub body: String,
    pub idempotency_key: String,
}

/// Durable receipt proving which exact model response produced an Agent Review result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentTurnCompletion {
    pub result: crate::WorkspaceAgentResult,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub assistant_message_id: String,
    pub body_sha256: String,
    pub completion_input_sha256: String,
    pub idempotency_key: String,
    pub completed_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceAgentTurnCompletionRow {
    pub run_id: String,
    pub result_id: String,
    pub packet_id: String,
    pub client_id: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub assistant_message_id: String,
    pub body_sha256: String,
    pub completion_input_sha256: String,
    pub idempotency_key: String,
    pub completed_at_ms: i64,
}
