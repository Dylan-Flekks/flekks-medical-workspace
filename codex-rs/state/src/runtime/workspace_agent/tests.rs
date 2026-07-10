use crate::StateRuntime;
use crate::migrations::WORKSPACE_MIGRATOR;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteConnectOptions;
use std::borrow::Cow;

async fn test_runtime() -> std::sync::Arc<StateRuntime> {
    StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize")
}

async fn seed_client_note(
    runtime: &StateRuntime,
) -> (crate::WorkspaceClient, crate::WorkspaceNote) {
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Jordan Patient".to_string(),
            summary: "Synthetic patient for agent provenance tests.".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            title: "Daily note".to_string(),
            kind: "daily".to_string(),
            body: "Human note revision one.".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("note should save");
    (client, note)
}

fn packet_envelope(request: &str, note_revision: i64) -> String {
    serde_json::json!({
        "assemblyVersion": "medical-context-packet-test-v2",
        "sourceMode": "agent_request",
        "includeDocuments": false,
        "humanRequest": request,
        "ids": {
            "selectedArtifactIds": [],
            "selectedDerivativeIds": [],
            "selectedClipIds": [],
        },
        "note": {
            "revision": note_revision,
        },
        "safety": [
            "read-only context packet; do not mutate workspace records",
            "do not sign notes, submit claims, send payer communications, or overwrite saved data",
        ],
        "promptSnapshot": "Synthetic packet snapshot without filesystem paths.",
    })
    .to_string()
}

