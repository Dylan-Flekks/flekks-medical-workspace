use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Durable, redacted status for one supervised regulated workflow.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct WorkflowSnapshot {
    pub id: String,
    pub title: String,
    pub state: WorkflowStateKind,
    pub boundary_status: WorkflowBoundaryStatus,
    #[serde(default)]
    pub gates: Vec<WorkflowGate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pending_approval: Option<WorkflowApprovalCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub latest_audit_event: Option<WorkflowAuditEvent>,
}

/// High-level lifecycle state for a supervised workflow.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum WorkflowStateKind {
    Idle,
    Planning,
    Running,
    WaitingForApproval,
    Blocked,
    Completed,
}

/// Current sensitive-data or provider-boundary posture for the workflow.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum WorkflowBoundaryStatus {
    Unknown,
    LocalOnly,
    OutboundAllowed,
    OutboundBlocked,
}

/// One policy, safety, or readiness gate evaluated for the workflow.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct WorkflowGate {
    pub id: String,
    pub label: String,
    pub status: WorkflowGateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
}

/// Result of evaluating a workflow gate.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum WorkflowGateStatus {
    Passed,
    Warning,
    WaitingForApproval,
    Blocked,
}

/// Human approval checkpoint for a workflow action.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct WorkflowApprovalCheckpoint {
    pub id: String,
    pub label: String,
    pub reason: String,
    pub action: String,
    pub irreversible: bool,
}

/// Redacted audit event suitable for local history and UI display.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct WorkflowAuditEvent {
    pub id: String,
    pub kind: WorkflowAuditEventKind,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub created_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

/// Category for a workflow audit event.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum WorkflowAuditEventKind {
    StateChanged,
    ToolObserved,
    PolicyEvaluated,
    ApprovalRequested,
    ApprovalResolved,
    Blocked,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn workflow_snapshot_round_trips_as_camel_case_json() {
        let snapshot = WorkflowSnapshot {
            id: "workflow-1".to_string(),
            title: "Documentation review".to_string(),
            state: WorkflowStateKind::WaitingForApproval,
            boundary_status: WorkflowBoundaryStatus::LocalOnly,
            gates: vec![WorkflowGate {
                id: "gate-1".to_string(),
                label: "PHI boundary".to_string(),
                status: WorkflowGateStatus::Blocked,
                summary: Some("Outbound provider is not configured.".to_string()),
                source: Some("policy".to_string()),
            }],
            pending_approval: Some(WorkflowApprovalCheckpoint {
                id: "approval-1".to_string(),
                label: "Export draft".to_string(),
                reason: "Irreversible export requires human approval.".to_string(),
                action: "exportDraft".to_string(),
                irreversible: true,
            }),
            latest_audit_event: Some(WorkflowAuditEvent {
                id: "audit-1".to_string(),
                kind: WorkflowAuditEventKind::ApprovalRequested,
                summary: "Approval requested before export.".to_string(),
                created_at_ms: Some(1_764_610_800_000),
                metadata: BTreeMap::from([("tool".to_string(), "export".to_string())]),
            }),
        };

        let json = serde_json::to_value(&snapshot).expect("serialize snapshot");
        assert_eq!(json["boundaryStatus"], "localOnly");
        assert_eq!(json["pendingApproval"]["irreversible"], true);
        assert_eq!(json["latestAuditEvent"]["kind"], "approvalRequested");

        let round_trip: WorkflowSnapshot =
            serde_json::from_value(json).expect("deserialize snapshot");
        assert_eq!(round_trip, snapshot);
    }

    #[test]
    fn empty_audit_metadata_is_omitted() {
        let event = WorkflowAuditEvent {
            id: "audit-1".to_string(),
            kind: WorkflowAuditEventKind::StateChanged,
            summary: "Workflow started.".to_string(),
            created_at_ms: None,
            metadata: BTreeMap::new(),
        };

        let json = serde_json::to_value(event).expect("serialize event");
        assert_eq!(json.get("metadata"), None);
        assert_eq!(json.get("createdAtMs"), None);
    }
}
