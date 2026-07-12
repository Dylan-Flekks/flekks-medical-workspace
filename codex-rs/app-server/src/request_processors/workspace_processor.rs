use super::*;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::WorkspaceAgentContextCategory;
use codex_app_server_protocol::WorkspaceAgentResultCreateParams;
use codex_app_server_protocol::WorkspaceAgentResultCreateResponse;
use codex_app_server_protocol::WorkspaceAgentResultListParams;
use codex_app_server_protocol::WorkspaceAgentResultListResponse;
use codex_app_server_protocol::WorkspaceAgentResultStatusUpdateParams;
use codex_app_server_protocol::WorkspaceAgentResultStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceAgentRunContextReadParams;
use codex_app_server_protocol::WorkspaceAgentRunContextReadResponse;
use codex_app_server_protocol::WorkspaceAgentRunListParams;
use codex_app_server_protocol::WorkspaceAgentRunListResponse;
use codex_app_server_protocol::WorkspaceAgentRunSourceListParams;
use codex_app_server_protocol::WorkspaceAgentRunSourceListResponse;
use codex_app_server_protocol::WorkspaceAgentRunStartParams;
use codex_app_server_protocol::WorkspaceAgentRunStartResponse;
use codex_app_server_protocol::WorkspaceAgentRunStatusUpdateParams;
use codex_app_server_protocol::WorkspaceAgentRunStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceArtifactDerivativeListParams;
use codex_app_server_protocol::WorkspaceArtifactDerivativeListResponse;
use codex_app_server_protocol::WorkspaceArtifactDerivativeStatusUpdateParams;
use codex_app_server_protocol::WorkspaceArtifactDerivativeStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceArtifactDerivativeUpsertParams;
use codex_app_server_protocol::WorkspaceArtifactDerivativeUpsertResponse;
use codex_app_server_protocol::WorkspaceAuditListParams;
use codex_app_server_protocol::WorkspaceAuditListResponse;
use codex_app_server_protocol::WorkspaceClientArchiveParams;
use codex_app_server_protocol::WorkspaceClientArchiveResponse;
use codex_app_server_protocol::WorkspaceClientGetParams;
use codex_app_server_protocol::WorkspaceClientGetResponse;
use codex_app_server_protocol::WorkspaceClientListParams;
use codex_app_server_protocol::WorkspaceClientListResponse;
use codex_app_server_protocol::WorkspaceClientUpsertParams;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceContext;
use codex_app_server_protocol::WorkspaceContextClipListParams;
use codex_app_server_protocol::WorkspaceContextClipListResponse;
use codex_app_server_protocol::WorkspaceContextClipStatusUpdateParams;
use codex_app_server_protocol::WorkspaceContextClipStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceContextClipUpsertParams;
use codex_app_server_protocol::WorkspaceContextClipUpsertResponse;
use codex_app_server_protocol::WorkspaceContextGetParams;
use codex_app_server_protocol::WorkspaceContextGetResponse;
use codex_app_server_protocol::WorkspaceContextPacketCreateParams;
use codex_app_server_protocol::WorkspaceContextPacketCreateResponse;
use codex_app_server_protocol::WorkspaceContextPacketListParams;
use codex_app_server_protocol::WorkspaceContextPacketListResponse;
use codex_app_server_protocol::WorkspaceContextPacketReplay;
use codex_app_server_protocol::WorkspaceContextPacketReplayParams;
use codex_app_server_protocol::WorkspaceContextPacketReplayResponse;
use codex_app_server_protocol::WorkspaceDocumentArchiveParams;
use codex_app_server_protocol::WorkspaceDocumentArchiveResponse;
use codex_app_server_protocol::WorkspaceDocumentGetParams;
use codex_app_server_protocol::WorkspaceDocumentGetResponse;
use codex_app_server_protocol::WorkspaceDocumentListParams;
use codex_app_server_protocol::WorkspaceDocumentListResponse;
use codex_app_server_protocol::WorkspaceDocumentUpsertParams;
use codex_app_server_protocol::WorkspaceDocumentUpsertResponse;
use codex_app_server_protocol::WorkspaceEncounterListParams;
use codex_app_server_protocol::WorkspaceEncounterListResponse;
use codex_app_server_protocol::WorkspaceEncounterUpsertParams;
use codex_app_server_protocol::WorkspaceEncounterUpsertResponse;
use codex_app_server_protocol::WorkspaceNoteAddendumCreateParams;
use codex_app_server_protocol::WorkspaceNoteAddendumCreateResponse;
use codex_app_server_protocol::WorkspaceNoteAddendumListParams;
use codex_app_server_protocol::WorkspaceNoteAddendumListResponse;
use codex_app_server_protocol::WorkspaceNoteArchiveParams;
use codex_app_server_protocol::WorkspaceNoteArchiveResponse;
use codex_app_server_protocol::WorkspaceNoteGetParams;
use codex_app_server_protocol::WorkspaceNoteGetResponse;
use codex_app_server_protocol::WorkspaceNoteListParams;
use codex_app_server_protocol::WorkspaceNoteListResponse;
use codex_app_server_protocol::WorkspaceNoteProposalCreateParams;
use codex_app_server_protocol::WorkspaceNoteProposalCreateResponse;
use codex_app_server_protocol::WorkspaceNoteProposalDecisionKind;
use codex_app_server_protocol::WorkspaceNoteProposalDecisionListParams;
use codex_app_server_protocol::WorkspaceNoteProposalDecisionListResponse;
use codex_app_server_protocol::WorkspaceNoteProposalListParams;
use codex_app_server_protocol::WorkspaceNoteProposalListResponse;
use codex_app_server_protocol::WorkspaceNoteProposalResolveParams;
use codex_app_server_protocol::WorkspaceNoteProposalResolveResponse;
use codex_app_server_protocol::WorkspaceNoteSignParams;
use codex_app_server_protocol::WorkspaceNoteSignResponse;
use codex_app_server_protocol::WorkspaceNoteSignatureListParams;
use codex_app_server_protocol::WorkspaceNoteSignatureListResponse;
use codex_app_server_protocol::WorkspaceNoteSummary;
use codex_app_server_protocol::WorkspaceNoteUpsertParams;
use codex_app_server_protocol::WorkspaceNoteUpsertResponse;
use codex_app_server_protocol::WorkspacePatientSafetyItemArchiveParams;
use codex_app_server_protocol::WorkspacePatientSafetyItemArchiveResponse;
use codex_app_server_protocol::WorkspacePatientSafetyItemListParams;
use codex_app_server_protocol::WorkspacePatientSafetyItemListResponse;
use codex_app_server_protocol::WorkspacePatientSafetyItemUpsertParams;
use codex_app_server_protocol::WorkspacePatientSafetyItemUpsertResponse;
use codex_app_server_protocol::WorkspacePracticeLibraryItem;
use codex_app_server_protocol::WorkspacePracticeLibraryListParams;
use codex_app_server_protocol::WorkspacePracticeLibraryListResponse;
use codex_app_server_protocol::WorkspaceTaskListParams;
use codex_app_server_protocol::WorkspaceTaskListResponse;
use codex_app_server_protocol::WorkspaceTaskStatusUpdateParams;
use codex_app_server_protocol::WorkspaceTaskStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceTaskSummary;
use codex_app_server_protocol::WorkspaceTaskUpsertParams;
use codex_app_server_protocol::WorkspaceTaskUpsertResponse;

#[path = "workspace_chart_commit_processor.rs"]
mod chart_commit;
#[path = "workspace_data_policy_processor.rs"]
mod data_policy;
#[path = "workspace_draft_processor.rs"]
mod drafts;
#[path = "workspace_guide_processor.rs"]
mod guides;

const AGENT_VISIBLE_PACKET_SAFETY_CONSTRAINTS: &[&str] = &[
    "use only the stored context packet envelope",
    "additional current workspace rows require an explicitly authorized run context read and are recorded as immutable source snapshots",
    "do not read unselected artifacts, derivatives, clips, or practice records",
    "original local files are never uploaded, parsed, transcribed, OCRed, or analyzed automatically",
    "packet context is read-only and grants no write, sign, submit, payer-contact, or record-mutation authority",
];

#[derive(Clone)]
pub(crate) struct WorkspaceRequestProcessor {
    state_db: Option<StateDbHandle>,
    synthetic_provisioning_authority: data_policy::WorkspaceSyntheticProvisioningAuthority,
}

impl WorkspaceRequestProcessor {
    pub(crate) fn new(
        state_db: Option<StateDbHandle>,
        config: &codex_core::config::Config,
    ) -> Self {
        Self {
            state_db,
            synthetic_provisioning_authority:
                data_policy::WorkspaceSyntheticProvisioningAuthority::from_config(config),
        }
    }

