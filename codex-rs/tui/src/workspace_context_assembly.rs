use codex_app_server_protocol::WorkspaceArtifactDerivative;
use codex_app_server_protocol::WorkspaceContextClip;
use codex_app_server_protocol::WorkspaceContextPacket;
use codex_app_server_protocol::WorkspaceDocument;
use codex_app_server_protocol::WorkspaceNoteSignature;
use codex_app_server_protocol::WorkspaceTask;
use codex_app_server_protocol::WorkspaceTaskPriority;
use codex_app_server_protocol::WorkspaceTaskStatus;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;

pub(crate) const MEDICAL_CONTEXT_ASSEMBLY_VERSION: &str = "medical-context-packet-v1";

const PACKET_SAFETY_LINES: &[&str] = &[
    "read-only context packet; do not mutate workspace records",
    "original local files are not uploaded, parsed, transcribed, OCRed, or analyzed automatically",
    "file entries are metadata unless an explicit derivative or clip is selected",
    "generated transcripts, OCR, EDI parsing, summaries, and video observations require human review before clinical or billing use",
    "do not sign notes, submit claims, send payer communications, or overwrite saved data",
];

pub(crate) struct MedicalContextDraftDocument<'a> {
    pub(crate) kind: &'a str,
    pub(crate) title: &'a str,
    pub(crate) local_path: &'a str,
    pub(crate) notes: &'a str,
}

pub(crate) struct MedicalContextDraftTask<'a> {
    pub(crate) title: &'a str,
    pub(crate) details: &'a str,
}

pub(crate) struct MedicalContextAssemblyInput<'a> {
    pub(crate) client_id: &'a str,
    pub(crate) encounter_id: &'a str,
    pub(crate) note_id: &'a str,
    pub(crate) source_mode: &'a str,
    pub(crate) unsaved_draft_included: bool,
    pub(crate) patient_display_name: &'a str,
    pub(crate) patient_preferred_name: &'a str,
    pub(crate) patient_date_of_birth: &'a str,
    pub(crate) patient_sex_or_gender: &'a str,
    pub(crate) patient_external_id: &'a str,
    pub(crate) patient_record_start_date: &'a str,
    pub(crate) patient_record_end_date: &'a str,
    pub(crate) patient_summary: &'a str,
    pub(crate) agent_request_body: &'a str,
    pub(crate) note_title: &'a str,
    pub(crate) note_status: &'a str,
    pub(crate) note_revision: i64,
    pub(crate) note_body: &'a str,
    pub(crate) note_locked: bool,
    pub(crate) signatures: &'a [WorkspaceNoteSignature],
    pub(crate) addenda_count: usize,
    pub(crate) pending_proposal_count: usize,
    pub(crate) selected_artifacts: Vec<&'a WorkspaceDocument>,
    pub(crate) total_artifact_count: usize,
    pub(crate) draft_document: Option<MedicalContextDraftDocument<'a>>,
    pub(crate) selected_derivatives: Vec<&'a WorkspaceArtifactDerivative>,
    pub(crate) total_derivative_count: usize,
    pub(crate) selected_clips: Vec<&'a WorkspaceContextClip>,
    pub(crate) total_clip_count: usize,
    pub(crate) active_tasks: Vec<&'a WorkspaceTask>,
    pub(crate) draft_task: Option<MedicalContextDraftTask<'a>>,
}

#[derive(Debug, Clone)]
pub(crate) struct MedicalContextAssembly {
    pub(crate) prompt: String,
    pub(crate) preview_lines: Vec<String>,
    pub(crate) human_request: String,
    pub(crate) selected_artifact_ids_json: String,
    pub(crate) selected_derivative_ids_json: String,
    pub(crate) selected_clip_ids_json: String,
    pub(crate) artifact_summary: String,
    pub(crate) derivative_summary: String,
    pub(crate) clip_summary: String,
    pub(crate) chart_context_summary: String,
    pub(crate) context_envelope_json: String,
}

pub(crate) fn assemble_medical_context(
    input: MedicalContextAssemblyInput<'_>,
) -> MedicalContextAssembly {
    let human_request = if input.agent_request_body.trim().is_empty() {
        "General Medical Agent Plan request".to_string()
    } else {
        input.agent_request_body.trim().to_string()
    };
    let selected_artifact_ids_json = selected_ids_json(
        input
            .selected_artifacts
            .iter()
            .map(|document| document.id.as_str()),
    );
    let selected_derivative_ids_json = selected_ids_json(
        input
            .selected_derivatives
            .iter()
            .map(|derivative| derivative.id.as_str()),
    );
    let selected_clip_ids_json =
        selected_ids_json(input.selected_clips.iter().map(|clip| clip.id.as_str()));
    let artifact_summary = selected_artifact_trace_summary(&input.selected_artifacts);
    let derivative_summary = selected_derivative_trace_summary(&input.selected_derivatives);
    let clip_summary = selected_clip_trace_summary(&input.selected_clips);
    let chart_context_summary = format!(
        "patient {}; note {} [{} r{}]; jobs {}; proposals {}",
        input.patient_display_name,
        input.note_title,
        input.note_status,
        input.note_revision,
        input.active_tasks.len(),
        input.pending_proposal_count
    );
    let preview_lines = context_packet_preview_lines(
        &input,
        &artifact_summary,
        &derivative_summary,
        &clip_summary,
    );
    let prompt = medical_agent_context_prompt(&input);
    let context_envelope_json = context_envelope_json(
        &input,
        &prompt,
        &preview_lines,
        &artifact_summary,
        &derivative_summary,
        &clip_summary,
        &chart_context_summary,
    );

    MedicalContextAssembly {
        prompt,
        preview_lines,
        human_request,
        selected_artifact_ids_json,
        selected_derivative_ids_json,
        selected_clip_ids_json,
        artifact_summary,
        derivative_summary,
        clip_summary,
        chart_context_summary,
        context_envelope_json,
    }
}

