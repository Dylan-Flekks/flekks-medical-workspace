use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;

struct Fixture {
    runtime: std::sync::Arc<StateRuntime>,
    client: crate::WorkspaceClient,
    encounter: crate::WorkspaceEncounter,
    note: crate::WorkspaceNote,
    checkpoint: crate::WorkspaceDraftCheckpoint,
    plan_session: crate::WorkspacePlanSession,
}

async fn fixture() -> Fixture {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state runtime");
    runtime
        .workspace()
        .provision_synthetic_workspace("workspace plan tests")
        .await
        .expect("synthetic policy");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Planning Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client");
    let encounter = runtime
        .workspace()
        .upsert_encounter(crate::WorkspaceEncounterUpsert {
            client_id: client.id.clone(),
            kind: "visit".to_string(),
            title: "Synthetic gait visit".to_string(),
            status: "completed".to_string(),
            ..Default::default()
        })
        .await
        .expect("encounter");
    let note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            encounter_id: Some(encounter.id.clone()),
            title: "Daily progress note".to_string(),
            kind: "daily_note".to_string(),
            body: "Synthetic baseline gait tolerance.".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("note");
    let checkpoint = checkpoint(&runtime, &client, &encounter, &note, None, "First").await;
    let plan_session = runtime
        .workspace()
        .open_plan_session(crate::WorkspacePlanSessionOpen {
            client_id: client.id.clone(),
            created_by: "Clinician Example".to_string(),
        })
        .await
        .expect("plan session");
    let plan_session = runtime
        .workspace()
        .bind_plan_session_thread(crate::WorkspacePlanSessionThreadBind {
            session_id: plan_session.id,
            client_id: client.id.clone(),
            expected_thread_id: None,
            source_thread_id: "planning-thread".to_string(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("thread binding");
    Fixture {
        runtime,
        client,
        encounter,
        note,
        checkpoint,
        plan_session,
    }
}

async fn checkpoint(
    runtime: &StateRuntime,
    client: &crate::WorkspaceClient,
    encounter: &crate::WorkspaceEncounter,
    note: &crate::WorkspaceNote,
    current: Option<&crate::WorkspaceDraftCheckpoint>,
    title: &str,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            session_id: current.map(|checkpoint| checkpoint.session_id.clone()),
            client_id: client.id.clone(),
            expected_current_checkpoint_id: current.map(|checkpoint| checkpoint.id.clone()),
            expected_current_checkpoint_revision: current.map(|checkpoint| checkpoint.revision),
            expected_current_checkpoint_sha256: current
                .map(|checkpoint| checkpoint.content_sha256.clone()),
            encounter_id: Some(encounter.id.clone()),
            note_id: Some(note.id.clone()),
            base_note_revision: Some(note.current_revision),
            draft_json: format!(r#"{{"schemaVersion":1,"note":{{"title":{title:?}}}}}"#),
            trigger: "focus_change".to_string(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("checkpoint")
}

#[tokio::test]
async fn planning_thread_binding_is_permanent_across_sessions_and_patients() {
    let fixture = fixture().await;
    let rebound = fixture
        .runtime
        .workspace()
        .bind_plan_session_thread(crate::WorkspacePlanSessionThreadBind {
            session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            expected_thread_id: Some("planning-thread".to_string()),
            source_thread_id: "replacement-planning-thread".to_string(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect_err("a patient planning session must never release its rollout thread");
    assert_eq!(rebound.kind(), "validation");
    assert!(rebound.to_string().contains("permanently bound"));

    fixture
        .runtime
        .workspace()
        .close_plan_session(crate::WorkspacePlanSessionClose {
            session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            reason: "synthetic completion".to_string(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("close original session");
    let other_client = fixture
        .runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Other Synthetic Planning Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("other client");
    let other_session = fixture
        .runtime
        .workspace()
        .open_plan_session(crate::WorkspacePlanSessionOpen {
            client_id: other_client.id.clone(),
            created_by: "Clinician Example".to_string(),
        })
        .await
        .expect("other plan session");
    let conflict = fixture
        .runtime
        .workspace()
        .bind_plan_session_thread(crate::WorkspacePlanSessionThreadBind {
            session_id: other_session.id,
            client_id: other_client.id,
            expected_thread_id: None,
            source_thread_id: "planning-thread".to_string(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect_err("a closed patient's rollout thread remains permanently owned");
    assert_eq!(conflict.kind(), "validation");
    assert!(conflict.to_string().contains("already bound"));
}

async fn start_and_claim(
    fixture: &Fixture,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    key: &str,
    turn_id: &str,
) -> (
    crate::WorkspaceGuideRun,
    crate::WorkspacePlanningGuideExecutionBinding,
) {
    let run = start_run(fixture, checkpoint, key).await;
    let execution = claim_run(fixture, checkpoint, &run, turn_id).await;
    (run, execution)
}

async fn claim_run(
    fixture: &Fixture,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    run: &crate::WorkspaceGuideRun,
    turn_id: &str,
) -> crate::WorkspacePlanningGuideExecutionBinding {
    fixture
        .runtime
        .workspace()
        .claim_planning_guide_turn(crate::WorkspacePlanningGuideTurnClaimRequest {
            guide_run_id: run.id.clone(),
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            source_checkpoint_id: checkpoint.id.clone(),
            source_checkpoint_revision: checkpoint.revision,
            source_checkpoint_sha256: checkpoint.content_sha256.clone(),
            source_thread_id: "planning-thread".to_string(),
            source_turn_id: turn_id.to_string(),
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            prompt: format!("Synthetic planning prompt for {turn_id}"),
        })
        .await
        .expect("turn claim")
}

async fn start_run(
    fixture: &Fixture,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    key: &str,
) -> crate::WorkspaceGuideRun {
    fixture
        .runtime
        .workspace()
        .start_guide_run(crate::WorkspaceGuideRunStart {
            client_id: fixture.client.id.clone(),
            session_id: checkpoint.session_id.clone(),
            source_checkpoint_id: checkpoint.id.clone(),
            source_checkpoint_revision: checkpoint.revision,
            source_checkpoint_sha256: checkpoint.content_sha256.clone(),
            request_json: serde_json::json!({
                "schemaVersion": 1,
                "intent": "patient plan",
                "planSessionId": fixture.plan_session.id.clone(),
            })
            .to_string(),
            idempotency_key: key.to_string(),
            trigger: "human_message".to_string(),
            actor: "Clinician Example".to_string(),
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            model_tool_mode: crate::WorkspaceGuideModelToolMode::WorkspacePlanningOnly,
        })
        .await
        .expect("guide run")
}

async fn stored_run(fixture: &Fixture, run_id: &str) -> crate::WorkspaceGuideRun {
    fixture
        .runtime
        .workspace()
        .list_guide_runs(crate::WorkspaceGuideRunFilter {
            client_id: fixture.client.id.clone(),
            session_id: Some(fixture.checkpoint.session_id.clone()),
            limit: Some(100),
            ..Default::default()
        })
        .await
        .expect("guide run list")
        .into_iter()
        .find(|run| run.id == run_id)
        .expect("stored guide run")
}

async fn required_evidence(
    fixture: &Fixture,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    key_prefix: &str,
) -> (
    crate::WorkspacePlanningContextRead,
    crate::WorkspacePlanningContextRead,
) {
    let chart = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "patient_chart".to_string(),
            max_records: Some(10),
            idempotency_key: format!("{key_prefix}-chart"),
        })
        .await
        .expect("patient chart read");
    let selected = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "selected_context".to_string(),
            max_records: Some(10),
            idempotency_key: format!("{key_prefix}-selected"),
        })
        .await
        .expect("selected context read");
    (chart, selected)
}

fn plan_completion_input(
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    evidence_read_ids: Vec<String>,
    key: &str,
) -> crate::WorkspacePlanTurnComplete {
    crate::WorkspacePlanTurnComplete {
        execution: execution.clone(),
        assistant_message_role: crate::WorkspacePlanMessageRole::Assistant,
        assistant_message: "Decision-complete synthetic patient plan.".to_string(),
        plan: Some(crate::WorkspacePlanArtifact {
            plan_markdown: "# Evidence-bound plan".to_string(),
            decisions_json: r#"["continue clinician review"]"#.to_string(),
            open_questions_json: "[]".to_string(),
        }),
        evidence_read_ids,
        idempotency_key: key.to_string(),
        actor: "Workspace Planner".to_string(),
    }
}

#[tokio::test]
async fn persistent_patient_plan_is_execution_bound_ordered_and_noncanonical() {
    let fixture = fixture().await;
    let replayed = fixture
        .runtime
        .workspace()
        .open_plan_session(crate::WorkspacePlanSessionOpen {
            client_id: fixture.client.id.clone(),
            created_by: "Different caller".to_string(),
        })
        .await
        .expect("active session replay");
    assert_eq!(replayed.id, fixture.plan_session.id);
    assert!(replayed.replayed);

    let run = start_run(&fixture, &fixture.checkpoint, "guide-one").await;
    assert_eq!(
        run.model_tool_mode,
        crate::WorkspaceGuideModelToolMode::WorkspacePlanningOnly.as_str()
    );

    let human = fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Human,
            content: "Gait tolerance improved today.".to_string(),
            idempotency_key: "message-human".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("human message");
    assert_eq!(human.sequence, 1);
    assert!(
        fixture
            .runtime
            .workspace()
            .append_plan_message(crate::WorkspacePlanMessageAppend {
                content: "Different content".to_string(),
                ..crate::WorkspacePlanMessageAppend {
                    plan_session_id: fixture.plan_session.id.clone(),
                    client_id: fixture.client.id.clone(),
                    guide_run_id: run.id.clone(),
                    role: crate::WorkspacePlanMessageRole::Human,
                    content: human.content.clone(),
                    idempotency_key: "message-human".to_string(),
                    source_thread_id: None,
                    source_turn_id: None,
                }
            })
            .await
            .is_err()
    );

    let execution = claim_run(&fixture, &fixture.checkpoint, &run, "turn-one").await;
    assert_eq!(execution.client_id, fixture.client.id);
    assert_eq!(execution.source_checkpoint_id, fixture.checkpoint.id);
    let replayed_human = fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Human,
            content: human.content.clone(),
            idempotency_key: "message-human".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("exact clinician-message retry after claim");
    assert!(replayed_human.replayed);
    let forged = fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Error,
            content: "Caller-supplied planner error".to_string(),
            idempotency_key: "forged-planner-error".to_string(),
            source_thread_id: Some(execution.source_thread_id.clone()),
            source_turn_id: Some(execution.source_turn_id.clone()),
        })
        .await
        .expect_err("public-style callers cannot create agent-attributed messages");
    assert_eq!(forged.kind(), "validation");

    let read = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "progress_notes".to_string(),
            max_records: Some(10),
            idempotency_key: "read-notes".to_string(),
        })
        .await
        .expect("authorized read");
    assert_eq!(read.sources.len(), 1);
    assert_eq!(read.sources[0].source_entity_id, fixture.note.id);
    assert!(!read.replayed);
    let replay = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "progress_notes".to_string(),
            max_records: Some(10),
            idempotency_key: "read-notes".to_string(),
        })
        .await
        .expect("authorized read replay");
    assert_eq!(replay.response_sha256, read.response_sha256);
    assert!(replay.replayed);
    let chart = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "patient_chart".to_string(),
            max_records: Some(10),
            idempotency_key: "read-chart".to_string(),
        })
        .await
        .expect("authorized patient chart read");
    assert_eq!(chart.sources.len(), 1);
    assert_eq!(chart.sources[0].source_entity_type, "patient_chart");
    assert!(chart.sources[0].snapshot_json.contains(&fixture.note.id));
    let selected = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "selected_context".to_string(),
            max_records: Some(10),
            idempotency_key: "read-selected".to_string(),
        })
        .await
        .expect("authorized selected context read");
    assert_eq!(selected.sources.len(), 1);
    assert_eq!(selected.sources[0].source_entity_type, "draft_checkpoint");
    assert_eq!(
        selected.sources[0].content_sha256,
        fixture.checkpoint.content_sha256
    );
    let mut stolen = execution.clone();
    stolen.execution_token.push('x');
    let error = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: stolen,
            category: "visit_history".to_string(),
            max_records: Some(10),
            idempotency_key: "stolen-read".to_string(),
        })
        .await
        .expect_err("stolen token must fail");
    assert_eq!(error.kind(), "validation");

    assert!(
        fixture
            .runtime
            .workspace()
            .list_completed_plan_turn_ids(
                &fixture.plan_session.id,
                &fixture.client.id,
                &execution.source_thread_id,
            )
            .await
            .expect("pre-completion history authorization")
            .is_empty()
    );
    let completion = fixture
        .runtime
        .workspace()
        .complete_plan_turn(crate::WorkspacePlanTurnComplete {
            execution: execution.clone(),
            assistant_message_role: crate::WorkspacePlanMessageRole::Assistant,
            assistant_message: "Decision-complete synthetic plan.".to_string(),
            plan: Some(crate::WorkspacePlanArtifact {
                plan_markdown: "# Synthetic patient plan".to_string(),
                decisions_json: r#"["Draft note update"]"#.to_string(),
                open_questions_json: "[]".to_string(),
            }),
            evidence_read_ids: vec![chart.id, selected.id],
            idempotency_key: "plan-one".to_string(),
            actor: "Workspace Planner".to_string(),
        })
        .await
        .expect("plan turn completion");
    assert_eq!(
        fixture
            .runtime
            .workspace()
            .list_completed_plan_turn_ids(
                &fixture.plan_session.id,
                &fixture.client.id,
                &execution.source_thread_id,
            )
            .await
            .expect("completed history authorization"),
        vec![execution.source_turn_id.clone()]
    );
    for (plan_session_id, client_id, source_thread_id) in [
        (
            fixture.plan_session.id.as_str(),
            fixture.client.id.as_str(),
            "different-planning-thread",
        ),
        (
            fixture.plan_session.id.as_str(),
            "different-client",
            execution.source_thread_id.as_str(),
        ),
        (
            "different-plan-session",
            fixture.client.id.as_str(),
            execution.source_thread_id.as_str(),
        ),
    ] {
        assert!(
            fixture
                .runtime
                .workspace()
                .list_completed_plan_turn_ids(plan_session_id, client_id, source_thread_id)
                .await
                .expect("cross-scope history lookup")
                .is_empty(),
            "completed planning history must require the exact patient session and thread"
        );
    }
    let revision = completion.revision.expect("plan revision");
    assert_eq!(revision.status, crate::WorkspacePlanRevisionStatus::Current);
    assert_eq!(revision.decisions_json, r#"["Draft note update"]"#);
    assert_eq!(revision.open_questions_json, "[]");
    let proposal = fixture
        .runtime
        .workspace()
        .create_plan_proposal(crate::WorkspacePlanProposalCreate {
            plan_session_id: fixture.plan_session.id.clone(),
            plan_revision_id: revision.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            payload: crate::WorkspacePlanProposalPayload::NoteRevision {
                note_id: fixture.note.id.clone(),
                base_revision: fixture.note.current_revision,
                proposed_body: "Agent-proposed synthetic body.".to_string(),
            },
            summary: "Propose a note revision".to_string(),
            rationale: "Reflect the clinician's documented gait change".to_string(),
            idempotency_key: "proposal-one".to_string(),
            source_thread_id: execution.source_thread_id.clone(),
            source_turn_id: execution.source_turn_id.clone(),
        })
        .await
        .expect("proposal");
    assert_eq!(proposal.status, crate::WorkspacePlanProposalStatus::Pending);
    let accepted = fixture
        .runtime
        .workspace()
        .resolve_plan_proposal(crate::WorkspacePlanProposalResolve {
            proposal_id: proposal.id,
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            resolution: crate::WorkspacePlanProposalResolution::Accept,
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("proposal decision");
    assert_eq!(
        accepted.status,
        crate::WorkspacePlanProposalStatus::Accepted
    );
    assert_eq!(
        fixture
            .runtime
            .workspace()
            .get_note(&fixture.note.id)
            .await
            .expect("note read")
            .expect("note exists")
            .body,
        fixture.note.body
    );
    let packet = fixture
        .runtime
        .workspace()
        .prepare_context_packet(plan_bound_packet_input(
            &fixture,
            &revision,
            "Execute the reviewed persistent plan.",
        ))
        .await
        .expect("plan-bound packet");
    let agent_run = fixture
        .runtime
        .workspace()
        .start_agent_run(plan_bound_run_start(&packet, "submit-plan-one"))
        .await
        .expect("plan-bound agent run");
    let submitted = fixture
        .runtime
        .workspace()
        .submit_plan_revision(crate::WorkspacePlanRevisionSubmit {
            revision_id: revision.id,
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            packet_id: packet.id,
            agent_run_id: agent_run.id,
            source_checkpoint_id: revision.source_checkpoint_id,
            source_checkpoint_revision: revision.source_checkpoint_revision,
            source_checkpoint_sha256: revision.source_checkpoint_sha256,
            content_sha256: revision.content_sha256,
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("plan submit");
    assert_eq!(
        submitted.status,
        crate::WorkspacePlanRevisionStatus::Submitted
    );
}

#[tokio::test]
async fn checkpoint_and_note_changes_outdate_current_plan_and_locked_proposals() {
    let fixture = fixture().await;
    let (run, execution) =
        start_and_claim(&fixture, &fixture.checkpoint, "guide-stale", "turn-stale").await;
    let chart = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "patient_chart".to_string(),
            max_records: Some(10),
            idempotency_key: "stale-chart".to_string(),
        })
        .await
        .expect("chart read");
    let selected = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "selected_context".to_string(),
            max_records: Some(10),
            idempotency_key: "stale-selected".to_string(),
        })
        .await
        .expect("selected context read");
    let completion = fixture
        .runtime
        .workspace()
        .complete_plan_turn(crate::WorkspacePlanTurnComplete {
            execution: execution.clone(),
            assistant_message_role: crate::WorkspacePlanMessageRole::Assistant,
            assistant_message: "Current plan.".to_string(),
            plan: Some(crate::WorkspacePlanArtifact {
                plan_markdown: "# Current plan".to_string(),
                decisions_json: "[]".to_string(),
                open_questions_json: "[]".to_string(),
            }),
            evidence_read_ids: vec![chart.id, selected.id],
            idempotency_key: "stale-plan".to_string(),
            actor: "Workspace Planner".to_string(),
        })
        .await
        .expect("plan completion");
    let revision = completion.revision.expect("plan revision");
    let proposal = fixture
        .runtime
        .workspace()
        .create_plan_proposal(crate::WorkspacePlanProposalCreate {
            plan_session_id: fixture.plan_session.id.clone(),
            plan_revision_id: revision.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id,
            payload: crate::WorkspacePlanProposalPayload::TaskDraft {
                title: "Review gait video".to_string(),
                details: "Synthetic planning task".to_string(),
                task_kind: "clinical_review".to_string(),
                priority: crate::WorkspaceTaskPriority::Normal,
                due_date: None,
                assigned_to: None,
            },
            summary: "Draft a review task".to_string(),
            rationale: "Preserve multimodal follow-up".to_string(),
            idempotency_key: "stale-proposal".to_string(),
            source_thread_id: execution.source_thread_id,
            source_turn_id: execution.source_turn_id,
        })
        .await
        .expect("task proposal");
    checkpoint(
        &fixture.runtime,
        &fixture.client,
        &fixture.encounter,
        &fixture.note,
        Some(&fixture.checkpoint),
        "Changed",
    )
    .await;
    let stored_revision = fixture
        .runtime
        .workspace()
        .get_plan_revision(&revision.id, &fixture.plan_session.id, &fixture.client.id)
        .await
        .expect("revision read")
        .expect("revision exists");
    assert_eq!(
        stored_revision.status,
        crate::WorkspacePlanRevisionStatus::Outdated
    );
    let proposals = fixture
        .runtime
        .workspace()
        .list_plan_proposals(crate::WorkspacePlanProposalFilter {
            plan_session_id: fixture.plan_session.id,
            client_id: fixture.client.id,
            ..Default::default()
        })
        .await
        .expect("proposal list");
    assert_eq!(proposals[0].id, proposal.id);
    assert_eq!(
        proposals[0].status,
        crate::WorkspacePlanProposalStatus::Outdated
    );
}

