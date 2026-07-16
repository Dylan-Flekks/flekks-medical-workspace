use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceDataClassification;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionOutcome;
use codex_app_server_protocol::WorkspaceDataPolicyProvisionResponse;
use codex_app_server_protocol::WorkspaceDataPolicyReadResponse;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;

const CLASSIFICATION_ENV: &str = "FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION";

struct PolicyHome {
    _root: TempDir,
    codex_home: PathBuf,
    sqlite_home: PathBuf,
}

impl PolicyHome {
    fn new(dedicated_sqlite_home: bool) -> Result<Self> {
        let root = TempDir::new()?;
        let codex_home = root.path().join("codex-home");
        let sqlite_home = if dedicated_sqlite_home {
            root.path().join("medical-sqlite-home")
        } else {
            codex_home.clone()
        };
        std::fs::create_dir_all(&codex_home)?;
        std::fs::create_dir_all(&sqlite_home)?;
        create_config_toml(&codex_home)?;
        Ok(Self {
            _root: root,
            codex_home,
            sqlite_home,
        })
    }

    async fn start(
        &self,
        classification: Option<&str>,
        sqlite_home_env: Option<&Path>,
    ) -> Result<TestAppServer> {
        let configured_sqlite_home = self.sqlite_home.to_string_lossy().into_owned();
        let sqlite_override = format!(
            "sqlite_home={}",
            serde_json::to_string(&configured_sqlite_home)?
        );
        let sqlite_home_env = sqlite_home_env.map(|path| path.to_string_lossy().into_owned());
        let mut server = TestAppServer::builder()
            .with_codex_home(&self.codex_home)
            .without_auto_env()
            .with_env_overrides(&[
                (CLASSIFICATION_ENV, classification),
                (codex_state::SQLITE_HOME_ENV, sqlite_home_env.as_deref()),
            ])
            .with_args(&["-c", sqlite_override.as_str()])
            .build()
            .await?;
        timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;
        Ok(server)
    }
}

#[tokio::test]
async fn dedicated_exact_launch_provisions_once_and_persists_across_restart() -> Result<()> {
    let home = PolicyHome::new(true)?;
    let mut server = home
        .start(Some("synthetic"), Some(&home.sqlite_home))
        .await?;

    let initial: WorkspaceDataPolicyReadResponse =
        request(&mut server, "workspace/dataPolicy/read", json!({})).await?;
    assert_eq!(
        initial.policy.data_classification,
        WorkspaceDataClassification::Unclassified
    );
    assert!(initial.synthetic_provisioning_enabled);

    let provisioned: WorkspaceDataPolicyProvisionResponse =
        request(&mut server, "workspace/dataPolicy/provision", json!({})).await?;
    assert_eq!(
        provisioned.outcome,
        WorkspaceDataPolicyProvisionOutcome::Provisioned
    );
    assert_eq!(
        provisioned.policy.data_classification,
        WorkspaceDataClassification::Synthetic
    );
    assert_eq!(
        provisioned.policy.classified_by.as_deref(),
        Some("app-server:FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION")
    );

    let replayed: WorkspaceDataPolicyProvisionResponse =
        request(&mut server, "workspace/dataPolicy/provision", json!({})).await?;
    assert_eq!(
        replayed.outcome,
        WorkspaceDataPolicyProvisionOutcome::AlreadySynthetic
    );
    assert_eq!(replayed.policy, provisioned.policy);
    drop(server);

    let mut restarted = home.start(None, None).await?;
    let persisted: WorkspaceDataPolicyReadResponse =
        request(&mut restarted, "workspace/dataPolicy/read", json!({})).await?;
    assert_eq!(persisted.policy, provisioned.policy);
    assert!(!persisted.synthetic_provisioning_enabled);
    Ok(())
}