fn context_packet_preview_lines(
    input: &MedicalContextAssemblyInput<'_>,
    artifact_summary: &str,
    derivative_summary: &str,
    clip_summary: &str,
) -> Vec<String> {
    let patient_files = input
        .selected_artifacts
        .iter()
        .filter(|document| artifact_scope_label(document) == "patient")
        .count();
    let practice_files = input.selected_artifacts.len().saturating_sub(patient_files);
    let excluded_files = input
        .total_artifact_count
        .saturating_sub(input.selected_artifacts.len());
    let excluded_derivatives = input
        .total_derivative_count
        .saturating_sub(input.selected_derivatives.len());
    let excluded_clips = input
        .total_clip_count
        .saturating_sub(input.selected_clips.len());
    let request = if input.agent_request_body.trim().is_empty() {
        "No agent instructions yet".to_string()
    } else {
        compact_preview(input.agent_request_body, 72)
    };
    let mut lines = vec![
        format!(
            "patient: {}; note: {} [{} r{}]",
            input.patient_display_name, input.note_title, input.note_status, input.note_revision
        ),
        format!("instructions: {request}"),
        "plan: Ctrl-G opens agent; inspect before submitting".to_string(),
        "Agent sees: this patient/note, request, selected files/text/clips/jobs".to_string(),
        "Agent cannot: read unselected records, fetch files, mutate/sign/submit/contact"
            .to_string(),
        "return: /workspacemedical; paste reviewed answer with :agent result".to_string(),
        format!(
            "agent context: files {} in/{excluded_files} out ({patient_files} patient/{practice_files} practice); text {} in/{excluded_derivatives} out; clips {} in/{excluded_clips} out; jobs {}; proposals {}",
            input.selected_artifacts.len(),
            input.selected_derivatives.len(),
            input.selected_clips.len(),
            input.active_tasks.len(),
            input.pending_proposal_count
        ),
        "boundary: selected-only, read-only Medical Agent Plan; human review before chart action"
            .to_string(),
        "no file copy/upload/parse/OCR/transcribe/analyze".to_string(),
        format!(
            "agent file references: {}",
            provider_file_reference_summary(artifact_summary)
        ),
        format!(
            "agent reviewed text: {}",
            provider_reviewed_text_summary(derivative_summary)
        ),
        format!("agent clips: {clip_summary}"),
    ];
    if input.selected_artifacts.is_empty() {
        lines.push(
            "included file references: none; saved file metadata excluded by default".to_string(),
        );
    } else {
        for document in input.selected_artifacts.iter().take(3) {
            lines.push(format!(
                "include metadata-only: {} [{}; {}]",
                document_context_label(document),
                artifact_presence_label(document),
                artifact_size_label(document)
            ));
        }
        let extra = input.selected_artifacts.len().saturating_sub(3);
        if extra > 0 {
            lines.push(format!(
                "include metadata-only: +{extra} more file reference(s)"
            ));
        }
    }
    if input.selected_derivatives.is_empty() {
        lines.push(
            "included reviewed text: none; saved reviewed text excluded by default".to_string(),
        );
    } else {
        for derivative in input.selected_derivatives.iter().take(3) {
            lines.push(format!(
                "include reviewed text: {} [{}; {}]",
                derivative_context_label(derivative),
                derivative_status_label(&derivative.review_status),
                compact_preview(&derivative.body, 56)
            ));
        }
        let extra = input.selected_derivatives.len().saturating_sub(3);
        if extra > 0 {
            lines.push(format!("include reviewed text: +{extra} more item(s)"));
        }
    }
    if input.selected_clips.is_empty() {
        lines.push("included clips: none; saved clips excluded by default".to_string());
    } else {
        for clip in input.selected_clips.iter().take(3) {
            lines.push(format!(
                "include clip excerpt: {} [{}; {}; {}]",
                clip_context_label(clip),
                derivative_status_label(&clip.review_status),
                clip_range_label(clip),
                compact_preview(&clip.body, 56)
            ));
        }
        let extra = input.selected_clips.len().saturating_sub(3);
        if extra > 0 {
            lines.push(format!("include clip excerpt: +{extra} more clip(s)"));
        }
    }
    let edi_labels = input
        .selected_artifacts
        .iter()
        .copied()
        .filter_map(edi_transaction_label)
        .take(4)
        .collect::<Vec<_>>();
    if !edi_labels.is_empty() {
        lines.push(format!("EDI detected: {}", edi_labels.join(", ")));
    }
    let selected_missing = input
        .selected_artifacts
        .iter()
        .filter(|document| artifact_presence_label(document) != "present")
        .count();
    if selected_missing > 0 {
        lines.push(format!(
            "attention: {selected_missing} selected local reference(s) missing/inaccessible"
        ));
    }
    lines
}

pub(crate) fn provider_file_reference_summary(summary: &str) -> String {
    summary
        .replace("selected artifact(s)", "selected file reference(s)")
        .replace("selected artifacts", "selected file references")
}

pub(crate) fn provider_reviewed_text_summary(summary: &str) -> String {
    summary
        .replace("selected derivative(s)", "selected reviewed text item(s)")
        .replace("selected derivatives", "selected reviewed text items")
}

