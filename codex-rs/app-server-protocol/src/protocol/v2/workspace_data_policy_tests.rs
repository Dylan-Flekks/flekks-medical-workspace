use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn data_policy_response_uses_explicit_camel_case_fields() {
    let response = WorkspaceDataPolicyProvisionResponse {
        policy: WorkspaceDataPolicyStatus {
            schema_version: 1,
            data_classification: WorkspaceDataClassification::Synthetic,
            classified_at: Some(42),
            classified_by: Some(
                "app-server:FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION".to_string(),
            ),
        },
        outcome: WorkspaceDataPolicyProvisionOutcome::Provisioned,
        synthetic_provisioning_enabled: true,
    };

    assert_eq!(
        serde_json::to_value(response).expect("policy response should serialize"),
        json!({
            "policy": {
                "schemaVersion": 1,
                "dataClassification": "synthetic",
                "classifiedAt": 42,
                "classifiedBy": "app-server:FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION"
            },
            "outcome": "provisioned",
            "syntheticProvisioningEnabled": true
        })
    );
}

#[test]
fn data_policy_requests_reject_client_supplied_authority() {
    assert!(
        serde_json::from_value::<WorkspaceDataPolicyReadParams>(json!({
            "classification": "synthetic"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WorkspaceDataPolicyProvisionParams>(json!({
            "actor": "caller",
            "sqliteHome": "/tmp/not-authority"
        }))
        .is_err()
    );
}
