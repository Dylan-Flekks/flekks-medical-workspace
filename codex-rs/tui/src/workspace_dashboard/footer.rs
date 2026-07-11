use crate::workspace_context_assembly::compact_preview;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MedicalKeyContext {
    PatientSearch,
    CommandPalette,
    Directory,
    PatientNotes,
    Documents,
    PatientField,
    NoteTitle,
    NoteBody,
    LockedNote,
    WorkflowEditor,
    CenterSections,
    ProposalReview,
    AgentTabs,
    AgentInput,
    PacketReview,
    ReturnedWorkReview,
    ReturnedWorkDraft,
    ChartConflict,
    ChartRetry,
    ChartBlocked,
    ChartManualMerge,
}

impl MedicalKeyContext {
    fn full_hint(self) -> &'static str {
        match self {
            Self::PatientSearch => "↑/↓ patient  Enter open  Esc chart",
            Self::CommandPalette => "↑/↓ command  Enter run  Esc close",
            Self::Directory => "↑/↓ patient  Enter open  / search  Tab/⇧Tab pane  Ctrl-P commands",
            Self::PatientNotes => "↑/↓ note  Enter open  Tab/⇧Tab pane  Ctrl-P commands",
            Self::Documents => {
                "↑/↓ file  ←/→ fold  Enter detail  Space agent  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::PatientField => {
                "↑/↓ field  Enter next  Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::NoteTitle => {
                "↑/↓ stay  Type title  Enter body  Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::NoteBody => {
                "↑/↓ stay  Type note  Enter newline  Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::LockedNote => "Read-only  :addendum  Tab/⇧Tab pane  Ctrl-P commands  Esc close",
            Self::WorkflowEditor => {
                "↑/↓ field  Enter next  Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::CenterSections => "↑/↓ section  PgUp/PgDn scroll  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ProposalReview => "↑/↓ scroll  PgUp/PgDn page  Tab/⇧Tab pane  Ctrl-P commands",
            Self::AgentTabs => {
                "←/→ Agent tab  ↑/↓ scroll  r request  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::AgentInput => {
                "Type request  Ctrl-G send  Tab/⇧Tab pane  Ctrl-P commands  Esc chart"
            }
            Self::PacketReview => {
                "Review packet  Ctrl-G send  r edit  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::ReturnedWorkReview => {
                "↑/↓ scroll  i inspect  r input  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::ReturnedWorkDraft => "Type result  Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ChartConflict => "Ctrl-S refresh  Esc/Ctrl-W close",
            Self::ChartRetry => "Ctrl-S retry exact save  Esc/Ctrl-W close",
            Self::ChartBlocked => "Esc/Ctrl-W discard and reload",
            Self::ChartManualMerge => "Edit focused draft  Ctrl-S blocked  Esc/Ctrl-W close",
        }
    }

    fn compact_hint(self) -> &'static str {
        match self {
            Self::PatientSearch => "↑/↓ patient  Enter  Esc",
            Self::CommandPalette => "↑/↓ command  Enter  Esc",
            Self::Directory => "↑/↓ patient  Tab/⇧Tab pane  Ctrl-P commands",
            Self::PatientNotes => "↑/↓ note  Tab/⇧Tab pane  Ctrl-P commands",
            Self::Documents => "↑/↓ file  ←/→ fold  Tab/⇧Tab pane  Ctrl-P commands",
            Self::PatientField | Self::WorkflowEditor => {
                "↑/↓ field  Ctrl-S  Tab/⇧Tab pane  Ctrl-P commands"
            }
            Self::NoteTitle => "↑/↓ stay  Enter body  Tab/⇧Tab pane  Ctrl-P commands",
            Self::NoteBody => "↑/↓ stay  Enter newline  Tab/⇧Tab pane  Ctrl-P commands",
            Self::LockedNote => "Read-only  Tab/⇧Tab pane  Ctrl-P commands",
            Self::CenterSections => "↑/↓ section  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ProposalReview => "↑/↓ scroll  Tab/⇧Tab pane  Ctrl-P commands",
            Self::AgentTabs => "←/→ Agent tab  ↑/↓ scroll  Tab/⇧Tab pane  Ctrl-P commands",
            Self::AgentInput => "Ctrl-G send  Tab/⇧Tab pane  Ctrl-P commands",
            Self::PacketReview => "Ctrl-G send  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ReturnedWorkReview => "↑/↓ scroll  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ReturnedWorkDraft => "Ctrl-S save  Tab/⇧Tab pane  Ctrl-P commands",
            Self::ChartConflict => "Ctrl-S refresh  Esc/Ctrl-W close",
            Self::ChartRetry => "Ctrl-S retry  Esc/Ctrl-W close",
            Self::ChartBlocked => "Esc/Ctrl-W discard",
            Self::ChartManualMerge => "Edit draft  Ctrl-S blocked  Esc/Ctrl-W close",
        }
    }
}

pub(super) fn compose_medical_footer(
    focus: &str,
    mode: &str,
    context: MedicalKeyContext,
    next: &str,
    status: &str,
    width: u16,
) -> String {
    let focus = format!("Focus: {focus}");
    let mode = format!("Mode: {mode}");
    let next = format!("Next: {next}");
    let status = format!("Status: {status}");

    let build = |mode: Option<&String>, next: Option<&String>, status: Option<&String>, hint| {
        let mut parts = vec![focus.clone()];
        if let Some(mode) = mode {
            parts.push(mode.clone());
        }
        parts.push(format!("Keys: {hint}"));
        if let Some(next) = next {
            parts.push(next.clone());
        }
        if let Some(status) = status {
            parts.push(status.clone());
        }
        parts
    };

    for parts in [
        build(Some(&mode), Some(&next), Some(&status), context.full_hint()),
        build(Some(&mode), Some(&next), None, context.full_hint()),
        build(Some(&mode), None, None, context.full_hint()),
        build(None, None, None, context.full_hint()),
        build(None, None, None, context.compact_hint()),
    ] {
        if joined_width(&parts) <= width as usize {
            return parts.join(" | ");
        }
    }

    fit_footer_parts(&[focus, format!("Keys: {}", context.compact_hint())], width)
}

pub(super) fn compose_medical_status_footer(focus: &str, status: &str, width: u16) -> String {
    fit_footer_parts(
        &[format!("Focus: {focus}"), format!("Status: {status}")],
        width,
    )
}

pub(super) fn compose_mode_footer(
    mode: &str,
    input: &str,
    context: MedicalKeyContext,
    width: u16,
) -> String {
    let mode = format!("Mode: {mode}");
    let input = truncate_to_width(input, 24);
    for parts in [
        vec![
            mode.clone(),
            format!("Keys: {}", context.full_hint()),
            input.clone(),
        ],
        vec![
            mode.clone(),
            format!("Keys: {}", context.compact_hint()),
            input.clone(),
        ],
    ] {
        if joined_width(&parts) <= width as usize {
            return parts.join(" | ");
        }
    }
    fit_footer_parts(
        &[mode, format!("Keys: {}", context.compact_hint()), input],
        width,
    )
}

pub(super) fn compose_legacy_footer(parts: &[String], width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    let mut parts = parts
        .iter()
        .filter(|part| !part.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    while parts.len() > 2 && legacy_joined_width(&parts) > width {
        parts.pop();
    }
    if legacy_joined_width(&parts) > width {
        let len = parts.len();
        let prefix_width = legacy_joined_width(&parts[..len.saturating_sub(1)]);
        let separator_width = if len > 1 { 3 } else { 0 };
        let available = width.saturating_sub(prefix_width + separator_width).max(8);
        parts[len - 1] = compact_preview(&parts[len - 1], available);
    }
    let joined = parts.join(" | ");
    if joined.chars().count() > width {
        compact_preview(&joined, width)
    } else {
        joined
    }
}

fn fit_footer_parts(parts: &[String], width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    let mut parts = parts
        .iter()
        .filter(|part| !part.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    if joined_width(&parts) > width {
        let len = parts.len();
        let prefix_width = joined_width(&parts[..len.saturating_sub(1)]);
        let separator_width = if len > 1 { 3 } else { 0 };
        let available = width.saturating_sub(prefix_width + separator_width).max(8);
        parts[len - 1] = truncate_to_width(&parts[len - 1], available);
    }
    let joined = parts.join(" | ");
    if UnicodeWidthStr::width(joined.as_str()) > width {
        truncate_to_width(&joined, width)
    } else {
        joined
    }
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let ellipsis = '…';
    let ellipsis_width = UnicodeWidthChar::width(ellipsis).unwrap_or(1);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }
    let content_width = max_width - ellipsis_width;
    let mut width = 0;
    let mut truncated = String::new();
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > content_width {
            break;
        }
        truncated.push(ch);
        width += ch_width;
    }
    truncated.push(ellipsis);
    truncated
}

fn joined_width(parts: &[String]) -> usize {
    parts
        .iter()
        .map(|part| UnicodeWidthStr::width(part.as_str()))
        .sum::<usize>()
        + parts.len().saturating_sub(1) * 3
}

fn legacy_joined_width(parts: &[String]) -> usize {
    parts.iter().map(|part| part.chars().count()).sum::<usize>() + parts.len().saturating_sub(1) * 3
}