fn medical_agent_context_prompt(input: &MedicalContextAssemblyInput<'_>) -> String {
    let mut prompt = String::new();
    prompt.push_str("Medical workspace context selected.\n\n");
    prompt.push_str("Saved workspace IDs:\n");
    prompt.push_str("- client_id: ");
    prompt.push_str(input.client_id);
    prompt.push('\n');
    prompt.push_str("- encounter_id: ");
    prompt.push_str(input.encounter_id);
    prompt.push('\n');
    prompt.push_str("- note_id: ");
    prompt.push_str(input.note_id);
    prompt.push('\n');
    prompt.push_str("- include_documents: false\n");
    prompt.push_str(
        "- selected artifact metadata is listed inline; do not fetch unselected documents\n\n",
    );
    prompt.push_str("Packet-scoped context read:\n");
    prompt.push_str("- backend endpoint: workspace/context/packet/replay\n");
    prompt.push_str(
        "- app attaches packet_id and context_envelope_sha256 after the packet is persisted\n",
    );
    prompt.push_str(
        "- workspace/context/get is broad human dashboard context and is not agent-visible\n",
    );
    prompt.push_str("- do not list or read unrelated workspace records to expand this packet\n\n");
    if input.unsaved_draft_included {
        prompt.push_str("Unsaved draft content is included below because it may not be persisted in the workspace database yet.\n\n");
    }
    prompt.push_str("Active patient: ");
    prompt.push_str(input.client_id);
    prompt.push('\n');
    prompt.push_str("Active encounter: ");
    prompt.push_str(input.encounter_id);
    prompt.push('\n');
    prompt.push_str("Active note: ");
    prompt.push_str(input.note_id);
    prompt.push_str("\n\n");
    prompt.push_str("Patient snapshot\n");
    push_context_line(&mut prompt, "Display name", input.patient_display_name);
    push_context_line(&mut prompt, "Preferred name", input.patient_preferred_name);
    push_context_line(&mut prompt, "Date of birth", input.patient_date_of_birth);
    push_context_line(&mut prompt, "Sex or gender", input.patient_sex_or_gender);
    push_context_line(&mut prompt, "Key identifier", input.patient_external_id);
    push_context_line(&mut prompt, "Chart start", input.patient_record_start_date);
    push_context_line(&mut prompt, "Chart end", input.patient_record_end_date);
    push_context_line(&mut prompt, "Summary", input.patient_summary);
    prompt.push('\n');
    prompt.push_str("Human agent request\n");
    if input.agent_request_body.trim().is_empty() {
        prompt.push_str("No typed agent request. Use the selected workspace context to suggest reviewable next steps only if appropriate.\n\n");
    } else {
        prompt.push_str(input.agent_request_body.trim());
        prompt.push_str("\n\n");
    }
    prompt.push_str("Context packet safety\n");
    prompt.push_str("- read-only context packet; do not mutate workspace records\n");
    prompt.push_str("- original local files are not uploaded, parsed, transcribed, OCRed, or analyzed automatically\n");
    prompt.push_str(
        "- file entries below are metadata unless an explicit derivative or clip is listed\n",
    );
    prompt.push_str("- generated transcripts, OCR, EDI parsing, summaries, and video observations require human review before clinical or billing use\n");
    prompt.push_str("- do not sign notes, submit claims, send payer communications, or overwrite saved data\n\n");

    push_selected_artifacts(&mut prompt, input);
    push_selected_derivatives(&mut prompt, input);
    push_selected_clips(&mut prompt, input);
    push_active_jobs(&mut prompt, input);
    push_note_context(&mut prompt, input);
    prompt.push_str("\n\nUse only this selected packet context. Do not infer access to the rest of the workspace, and do not fetch unselected artifact, derivative, clip, patient, practice, or task records. If more context is needed, ask the human to build and send another packet. Review the selected patient, active note, chart dates, encounter, signature state, selected artifact metadata, selected derivative text, selected clip excerpts, human request, and active jobs above. If content above is marked unsaved draft, rely on the inline draft text. Return reviewable proposals only: note edits or replacement drafts for unsigned notes, addenda for signed notes, task/document follow-ups, billing/file findings, or questions for the human. Do not overwrite saved workspace data, sign notes, submit claims, contact payers, or mutate records.");
    prompt
}