fn packet_create(
    client: &crate::WorkspaceClient,
    note: &crate::WorkspaceNote,
) -> crate::WorkspaceContextPacketCreate {
    let request = "Generate a daily-note template from authorized history.";
    crate::WorkspaceContextPacketCreate {
        client_id: client.id.clone(),
        note_id: Some(note.id.clone()),
        human_request: request.to_string(),
        selected_artifact_ids_json: "[]".to_string(),
        selected_derivative_ids_json: "[]".to_string(),
        selected_clip_ids_json: "[]".to_string(),
        artifact_summary: "0 selected files".to_string(),
        derivative_summary: "0 selected reviewed text items".to_string(),
        clip_summary: "0 selected clips".to_string(),
        chart_context_summary: "synthetic patient; daily note".to_string(),
        context_envelope_json: packet_envelope(request, note.current_revision),
        base_note_revision: Some(note.current_revision),
        authorized_scope_json: serde_json::json!({
            "version": 1,
            "categories": ["active_note", "prior_notes", "visit_history"],
            "noteKinds": ["daily", "progress"],
            "maxRecords": 25,
        })
        .to_string(),
        expected_output_kind: "template_proposal".to_string(),
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

fn run_start(packet: &crate::WorkspaceContextPacket) -> crate::WorkspaceAgentRunStart {
    crate::WorkspaceAgentRunStart {
        packet_id: packet.id.clone(),
        expected_client_id: packet.client_id.clone(),
        expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
        run_kind: "agent".to_string(),
        idempotency_key: "turn:synthetic-1".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
        source_thread_id: Some("thread-synthetic".to_string()),
        source_turn_id: None,
        actor: "Clinician Example".to_string(),
    }
}

fn result_create(
    packet: &crate::WorkspaceContextPacket,
    run: &crate::WorkspaceAgentRun,
) -> crate::WorkspaceAgentResultCreate {
    crate::WorkspaceAgentResultCreate {
        packet_id: packet.id.clone(),
        run_id: Some(run.id.clone()),
        source_thread_id: run.source_thread_id.clone(),
        source_turn_id: Some("turn-synthetic".to_string()),
        body: "Subjective\n\nObjective\n\nAssessment\n\nPlan".to_string(),
        summary: "Daily-note template recommendation".to_string(),
        result_kind: "template_proposal".to_string(),
        structured_changes_json: r#"[{"id":"section-plan","kind":"insert"}]"#.to_string(),
        rationale_summary: "Matches the authorized daily-note pattern.".to_string(),
        status: "review_pending".to_string(),
        actor: "agent".to_string(),
        expected_client_id: Some(packet.client_id.clone()),
        expected_note_id: packet.note_id.clone(),
        expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
    }
}

#[tokio::test]
async fn workspace_agent_run_preserves_packet_revision_and_source_manifest() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_create(&client, &note))
        .await
        .expect("packet should prepare");
    assert_eq!(packet.status, "prepared");
    assert_eq!(packet.base_note_revision, Some(note.current_revision));
    assert_eq!(packet.clinician_actor, "Clinician Example");
    assert_eq!(packet.submitted_at, None);

    let prepared_replay = runtime
        .workspace()
        .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
            client_id: client.id.clone(),
            packet_id: packet.id.clone(),
            context_envelope_sha256: packet.context_envelope_sha256.clone(),
        })
        .await
        .expect("prepared replay lookup should succeed");
    assert_eq!(prepared_replay, None);

    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");
    let retried = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run start should be idempotent");
    assert_eq!(retried, run);
    assert_eq!(run.base_note_revision, packet.base_note_revision);
    assert_eq!(run.context_envelope_sha256, packet.context_envelope_sha256);
    let run_audit = runtime
        .workspace()
        .list_audit_events("agent_run", &run.id)
        .await
        .expect("run audit should list");
    assert!(run_audit.iter().any(|event| {
        event.action == "started"
            && event.actor == "Clinician Example"
            && event.actor_kind == "clinician"
    }));

    let initial_sources = runtime
        .workspace()
        .list_agent_run_sources(&run.id)
        .await
        .expect("initial source should list");
    assert_eq!(initial_sources.len(), 2);
    let packet_source = initial_sources
        .iter()
        .find(|source| source.source_entity_type == "context_packet")
        .expect("packet envelope source");
    assert_eq!(packet_source.source_entity_id, packet.id);
    assert_eq!(packet_source.source_revision, packet.base_note_revision);
    assert_eq!(packet_source.snapshot_json, packet.context_envelope_json);
    assert_eq!(packet_source.content_sha256, packet.context_envelope_sha256);
    let contract_source = initial_sources
        .iter()
        .find(|source| source.source_entity_type == "packet_contract")
        .expect("packet authorization contract source");
    assert!(contract_source.snapshot_json.contains("authorizedScope"));
    assert!(contract_source.snapshot_json.contains("expectedOutputKind"));
    assert_eq!(
        contract_source.content_sha256,
        format!(
            "{:x}",
            Sha256::digest(contract_source.snapshot_json.as_bytes())
        )
    );

    let note_snapshot = serde_json::json!({
        "clientId": client.id,
        "noteId": note.id,
        "revision": note.current_revision,
        "body": note.body,
    })
    .to_string();
    let note_source = runtime
        .workspace()
        .record_agent_run_source(crate::WorkspaceAgentRunSourceCreate {
            run_id: run.id.clone(),
            source_entity_type: "note_revision".to_string(),
            source_entity_id: note.id.clone(),
            source_revision: Some(note.current_revision),
            display_label: "Daily note revision 1".to_string(),
            snapshot_json: note_snapshot.clone(),
            access_purpose: "authorized prior-note comparison".to_string(),
        })
        .await
        .expect("note source should record");
    assert_eq!(
        note_source.content_sha256,
        format!("{:x}", Sha256::digest(note_snapshot.as_bytes()))
    );

    let mut mismatched_result = result_create(&packet, &run);
    mismatched_result.result_kind = "unrelated_recommendation".to_string();
    let mismatch_error = runtime
        .workspace()
        .complete_agent_run_with_result(mismatched_result)
        .await
        .expect_err("result kind must match the packet contract")
        .to_string();
    assert!(mismatch_error.contains("does not match packet expected output kind"));

    let result = runtime
        .workspace()
        .complete_agent_run_with_result(result_create(&packet, &run))
        .await
        .expect("run result should save");
    assert_eq!(result.run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(result.base_note_revision, packet.base_note_revision);
    assert_eq!(result.packet_context_sha256, packet.context_envelope_sha256);
    let completed_runs = runtime
        .workspace()
        .list_agent_runs(crate::WorkspaceAgentRunFilter {
            client_id: client.id.clone(),
            note_id: Some(note.id.clone()),
            packet_id: Some(packet.id.clone()),
            limit: Some(10),
        })
        .await
        .expect("completed run should list");
    assert_eq!(
        completed_runs[0].source_thread_id.as_deref(),
        Some("thread-synthetic")
    );
    assert_eq!(
        completed_runs[0].source_turn_id.as_deref(),
        Some("turn-synthetic")
    );

    let packets = runtime
        .workspace()
        .list_context_packets(crate::WorkspaceContextPacketFilter {
            client_id: client.id.clone(),
            note_id: Some(note.id.clone()),
            limit: Some(10),
        })
        .await
        .expect("packet history should list");
    assert_eq!(packets[0].status, "submitted");
    assert!(packets[0].submitted_at.is_some());
    let replay = runtime
        .workspace()
        .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
            client_id: client.id,
            packet_id: packet.id,
            context_envelope_sha256: packet.context_envelope_sha256,
        })
        .await
        .expect("submitted replay lookup should succeed");
    assert!(replay.is_some());
}

