use super::*;
use pretty_assertions::assert_eq;

#[test]
fn medical_first_checkpoint_retries_with_one_key_then_uses_session_id() -> Result<()> {
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
            "Idempotent Bootstrap Patient",
            "Daily note",
            "First response will be lost after persistence.",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;
        app_server.fail_next_workspace_draft_checkpoint_after_response_for_tests();

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let first_request = app_server
            .workspace_draft_checkpoint_requests_for_tests()
            .first()
            .expect("first request should be captured")
            .clone();
        let first_key = first_request
            .session_creation_key
            .as_deref()
            .expect("first request should use a creation key")
            .to_string();
        assert!(!first_key.is_empty());
        assert!(first_key.len() <= 256);
        assert!(first_request.session_id.is_none());

        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("lost first response must retain the dashboard");
        let client_id = dashboard
            .client_id_for_tests()
            .expect("canonical bootstrap should already be saved")
            .to_string();
        assert_eq!(
            dashboard.draft_session_creation_key_for_tests(),
            Some(first_key.as_str())
        );
        assert!(dashboard.draft_session_id_for_tests().is_none());
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("will retry")
        );
        assert!(
            !dashboard
                .draft_checkpoint_status_for_tests()
                .contains(&first_key),
            "creation key must not leak into the status footer"
        );
        assert!(!dashboard.can_clear_dashboard_checkpoint_safely());
        assert!(dashboard.draft_checkpoint_pending_delay().is_some());

        let persisted = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id: Some(client_id.clone()),
                all_clients: false,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(persisted.data.len(), 1);
        let persisted_session_id = persisted.data[0].id.clone();

        app.workspace_dashboard
            .as_mut()
            .expect("dashboard should remain available for a newer edit")
            .set_note_body_for_tests("Newer edit keeps the same creation key.");
        app_server.fail_next_workspace_draft_checkpoint_after_response_for_tests();
        let _error = app
            .workspace_dashboard
            .as_mut()
            .expect("dashboard should retry the checkpoint")
            .checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
            )
            .await
            .expect_err("second persisted response should also be lost");

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].session_id.is_none());
        assert_eq!(
            requests[1].session_creation_key.as_deref(),
            Some(first_key.as_str())
        );
        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("second lost response must retain the dashboard");
        assert_eq!(
            dashboard.draft_session_creation_key_for_tests(),
            Some(first_key.as_str())
        );
        assert!(dashboard.draft_session_id_for_tests().is_none());
        assert!(
            !dashboard
                .draft_checkpoint_status_for_tests()
                .contains(&first_key),
            "retry status must not expose the creation key"
        );

        assert_eq!(
            app.workspace_dashboard
                .as_mut()
                .expect("dashboard should accept the eventual response")
                .checkpoint_draft(
                    &mut app_server,
                    crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
                )
                .await?,
            crate::workspace_dashboard::DashboardCheckpointOutcome::Saved
        );

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(requests.len(), 3);
        assert!(requests[2].session_id.is_none());
        assert_eq!(
            requests[2].session_creation_key.as_deref(),
            Some(first_key.as_str())
        );
        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("eventual response should retain the dashboard");
        assert_eq!(
            dashboard.draft_session_id_for_tests(),
            Some(persisted_session_id.as_str())
        );
        assert!(dashboard.draft_session_creation_key_for_tests().is_none());

        app.workspace_dashboard
            .as_mut()
            .expect("dashboard should accept another edit")
            .set_note_body_for_tests("Checkpoint after session adoption.");
        assert_eq!(
            app.workspace_dashboard
                .as_mut()
                .expect("dashboard should checkpoint the adopted session")
                .checkpoint_draft(
                    &mut app_server,
                    crate::workspace_draft::WorkspaceDraftCheckpointTrigger::FocusChange,
                )
                .await?,
            crate::workspace_dashboard::DashboardCheckpointOutcome::Saved
        );

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[3].session_id.as_deref(),
            Some(persisted_session_id.as_str())
        );
        assert!(requests[3].session_creation_key.is_none());

        let dashboard = app
            .workspace_dashboard
            .as_mut()
            .expect("dashboard should remain available for canonical save");
        dashboard.save(&mut app_server).await?;
        dashboard.mark_canonical_save_pending_close();
        dashboard
            .close_draft_after_canonical_save(&mut app_server)
            .await?;
        assert!(dashboard.draft_session_id_for_tests().is_none());
        assert!(dashboard.draft_session_creation_key_for_tests().is_none());
        dashboard.select_client(&mut app_server, usize::MAX).await?;
        dashboard.set_context_for_tests(
            "Rotated Key Patient",
            "Initial note",
            "A new patient scope must receive a different creation key.",
        );

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(requests.len(), 5);
        assert!(requests[4].session_id.is_none());
        let next_key = requests[4]
            .session_creation_key
            .as_deref()
            .expect("new scope should receive a creation key");
        assert!(!next_key.is_empty());
        assert_ne!(next_key, first_key);
        app_server.shutdown().await?;
        Ok(())
    }))
}