#[tokio::test]
async fn plan_turn_completion_rejects_missing_or_incomplete_evidence_without_terminalizing() {
    let fixture = fixture().await;
    let (run, execution) = start_and_claim(
        &fixture,
        &fixture.checkpoint,
        "guide-no-evidence",
        "turn-no-evidence",
    )
    .await;
    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(plan_completion_input(
            &execution,
            Vec::new(),
            "complete-no-evidence",
        ))
        .await
        .expect_err("published plan without reads must fail");
    assert_eq!(error.kind(), "validation");

    let chart = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "patient_chart".to_string(),
            max_records: Some(10),
            idempotency_key: "no-evidence-chart".to_string(),
        })
        .await
        .expect("chart read");
    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(plan_completion_input(
            &execution,
            vec![chart.id],
            "complete-missing-selected",
        ))
        .await
        .expect_err("published plan without selected context must fail");
    assert_eq!(error.kind(), "validation");

    let persisted_status: String =
        sqlx::query_scalar("SELECT status FROM workspace_guide_runs WHERE id = ?")
            .bind(&run.id)
            .fetch_one(fixture.runtime.workspace().pool.as_ref())
            .await
            .expect("guide status");
    assert_eq!(persisted_status, "running");
    assert!(
        fixture
            .runtime
            .workspace()
            .list_plan_messages(crate::WorkspacePlanMessageFilter {
                plan_session_id: fixture.plan_session.id,
                client_id: fixture.client.id,
                ..Default::default()
            })
            .await
            .expect("message list")
            .is_empty()
    );
}