#[tokio::test]
async fn workspace_manual_result_import_is_audited_as_clinician_work() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_create(&client, &note))
        .await
        .expect("packet should prepare");
    let result = runtime
        .workspace()
        .create_agent_result(crate::WorkspaceAgentResultCreate {
            packet_id: packet.id.clone(),
            run_id: None,
            body: "Clinician-pasted external recommendation.".to_string(),
            summary: "Manual recovery import".to_string(),
            result_kind: packet.expected_output_kind.clone(),
            structured_changes_json: "[]".to_string(),
            status: "review_pending".to_string(),
            actor: "Clinician Example".to_string(),
            expected_client_id: Some(client.id.clone()),
            expected_note_id: Some(note.id.clone()),
            expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
            ..Default::default()
        })
        .await
        .expect("manual result import should save");
    let run_id = result.run_id.as_deref().expect("manual import run id");
    let runs = runtime
        .workspace()
        .list_agent_runs(crate::WorkspaceAgentRunFilter {
            client_id: client.id,
            note_id: Some(note.id),
            packet_id: Some(packet.id),
            limit: Some(10),
        })
        .await
        .expect("manual run should list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run_id);
    assert_eq!(runs[0].run_kind, "manual_import");
    let audit = runtime
        .workspace()
        .list_audit_events("agent_result", &result.id)
        .await
        .expect("manual result audit should list");
    assert!(audit.iter().any(|event| {
        event.action == "saved"
            && event.actor == "Clinician Example"
            && event.actor_kind == "clinician"
            && event.source == "manual_import"
    }));
}

#[tokio::test]
async fn workspace_canceled_packet_blocks_replay_and_run_start() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_create(&client, &note))
        .await
        .expect("packet should prepare");
    let canceled = runtime
        .workspace()
        .cancel_context_packet(crate::WorkspaceContextPacketLifecycleUpdate {
            packet_id: packet.id.clone(),
            client_id: client.id.clone(),
            expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("cancel should succeed")
        .expect("packet should exist");
    assert_eq!(canceled.status, "canceled");
    assert!(canceled.canceled_at.is_some());
    assert!(
        runtime
            .workspace()
            .start_agent_run(run_start(&packet))
            .await
            .is_err()
    );
    let replay = runtime
        .workspace()
        .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
            client_id: client.id,
            packet_id: packet.id,
            context_envelope_sha256: packet.context_envelope_sha256,
        })
        .await
        .expect("canceled replay lookup should succeed");
    assert_eq!(replay, None);
}

