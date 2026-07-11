use super::*;
use codex_app_server_protocol::WorkspaceDraftCheckpointListParams;
use pretty_assertions::assert_eq;

async fn saved_dashboard_with_selected_document(
    app_server: &mut AppServerSession,
) -> Result<WorkspaceDashboard> {
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.load(app_server).await?;
    dashboard.select_client(app_server, usize::MAX).await?;
    dashboard.set_context_for_tests(
        "Checkpoint Context Patient",
        "Daily note",
        "Canonical clinician-authored note body.",
    );
    dashboard.save(app_server).await?;
    dashboard.set_document_draft_for_tests(
        "lab",
        "Saved lab",
        "/tmp/saved-lab.pdf",
        "metadata only",
    );
    dashboard.save(app_server).await?;
    dashboard.set_new_document_draft_for_tests(
        "referral",
        "Saved referral",
        "/tmp/saved-referral.pdf",
        "metadata only",
    );
    dashboard.save(app_server).await?;
    assert_eq!(dashboard.document_titles_for_tests().len(), 2);
    assert_eq!(
        dashboard.execute_workspace_command(":artifact select 1"),
        WorkspaceDashboardAction::Consumed
    );
    Ok(dashboard)
}

async fn seed_other_patient(app_server: &mut AppServerSession) -> Result<String> {
    Ok(app_server
        .workspace_client_upsert(WorkspaceClientUpsertParams {
            display_name: "Other Existing Patient".to_string(),
            summary: "Synthetic patient for scope-switch coverage.".to_string(),
            ..WorkspaceClientUpsertParams::default()
        })
        .await?
        .client
        .id)
}

fn sorted_checkpoint_drafts(
    mut checkpoints: Vec<codex_app_server_protocol::WorkspaceDraftCheckpoint>,
) -> Vec<codex_app_server_protocol::WorkspaceDraftCheckpoint> {
    checkpoints.sort_by_key(|checkpoint| checkpoint.revision);
    checkpoints
}

