use super::*;
use chrono::DateTime;
use chrono::Utc;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

const RECOVERY_MODAL_TITLE: &str = "Draft Recovery";
const RECOVERY_ACTIONS: &str = "R restore  ·  D discard  ·  N/P next/previous  ·  Esc decide later";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryRenderDensity {
    Full,
    Tiny,
}

#[derive(Debug)]
struct RecoveryConflictDetail {
    text: String,
    needs_attention: bool,
}

impl WorkspaceDashboard {
    /// Render the blocking local-draft recovery prompt above every dashboard surface.
    ///
    /// Callers may invoke this unconditionally after all other workspace overlays; the method is
    /// a no-op unless draft recovery currently owns input.
    pub(crate) fn render_draft_recovery_modal(&self, area: Rect, buf: &mut Buffer) {
        if !self.recovery_modal_visible() || area.width == 0 || area.height == 0 {
            return;
        }

        let modal_area = draft_recovery_modal_rect(area);
        Clear.render(modal_area, buf);
        let block = Block::default()
            .title(RECOVERY_MODAL_TITLE)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(modal_area);
        block.render(modal_area, buf);
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        Paragraph::new(draft_recovery_modal_lines(self, inner.width, inner.height))
            .render(inner, buf);
    }
}

fn draft_recovery_modal_lines(
    dashboard: &WorkspaceDashboard,
    width: u16,
    height: u16,
) -> Vec<Line<'static>> {
    let Some(item) = dashboard.current_recovery_item() else {
        return fit_modal_lines(
            vec![styled_line(
                "No unfinished local draft is selected.",
                Style::default().fg(Color::DarkGray),
                width,
            )],
            recovery_action_lines(width),
            height,
        );
    };
    let density = if width < 50 || height <= 10 {
        RecoveryRenderDensity::Tiny
    } else {
        RecoveryRenderDensity::Full
    };
    let session = &item.session;
    let checkpoint = &session.current_checkpoint;
    let item_count = dashboard.draft_recovery.items.len();
    let item_index = dashboard
        .draft_recovery
        .index
        .min(item_count.saturating_sub(1))
        .saturating_add(1);
    let patient_name = recovery_patient_name(dashboard, item);
    let note_title = recovery_note_title(item);
    let note_identity = checkpoint.note_id.as_deref().unwrap_or("new note");
    let trigger = nonempty_label(&checkpoint.trigger, "unknown trigger");
    let conflict = recovery_conflict_detail(dashboard, item);
    let item_summary = recovery_item_summary(item);

    let mut detail_lines = match density {
        RecoveryRenderDensity::Full => {
            let mut lines = vec![
                styled_line(
                    &format!(
                        "Draft {item_index} of {item_count}  ·  checkpoint revision {}  ·  schema {}",
                        session.current_revision, checkpoint.schema_version
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                    width,
                ),
                plain_line(
                    &format!("Patient: {patient_name}  ·  {}", session.client_id),
                    width,
                ),
                plain_line(&format!("Note: {note_title}  ·  {note_identity}"), width),
                plain_line(&format!("Session: {}", session.id), width),
                plain_line(
                    &format!(
                        "Saved: {}  ·  trigger: {trigger}",
                        recovery_time_label(session.updated_at, /*compact*/ false)
                    ),
                    width,
                ),
                plain_line(&format!("Items: {item_summary}"), width),
            ];
            lines.extend(wrapped_styled_lines(
                &format!("Conflict check: {}", conflict.text),
                width,
                if conflict.needs_attention {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ));
            lines.extend(wrapped_styled_lines(
                "Restore opens this durable checkpoint for review; the canonical chart remains unchanged until an explicit save.",
                width,
                Style::default().fg(Color::DarkGray),
            ));
            lines
        }
        RecoveryRenderDensity::Tiny => vec![
            styled_line(
                &format!(
                    "Draft {item_index}/{item_count}  ·  r{}  ·  {}",
                    session.current_revision,
                    recovery_time_label(session.updated_at, /*compact*/ true)
                ),
                Style::default().add_modifier(Modifier::BOLD),
                width,
            ),
            plain_line(&format!("Patient: {patient_name}"), width),
            plain_line(&format!("Note: {note_title}  ·  {note_identity}"), width),
            plain_line(
                &format!(
                    "Session: {}  ·  {trigger}",
                    truncate_display(&session.id, 12)
                ),
                width,
            ),
            plain_line(&format!("Items: {item_summary}"), width),
            styled_line(
                &format!("Conflict: {}", conflict.text),
                if conflict.needs_attention {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
                width,
            ),
            styled_line(
                "Canonical chart unchanged.",
                Style::default().fg(Color::DarkGray),
                width,
            ),
        ],
    };

    if dashboard.status.starts_with("Draft restore failed")
        || dashboard.status.starts_with("Draft discard failed")
    {
        let mut failure_lines = wrapped_styled_lines(
            &dashboard.status,
            width,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        failure_lines.append(&mut detail_lines);
        detail_lines = failure_lines;
    }

    fit_modal_lines(detail_lines, recovery_action_lines(width), height)
}

fn draft_recovery_modal_rect(area: Rect) -> Rect {
    if area.width < 100 || area.height < 24 {
        return area;
    }
    let width = area
        .width
        .saturating_mul(4)
        .saturating_div(5)
        .max(72)
        .min(area.width);
    let height = area
        .height
        .saturating_mul(3)
        .saturating_div(4)
        .clamp(18, 26)
        .min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn recovery_patient_name(dashboard: &WorkspaceDashboard, item: &DraftRecoveryItem) -> String {
    snapshot_text(
        &item.session.current_checkpoint.draft,
        "/client/displayName",
    )
    .map(one_line)
    .filter(|value| !value.is_empty())
    .or_else(|| {
        dashboard
            .clients
            .iter()
            .find(|client| client.id == item.session.client_id)
            .map(|client| one_line(&client.display_name))
    })
    .unwrap_or_else(|| "Unknown patient".to_string())
}

fn recovery_note_title(item: &DraftRecoveryItem) -> String {
    snapshot_text(&item.session.current_checkpoint.draft, "/note/title")
        .map(one_line)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled note".to_string())
}

fn recovery_item_summary(item: &DraftRecoveryItem) -> String {
    let draft = &item.session.current_checkpoint.draft;
    let mut parts = vec!["patient", "note"];
    if snapshot_text(draft, "/agentRequestBody").is_some_and(|body| !body.trim().is_empty()) {
        parts.push("agent request");
    }
    let artifact_count = snapshot_array_len(draft, "/selectedArtifactIds");
    let derivative_count = snapshot_array_len(draft, "/selectedDerivativeIds");
    let clip_count = snapshot_array_len(draft, "/selectedClipIds");
    let context_count = artifact_count
        .saturating_add(derivative_count)
        .saturating_add(clip_count);
    let mut summary = parts.join(", ");
    if context_count > 0 {
        summary.push_str(&format!(", {context_count} selected context item(s)"));
    }
    summary
}

fn recovery_conflict_detail(
    dashboard: &WorkspaceDashboard,
    item: &DraftRecoveryItem,
) -> RecoveryConflictDetail {
    let session = &item.session;
    let checkpoint = &session.current_checkpoint;
    let same_note_count = dashboard
        .draft_recovery
        .items
        .iter()
        .filter(|queued| queued.session.id != session.id)
        .filter(|queued| queued.session.client_id == session.client_id)
        .filter(|queued| queued.session.current_checkpoint.note_id == checkpoint.note_id)
        .count();
    if same_note_count > 0 {
        return RecoveryConflictDetail {
            text: format!(
                "{same_note_count} other unfinished draft(s) target this same patient and note; restore one at a time"
            ),
            needs_attention: true,
        };
    }
    let Some(canonical_client) = dashboard
        .clients
        .iter()
        .find(|client| client.id == session.client_id)
    else {
        return RecoveryConflictDetail {
            text: "patient is not loaded; restore will revalidate before opening".to_string(),
            needs_attention: true,
        };
    };
    if snapshot_text(&checkpoint.draft, "/baseClientVersion")
        .is_none_or(|version| version != canonical_client.version.as_str())
    {
        return RecoveryConflictDetail {
            text: "patient chart changed since this checkpoint; restore will stop safely"
                .to_string(),
            needs_attention: true,
        };
    }

    let Some(note_id) = checkpoint.note_id.as_deref() else {
        return RecoveryConflictDetail {
            text: "new note; current patient baseline matches".to_string(),
            needs_attention: false,
        };
    };
    if dashboard.draft_client.id.as_deref() != Some(session.client_id.as_str()) {
        return RecoveryConflictDetail {
            text: "note belongs to another loaded patient; restore will revalidate it".to_string(),
            needs_attention: false,
        };
    }
    let Some(canonical_note) = dashboard
        .notes
        .iter()
        .find(|note| note.id == note_id && note.client_id == session.client_id)
    else {
        return RecoveryConflictDetail {
            text: "note is not loaded; restore will revalidate before opening".to_string(),
            needs_attention: true,
        };
    };
    if checkpoint.base_note_revision != Some(canonical_note.current_revision) {
        return RecoveryConflictDetail {
            text: format!(
                "note is now r{} but this draft is based on r{}; restore will stop safely",
                canonical_note.current_revision,
                checkpoint
                    .base_note_revision
                    .map_or_else(|| "?".to_string(), |revision| revision.to_string())
            ),
            needs_attention: true,
        };
    }
    RecoveryConflictDetail {
        text: "no conflict detected in the loaded chart; restore still revalidates".to_string(),
        needs_attention: false,
    }
}

fn recovery_action_lines(width: u16) -> Vec<Line<'static>> {
    wrapped_styled_lines(
        RECOVERY_ACTIONS,
        width,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn fit_modal_lines(
    mut detail_lines: Vec<Line<'static>>,
    mut action_lines: Vec<Line<'static>>,
    height: u16,
) -> Vec<Line<'static>> {
    let height = usize::from(height);
    if height == 0 {
        return Vec::new();
    }
    if action_lines.len() >= height {
        action_lines.truncate(height);
        return action_lines;
    }
    detail_lines.truncate(height - action_lines.len());
    detail_lines.append(&mut action_lines);
    detail_lines
}

fn plain_line(text: &str, width: u16) -> Line<'static> {
    styled_line(text, Style::default(), width)
}

fn styled_line(text: &str, style: Style, width: u16) -> Line<'static> {
    Line::from(Span::styled(
        truncate_display(&one_line(text), usize::from(width)),
        style,
    ))
}

fn wrapped_styled_lines(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let text = one_line(text);
    textwrap::wrap(&text, usize::from(width.max(1)))
        .into_iter()
        .map(|line| Line::from(Span::styled(line.into_owned(), style)))
        .collect()
}

fn snapshot_text<'a>(draft: &'a serde_json::Value, pointer: &str) -> Option<&'a str> {
    draft.pointer(pointer).and_then(serde_json::Value::as_str)
}

fn snapshot_array_len(draft: &serde_json::Value, pointer: &str) -> usize {
    draft
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

fn recovery_time_label(timestamp: i64, compact: bool) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|time| {
            if compact {
                time.format("%Y-%m-%d").to_string()
            } else {
                time.format("%Y-%m-%d %H:%M UTC").to_string()
            }
        })
        .unwrap_or_else(|| "unknown time".to_string())
}

fn nonempty_label<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    }
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_display(text: &str, max_width: usize) -> String {
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
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > content_width {
            break;
        }
        truncated.push(character);
        width += character_width;
    }
    truncated.push(ellipsis);
    truncated
}
