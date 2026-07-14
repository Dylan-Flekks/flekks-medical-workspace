//! Typed app-server calls for local workspace draft recovery.

use super::AppServerSession;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpointListParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointListResponse;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseResponse;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use codex_app_server_protocol::WorkspaceDraftSessionListResponse;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

impl AppServerSession {
    pub(crate) async fn workspace_draft_checkpoint_create(
        &mut self,
        params: WorkspaceDraftCheckpointCreateParams,
    ) -> Result<WorkspaceDraftCheckpointCreateResponse> {
        self.ensure_synthetic_workspace().await?;
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDraftCheckpointCreate { request_id, params })
            .await
            .wrap_err("workspace/draft/checkpoint/create failed in TUI")
    }

    #[allow(dead_code)]
    pub(crate) async fn workspace_draft_checkpoint_list(
        &mut self,
        params: WorkspaceDraftCheckpointListParams,
    ) -> Result<WorkspaceDraftCheckpointListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDraftCheckpointList { request_id, params })
            .await
            .wrap_err("workspace/draft/checkpoint/list failed in TUI")
    }

    pub(crate) async fn workspace_draft_session_list(
        &mut self,
        params: WorkspaceDraftSessionListParams,
    ) -> Result<WorkspaceDraftSessionListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDraftSessionList { request_id, params })
            .await
            .wrap_err("workspace/draft/session/list failed in TUI")
    }

    pub(crate) async fn workspace_draft_session_close(
        &mut self,
        params: WorkspaceDraftSessionCloseParams,
    ) -> Result<WorkspaceDraftSessionCloseResponse> {
        self.ensure_synthetic_workspace().await?;
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDraftSessionClose { request_id, params })
            .await
            .wrap_err("workspace/draft/session/close failed in TUI")
    }
}
