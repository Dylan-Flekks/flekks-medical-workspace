use super::*;
use codex_app_server_protocol::WorkspaceDataClassification;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionOutcome;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionParams;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionResponse;
use codex_app_server_protocol::WorkspaceDataPolicyReadParams;
use codex_app_server_protocol::WorkspaceDataPolicyReadResponse;
use codex_app_server_protocol::WorkspaceDataPolicyStatus;

const SYNTHETIC_CLASSIFICATION_ENV: &str = "FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION";
const SYNTHETIC_CLASSIFICATION_VALUE: &str = "synthetic";
const SYNTHETIC_CLASSIFIED_BY: &str = "app-server:FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION";

#[derive(Clone, Copy)]
pub(super) struct WorkspaceSyntheticProvisioningAuthority {
    available: bool,
}

impl WorkspaceSyntheticProvisioningAuthority {
    pub(super) fn from_config(config: &codex_core::config::Config) -> Self {
        let classification_is_synthetic = std::env::var(SYNTHETIC_CLASSIFICATION_ENV)
            .is_ok_and(|value| value == SYNTHETIC_CLASSIFICATION_VALUE);
        let sqlite_home_matches = std::env::var(codex_state::SQLITE_HOME_ENV)
            .ok()
            .map(std::path::PathBuf::from)
            .is_some_and(|path| {
                path.is_absolute()
                    && paths_resolve_to_same_location(&path, &config.sqlite_home)
                    && !paths_resolve_to_same_location(&path, config.codex_home.as_path())
            });
        Self {
            available: classification_is_synthetic && sqlite_home_matches,
        }
    }
}

fn paths_resolve_to_same_location(left: &std::path::Path, right: &std::path::Path) -> bool {
    if left == right {
        return true;
    }
    std::fs::canonicalize(left)
        .ok()
        .zip(std::fs::canonicalize(right).ok())
        .is_some_and(|(left, right)| left == right)
}

impl WorkspaceRequestProcessor {
    pub(crate) async fn data_policy_read(
        &self,
        _params: WorkspaceDataPolicyReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let status = self
            .state_db()?
            .workspace()
            .workspace_data_policy_status()
            .await
            .map_err(|err| {
                internal_error(format!("failed to read workspace data policy: {err}"))
            })?;
        Ok(Some(
            WorkspaceDataPolicyReadResponse {
                policy: api_policy_status(status),
                synthetic_provisioning_enabled: self.synthetic_provisioning_authority.available,
            }
            .into(),
        ))
    }

    pub(crate) async fn data_policy_provision(
        &self,
        _params: WorkspaceDataPolicyProvisionParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if !self.synthetic_provisioning_authority.available {
            return Err(invalid_request(
                "workspace synthetic provisioning requires the dedicated synthetic-only launcher configuration",
            ));
        }
        let outcome = self
            .state_db()?
            .workspace()
            .provision_synthetic_workspace(SYNTHETIC_CLASSIFIED_BY)
            .await
            .map_err(api_provision_error)?;
        let (outcome, status) = match outcome {
            codex_state::WorkspaceSyntheticProvisionOutcome::Provisioned(status) => {
                (WorkspaceDataPolicyProvisionOutcome::Provisioned, status)
            }
            codex_state::WorkspaceSyntheticProvisionOutcome::AlreadySynthetic(status) => (
                WorkspaceDataPolicyProvisionOutcome::AlreadySynthetic,
                status,
            ),
        };
        Ok(Some(
            WorkspaceDataPolicyProvisionResponse {
                policy: api_policy_status(status),
                outcome,
                synthetic_provisioning_enabled: true,
            }
            .into(),
        ))
    }
}

fn api_provision_error(error: codex_state::WorkspaceSyntheticProvisionError) -> JSONRPCErrorError {
    match error {
        codex_state::WorkspaceSyntheticProvisionError::Validation { message }
        | codex_state::WorkspaceSyntheticProvisionError::Conflict { message } => invalid_request(
            format!("workspace synthetic provisioning failed: {message}"),
        ),
        codex_state::WorkspaceSyntheticProvisionError::Storage { message } => internal_error(
            format!("workspace synthetic provisioning failed: {message}"),
        ),
    }
}

fn api_policy_status(value: codex_state::WorkspaceDataPolicyStatus) -> WorkspaceDataPolicyStatus {
    WorkspaceDataPolicyStatus {
        schema_version: value.schema_version,
        data_classification: match value.data_classification {
            codex_state::WorkspaceDataClassification::Unclassified => {
                WorkspaceDataClassification::Unclassified
            }
            codex_state::WorkspaceDataClassification::Synthetic => {
                WorkspaceDataClassification::Synthetic
            }
        },
        classified_at: value.classified_at.map(|value| value.timestamp()),
        classified_by: value.classified_by,
    }
}
