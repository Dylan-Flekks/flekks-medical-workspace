use super::*;

#[test]
fn checkpoint_wait_budget_is_shared_across_boundary_phases() {
    let started_at = Instant::now();
    let budget = CheckpointWaitBudget::new_at(Duration::from_secs(1), started_at);

    assert_eq!(
        budget.remaining_at(started_at + Duration::from_millis(600)),
        Duration::from_millis(400)
    );
    assert_eq!(
        budget.remaining_at(started_at + Duration::from_millis(1_100)),
        Duration::ZERO
    );
}

#[test]
fn saved_bootstrap_checkpoint_status_never_claims_canonical_is_unchanged() {
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.mark_canonical_save_pending_close();

    dashboard.set_checkpoint_pending_status();
    assert!(dashboard.status.contains("Canonical chart saved"));
    assert!(dashboard.status.contains("draft session remains open"));
    assert!(!dashboard.status.contains("canonical chart unchanged"));

    dashboard.set_checkpoint_failure_status(&color_eyre::eyre::eyre!("simulated failure"));
    assert!(dashboard.status.contains("Canonical chart saved"));
    assert!(dashboard.status.contains("draft session remains open"));
    assert!(!dashboard.status.contains("canonical chart unchanged"));
    assert!(dashboard.draft_coordinator.canonical_save_pending_close());
}