#[tokio::test]
async fn provisioning_requires_exact_classification_and_matching_dedicated_home() -> Result<()> {
    for classification in [None, Some("Synthetic"), Some(" synthetic")] {
        let home = PolicyHome::new(true)?;
        let mut server = home.start(classification, Some(&home.sqlite_home)).await?;
        assert_provisioning_disabled(&mut server).await?;
    }

    let missing_sqlite_env = PolicyHome::new(true)?;
    let mut server = missing_sqlite_env.start(Some("synthetic"), None).await?;
    assert_provisioning_disabled(&mut server).await?;

    let relative_sqlite_env = PolicyHome::new(true)?;
    let mut server = relative_sqlite_env
        .start(Some("synthetic"), Some(Path::new("relative-sqlite-home")))
        .await?;
    assert_provisioning_disabled(&mut server).await?;

    let mismatched = PolicyHome::new(true)?;
    let other_home = mismatched._root.path().join("other-sqlite-home");
    std::fs::create_dir_all(&other_home)?;
    let mut server = mismatched
        .start(Some("synthetic"), Some(&other_home))
        .await?;
    assert_provisioning_disabled(&mut server).await?;

    let shared = PolicyHome::new(false)?;
    let mut server = shared
        .start(Some("synthetic"), Some(&shared.sqlite_home))
        .await?;
    assert_provisioning_disabled(&mut server).await?;
    Ok(())
}

#[tokio::test]
async fn nonempty_store_and_caller_supplied_authority_fail_closed() -> Result<()> {
    let home = PolicyHome::new(true)?;
    let mut server = home
        .start(Some("synthetic"), Some(&home.sqlite_home))
        .await?;

    let _: WorkspaceClientUpsertResponse = request(
        &mut server,
        "workspace/client/upsert",
        json!({"displayName": "Synthetic pre-policy record", "summary": ""}),
    )
    .await?;
    let nonempty = request_error(&mut server, "workspace/dataPolicy/provision", json!({})).await?;
    assert!(
        nonempty
            .error
            .message
            .contains("cannot change after workspace records exist")
    );
    let unchanged: WorkspaceDataPolicyReadResponse =
        request(&mut server, "workspace/dataPolicy/read", json!({})).await?;
    assert_eq!(
        unchanged.policy.data_classification,
        WorkspaceDataClassification::Unclassified
    );

    for (method, params) in [
        (
            "workspace/dataPolicy/read",
            json!({"classification": "synthetic"}),
        ),
        (
            "workspace/dataPolicy/provision",
            json!({"actor": "caller", "sqliteHome": "/tmp/not-authority"}),
        ),
    ] {
        let error = request_error(&mut server, method, params).await?;
        assert!(error.error.message.contains("Invalid request"));
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn provisioning_resolves_dedicated_and_shared_home_symlink_aliases() -> Result<()> {
    let dedicated = PolicyHome::new(true)?;
    let dedicated_alias = dedicated._root.path().join("medical-sqlite-alias");
    std::os::unix::fs::symlink(&dedicated.sqlite_home, &dedicated_alias)?;
    let mut server = dedicated
        .start(Some("synthetic"), Some(&dedicated_alias))
        .await?;
    let enabled: WorkspaceDataPolicyReadResponse =
        request(&mut server, "workspace/dataPolicy/read", json!({})).await?;
    assert!(enabled.synthetic_provisioning_enabled);
    let provisioned: WorkspaceDataPolicyProvisionResponse =
        request(&mut server, "workspace/dataPolicy/provision", json!({})).await?;
    assert_eq!(
        provisioned.outcome,
        WorkspaceDataPolicyProvisionOutcome::Provisioned
    );

    let shared = PolicyHome::new(false)?;
    let shared_alias = shared._root.path().join("codex-home-alias");
    std::os::unix::fs::symlink(&shared.codex_home, &shared_alias)?;
    let mut server = shared.start(Some("synthetic"), Some(&shared_alias)).await?;
    assert_provisioning_disabled(&mut server).await?;
    Ok(())
}

async fn assert_provisioning_disabled(server: &mut TestAppServer) -> Result<()> {
    let read: WorkspaceDataPolicyReadResponse =
        request(server, "workspace/dataPolicy/read", json!({})).await?;
    assert_eq!(
        read.policy.data_classification,
        WorkspaceDataClassification::Unclassified
    );
    assert!(!read.synthetic_provisioning_enabled);
    let error = request_error(server, "workspace/dataPolicy/provision", json!({})).await?;
    assert!(
        error
            .error
            .message
            .contains("dedicated synthetic-only launcher configuration")
    );
    Ok(())
}

async fn request<T: DeserializeOwned>(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<T> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn request_error(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<JSONRPCError> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}
