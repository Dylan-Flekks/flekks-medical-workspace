use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;

#[test]
fn draft_session_list_preserves_patient_scope_and_requires_explicit_global_wire_intent() {
    let patient: WorkspaceDraftSessionListParams = serde_json::from_value(json!({
        "clientId": "patient-1",
    }))
    .expect("existing patient-scoped request should decode");
    assert_eq!(
        patient,
        WorkspaceDraftSessionListParams {
            client_id: Some("patient-1".to_string()),
            all_clients: false,
            include_closed: false,
            cursor: None,
            limit: None,
        }
    );

    let global: WorkspaceDraftSessionListParams = serde_json::from_value(json!({
        "allClients": true,
        "limit": 1,
    }))
    .expect("explicit global request should decode");
    assert_eq!(
        global,
        WorkspaceDraftSessionListParams {
            client_id: None,
            all_clients: true,
            include_closed: false,
            cursor: None,
            limit: Some(1),
        }
    );
}

#[test]
fn stable_schema_filters_global_draft_discovery_and_experimental_schema_exposes_it() {
    let stable_dir = tempfile::tempdir().expect("stable schema directory should create");
    crate::generate_json_with_experimental(stable_dir.path(), /*experimental_api*/ false)
        .expect("stable schema should generate");
    let stable = fs::read_to_string(stable_dir.path().join("ClientRequest.json"))
        .expect("stable client request schema should read");
    assert!(!stable.contains("workspace/draft/session/list"));

    let experimental_dir =
        tempfile::tempdir().expect("experimental schema directory should create");
    crate::generate_json_with_experimental(experimental_dir.path(), /*experimental_api*/ true)
        .expect("experimental schema should generate");
    let experimental = fs::read_to_string(experimental_dir.path().join("ClientRequest.json"))
        .expect("experimental client request schema should read");
    assert!(experimental.contains("workspace/draft/session/list"));
    assert!(experimental.contains("allClients"));
}