pub(crate) fn packet_scoped_agent_handoff_prompt(packet: &WorkspaceContextPacket) -> String {
    let envelope = serde_json::from_str::<serde_json::Value>(&packet.context_envelope_json).ok();
    let prompt_snapshot = envelope
        .as_ref()
        .and_then(|envelope| {
            envelope
                .pointer("/promptSnapshot")
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_string);
    let patient_label = envelope_string(envelope.as_ref(), "/patient/displayName")
        .unwrap_or(packet.client_id.as_str())
        .to_string();
    let note_label = envelope_note_label(envelope.as_ref(), packet);
    let request_label = envelope_string(envelope.as_ref(), "/humanRequest")
        .or_else(|| nonempty(packet.human_request.as_str()))
        .unwrap_or("General Medical Agent Plan request")
        .to_string();
    let chart_summary = envelope_string(envelope.as_ref(), "/summaries/chartContextSummary")
        .or_else(|| nonempty(packet.chart_context_summary.as_str()))
        .map(str::to_string);
    let mut prompt = String::new();
    prompt.push_str("Medical workspace context packet selected.\n\n");
    prompt.push_str("Agent-visible packet handle:\n");
    prompt.push_str("- backend endpoint: workspace/context/packet/replay\n");
    prompt.push_str("- client_id: ");
    prompt.push_str(&packet.client_id);
    prompt.push('\n');
    prompt.push_str("- packet_id: ");
    prompt.push_str(&packet.id);
    prompt.push('\n');
    prompt.push_str("- context_envelope_sha256: ");
    prompt.push_str(&packet.context_envelope_sha256);
    prompt.push('\n');
    if let Some(encounter_id) = packet.encounter_id.as_deref() {
        prompt.push_str("- encounter_id: ");
        prompt.push_str(encounter_id);
        prompt.push('\n');
    }
    if let Some(note_id) = packet.note_id.as_deref() {
        prompt.push_str("- note_id: ");
        prompt.push_str(note_id);
        prompt.push('\n');
    }
    prompt.push_str("- include_documents: false\n\n");
    prompt.push_str("Round-trip origin:\n");
    prompt.push_str("- patient: ");
    prompt.push_str(&patient_label);
    prompt.push('\n');
    prompt.push_str("- note: ");
    prompt.push_str(&note_label);
    prompt.push('\n');
    prompt.push_str("- request: ");
    prompt.push_str(&compact_preview(&request_label, 140));
    prompt.push('\n');
    if let Some(chart_summary) = chart_summary.as_deref() {
        prompt.push_str("- chart summary: ");
        prompt.push_str(&compact_preview(chart_summary, 140));
        prompt.push('\n');
    }
    prompt.push_str("- return: reopen /workspacemedical, review saved returned work with :agent result, and keep chart changes human-approved\n");
    prompt.push_str("- do not submit this composer prompt until the packet id/hash and scope match the intended chart\n\n");
    prompt.push_str("Packet access boundary:\n");
    prompt.push_str("- use only this packet context and the stored envelope below\n");
    prompt.push_str("- do not infer access to the rest of the workspace\n");
    prompt.push_str("- do not call workspace/context/get, list documents, or read unselected artifacts, derivatives, clips, patient records, practice records, or tasks to expand this packet\n");
    prompt.push_str("- current source rows may have changed; the stored envelope is the authoritative sent snapshot\n");
    prompt.push_str("- original local files are not uploaded, parsed, transcribed, OCRed, or analyzed automatically\n");
    prompt.push_str(
        "- do not write to chart, sign notes, submit claims, contact payers, or mutate records\n",
    );
    prompt.push_str(
        "- returning work to the chart requires explicit human save/review in /workspacemedical\n",
    );
    prompt.push_str(
        "- if more context is needed, ask the human to build and send another packet\n\n",
    );
    prompt.push_str("Stored packet envelope JSON (authoritative sent snapshot):\n");
    prompt.push_str(&packet.context_envelope_json);
    if let Some(prompt_snapshot) = prompt_snapshot {
        prompt.push_str("\n\nRendered packet context snapshot:\n");
        prompt.push_str(&prompt_snapshot);
    }
    prompt
}

fn envelope_string<'a>(envelope: Option<&'a serde_json::Value>, pointer: &str) -> Option<&'a str> {
    envelope
        .and_then(|envelope| envelope.pointer(pointer))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn envelope_note_label(
    envelope: Option<&serde_json::Value>,
    packet: &WorkspaceContextPacket,
) -> String {
    let title = envelope_string(envelope, "/note/title")
        .or(packet.note_id.as_deref())
        .unwrap_or("unknown note");
    let status = envelope_string(envelope, "/note/status");
    let revision = envelope
        .and_then(|envelope| envelope.pointer("/note/revision"))
        .and_then(serde_json::Value::as_i64);

    match (status, revision) {
        (Some(status), Some(revision)) => format!("{title} [{status} r{revision}]"),
        (Some(status), None) => format!("{title} [{status}]"),
        (None, Some(revision)) => format!("{title} [r{revision}]"),
        (None, None) => title.to_string(),
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}

fn push_selected_artifacts(prompt: &mut String, input: &MedicalContextAssemblyInput<'_>) {
    prompt.push_str("Selected artifact metadata\n");
    if input.selected_artifacts.is_empty() && input.draft_document.is_none() {
        prompt.push_str(
            "No saved artifact metadata selected; saved file metadata is excluded by default.\n",
        );
    } else {
        for document in input.selected_artifacts.iter().take(8) {
            prompt.push_str("- ");
            prompt.push_str(&document_context_label(document));
            prompt.push_str(" (");
            prompt.push_str(&document.id);
            prompt.push_str(") metadata-only; scope: ");
            prompt.push_str(artifact_scope_label(document));
            prompt.push_str("; status: ");
            prompt.push_str(&artifact_presence_label(document));
            prompt.push_str("; size: ");
            prompt.push_str(&artifact_size_label(document));
            if let Some(sha256) = document.sha256.as_deref() {
                prompt.push_str("; sha256: ");
                prompt.push_str(sha256);
            }
            if let Some(edi) = edi_transaction_label(document) {
                prompt.push_str("; EDI: ");
                prompt.push_str(edi);
            }
            if !document.tags.trim().is_empty() {
                prompt.push_str("; tags: ");
                prompt.push_str(document.tags.trim());
            }
            if !document.source_label.trim().is_empty() {
                prompt.push_str("; source/batch: ");
                prompt.push_str(document.source_label.trim());
            }
            prompt.push_str("; local_ref: ");
            prompt.push_str(&document.local_path);
            if !document.notes.trim().is_empty() {
                prompt.push_str(" -- ");
                prompt.push_str(document.notes.trim());
            }
            prompt.push('\n');
        }
        let excluded_count = input
            .total_artifact_count
            .saturating_sub(input.selected_artifacts.len());
        if excluded_count > 0 {
            prompt.push_str("- excluded saved artifact metadata count: ");
            prompt.push_str(&excluded_count.to_string());
            prompt.push('\n');
        }
        if let Some(draft) = &input.draft_document {
            prompt.push_str("- unsaved draft: ");
            prompt.push_str(draft.kind.trim());
            prompt.push_str(" / ");
            prompt.push_str(draft.title.trim());
            prompt.push_str(" / ");
            prompt.push_str(draft.local_path.trim());
            if !draft.notes.trim().is_empty() {
                prompt.push_str(" -- ");
                prompt.push_str(draft.notes.trim());
            }
            prompt.push('\n');
        }
    }
    prompt.push('\n');
}

fn push_selected_derivatives(prompt: &mut String, input: &MedicalContextAssemblyInput<'_>) {
    prompt.push_str("Selected human-provided artifact derivatives\n");
    if input.selected_derivatives.is_empty() {
        prompt.push_str(
            "No derivative text selected; saved derivative text is excluded by default.\n",
        );
    } else {
        for derivative in input.selected_derivatives.iter().take(8) {
            prompt.push_str("- ");
            prompt.push_str(&derivative_context_label(derivative));
            prompt.push_str(" (");
            prompt.push_str(&derivative.id);
            prompt.push_str(") human-provided; artifact_id: ");
            prompt.push_str(&derivative.document_id);
            prompt.push_str("; status: ");
            prompt.push_str(derivative_status_label(&derivative.review_status));
            prompt.push_str("; source: ");
            prompt.push_str(nonempty_or(&derivative.source_method, "unknown").as_str());
            prompt.push_str("; range: ");
            prompt.push_str(&derivative_range_label(derivative));
            if !derivative.tags.trim().is_empty() {
                prompt.push_str("; tags: ");
                prompt.push_str(derivative.tags.trim());
            }
            prompt.push_str("\n  text:\n");
            prompt.push_str(derivative.body.trim());
            prompt.push('\n');
        }
        let excluded_count = input
            .total_derivative_count
            .saturating_sub(input.selected_derivatives.len());
        if excluded_count > 0 {
            prompt.push_str("- excluded saved derivative text count: ");
            prompt.push_str(&excluded_count.to_string());
            prompt.push('\n');
        }
    }
    prompt.push('\n');
}

fn push_selected_clips(prompt: &mut String, input: &MedicalContextAssemblyInput<'_>) {
    prompt.push_str("Selected human-reviewed context clips\n");
    if input.selected_clips.is_empty() {
        prompt.push_str("No clip excerpts selected; saved clips are excluded by default.\n");
    } else {
        for clip in input.selected_clips.iter().take(8) {
            prompt.push_str("- ");
            prompt.push_str(&clip_context_label(clip));
            prompt.push_str(" (");
            prompt.push_str(&clip.id);
            prompt.push_str(") human-selected excerpt; derivative_id: ");
            prompt.push_str(&clip.derivative_id);
            prompt.push_str("; artifact_id: ");
            prompt.push_str(&clip.document_id);
            prompt.push_str("; status: ");
            prompt.push_str(derivative_status_label(&clip.review_status));
            prompt.push_str("; source: ");
            prompt.push_str(nonempty_or(&clip.source_method, "unknown").as_str());
            prompt.push_str("; range: ");
            prompt.push_str(&clip_range_label(clip));
            if !clip.tags.trim().is_empty() {
                prompt.push_str("; tags: ");
                prompt.push_str(clip.tags.trim());
            }
            prompt.push_str("\n  excerpt:\n");
            prompt.push_str(clip.body.trim());
            prompt.push('\n');
        }
        let excluded_count = input
            .total_clip_count
            .saturating_sub(input.selected_clips.len());
        if excluded_count > 0 {
            prompt.push_str("- excluded saved clip count: ");
            prompt.push_str(&excluded_count.to_string());
            prompt.push('\n');
        }
    }
    prompt.push('\n');
}

fn push_active_jobs(prompt: &mut String, input: &MedicalContextAssemblyInput<'_>) {
    prompt.push_str("Active jobs\n");
    for task in input.active_tasks.iter().take(8) {
        prompt.push_str("- ");
        prompt.push_str(&task.title);
        prompt.push_str(" (");
        prompt.push_str(&task.id);
        prompt.push_str(") ");
        prompt.push_str(workspace_task_status_label(task.status));
        prompt.push_str(" / ");
        prompt.push_str(workspace_task_priority_label(task.priority));
        if let Some(due_date) = task.due_date.as_deref() {
            prompt.push_str(" due ");
            prompt.push_str(due_date);
        }
        if let Some(assigned_to) = task.assigned_to.as_deref() {
            prompt.push_str(" assigned to ");
            prompt.push_str(assigned_to);
        }
        prompt.push('\n');
    }
    if let Some(draft) = &input.draft_task {
        prompt.push_str("- unsaved draft: ");
        prompt.push_str(draft.title.trim());
        if !draft.details.trim().is_empty() {
            prompt.push_str(" -- ");
            prompt.push_str(draft.details.trim());
        }
        prompt.push('\n');
    } else if input.active_tasks.is_empty() {
        prompt.push_str("No open jobs for this patient.\n");
    }
    prompt.push('\n');
}

fn push_note_context(prompt: &mut String, input: &MedicalContextAssemblyInput<'_>) {
    prompt.push_str("Active note draft\n");
    push_context_line(prompt, "Title", input.note_title);
    push_context_line(prompt, "Status", input.note_status);
    push_context_line(prompt, "Revision", &input.note_revision.to_string());
    if let Some(signature) = input.signatures.first() {
        push_context_line(
            prompt,
            "Signature",
            &format!(
                "signed revision {} by {}",
                signature.revision, signature.signer
            ),
        );
    }
    if input.addenda_count > 0 {
        push_context_line(
            prompt,
            "Addenda",
            &format!("{} addenda linked to this note", input.addenda_count),
        );
    }
    if input.pending_proposal_count > 0 {
        push_context_line(
            prompt,
            "Pending proposals",
            &format!(
                "{} pending proposal(s) for human review",
                input.pending_proposal_count
            ),
        );
    }
    prompt.push_str("Body:\n");
    prompt.push_str(input.note_body);
    if input.note_locked {
        prompt.push_str("\n\nThis note is signed/locked. Please propose an addendum or follow-up task for human review instead of a replacement edit.");
    }
}

fn selected_artifact_trace_summary(selected_artifacts: &[&WorkspaceDocument]) -> String {
    if selected_artifacts.is_empty() {
        return "0 selected artifacts".to_string();
    }
    let mut labels = selected_artifacts
        .iter()
        .take(3)
        .map(|document| {
            format!(
                "{} [{}; {}; {}]",
                document_context_label(document),
                artifact_presence_label(document),
                artifact_size_label(document),
                compact_preview(&document.local_path, 40)
            )
        })
        .collect::<Vec<_>>();
    let extra = selected_artifacts.len().saturating_sub(labels.len());
    if extra > 0 {
        labels.push(format!("+{extra} more"));
    }
    format!(
        "{} selected artifact(s): {}",
        selected_artifacts.len(),
        labels.join("; ")
    )
}

fn selected_derivative_trace_summary(
    selected_derivatives: &[&WorkspaceArtifactDerivative],
) -> String {
    if selected_derivatives.is_empty() {
        return "0 selected derivatives".to_string();
    }
    let mut labels = selected_derivatives
        .iter()
        .take(3)
        .map(|derivative| {
            format!(
                "{} [{}; {}]",
                derivative_context_label(derivative),
                derivative_status_label(&derivative.review_status),
                compact_preview(&derivative.body, 48)
            )
        })
        .collect::<Vec<_>>();
    let extra = selected_derivatives.len().saturating_sub(labels.len());
    if extra > 0 {
        labels.push(format!("+{extra} more"));
    }
    format!(
        "{} selected derivative(s): {}",
        selected_derivatives.len(),
        labels.join("; ")
    )
}

fn selected_clip_trace_summary(selected_clips: &[&WorkspaceContextClip]) -> String {
    if selected_clips.is_empty() {
        return "0 selected clips".to_string();
    }
    let mut labels = selected_clips
        .iter()
        .take(3)
        .map(|clip| {
            format!(
                "{} [{}; {}; {}]",
                clip_context_label(clip),
                derivative_status_label(&clip.review_status),
                clip_range_label(clip),
                compact_preview(&clip.body, 48)
            )
        })
        .collect::<Vec<_>>();
    let extra = selected_clips.len().saturating_sub(labels.len());
    if extra > 0 {
        labels.push(format!("+{extra} more"));
    }
    format!(
        "{} selected clip(s): {}",
        selected_clips.len(),
        labels.join("; ")
    )
}

fn selected_ids_json<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    let mut unique_ids = BTreeSet::new();
    let encoded = ids
        .into_iter()
        .filter_map(|id| {
            let id = id.trim();
            (!id.is_empty() && unique_ids.insert(id.to_string()))
                .then(|| format!("\"{}\"", json_string_escape(id)))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{encoded}]")
}

fn context_envelope_json(
    input: &MedicalContextAssemblyInput<'_>,
    prompt: &str,
    preview_lines: &[String],
    artifact_summary: &str,
    derivative_summary: &str,
    clip_summary: &str,
    chart_context_summary: &str,
) -> String {
    let human_request = if input.agent_request_body.trim().is_empty() {
        "General Medical Agent Plan request".to_string()
    } else {
        input.agent_request_body.trim().to_string()
    };
    let selected_artifact_ids = input
        .selected_artifacts
        .iter()
        .map(|document| document.id.as_str())
        .collect::<Vec<_>>();
    let selected_derivative_ids = input
        .selected_derivatives
        .iter()
        .map(|derivative| derivative.id.as_str())
        .collect::<Vec<_>>();
    let selected_clip_ids = input
        .selected_clips
        .iter()
        .map(|clip| clip.id.as_str())
        .collect::<Vec<_>>();
    let selected_artifacts = input
        .selected_artifacts
        .iter()
        .map(|document| {
            json!({
                "id": document.id,
                "title": document.title,
                "kind": document.kind,
                "scope": artifact_scope_label(document),
                "detectedKind": document.detected_kind,
                "mediaLabel": artifact_media_label(document),
                "mimeType": document.mime_type.as_deref(),
                "localPath": document.local_path,
                "notes": document.notes,
                "tags": document.tags,
                "sourceLabel": document.source_label,
                "existenceStatus": artifact_presence_label(document),
                "recordedExistenceStatus": document.existence_status,
                "fileSizeBytes": document.file_size_bytes,
                "modifiedAt": document.modified_at,
                "sha256": document.sha256.as_deref(),
                "ediTransaction": edi_transaction_label(document),
                "contextLabel": document_context_label(document),
                "inclusionMode": "metadata_only",
            })
        })
        .collect::<Vec<_>>();
    let selected_derivatives = input
        .selected_derivatives
        .iter()
        .map(|derivative| {
            json!({
                "id": derivative.id,
                "artifactId": derivative.document_id,
                "clientId": derivative.client_id,
                "encounterId": derivative.encounter_id.as_deref(),
                "noteId": derivative.note_id.as_deref(),
                "kind": derivative.kind,
                "kindLabel": derivative_kind_title(&derivative.kind),
                "title": derivative.title,
                "body": derivative.body,
                "reviewStatus": derivative.review_status,
                "reviewStatusLabel": derivative_status_label(&derivative.review_status),
                "sourceMethod": derivative.source_method,
                "pageRange": derivative.page_range,
                "timestampRange": derivative.timestamp_range,
                "segmentLabel": derivative.segment_label,
                "rangeLabel": derivative_range_label(derivative),
                "tags": derivative.tags,
                "contextLabel": derivative_context_label(derivative),
                "inclusionMode": "selected_derivative_text",
            })
        })
        .collect::<Vec<_>>();
    let selected_clips = input
        .selected_clips
        .iter()
        .map(|clip| {
            json!({
                "id": clip.id,
                "derivativeId": clip.derivative_id,
                "artifactId": clip.document_id,
                "clientId": clip.client_id,
                "encounterId": clip.encounter_id.as_deref(),
                "noteId": clip.note_id.as_deref(),
                "kind": clip.kind,
                "kindLabel": clip_kind_title(&clip.kind),
                "title": clip.title,
                "body": clip.body,
                "reviewStatus": clip.review_status,
                "reviewStatusLabel": derivative_status_label(&clip.review_status),
                "sourceMethod": clip.source_method,
                "pageRange": clip.page_range,
                "timestampRange": clip.timestamp_range,
                "lineRange": clip.line_range,
                "segmentLabel": clip.segment_label,
                "rangeLabel": clip_range_label(clip),
                "tags": clip.tags,
                "contextLabel": clip_context_label(clip),
                "inclusionMode": "selected_clip_excerpt",
            })
        })
        .collect::<Vec<_>>();
    let active_jobs = input
        .active_tasks
        .iter()
        .map(|task| {
            json!({
                "id": task.id,
                "title": task.title,
                "details": task.details,
                "status": workspace_task_status_label(task.status),
                "priority": workspace_task_priority_label(task.priority),
                "dueDate": task.due_date.as_deref(),
                "assignedTo": task.assigned_to.as_deref(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "assemblyVersion": MEDICAL_CONTEXT_ASSEMBLY_VERSION,
        "sourceMode": nonempty_or(input.source_mode, "ctrl_g_handoff"),
        "includeDocuments": false,
        "humanRequest": human_request,
        "ids": {
            "clientId": input.client_id,
            "encounterId": input.encounter_id,
            "noteId": input.note_id,
            "selectedArtifactIds": selected_artifact_ids,
            "selectedDerivativeIds": selected_derivative_ids,
            "selectedClipIds": selected_clip_ids,
        },
        "patient": {
            "displayName": input.patient_display_name,
            "preferredName": input.patient_preferred_name,
            "dateOfBirth": input.patient_date_of_birth,
            "sexOrGender": input.patient_sex_or_gender,
            "externalId": input.patient_external_id,
            "recordStartDate": input.patient_record_start_date,
            "recordEndDate": input.patient_record_end_date,
            "summary": input.patient_summary,
        },
        "note": {
            "title": input.note_title,
            "status": input.note_status,
            "revision": input.note_revision,
            "locked": input.note_locked,
            "addendaCount": input.addenda_count,
            "pendingProposalCount": input.pending_proposal_count,
        },
        "summaries": {
            "artifactSummary": artifact_summary,
            "derivativeSummary": derivative_summary,
            "clipSummary": clip_summary,
            "chartContextSummary": chart_context_summary,
        },
        "counts": {
            "selectedArtifacts": input.selected_artifacts.len(),
            "totalArtifacts": input.total_artifact_count,
            "selectedDerivatives": input.selected_derivatives.len(),
            "totalDerivatives": input.total_derivative_count,
            "selectedClips": input.selected_clips.len(),
            "totalClips": input.total_clip_count,
            "activeJobs": input.active_tasks.len(),
        },
        "selectedArtifacts": selected_artifacts,
        "selectedDerivatives": selected_derivatives,
        "selectedClips": selected_clips,
        "activeJobs": active_jobs,
        "previewSnapshot": preview_lines,
        "promptSnapshot": prompt,
        "safety": PACKET_SAFETY_LINES,
        "replay": {
            "historical": true,
            "sourceRowsMayHaveChanged": true,
            "originalFilesUnchanged": true,
        },
    })
    .to_string()
}

fn json_string_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(crate) fn compact_preview(value: &str, max_chars: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= max_chars {
        single_line
    } else {
        let mut preview = single_line
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        preview.push_str("...");
        preview
    }
}

pub(crate) fn document_context_label(document: &WorkspaceDocument) -> String {
    let scope = artifact_scope_label(document);
    let media = artifact_media_label(document);
    if let Some(edi) = edi_transaction_label(document) {
        format!("{scope} {media} {edi}: {}", document.title)
    } else {
        format!("{scope} {media}: {}", document.title)
    }
}

pub(crate) fn derivative_context_label(derivative: &WorkspaceArtifactDerivative) -> String {
    format!(
        "{}: {}",
        derivative_kind_title(&derivative.kind),
        derivative.title
    )
}

pub(crate) fn clip_context_label(clip: &WorkspaceContextClip) -> String {
    format!("{}: {}", clip_kind_title(&clip.kind), clip.title)
}

pub(crate) fn derivative_kind_title(kind: &str) -> &'static str {
    let kind = kind.trim().to_ascii_lowercase();
    if kind.contains("transcript") {
        "Transcript"
    } else if kind.contains("ocr") || kind.contains("extract") {
        "OCR/extracted text"
    } else if kind.contains("video") || kind.contains("gait") {
        "Video observation"
    } else if kind.contains("edi") {
        "EDI summary"
    } else if kind.contains("validation") {
        "Validation note"
    } else if kind.contains("billing") {
        "Billing note"
    } else if kind.contains("keyframe") {
        "Keyframe note"
    } else {
        "Human annotation"
    }
}

pub(crate) fn clip_kind_title(kind: &str) -> &'static str {
    let kind = kind.trim().to_ascii_lowercase();
    if kind.contains("transcript") {
        "Transcript excerpt"
    } else if kind.contains("ocr") || kind.contains("extract") {
        "OCR excerpt"
    } else if kind.contains("video") || kind.contains("gait") {
        "Video observation excerpt"
    } else if kind.contains("edi") {
        "EDI excerpt"
    } else if kind.contains("validation") {
        "Validation excerpt"
    } else if kind.contains("billing") {
        "Billing excerpt"
    } else if kind.contains("annotation") {
        "Annotation excerpt"
    } else {
        "Context excerpt"
    }
}

pub(crate) fn default_derivative_kind_for_document(document: &WorkspaceDocument) -> &'static str {
    match artifact_media_label(document) {
        "audio" => "transcript",
        "video" => "video observation",
        "EDI" => "EDI summary",
        "PDF" | "image" => "OCR/extracted text",
        _ => "human annotation",
    }
}

pub(crate) fn default_clip_kind_for_derivative(kind: &str) -> &'static str {
    let kind = kind.trim().to_ascii_lowercase();
    if kind.contains("transcript") {
        "transcript excerpt"
    } else if kind.contains("ocr") || kind.contains("extract") {
        "OCR excerpt"
    } else if kind.contains("video") || kind.contains("gait") {
        "video observation excerpt"
    } else if kind.contains("edi") {
        "EDI summary excerpt"
    } else if kind.contains("validation") {
        "validation note excerpt"
    } else if kind.contains("billing") {
        "billing note excerpt"
    } else {
        "generic excerpt"
    }
}

