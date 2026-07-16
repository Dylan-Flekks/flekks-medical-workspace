use super::model_tool_mode::user_turn;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::ModelToolMode;
use codex_protocol::config_types::Settings;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use serde_json::json;

pub(super) const PLAN_INSTRUCTIONS_MARKER: &str = "patient-plan-mode-instructions-marker";

pub(super) struct PlanningFixture {
    pub(super) client_id: String,
    pub(super) plan_session_id: String,
}

fn plan_collaboration(model: &str) -> CollaborationMode {
    CollaborationMode {
        mode: ModeKind::Plan,
        settings: Settings {
            model: model.to_string(),
            reasoning_effort: None,
            developer_instructions: Some(PLAN_INSTRUCTIONS_MARKER.to_string()),
        },
    }
}

pub(super) async fn configure_dedicated_plan_thread(test: &TestCodex) -> anyhow::Result<()> {
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(plan_collaboration(&test.session_configured.model)),
            model_tool_mode: Some(ModelToolMode::Disabled),
            ..Default::default()
        },
    )
    .await
}

pub(super) fn planning_turn(test: &TestCodex, prompt: &str) -> Op {
    let mut op = user_turn(prompt, Some(ModelToolMode::WorkspacePlanningOnly));
    let Op::UserInput {
        thread_settings, ..
    } = &mut op
    else {
        unreachable!("user_turn always returns user input");
    };
    thread_settings.collaboration_mode = Some(plan_collaboration(&test.session_configured.model));
    op
}

pub(super) async fn planning_fixture(test: &TestCodex) -> anyhow::Result<PlanningFixture> {
    let state_db = test
        .codex
        .state_db()
        .ok_or_else(|| anyhow::anyhow!("planning test requires SQLite state"))?;
    state_db
        .workspace()
        .provision_synthetic_workspace("core workspacePlanningOnly test fixture")
        .await?;
    let client = state_db
        .workspace()
        .upsert_client(codex_state::WorkspaceClientUpsert {
            display_name: "Synthetic Planning Patient".to_string(),
            summary: "Synthetic persistent planning fixture.".to_string(),
            ..Default::default()
        })
        .await?;
    let plan_session = state_db
        .workspace()
        .open_plan_session(codex_state::WorkspacePlanSessionOpen {
            client_id: client.id.clone(),
            created_by: "Synthetic Clinician".to_string(),
        })
        .await?;
    state_db
        .workspace()
        .bind_plan_session_thread(codex_state::WorkspacePlanSessionThreadBind {
            session_id: plan_session.id.clone(),
            client_id: client.id.clone(),
            expected_thread_id: None,
            source_thread_id: test.session_configured.thread_id.to_string(),
            actor: "Synthetic Clinician".to_string(),
        })
        .await?;
    Ok(PlanningFixture {
        client_id: client.id,
        plan_session_id: plan_session.id,
    })
}

pub(super) async fn planning_prompt(
    test: &TestCodex,
    fixture: &PlanningFixture,
    human_content: &str,
) -> anyhow::Result<String> {
    let state_db = test
        .codex
        .state_db()
        .ok_or_else(|| anyhow::anyhow!("planning test requires SQLite state"))?;
    let checkpoint = state_db
        .workspace()
        .create_draft_checkpoint(codex_state::WorkspaceDraftCheckpointCreate {
            client_id: fixture.client_id.clone(),
            draft_json: json!({
                "schemaVersion": 2,
                "note": { "title": "Synthetic daily note", "body": human_content },
            })
            .to_string(),
            trigger: "plan_message".to_string(),
            actor: "Synthetic Clinician".to_string(),
            ..Default::default()
        })
        .await?;
    let run = state_db
        .workspace()
        .start_guide_run(codex_state::WorkspaceGuideRunStart {
            client_id: fixture.client_id.clone(),
            session_id: checkpoint.session_id.clone(),
            source_checkpoint_id: checkpoint.id.clone(),
            source_checkpoint_revision: checkpoint.revision,
            source_checkpoint_sha256: checkpoint.content_sha256.clone(),
            request_json: json!({
                "kind": "patient_plan_message",
                "planSessionId": fixture.plan_session_id,
                "humanContent": human_content,
            })
            .to_string(),
            idempotency_key: format!("planning-{}", checkpoint.id),
            trigger: "plan_message".to_string(),
            actor: "Synthetic Clinician".to_string(),
            provider: test.session_configured.model_provider_id.clone(),
            model: test.session_configured.model.clone(),
            model_tool_mode: codex_state::WorkspaceGuideModelToolMode::WorkspacePlanningOnly,
        })
        .await?;
    Ok(format!(
        "Patient-scoped planning request.\n- run_id: {}\n- plan_session_id: {}\n- patient_id: {}\n- checkpoint_id: {}\n- checkpoint_revision: {}\n- checkpoint_sha256: {}\n\nClinician message:\n{}",
        run.id,
        fixture.plan_session_id,
        fixture.client_id,
        checkpoint.id,
        checkpoint.revision,
        checkpoint.content_sha256,
        human_content,
    ))
}
