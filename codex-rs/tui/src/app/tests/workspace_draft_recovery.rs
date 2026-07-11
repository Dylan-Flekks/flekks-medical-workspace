use super::*;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use pretty_assertions::assert_eq;

pub(super) async fn seed_recoverable_dashboard(
    app_server: &mut AppServerSession,
    patient_name: &str,
    recovered_body: &str,
    select_document: bool,
) -> Result<WorkspaceDashboard> {
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.load(app_server).await?;
    dashboard.select_client(app_server, usize::MAX).await?;
    dashboard.set_context_for_tests(
        patient_name,
        "Daily treatment note",
        "Canonical clinician note.",
    );
    dashboard.save(app_server).await?;
    if select_document {
        dashboard.set_document_draft_for_tests(
            "lab",
            "Recovery source document",
            "/tmp/recovery-source.pdf",
            "Synthetic recovery test metadata.",
        );
        dashboard.save(app_server).await?;
        assert_eq!(
            dashboard.execute_workspace_command(":artifact select 1"),
            WorkspaceDashboardAction::Consumed
        );
    }
    dashboard.set_note_body_for_tests(recovered_body);
    let outcome = dashboard
        .checkpoint_draft(
            app_server,
            crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
        )
        .await?;
    assert_eq!(
        outcome,
        crate::workspace_dashboard::DashboardCheckpointOutcome::Saved
    );
    Ok(dashboard)
}

pub(super) fn recovery_action_session_id(dashboard: &mut WorkspaceDashboard) -> String {
    match dashboard.handle_key_event(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('r'),
    )) {
        WorkspaceDashboardAction::RestoreRecoveryDraft { session_id } => session_id,
        action => panic!("expected recovery restore action, got {action:?}"),
    }
}

