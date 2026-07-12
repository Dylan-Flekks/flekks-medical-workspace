use chrono::DateTime;
use chrono::Utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceDataClassification {
    Unclassified,
    Synthetic,
}

impl WorkspaceDataClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unclassified => "unclassified",
            Self::Synthetic => "synthetic",
        }
    }

    pub(crate) fn from_stored(value: &str) -> anyhow::Result<Self> {
        match value {
            "unclassified" => Ok(Self::Unclassified),
            "synthetic" => Ok(Self::Synthetic),
            other => anyhow::bail!("unknown stored workspace data classification `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDataPolicyStatus {
    pub schema_version: i64,
    pub data_classification: WorkspaceDataClassification,
    pub classified_at: Option<DateTime<Utc>>,
    pub classified_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSyntheticProvisionOutcome {
    Provisioned(WorkspaceDataPolicyStatus),
    AlreadySynthetic(WorkspaceDataPolicyStatus),
}
