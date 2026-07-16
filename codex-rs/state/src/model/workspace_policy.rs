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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceSyntheticProvisionError {
    Validation { message: String },
    Conflict { message: String },
    Storage { message: String },
}

impl std::fmt::Display for WorkspaceSyntheticProvisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation { message }
            | Self::Conflict { message }
            | Self::Storage { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WorkspaceSyntheticProvisionError {}

impl From<anyhow::Error> for WorkspaceSyntheticProvisionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<sqlx::Error> for WorkspaceSyntheticProvisionError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}
