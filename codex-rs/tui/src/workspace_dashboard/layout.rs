use super::WorkspaceLayoutMode;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;

/// Authoritative geometry for the focused document workpane.
///
/// Rendering, cursor placement, and pointer hit testing must all consume these
/// rectangles so a visible editor never has an interaction region from the
/// standard three-zone medical layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MedicalDedicatedWorkpaneAreas {
    pub(super) header: Rect,
    pub(super) clients: Rect,
    pub(super) notes: Rect,
    pub(super) workpane: Rect,
    pub(super) status: Rect,
}

impl MedicalDedicatedWorkpaneAreas {
    pub(super) fn new(area: Rect) -> Option<Self> {
        let mode = WorkspaceLayoutMode::for_area(area);
        if mode == WorkspaceLayoutMode::Tiny {
            return None;
        }
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(if mode == WorkspaceLayoutMode::Compact {
                    1
                } else {
                    2
                }),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(area);
        if mode == WorkspaceLayoutMode::Compact {
            return Some(Self {
                header: vertical[0],
                clients: Rect::default(),
                notes: Rect::default(),
                workpane: vertical[1],
                status: vertical[2],
            });
        }

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(32), Constraint::Min(40)])
            .split(vertical[1]);
        let explorer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(columns[0]);
        Some(Self {
            header: vertical[0],
            clients: explorer[0],
            notes: explorer[1],
            workpane: columns[1],
            status: vertical[2],
        })
    }
}
