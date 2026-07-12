use super::workspace_chart_commit::create_config_toml;
use super::workspace_chart_commit::send_client_update;
use super::workspace_chart_commit::send_commit;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceBillingReadiness;
use codex_app_server_protocol::WorkspaceCoverageListResponse;
use codex_app_server_protocol::WorkspaceCoverageMatchResult;
use codex_app_server_protocol::WorkspaceCoverageVerificationCreateResponse;
use codex_app_server_protocol::WorkspaceCoverageVerificationListResponse;
use codex_app_server_protocol::WorkspaceDocumentUpsertResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const COVERAGE_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[tokio::test]
async fn coverage_rpc_round_trips_structured_demographics_and_card_verification() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(COVERAGE_READ_TIMEOUT, server.initialize()).await??;

    let client = send_client_update(
        &mut server,
        json!({
            "displayName": "Avery Test",
            "legalFirstName": "Avery",
            "legalMiddleName": "Q",
            "legalLastName": "Test",
            "previousName": "Avery Previous",
            "dateOfBirth": "1980-04-11",
            "administrativeSex": "female",
            "preferredLanguage": "English",
            "interpreterRequired": false,
            "summary": "synthetic patient",
            "primaryPhone": "312-555-0100",
            "primaryPhoneUse": "mobile",
            "primaryEmail": "avery@example.test",
            "secondaryEmail": "avery.secondary@example.test",
            "addressLine1": "100 Test Street",
            "city": "Testville",
            "stateOrProvince": "IL",
            "postalCode": "60601",
            "country": "US",
            "addressUse": "home_and_mailing",
            "emergencyContactName": "Jordan Test",
            "emergencyContactRelationship": "spouse",
            "emergencyContactPhone": "312-555-0101",
            "payerName": "Medicare",
            "memberId": "1EG4TE5MK73",
            "coverageType": "medicare",
            "coverageStatus": "active"
        }),
    )
    .await?
    .client;
    assert_eq!(client.legal_first_name.as_deref(), Some("Avery"));
    assert_eq!(client.primary_email.as_deref(), Some("avery@example.test"));
    assert_eq!(client.email, client.primary_email);
    assert_eq!(client.address_line_1.as_deref(), Some("100 Test Street"));

    let page = list_coverages(&mut server, &client.id, None, Some(1)).await?;
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].priority, 1);
    assert_eq!(
        page.data[0].billing_readiness,
        WorkspaceBillingReadiness::Unverified
    );
    let primary = page.data[0].clone();

    let document = upsert_document(
        &mut server,
        json!({
            "clientId": client.id,
            "title": "Synthetic Medicare card",
            "kind": "insurance_card",
            "localPath": "/synthetic/card.png",
            "notes": "",
            "scope": "patient",
            "detectedKind": "insurance_card",
            "tags": "",
            "sourceLabel": "synthetic",
            "existenceStatus": "present",
            "metadataJson": "{}",
            "originalPath": "/synthetic/card.png",
            "referenceKind": "local_reference",
            "vaultPath": "",
            "contentSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "thumbnailPath": "",
            "thumbnailStatus": "none",
            "intakeSource": "test"
        }),
    )
    .await?
    .document;
    let matched = create_verification(
        &mut server,
        json!({
            "coverageId": primary.id,
            "sourceDocumentId": document.id,
            "expectedPatientVersion": client.version,
            "expectedCoverageVersion": primary.version,
            "expectedDocumentVersion": document.version,
            "comparedSubject": "beneficiary",
            "observedFirstName": "avery",
            "observedMiddleName": "Q",
            "observedLastName": "TEST",
            "observedMemberId": "1EG4-TE5-MK73",
            "actor": "Dr. Test"
        }),
    )
    .await?;
    assert_eq!(
        matched.verification.match_result,
        WorkspaceCoverageMatchResult::Match
    );
    assert_eq!(matched.billing_readiness, WorkspaceBillingReadiness::Match);

    send_client_update(
        &mut server,
        json!({
            "id": client.id,
            "expectedVersion": client.version,
            "displayName": "Avery Changed",
            "legalFirstName": "Avery",
            "legalMiddleName": "Q",
            "legalLastName": "Changed",
            "dateOfBirth": "1980-04-11",
            "administrativeSex": "female",
            "summary": "synthetic patient",
            "primaryPhone": "312-555-0100",
            "addressLine1": "100 Test Street",
            "city": "Testville",
            "stateOrProvince": "IL",
            "postalCode": "60601",
            "country": "US",
            "payerName": "Medicare",
            "memberId": "1EG4TE5MK73",
            "coverageType": "medicare",
            "coverageStatus": "active"
        }),
    )
    .await?;
    let history = list_verifications(&mut server, &primary.id).await?;
    assert_eq!(history.data.len(), 1);
    assert!(history.data[0].is_stale);
    let stale_page = list_coverages(&mut server, &client.id, None, None).await?;
    assert_eq!(
        stale_page.data[0].billing_readiness,
        WorkspaceBillingReadiness::Stale
    );
    Ok(())
}

