use super::*;
use pretty_assertions::assert_eq;

#[test]
fn workspace_dashboard_failed_patient_selection_is_atomic_and_retryable() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests("Jordan Patient", "Jordan note", "Jordan body.");
        dashboard.save(&mut app_server).await?;
        dashboard.select_client(&mut app_server, usize::MAX).await?;
        dashboard.set_context_for_tests("Riley Patient", "Riley note", "Riley body.");
        dashboard.save(&mut app_server).await?;

        let jordan_index = dashboard
            .client_index_for_display_name_for_tests("Jordan Patient")
            .expect("Jordan should be listed");
        let riley_index = dashboard
            .client_index_for_display_name_for_tests("Riley Patient")
            .expect("Riley should be listed");
        dashboard
            .select_client(&mut app_server, jordan_index)
            .await?;
        let jordan_id = dashboard
            .client_id_for_tests()
            .expect("Jordan should be active")
            .to_string();
        assert_eq!(
            dashboard.checkpoint_client_id_for_tests(),
            Some(jordan_id.as_str())
        );

        app_server.fail_next_workspace_document_list_for_tests();
        let error = dashboard
            .select_client(&mut app_server, riley_index)
            .await
            .expect_err("mid-reload document failure should abort patient selection");
        assert!(
            error
                .to_string()
                .contains("injected workspace/document/list")
        );
        assert_eq!(dashboard.client_id_for_tests(), Some(jordan_id.as_str()));
        assert_eq!(dashboard.client_display_name_for_tests(), "Jordan Patient");
        assert_eq!(dashboard.note_title_for_tests(), "Jordan note");
        assert_eq!(dashboard.note_body_for_tests(), "Jordan body.");
        assert_eq!(
            dashboard.checkpoint_client_id_for_tests(),
            Some(jordan_id.as_str())
        );

        dashboard
            .select_client(&mut app_server, riley_index)
            .await?;
        assert_eq!(dashboard.client_display_name_for_tests(), "Riley Patient");
        assert_eq!(dashboard.note_title_for_tests(), "Riley note");
        assert_eq!(dashboard.note_body_for_tests(), "Riley body.");
        assert_eq!(
            dashboard.checkpoint_client_id_for_tests(),
            dashboard.client_id_for_tests()
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn medical_generation_zero_new_patient_save_bootstraps_local_checkpoint() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests(
            "Generation Zero Patient",
            "Initial note",
            "No note_edit call created this draft.",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let client_id = app
            .workspace_dashboard
            .as_ref()
            .and_then(WorkspaceDashboard::client_id_for_tests)
            .expect("local bootstrap should create the canonical patient")
            .to_string();
        let sessions = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(
            sessions.data[0].status,
            codex_app_server_protocol::WorkspaceDraftSessionStatus::Closed
        );
        assert_eq!(
            sessions.data[0].current_checkpoint.draft["note"]["body"],
            "No note_edit call created this draft."
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn medical_generation_zero_new_patient_remote_save_rejects_before_canonical_creation() -> Result<()>
{
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests(
            "Remote Generation Zero Patient",
            "Initial note",
            "Must remain local and unsaved.",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;

        app_server.set_thread_params_mode_for_tests(ThreadParamsMode::Remote);
        app.save_workspace_with_checkpoint(&mut app_server).await;
        app_server.set_thread_params_mode_for_tests(ThreadParamsMode::Embedded);

        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("rejected remote save should retain the draft");
        assert!(dashboard.client_id_for_tests().is_none());
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("no workspace snapshot was sent")
        );
        assert!(app_server.workspace_client_list().await?.clients.is_empty());
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn medical_bootstrap_pending_checkpoint_continues_and_closes_on_idle() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests(
            "Pending Bootstrap Patient",
            "Daily note",
            "Actual checkpoint request is held past the save boundary.",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;
        let checkpoint_gate = app_server.hold_next_workspace_draft_checkpoint_for_tests();

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("pending bootstrap should keep the dashboard loaded");
        let client_id = dashboard
            .client_id_for_tests()
            .expect("canonical bootstrap should allocate the patient before checkpoint completion")
            .to_string();
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("still saving")
        );
        assert!(!dashboard.can_clear_dashboard_checkpoint_safely());
        assert!(
            app_server
                .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                    client_id: client_id.clone(),
                    include_closed: true,
                    cursor: None,
                    limit: Some(10),
                })
                .await?
                .data
                .is_empty(),
            "held request must not reach the checkpoint RPC before release"
        );

        checkpoint_gate.add_permits(1);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                app.checkpoint_idle_workspace_draft(&mut app_server).await;
                if app.workspace_dashboard.as_ref().is_some_and(|dashboard| {
                    dashboard.draft_checkpoint_status_for_tests()
                        == "Canonical chart saved; local draft checkpoint session closed."
                }) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;

        let sessions = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(
            sessions.data[0].status,
            codex_app_server_protocol::WorkspaceDraftSessionStatus::Closed
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn medical_bootstrap_unknown_first_checkpoint_error_stays_blocked_without_retry() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests(
            "Blocked Bootstrap Patient",
            "Daily note",
            "Canonical save succeeds before an unknown first checkpoint outcome.",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;
        let checkpoint_gate = app_server.hold_next_workspace_draft_checkpoint_for_tests();
        checkpoint_gate.close();

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("unknown checkpoint outcome must retain the dashboard");
        let client_id = dashboard
            .client_id_for_tests()
            .expect("canonical bootstrap should already be saved")
            .to_string();
        let blocked_status = dashboard.draft_checkpoint_status_for_tests().to_string();
        assert!(blocked_status.contains("outcome is unknown"));
        assert!(blocked_status.contains("automatic retry is blocked"));
        assert!(!dashboard.can_clear_dashboard_checkpoint_safely());
        assert_eq!(dashboard.draft_checkpoint_pending_delay(), None);

        app.checkpoint_idle_workspace_draft(&mut app_server).await;
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .expect("idle tick must retain blocked draft")
                .draft_checkpoint_status_for_tests(),
            blocked_status
        );
        let sessions = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert!(sessions.data.is_empty());
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn medical_existing_session_close_failure_arms_retry_and_clear_protection() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        dashboard.load(&mut app_server).await?;
        dashboard.set_context_for_tests("Jordan Patient", "Daily note", "Canonical note.");
        dashboard.save(&mut app_server).await?;
        dashboard.set_note_body_for_tests("Checkpointed and canonically saved body.");
        dashboard
            .checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::FocusChange,
            )
            .await?;
        let client_id = dashboard
            .client_id_for_tests()
            .expect("saved patient should have an id")
            .to_string();
        dashboard.corrupt_confirmed_checkpoint_sha_for_tests();
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("failed close must retain the dashboard");
        assert!(!dashboard.can_clear_dashboard_checkpoint_safely());
        assert!(dashboard.draft_checkpoint_pending_delay().is_some());
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("draft session remains open and will retry closing")
        );
        let sessions = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(sessions.data.len(), 1);
        assert_eq!(
            sessions.data[0].status,
            codex_app_server_protocol::WorkspaceDraftSessionStatus::Active
        );
        let mut canonical = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        canonical.load(&mut app_server).await?;
        assert_eq!(
            canonical.note_body_for_tests(),
            "Checkpointed and canonically saved body."
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}