#[tokio::test]
async fn plan_turn_completion_is_exactly_idempotent_and_evidence_bound() {
    let fixture = fixture().await;
    let (run, execution) =
        start_and_claim(&fixture, &fixture.checkpoint, "guide-replay", "turn-replay").await;
    let (chart, selected) = required_evidence(&fixture, &execution, "replay").await;
    let input = plan_completion_input(
        &execution,
        vec![selected.id.clone(), chart.id.clone()],
        "complete-replay",
    );
    let completed = fixture
        .runtime
        .workspace()
        .complete_plan_turn(input.clone())
        .await
        .expect("completion");
    assert!(!completed.replayed);
    assert_eq!(completed.evidence_manifest[0].context_read_id, selected.id);
    assert_eq!(completed.evidence_manifest[1].context_read_id, chart.id);
    assert_eq!(
        completed
            .revision
            .as_ref()
            .expect("revision")
            .evidence_manifest_sha256,
        completed.evidence_manifest_sha256
    );
    assert!(
        completed
            .run
            .terminal_envelope_json
            .as_deref()
            .expect("terminal receipt")
            .len()
            < 2_048
    );

    let replayed = fixture
        .runtime
        .workspace()
        .complete_plan_turn(input.clone())
        .await
        .expect("exact replay");
    assert!(replayed.replayed);
    assert!(replayed.run.replayed);
    assert!(replayed.assistant_message.replayed);
    assert!(replayed.revision.expect("replayed revision").replayed);
    assert_eq!(replayed.receipt.guide_run_id, run.id);

    let mut conflict = input;
    conflict.assistant_message.push_str(" Different.");
    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(conflict)
        .await
        .expect_err("changed terminal replay must fail");
    assert_eq!(error.kind(), "terminalConflict");
}

