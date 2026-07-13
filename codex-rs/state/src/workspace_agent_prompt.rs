use crate::WorkspaceContextPacket;

/// Stable inputs for the exact medical handoff prompt authorized by an agent run.
///
/// Keeping this renderer below the TUI gives core and state the same canonical prompt to verify
/// before any model request or chart-context read is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentHandoffPromptInput {
    pub packet_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub human_request: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    pub context_envelope_sha256: String,
    pub authorized_scope_json: String,
}

impl From<&WorkspaceContextPacket> for WorkspaceAgentHandoffPromptInput {
    fn from(packet: &WorkspaceContextPacket) -> Self {
        Self {
            packet_id: packet.id.clone(),
            client_id: packet.client_id.clone(),
            encounter_id: packet.encounter_id.clone(),
            note_id: packet.note_id.clone(),
            human_request: packet.human_request.clone(),
            chart_context_summary: packet.chart_context_summary.clone(),
            context_envelope_json: packet.context_envelope_json.clone(),
            context_envelope_sha256: packet.context_envelope_sha256.clone(),
            authorized_scope_json: packet.authorized_scope_json.clone(),
        }
    }
}

pub fn render_workspace_agent_handoff_prompt(
    packet: &WorkspaceAgentHandoffPromptInput,
    run_id: Option<&str>,
) -> String {
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
    prompt.push_str(&packet.packet_id);
    prompt.push('\n');
    prompt.push_str("- context_envelope_sha256: ");
    prompt.push_str(&packet.context_envelope_sha256);
    prompt.push('\n');
    if let Some(run_id) = run_id {
        prompt.push_str("- run_id: ");
        prompt.push_str(run_id);
        prompt.push('\n');
        prompt.push_str("- authorized context endpoint: workspace/agent/run/context/read\n");
        prompt.push_str("- model tool: workspace_context_read (pass this run_id plus visit_history or progress_notes)\n");
    }
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
    prompt.push_str("Authorized run scope JSON:\n");
    prompt.push_str(&packet.authorized_scope_json);
    prompt.push_str("\n\n");
    if let Some(run_id) = run_id {
        prompt.push_str("Execution audit:\n");
        prompt.push_str("- this packet handoff is recorded as run ");
        prompt.push_str(run_id);
        prompt.push('\n');
        prompt.push_str("- bind returned work to this run so the result, sources, proposal, and clinician decision remain traceable\n\n");
    }
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
    prompt.push_str("- return: reopen /workspacemedical; the matching completed response is saved automatically as review-pending Agent Work\n");
    prompt.push_str("- do not submit this composer prompt until the packet id/hash and scope match the intended chart\n\n");
    prompt.push_str("Packet access boundary:\n");
    prompt.push_str("- use this packet envelope plus only categories returned by workspace_context_read for this run id\n");
    prompt.push_str("- do not infer access to the rest of the workspace\n");
    prompt.push_str("- do not call workspace/context/get, list documents, or use any other workspace read to expand this packet\n");
    prompt.push_str("- each authorized context read returns and records immutable source snapshots; if a category is denied, ask the clinician for a new packet\n");
    prompt.push_str("- current source rows may have changed; the stored envelope is the authoritative sent snapshot\n");
    prompt.push_str("- original local files are not uploaded, parsed, transcribed, OCRed, or analyzed automatically\n");
    prompt.push_str(
        "- do not write to chart, sign notes, submit claims, contact payers, or mutate records\n",
    );
    prompt.push_str(
        "- captured Agent Work cannot change the chart without explicit human review in /workspacemedical\n",
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
    packet: &WorkspaceAgentHandoffPromptInput,
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

fn compact_preview(value: &str, max_chars: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_prompt_binds_the_run_packet_and_authoritative_envelope() {
        let input = WorkspaceAgentHandoffPromptInput {
            packet_id: "packet-1".to_string(),
            client_id: "patient-1".to_string(),
            encounter_id: Some("encounter-1".to_string()),
            note_id: Some("note-1".to_string()),
            human_request: "Draft a reviewable daily note.".to_string(),
            chart_context_summary: "Synthetic chart".to_string(),
            context_envelope_json: serde_json::json!({
                "patient": { "displayName": "Synthetic Patient" },
                "note": { "title": "Daily note", "status": "draft", "revision": 2 },
                "promptSnapshot": "Exact synthetic snapshot",
            })
            .to_string(),
            context_envelope_sha256: "envelope-hash".to_string(),
            authorized_scope_json: r#"{"categories":["progress_notes"]}"#.to_string(),
        };

        let prompt = render_workspace_agent_handoff_prompt(&input, Some("run-1"));

        assert!(prompt.contains("- packet_id: packet-1"));
        assert!(prompt.contains("- context_envelope_sha256: envelope-hash"));
        assert!(prompt.contains("- run_id: run-1"));
        assert!(prompt.contains("Daily note [draft r2]"));
        assert!(prompt.contains("Exact synthetic snapshot"));
    }
}