#[tokio::test]
async fn workspace_stale_agent_result_proposal_keeps_original_base_and_decisions() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_create(&client, &note))
        .await
        .expect("packet should prepare");
    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");
    let result = runtime
        .workspace()
        .complete_agent_run_with_result(result_create(&packet, &run))
        .await
        .expect("result should save");
    let updated_note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            id: Some(note.id.clone()),
            client_id: client.id.clone(),
            title: note.title.clone(),
            kind: note.kind.clone(),
            body: "Human note revision two.".to_string(),
            status: note.status.clone(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("human note update should save");
    let proposal = runtime
        .workspace()
        .create_note_proposal_from_agent_result(crate::WorkspaceNoteProposalCreate {
            note_id: note.id.clone(),
            base_revision: updated_note.current_revision,
            agent_result_id: Some(result.id.clone()),
            proposed_body: result.body.clone(),
            summary: result.summary.clone(),
            ..Default::default()
        })
        .await
        .expect("stale linked proposal should remain reviewable");
    assert_eq!(proposal.base_revision, note.current_revision);
    assert_eq!(
        proposal.agent_result_id.as_deref(),
        Some(result.id.as_str())
    );
    assert!(
        runtime
            .workspace()
            .resolve_note_proposal(&proposal.id, /*accept*/ true, "Clinician Example")
            .await
            .is_err()
    );

    runtime
        .workspace()
        .record_note_proposal_change_decision(crate::WorkspaceNoteProposalChangeDecisionCreate {
            proposal_id: proposal.id.clone(),
            decision_kind: crate::WorkspaceNoteProposalDecisionKind::CopiedChange,
            change_id: "section-plan".to_string(),
            applied_text: Some("Plan".to_string()),
            actor: "Clinician Example".to_string(),
            reason: "Copied into the human draft for manual integration.".to_string(),
        })
        .await
        .expect("partial decision should save");
    runtime
        .workspace()
        .resolve_note_proposal(&proposal.id, /*accept*/ false, "Clinician Example")
        .await
        .expect("stale proposal should decline")
        .expect("proposal should exist");
    let decisions = runtime
        .workspace()
        .list_note_proposal_decisions(&proposal.id)
        .await
        .expect("decisions should list");
    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[0].decision_kind,
        crate::WorkspaceNoteProposalDecisionKind::CopiedChange
    );
    assert_eq!(
        decisions[1].decision_kind,
        crate::WorkspaceNoteProposalDecisionKind::RejectedAll
    );
    let unchanged = runtime
        .workspace()
        .get_note(&note.id)
        .await
        .expect("note should load")
        .expect("note should exist");
    assert_eq!(unchanged, updated_note);
}

#[tokio::test]
async fn workspace_edited_acceptance_records_exact_resulting_revision() {
    let runtime = test_runtime().await;
    let (_client, note) = seed_client_note(&runtime).await;
    let proposal = runtime
        .workspace()
        .create_note_proposal(crate::WorkspaceNoteProposalCreate {
            note_id: note.id.clone(),
            base_revision: note.current_revision,
            proposed_body: "Agent proposal.".to_string(),
            summary: "Synthetic recommendation".to_string(),
            ..Default::default()
        })
        .await
        .expect("proposal should save");
    let resolved = runtime
        .workspace()
        .resolve_note_proposal_with(crate::WorkspaceNoteProposalResolve {
            proposal_id: proposal.id.clone(),
            resolution: crate::WorkspaceNoteProposalResolution::AcceptEdited {
                body: "Clinician-edited accepted body.".to_string(),
            },
            actor: "Clinician Example".to_string(),
            reason: "Adjusted wording before acceptance.".to_string(),
        })
        .await
        .expect("edited acceptance should save")
        .expect("proposal should exist");
    assert_eq!(
        resolved.status,
        crate::WorkspaceNoteProposalStatus::Accepted
    );
    let decisions = runtime
        .workspace()
        .list_note_proposal_decisions(&proposal.id)
        .await
        .expect("decision should list");
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision_kind,
        crate::WorkspaceNoteProposalDecisionKind::AcceptedEdited
    );
    assert_eq!(
        decisions[0].resulting_note_revision,
        Some(note.current_revision + 1)
    );
    assert_eq!(
        decisions[0].applied_text.as_deref(),
        Some("Clinician-edited accepted body.")
    );
}