pub(crate) fn derivative_status_label(status: &str) -> &'static str {
    match status.trim() {
        "human_reviewed" => "human reviewed",
        "superseded" => "superseded",
        "archived" => "archived",
        "draft" => "draft",
        _ => "unverified",
    }
}

pub(crate) fn derivative_range_label(derivative: &WorkspaceArtifactDerivative) -> String {
    let mut parts = Vec::new();
    if !derivative.page_range.trim().is_empty() {
        parts.push(format!("page {}", derivative.page_range.trim()));
    }
    if !derivative.timestamp_range.trim().is_empty() {
        parts.push(format!("time {}", derivative.timestamp_range.trim()));
    }
    if !derivative.segment_label.trim().is_empty() {
        parts.push(format!("segment {}", derivative.segment_label.trim()));
    }
    if parts.is_empty() {
        "whole derivative".to_string()
    } else {
        parts.join("; ")
    }
}

pub(crate) fn clip_range_label(clip: &WorkspaceContextClip) -> String {
    let mut parts = Vec::new();
    if !clip.page_range.trim().is_empty() {
        parts.push(format!("page {}", clip.page_range.trim()));
    }
    if !clip.timestamp_range.trim().is_empty() {
        parts.push(format!("time {}", clip.timestamp_range.trim()));
    }
    if !clip.line_range.trim().is_empty() {
        parts.push(format!("lines {}", clip.line_range.trim()));
    }
    if !clip.segment_label.trim().is_empty() {
        parts.push(format!("segment {}", clip.segment_label.trim()));
    }
    if parts.is_empty() {
        "selected excerpt".to_string()
    } else {
        parts.join("; ")
    }
}

