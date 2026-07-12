use super::*;
use codex_app_server_protocol::WorkspaceDataClassification;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionParams;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionResponse;
use codex_app_server_protocol::WorkspaceDataPolicyReadParams;
use codex_app_server_protocol::WorkspaceDataPolicyReadResponse;

pub(crate) const REMOTE_UNCLASSIFIED_MESSAGE: &str = "remote medical workspace is unclassified; provision it explicitly on the app-server because the TUI never classifies remote stores";
pub(crate) const LOCAL_UNCONFIGURED_MESSAGE: &str = "medical workspace is unclassified and this process lacks synthetic provisioning authority; restart with `just medical-workspace` (no chart data was changed)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceDataPolicyPreflight {
    Ready,
    ProvisionLocal,
    RejectRemote,
    RejectUnconfigured,
}

impl AppServerSession {
    pub(crate) async fn ensure_synthetic_workspace(&mut self) -> Result<()> {
        if self.workspace_synthetic_ready {
            return Ok(());
        }

        let read = self.workspace_data_policy_read().await?;
        match workspace_data_policy_preflight(
            read.policy.data_classification,
            read.synthetic_provisioning_enabled,
            self.uses_remote_workspace(),
        ) {
            WorkspaceDataPolicyPreflight::Ready => {
                self.workspace_synthetic_ready = true;
                Ok(())
            }
            WorkspaceDataPolicyPreflight::ProvisionLocal => {
                let response = self.workspace_data_policy_provision().await?;
                if response.policy.data_classification != WorkspaceDataClassification::Synthetic {
                    color_eyre::eyre::bail!(
                        "workspace provisioning returned without a synthetic data classification"
                    );
                }
                self.workspace_synthetic_ready = true;
                Ok(())
            }
            WorkspaceDataPolicyPreflight::RejectRemote => {
                color_eyre::eyre::bail!(REMOTE_UNCLASSIFIED_MESSAGE)
            }
            WorkspaceDataPolicyPreflight::RejectUnconfigured => {
                color_eyre::eyre::bail!(LOCAL_UNCONFIGURED_MESSAGE)
            }
        }
    }

    async fn workspace_data_policy_read(&mut self) -> Result<WorkspaceDataPolicyReadResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDataPolicyRead {
                request_id,
                params: WorkspaceDataPolicyReadParams {},
            })
            .await
            .wrap_err("workspace/dataPolicy/read failed in TUI")
    }

    async fn workspace_data_policy_provision(
        &mut self,
    ) -> Result<WorkspaceDataPolicyProvisionResponse> {
        let request_id = self.next_request_id();
        self.client
            .request_typed(ClientRequest::WorkspaceDataPolicyProvision {
                request_id,
                params: WorkspaceDataPolicyProvisionParams {},
            })
            .await
            .wrap_err("workspace/dataPolicy/provision failed in TUI")
    }
}

fn workspace_data_policy_preflight(
    classification: WorkspaceDataClassification,
    provisioning_enabled: bool,
    remote: bool,
) -> WorkspaceDataPolicyPreflight {
    match (classification, remote, provisioning_enabled) {
        (WorkspaceDataClassification::Synthetic, _, _) => WorkspaceDataPolicyPreflight::Ready,
        (WorkspaceDataClassification::Unclassified, true, _) => {
            WorkspaceDataPolicyPreflight::RejectRemote
        }
        (WorkspaceDataClassification::Unclassified, false, true) => {
            WorkspaceDataPolicyPreflight::ProvisionLocal
        }
        (WorkspaceDataClassification::Unclassified, false, false) => {
            WorkspaceDataPolicyPreflight::RejectUnconfigured
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_synthetic_policy_is_ready_on_every_transport() {
        for remote in [false, true] {
            for provisioning_enabled in [false, true] {
                assert_eq!(
                    workspace_data_policy_preflight(
                        WorkspaceDataClassification::Synthetic,
                        provisioning_enabled,
                        remote,
                    ),
                    WorkspaceDataPolicyPreflight::Ready
                );
            }
        }
    }

    #[test]
    fn only_configured_local_unclassified_store_can_provision() {
        assert_eq!(
            workspace_data_policy_preflight(WorkspaceDataClassification::Unclassified, true, false,),
            WorkspaceDataPolicyPreflight::ProvisionLocal
        );
        assert_eq!(
            workspace_data_policy_preflight(
                WorkspaceDataClassification::Unclassified,
                false,
                false,
            ),
            WorkspaceDataPolicyPreflight::RejectUnconfigured
        );
    }

    #[test]
    fn remote_unclassified_store_never_auto_provisions() {
        for provisioning_enabled in [false, true] {
            assert_eq!(
                workspace_data_policy_preflight(
                    WorkspaceDataClassification::Unclassified,
                    provisioning_enabled,
                    true,
                ),
                WorkspaceDataPolicyPreflight::RejectRemote
            );
        }
    }

    #[test]
    fn blocked_preflight_guidance_snapshot() {
        insta::assert_snapshot!(
            format!(
                "local: {LOCAL_UNCONFIGURED_MESSAGE}\nremote: {REMOTE_UNCLASSIFIED_MESSAGE}"
            ),
            @r"local: medical workspace is unclassified and this process lacks synthetic provisioning authority; restart with `just medical-workspace` (no chart data was changed)
        remote: remote medical workspace is unclassified; provision it explicitly on the app-server because the TUI never classifies remote stores"
        );
    }
}
