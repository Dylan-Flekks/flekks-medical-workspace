use super::*;

#[test]
fn newer_edit_generation_postpones_idle_checkpoint() {
    let start = Instant::now();
    let mut coordinator = WorkspaceDraftCoordinator::default();
    coordinator.note_edit_at(start);
    assert!(!coordinator.idle_checkpoint_is_due_at(start));

    let second_edit = start + Duration::from_millis(800);
    coordinator.note_edit_at(second_edit);

    assert!(
        !coordinator
            .idle_checkpoint_is_due_at(start + CHECKPOINT_IDLE_DELAY + Duration::from_millis(1))
    );
    assert!(coordinator.idle_checkpoint_is_due_at(second_edit + CHECKPOINT_IDLE_DELAY));
}

#[test]
fn handoff_forces_exact_checkpoint_without_edits() {
    let coordinator = WorkspaceDraftCoordinator::default();

    assert!(!coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::FocusChange));
    assert!(coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::Handoff));
}