pub(crate) fn artifact_scope_label(document: &WorkspaceDocument) -> &'static str {
    match document.scope.trim().to_ascii_lowercase().as_str() {
        "patient" | "patient-chart" | "chart" => return "patient",
        "practice" | "practice-wide" | "billing" | "payer" => return "practice",
        _ => {}
    }
    let haystack = artifact_haystack(document);
    if edi_transaction_label(document).is_some()
        || haystack.contains("practice")
        || haystack.contains("payer")
        || haystack.contains("billing")
        || haystack.contains("claim")
        || haystack.contains("fee schedule")
        || haystack.contains("x12")
        || haystack.contains("edi")
    {
        "practice"
    } else {
        "patient"
    }
}

fn artifact_media_label(document: &WorkspaceDocument) -> &'static str {
    let detected = document.detected_kind.trim().to_ascii_lowercase();
    if !detected.is_empty() {
        if detected.contains("edi") {
            return "EDI";
        }
        if detected.contains("pdf") {
            return "PDF";
        }
        if detected.contains("video") {
            return "video";
        }
        if detected.contains("audio") {
            return "audio";
        }
        if detected.contains("image") {
            return "image";
        }
    }
    let haystack = artifact_haystack(document);
    if edi_transaction_label(document).is_some() || haystack.contains("x12") {
        "EDI"
    } else if haystack.contains("pdf") {
        "PDF"
    } else if haystack.contains("mp4") || haystack.contains("video") || haystack.contains("gait") {
        "video"
    } else if haystack.contains("wav")
        || haystack.contains("mp3")
        || haystack.contains("m4a")
        || haystack.contains("audio")
        || haystack.contains("dictation")
    {
        "audio"
    } else if haystack.contains("jpg")
        || haystack.contains("jpeg")
        || haystack.contains("png")
        || haystack.contains("image")
    {
        "image"
    } else if haystack.contains("hl7") {
        "HL7"
    } else if haystack.contains("fhir") {
        "FHIR"
    } else if haystack.contains("cda") || haystack.contains("ccd") {
        "CDA"
    } else if haystack.contains("dicom") || haystack.contains("dcm") {
        "DICOM"
    } else if haystack.contains("csv") {
        "CSV"
    } else if haystack.contains("xlsx") || haystack.contains("xls") {
        "spreadsheet"
    } else {
        "file"
    }
}

