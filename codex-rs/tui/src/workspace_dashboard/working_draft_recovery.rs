use super::*;

impl WorkspaceDashboard {
    pub(crate) fn medical_working_draft(
        &self,
    ) -> std::result::Result<Option<MedicalWorkspaceWorkingDraftV1>, WorkspaceDraftError> {
        if self.profile != WorkspaceProfile::Medical {
            return Ok(None);
        }
        let Some(client_id) = self.draft_client.id.clone() else {
            return Ok(None);
        };
        MedicalWorkspaceWorkingDraftV1::new(MedicalWorkspaceWorkingDraftInput {
            client_id,
            note_id: self.draft_note.id.clone(),
            working_note_id: self.draft_note.working_note_id.clone(),
            encounter_id: self.draft_note.encounter_id.clone(),
            base_note_revision: self
                .draft_note
                .id
                .as_ref()
                .map(|_| self.draft_note.current_revision),
            note_title: self.draft_note.title.clone(),
            note_body: self.draft_note.body.clone(),
            agent_request_body: self.agent_request.body.clone(),
            selected_file_ids: self.selected_artifact_ids.iter().cloned().collect(),
            selected_reviewed_text_ids: self.selected_derivative_ids.iter().cloned().collect(),
            selected_clip_ids: self.selected_clip_ids.iter().cloned().collect(),
        })
        .map(Some)
    }

    pub(crate) fn apply_recovered_medical_working_draft(
        &mut self,
        recovered: MedicalWorkspaceWorkingDraftV1,
    ) -> std::result::Result<(), WorkspaceDraftError> {
        if self.profile != WorkspaceProfile::Medical {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "medical working draft cannot be restored in a generic workspace".to_string(),
            ));
        }
        if self.draft_client.id.as_deref() != Some(recovered.client_id.as_str()) {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "working draft belongs to a different patient".to_string(),
            ));
        }
        if self.draft_note.id != recovered.note.note_id {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "working draft belongs to a different note".to_string(),
            ));
        }
        if self.draft_note.encounter_id != recovered.note.encounter_id {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "working draft belongs to a different encounter".to_string(),
            ));
        }
        if self.draft_note.id.is_some()
            && recovered.note.base_revision != Some(self.draft_note.current_revision)
        {
            return Err(WorkspaceDraftError::InvalidRecovery(format!(
                "saved note advanced from draft base r{} to r{}; compare manually before applying",
                recovered.note.base_revision.unwrap_or_default(),
                self.draft_note.current_revision
            )));
        }

        self.draft_note.working_note_id = recovered.note.working_note_id;
        self.draft_note.title = recovered.note.title;
        self.draft_note.body = recovered.note.body;
        self.note_body_editor.reset(&self.draft_note.body);
        self.agent_request.body = recovered.agent_request_body;
        self.agent_request.active = !self.agent_request.body.trim().is_empty();
        self.agent_request_editor.reset(&self.agent_request.body);

        let document_ids = self
            .documents
            .iter()
            .map(|document| document.id.as_str())
            .collect::<BTreeSet<_>>();
        let derivative_ids = self
            .derivatives
            .iter()
            .map(|derivative| derivative.id.as_str())
            .collect::<BTreeSet<_>>();
        let clip_ids = self
            .clips
            .iter()
            .map(|clip| clip.id.as_str())
            .collect::<BTreeSet<_>>();
        self.selected_artifact_ids = recovered
            .selected_file_ids
            .into_iter()
            .filter(|id| document_ids.contains(id.as_str()))
            .collect();
        self.selected_derivative_ids = recovered
            .selected_reviewed_text_ids
            .into_iter()
            .filter(|id| derivative_ids.contains(id.as_str()))
            .collect();
        self.selected_clip_ids = recovered
            .selected_clip_ids
            .into_iter()
            .filter(|id| clip_ids.contains(id.as_str()))
            .collect();

        self.dirty = self.medical_note_tree_draft_label().is_some();
        self.draft_recovery_available = false;
        self.draft_persistence_message = Some(
            "Recovered local working state; Ctrl-S is still required for the canonical chart."
                .to_string(),
        );
        if self.agent_request.is_active() {
            self.focus = WorkspaceFocus::Agent;
            self.agent_rail_tab = AgentRailTab::Pending;
            self.set_workflow_section(MedicalWorkflowSection::AgentRequest);
        } else {
            self.focus = WorkspaceFocus::NoteBody;
        }
        self.status =
            "Recovered local draft for review; canonical chart was not changed.".to_string();
        Ok(())
    }

    pub(crate) fn set_draft_recovery_available(&mut self, available: bool) {
        self.draft_recovery_available = available;
        if available {
            self.draft_persistence_message = Some(
                "Recovery available: Ctrl-P → Restore local draft or Discard local draft."
                    .to_string(),
            );
        }
    }

    pub(crate) fn set_draft_persistence_message(&mut self, message: Option<String>) {
        self.draft_persistence_message = message;
    }
}