#[test]
fn discovery_pages_all_patients_excludes_owned_and_remote_sends_no_rpc() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;

        let mut owned = seed_recoverable_dashboard(
            &mut app_server,
            "Recovery Patient A",
            "Recovered body A.",
            /*select_document*/ false,
        )
        .await?;
        let owned_session_id = owned
            .draft_session_id_for_tests()
            .expect("owned session")
            .to_string();
        let second = seed_recoverable_dashboard(
            &mut app_server,
            "Recovery Patient B",
            "Recovered body B.",
            /*select_document*/ false,
        )
        .await?;
        let second_id = second
            .draft_session_id_for_tests()
            .expect("second session")
            .to_string();
        let third = seed_recoverable_dashboard(
            &mut app_server,
            "Recovery Patient C",
            "Recovered body C.",
            /*select_document*/ false,
        )
        .await?;
        let third_id = third
            .draft_session_id_for_tests()
            .expect("third session")
            .to_string();
        drop(second);
        drop(third);

        owned.discover_draft_recovery(&mut app_server).await?;
        assert!(
            owned
                .draft_checkpoint_status_for_tests()
                .contains("2 other unfinished")
        );

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        let mut discovered = Vec::new();
        for index in 0..3 {
            discovered.push(recovery_action_session_id(&mut recovery));
            if index < 2 {
                assert_eq!(
                    recovery.handle_key_event(crossterm::event::KeyEvent::from(
                        crossterm::event::KeyCode::Char('n'),
                    )),
                    WorkspaceDashboardAction::Consumed
                );
            }
        }
        discovered.sort();
        let mut expected = vec![owned_session_id, second_id, third_id];
        expected.sort();
        assert_eq!(discovered, expected);

        let global_requests = app_server
            .workspace_draft_session_list_requests_for_tests()
            .iter()
            .filter(|params| params.all_clients)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(global_requests.len(), 6, "two complete three-page scans");
        assert!(global_requests.iter().all(|params| {
            params.client_id.is_none() && !params.include_closed && params.limit == Some(100)
        }));

        let requests_before_remote = app_server
            .workspace_draft_session_list_requests_for_tests()
            .len();
        app_server.set_thread_params_mode_for_tests(ThreadParamsMode::Remote);
        recovery.discover_draft_recovery(&mut app_server).await?;
        assert_eq!(
            app_server
                .workspace_draft_session_list_requests_for_tests()
                .len(),
            requests_before_remote
        );
        assert!(
            recovery
                .draft_checkpoint_status_for_tests()
                .contains("unavailable through a remote app-server")
        );
        app_server.set_thread_params_mode_for_tests(ThreadParamsMode::Embedded);
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn required_restore_failure_is_atomic_and_keeps_the_same_recovery_choice() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let seeded = seed_recoverable_dashboard(
            &mut app_server,
            "Atomic Recovery Patient",
            "Recovered body must not partially apply.",
            /*select_document*/ true,
        )
        .await?;
        let session_id = seeded
            .draft_session_id_for_tests()
            .expect("seeded session")
            .to_string();
        drop(seeded);

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        let before = (
            recovery.client_id_for_tests().map(ToString::to_string),
            recovery.note_body_for_tests().to_string(),
            recovery.selected_context_counts_for_tests(),
            recovery
                .draft_session_id_for_tests()
                .map(ToString::to_string),
        );
        app_server.fail_next_workspace_document_list_for_tests();

        let _error = recovery
            .restore_current_recovery(&mut app_server, &session_id)
            .await
            .expect_err("required document load must fail recovery atomically");

        let after = (
            recovery.client_id_for_tests().map(ToString::to_string),
            recovery.note_body_for_tests().to_string(),
            recovery.selected_context_counts_for_tests(),
            recovery
                .draft_session_id_for_tests()
                .map(ToString::to_string),
        );
        assert_eq!(after, before);
        assert_eq!(recovery_action_session_id(&mut recovery), session_id);
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn optional_restore_failure_degrades_after_atomic_adoption_and_keeps_unsent_context() -> Result<()>
{
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let seeded = seed_recoverable_dashboard(
            &mut app_server,
            "Degraded Recovery Patient",
            "Recovered clinician edit with selected context.",
            /*select_document*/ true,
        )
        .await?;
        let session_id = seeded
            .draft_session_id_for_tests()
            .expect("seeded session")
            .to_string();
        drop(seeded);

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        app_server.fail_next_workspace_note_signature_list_for_tests();
        recovery
            .restore_current_recovery(&mut app_server, &session_id)
            .await?;

        assert_eq!(
            recovery.note_body_for_tests(),
            "Recovered clinician edit with selected context."
        );
        assert_eq!(recovery.selected_context_counts_for_tests(), (1, 0, 0));
        assert_eq!(
            recovery.draft_session_id_for_tests(),
            Some(session_id.as_str())
        );
        assert!(recovery.has_unsent_checkpoint_context());
        assert!(
            recovery
                .draft_checkpoint_status_for_tests()
                .contains("optional history")
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn submitted_selection_and_lost_discard_response_reconcile_across_restart() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;

        let submitted = seed_recoverable_dashboard(
            &mut app_server,
            "Submitted Recovery Patient",
            "Recovered post-handoff clinician edit.",
            /*select_document*/ true,
        )
        .await?;
        app.workspace_dashboard = Some(submitted);
        app.workspace_dashboard_visible = true;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.send_workspace_context_after_checkpoint(&mut tui, &mut app_server)
            .await;
        let submitted_session_id = app
            .workspace_dashboard
            .as_ref()
            .and_then(WorkspaceDashboard::draft_session_id_for_tests)
            .expect("handoff keeps session active")
            .to_string();
        app.workspace_dashboard = None;

        let mut restored = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        restored.load(&mut app_server).await?;
        restored.discover_draft_recovery(&mut app_server).await?;
        restored
            .restore_current_recovery(&mut app_server, &submitted_session_id)
            .await?;
        assert_eq!(restored.selected_context_counts_for_tests(), (1, 0, 0));
        assert!(!restored.has_unsent_checkpoint_context());

        let discard_seed = seed_recoverable_dashboard(
            &mut app_server,
            "Discard Recovery Patient",
            "Draft whose close response is lost.",
            /*select_document*/ false,
        )
        .await?;
        let discard_session_id = discard_seed
            .draft_session_id_for_tests()
            .expect("discard session")
            .to_string();
        let discard_client_id = discard_seed
            .client_id_for_tests()
            .expect("discard patient")
            .to_string();
        drop(discard_seed);
        let mut discard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        discard.load(&mut app_server).await?;
        discard.discover_draft_recovery(&mut app_server).await?;
        while recovery_action_session_id(&mut discard) != discard_session_id {
            assert_eq!(
                discard.handle_key_event(crossterm::event::KeyEvent::from(
                    crossterm::event::KeyCode::Char('n'),
                )),
                WorkspaceDashboardAction::Consumed
            );
        }
        app_server.fail_next_workspace_draft_session_close_after_response_for_tests();
        discard
            .discard_current_recovery(&mut app_server, &discard_session_id)
            .await?;
        let closed = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id: Some(discard_client_id),
                all_clients: false,
                include_closed: true,
                cursor: None,
                limit: Some(100),
            })
            .await?;
        assert!(closed.data.iter().any(|session| {
            session.id == discard_session_id
                && session.status == WorkspaceDraftSessionStatus::Discarded
        }));
        app_server.shutdown().await?;
        Ok(())
    }))
}