#[tokio::test]
async fn plan_message_default_page_returns_the_latest_messages_in_display_order() {
    let fixture = fixture().await;
    let run = start_run(&fixture, &fixture.checkpoint, "guide-message-window").await;
    for index in 1..=205 {
        fixture
            .runtime
            .workspace()
            .append_plan_message(crate::WorkspacePlanMessageAppend {
                plan_session_id: fixture.plan_session.id.clone(),
                client_id: fixture.client.id.clone(),
                guide_run_id: run.id.clone(),
                role: crate::WorkspacePlanMessageRole::Human,
                content: format!("message {index}"),
                idempotency_key: format!("message-window-{index}"),
                source_thread_id: None,
                source_turn_id: None,
            })
            .await
            .expect("append message");
    }

    let latest = fixture
        .runtime
        .workspace()
        .list_plan_messages(crate::WorkspacePlanMessageFilter {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            after_sequence: None,
            limit: Some(200),
        })
        .await
        .expect("latest message window");
    assert_eq!(latest.len(), 200);
    assert_eq!(latest.first().map(|message| message.sequence), Some(6));
    assert_eq!(latest.last().map(|message| message.sequence), Some(205));

    let incremental = fixture
        .runtime
        .workspace()
        .list_plan_messages(crate::WorkspacePlanMessageFilter {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id,
            after_sequence: Some(200),
            limit: Some(200),
        })
        .await
        .expect("incremental message window");
    assert_eq!(
        incremental
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>(),
        vec![201, 202, 203, 204, 205]
    );
}

#[tokio::test]
async fn plan_turn_completion_rolls_back_every_write_when_receipt_insert_fails() {
    let fixture = fixture().await;
    let (run, execution) = start_and_claim(
        &fixture,
        &fixture.checkpoint,
        "guide-rollback",
        "turn-rollback",
    )
    .await;
    let (chart, selected) = required_evidence(&fixture, &execution, "rollback").await;
    sqlx::query(
        r#"
CREATE TRIGGER workspace_test_fail_plan_completion
BEFORE INSERT ON workspace_plan_turn_completions
BEGIN
    SELECT RAISE(ABORT, 'injected plan completion failure');
END
        "#,
    )
    .execute(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("failure trigger");

    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(plan_completion_input(
            &execution,
            vec![chart.id, selected.id],
            "complete-rollback",
        ))
        .await
        .expect_err("injected completion must fail");
    assert_eq!(error.kind(), "storage");
    let status: String = sqlx::query_scalar("SELECT status FROM workspace_guide_runs WHERE id = ?")
        .bind(&run.id)
        .fetch_one(fixture.runtime.workspace().pool.as_ref())
        .await
        .expect("guide status");
    assert_eq!(status, "running");
    let counts: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
SELECT
    (SELECT COUNT(*) FROM workspace_plan_messages WHERE guide_run_id = ?),
    (SELECT COUNT(*) FROM workspace_plan_revisions WHERE guide_run_id = ?),
    (SELECT COUNT(*) FROM workspace_plan_turn_evidence WHERE guide_run_id = ?),
    (SELECT COUNT(*) FROM workspace_plan_turn_completions WHERE guide_run_id = ?)
        "#,
    )
    .bind(&run.id)
    .bind(&run.id)
    .bind(&run.id)
    .bind(&run.id)
    .fetch_one(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("rollback counts");
    assert_eq!(counts, (0, 0, 0, 0));
    let latest_revision: i64 =
        sqlx::query_scalar("SELECT latest_revision FROM workspace_plan_sessions WHERE id = ?")
            .bind(&fixture.plan_session.id)
            .fetch_one(fixture.runtime.workspace().pool.as_ref())
            .await
            .expect("session revision");
    assert_eq!(latest_revision, 0);
}

#[tokio::test]
async fn recovery_cancels_an_unclaimed_plan_run_once_and_unblocks_the_draft_session() {
    let fixture = fixture().await;
    let run = start_run(&fixture, &fixture.checkpoint, "guide-unclaimed-recovery").await;

    let recovered = fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("unclaimed recovery");
    assert!(recovered.active_runs.is_empty());
    assert!(recovered.session.updated_at > fixture.plan_session.updated_at);
    let canceled = stored_run(&fixture, &run.id).await;
    assert_eq!(canceled.status, crate::WorkspaceGuideRunStatus::Canceled);
    assert_eq!(canceled.source_thread_id, None);
    assert_eq!(canceled.source_turn_id, None);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            canceled
                .terminal_envelope_json
                .as_deref()
                .expect("cancellation envelope"),
        )
        .expect("valid cancellation envelope"),
        serde_json::json!({
            "schemaVersion": 1,
            "type": "canceled",
            "reason": "planning process ended before the model turn was durably claimed",
        })
    );
    let cancellation_audits = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE entity_type = 'guide_run' AND entity_id = ? AND action = 'canceled' AND source = 'workspace_plan'",
    )
    .bind(&run.id)
    .fetch_one(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("cancellation audit count");
    assert_eq!(cancellation_audits, 1);

    let replayed = fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("repeated recovery");
    assert_eq!(replayed.session.updated_at, recovered.session.updated_at);
    assert_eq!(stored_run(&fixture, &run.id).await, canceled);
    let replayed_audits = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE entity_type = 'guide_run' AND entity_id = ? AND action = 'canceled' AND source = 'workspace_plan'",
    )
    .bind(&run.id)
    .fetch_one(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("replayed cancellation audit count");
    assert_eq!(replayed_audits, cancellation_audits);

    let replacement = start_run(
        &fixture,
        &fixture.checkpoint,
        "guide-after-unclaimed-recovery",
    )
    .await;
    assert_eq!(replacement.status, crate::WorkspaceGuideRunStatus::Running);
}

