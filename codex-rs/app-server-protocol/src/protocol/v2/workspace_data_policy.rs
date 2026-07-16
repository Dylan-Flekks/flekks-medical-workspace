use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceDataClassification {
    Unclassified,
    Synthetic,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDataPolicyStatus {
    #[ts(type = "number")]
    pub schema_version: i64,
    pub data_classification: WorkspaceDataClassification,
    #[ts(type = "number | null")]
    pub classified_at: Option<i64>,
    pub classified_by: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct WorkspaceDataPolicyReadParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDataPolicyReadResponse {
    pub policy: WorkspaceDataPolicyStatus,
    pub synthetic_provisioning_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct WorkspaceDataPolicyProvisionParams {}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceDataPolicyProvisionOutcome {
    Provisioned,
    AlreadySynthetic,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDataPolicyProvisionResponse {
    pub policy: WorkspaceDataPolicyStatus,
    pub outcome: WorkspaceDataPolicyProvisionOutcome,
    pub synthetic_provisioning_enabled: bool,
}

#[cfg(test)]
#[path = "workspace_data_policy_tests.rs"]
mod tests;