#[tokio::test]
async fn workspace_packet_rejects_path_bearing_keys_and_values() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    for forbidden_key in ["localPath", "LOCAL_path", "preview-cache-path"] {
        let mut packet = packet_create(&client, &note);
        let mut envelope: serde_json::Value =
            serde_json::from_str(&packet.context_envelope_json).expect("valid test envelope");
        let mut artifact = serde_json::json!({
            "id": "document-1",
            "fileReference": "referral.pdf",
        });
        artifact[forbidden_key] = serde_json::json!("/Users/example/private/referral.pdf");
        envelope["selectedArtifacts"] = serde_json::json!([artifact]);
        packet.context_envelope_json = envelope.to_string();
        let error = runtime
            .workspace()
            .prepare_context_packet(packet)
            .await
            .expect_err("path-bearing packet should fail")
            .to_string();
        assert!(error.contains(forbidden_key));
    }

    for forbidden_value in [
        "/Users/example/private/referral.pdf",
        r"C:\Users\example\private\referral.pdf",
        r"\\server\share\referral.pdf",
    ] {
        let mut packet = packet_create(&client, &note);
        let mut envelope: serde_json::Value =
            serde_json::from_str(&packet.context_envelope_json).expect("valid test envelope");
        envelope["selectedArtifacts"] = serde_json::json!([{
            "id": "document-1",
            "fileReference": "referral.pdf",
            "notes": forbidden_value,
        }]);
        packet.context_envelope_json = envelope.to_string();
        let error = runtime
            .workspace()
            .prepare_context_packet(packet)
            .await
            .expect_err("absolute path value should fail")
            .to_string();
        assert!(error.contains("absolute filesystem path values"));
    }

    let mut packet = packet_create(&client, &note);
    packet.authorized_scope_json = serde_json::json!({
        "categories": ["visit_history"],
        "maxRecords": 5,
        "innocentLabel": "/tmp/private-patient-export.json",
    })
    .to_string();
    let error = runtime
        .workspace()
        .prepare_context_packet(packet)
        .await
        .expect_err("path-bearing authorized scope should fail")
        .to_string();
    assert!(error.contains("authorized scope"));
    assert!(error.contains("absolute filesystem path values"));
}

#[tokio::test]
async fn workspace_authorized_context_reader_returns_hashed_patient_owned_records() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let encounter = runtime
        .workspace()
        .upsert_encounter(crate::WorkspaceEncounterUpsert {
            client_id: client.id.clone(),
            kind: "therapy".to_string(),
            title: "Synthetic visit one".to_string(),
            status: "completed".to_string(),
            ..Default::default()
        })
        .await
        .expect("encounter should save");
    let prior_note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            encounter_id: Some(encounter.id.clone()),
            title: "Progress note".to_string(),
            kind: "progress".to_string(),
            body: "Exact synthetic progress-note body.".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("prior note should save");

    let (other_client, _other_note) = seed_client_note(&runtime).await;
    runtime
        .workspace()
        .upsert_encounter(crate::WorkspaceEncounterUpsert {
            client_id: other_client.id.clone(),
            kind: "therapy".to_string(),
            title: "Other patient's visit".to_string(),
            status: "completed".to_string(),
            ..Default::default()
        })
        .await
        .expect("cross-client encounter should save");

    let mut create = packet_create(&client, &note);
    create.authorized_scope_json = serde_json::json!({
        "categories": ["visit_history", "progress_notes"],
        "maxRecords": 10,
    })
    .to_string();
    let packet = runtime
        .workspace()
        .prepare_context_packet(create)
        .await
        .expect("packet should prepare");
    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");

    let visits = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id.clone(),
            category: "visit_history".to_string(),
            max_records: Some(10),
        })
        .await
        .expect("visit history should be authorized");
    assert_eq!(visits.run_id, run.id);
    assert_eq!(visits.packet_id, packet.id);
    assert_eq!(visits.client_id, client.id);
    assert_eq!(visits.note_id.as_deref(), Some(note.id.as_str()));
    assert_eq!(visits.max_records, 10);
    assert_eq!(visits.sources.len(), 1);
    assert_eq!(visits.sources[0].source_entity_type, "encounter");
    assert_eq!(visits.sources[0].source_entity_id, encounter.id);
    let visit_snapshot: serde_json::Value =
        serde_json::from_str(&visits.sources[0].snapshot_json).expect("visit snapshot is JSON");
    assert_eq!(visit_snapshot["client_id"], client.id);
    assert_eq!(visit_snapshot["title"], "Synthetic visit one");
    assert_eq!(
        visits.sources[0].content_sha256,
        format!(
            "{:x}",
            Sha256::digest(visits.sources[0].snapshot_json.as_bytes())
        )
    );

    let notes = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id.clone(),
            category: "progress_notes".to_string(),
            max_records: Some(10),
        })
        .await
        .expect("progress notes should be authorized");
    assert_eq!(notes.sources.len(), 2);
    assert!(notes.sources.iter().all(|source| {
        let snapshot: serde_json::Value =
            serde_json::from_str(&source.snapshot_json).expect("note snapshot is JSON");
        snapshot["client_id"] == client.id
            && source.source_entity_type == "note_revision"
            && source.source_revision.is_some()
    }));
    assert!(notes.sources.iter().any(|source| {
        source.source_entity_id == prior_note.id
            && source
                .snapshot_json
                .contains("Exact synthetic progress-note body.")
    }));
    assert!(
        !visits
            .sources
            .iter()
            .chain(notes.sources.iter())
            .any(|source| source.snapshot_json.contains(&other_client.id))
    );

    let manifest = runtime
        .workspace()
        .list_agent_run_sources(&run.id)
        .await
        .expect("source manifest should list");
    assert_eq!(
        manifest.len(),
        5,
        "packet envelope + packet contract + visit + two note snapshots"
    );
}

