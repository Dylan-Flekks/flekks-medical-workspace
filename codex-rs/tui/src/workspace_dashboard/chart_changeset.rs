use codex_app_server_protocol::WorkspaceChartCommitParams;
use codex_app_server_protocol::WorkspaceChartEntityKind;
use codex_app_server_protocol::WorkspaceChartExpectedVersions;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChartChangesetPurpose {
    #[default]
    General,
    SaveDerivative,
    SaveClip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ChartChangesetReviewState {
    Ready,
    RetryableFailure {
        summary: String,
    },
    Conflict {
        summary: String,
        target: Option<WorkspaceChartEntityKind>,
    },
    MergeRequired {
        summary: String,
        target: Option<WorkspaceChartEntityKind>,
    },
    Blocked {
        summary: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingChartChangeset {
    params: WorkspaceChartCommitParams,
    review_state: ChartChangesetReviewState,
    purpose: ChartChangesetPurpose,
}

impl PendingChartChangeset {
    #[cfg(test)]
    pub(super) fn new(mut params: WorkspaceChartCommitParams) -> Self {
        params.idempotency_key = format!("tui-chart-commit-{}", Uuid::new_v4());
        Self {
            params,
            review_state: ChartChangesetReviewState::Ready,
            purpose: ChartChangesetPurpose::General,
        }
    }

    pub(super) fn new_for_purpose(
        mut params: WorkspaceChartCommitParams,
        purpose: ChartChangesetPurpose,
    ) -> Self {
        params.idempotency_key = format!("tui-chart-commit-{}", Uuid::new_v4());
        Self {
            params,
            review_state: ChartChangesetReviewState::Ready,
            purpose,
        }
    }

    #[cfg(test)]
    pub(super) fn new_for_tests(
        mut params: WorkspaceChartCommitParams,
        idempotency_key: &str,
    ) -> Self {
        params.idempotency_key = idempotency_key.to_string();
        Self {
            params,
            review_state: ChartChangesetReviewState::Ready,
            purpose: ChartChangesetPurpose::General,
        }
    }

    pub(super) fn request(&self) -> WorkspaceChartCommitParams {
        self.params.clone()
    }

    pub(super) fn params(&self) -> &WorkspaceChartCommitParams {
        &self.params
    }

    pub(super) fn review_state(&self) -> &ChartChangesetReviewState {
        &self.review_state
    }

    pub(super) fn purpose(&self) -> ChartChangesetPurpose {
        self.purpose
    }

    #[cfg(test)]
    pub(super) fn can_retry(&self) -> bool {
        matches!(
            self.review_state,
            ChartChangesetReviewState::Ready | ChartChangesetReviewState::RetryableFailure { .. }
        )
    }

    pub(super) fn needs_canonical_refresh(&self) -> bool {
        matches!(
            self.review_state,
            ChartChangesetReviewState::Conflict { .. }
        )
    }

    pub(super) fn needs_manual_merge(&self) -> bool {
        matches!(
            self.review_state,
            ChartChangesetReviewState::MergeRequired { .. }
        )
    }

    pub(super) fn is_blocked(&self) -> bool {
        matches!(self.review_state, ChartChangesetReviewState::Blocked { .. })
    }

    pub(super) fn merge_target(&self) -> Option<WorkspaceChartEntityKind> {
        match &self.review_state {
            ChartChangesetReviewState::Conflict { target, .. }
            | ChartChangesetReviewState::MergeRequired { target, .. } => *target,
            ChartChangesetReviewState::Ready
            | ChartChangesetReviewState::RetryableFailure { .. }
            | ChartChangesetReviewState::Blocked { .. } => None,
        }
    }

    pub(super) fn mark_retryable_failure(&mut self, summary: impl Into<String>) {
        self.review_state = ChartChangesetReviewState::RetryableFailure {
            summary: summary.into(),
        };
    }

    pub(super) fn mark_conflict(
        &mut self,
        summary: impl Into<String>,
        target: Option<WorkspaceChartEntityKind>,
    ) {
        self.review_state = ChartChangesetReviewState::Conflict {
            summary: summary.into(),
            target,
        };
    }

    pub(super) fn mark_merge_required(&mut self, summary: impl Into<String>) {
        let target = self.merge_target();
        self.review_state = ChartChangesetReviewState::MergeRequired {
            summary: summary.into(),
            target,
        };
    }

    pub(super) fn mark_canonical_refreshed(&mut self) {
        if self.can_manual_merge_after_canonical_refresh() {
            self.mark_merge_required(
                "Canonical note refreshed; compare it with the preserved human draft and make an explicit merge edit.",
            );
        } else {
            self.mark_blocked(
                "Canonical chart refreshed, but this stale non-note or multi-entity draft cannot be merged safely. Close to discard it, then reopen to load canonical data.",
            );
        }
    }

    pub(super) fn mark_blocked(&mut self, summary: impl Into<String>) {
        self.review_state = ChartChangesetReviewState::Blocked {
            summary: summary.into(),
        };
    }

    pub(super) fn included_entity_labels(&self) -> Vec<&'static str> {
        let params = &self.params;
        let mut labels = Vec::new();
        if params.client.is_some() {
            labels.push("demographics");
        }
        if params.coverage.is_some() {
            labels.push("coverage");
        }
        if params.safety_item.is_some() {
            labels.push("safety");
        }
        if params.encounter.is_some() {
            labels.push("encounter");
        }
        if params.note.is_some() {
            labels.push("note");
        }
        if params.document.is_some() {
            labels.push("document");
        }
        if params.artifact_derivative.is_some() {
            labels.push("reviewed text");
        }
        if params.context_clip.is_some() {
            labels.push("context clip");
        }
        if params.task.is_some() {
            labels.push("job");
        }
        labels
    }

    pub(super) fn expected_guard_count(&self) -> usize {
        let Some(WorkspaceChartExpectedVersions {
            client,
            coverage,
            safety_item,
            encounter,
            document,
            artifact_derivative,
            context_clip,
            task,
        }) = self.params.expected_versions.as_ref()
        else {
            return 0;
        };
        [
            client,
            coverage,
            safety_item,
            encounter,
            document,
            artifact_derivative,
            context_clip,
            task,
        ]
        .into_iter()
        .filter(|version| version.is_some())
        .count()
    }

    fn can_manual_merge_after_canonical_refresh(&self) -> bool {
        self.merge_target() == Some(WorkspaceChartEntityKind::Note)
            && self.params.note.is_some()
            && self.params.client.is_none()
            && self.params.coverage.is_none()
            && self.params.safety_item.is_none()
            && self.params.encounter.is_none()
            && self.params.document.is_none()
            && self.params.artifact_derivative.is_none()
            && self.params.context_clip.is_none()
            && self.params.task.is_none()
            && self.expected_guard_count() == 0
    }

    pub(super) fn note_revision_guard(&self) -> Option<i64> {
        self.params
            .note
            .as_ref()
            .and_then(|note| note.expected_base_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::WorkspaceChartNoteChange;
    use codex_app_server_protocol::WorkspaceNoteUpsertParams;

    fn empty_params() -> WorkspaceChartCommitParams {
        WorkspaceChartCommitParams {
            idempotency_key: String::new(),
            actor: "local clinician".to_string(),
            reason: "Save reviewed chart changes".to_string(),
            source_thread_id: None,
            source_turn_id: None,
            client_id: Some("client-1".to_string()),
            client: None,
            coverage: None,
            expected_versions: None,
            safety_item: None,
            encounter: None,
            note: None,
            document: None,
            artifact_derivative: None,
            context_clip: None,
            task: None,
        }
    }

    #[test]
    fn unchanged_retry_clones_the_exact_request_and_key() {
        let pending = PendingChartChangeset::new(empty_params());

        let first = pending.request();
        let retry = pending.request();

        assert_eq!(first, retry);
        assert!(first.idempotency_key.starts_with("tui-chart-commit-"));
    }

    #[test]
    fn conflict_blocks_retry_without_discarding_the_request() {
        let mut pending = PendingChartChangeset::new(empty_params());
        let request = pending.request();

        pending.mark_conflict(
            "note revision changed",
            Some(WorkspaceChartEntityKind::Note),
        );

        assert!(!pending.can_retry());
        assert_eq!(pending.request(), request);
        assert!(matches!(
            pending.review_state(),
            ChartChangesetReviewState::Conflict { summary, target }
                if summary == "note revision changed"
                    && *target == Some(WorkspaceChartEntityKind::Note)
        ));
    }

    #[test]
    fn refreshed_conflict_allows_manual_merge_only_for_an_unguarded_note_only_request() {
        let mut note_only_params = empty_params();
        note_only_params.note = Some(WorkspaceChartNoteChange {
            upsert: WorkspaceNoteUpsertParams {
                id: Some("note-1".to_string()),
                client_id: "client-1".to_string(),
                encounter_id: None,
                title: "Daily note".to_string(),
                kind: "daily_note".to_string(),
                body: "Clinician draft".to_string(),
                status: "draft".to_string(),
                summary: None,
            },
            expected_base_revision: Some(3),
        });
        let mut note_only = PendingChartChangeset::new(note_only_params.clone());
        note_only.mark_conflict("note changed", Some(WorkspaceChartEntityKind::Note));
        note_only.mark_canonical_refreshed();

        assert!(matches!(
            note_only.review_state(),
            ChartChangesetReviewState::MergeRequired {
                target: Some(WorkspaceChartEntityKind::Note),
                ..
            }
        ));

        note_only_params.expected_versions = Some(WorkspaceChartExpectedVersions {
            client: Some("client-version-2".to_string()),
            ..WorkspaceChartExpectedVersions::default()
        });
        let mut mixed = PendingChartChangeset::new(note_only_params);
        mixed.mark_conflict("note changed", Some(WorkspaceChartEntityKind::Note));
        mixed.mark_canonical_refreshed();

        assert!(matches!(
            mixed.review_state(),
            ChartChangesetReviewState::Blocked { .. }
        ));

        let mut non_note = PendingChartChangeset::new(empty_params());
        non_note.mark_conflict("client changed", Some(WorkspaceChartEntityKind::Client));
        non_note.mark_canonical_refreshed();

        assert!(matches!(
            non_note.review_state(),
            ChartChangesetReviewState::Blocked { .. }
        ));
    }
}
