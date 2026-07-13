use super::*;

pub(super) fn coverage_draft_has_input(draft: &CoverageDraft) -> bool {
    CoverageField::ALL.iter().any(|field| {
        field.is_toggle() && draft.subscriber_address_same_as_patient
            || draft
                .text(*field)
                .is_some_and(|value| !value.trim().is_empty())
    })
}

impl WorkspaceDashboard {
    pub(super) fn start_coverage_verification(&mut self) {
        if self.dirty {
            self.status =
                "Save the patient and coverage before recording a card comparison.".to_string();
            return;
        }
        let Some(coverage_id) = self.coverage_draft.id.clone() else {
            self.status = "Save this coverage before recording a card comparison.".to_string();
            return;
        };
        let Some(coverage_version) = self.coverage_draft.version.clone() else {
            self.status = "Reload this coverage before recording a card comparison.".to_string();
            return;
        };
        let Some(client_id) = self.draft_client.id.as_deref() else {
            self.status = "Save the patient before recording a card comparison.".to_string();
            return;
        };
        let Some(patient_version) = self
            .clients
            .iter()
            .find(|client| client.id == client_id)
            .map(|client| client.version.clone())
        else {
            self.status = "Reload the patient before recording a card comparison.".to_string();
            return;
        };
        let selected_document_id = self.selected_document_id_for_file_operation();
        let source_document = selected_document_id
            .as_deref()
            .and_then(|document_id| {
                self.documents.iter().find(|document| {
                    document.id == document_id && coverage_card_document_is_eligible(document)
                })
            })
            .or_else(|| {
                self.documents
                    .iter()
                    .find(|document| coverage_card_document_is_eligible(document))
            });
        let Some(source_document) = source_document else {
            self.status =
                "Add a present, hashed local insurance-card reference before recording a comparison."
                    .to_string();
            return;
        };
        let subject = if self
            .coverage_draft
            .patient_relationship_to_subscriber
            .trim()
            .eq_ignore_ascii_case("self")
            || (self.coverage_draft.priority == CoveragePriority::Primary
                && self
                    .coverage_draft
                    .coverage_type
                    .to_ascii_lowercase()
                    .contains("medicare"))
        {
            WorkspaceCoverageVerificationSubject::Beneficiary
        } else {
            WorkspaceCoverageVerificationSubject::Subscriber
        };
        let mut draft = CardVerificationDraft::new(coverage_id, subject);
        draft.expected_patient_version = patient_version;
        draft.expected_coverage_version = coverage_version;
        draft.expected_document_version = source_document.version.clone();
        draft.source_document_id = source_document.id.clone();
        draft.actor = "local clinician".to_string();
        self.card_verification_draft = Some(draft);
        self.card_verification_field = CardVerificationField::SourceDocument;
        self.status =
            "Human card comparison ready. Type printed values exactly; Ctrl-S records append-only verification."
                .to_string();
    }

    pub(super) fn request_coverage_verification_create(&mut self) -> WorkspaceDashboardAction {
        if self.dirty {
            self.status =
                "Close card comparison, save patient/coverage changes, then verify the current versions."
                    .to_string();
            return WorkspaceDashboardAction::Consumed;
        }
        let Some(draft) = self.card_verification_draft.as_ref() else {
            return WorkspaceDashboardAction::Consumed;
        };
        if let Some(issue) = draft.submission_issue() {
            self.status = issue.to_string();
            return WorkspaceDashboardAction::Consumed;
        }
        WorkspaceDashboardAction::CreateCoverageVerification(
            WorkspaceCoverageVerificationCreateParams::from(draft),
        )
    }

    pub(crate) async fn create_coverage_verification(
        &mut self,
        app_server: &mut AppServerSession,
        params: WorkspaceCoverageVerificationCreateParams,
    ) -> Result<()> {
        let response = app_server
            .workspace_coverage_verification_create(params)
            .await?;
        if let Some(coverage) = self
            .coverages
            .iter_mut()
            .find(|coverage| coverage.id == response.verification.coverage_id)
        {
            coverage.billing_readiness = response.billing_readiness;
        }
        self.coverage_draft.billing_readiness = response.billing_readiness;
        self.coverage_verifications
            .retain(|verification| verification.id != response.verification.id);
        self.coverage_verifications.insert(0, response.verification);
        self.card_verification_draft = None;
        self.status = format!(
            "Card comparison recorded: {}. Clinical chart saves remain available.",
            billing_readiness_label(response.billing_readiness)
        );
        Ok(())
    }

    pub(crate) async fn select_coverage_priority(
        &mut self,
        app_server: &mut AppServerSession,
        priority: i64,
    ) -> Result<()> {
        let priority = CoveragePriority::try_from(priority)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        self.load_coverage_draft(priority);
        self.reload_coverage_verification_history(app_server)
            .await?;
        self.status = format!("Editing {} coverage.", priority.label());
        Ok(())
    }

    pub(super) fn load_coverage_draft(&mut self, priority: CoveragePriority) {
        let client_id = self.draft_client.id.clone().unwrap_or_default();
        self.coverage_draft = self
            .coverages
            .iter()
            .find(|coverage| coverage.priority == priority.number())
            .and_then(|coverage| CoverageDraft::try_from(coverage).ok())
            .unwrap_or_else(|| CoverageDraft::new(client_id, priority));
        self.coverage_field = CoverageField::PayerName;
        self.card_verification_draft = None;
        self.card_verification_field = CardVerificationField::SourceDocument;
    }

    pub(super) async fn reload_coverage_verification_history(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        let Some(coverage_id) = self.coverage_draft.id.clone() else {
            self.coverage_verifications.clear();
            return Ok(());
        };
        self.coverage_verifications = app_server
            .workspace_coverage_verification_list(WorkspaceCoverageVerificationListParams {
                coverage_id,
                cursor: None,
                limit: Some(8),
            })
            .await?
            .data;
        Ok(())
    }
}