#[tokio::test]
async fn workspace_authorized_context_reader_enforces_default_and_max_limits() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    for index in 0..25 {
        runtime
            .workspace()
            .upsert_encounter(crate::WorkspaceEncounterUpsert {
                client_id: client.id.clone(),
                kind: "therapy".to_string(),
                title: format!("Synthetic visit {index:02}"),
                status: "completed".to_string(),
                ..Default::default()
            })
            .await
            .expect("encounter should save");
    }
    let mut create = packet_create(&client, &note);
    create.authorized_scope_json = serde_json::json!({
        "read": {
            "categories": ["visit_history"],
            "maxRecords": 500,
        },
    })
    .to_string();
    let packet = runtime
        .workspace()
        .prepare_context_packet(create)
        .await
        .expect("packet should prepare");
    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");

    let defaulted = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id.clone(),
            category: "visit_history".to_string(),
            max_records: None,
        })
        .await
        .expect("default limit read should succeed");
    assert_eq!(defaulted.max_records, 20);
    assert_eq!(defaulted.sources.len(), 20);

    let clamped = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id,
            category: "visit_history".to_string(),
            max_records: Some(u32::MAX),
        })
        .await
        .expect("maximum limit read should succeed");
    assert_eq!(clamped.max_records, 100);
    assert_eq!(clamped.sources.len(), 25);
}

#[tokio::test]
async fn workspace_authorized_context_reader_bounds_and_redacts_note_content() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let large_body = format!(
        "Synthetic note with /Users/example/private/patient.txt {}",
        "x".repeat(40 * 1024)
    );
    for index in 0..20 {
        runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: format!("Large progress note {index:02}"),
                kind: "progress_note".to_string(),
                body: large_body.clone(),
                status: "draft".to_string(),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("large progress note should save");
    }
    let mut create = packet_create(&client, &note);
    create.authorized_scope_json = serde_json::json!({
        "categories": ["progress_notes"],
        "maxRecords": 100,
    })
    .to_string();
    let packet = runtime
        .workspace()
        .prepare_context_packet(create)
        .await
        .expect("packet should prepare");
    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");

    let notes = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id,
            category: "progress_notes".to_string(),
            max_records: Some(100),
        })
        .await
        .expect("bounded progress notes should read");
    assert!(!notes.sources.is_empty());
    assert!(
        notes
            .sources
            .iter()
            .map(|source| source.snapshot_json.len())
            .sum::<usize>()
            <= super::MAX_AGENT_CONTEXT_SNAPSHOT_BYTES
    );
    let large_snapshot = notes
        .sources
        .iter()
        .map(|source| {
            serde_json::from_str::<serde_json::Value>(&source.snapshot_json)
                .expect("note source snapshot should be JSON")
        })
        .find(|snapshot| snapshot["body_original_bytes"].as_u64().unwrap_or_default() > 40_000)
        .expect("large source snapshot");
    assert_eq!(large_snapshot["body_truncated"], true);
    assert_eq!(large_snapshot["body_local_paths_redacted"], true);
    assert!(
        large_snapshot["body"]
            .as_str()
            .expect("returned body")
            .len()
            <= super::MAX_AGENT_NOTE_BODY_BYTES
    );
    assert!(
        !large_snapshot["body"]
            .as_str()
            .expect("returned body")
            .contains("/Users/example")
    );
    assert_eq!(
        large_snapshot["body_original_sha256"],
        format!("{:x}", Sha256::digest(large_body.as_bytes()))
    );
}

