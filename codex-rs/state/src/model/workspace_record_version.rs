use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

fn record_version<T: Serialize>(domain: &str, value: &T) -> anyhow::Result<String> {
    let serialized = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(b"workspace-record-version:v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(serialized);
    Ok(format!("{:x}", hasher.finalize()))
}

macro_rules! impl_record_version {
    ($type:ty, $domain:literal) => {
        impl $type {
            pub fn record_version(&self) -> anyhow::Result<String> {
                record_version($domain, self)
            }
        }
    };
}

impl_record_version!(super::WorkspaceClient, "client");
impl_record_version!(super::WorkspaceCoverage, "coverage");
impl_record_version!(super::WorkspacePatientSafetyItem, "patient-safety-item");
impl_record_version!(super::WorkspaceEncounter, "encounter");
impl_record_version!(super::WorkspaceDocument, "document");
impl_record_version!(super::WorkspaceArtifactDerivative, "artifact-derivative");
impl_record_version!(super::WorkspaceContextClip, "context-clip");
impl_record_version!(super::WorkspaceTask, "task");