    pub(crate) async fn client_list(
        &self,
        _params: WorkspaceClientListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let clients = state_db
            .workspace()
            .list_clients()
            .await
            .map_err(|err| internal_error(format!("failed to list workspace clients: {err}")))?
            .into_iter()
            .map(api_workspace_client_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspaceClientListResponse { clients }.into()))
    }

    pub(crate) async fn client_get(
        &self,
        params: WorkspaceClientGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let client = state_db
            .workspace()
            .get_client(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to read workspace client: {err}")))?
            .map(api_workspace_client_from_state)
            .transpose()?;
        Ok(Some(WorkspaceClientGetResponse { client }.into()))
    }

    pub(crate) async fn client_upsert(
        &self,
        params: WorkspaceClientUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let display_name = params.display_name.trim();
        if display_name.is_empty() {
            return Err(invalid_request(
                "workspace client displayName must not be empty",
            ));
        }
        let client = state_db
            .workspace()
            .upsert_client(codex_state::WorkspaceClientUpsert {
                id: empty_to_none(params.id),
                display_name: display_name.to_string(),
                preferred_name: empty_to_none(params.preferred_name),
                date_of_birth: empty_to_none(params.date_of_birth),
                sex_or_gender: empty_to_none(params.sex_or_gender),
                external_id: empty_to_none(params.external_id),
                record_start_date: empty_to_none(params.record_start_date),
                record_end_date: empty_to_none(params.record_end_date),
                summary: params.summary,
                primary_phone: empty_to_none(params.primary_phone),
                secondary_phone: empty_to_none(params.secondary_phone),
                email: empty_to_none(params.email),
                preferred_contact_method: empty_to_none(params.preferred_contact_method),
                emergency_contact_name: empty_to_none(params.emergency_contact_name),
                emergency_contact_relationship: empty_to_none(
                    params.emergency_contact_relationship,
                ),
                emergency_contact_phone: empty_to_none(params.emergency_contact_phone),
                emergency_contact_email: empty_to_none(params.emergency_contact_email),
                contact_notes: empty_to_none(params.contact_notes),
                payer_name: empty_to_none(params.payer_name),
                plan_name: empty_to_none(params.plan_name),
                member_id: empty_to_none(params.member_id),
                group_number: empty_to_none(params.group_number),
                coverage_type: empty_to_none(params.coverage_type),
                coverage_status: empty_to_none(params.coverage_status),
                coverage_notes: empty_to_none(params.coverage_notes),
            })
            .await
            .map_err(|err| internal_error(format!("failed to save workspace client: {err}")))?;
        Ok(Some(
            WorkspaceClientUpsertResponse {
                client: api_workspace_client_from_state(client)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn client_archive(
        &self,
        params: WorkspaceClientArchiveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let archived = state_db
            .workspace()
            .archive_client(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to archive workspace client: {err}")))?;
        Ok(Some(WorkspaceClientArchiveResponse { archived }.into()))
    }

    pub(crate) async fn document_list(
        &self,
        params: WorkspaceDocumentListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let documents = state_db
            .workspace()
            .list_documents(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to list workspace documents: {err}")))?
            .into_iter()
            .map(api_workspace_document_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspaceDocumentListResponse { documents }.into()))
    }

    pub(crate) async fn practice_library_list(
        &self,
        params: WorkspacePracticeLibraryListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let items = state_db
            .workspace()
            .list_practice_library_items(codex_state::WorkspacePracticeLibraryFilter {
                active_client_id: empty_to_none(params.active_client_id),
                query: empty_to_none(params.query),
                limit: params.limit,
            })
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace practice library: {err}"))
            })?
            .into_iter()
            .map(api_workspace_practice_library_item_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspacePracticeLibraryListResponse { items }.into()))
    }

    pub(crate) async fn document_get(
        &self,
        params: WorkspaceDocumentGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let document = state_db
            .workspace()
            .get_document(&params.document_id)
            .await
            .map_err(|err| internal_error(format!("failed to read workspace document: {err}")))?
            .map(api_workspace_document_from_state)
            .transpose()?;
        Ok(Some(WorkspaceDocumentGetResponse { document }.into()))
    }

    pub(crate) async fn document_upsert(
        &self,
        params: WorkspaceDocumentUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace document clientId must not be empty",
            ));
        }
        if params.title.trim().is_empty() {
            return Err(invalid_request(
                "workspace document title must not be empty",
            ));
        }
        if params.local_path.trim().is_empty() {
            return Err(invalid_request(
                "workspace document localPath must not be empty",
            ));
        }
        let document = state_db
            .workspace()
            .upsert_document(codex_state::WorkspaceDocumentUpsert {
                id: empty_to_none(params.id),
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                title: params.title.trim().to_string(),
                kind: nonempty_or(params.kind, "document"),
                local_path: params.local_path.trim().to_string(),
                notes: params.notes,
                scope: nonempty_or(params.scope, "patient"),
                detected_kind: params.detected_kind.trim().to_string(),
                mime_type: empty_to_none(params.mime_type),
                file_size_bytes: params.file_size_bytes,
                modified_at: params
                    .modified_at
                    .map(unix_seconds_to_datetime)
                    .transpose()?,
                sha256: empty_to_none(params.sha256),
                tags: params.tags,
                source_label: params.source_label,
                existence_status: nonempty_or(params.existence_status, "unknown"),
                metadata_json: nonempty_or(params.metadata_json, "{}"),
                original_path: nonempty_or(params.original_path, &params.local_path),
                reference_kind: nonempty_or(params.reference_kind, "local_reference"),
                vault_path: params.vault_path,
                content_sha256: empty_to_none(params.content_sha256),
                thumbnail_path: params.thumbnail_path,
                thumbnail_status: nonempty_or(params.thumbnail_status, "none"),
                thumbnail_mime_type: empty_to_none(params.thumbnail_mime_type),
                intake_source: params.intake_source,
                imported_at: params
                    .imported_at
                    .map(unix_seconds_to_datetime)
                    .transpose()?,
            })
            .await
            .map_err(|err| internal_error(format!("failed to save workspace document: {err}")))?;
        Ok(Some(
            WorkspaceDocumentUpsertResponse {
                document: api_workspace_document_from_state(document)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn document_archive(
        &self,
        params: WorkspaceDocumentArchiveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let archived = state_db
            .workspace()
            .archive_document(&params.document_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to archive workspace document: {err}"))
            })?;
        Ok(Some(WorkspaceDocumentArchiveResponse { archived }.into()))
    }

    pub(crate) async fn patient_safety_item_list(
        &self,
        params: WorkspacePatientSafetyItemListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace patient safety clientId must not be empty",
            ));
        }
        let items = state_db
            .workspace()
            .list_patient_safety_items(&params.client_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace patient safety: {err}"))
            })?
            .into_iter()
            .map(api_workspace_patient_safety_item_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspacePatientSafetyItemListResponse { items }.into(),
        ))
    }

    pub(crate) async fn patient_safety_item_upsert(
        &self,
        params: WorkspacePatientSafetyItemUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace patient safety clientId must not be empty",
            ));
        }
        let category = workspace_patient_safety_category(params.category)?;
        let name = params.name.trim();
        if name.is_empty() {
            return Err(invalid_request(
                "workspace patient safety name must not be empty",
            ));
        }
        let item = state_db
            .workspace()
            .upsert_patient_safety_item(codex_state::WorkspacePatientSafetyItemUpsert {
                id: empty_to_none(params.id),
                client_id: params.client_id,
                category,
                name: name.to_string(),
                reaction: empty_to_none(params.reaction),
                severity: empty_to_none(params.severity),
                dose: empty_to_none(params.dose),
                route: empty_to_none(params.route),
                frequency: empty_to_none(params.frequency),
                status: empty_to_none(params.status),
                recorded_date: empty_to_none(params.recorded_date),
                notes: params.notes,
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to save workspace patient safety: {err}"))
            })?;
        Ok(Some(
            WorkspacePatientSafetyItemUpsertResponse {
                item: api_workspace_patient_safety_item_from_state(item)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn patient_safety_item_archive(
        &self,
        params: WorkspacePatientSafetyItemArchiveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.item_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace patient safety itemId must not be empty",
            ));
        }
        let archived = state_db
            .workspace()
            .archive_patient_safety_item(&params.item_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to archive workspace patient safety: {err}"))
            })?;
        Ok(Some(
            WorkspacePatientSafetyItemArchiveResponse { archived }.into(),
        ))
    }

    pub(crate) async fn artifact_derivative_list(
        &self,
        params: WorkspaceArtifactDerivativeListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let derivatives = state_db
            .workspace()
            .list_artifact_derivatives(codex_state::WorkspaceArtifactDerivativeFilter {
                client_id: params.client_id,
                document_id: empty_to_none(params.document_id),
                note_id: empty_to_none(params.note_id),
                limit: params.limit,
            })
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to list workspace artifact derivatives: {err}"
                ))
            })?
            .into_iter()
            .map(api_workspace_artifact_derivative_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspaceArtifactDerivativeListResponse { derivatives }.into(),
        ))
    }

    pub(crate) async fn artifact_derivative_upsert(
        &self,
        params: WorkspaceArtifactDerivativeUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace artifact derivative clientId must not be empty",
            ));
        }
        if params.document_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace artifact derivative documentId must not be empty",
            ));
        }
        if params.title.trim().is_empty() {
            return Err(invalid_request(
                "workspace artifact derivative title must not be empty",
            ));
        }
        if params.body.trim().is_empty() {
            return Err(invalid_request(
                "workspace artifact derivative body must not be empty",
            ));
        }
        let review_status = nonempty_or(params.review_status, "draft");
        if !matches!(
            review_status.as_str(),
            "draft" | "human_reviewed" | "superseded" | "archived"
        ) {
            return Err(invalid_request(
                "workspace artifact derivative reviewStatus must be draft, human_reviewed, superseded, or archived",
            ));
        }
        let derivative = state_db
            .workspace()
            .upsert_artifact_derivative(codex_state::WorkspaceArtifactDerivativeUpsert {
                id: empty_to_none(params.id),
                document_id: params.document_id,
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                note_id: empty_to_none(params.note_id),
                kind: nonempty_or(params.kind, "human annotation"),
                title: params.title.trim().to_string(),
                body: params.body,
                review_status,
                source_method: nonempty_or(params.source_method, "human_typed"),
                page_range: params.page_range,
                timestamp_range: params.timestamp_range,
                segment_label: params.segment_label,
                tags: params.tags,
                metadata_json: nonempty_or(params.metadata_json, "{}"),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to save workspace artifact derivative: {err}"
                ))
            })?;
        Ok(Some(
            WorkspaceArtifactDerivativeUpsertResponse {
                derivative: api_workspace_artifact_derivative_from_state(derivative)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn artifact_derivative_status_update(
        &self,
        params: WorkspaceArtifactDerivativeStatusUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.derivative_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace artifact derivative derivativeId must not be empty",
            ));
        }
        let review_status = params.review_status.trim();
        if !matches!(
            review_status,
            "draft" | "human_reviewed" | "superseded" | "archived"
        ) {
            return Err(invalid_request(
                "workspace artifact derivative reviewStatus must be draft, human_reviewed, superseded, or archived",
            ));
        }
        let derivative = state_db
            .workspace()
            .update_artifact_derivative_status(
                codex_state::WorkspaceArtifactDerivativeStatusUpdate {
                    derivative_id: params.derivative_id,
                    review_status: review_status.to_string(),
                    actor: "human".to_string(),
                },
            )
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to update workspace artifact derivative: {err}"
                ))
            })?
            .map(api_workspace_artifact_derivative_from_state)
            .transpose()?;
        Ok(Some(
            WorkspaceArtifactDerivativeStatusUpdateResponse { derivative }.into(),
        ))
    }

    pub(crate) async fn context_clip_list(
        &self,
        params: WorkspaceContextClipListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let clips = state_db
            .workspace()
            .list_context_clips(codex_state::WorkspaceContextClipFilter {
                client_id: params.client_id,
                derivative_id: empty_to_none(params.derivative_id),
                document_id: empty_to_none(params.document_id),
                note_id: empty_to_none(params.note_id),
                limit: params.limit,
            })
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace context clips: {err}"))
            })?
            .into_iter()
            .map(api_workspace_context_clip_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspaceContextClipListResponse { clips }.into()))
    }

    pub(crate) async fn context_clip_upsert(
        &self,
        params: WorkspaceContextClipUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip clientId must not be empty",
            ));
        }
        if params.document_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip documentId must not be empty",
            ));
        }
        if params.derivative_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip derivativeId must not be empty",
            ));
        }
        if params.title.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip title must not be empty",
            ));
        }
        if params.body.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip body must not be empty",
            ));
        }
        let review_status = nonempty_or(params.review_status, "draft");
        if !matches!(
            review_status.as_str(),
            "draft" | "human_reviewed" | "superseded" | "archived"
        ) {
            return Err(invalid_request(
                "workspace context clip reviewStatus must be draft, human_reviewed, superseded, or archived",
            ));
        }
        let clip = state_db
            .workspace()
            .upsert_context_clip(codex_state::WorkspaceContextClipUpsert {
                id: empty_to_none(params.id),
                derivative_id: params.derivative_id,
                document_id: params.document_id,
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                note_id: empty_to_none(params.note_id),
                kind: nonempty_or(params.kind, "generic excerpt"),
                title: params.title.trim().to_string(),
                body: params.body,
                review_status,
                source_method: nonempty_or(params.source_method, "human_selected"),
                page_range: params.page_range,
                timestamp_range: params.timestamp_range,
                line_range: params.line_range,
                segment_label: params.segment_label,
                tags: params.tags,
                metadata_json: nonempty_or(params.metadata_json, "{}"),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to save workspace context clip: {err}"))
            })?;
        Ok(Some(
            WorkspaceContextClipUpsertResponse {
                clip: api_workspace_context_clip_from_state(clip)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn context_clip_status_update(
        &self,
        params: WorkspaceContextClipStatusUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.clip_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context clip clipId must not be empty",
            ));
        }
        let review_status = params.review_status.trim();
        if !matches!(
            review_status,
            "draft" | "human_reviewed" | "superseded" | "archived"
        ) {
            return Err(invalid_request(
                "workspace context clip reviewStatus must be draft, human_reviewed, superseded, or archived",
            ));
        }
        let clip = state_db
            .workspace()
            .update_context_clip_status(codex_state::WorkspaceContextClipStatusUpdate {
                clip_id: params.clip_id,
                review_status: review_status.to_string(),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to update workspace context clip: {err}"))
            })?
            .map(api_workspace_context_clip_from_state)
            .transpose()?;
        Ok(Some(
            WorkspaceContextClipStatusUpdateResponse { clip }.into(),
        ))
    }

    pub(crate) async fn task_list(
        &self,
        params: WorkspaceTaskListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let tasks = state_db
            .workspace()
            .list_tasks(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to list workspace tasks: {err}")))?
            .into_iter()
            .map(api_workspace_task_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspaceTaskListResponse { tasks }.into()))
    }

    pub(crate) async fn task_upsert(
        &self,
        params: WorkspaceTaskUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request("workspace task clientId must not be empty"));
        }
        let title = params.title.trim();
        if title.is_empty() {
            return Err(invalid_request("workspace task title must not be empty"));
        }
        let task = state_db
            .workspace()
            .upsert_task(codex_state::WorkspaceTaskUpsert {
                id: empty_to_none(params.id),
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                note_id: empty_to_none(params.note_id),
                document_id: empty_to_none(params.document_id),
                title: title.to_string(),
                details: params.details,
                kind: nonempty_or(params.kind, "task"),
                status: state_workspace_task_status_from_api(params.status),
                priority: state_workspace_task_priority_from_api(params.priority),
                due_date: empty_to_none(params.due_date),
                assigned_to: empty_to_none(params.assigned_to),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| invalid_request(format!("failed to save workspace task: {err}")))?;
        Ok(Some(
            WorkspaceTaskUpsertResponse {
                task: api_workspace_task_from_state(task)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn task_status_update(
        &self,
        params: WorkspaceTaskStatusUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request("workspace task clientId must not be empty"));
        }
        if params.task_id.trim().is_empty() {
            return Err(invalid_request("workspace task taskId must not be empty"));
        }
        let task = state_db
            .workspace()
            .update_task_status(codex_state::WorkspaceTaskStatusUpdate {
                client_id: params.client_id,
                task_id: params.task_id,
                status: state_workspace_task_status_from_api(params.status),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to update workspace task status: {err}"))
            })?
            .map(api_workspace_task_from_state)
            .transpose()?;
        Ok(Some(WorkspaceTaskStatusUpdateResponse { task }.into()))
    }

    pub(crate) async fn encounter_list(
        &self,
        params: WorkspaceEncounterListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let encounters = state_db
            .workspace()
            .list_encounters(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to list workspace encounters: {err}")))?
            .into_iter()
            .map(api_workspace_encounter_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(WorkspaceEncounterListResponse { encounters }.into()))
    }

    pub(crate) async fn encounter_upsert(
        &self,
        params: WorkspaceEncounterUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace encounter clientId must not be empty",
            ));
        }
        let title = params.title.trim();
        if title.is_empty() {
            return Err(invalid_request(
                "workspace encounter title must not be empty",
            ));
        }
        let encounter = state_db
            .workspace()
            .upsert_encounter(codex_state::WorkspaceEncounterUpsert {
                id: empty_to_none(params.id),
                client_id: params.client_id,
                kind: nonempty_or(params.kind, "encounter"),
                title: title.to_string(),
                status: nonempty_or(params.status, "open"),
                started_at: params
                    .started_at
                    .map(unix_seconds_to_datetime)
                    .transpose()?,
                ended_at: params.ended_at.map(unix_seconds_to_datetime).transpose()?,
            })
            .await
            .map_err(|err| internal_error(format!("failed to save workspace encounter: {err}")))?;
        Ok(Some(
            WorkspaceEncounterUpsertResponse {
                encounter: api_workspace_encounter_from_state(encounter)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn context_get(
        &self,
        params: WorkspaceContextGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let client_id = params.client_id.trim();
        if client_id.is_empty() {
            return Err(invalid_request(
                "workspace context clientId must not be empty",
            ));
        }

        let Some(client) = state_db
            .workspace()
            .get_client(client_id)
            .await
            .map_err(|err| internal_error(format!("failed to read workspace context: {err}")))?
        else {
            return Ok(Some(WorkspaceContextGetResponse { context: None }.into()));
        };

        let active_note = match params
            .note_id
            .as_deref()
            .map(str::trim)
            .filter(|note_id| !note_id.is_empty())
        {
            Some(note_id) => state_db
                .workspace()
                .get_note(note_id)
                .await
                .map_err(|err| {
                    internal_error(format!("failed to read workspace context note: {err}"))
                })?
                .filter(|note| note.client_id == client.id)
                .map(api_workspace_note_from_state),
            None => None,
        };

        let notes = state_db
            .workspace()
            .list_notes(&client.id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace context notes: {err}"))
            })?;
        let recent_notes = notes
            .iter()
            .take(10)
            .map(api_workspace_note_summary_from_state)
            .collect();

        let documents = if params.include_documents.unwrap_or(true) {
            state_db
                .workspace()
                .list_documents(&client.id)
                .await
                .map_err(|err| {
                    internal_error(format!("failed to list workspace context documents: {err}"))
                })?
                .into_iter()
                .map(api_workspace_document_from_state)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let tasks = state_db
            .workspace()
            .list_open_tasks(&client.id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace context tasks: {err}"))
            })?
            .into_iter()
            .take(10)
            .map(api_workspace_task_summary_from_state)
            .collect();

        Ok(Some(
            WorkspaceContextGetResponse {
                context: Some(WorkspaceContext {
                    client: api_workspace_client_from_state(client)?,
                    active_note,
                    recent_notes,
                    documents,
                    tasks,
                }),
            }
            .into(),
        ))
    }

    pub(crate) async fn context_packet_list(
        &self,
        params: WorkspaceContextPacketListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context packet clientId must not be empty",
            ));
        }
        let packets = state_db
            .workspace()
            .list_context_packets(codex_state::WorkspaceContextPacketFilter {
                client_id: params.client_id,
                note_id: empty_to_none(params.note_id),
                limit: params.limit,
            })
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace context packets: {err}"))
            })?
            .into_iter()
            .map(api_workspace_context_packet_from_state)
            .collect();
        Ok(Some(WorkspaceContextPacketListResponse { packets }.into()))
    }

    pub(crate) async fn context_packet_create(
        &self,
        params: WorkspaceContextPacketCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context packet clientId must not be empty",
            ));
        }
        if params.human_request.trim().is_empty() {
            return Err(invalid_request(
                "workspace context packet humanRequest must not be empty",
            ));
        }
        let clinician_actor = params
            .clinician_actor
            .as_deref()
            .map(str::trim)
            .filter(|actor| !actor.is_empty())
            .unwrap_or("local clinician")
            .to_string();
        let authorized_scope_json = params
            .authorized_scope_json
            .filter(|scope| !scope.trim().is_empty())
            .unwrap_or_else(|| "{\"version\":1,\"categories\":[\"packet_snapshot\"]}".to_string());
        let expected_output_kind = params
            .expected_output_kind
            .filter(|kind| !kind.trim().is_empty())
            .unwrap_or_else(|| "note_proposal".to_string());
        let packet = state_db
            .workspace()
            .prepare_context_packet(codex_state::WorkspaceContextPacketCreate {
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                note_id: empty_to_none(params.note_id),
                human_request: params.human_request.trim().to_string(),
                selected_artifact_ids_json: if params.selected_artifact_ids_json.trim().is_empty() {
                    "[]".to_string()
                } else {
                    params.selected_artifact_ids_json
                },
                selected_derivative_ids_json: if params
                    .selected_derivative_ids_json
                    .trim()
                    .is_empty()
                {
                    "[]".to_string()
                } else {
                    params.selected_derivative_ids_json
                },
                selected_clip_ids_json: if params.selected_clip_ids_json.trim().is_empty() {
                    "[]".to_string()
                } else {
                    params.selected_clip_ids_json
                },
                artifact_summary: params.artifact_summary,
                derivative_summary: params.derivative_summary,
                clip_summary: params.clip_summary,
                chart_context_summary: params.chart_context_summary,
                context_envelope_json: if params.context_envelope_json.trim().is_empty() {
                    "{}".to_string()
                } else {
                    params.context_envelope_json
                },
                base_note_revision: params.base_note_revision,
                authorized_scope_json,
                expected_output_kind,
                status: "prepared".to_string(),
                actor: clinician_actor,
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to create workspace context packet: {err}"))
            })?;
        Ok(Some(
            WorkspaceContextPacketCreateResponse {
                packet: api_workspace_context_packet_from_state(packet),
            }
            .into(),
        ))
    }

    pub(crate) async fn context_packet_replay(
        &self,
        params: WorkspaceContextPacketReplayParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context packet replay clientId must not be empty",
            ));
        }
        if params.packet_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace context packet replay packetId must not be empty",
            ));
        }
        let replay = state_db
            .workspace()
            .get_context_packet_replay(codex_state::WorkspaceContextPacketReplayFilter {
                client_id: params.client_id,
                packet_id: params.packet_id,
                context_envelope_sha256: empty_to_none(params.context_envelope_sha256)
                    .unwrap_or_default(),
            })
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to read workspace context packet replay: {err}"
                ))
            })?
            .map(api_workspace_context_packet_replay_from_state);
        Ok(Some(WorkspaceContextPacketReplayResponse { replay }.into()))
    }

    pub(crate) async fn agent_run_start(
        &self,
        params: WorkspaceAgentRunStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.packet_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run packetId must not be empty",
            ));
        }
        if params.idempotency_key.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run idempotencyKey must not be empty",
            ));
        }
        let run = self
            .state_db()?
            .workspace()
            .start_agent_run(codex_state::WorkspaceAgentRunStart {
                packet_id: params.packet_id,
                expected_client_id: params.client_id.unwrap_or_default(),
                expected_context_envelope_sha256: params
                    .context_envelope_sha256
                    .unwrap_or_default(),
                run_kind: "agent".to_string(),
                idempotency_key: params.idempotency_key,
                provider: params.provider.unwrap_or_default(),
                model: params.model.unwrap_or_default(),
                source_thread_id: empty_to_none(params.source_thread_id),
                source_turn_id: empty_to_none(params.source_turn_id),
                actor: "local clinician".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to start workspace agent run: {err}"))
            })?;
        Ok(Some(
            WorkspaceAgentRunStartResponse {
                run: api_workspace_agent_run_from_state(run),
            }
            .into(),
        ))
    }

    pub(crate) async fn agent_run_list(
        &self,
        params: WorkspaceAgentRunListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run clientId must not be empty",
            ));
        }
        let runs = self
            .state_db()?
            .workspace()
            .list_agent_runs(codex_state::WorkspaceAgentRunFilter {
                client_id: params.client_id,
                note_id: empty_to_none(params.note_id),
                packet_id: empty_to_none(params.packet_id),
                limit: params.limit,
            })
            .await
            .map_err(|err| internal_error(format!("failed to list workspace agent runs: {err}")))?
            .into_iter()
            .map(api_workspace_agent_run_from_state)
            .collect();
        Ok(Some(WorkspaceAgentRunListResponse { runs }.into()))
    }

    pub(crate) async fn agent_run_status_update(
        &self,
        params: WorkspaceAgentRunStatusUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.run_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run runId must not be empty",
            ));
        }
        let status = params.status.trim();
        if !matches!(status, "failed" | "canceled") {
            return Err(invalid_request(
                "workspace agent run status must be failed or canceled; result creation owns completion",
            ));
        }
        let run = self
            .state_db()?
            .workspace()
            .update_agent_run_status(codex_state::WorkspaceAgentRunStatusUpdate {
                run_id: params.run_id,
                status: status.to_string(),
                error_summary: params.error_summary.unwrap_or_default(),
                actor: "agent harness".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to update workspace agent run status: {err}"
                ))
            })?
            .map(api_workspace_agent_run_from_state);
        Ok(Some(WorkspaceAgentRunStatusUpdateResponse { run }.into()))
    }

    pub(crate) async fn agent_run_source_list(
        &self,
        params: WorkspaceAgentRunSourceListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.run_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run source runId must not be empty",
            ));
        }
        let limit = params.limit.unwrap_or(100).clamp(1, 500) as usize;
        let sources = self
            .state_db()?
            .workspace()
            .list_agent_run_sources(&params.run_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace agent run sources: {err}"))
            })?
            .into_iter()
            .take(limit)
            .map(api_workspace_agent_run_source_from_state)
            .collect();
        Ok(Some(WorkspaceAgentRunSourceListResponse { sources }.into()))
    }

    pub(crate) async fn agent_run_context_read(
        &self,
        params: WorkspaceAgentRunContextReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.run_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent run context read runId must not be empty",
            ));
        }
        let category = params.category;
        let category_name = match category {
            WorkspaceAgentContextCategory::VisitHistory => "visit_history",
            WorkspaceAgentContextCategory::ProgressNotes => "progress_notes",
        };
        let read = self
            .state_db()?
            .workspace()
            .read_authorized_agent_context(codex_state::WorkspaceAgentContextReadRequest {
                run_id: params.run_id,
                category: category_name.to_string(),
                max_records: params.limit,
            })
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to read authorized workspace agent context: {err}"
                ))
            })?;
        let sources = read
            .sources
            .into_iter()
            .map(api_workspace_agent_run_source_from_state)
            .collect();
        Ok(Some(
            WorkspaceAgentRunContextReadResponse { category, sources }.into(),
        ))
    }

    pub(crate) async fn agent_result_list(
        &self,
        params: WorkspaceAgentResultListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent result clientId must not be empty",
            ));
        }
        let results = state_db
            .workspace()
            .list_agent_results(codex_state::WorkspaceAgentResultFilter {
                client_id: params.client_id,
                note_id: empty_to_none(params.note_id),
                packet_id: empty_to_none(params.packet_id),
                limit: params.limit,
            })
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace agent results: {err}"))
            })?
            .into_iter()
            .map(api_workspace_agent_result_from_state)
            .collect();
        Ok(Some(WorkspaceAgentResultListResponse { results }.into()))
    }

    pub(crate) async fn agent_result_create(
        &self,
        params: WorkspaceAgentResultCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.packet_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent result packetId must not be empty",
            ));
        }
        if params.body.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent result body must not be empty",
            ));
        }
        let summary = params
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|summary| !summary.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| compact_text(&params.body, 80));
        let run_id = empty_to_none(params.run_id);
        let linked_run = run_id.is_some();
        let input = codex_state::WorkspaceAgentResultCreate {
            packet_id: params.packet_id,
            run_id,
            source_thread_id: empty_to_none(params.source_thread_id),
            source_turn_id: empty_to_none(params.source_turn_id),
            body: params.body,
            summary,
            result_kind: params
                .result_kind
                .filter(|kind| !kind.trim().is_empty())
                .unwrap_or_else(|| "recommendation".to_string()),
            structured_changes_json: params
                .structured_changes_json
                .filter(|changes| !changes.trim().is_empty())
                .unwrap_or_else(|| "[]".to_string()),
            rationale_summary: params.rationale_summary.unwrap_or_default(),
            status: "review_pending".to_string(),
            actor: if linked_run {
                "agent harness".to_string()
            } else {
                "local clinician".to_string()
            },
            expected_client_id: empty_to_none(params.client_id),
            expected_note_id: empty_to_none(params.note_id),
            expected_context_envelope_sha256: empty_to_none(params.context_envelope_sha256)
                .unwrap_or_default(),
        };
        let result = if linked_run {
            state_db
                .workspace()
                .complete_agent_run_with_result(input)
                .await
        } else {
            state_db.workspace().create_agent_result(input).await
        }
        .map_err(|err| {
            invalid_request(format!("failed to create workspace agent result: {err}"))
        })?;
        Ok(Some(
            WorkspaceAgentResultCreateResponse {
                result: api_workspace_agent_result_from_state(result),
            }
            .into(),
        ))
    }

    pub(crate) async fn agent_result_status_update(
        &self,
        params: WorkspaceAgentResultStatusUpdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.result_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace agent result resultId must not be empty",
            ));
        }
        let status = params.status.trim();
        if !matches!(status, "reviewed" | "dismissed") {
            return Err(invalid_request(
                "workspace agent result status must be reviewed or dismissed; proposal creation owns the converted transition",
            ));
        }
        let result = state_db
            .workspace()
            .update_agent_result_status(codex_state::WorkspaceAgentResultStatusUpdate {
                result_id: params.result_id,
                status: status.to_string(),
                actor: "human".to_string(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!(
                    "failed to update workspace agent result status: {err}"
                ))
            })?
            .map(api_workspace_agent_result_from_state);
        Ok(Some(
            WorkspaceAgentResultStatusUpdateResponse { result }.into(),
        ))
    }

    pub(crate) async fn note_list(
        &self,
        params: WorkspaceNoteListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let notes = state_db
            .workspace()
            .list_notes(&params.client_id)
            .await
            .map_err(|err| internal_error(format!("failed to list workspace notes: {err}")))?
            .into_iter()
            .map(api_workspace_note_from_state)
            .collect();
        Ok(Some(WorkspaceNoteListResponse { notes }.into()))
    }

    pub(crate) async fn note_get(
        &self,
        params: WorkspaceNoteGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let note = state_db
            .workspace()
            .get_note(&params.note_id)
            .await
            .map_err(|err| internal_error(format!("failed to read workspace note: {err}")))?
            .map(api_workspace_note_from_state);
        Ok(Some(WorkspaceNoteGetResponse { note }.into()))
    }

    pub(crate) async fn note_upsert(
        &self,
        params: WorkspaceNoteUpsertParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.client_id.trim().is_empty() {
            return Err(invalid_request("workspace note clientId must not be empty"));
        }
        if params.title.trim().is_empty() {
            return Err(invalid_request("workspace note title must not be empty"));
        }
        let note = state_db
            .workspace()
            .upsert_note(codex_state::WorkspaceNoteUpsert {
                id: empty_to_none(params.id),
                client_id: params.client_id,
                encounter_id: empty_to_none(params.encounter_id),
                title: params.title.trim().to_string(),
                kind: nonempty_or(params.kind, "note"),
                body: params.body,
                status: nonempty_or(params.status, "draft"),
                actor: "human".to_string(),
                source_thread_id: None,
                source_turn_id: None,
                summary: empty_to_none(params.summary),
            })
            .await
            .map_err(|err| invalid_request(format!("failed to save workspace note: {err}")))?;
        Ok(Some(
            WorkspaceNoteUpsertResponse {
                note: api_workspace_note_from_state(note),
            }
            .into(),
        ))
    }

    pub(crate) async fn note_archive(
        &self,
        params: WorkspaceNoteArchiveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let archived = state_db
            .workspace()
            .archive_note(&params.note_id, "human")
            .await
            .map_err(|err| internal_error(format!("failed to archive workspace note: {err}")))?;
        Ok(Some(WorkspaceNoteArchiveResponse { archived }.into()))
    }

    pub(crate) async fn note_sign(
        &self,
        params: WorkspaceNoteSignParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.note_id.trim().is_empty() {
            return Err(invalid_request("workspace note noteId must not be empty"));
        }
        let signer = params.signer.trim();
        if signer.is_empty() {
            return Err(invalid_request("workspace note signer must not be empty"));
        }
        let signature = state_db
            .workspace()
            .sign_note(codex_state::WorkspaceNoteSign {
                note_id: params.note_id,
                signer: signer.to_string(),
            })
            .await
            .map_err(|err| invalid_request(format!("failed to sign workspace note: {err}")))?;
        Ok(Some(
            WorkspaceNoteSignResponse {
                signature: api_workspace_note_signature_from_state(signature),
            }
            .into(),
        ))
    }

    pub(crate) async fn note_signature_list(
        &self,
        params: WorkspaceNoteSignatureListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let signatures = state_db
            .workspace()
            .list_note_signatures(&params.note_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace note signatures: {err}"))
            })?
            .into_iter()
            .map(api_workspace_note_signature_from_state)
            .collect();
        Ok(Some(
            WorkspaceNoteSignatureListResponse { signatures }.into(),
        ))
    }

    pub(crate) async fn note_addendum_create(
        &self,
        params: WorkspaceNoteAddendumCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.body.trim().is_empty() {
            return Err(invalid_request(
                "workspace note addendum body must not be empty",
            ));
        }
        let author = params.author.trim();
        if author.is_empty() {
            return Err(invalid_request(
                "workspace note addendum author must not be empty",
            ));
        }
        let addendum = state_db
            .workspace()
            .create_note_addendum(codex_state::WorkspaceNoteAddendumCreate {
                note_id: params.note_id,
                base_revision: params.base_revision,
                body: params.body,
                author: author.to_string(),
                source_thread_id: empty_to_none(params.source_thread_id),
                source_turn_id: empty_to_none(params.source_turn_id),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to create workspace note addendum: {err}"))
            })?;
        Ok(Some(
            WorkspaceNoteAddendumCreateResponse {
                addendum: api_workspace_note_addendum_from_state(addendum),
            }
            .into(),
        ))
    }

    pub(crate) async fn note_addendum_list(
        &self,
        params: WorkspaceNoteAddendumListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let addenda = state_db
            .workspace()
            .list_note_addenda(&params.note_id)
            .await
            .map_err(|err| internal_error(format!("failed to list workspace note addenda: {err}")))?
            .into_iter()
            .map(api_workspace_note_addendum_from_state)
            .collect();
        Ok(Some(WorkspaceNoteAddendumListResponse { addenda }.into()))
    }

    pub(crate) async fn proposal_list(
        &self,
        params: WorkspaceNoteProposalListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let proposals = state_db
            .workspace()
            .list_note_proposals(&params.note_id)
            .await
            .map_err(|err| {
                internal_error(format!("failed to list workspace note proposals: {err}"))
            })?
            .into_iter()
            .map(api_workspace_note_proposal_from_state)
            .collect();
        Ok(Some(WorkspaceNoteProposalListResponse { proposals }.into()))
    }

    pub(crate) async fn proposal_create(
        &self,
        params: WorkspaceNoteProposalCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        if params.proposed_body.trim().is_empty() {
            return Err(invalid_request(
                "workspace note proposal body must not be empty",
            ));
        }
        let input = codex_state::WorkspaceNoteProposalCreate {
            note_id: params.note_id,
            base_revision: params.base_revision,
            agent_result_id: empty_to_none(params.agent_result_id),
            proposed_body: params.proposed_body,
            summary: params.summary,
            source_thread_id: empty_to_none(params.source_thread_id),
            source_turn_id: empty_to_none(params.source_turn_id),
        };
        let linked_result = input.agent_result_id.is_some();
        let proposal = if linked_result {
            state_db
                .workspace()
                .create_note_proposal_from_agent_result(input)
                .await
        } else {
            state_db.workspace().create_note_proposal(input).await
        }
        .map_err(|err| {
            invalid_request(format!("failed to create workspace note proposal: {err}"))
        })?;
        Ok(Some(
            WorkspaceNoteProposalCreateResponse {
                proposal: api_workspace_note_proposal_from_state(proposal),
            }
            .into(),
        ))
    }

    pub(crate) async fn proposal_resolve(
        &self,
        params: WorkspaceNoteProposalResolveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let edited_body = empty_to_none(params.edited_body);
        if !params.accept && edited_body.is_some() {
            return Err(invalid_request(
                "workspace note proposal editedBody is only valid when accept is true",
            ));
        }
        let resolution = match (params.accept, edited_body) {
            (true, Some(body)) => {
                codex_state::WorkspaceNoteProposalResolution::AcceptEdited { body }
            }
            (true, None) => codex_state::WorkspaceNoteProposalResolution::Accept,
            (false, None) => codex_state::WorkspaceNoteProposalResolution::Decline,
            (false, Some(_)) => unreachable!("edited decline was rejected above"),
        };
        let proposal = state_db
            .workspace()
            .resolve_note_proposal_with(codex_state::WorkspaceNoteProposalResolve {
                proposal_id: params.proposal_id,
                resolution,
                actor: "local clinician".to_string(),
                reason: String::new(),
            })
            .await
            .map_err(|err| {
                invalid_request(format!("failed to resolve workspace note proposal: {err}"))
            })?
            .map(api_workspace_note_proposal_from_state);
        Ok(Some(
            WorkspaceNoteProposalResolveResponse { proposal }.into(),
        ))
    }

    pub(crate) async fn proposal_decision_list(
        &self,
        params: WorkspaceNoteProposalDecisionListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        if params.proposal_id.trim().is_empty() {
            return Err(invalid_request(
                "workspace note proposal decision proposalId must not be empty",
            ));
        }
        let decisions = self
            .state_db()?
            .workspace()
            .list_note_proposal_decisions(&params.proposal_id)
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to list workspace note proposal decisions: {err}"
                ))
            })?
            .into_iter()
            .map(api_workspace_note_proposal_decision_from_state)
            .collect();
        Ok(Some(
            WorkspaceNoteProposalDecisionListResponse { decisions }.into(),
        ))
    }

    pub(crate) async fn audit_list(
        &self,
        params: WorkspaceAuditListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let cursor_created_at_ms = params
            .cursor
            .as_deref()
            .map(str::parse::<i64>)
            .transpose()
            .map_err(|err| invalid_request(format!("invalid workspace audit cursor: {err}")))?;
        let events = state_db
            .workspace()
            .list_audit_events_filtered(codex_state::WorkspaceAuditEventFilter {
                entity_type: empty_to_none(params.entity_type),
                entity_id: empty_to_none(params.entity_id),
                client_id: empty_to_none(params.client_id),
                note_id: empty_to_none(params.note_id),
                cursor_created_at_ms,
                limit: params.limit,
            })
            .await
            .map_err(|err| internal_error(format!("failed to list workspace audit: {err}")))?;
        let next_cursor = events
            .last()
            .filter(|_| {
                params
                    .limit
                    .is_some_and(|limit| events.len() >= limit as usize)
            })
            .map(|event| event.created_at.timestamp_millis().to_string());
        let data = events
            .into_iter()
            .map(api_workspace_audit_event_from_state)
            .collect();
        Ok(Some(
            WorkspaceAuditListResponse { data, next_cursor }.into(),
        ))
    }

    fn state_db(&self) -> Result<StateDbHandle, JSONRPCErrorError> {
        self.state_db
            .clone()
            .ok_or_else(|| invalid_request("workspace store is unavailable"))
    }
}