#[test]
fn ctrl_s_checkpoints_v2_context_before_unsupported_chart_save() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = saved_dashboard_with_selected_document(&mut app_server).await?;
        dashboard.set_agent_request_for_tests("Generate a similar daily note template.");
        dashboard.set_new_document_draft_for_tests(
            "lab",
            "Pending lab file",
            "/tmp/pending-lab.pdf",
            "unsupported chart editor must save only after V2 context",
        );
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(
            requests.len(),
            2,
            "pre-save and post-canonical V2 checkpoints"
        );
        assert_eq!(requests[0].trigger, "explicit_save");
        assert_eq!(requests[0].draft["schemaVersion"], 2);
        assert_eq!(
            requests[0].draft["agentRequestBody"],
            "Generate a similar daily note template."
        );
        assert_eq!(
            requests[0].draft["selectedArtifactIds"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(requests[1].trigger, "post_canonical_save");
        assert_eq!(
            requests[1].draft["agentRequestBody"],
            "Generate a similar daily note template."
        );
        assert_eq!(
            requests[0].draft["baseClientVersion"], requests[1].draft["baseClientVersion"],
            "a document-only commit leaves the canonical client row version unchanged"
        );
        assert_eq!(requests[0].base_note_revision, Some(1));
        assert_eq!(requests[1].base_note_revision, Some(1));
        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("dashboard remains open");
        assert_eq!(
            requests[1].draft["baseClientVersion"].as_str(),
            dashboard.client_version_for_tests()
        );
        assert!(
            dashboard
                .document_titles_for_tests()
                .contains(&"Pending lab file")
        );
        assert!(dashboard.draft_session_id_for_tests().is_some());
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("remains open for handoff")
        );
        assert_eq!(
            dashboard.note_body_for_tests(),
            "Canonical clinician-authored note body."
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn ctrl_s_note_edit_refreshes_post_canonical_v2_baseline_and_keeps_context_open() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = saved_dashboard_with_selected_document(&mut app_server).await?;
        dashboard.set_agent_request_for_tests("Keep this context across canonical save.");
        dashboard.set_note_body_for_tests("Updated canonical clinician note body.");
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;

        app.save_workspace_with_checkpoint(&mut app_server).await;

        let requests = app_server.workspace_draft_checkpoint_requests_for_tests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].trigger, "explicit_save");
        assert_eq!(requests[1].trigger, "post_canonical_save");
        assert_eq!(requests[0].base_note_revision, Some(1));
        assert_eq!(requests[0].draft["note"]["currentRevision"], 1);
        assert_eq!(requests[1].base_note_revision, Some(2));
        assert_eq!(requests[1].draft["note"]["currentRevision"], 2);
        assert_eq!(
            requests[0].draft["agentRequestBody"],
            requests[1].draft["agentRequestBody"]
        );
        assert_eq!(
            requests[0].draft["selectedArtifactIds"],
            requests[1].draft["selectedArtifactIds"]
        );
        let dashboard = app
            .workspace_dashboard
            .as_ref()
            .expect("context stays open");
        assert_eq!(dashboard.note_revision_for_tests(), 2);
        assert_eq!(
            dashboard.note_body_for_tests(),
            "Updated canonical clinician note body."
        );
        assert_eq!(
            requests[1].draft["baseClientVersion"].as_str(),
            dashboard.client_version_for_tests()
        );
        assert!(dashboard.draft_session_id_for_tests().is_some());
        assert!(
            dashboard
                .draft_checkpoint_status_for_tests()
                .contains("remains open for handoff")
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn unsupported_editors_still_block_handoff_close_and_non_chart_save() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;

        let mut file = saved_dashboard_with_selected_document(&mut app_server).await?;
        file.set_agent_request_for_tests("Do not bypass unsupported drafts.");
        file.set_new_document_draft_for_tests(
            "lab",
            "Unsaved lab",
            "/tmp/unsaved-lab.pdf",
            "metadata only",
        );
        assert_eq!(
            file.checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::Handoff,
            )
            .await?,
            crate::workspace_dashboard::DashboardCheckpointOutcome::Unavailable
        );
        assert_eq!(
            file.checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::Close,
            )
            .await?,
            crate::workspace_dashboard::DashboardCheckpointOutcome::Unavailable
        );

        let mut addendum = saved_dashboard_with_selected_document(&mut app_server).await?;
        addendum.set_agent_request_for_tests("Addendum must remain a blocker.");
        addendum.set_addendum_draft_for_tests("Unsaved clinician addendum.");
        for trigger in [
            crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
            crate::workspace_draft::WorkspaceDraftCheckpointTrigger::Handoff,
        ] {
            assert_eq!(
                addendum.checkpoint_draft(&mut app_server, trigger).await?,
                crate::workspace_dashboard::DashboardCheckpointOutcome::Unavailable
            );
        }

        let mut result = saved_dashboard_with_selected_document(&mut app_server).await?;
        result.set_agent_request_for_tests("Returned work must remain a blocker.");
        result.set_agent_result_draft_for_tests("Unsaved returned agent work.");
        for trigger in [
            crate::workspace_draft::WorkspaceDraftCheckpointTrigger::ExplicitSave,
            crate::workspace_draft::WorkspaceDraftCheckpointTrigger::Handoff,
        ] {
            assert_eq!(
                result.checkpoint_draft(&mut app_server, trigger).await?,
                crate::workspace_dashboard::DashboardCheckpointOutcome::Unavailable
            );
        }
        assert!(
            app_server
                .workspace_draft_checkpoint_requests_for_tests()
                .is_empty(),
            "blocked routes must not serialize or send partial snapshots"
        );
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn handoff_wrapper_checkpoints_request_then_cleared_state_and_later_closes() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let other_client_id = seed_other_patient(&mut app_server).await?;
        let mut dashboard = saved_dashboard_with_selected_document(&mut app_server).await?;
        dashboard.set_agent_request_for_tests("First durable agent request.");
        let client_id = dashboard
            .client_id_for_tests()
            .expect("saved patient")
            .to_string();
        let canonical_note = dashboard.note_body_for_tests().to_string();
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;
        let mut tui = crate::tui::test_support::make_test_tui()?;

        app.send_workspace_context_after_checkpoint(&mut tui, &mut app_server)
            .await;

        assert!(!app.workspace_dashboard_active());
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .expect("handoff retains dashboard")
                .context_packet_count_for_tests(),
            1
        );
        let sessions = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id: Some(client_id.clone()),
                all_clients: false,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(sessions.data.len(), 1);
        let session_id = sessions.data[0].id.clone();
        let first = sorted_checkpoint_drafts(
            app_server
                .workspace_draft_checkpoint_list(WorkspaceDraftCheckpointListParams {
                    client_id: client_id.clone(),
                    session_id: session_id.clone(),
                    cursor: None,
                    limit: Some(20),
                })
                .await?
                .data,
        );
        assert_eq!(first.len(), 2);
        assert_eq!(
            first[0].draft["agentRequestBody"],
            "First durable agent request."
        );
        assert_eq!(first[1].draft["agentRequestBody"], "");
        assert_eq!(
            first[0].draft["selectedArtifactIds"],
            first[1].draft["selectedArtifactIds"]
        );
        let first_selected_ids = first[0].draft["selectedArtifactIds"].clone();

        let dashboard = app
            .workspace_dashboard
            .as_mut()
            .expect("dashboard for second request");
        assert_eq!(
            dashboard.execute_workspace_command(":artifact deselect 1"),
            WorkspaceDashboardAction::Consumed
        );
        assert_eq!(
            dashboard.execute_workspace_command(":artifact select 2"),
            WorkspaceDashboardAction::Consumed
        );
        dashboard.set_agent_request_for_tests("Second durable agent request.");
        app.workspace_dashboard_visible = true;
        app.send_workspace_context_after_checkpoint(&mut tui, &mut app_server)
            .await;

        let second = sorted_checkpoint_drafts(
            app_server
                .workspace_draft_checkpoint_list(WorkspaceDraftCheckpointListParams {
                    client_id: client_id.clone(),
                    session_id: session_id.clone(),
                    cursor: None,
                    limit: Some(20),
                })
                .await?
                .data,
        );
        assert_eq!(second.len(), 4);
        assert_eq!(
            second[2].draft["agentRequestBody"],
            "Second durable agent request."
        );
        assert_eq!(second[3].draft["agentRequestBody"], "");
        assert_ne!(second[2].draft["selectedArtifactIds"], first_selected_ids);
        assert_eq!(
            second[2].draft["selectedArtifactIds"],
            second[3].draft["selectedArtifactIds"]
        );
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .expect("second handoff retains dashboard")
                .context_packet_count_for_tests(),
            2
        );
        assert_eq!(
            app.workspace_dashboard
                .as_ref()
                .expect("canonical note remains loaded")
                .note_body_for_tests(),
            canonical_note
        );

        app.workspace_dashboard_visible = true;
        app.save_workspace_with_checkpoint(&mut app_server).await;
        let closed = app_server
            .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                client_id: Some(client_id),
                all_clients: false,
                include_closed: true,
                cursor: None,
                limit: Some(10),
            })
            .await?;
        assert_eq!(
            closed.data[0].status,
            codex_app_server_protocol::WorkspaceDraftSessionStatus::Closed
        );
        let dashboard = app
            .workspace_dashboard
            .as_mut()
            .expect("closed dashboard remains");
        assert_eq!(dashboard.selected_context_counts_for_tests(), (1, 0, 0));
        let other_index = dashboard
            .client_index_for_display_name_for_tests("Other Existing Patient")
            .expect("other saved patient should be listed");
        dashboard
            .select_client(&mut app_server, other_index)
            .await?;
        assert_eq!(
            dashboard.client_id_for_tests(),
            Some(other_client_id.as_str())
        );
        assert_eq!(
            dashboard.checkpoint_client_id_for_tests(),
            Some(other_client_id.as_str())
        );
        assert_eq!(dashboard.selected_context_counts_for_tests(), (0, 0, 0));
        app_server.shutdown().await?;
        Ok(())
    }))
}

