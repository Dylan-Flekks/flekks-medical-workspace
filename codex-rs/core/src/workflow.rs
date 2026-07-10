use codex_protocol::items::TurnItem;
use codex_protocol::items::WorkflowStatusItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::workflow::WorkflowSnapshot;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::turn_timing::now_unix_timestamp_ms;

impl Session {
    pub(crate) async fn collect_workflow_snapshots(&self) -> Vec<WorkflowSnapshot> {
        let contributors = self.services.extensions.workflow_contributors().to_vec();
        let mut snapshots = Vec::new();
        for contributor in contributors {
            if let Some(snapshot) = contributor
                .snapshot(
                    &self.services.session_extension_data,
                    &self.services.thread_extension_data,
                )
                .await
            {
                snapshots.push(snapshot);
            }
        }
        snapshots
    }

    pub(crate) async fn emit_workflow_status_snapshots(&self, turn_context: &TurnContext) {
        for snapshot in self.collect_workflow_snapshots().await {
            let msg = EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: self.thread_id,
                turn_id: turn_context.sub_id.clone(),
                item: TurnItem::WorkflowStatus(WorkflowStatusItem::new(snapshot)),
                completed_at_ms: now_unix_timestamp_ms(),
            });
            self.services
                .rollout_thread_trace
                .record_codex_turn_event(&turn_context.sub_id, &msg);
            self.services
                .rollout_thread_trace
                .record_tool_call_event(turn_context.sub_id.clone(), &msg);
            self.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg,
            })
            .await;
        }
    }
}