fn api_workspace_client_from_state(
    value: codex_state::WorkspaceClient,
) -> Result<codex_app_server_protocol::WorkspaceClient, JSONRPCErrorError> {
    let version = value
        .record_version()
        .map_err(|err| internal_error(format!("failed to version workspace client: {err}")))?;
    Ok(codex_app_server_protocol::WorkspaceClient {
        id: value.id,
        version,
        display_name: value.display_name,
        preferred_name: value.preferred_name,
        date_of_birth: value.date_of_birth,
        sex_or_gender: value.sex_or_gender,
        external_id: value.external_id,
        record_start_date: value.record_start_date,
        record_end_date: value.record_end_date,
        summary: value.summary,
        primary_phone: value.primary_phone,
        secondary_phone: value.secondary_phone,
        email: value.email,
        preferred_contact_method: value.preferred_contact_method,
        emergency_contact_name: value.emergency_contact_name,
        emergency_contact_relationship: value.emergency_contact_relationship,
        emergency_contact_phone: value.emergency_contact_phone,
        emergency_contact_email: value.emergency_contact_email,
        contact_notes: value.contact_notes,
        payer_name: value.payer_name,
        plan_name: value.plan_name,
        member_id: value.member_id,
        group_number: value.group_number,
        coverage_type: value.coverage_type,
        coverage_status: value.coverage_status,
        coverage_notes: value.coverage_notes,
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_document_from_state(
    value: codex_state::WorkspaceDocument,
) -> Result<codex_app_server_protocol::WorkspaceDocument, JSONRPCErrorError> {
    let version = value
        .record_version()
        .map_err(|err| internal_error(format!("failed to version workspace document: {err}")))?;
    Ok(codex_app_server_protocol::WorkspaceDocument {
        id: value.id,
        version,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        title: value.title,
        kind: value.kind,
        local_path: value.local_path,
        notes: value.notes,
        scope: value.scope,
        detected_kind: value.detected_kind,
        mime_type: value.mime_type,
        file_size_bytes: value.file_size_bytes,
        modified_at: value.modified_at.map(|value| value.timestamp()),
        sha256: value.sha256,
        tags: value.tags,
        source_label: value.source_label,
        existence_status: value.existence_status,
        metadata_json: value.metadata_json,
        original_path: value.original_path,
        reference_kind: value.reference_kind,
        vault_path: value.vault_path,
        content_sha256: value.content_sha256,
        thumbnail_path: value.thumbnail_path,
        thumbnail_status: value.thumbnail_status,
        thumbnail_mime_type: value.thumbnail_mime_type,
        intake_source: value.intake_source,
        imported_at: value.imported_at.map(|value| value.timestamp()),
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_patient_safety_item_from_state(
    value: codex_state::WorkspacePatientSafetyItem,
) -> Result<codex_app_server_protocol::WorkspacePatientSafetyItem, JSONRPCErrorError> {
    let version = value.record_version().map_err(|err| {
        internal_error(format!(
            "failed to version workspace patient safety item: {err}"
        ))
    })?;
    Ok(codex_app_server_protocol::WorkspacePatientSafetyItem {
        id: value.id,
        version,
        client_id: value.client_id,
        category: value.category,
        name: value.name,
        reaction: value.reaction,
        severity: value.severity,
        dose: value.dose,
        route: value.route,
        frequency: value.frequency,
        status: value.status,
        recorded_date: value.recorded_date,
        notes: value.notes,
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_practice_library_item_from_state(
    value: codex_state::WorkspacePracticeLibraryItem,
) -> Result<WorkspacePracticeLibraryItem, JSONRPCErrorError> {
    Ok(WorkspacePracticeLibraryItem {
        document: api_workspace_document_from_state(value.document)?,
        owner_client_id: value.owner_client_id,
        owner_display_name: value.owner_display_name,
        linked_to_active_client: value.linked_to_active_client,
        linked_document_id: value.linked_document_id,
        scope_reason: value.scope_reason,
        reviewed_text_count: value.reviewed_text_count,
        clip_count: value.clip_count,
    })
}

fn api_workspace_artifact_derivative_from_state(
    value: codex_state::WorkspaceArtifactDerivative,
) -> Result<codex_app_server_protocol::WorkspaceArtifactDerivative, JSONRPCErrorError> {
    let version = value.record_version().map_err(|err| {
        internal_error(format!(
            "failed to version workspace artifact derivative: {err}"
        ))
    })?;
    Ok(codex_app_server_protocol::WorkspaceArtifactDerivative {
        id: value.id,
        version,
        document_id: value.document_id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        kind: value.kind,
        title: value.title,
        body: value.body,
        review_status: value.review_status,
        source_method: value.source_method,
        page_range: value.page_range,
        timestamp_range: value.timestamp_range,
        segment_label: value.segment_label,
        tags: value.tags,
        metadata_json: value.metadata_json,
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_context_clip_from_state(
    value: codex_state::WorkspaceContextClip,
) -> Result<codex_app_server_protocol::WorkspaceContextClip, JSONRPCErrorError> {
    let version = value.record_version().map_err(|err| {
        internal_error(format!("failed to version workspace context clip: {err}"))
    })?;
    Ok(codex_app_server_protocol::WorkspaceContextClip {
        id: value.id,
        version,
        derivative_id: value.derivative_id,
        document_id: value.document_id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        kind: value.kind,
        title: value.title,
        body: value.body,
        review_status: value.review_status,
        source_method: value.source_method,
        page_range: value.page_range,
        timestamp_range: value.timestamp_range,
        line_range: value.line_range,
        segment_label: value.segment_label,
        tags: value.tags,
        metadata_json: value.metadata_json,
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_task_from_state(
    value: codex_state::WorkspaceTask,
) -> Result<codex_app_server_protocol::WorkspaceTask, JSONRPCErrorError> {
    let version = value
        .record_version()
        .map_err(|err| internal_error(format!("failed to version workspace task: {err}")))?;
    Ok(codex_app_server_protocol::WorkspaceTask {
        id: value.id,
        version,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        document_id: value.document_id,
        title: value.title,
        details: value.details,
        kind: value.kind,
        status: api_workspace_task_status_from_state(value.status),
        priority: api_workspace_task_priority_from_state(value.priority),
        due_date: value.due_date,
        assigned_to: value.assigned_to,
        completed_at: value.completed_at.map(|value| value.timestamp()),
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_task_summary_from_state(
    value: codex_state::WorkspaceTask,
) -> WorkspaceTaskSummary {
    WorkspaceTaskSummary {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        document_id: value.document_id,
        title: value.title,
        kind: value.kind,
        status: api_workspace_task_status_from_state(value.status),
        priority: api_workspace_task_priority_from_state(value.priority),
        due_date: value.due_date,
        assigned_to: value.assigned_to,
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_task_status_from_state(
    value: codex_state::WorkspaceTaskStatus,
) -> codex_app_server_protocol::WorkspaceTaskStatus {
    match value {
        codex_state::WorkspaceTaskStatus::Open => {
            codex_app_server_protocol::WorkspaceTaskStatus::Open
        }
        codex_state::WorkspaceTaskStatus::InProgress => {
            codex_app_server_protocol::WorkspaceTaskStatus::InProgress
        }
        codex_state::WorkspaceTaskStatus::Blocked => {
            codex_app_server_protocol::WorkspaceTaskStatus::Blocked
        }
        codex_state::WorkspaceTaskStatus::Done => {
            codex_app_server_protocol::WorkspaceTaskStatus::Done
        }
        codex_state::WorkspaceTaskStatus::Canceled => {
            codex_app_server_protocol::WorkspaceTaskStatus::Canceled
        }
    }
}

fn state_workspace_task_status_from_api(
    value: codex_app_server_protocol::WorkspaceTaskStatus,
) -> codex_state::WorkspaceTaskStatus {
    match value {
        codex_app_server_protocol::WorkspaceTaskStatus::Open => {
            codex_state::WorkspaceTaskStatus::Open
        }
        codex_app_server_protocol::WorkspaceTaskStatus::InProgress => {
            codex_state::WorkspaceTaskStatus::InProgress
        }
        codex_app_server_protocol::WorkspaceTaskStatus::Blocked => {
            codex_state::WorkspaceTaskStatus::Blocked
        }
        codex_app_server_protocol::WorkspaceTaskStatus::Done => {
            codex_state::WorkspaceTaskStatus::Done
        }
        codex_app_server_protocol::WorkspaceTaskStatus::Canceled => {
            codex_state::WorkspaceTaskStatus::Canceled
        }
    }
}

fn api_workspace_task_priority_from_state(
    value: codex_state::WorkspaceTaskPriority,
) -> codex_app_server_protocol::WorkspaceTaskPriority {
    match value {
        codex_state::WorkspaceTaskPriority::Low => {
            codex_app_server_protocol::WorkspaceTaskPriority::Low
        }
        codex_state::WorkspaceTaskPriority::Normal => {
            codex_app_server_protocol::WorkspaceTaskPriority::Normal
        }
        codex_state::WorkspaceTaskPriority::High => {
            codex_app_server_protocol::WorkspaceTaskPriority::High
        }
        codex_state::WorkspaceTaskPriority::Urgent => {
            codex_app_server_protocol::WorkspaceTaskPriority::Urgent
        }
    }
}

fn state_workspace_task_priority_from_api(
    value: codex_app_server_protocol::WorkspaceTaskPriority,
) -> codex_state::WorkspaceTaskPriority {
    match value {
        codex_app_server_protocol::WorkspaceTaskPriority::Low => {
            codex_state::WorkspaceTaskPriority::Low
        }
        codex_app_server_protocol::WorkspaceTaskPriority::Normal => {
            codex_state::WorkspaceTaskPriority::Normal
        }
        codex_app_server_protocol::WorkspaceTaskPriority::High => {
            codex_state::WorkspaceTaskPriority::High
        }
        codex_app_server_protocol::WorkspaceTaskPriority::Urgent => {
            codex_state::WorkspaceTaskPriority::Urgent
        }
    }
}

fn api_workspace_encounter_from_state(
    value: codex_state::WorkspaceEncounter,
) -> Result<codex_app_server_protocol::WorkspaceEncounter, JSONRPCErrorError> {
    let version = value
        .record_version()
        .map_err(|err| internal_error(format!("failed to version workspace encounter: {err}")))?;
    Ok(codex_app_server_protocol::WorkspaceEncounter {
        id: value.id,
        version,
        client_id: value.client_id,
        kind: value.kind,
        title: value.title,
        status: value.status,
        started_at: value.started_at.map(|value| value.timestamp()),
        ended_at: value.ended_at.map(|value| value.timestamp()),
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    })
}

fn api_workspace_note_from_state(
    value: codex_state::WorkspaceNote,
) -> codex_app_server_protocol::WorkspaceNote {
    codex_app_server_protocol::WorkspaceNote {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        title: value.title,
        kind: value.kind,
        body: value.body,
        status: value.status,
        current_revision: value.current_revision,
        archived_at: value.archived_at.map(|value| value.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_note_summary_from_state(
    value: &codex_state::WorkspaceNote,
) -> WorkspaceNoteSummary {
    WorkspaceNoteSummary {
        id: value.id.clone(),
        client_id: value.client_id.clone(),
        encounter_id: value.encounter_id.clone(),
        title: value.title.clone(),
        kind: value.kind.clone(),
        status: value.status.clone(),
        current_revision: value.current_revision,
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_note_signature_from_state(
    value: codex_state::WorkspaceNoteSignature,
) -> codex_app_server_protocol::WorkspaceNoteSignature {
    codex_app_server_protocol::WorkspaceNoteSignature {
        id: value.id,
        note_id: value.note_id,
        revision: value.revision,
        signer: value.signer,
        body_sha256: value.body_sha256,
        signed_at: value.signed_at.timestamp(),
    }
}

fn api_workspace_note_addendum_from_state(
    value: codex_state::WorkspaceNoteAddendum,
) -> codex_app_server_protocol::WorkspaceNoteAddendum {
    codex_app_server_protocol::WorkspaceNoteAddendum {
        id: value.id,
        note_id: value.note_id,
        base_revision: value.base_revision,
        body: value.body,
        author: value.author,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        created_at: value.created_at.timestamp(),
    }
}

fn api_workspace_context_packet_from_state(
    value: codex_state::WorkspaceContextPacket,
) -> codex_app_server_protocol::WorkspaceContextPacket {
    codex_app_server_protocol::WorkspaceContextPacket {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        human_request: value.human_request,
        selected_artifact_ids_json: value.selected_artifact_ids_json,
        selected_derivative_ids_json: value.selected_derivative_ids_json,
        selected_clip_ids_json: value.selected_clip_ids_json,
        artifact_summary: value.artifact_summary,
        derivative_summary: value.derivative_summary,
        clip_summary: value.clip_summary,
        chart_context_summary: value.chart_context_summary,
        context_envelope_json: value.context_envelope_json,
        context_envelope_sha256: value.context_envelope_sha256,
        clinician_actor: value.clinician_actor,
        base_note_revision: value.base_note_revision,
        authorized_scope_json: value.authorized_scope_json,
        expected_output_kind: value.expected_output_kind,
        status: value.status,
        created_at: value.created_at.timestamp(),
        sent_at: value.sent_at.timestamp(),
        submitted_at: value
            .submitted_at
            .map(|submitted_at| submitted_at.timestamp()),
        canceled_at: value.canceled_at.map(|canceled_at| canceled_at.timestamp()),
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_context_packet_replay_from_state(
    value: codex_state::WorkspaceContextPacket,
) -> WorkspaceContextPacketReplay {
    WorkspaceContextPacketReplay {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        human_request: value.human_request,
        context_envelope_json: value.context_envelope_json,
        context_envelope_sha256: value.context_envelope_sha256,
        clinician_actor: value.clinician_actor,
        base_note_revision: value.base_note_revision,
        authorized_scope_json: value.authorized_scope_json,
        expected_output_kind: value.expected_output_kind,
        read_only_safety_constraints: AGENT_VISIBLE_PACKET_SAFETY_CONSTRAINTS
            .iter()
            .map(|line| (*line).to_string())
            .collect(),
        status: value.status,
        sent_at: value.sent_at.timestamp(),
        submitted_at: value
            .submitted_at
            .map(|submitted_at| submitted_at.timestamp()),
    }
}

fn api_workspace_agent_run_from_state(
    value: codex_state::WorkspaceAgentRun,
) -> codex_app_server_protocol::WorkspaceAgentRun {
    let provider = (!value.provider.trim().is_empty()).then_some(value.provider);
    let model = (!value.model.trim().is_empty()).then_some(value.model);
    let error_summary = (!value.error_summary.trim().is_empty()).then_some(value.error_summary);
    codex_app_server_protocol::WorkspaceAgentRun {
        id: value.id,
        packet_id: value.packet_id,
        client_id: value.client_id,
        note_id: value.note_id,
        base_note_revision: value.base_note_revision,
        context_envelope_sha256: value.context_envelope_sha256,
        run_kind: value.run_kind,
        idempotency_key: value.idempotency_key,
        provider,
        model,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        status: value.status,
        error_summary,
        started_at: value.started_at.timestamp(),
        completed_at: value
            .completed_at
            .map(|completed_at| completed_at.timestamp()),
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_agent_run_source_from_state(
    value: codex_state::WorkspaceAgentRunSource,
) -> codex_app_server_protocol::WorkspaceAgentRunSource {
    codex_app_server_protocol::WorkspaceAgentRunSource {
        id: value.id,
        run_id: value.run_id,
        source_entity_type: value.source_entity_type,
        source_entity_id: value.source_entity_id,
        source_revision: value.source_revision,
        display_label: value.display_label,
        snapshot_json: value.snapshot_json,
        content_sha256: value.content_sha256,
        access_purpose: value.access_purpose,
        accessed_at: value.accessed_at.timestamp(),
    }
}

fn api_workspace_agent_result_from_state(
    value: codex_state::WorkspaceAgentResult,
) -> codex_app_server_protocol::WorkspaceAgentResult {
    codex_app_server_protocol::WorkspaceAgentResult {
        id: value.id,
        run_id: value.run_id,
        packet_id: value.packet_id,
        client_id: value.client_id,
        note_id: value.note_id,
        context_envelope_sha256: value.context_envelope_sha256,
        base_note_revision: value.base_note_revision,
        packet_context_sha256: value.packet_context_sha256,
        result_kind: value.result_kind,
        structured_changes_json: value.structured_changes_json,
        rationale_summary: value.rationale_summary,
        body: value.body,
        summary: value.summary,
        status: value.status,
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
    }
}

fn api_workspace_note_proposal_from_state(
    value: codex_state::WorkspaceNoteProposal,
) -> codex_app_server_protocol::WorkspaceNoteProposal {
    codex_app_server_protocol::WorkspaceNoteProposal {
        id: value.id,
        note_id: value.note_id,
        base_revision: value.base_revision,
        proposed_body: value.proposed_body,
        summary: value.summary,
        status: match value.status {
            codex_state::WorkspaceNoteProposalStatus::Pending => {
                codex_app_server_protocol::WorkspaceNoteProposalStatus::Pending
            }
            codex_state::WorkspaceNoteProposalStatus::Accepted => {
                codex_app_server_protocol::WorkspaceNoteProposalStatus::Accepted
            }
            codex_state::WorkspaceNoteProposalStatus::Declined => {
                codex_app_server_protocol::WorkspaceNoteProposalStatus::Declined
            }
        },
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        agent_result_id: value.agent_result_id,
        created_at: value.created_at.timestamp(),
        resolved_at: value.resolved_at.map(|value| value.timestamp()),
    }
}

fn api_workspace_note_proposal_decision_from_state(
    value: codex_state::WorkspaceNoteProposalDecision,
) -> codex_app_server_protocol::WorkspaceNoteProposalDecision {
    let decision_kind = match value.decision_kind {
        codex_state::WorkspaceNoteProposalDecisionKind::AcceptedAll => {
            WorkspaceNoteProposalDecisionKind::AcceptedAll
        }
        codex_state::WorkspaceNoteProposalDecisionKind::AcceptedEdited => {
            WorkspaceNoteProposalDecisionKind::AcceptedEdited
        }
        codex_state::WorkspaceNoteProposalDecisionKind::RejectedAll => {
            WorkspaceNoteProposalDecisionKind::RejectedAll
        }
        codex_state::WorkspaceNoteProposalDecisionKind::CopiedChange => {
            WorkspaceNoteProposalDecisionKind::CopiedChange
        }
        codex_state::WorkspaceNoteProposalDecisionKind::RejectedChange => {
            WorkspaceNoteProposalDecisionKind::RejectedChange
        }
    };
    codex_app_server_protocol::WorkspaceNoteProposalDecision {
        id: value.id,
        proposal_id: value.proposal_id,
        agent_result_id: value.agent_result_id,
        note_id: value.note_id,
        base_revision: value.base_revision,
        decision_kind,
        change_id: value.change_id,
        applied_text: value.applied_text,
        applied_text_sha256: value.applied_text_sha256,
        resulting_note_revision: value.resulting_note_revision,
        actor: value.actor,
        reason: (!value.reason.trim().is_empty()).then_some(value.reason),
        created_at: value.created_at.timestamp(),
    }
}

fn api_workspace_audit_event_from_state(
    value: codex_state::WorkspaceAuditEvent,
) -> codex_app_server_protocol::WorkspaceAuditEvent {
    codex_app_server_protocol::WorkspaceAuditEvent {
        id: value.id,
        entity_type: value.entity_type,
        entity_id: value.entity_id,
        action: value.action,
        actor: value.actor,
        actor_kind: value.actor_kind,
        source: value.source,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        document_id: value.document_id,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        success: value.success,
        summary: value.summary,
        metadata_json: value.metadata_json,
        created_at: value.created_at.timestamp(),
    }
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn workspace_patient_safety_category(value: String) -> Result<String, JSONRPCErrorError> {
    let normalized = match value.trim().to_ascii_lowercase().as_str() {
        "allergy" | "allergies" => "allergy",
        "medication" | "medications" | "med" | "meds" => "medication",
        "condition" | "conditions" | "problem" | "problems" => "condition",
        "precaution" | "precautions" | "restriction" | "restrictions" => "precaution",
        other => {
            return Err(invalid_request(format!(
                "workspace patient safety category `{other}` is unsupported"
            )));
        }
    };
    Ok(normalized.to_string())
}

fn unix_seconds_to_datetime(value: i64) -> Result<DateTime<Utc>, JSONRPCErrorError> {
    DateTime::<Utc>::from_timestamp(value, 0)
        .ok_or_else(|| invalid_request(format!("invalid workspace timestamp `{value}`")))
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut output = String::new();
    for ch in collapsed.chars().take(max_chars) {
        output.push(ch);
    }
    if collapsed.chars().count() > max_chars {
        output.push_str("...");
    }
    output
}

fn nonempty_or(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