pub(crate) fn edi_transaction_label(document: &WorkspaceDocument) -> Option<&'static str> {
    let haystack = format!(
        "{} {}",
        artifact_haystack(document),
        document.metadata_json.to_ascii_lowercase()
    );
    edi_transaction_label_from_text(&haystack)
}

pub(crate) fn edi_transaction_label_from_text(text: &str) -> Option<&'static str> {
    const TYPES: &[(&str, &str)] = &[
        ("277ca", "277CA claim ack"),
        ("837p", "837P professional claim"),
        ("837i", "837I institutional claim"),
        ("837d", "837D dental claim"),
        ("835", "835 remittance"),
        ("270", "270 eligibility inquiry"),
        ("271", "271 eligibility response"),
        ("276", "276 claim status request"),
        ("277", "277 claim status response"),
        ("278", "278 prior authorization"),
        ("275", "275 attachment"),
        ("834", "834 enrollment"),
        ("820", "820 payment order"),
        ("999", "999 implementation ack"),
        ("997", "997 functional ack"),
        ("ta1", "TA1 interchange ack"),
    ];
    TYPES
        .iter()
        .find_map(|(needle, label)| text.contains(needle).then_some(*label))
}

fn artifact_haystack(document: &WorkspaceDocument) -> String {
    format!(
        "{} {} {} {} {} {} {}",
        document.kind,
        document.title,
        document.local_path,
        document.notes,
        document.tags,
        document.source_label,
        document.detected_kind
    )
    .to_ascii_lowercase()
}