#[tokio::test]
async fn workspace_authorized_context_reader_denies_scope_and_lifecycle_expansion() {
    let runtime = test_runtime().await;
    let (client, note) = seed_client_note(&runtime).await;
    let mut create = packet_create(&client, &note);
    create.authorized_scope_json = serde_json::json!({
        "categories": ["visit_history"],
        "maxRecords": 3,
    })
    .to_string();
    let packet = runtime
        .workspace()
        .prepare_context_packet(create)
        .await
        .expect("packet should prepare");
    let run = runtime
        .workspace()
        .start_agent_run(run_start(&packet))
        .await
        .expect("run should start");

    let scope_error = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id.clone(),
            category: "progress_notes".to_string(),
            max_records: Some(100),
        })
        .await
        .expect_err("an omitted category must be denied")
        .to_string();
    assert!(scope_error.contains("does not explicitly authorize"));
    let manifest = runtime
        .workspace()
        .list_agent_run_sources(&run.id)
        .await
        .expect("source manifest should list");
    assert_eq!(
        manifest.len(),
        2,
        "denied read must not add to the packet envelope and contract sources"
    );

    sqlx::query("UPDATE workspace_context_packets SET status = 'sent' WHERE id = ?")
        .bind(&packet.id)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("test should change packet lifecycle");
    let lifecycle_error = runtime
        .workspace()
        .read_authorized_agent_context(crate::WorkspaceAgentContextReadRequest {
            run_id: run.id,
            category: "visit_history".to_string(),
            max_records: Some(100),
        })
        .await
        .expect_err("only a submitted packet may authorize reads")
        .to_string();
    assert!(lifecycle_error.contains("does not authorize agent context reads"));
}