#[tokio::test]
async fn direct_client_update_uses_cas_and_patch_semantics_without_touching_coverage() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(COVERAGE_READ_TIMEOUT, server.initialize()).await??;
    let created = send_client_update(
        &mut server,
        json!({
            "displayName": "Patch Patient",
            "legalFirstName": "Patch",
            "legalLastName": "Patient",
            "secondaryEmail": "secondary@example.test",
            "addressLine1": "100 Test Street",
            "summary": "synthetic",
            "payerName": "Synthetic Payer",
            "memberId": "SYN-100",
            "coverageStatus": "active"
        }),
    )
    .await?
    .client;
    let coverage_before = list_coverages(&mut server, &created.id, None, None)
        .await?
        .data
        .remove(0);

    let error = send_client_update_error(
        &mut server,
        json!({
            "id": created.id,
            "displayName": "Missing CAS",
            "summary": "synthetic"
        }),
    )
    .await?;
    assert!(error.error.message.contains("expectedVersion is required"));

    let preserved = send_client_update(
        &mut server,
        json!({
            "id": created.id,
            "expectedVersion": created.version,
            "displayName": "Patched Display",
            "summary": "synthetic"
        }),
    )
    .await?
    .client;
    assert_eq!(preserved.legal_first_name.as_deref(), Some("Patch"));
    assert_eq!(preserved.legal_last_name.as_deref(), Some("Patient"));
    assert_eq!(
        preserved.secondary_email.as_deref(),
        Some("secondary@example.test")
    );
    assert_eq!(preserved.address_line_1.as_deref(), Some("100 Test Street"));
    let coverage_after = list_coverages(&mut server, &created.id, None, None)
        .await?
        .data
        .remove(0);
    assert_eq!(coverage_after, coverage_before);

    let alias_error = send_client_update_error(
        &mut server,
        json!({
            "id": preserved.id,
            "expectedVersion": preserved.version,
            "displayName": preserved.display_name,
            "summary": preserved.summary,
            "email": "legacy@example.test",
            "primaryEmail": null
        }),
    )
    .await?;
    assert!(alias_error.error.message.contains("must match"));

    let cleared = send_client_update(
        &mut server,
        json!({
            "id": preserved.id,
            "expectedVersion": preserved.version,
            "displayName": preserved.display_name,
            "summary": preserved.summary,
            "secondaryEmail": null
        }),
    )
    .await?
    .client;
    assert_eq!(cleared.secondary_email, None);
    assert_eq!(cleared.legal_first_name.as_deref(), Some("Patch"));
    Ok(())
}

#[tokio::test]
async fn chart_commit_adds_secondary_coverage_with_independent_version() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(COVERAGE_READ_TIMEOUT, server.initialize()).await??;
    let client = send_client_update(
        &mut server,
        json!({
            "displayName": "Dependent Patient",
            "legalFirstName": "Dependent",
            "legalLastName": "Patient",
            "dateOfBirth": "2010-02-03",
            "administrativeSex": "female",
            "summary": "synthetic patient"
        }),
    )
    .await?
    .client;
    let committed = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "secondary-coverage-create",
            "actor": "Dr. Test",
            "reason": "add secondary coverage",
            "clientId": client.id,
            "coverage": {
                "clientId": client.id,
                "priority": 2,
                "payerName": "Synthetic Secondary",
                "memberId": "SYN-200",
                "coverageStatus": "active",
                "patientRelationshipToSubscriber": "child",
                "subscriberFirstName": "Jordan",
                "subscriberLastName": "Subscriber",
                "subscriberDateOfBirth": "1970-01-01",
                "subscriberAdministrativeSex": "male",
                "subscriberAddressSameAsPatient": true
            }
        }),
    )
    .await?;
    let secondary = committed.coverage.expect("coverage response");
    assert_eq!(secondary.priority, 2);
    assert!(!secondary.version.is_empty());
    assert_eq!(
        secondary.billing_readiness,
        WorkspaceBillingReadiness::Incomplete
    );

    let first_page = list_coverages(&mut server, &client.id, None, Some(1)).await?;
    assert_eq!(first_page.data, vec![secondary]);
    assert_eq!(first_page.next_cursor, None);
    Ok(())
}

async fn list_coverages(
    server: &mut TestAppServer,
    client_id: &str,
    cursor: Option<&str>,
    limit: Option<u32>,
) -> Result<WorkspaceCoverageListResponse> {
    send_response(
        server,
        "workspace/coverage/list",
        json!({ "clientId": client_id, "cursor": cursor, "limit": limit }),
    )
    .await
}

async fn list_verifications(
    server: &mut TestAppServer,
    coverage_id: &str,
) -> Result<WorkspaceCoverageVerificationListResponse> {
    send_response(
        server,
        "workspace/coverage/verification/list",
        json!({ "coverageId": coverage_id }),
    )
    .await
}

async fn create_verification(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceCoverageVerificationCreateResponse> {
    send_response(
        server,
        "workspace/coverage/verification/create",
        params,
    )
    .await
}

async fn upsert_document(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceDocumentUpsertResponse> {
    send_response(server, "workspace/document/upsert", params).await
}

async fn send_response<T: serde::de::DeserializeOwned>(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<T> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    let response: JSONRPCResponse = timeout(
        COVERAGE_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn send_client_update_error(
    server: &mut TestAppServer,
    params: Value,
) -> Result<JSONRPCError> {
    let request_id = server
        .send_raw_request("workspace/client/upsert", Some(params))
        .await?;
    timeout(
        COVERAGE_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}