pub(crate) fn artifact_size_label(document: &WorkspaceDocument) -> String {
    match document.file_size_bytes {
        Some(bytes) if bytes >= 1024 * 1024 => format!("{:.1} MiB", bytes as f64 / 1048576.0),
        Some(bytes) if bytes >= 1024 => format!("{:.1} KiB", bytes as f64 / 1024.0),
        Some(bytes) => format!("{bytes} bytes"),
        None => "size unknown".to_string(),
    }
}

pub(crate) fn artifact_presence_label(document: &WorkspaceDocument) -> String {
    match fs::metadata(document.local_path.trim()) {
        Ok(metadata) if metadata.is_file() => "present".to_string(),
        Ok(_) => "present but not a regular file".to_string(),
        Err(_) => match document.existence_status.trim() {
            "missing" | "inaccessible" => document.existence_status.clone(),
            _ => "missing or inaccessible".to_string(),
        },
    }
}

fn push_context_line(prompt: &mut String, label: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    prompt.push_str(label);
    prompt.push_str(": ");
    prompt.push_str(value.trim());
    prompt.push('\n');
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn workspace_task_status_label(status: WorkspaceTaskStatus) -> &'static str {
    match status {
        WorkspaceTaskStatus::Open => "open",
        WorkspaceTaskStatus::InProgress => "in progress",
        WorkspaceTaskStatus::Blocked => "blocked",
        WorkspaceTaskStatus::Done => "done",
        WorkspaceTaskStatus::Canceled => "canceled",
    }
}

fn workspace_task_priority_label(priority: WorkspaceTaskPriority) -> &'static str {
    match priority {
        WorkspaceTaskPriority::Low => "low",
        WorkspaceTaskPriority::Normal => "normal",
        WorkspaceTaskPriority::High => "high",
        WorkspaceTaskPriority::Urgent => "urgent",
    }
}
