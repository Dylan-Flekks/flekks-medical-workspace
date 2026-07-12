//! Typed app-server calls for local coverage and human card verification.

use super::AppServerSession;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::WorkspaceCoverageListParams;
use codex_app_server_protocol::WorkspaceCoverageListResponse;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateParams;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateResponse;
use codex_app_server_protocol::WorkspaceCoverageVerificationListParams;
use codex_app_server_protocol::WorkspaceCoverageVerificationListResponse;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;

impl AppServerSession {
    pub(crate) async fn workspace_coverage_list(
        &mut self,
        params: WorkspaceCoverageListParams,
    ) -> Result<WorkspaceCoverageListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceCoverageList { request_id, params })
            .await
            .wrap_err("workspace/coverage/list failed in TUI")
    }

    pub(crate) async fn workspace_coverage_verification_list(
        &mut self,
        params: WorkspaceCoverageVerificationListParams,
    ) -> Result<WorkspaceCoverageVerificationListResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceCoverageVerificationList {
                request_id,
                params,
            })
            .await
            .wrap_err("workspace/coverage/verification/list failed in TUI")
    }

    pub(crate) async fn workspace_coverage_verification_create(
        &mut self,
        params: WorkspaceCoverageVerificationCreateParams,
    ) -> Result<WorkspaceCoverageVerificationCreateResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceCoverageVerificationCreate {
                request_id,
                params,
            })
            .await
            .wrap_err("workspace/coverage/verification/create failed in TUI")
    }
}
