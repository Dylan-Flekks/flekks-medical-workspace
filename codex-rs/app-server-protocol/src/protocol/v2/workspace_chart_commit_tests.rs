use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn chart_commit_params_use_camel_case_and_allow_omitted_children() {
    let value = json!({
        "idempotencyKey": "save-1",
        "actor": "clinician-1",
        "reason": "daily note save",
        "sourceThreadId": "thread-1",
        "client": {
            "displayName": "Patient One",
            "summary": ""
        }
    });

    let params: WorkspaceChartCommitParams =
        serde_json::from_value(value).expect("chart commit params should deserialize");

    assert_eq!(params.idempotency_key, "save-1");
    assert_eq!(params.source_thread_id.as_deref(), Some("thread-1"));
    assert_eq!(params.source_turn_id, None);
    assert_eq!(params.client_id, None);
    assert_eq!(
        params
            .client
            .as_ref()
            .map(|client| client.display_name.as_str()),
        Some("Patient One")
    );
    assert_eq!(params.expected_versions, None);
    assert_eq!(params.encounter, None);
    assert_eq!(params.note, None);
}

#[test]
fn client_demographic_patch_distinguishes_omitted_null_and_value() {
    let omitted: WorkspaceClientUpsertParams = serde_json::from_value(json!({
        "displayName": "Patient One",
        "summary": ""
    }))
    .expect("omitted demographic fields");
    assert_eq!(omitted.legal_first_name, None);
    assert_eq!(omitted.interpreter_required, None);

    let explicit: WorkspaceClientUpsertParams = serde_json::from_value(json!({
        "displayName": "Patient One",
        "summary": "",
        "legalFirstName": null,
        "interpreterRequired": false
    }))
    .expect("explicit demographic patch fields");
    assert_eq!(explicit.legal_first_name, Some(None));
    assert_eq!(explicit.interpreter_required, Some(Some(false)));

    let value: WorkspaceClientUpsertParams = serde_json::from_value(json!({
        "displayName": "Patient One",
        "summary": "",
        "legalFirstName": "Avery"
    }))
    .expect("demographic patch value");
    assert_eq!(value.legal_first_name, Some(Some("Avery".to_string())));
}

#[test]
fn chart_commit_params_allow_existing_client_identity_without_snapshot() {
    let params: WorkspaceChartCommitParams = serde_json::from_value(json!({
        "idempotencyKey": "save-identity-only",
        "actor": "clinician-1",
        "reason": "daily note save",
        "clientId": "client-1",
        "note": {
            "upsert": {
                "id": "note-1",
                "clientId": "client-1",
                "title": "Daily note",
                "kind": "progress",
                "body": "Body",
                "status": "draft"
            },
            "expectedBaseRevision": 4
        }
    }))
    .expect("identity-only chart commit should deserialize");

    assert_eq!(params.client_id.as_deref(), Some("client-1"));
    assert_eq!(params.client, None);
}

#[test]
fn chart_commit_note_change_preserves_expected_revision() {
    let value = json!({
        "idempotencyKey": "save-2",
        "actor": "clinician-1",
        "reason": "daily note save",
        "client": {
            "id": "client-1",
            "displayName": "Patient One",
            "summary": ""
        },
        "note": {
            "upsert": {
                "id": "note-1",
                "clientId": "client-1",
                "encounterId": null,
                "title": "Daily note",
                "kind": "progress",
                "body": "Body",
                "status": "draft",
                "summary": null
            },
            "expectedBaseRevision": 4
        }
    });

    let params: WorkspaceChartCommitParams =
        serde_json::from_value(value).expect("chart commit params should deserialize");

    assert_eq!(
        params.note.map(|change| change.expected_base_revision),
        Some(Some(4))
    );
}

#[test]
fn chart_commit_kinds_and_error_data_serialize_for_clients() {
    assert_eq!(
        serde_json::to_value(WorkspaceChartEntityKind::Coverage)
            .expect("coverage kind should serialize"),
        json!("coverage")
    );
    assert_eq!(
        serde_json::to_value(WorkspaceChartEntityKind::ArtifactDerivative)
            .expect("entity kind should serialize"),
        json!("artifactDerivative")
    );
    assert_eq!(
        serde_json::to_value(WorkspaceChartCommitErrorData::StaleNoteRevision {
            note_id: "note-1".to_string(),
            expected_revision: 4,
            actual_revision: 5,
        })
        .expect("error data should serialize"),
        json!({
            "kind": "staleNoteRevision",
            "noteId": "note-1",
            "expectedRevision": 4,
            "actualRevision": 5
        })
    );
    assert_eq!(
        serde_json::to_value(WorkspaceChartCommitErrorData::StaleEntityVersion {
            entity_kind: WorkspaceChartEntityKind::Task,
            entity_id: "task-1".to_string(),
            expected_version: "version-old".to_string(),
            actual_version: "version-new".to_string(),
        })
        .expect("stale entity error should serialize"),
        json!({
            "kind": "staleEntityVersion",
            "entityKind": "task",
            "entityId": "task-1",
            "expectedVersion": "version-old",
            "actualVersion": "version-new"
        })
    );
}

#[test]
fn chart_versions_serialize_with_camel_case_tokens() {
    let client: super::WorkspaceClient = serde_json::from_value(json!({
        "id": "client-1",
        "version": "client-version",
        "displayName": "Patient One",
        "summary": "",
        "createdAt": 1,
        "updatedAt": 2
    }))
    .expect("workspace client should deserialize");
    assert_eq!(
        serde_json::to_value(client).expect("workspace client should serialize")["version"],
        json!("client-version")
    );

    let expected_versions = WorkspaceChartExpectedVersions {
        client: Some("client-version".to_string()),
        artifact_derivative: Some("derivative-version".to_string()),
        context_clip: Some("clip-version".to_string()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(expected_versions).expect("expected versions should serialize"),
        json!({
            "client": "client-version",
            "coverage": null,
            "safetyItem": null,
            "encounter": null,
            "document": null,
            "artifactDerivative": "derivative-version",
            "contextClip": "clip-version",
            "task": null
        })
    );
}
