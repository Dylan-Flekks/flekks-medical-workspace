use super::workspace_draft_recovery::recovery_action_session_id;
use super::workspace_draft_recovery::seed_recoverable_dashboard;
use super::*;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use pretty_assertions::assert_eq;

async fn session_for_client(
    app_server: &mut AppServerSession,
    client_id: &str,
    session_id: &str,
) -> Result<WorkspaceDraftSession> {
    app_server
        .workspace_draft_session_list(WorkspaceDraftSessionListParams {
            client_id: Some(client_id.to_string()),
            all_clients: false,
            include_closed: true,
            cursor: None,
            limit: Some(100),
        })
        .await?
        .data
        .into_iter()
        .find(|session| session.id == session_id)
        .ok_or_else(|| color_eyre::eyre::eyre!("test draft session was not found"))
}

#[test]
fn v1_recovery_has_no_context_and_closes_after_canonical_save() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let seeded = seed_recoverable_dashboard(
            &mut app_server,
            "Legacy Recovery Patient",
            "Recovered legacy clinician edit.",
            /*select_document*/ false,
        )
        .await?;
        let client_id = seeded
            .client_id_for_tests()
            .expect("legacy patient")
            .to_string();
        let v2_session_id = seeded
            .draft_session_id_for_tests()
            .expect("seed V2 session")
            .to_string();
        drop(seeded);
        let v2 = session_for_client(&mut app_server, &client_id, &v2_session_id).await?;
        let checkpoint = &v2.current_checkpoint;
        app_server
            .workspace_draft_session_close(WorkspaceDraftSessionCloseParams {
                session_id: v2.id.clone(),
                client_id: client_id.clone(),
                status: WorkspaceDraftSessionCloseStatus::Discarded,
                expected_current_checkpoint_id: Some(checkpoint.id.clone()),
                expected_current_checkpoint_revision: Some(checkpoint.revision),
                expected_current_checkpoint_sha256: Some(checkpoint.content_sha256.clone()),
                actor: "recovery lifecycle test".to_string(),
                reason: "replace V2 fixture with a legacy V1 fixture".to_string(),
            })
            .await?;

        let v1_draft = serde_json::json!({
            "schemaVersion": 1,
            "baseClientVersion": checkpoint.draft["baseClientVersion"].clone(),
            "client": checkpoint.draft["client"].clone(),
            "note": checkpoint.draft["note"].clone(),
            "focus": checkpoint.draft["focus"].clone(),
        });
        let v1_checkpoint = app_server
            .spawn_workspace_draft_checkpoint_create(WorkspaceDraftCheckpointCreateParams {
                session_id: None,
                session_creation_key: Some("legacy-v1-recovery-fixture".to_string()),
                client_id: client_id.clone(),
                encounter_id: checkpoint.encounter_id.clone(),
                note_id: checkpoint.note_id.clone(),
                base_note_revision: checkpoint.base_note_revision,
                draft: v1_draft,
                trigger: "focus_change".to_string(),
                actor: "recovery lifecycle test".to_string(),
            })
            .await??
            .checkpoint;

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        recovery
            .restore_current_recovery(&mut app_server, &v1_checkpoint.session_id)
            .await?;
        assert!(!recovery.has_unsent_checkpoint_context());
        assert_eq!(recovery.selected_context_counts_for_tests(), (0, 0, 0));
        app.workspace_dashboard = Some(recovery);
        app.workspace_dashboard_visible = true;
        app.save_workspace_with_checkpoint(&mut app_server).await;

        let closed =
            session_for_client(&mut app_server, &client_id, &v1_checkpoint.session_id).await?;
        assert_eq!(closed.status, WorkspaceDraftSessionStatus::Closed);
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .and_then(WorkspaceDashboard::draft_session_id_for_tests),
            None
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn v2_restore_save_restart_handoff_clear_restart_then_close() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut seeded = seed_recoverable_dashboard(
            &mut app_server,
            "Restart Recovery Patient",
            "Recovered clinician edit before canonical save.",
            /*select_document*/ true,
        )
        .await?;
        seeded.set_agent_request_for_tests("Prepare the next daily note template.");
        seeded
            .checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
            )
            .await?;
        let client_id = seeded
            .client_id_for_tests()
            .expect("restart patient")
            .to_string();
        let session_id = seeded
            .draft_session_id_for_tests()
            .expect("restart session")
            .to_string();
        drop(seeded);

        let mut first_restore = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        first_restore.load(&mut app_server).await?;
        first_restore
            .discover_draft_recovery(&mut app_server)
            .await?;
        first_restore
            .restore_current_recovery(&mut app_server, &session_id)
            .await?;
        assert!(first_restore.has_unsent_checkpoint_context());
        app.workspace_dashboard = Some(first_restore);
        app.workspace_dashboard_visible = true;
        app.save_workspace_with_checkpoint(&mut app_server).await;
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .and_then(WorkspaceDashboard::draft_session_id_for_tests),
            Some(session_id.as_str())
        );
        drop(app.workspace_dashboard.take());

        let mut second_restore = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        second_restore.load(&mut app_server).await?;
        second_restore
            .discover_draft_recovery(&mut app_server)
            .await?;
        second_restore
            .restore_current_recovery(&mut app_server, &session_id)
            .await?;
        assert!(second_restore.has_unsent_checkpoint_context());
        assert_eq!(
            second_restore.selected_context_counts_for_tests(),
            (1, 0, 0)
        );
        app.workspace_dashboard = Some(second_restore);
        app.workspace_dashboard_visible = true;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.send_workspace_context_after_checkpoint(&mut tui, &mut app_server)
            .await;
        drop(app.workspace_dashboard.take());

        let mut third_restore = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        third_restore.load(&mut app_server).await?;
        third_restore
            .discover_draft_recovery(&mut app_server)
            .await?;
        third_restore
            .restore_current_recovery(&mut app_server, &session_id)
            .await?;
        assert!(!third_restore.has_unsent_checkpoint_context());
        assert_eq!(third_restore.selected_context_counts_for_tests(), (1, 0, 0));
        app.workspace_dashboard = Some(third_restore);
        app.workspace_dashboard_visible = true;
        app.save_workspace_with_checkpoint(&mut app_server).await;

        assert_eq!(
            session_for_client(&mut app_server, &client_id, &session_id)
                .await?
                .status,
            WorkspaceDraftSessionStatus::Closed
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn restored_session_defers_remaining_queue_until_exact_canonical_close() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let first = seed_recoverable_dashboard(
            &mut app_server,
            "Deferred Recovery A",
            "Recovered A.",
            /*select_document*/ false,
        )
        .await?;
        let first_id = first
            .draft_session_id_for_tests()
            .expect("first session")
            .to_string();
        let first_client_id = first
            .client_id_for_tests()
            .expect("first client")
            .to_string();
        let second = seed_recoverable_dashboard(
            &mut app_server,
            "Deferred Recovery B",
            "Recovered B.",
            /*select_document*/ false,
        )
        .await?;
        let second_id = second
            .draft_session_id_for_tests()
            .expect("second session")
            .to_string();
        drop(first);
        drop(second);

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        while recovery_action_session_id(&mut recovery) != first_id {
            recovery.handle_key_event(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('n'),
            ));
        }
        recovery
            .restore_current_recovery(&mut app_server, &first_id)
            .await?;
        app.workspace_dashboard = Some(recovery);
        app.workspace_dashboard_visible = true;
        app.save_workspace_with_checkpoint(&mut app_server).await;
        let dashboard = app.workspace_dashboard.as_mut().expect("dashboard remains");
        assert_eq!(recovery_action_session_id(dashboard), second_id);
        assert_eq!(
            session_for_client(&mut app_server, &first_client_id, &first_id)
                .await?
                .status,
            WorkspaceDraftSessionStatus::Closed
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn discard_cas_race_refreshes_newer_checkpoint_without_discarding_it() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut updater =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let seeded = seed_recoverable_dashboard(
            &mut app_server,
            "CAS Recovery Patient",
            "Recovered body before CAS race.",
            /*select_document*/ false,
        )
        .await?;
        let client_id = seeded
            .client_id_for_tests()
            .expect("CAS client")
            .to_string();
        let session_id = seeded
            .draft_session_id_for_tests()
            .expect("CAS session")
            .to_string();
        drop(seeded);
        let original = session_for_client(&mut updater, &client_id, &session_id).await?;
        let mut newer_draft = original.current_checkpoint.draft.clone();
        newer_draft["note"]["body"] =
            serde_json::Value::String("A newer concurrently checkpointed body.".to_string());

        let mut recovery = WorkspaceDashboard::new(WorkspaceProfile::Medical);
        recovery.load(&mut app_server).await?;
        recovery.discover_draft_recovery(&mut app_server).await?;
        let (gate, reached) = app_server.hold_next_workspace_draft_session_close_for_tests();
        let update = async {
            reached.acquire().await?.forget();
            let result = updater
                .spawn_workspace_draft_checkpoint_create(WorkspaceDraftCheckpointCreateParams {
                    session_id: Some(session_id.clone()),
                    session_creation_key: None,
                    client_id: client_id.clone(),
                    encounter_id: original.current_checkpoint.encounter_id.clone(),
                    note_id: original.current_checkpoint.note_id.clone(),
                    base_note_revision: original.current_checkpoint.base_note_revision,
                    draft: newer_draft,
                    trigger: "focus_change".to_string(),
                    actor: "concurrent recovery test".to_string(),
                })
                .await?;
            gate.add_permits(1);
            result
        };
        let (discard_result, update_result) = tokio::join!(
            recovery.discard_current_recovery(&mut app_server, &session_id),
            update
        );
        update_result?;
        discard_result?;

        let current = session_for_client(&mut updater, &client_id, &session_id).await?;
        assert_eq!(current.status, WorkspaceDraftSessionStatus::Active);
        assert_eq!(current.current_revision, original.current_revision + 1);
        assert_eq!(recovery_action_session_id(&mut recovery), session_id);
        assert!(
            recovery
                .draft_checkpoint_status_for_tests()
                .contains("newer draft revision")
        );
        updater.shutdown().await?;
        app_server.shutdown().await?;
        Ok(())
    }))
}