#[tokio::test]
async fn recovery_records_one_error_when_the_human_message_precedes_an_unclaimed_crash() {
    let fixture = fixture().await;
    let run = start_run(
        &fixture,
        &fixture.checkpoint,
        "guide-unclaimed-human-recovery",
    )
    .await;
    let human = fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Human,
            content: "Please review the saved gait context.".to_string(),
            idempotency_key: "unclaimed-recovery-human".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("persisted human message");

    fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("unclaimed human-message recovery");
    let messages = fixture
        .runtime
        .workspace()
        .list_plan_messages(crate::WorkspacePlanMessageFilter {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            ..Default::default()
        })
        .await
        .expect("recovered messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], human);
    assert_eq!(messages[1].role, crate::WorkspacePlanMessageRole::Error);
    assert_eq!(messages[1].guide_run_id, run.id);
    assert!(messages[1].content.contains("Your message is saved"));

    fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("repeated human-message recovery");
    assert_eq!(
        fixture
            .runtime
            .workspace()
            .list_plan_messages(crate::WorkspacePlanMessageFilter {
                plan_session_id: fixture.plan_session.id,
                client_id: fixture.client.id,
                ..Default::default()
            })
            .await
            .expect("replayed recovered messages"),
        messages
    );
}

#[tokio::test]
async fn planning_finish_atomically_records_one_system_terminal_message() {
    let fixture = fixture().await;
    let run = start_run(&fixture, &fixture.checkpoint, "guide-system-terminal").await;
    fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Human,
            content: "Please check the longitudinal gait context.".to_string(),
            idempotency_key: "system-terminal-human".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("human message");
    let execution = claim_run(&fixture, &fixture.checkpoint, &run, "turn-system-terminal").await;
    let finish = crate::WorkspaceGuideRunFinish {
        run_id: run.id.clone(),
        client_id: fixture.client.id.clone(),
        session_id: run.session_id.clone(),
        source_checkpoint_id: run.source_checkpoint_id.clone(),
        source_checkpoint_revision: run.source_checkpoint_revision,
        source_checkpoint_sha256: run.source_checkpoint_sha256.clone(),
        request_envelope_sha256: run.request_envelope_sha256.clone(),
        source_thread_id: Some(execution.source_thread_id.clone()),
        source_turn_id: Some(execution.source_turn_id.clone()),
        outcome: crate::WorkspaceGuideRunOutcome::Failed {
            error_summary: "caller-controlled provider detail".to_string(),
        },
        actor: "workspace plan harness".to_string(),
    };
    fixture
        .runtime
        .workspace()
        .finish_guide_run(finish.clone())
        .await
        .expect("failed planning finish");
    fixture
        .runtime
        .workspace()
        .finish_guide_run(finish)
        .await
        .expect("exact terminal replay");

    let messages = fixture
        .runtime
        .workspace()
        .list_plan_messages(crate::WorkspacePlanMessageFilter {
            plan_session_id: fixture.plan_session.id,
            client_id: fixture.client.id,
            ..Default::default()
        })
        .await
        .expect("terminal messages");
    assert_eq!(messages.len(), 2);
    let terminal = &messages[1];
    assert_eq!(terminal.role, crate::WorkspacePlanMessageRole::Error);
    assert!(!terminal.content.contains("caller-controlled"));
    let actor_kind = sqlx::query_scalar::<_, String>(
        "SELECT actor_kind FROM workspace_audit_events WHERE entity_type = 'plan_message' AND entity_id = ?",
    )
    .bind(&terminal.id)
    .fetch_one(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("terminal message audit actor");
    assert_eq!(actor_kind, "system");
}

#[tokio::test]
async fn recovery_preserves_a_claimed_turn_after_the_human_message_and_rebuilds_pending_state() {
    let fixture = fixture().await;
    let run = start_run(&fixture, &fixture.checkpoint, "guide-recovery").await;
    let human = fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: run.id.clone(),
            role: crate::WorkspacePlanMessageRole::Human,
            content: "Please review today's gait tolerance.".to_string(),
            idempotency_key: "recovery-human".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("persisted human message");
    let execution = claim_run(&fixture, &fixture.checkpoint, &run, "turn-recovery").await;
    let chart = fixture
        .runtime
        .workspace()
        .read_authorized_planning_context(crate::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: "patient_chart".to_string(),
            max_records: Some(10),
            idempotency_key: "recovery-chart".to_string(),
        })
        .await
        .expect("chart read");
    let bound = fixture
        .runtime
        .workspace()
        .get_active_plan_session_by_thread("planning-thread")
        .await
        .expect("thread lookup")
        .expect("bound session");
    assert_eq!(bound.id, fixture.plan_session.id);
    let active = fixture
        .runtime
        .workspace()
        .list_active_plan_runs(crate::WorkspacePlanActiveRunFilter {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("active runs");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].run.id, run.id);
    assert_eq!(active[0].context_read_count, 1);
    let recovered_running = fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("claimed run recovery");
    assert_eq!(recovered_running.active_runs, active);
    assert_eq!(
        stored_run(&fixture, &run.id).await.status,
        crate::WorkspaceGuideRunStatus::Running
    );
    assert_eq!(
        fixture
            .runtime
            .workspace()
            .list_plan_messages(crate::WorkspacePlanMessageFilter {
                plan_session_id: fixture.plan_session.id.clone(),
                client_id: fixture.client.id.clone(),
                ..Default::default()
            })
            .await
            .expect("claimed run messages"),
        vec![human]
    );

    let completion = fixture
        .runtime
        .workspace()
        .complete_plan_turn(crate::WorkspacePlanTurnComplete {
            execution: execution.clone(),
            assistant_message_role: crate::WorkspacePlanMessageRole::Question,
            assistant_message: "What changed in gait tolerance today?".to_string(),
            plan: None,
            evidence_read_ids: vec![chart.id],
            idempotency_key: "complete-question".to_string(),
            actor: "Workspace Planner".to_string(),
        })
        .await
        .expect("question completion");
    assert!(completion.revision.is_none());

    let recovered = fixture
        .runtime
        .workspace()
        .reconcile_plan_session(crate::WorkspacePlanRecoveryRequest {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
        })
        .await
        .expect("recovery snapshot");
    assert!(recovered.active_runs.is_empty());
    assert_eq!(recovered.pending_questions.len(), 1);
    assert_eq!(
        recovered
            .last_completion
            .expect("completion receipt")
            .guide_run_id,
        run.id
    );
    let answer_run = start_run(&fixture, &fixture.checkpoint, "guide-recovery-answer").await;
    fixture
        .runtime
        .workspace()
        .append_plan_message(crate::WorkspacePlanMessageAppend {
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            guide_run_id: answer_run.id,
            role: crate::WorkspacePlanMessageRole::Human,
            content: "Synthetic tolerance improved.".to_string(),
            idempotency_key: "recovery-answer".to_string(),
            source_thread_id: None,
            source_turn_id: None,
        })
        .await
        .expect("answer message");
    assert!(
        fixture
            .runtime
            .workspace()
            .list_pending_plan_questions(crate::WorkspacePlanPendingQuestionFilter {
                plan_session_id: fixture.plan_session.id,
                client_id: fixture.client.id,
            })
            .await
            .expect("pending questions")
            .is_empty()
    );
}

fn plan_bound_packet_input(
    fixture: &Fixture,
    revision: &crate::WorkspacePlanRevision,
    request: &str,
) -> crate::WorkspaceContextPacketCreate {
    crate::WorkspaceContextPacketCreate {
        client_id: fixture.client.id.clone(),
        encounter_id: Some(fixture.encounter.id.clone()),
        note_id: Some(fixture.note.id.clone()),
        human_request: request.to_string(),
        selected_artifact_ids_json: "[]".to_string(),
        selected_derivative_ids_json: "[]".to_string(),
        selected_clip_ids_json: "[]".to_string(),
        artifact_summary: "0 selected files".to_string(),
        derivative_summary: "0 selected reviewed text items".to_string(),
        clip_summary: "0 selected clips".to_string(),
        chart_context_summary: "Synthetic evidence-bound planning context".to_string(),
        context_envelope_json: serde_json::json!({
            "assemblyVersion": "workspace-plan-binding-test-v1",
            "includeDocuments": false,
            "humanRequest": request,
            "sourceMode": "persistent_plan_master_handoff",
            "workspacePlanRevision": {
                "id": revision.id,
                "contentSha256": revision.content_sha256,
                "evidenceManifestSha256": revision.evidence_manifest_sha256,
            },
            "workspacePlanMarkdown": revision.plan_markdown,
            "ids": {
                "selectedArtifactIds": [],
                "selectedDerivativeIds": [],
                "selectedClipIds": [],
            },
            "safety": [
                "read-only context packet; do not mutate workspace records",
                "do not sign notes, submit claims, send payer communications, or overwrite saved data",
            ],
            "promptSnapshot": "Synthetic plan-bound packet.",
        })
        .to_string(),
        base_note_revision: Some(fixture.note.current_revision),
        authorized_scope_json: r#"{"version":1,"categories":["packet_snapshot"]}"#
            .to_string(),
        expected_output_kind: "medical_plan_execution".to_string(),
        workspace_profile: "medical".to_string(),
        plan_schema_version: Some(1),
        source_checkpoint_id: Some(revision.source_checkpoint_id.clone()),
        source_checkpoint_sha256: Some(revision.source_checkpoint_sha256.clone()),
        readiness_json:
            r#"{"version":1,"warnings":[],"acknowledgements":[],"legacy":false}"#
                .to_string(),
        workspace_plan_revision_id: Some(revision.id.clone()),
        workspace_plan_content_sha256: Some(revision.content_sha256.clone()),
        workspace_plan_evidence_manifest_sha256: Some(
            revision.evidence_manifest_sha256.clone(),
        ),
        status: "prepared".to_string(),
        actor: "Clinician Example".to_string(),
    }
}

fn plan_bound_run_start(
    packet: &crate::WorkspaceContextPacket,
    key: &str,
) -> crate::WorkspaceAgentRunStart {
    crate::WorkspaceAgentRunStart {
        packet_id: packet.id.clone(),
        expected_client_id: packet.client_id.clone(),
        expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
        expected_workspace_plan_revision_id: packet.workspace_plan_revision_id.clone(),
        expected_workspace_plan_content_sha256: packet.workspace_plan_content_sha256.clone(),
        expected_workspace_plan_evidence_manifest_sha256: packet
            .workspace_plan_evidence_manifest_sha256
            .clone(),
        run_kind: "agent".to_string(),
        idempotency_key: key.to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
        source_thread_id: Some("master-agent-thread".to_string()),
        source_turn_id: None,
        actor: "Clinician Example".to_string(),
    }
}

#[tokio::test]
async fn context_packets_agent_runs_and_claims_preserve_exact_plan_revision_binding() {
    let fixture = fixture().await;
    let (guide, execution) = start_and_claim(
        &fixture,
        &fixture.checkpoint,
        "guide-packet-binding",
        "turn-packet-binding",
    )
    .await;
    let (chart, selected) = required_evidence(&fixture, &execution, "packet-binding").await;
    let completion = fixture
        .runtime
        .workspace()
        .complete_plan_turn(plan_completion_input(
            &execution,
            vec![chart.id, selected.id],
            "complete-packet-binding",
        ))
        .await
        .expect("decision-complete plan");
    let revision = completion.revision.expect("published plan revision");

    let packet = fixture
        .runtime
        .workspace()
        .prepare_context_packet(plan_bound_packet_input(
            &fixture,
            &revision,
            "Execute the reviewed medical plan.",
        ))
        .await
        .expect("current plan revision should bind a packet");
    assert_eq!(
        packet.workspace_plan_revision_id.as_deref(),
        Some(revision.id.as_str())
    );
    assert_eq!(
        packet.workspace_plan_content_sha256.as_deref(),
        Some(revision.content_sha256.as_str())
    );
    assert_eq!(
        packet.workspace_plan_evidence_manifest_sha256.as_deref(),
        Some(revision.evidence_manifest_sha256.as_str())
    );
    let listed = fixture
        .runtime
        .workspace()
        .list_context_packets(crate::WorkspaceContextPacketFilter {
            client_id: fixture.client.id.clone(),
            note_id: Some(fixture.note.id.clone()),
            limit: Some(10),
        })
        .await
        .expect("packet list");
    assert_eq!(
        listed[0].workspace_plan_revision_id,
        Some(revision.id.clone())
    );

    let mut substituted = plan_bound_run_start(&packet, "agent-packet-binding");
    substituted.expected_workspace_plan_content_sha256 = Some("f".repeat(64));
    let error = fixture
        .runtime
        .workspace()
        .start_agent_run(substituted)
        .await
        .expect_err("caller substitution must fail")
        .to_string();
    assert!(error.contains("expected plan binding does not match"));

    let run = fixture
        .runtime
        .workspace()
        .start_agent_run(plan_bound_run_start(&packet, "agent-packet-binding"))
        .await
        .expect("packet-bound run");
    assert_eq!(
        run.workspace_plan_revision_id,
        packet.workspace_plan_revision_id
    );
    assert_eq!(
        run.workspace_plan_content_sha256,
        packet.workspace_plan_content_sha256
    );
    assert_eq!(
        run.workspace_plan_evidence_manifest_sha256,
        packet.workspace_plan_evidence_manifest_sha256
    );
    let replayed_run = fixture
        .runtime
        .workspace()
        .start_agent_run(plan_bound_run_start(&packet, "agent-packet-binding"))
        .await
        .expect("exact run replay");
    assert_eq!(replayed_run.id, run.id);
    assert_eq!(
        replayed_run.workspace_plan_evidence_manifest_sha256,
        run.workspace_plan_evidence_manifest_sha256
    );

    let agent_execution = crate::WorkspaceAgentExecutionBinding {
        run_id: run.id.clone(),
        source_thread_id: run
            .source_thread_id
            .clone()
            .expect("packet-bound run has a source thread"),
        source_turn_id: "master-agent-turn".to_string(),
        provider: run.provider.clone(),
        model: run.model.clone(),
    };
    let handoff_prompt = crate::render_workspace_agent_handoff_prompt(
        &crate::WorkspaceAgentHandoffPromptInput::from(&packet),
        Some(&run.id),
    );
    let error = fixture
        .runtime
        .workspace()
        .claim_agent_turn(crate::WorkspaceAgentTurnClaim {
            execution: agent_execution.clone(),
            prompt: handoff_prompt.clone(),
        })
        .await
        .expect_err("a current plan revision must not authorize the master turn")
        .to_string();
    assert!(error.contains("has no durable submission receipt"));

    let submit_input = crate::WorkspacePlanRevisionSubmit {
        revision_id: revision.id.clone(),
        plan_session_id: fixture.plan_session.id.clone(),
        client_id: fixture.client.id.clone(),
        packet_id: packet.id.clone(),
        agent_run_id: run.id.clone(),
        source_checkpoint_id: revision.source_checkpoint_id.clone(),
        source_checkpoint_revision: revision.source_checkpoint_revision,
        source_checkpoint_sha256: revision.source_checkpoint_sha256.clone(),
        content_sha256: revision.content_sha256.clone(),
        actor: "Clinician Example".to_string(),
    };
    let mut wrong_handoff = submit_input.clone();
    wrong_handoff.agent_run_id = "wrong-agent-run".to_string();
    let error = fixture
        .runtime
        .workspace()
        .submit_plan_revision(wrong_handoff.clone())
        .await
        .expect_err("revision submission requires the exact packet run")
        .to_string();
    assert!(error.contains("was not found for plan submission"));
    let still_current = fixture
        .runtime
        .workspace()
        .get_plan_revision(&revision.id, &fixture.plan_session.id, &fixture.client.id)
        .await
        .expect("revision read after rejected handoff")
        .expect("revision remains present");
    assert_eq!(
        still_current.status,
        crate::WorkspacePlanRevisionStatus::Current
    );

    let submitted = fixture
        .runtime
        .workspace()
        .submit_plan_revision(submit_input.clone())
        .await
        .expect("revision submit");
    assert_eq!(
        submitted.status,
        crate::WorkspacePlanRevisionStatus::Submitted
    );
    let receipt = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
SELECT plan_revision_id, packet_id, agent_run_id, plan_content_sha256,
       evidence_manifest_sha256
FROM workspace_plan_submission_receipts
WHERE plan_revision_id = ?
        "#,
    )
    .bind(&submitted.id)
    .fetch_one(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("submission receipt should be durable");
    assert_eq!(
        receipt,
        (
            submitted.id.clone(),
            packet.id.clone(),
            run.id.clone(),
            submitted.content_sha256.clone(),
            submitted.evidence_manifest_sha256.clone(),
        )
    );
    let submission_receipts = fixture
        .runtime
        .workspace()
        .list_plan_submission_receipts(
            &fixture.plan_session.id,
            &fixture.client.id,
            std::slice::from_ref(&submitted.id),
        )
        .await
        .expect("exact immutable submission receipt lookup");
    assert_eq!(
        submission_receipts,
        vec![crate::WorkspacePlanSubmissionReceipt {
            plan_revision_id: submitted.id.clone(),
            packet_id: packet.id.clone(),
            agent_run_id: run.id.clone(),
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            plan_content_sha256: submitted.content_sha256.clone(),
            evidence_manifest_sha256: submitted.evidence_manifest_sha256.clone(),
            submitted_by: "Clinician Example".to_string(),
            submitted_at: submitted
                .submitted_at
                .as_ref()
                .expect("submitted revision timestamp")
                .to_owned(),
        }]
    );
    assert!(
        fixture
            .runtime
            .workspace()
            .list_plan_submission_receipts(&fixture.plan_session.id, &fixture.client.id, &[],)
            .await
            .expect("empty receipt lookup")
            .is_empty()
    );
    let missing_receipt_error = fixture
        .runtime
        .workspace()
        .list_plan_submission_receipts(
            &fixture.plan_session.id,
            &fixture.client.id,
            &[
                submitted.id.clone(),
                "missing-submitted-revision".to_string(),
            ],
        )
        .await
        .expect_err("receipt lookup must not return a partial handoff")
        .to_string();
    assert!(missing_receipt_error.contains("missing their exact immutable submission receipt"));
    for status in ["failed", "canceled"] {
        let error = fixture
            .runtime
            .workspace()
            .update_agent_run_status(crate::WorkspaceAgentRunStatusUpdate {
                run_id: run.id.clone(),
                status: status.to_string(),
                error_summary: "synthetic enqueue interruption".to_string(),
                actor: "workspace handoff".to_string(),
            })
            .await
            .expect_err("receipt-bound unclaimed run must remain resumable")
            .to_string();
        assert!(error.contains("immutable Plan submission receipt"));
        assert!(error.contains("must remain resumable"));
    }
    assert_eq!(
        fixture
            .runtime
            .workspace()
            .reconcile_orphaned_agent_turns()
            .await
            .expect("startup recovery must preserve receipt-bound unclaimed run"),
        0
    );
    let preserved_run = fixture
        .runtime
        .workspace()
        .list_agent_runs(crate::WorkspaceAgentRunFilter {
            client_id: fixture.client.id.clone(),
            packet_id: Some(packet.id.clone()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .expect("preserved run lookup")
        .into_iter()
        .find(|candidate| candidate.id == run.id)
        .expect("receipt-bound run remains present");
    assert_eq!(preserved_run.status, "running");
    assert_eq!(preserved_run.source_turn_id, None);
    let claimed = fixture
        .runtime
        .workspace()
        .claim_agent_turn(crate::WorkspaceAgentTurnClaim {
            execution: agent_execution.clone(),
            prompt: handoff_prompt,
        })
        .await
        .expect("the exact submitted packet and run receipt should authorize the master turn");
    assert_eq!(claimed, agent_execution);
    let replayed_submission = fixture
        .runtime
        .workspace()
        .submit_plan_revision(submit_input)
        .await
        .expect("exact revision submission replay");
    assert!(replayed_submission.replayed);
    let error = fixture
        .runtime
        .workspace()
        .submit_plan_revision(wrong_handoff)
        .await
        .expect_err("submitted replay still requires the exact packet run")
        .to_string();
    assert!(error.contains("was not found for plan submission"));
    let submitted_packet = fixture
        .runtime
        .workspace()
        .prepare_context_packet(plan_bound_packet_input(
            &fixture,
            &submitted,
            "Replay the exact submitted medical plan.",
        ))
        .await
        .expect("exact submitted revision replay should bind");
    assert_eq!(
        submitted_packet.workspace_plan_revision_id.as_deref(),
        Some(submitted.id.as_str())
    );
    let alternate_run = fixture
        .runtime
        .workspace()
        .start_agent_run(plan_bound_run_start(
            &submitted_packet,
            "alternate-submitted-run",
        ))
        .await
        .expect("a submitted revision can prepare an exact replay run");
    let error = fixture
        .runtime
        .workspace()
        .submit_plan_revision(crate::WorkspacePlanRevisionSubmit {
            revision_id: submitted.id.clone(),
            plan_session_id: fixture.plan_session.id.clone(),
            client_id: fixture.client.id.clone(),
            packet_id: submitted_packet.id.clone(),
            agent_run_id: alternate_run.id,
            source_checkpoint_id: submitted.source_checkpoint_id.clone(),
            source_checkpoint_revision: submitted.source_checkpoint_revision,
            source_checkpoint_sha256: submitted.source_checkpoint_sha256.clone(),
            content_sha256: submitted.content_sha256.clone(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect_err("submitted replay must preserve its first durable packet and run receipt")
        .to_string();
    assert!(error.contains("submitted with a different context packet or agent run"));
    checkpoint(
        &fixture.runtime,
        &fixture.client,
        &fixture.encounter,
        &fixture.note,
        Some(&fixture.checkpoint),
        "Later chart work",
    )
    .await;
    let stale_checkpoint_replay = fixture
        .runtime
        .workspace()
        .prepare_context_packet(plan_bound_packet_input(
            &fixture,
            &submitted,
            "Replay the exact submitted plan after later chart work.",
        ))
        .await
        .expect("submitted plan replay remains valid after a later checkpoint");
    assert_eq!(
        stale_checkpoint_replay.workspace_plan_content_sha256,
        Some(submitted.content_sha256.clone())
    );

    let packet_count_before = listed.len() + 2;
    let mut wrong_markdown = plan_bound_packet_input(
        &fixture,
        &submitted,
        "Reject substituted model-visible plan markdown.",
    );
    let mut envelope: serde_json::Value =
        serde_json::from_str(&wrong_markdown.context_envelope_json).expect("test envelope JSON");
    envelope["workspacePlanMarkdown"] =
        serde_json::Value::String("Substituted plan markdown.".to_string());
    wrong_markdown.context_envelope_json = serde_json::to_string(&envelope).expect("test envelope");
    let error = fixture
        .runtime
        .workspace()
        .prepare_context_packet(wrong_markdown)
        .await
        .expect_err("model-visible plan markdown substitution must fail")
        .to_string();
    assert!(error.contains("plan markdown does not match"));

    let mut wrong_hash = plan_bound_packet_input(
        &fixture,
        &submitted,
        "Reject a substituted medical plan hash.",
    );
    wrong_hash.workspace_plan_content_sha256 = Some("e".repeat(64));
    let error = fixture
        .runtime
        .workspace()
        .prepare_context_packet(wrong_hash)
        .await
        .expect_err("packet hash substitution must fail")
        .to_string();
    assert!(error.contains("hashes do not match"));
    let packets = fixture
        .runtime
        .workspace()
        .list_context_packets(crate::WorkspaceContextPacketFilter {
            client_id: fixture.client.id.clone(),
            note_id: Some(fixture.note.id.clone()),
            limit: Some(10),
        })
        .await
        .expect("packet list after rejected substitution");
    assert_eq!(packets.len(), packet_count_before);

    assert_eq!(guide.client_id, fixture.client.id);
}

#[tokio::test]
async fn plan_completion_rejects_open_questions_without_partial_terminal_state() {
    let fixture = fixture().await;
    let (_guide, execution) = start_and_claim(
        &fixture,
        &fixture.checkpoint,
        "guide-open-question-binding",
        "turn-open-question-binding",
    )
    .await;
    let (chart, selected) = required_evidence(&fixture, &execution, "open-question-binding").await;
    let mut non_string_decisions = plan_completion_input(
        &execution,
        vec![chart.id.clone(), selected.id.clone()],
        "complete-invalid-decision-array",
    );
    non_string_decisions
        .plan
        .as_mut()
        .expect("plan artifact")
        .decisions_json = r#"[{"decision":"not a string"}]"#.to_string();
    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(non_string_decisions)
        .await
        .expect_err("decision objects must not enter a strict plan revision")
        .to_string();
    assert!(error.contains("decisions must be a JSON array of strings"));

    let mut completion_input = plan_completion_input(
        &execution,
        vec![chart.id, selected.id],
        "complete-open-question-binding",
    );
    completion_input
        .plan
        .as_mut()
        .expect("plan artifact")
        .open_questions_json = r#"["Is more data needed?"]"#.to_string();
    let error = fixture
        .runtime
        .workspace()
        .complete_plan_turn(completion_input)
        .await
        .expect_err("open questions must block revision publication")
        .to_string();
    assert!(error.contains("cannot be published while open questions remain"));
    assert!(
        fixture
            .runtime
            .workspace()
            .get_plan_turn_completion(
                &execution.guide_run_id,
                &fixture.plan_session.id,
                &fixture.client.id,
            )
            .await
            .expect("completion lookup")
            .is_none(),
        "a rejected plan must not leave a terminal completion"
    );
}

#[tokio::test]
async fn context_packet_binding_rejects_plan_missing_required_evidence_category() {
    let fixture = fixture().await;
    let (guide, _execution) = start_and_claim(
        &fixture,
        &fixture.checkpoint,
        "guide-missing-evidence-binding",
        "turn-missing-evidence-binding",
    )
    .await;
    let plan_markdown = "# Incomplete evidence plan";
    let decisions_json = "[]";
    let open_questions_json = "[]";
    let content_json = serde_json::to_string(&serde_json::json!({
        "planMarkdown": plan_markdown,
        "decisions": serde_json::from_str::<serde_json::Value>(decisions_json)
            .expect("decisions JSON"),
        "openQuestions": serde_json::from_str::<serde_json::Value>(open_questions_json)
            .expect("questions JSON"),
    }))
    .expect("content JSON");
    let content_sha256 = format!("{:x}", Sha256::digest(content_json.as_bytes()));
    let evidence_manifest_json = r#"[{"category":"patient_chart"}]"#;
    let evidence_manifest_sha256 =
        format!("{:x}", Sha256::digest(evidence_manifest_json.as_bytes()));
    let revision_id = "missing-evidence-category-revision";
    sqlx::query(
        r#"
INSERT INTO workspace_plan_revisions (
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key, status,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    encounter_id, note_id, source_thread_id, source_turn_id, created_at_ms
) VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, 1, ?, 'current', ?, ?, ?, ?, ?, ?, ?, 1)
        "#,
    )
    .bind(revision_id)
    .bind(&fixture.plan_session.id)
    .bind(&fixture.client.id)
    .bind(&guide.id)
    .bind(plan_markdown)
    .bind(decisions_json)
    .bind(open_questions_json)
    .bind(&content_sha256)
    .bind(evidence_manifest_json)
    .bind(&evidence_manifest_sha256)
    .bind("missing-evidence-category")
    .bind(&fixture.checkpoint.id)
    .bind(fixture.checkpoint.revision)
    .bind(&fixture.checkpoint.content_sha256)
    .bind(&fixture.encounter.id)
    .bind(&fixture.note.id)
    .bind("planning-thread")
    .bind("turn-missing-evidence-binding")
    .execute(fixture.runtime.workspace().pool.as_ref())
    .await
    .expect("synthetic adversarial plan revision");
    let revision = fixture
        .runtime
        .workspace()
        .get_plan_revision(revision_id, &fixture.plan_session.id, &fixture.client.id)
        .await
        .expect("revision read")
        .expect("revision exists");

    let error = fixture
        .runtime
        .workspace()
        .prepare_context_packet(plan_bound_packet_input(
            &fixture,
            &revision,
            "Do not execute an under-evidenced plan.",
        ))
        .await
        .expect_err("required evidence categories must be reasserted at handoff")
        .to_string();
    assert!(error.contains("missing required `selected_context`"));
}