#[test]
fn hidden_dashboard_draw_completes_pending_cleared_handoff_checkpoint() -> Result<()> {
    run_workspace_dashboard_runtime_test(Box::pin(async {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        app.config.sqlite_home = codex_home.path().to_path_buf();
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut dashboard = saved_dashboard_with_selected_document(&mut app_server).await?;
        dashboard.set_agent_request_for_tests("Clear me only after durable handoff.");
        let client_id = dashboard
            .client_id_for_tests()
            .expect("saved patient")
            .to_string();
        dashboard
            .checkpoint_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::Handoff,
            )
            .await?;
        let session_id = dashboard
            .draft_session_id_for_tests()
            .expect("pre-handoff checkpoint session")
            .to_string();
        app.workspace_dashboard = Some(dashboard);
        app.workspace_dashboard_visible = true;
        assert!(app.send_workspace_context_to_agent(&mut app_server).await?);
        assert!(!app.workspace_dashboard_active());

        let gate = app_server.hold_next_workspace_draft_checkpoint_for_tests();
        assert_eq!(
            app.checkpoint_workspace_draft(
                &mut app_server,
                crate::workspace_draft::WorkspaceDraftCheckpointTrigger::HandoffCleared,
            )
            .await?,
            crate::workspace_dashboard::DashboardCheckpointOutcome::Pending
        );
        gate.add_permits(1);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
                    .await?;
                let checkpoints = app_server
                    .workspace_draft_checkpoint_list(WorkspaceDraftCheckpointListParams {
                        client_id: client_id.clone(),
                        session_id: session_id.clone(),
                        cursor: None,
                        limit: Some(20),
                    })
                    .await?;
                let continuation_consumed = app
                    .workspace_dashboard
                    .as_ref()
                    .is_some_and(|dashboard| dashboard.draft_checkpoint_pending_delay().is_none());
                if checkpoints.data.len() == 2 && continuation_consumed {
                    return Ok::<(), color_eyre::Report>(());
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;

        assert!(!app.workspace_dashboard_active());
        let checkpoints = sorted_checkpoint_drafts(
            app_server
                .workspace_draft_checkpoint_list(WorkspaceDraftCheckpointListParams {
                    client_id,
                    session_id,
                    cursor: None,
                    limit: Some(20),
                })
                .await?
                .data,
        );
        assert_eq!(
            checkpoints[0].draft["agentRequestBody"],
            "Clear me only after durable handoff."
        );
        assert_eq!(checkpoints[1].draft["agentRequestBody"], "");
        app_server.shutdown().await?;
        Ok(())
    }))
}