#[tokio::test]
async fn workspace_provenance_migration_backfills_legacy_rows() {
    let codex_home = unique_temp_dir();
    tokio::fs::create_dir_all(&codex_home)
        .await
        .expect("codex home should create");
    let workspace_path = crate::workspace_db_path(&codex_home);
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(&workspace_path)
            .create_if_missing(true),
    )
    .await
    .expect("legacy workspace db should open");
    let old_migrator = Migrator {
        migrations: Cow::Owned(
            WORKSPACE_MIGRATOR
                .migrations
                .iter()
                .filter(|migration| migration.version <= 13)
                .cloned()
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
        table_name: WORKSPACE_MIGRATOR.table_name.clone(),
        create_schemas: WORKSPACE_MIGRATOR.create_schemas.clone(),
    };
    old_migrator
        .run(&pool)
        .await
        .expect("legacy workspace migrations should apply");
    let envelope = packet_envelope("Legacy request", 1);
    let envelope_hash = format!("{:x}", Sha256::digest(envelope.as_bytes()));
    sqlx::query(
        "INSERT INTO workspace_clients (id, display_name, summary, created_at_ms, updated_at_ms) VALUES ('legacy-client', 'Legacy Synthetic Patient', '', 10, 10)",
    )
    .execute(&pool)
    .await
    .expect("legacy client should insert");
    sqlx::query(
        "INSERT INTO workspace_notes (id, client_id, title, kind, body, status, current_revision, created_at_ms, updated_at_ms) VALUES ('legacy-note', 'legacy-client', 'Legacy note', 'daily', 'Legacy body', 'draft', 1, 10, 10)",
    )
    .execute(&pool)
    .await
    .expect("legacy note should insert");
    sqlx::query(
        "INSERT INTO workspace_note_revisions (note_id, revision, body, actor, created_at_ms) VALUES ('legacy-note', 1, 'Legacy body', 'Legacy Clinician', 10)",
    )
    .execute(&pool)
    .await
    .expect("legacy revision should insert");
    sqlx::query(
        r#"
INSERT INTO workspace_context_packets (
    id, client_id, note_id, human_request, selected_artifact_ids_json,
    selected_derivative_ids_json, selected_clip_ids_json, artifact_summary,
    derivative_summary, clip_summary, chart_context_summary,
    context_envelope_json, context_envelope_sha256, status,
    created_at_ms, sent_at_ms, updated_at_ms
) VALUES (
    'legacy-packet', 'legacy-client', 'legacy-note', 'Legacy request', '[]',
    '[]', '[]', '', '', '', '', ?, ?, 'result_saved', 20, 21, 22
)
        "#,
    )
    .bind(&envelope)
    .bind(&envelope_hash)
    .execute(&pool)
    .await
    .expect("legacy packet should insert");
    sqlx::query(
        "INSERT INTO workspace_agent_results (id, packet_id, client_id, note_id, body, summary, status, created_at_ms, updated_at_ms) VALUES ('legacy-result', 'legacy-packet', 'legacy-client', 'legacy-note', 'Legacy recommendation', 'Legacy summary', 'converted', 30, 31)",
    )
    .execute(&pool)
    .await
    .expect("legacy result should insert");
    sqlx::query(
        "INSERT INTO workspace_note_proposals (id, note_id, base_revision, proposed_body, summary, status, source_turn_id, created_at_ms, resolved_at_ms) VALUES ('legacy-proposal', 'legacy-note', 1, 'Legacy recommendation', 'Legacy summary', 'accepted', 'legacy-result', 32, 33)",
    )
    .execute(&pool)
    .await
    .expect("legacy proposal should insert");
    sqlx::query(
        "INSERT INTO workspace_audit_events (id, entity_type, entity_id, action, actor, summary, created_at_ms) VALUES ('legacy-audit', 'note_proposal', 'legacy-proposal', 'accepted', 'Legacy Clinician', '', 33)",
    )
    .execute(&pool)
    .await
    .expect("legacy audit should insert");
    pool.close().await;

    let runtime = StateRuntime::init(codex_home, "test-provider".to_string())
        .await
        .expect("current runtime should migrate legacy workspace");
    let packets = runtime
        .workspace()
        .list_context_packets(crate::WorkspaceContextPacketFilter {
            client_id: "legacy-client".to_string(),
            note_id: Some("legacy-note".to_string()),
            limit: Some(10),
        })
        .await
        .expect("migrated packet should list");
    assert_eq!(packets[0].base_note_revision, Some(1));
    assert!(packets[0].submitted_at.is_some());
    assert!(packets[0].authorized_scope_json.contains("legacy"));
    let runs = runtime
        .workspace()
        .list_agent_runs(crate::WorkspaceAgentRunFilter {
            client_id: "legacy-client".to_string(),
            note_id: Some("legacy-note".to_string()),
            packet_id: Some("legacy-packet".to_string()),
            limit: Some(10),
        })
        .await
        .expect("migrated run should list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_kind, "legacy_import");
    assert_eq!(runs[0].base_note_revision, Some(1));
    let results = runtime
        .workspace()
        .list_agent_results(crate::WorkspaceAgentResultFilter {
            client_id: "legacy-client".to_string(),
            note_id: Some("legacy-note".to_string()),
            packet_id: Some("legacy-packet".to_string()),
            limit: Some(10),
        })
        .await
        .expect("migrated result should list");
    assert_eq!(results[0].run_id.as_deref(), Some(runs[0].id.as_str()));
    assert_eq!(results[0].base_note_revision, Some(1));
    assert_eq!(results[0].packet_context_sha256, envelope_hash);
    let proposals = runtime
        .workspace()
        .list_note_proposals("legacy-note")
        .await
        .expect("migrated proposal should list");
    assert_eq!(
        proposals[0].agent_result_id.as_deref(),
        Some("legacy-result")
    );
    let decisions = runtime
        .workspace()
        .list_note_proposal_decisions("legacy-proposal")
        .await
        .expect("migrated decision should list");
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision_kind,
        crate::WorkspaceNoteProposalDecisionKind::AcceptedAll
    );
    assert_eq!(decisions[0].actor, "Legacy Clinician");
    assert_eq!(decisions[0].resulting_note_revision, Some(2));
}
