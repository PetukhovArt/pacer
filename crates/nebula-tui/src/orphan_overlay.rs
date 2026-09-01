//! The ORPHANED SESSIONS overlay: the conversations of the selected
//! project whose WORKTREE was deleted, and the Enter that brings one back.
//!
//! It is a modal rather than a group in the SESSIONS PANEL because that
//! panel is scoped to one worktree (`App::visible_sessions` filters on
//! `worktree_id`) and these rows have none — the worktree is exactly the
//! thing that stopped existing. The list is also not part of the entity
//! tree: half of it is derived from the agent CLI's own transcript store,
//! which the daemon does not watch, so it is fetched when the user opens it
//! (`ListOrphanedSessions`) instead of streamed as deltas.

use crate::app::{clamp_selection, window_start, App, Focus, Overlay};
use crate::text_input::TextInput;
use crate::theme::Theme;
use crate::ui::{
    below_first_row, empty_list_row, fmt_mem, render_modal_frame, row_rect, search_line, truncate,
    NO_MATCHES,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use nebula_core::{ClientRequest, OrphanedSession, WorktreeId};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::Frame;

const ORPHANS_SIZE: (u16, u16) = (86, 20);

/// The ORPHANED SESSIONS list. `sessions` is the daemon's answer, in its
/// order (newest first); `matches` indexes into it through the fuzzy query.
#[derive(Debug, Clone)]
pub struct OrphansView {
    pub sessions: Vec<OrphanedSession>,
    /// Always-live fuzzy filter over "branch name".
    pub query: TextInput,
    /// Indices into `sessions`, in match order.
    pub matches: Vec<usize>,
    /// Cursor into `matches`.
    pub selected: usize,
    /// The WORKTREE a resume lands in: whichever one was selected when the
    /// list was opened, so the conversation comes back where the user is
    /// working rather than somewhere they have to navigate to.
    pub target: WorktreeId,
    /// False until the daemon's answer lands, so an empty list reads as
    /// "still asking" rather than "nothing to show".
    pub loaded: bool,
    /// Whole modal rect, written back during draw so clicks outside close.
    pub area: Rect,
    /// Screen rect of the rows, written back during draw for hit-testing.
    pub list_area: Rect,
}

impl OrphansView {
    fn new(target: WorktreeId) -> Self {
        Self {
            sessions: Vec::new(),
            query: TextInput::new(),
            matches: Vec::new(),
            selected: 0,
            target,
            loaded: false,
            area: Rect::default(),
            list_area: Rect::default(),
        }
    }

    /// Re-rank against the current query, keeping the cursor in range.
    fn apply_filter(&mut self) {
        let haystack: Vec<String> = self
            .sessions
            .iter()
            .map(|o| format!("{} {}", o.branch, o.name))
            .collect();
        self.matches = crate::fuzzy::rank(self.query.as_str(), haystack.iter().map(String::as_str))
            .into_iter()
            .map(|(i, _)| i)
            .collect();
        self.selected = clamp_selection(self.selected as i64, self.matches.len());
    }

    pub fn window_start(&self, height: usize) -> usize {
        window_start(self.selected, height)
    }

    fn selected_session(&self) -> Option<&OrphanedSession> {
        self.matches
            .get(self.selected)
            .and_then(|i| self.sessions.get(*i))
    }
}

/// Open the list for the selected worktree's project. Needs a worktree
/// rather than just a project: it is both the scope of the question and
/// where a resume will land.
pub(crate) fn open(app: &mut App, out: &mut Vec<ClientRequest>) {
    let Some(worktree) = app.selected_worktree().cloned() else {
        app.flash = Some("orphaned sessions: select a worktree first".into());
        return;
    };
    app.overlay = Some(Overlay::Orphans(OrphansView::new(worktree.id)));
    let project = worktree.project_id.clone();
    crate::event_loop::send(app, out, |req_id| ClientRequest::ListOrphanedSessions {
        req_id,
        project,
    });
}

/// The daemon's answer. Ignored unless the list is still open — the user
/// may have closed it while the scan ran.
pub(crate) fn receive(app: &mut App, sessions: Vec<OrphanedSession>) {
    if let Some(Overlay::Orphans(view)) = &mut app.overlay {
        view.sessions = sessions;
        view.loaded = true;
        view.apply_filter();
        app.dirty = true;
    }
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent, out: &mut Vec<ClientRequest>) {
    let Some(Overlay::Orphans(view)) = &mut app.overlay else {
        return;
    };
    match key.code {
        KeyCode::Esc => {
            app.overlay = None;
        }
        KeyCode::Up => view.selected = view.selected.saturating_sub(1),
        KeyCode::Down => {
            view.selected = clamp_selection(view.selected as i64 + 1, view.matches.len())
        }
        KeyCode::Enter => resume_selected(app, out),
        // Ctrl+U clears the query, the one editing shortcut worth having
        // beyond what TextInput already handles.
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            view.query.clear();
            view.apply_filter();
        }
        _ => {
            if view.query.handle_key(&key).changed() {
                view.apply_filter();
            }
        }
    }
    app.dirty = true;
}

/// Ask the daemon to bring the selected conversation back. The row it
/// creates arrives as an ordinary agent upsert, and the Ack attaches to it.
fn resume_selected(app: &mut App, out: &mut Vec<ClientRequest>) {
    let Some(Overlay::Orphans(view)) = &app.overlay else {
        return;
    };
    let Some(session) = view.selected_session() else {
        return;
    };
    let session_id = session.session_id.clone();
    let worktree = view.target.clone();
    app.overlay = None;
    app.focus = Focus::Sessions;
    crate::event_loop::send_with(
        app,
        out,
        crate::app::PendingIntent::AttachCreated,
        |req_id| ClientRequest::ResumeOrphanedSession {
            req_id,
            session_id,
            worktree,
        },
    );
}

/// The wheel moves the cursor, a click on a row resumes it (rows are
/// actions here, like the HOSTS picker's), a click outside closes, and
/// everything else is swallowed so the sidebar behind cannot be reached
/// through an open modal.
pub(crate) fn handle_mouse(
    app: &mut App,
    mouse: MouseEvent,
    mouse_pos: Position,
    out: &mut Vec<ClientRequest>,
) {
    let Some(Overlay::Orphans(view)) = &mut app.overlay else {
        return;
    };
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            view.selected = clamp_selection(view.selected as i64 - 1, view.matches.len());
        }
        MouseEventKind::ScrollDown => {
            view.selected = clamp_selection(view.selected as i64 + 1, view.matches.len());
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let list = view.list_area;
            if list.contains(mouse_pos) {
                let start = view.window_start(list.height as usize);
                let index = start + (mouse.row - list.y) as usize;
                if index < view.matches.len() {
                    view.selected = index;
                    resume_selected(app, out);
                }
            } else if !view.area.contains(mouse_pos) {
                app.overlay = None;
            }
        }
        _ => {}
    }
    app.dirty = true;
}

pub(crate) fn draw(f: &mut Frame, app: &mut App, view: &OrphansView, th: Theme) {
    let area = crate::ui::centered_rect(f.area(), ORPHANS_SIZE.0, ORPHANS_SIZE.1);
    f.render_widget(Clear, area);
    let title = if view.query.as_str().is_empty() {
        format!(" Orphaned sessions ({}) ", view.sessions.len())
    } else {
        format!(
            " Orphaned sessions ({}/{}) ",
            view.matches.len(),
            view.sessions.len()
        )
    };
    let inner = render_modal_frame(f, area, title, th);

    if let Some(query_area) = row_rect(inner, 0) {
        let line = search_line(&view.query, "type to search…", query_area, th);
        f.render_widget(ratatui::widgets::Paragraph::new(line), query_area);
    }
    let list_inner = below_first_row(inner);

    if view.matches.is_empty() {
        let note = if !view.loaded {
            "looking…"
        } else if view.sessions.is_empty() {
            "no orphaned sessions — deleting a worktree keeps its conversations here"
        } else {
            NO_MATCHES
        };
        empty_list_row(f, list_inner, note, th);
    }

    let start = view.window_start(list_inner.height as usize);
    for (row, (rank, i)) in view.matches.iter().enumerate().skip(start).enumerate() {
        let Some(row_area) = row_rect(list_inner, row) else {
            break;
        };
        let Some(session) = view.sessions.get(*i) else {
            continue;
        };
        let budget = (list_inner.width as usize).saturating_sub(2);
        // "⊘ branch · name" on the left, "12d ago  626 KB" pinned right.
        // The glyph is the ARCHIVED group's, and means the same thing here:
        // a row with no process behind it.
        let right = right_column(session);
        let right_w = right.chars().count();
        let text_budget = budget.saturating_sub(right_w + 2);
        let left = if session.branch.is_empty() {
            session.name.clone()
        } else if session.branch == session.name {
            session.branch.clone()
        } else {
            format!("{} · {}", session.branch, session.name)
        };
        let left = truncate(&left, text_budget.saturating_sub(2));
        let mut spans = vec![
            Span::styled("⊘ ", Style::default().fg(th.dim)),
            Span::raw(left.clone()),
        ];
        let used = left.chars().count() + 2;
        if used + right_w < budget {
            spans.push(Span::raw(" ".repeat(budget - used - right_w)));
            spans.push(Span::styled(right, Style::default().fg(th.dim)));
        }
        crate::ui::render_row(f, row_area, spans, rank == view.selected, true, th);
    }

    // Draw works on a clone: hand the rects and the clamped cursor back.
    if let Some(Overlay::Orphans(v)) = &mut app.overlay {
        v.area = area;
        v.list_area = list_inner;
    }
}

/// "12d ago  626 KB", with the size dropped when the conversation is known
/// only from the store — no transcript found means no size to claim, and a
/// "0 B" there would read as an empty conversation rather than an unknown.
fn right_column(session: &OrphanedSession) -> String {
    let age = crate::hosts::ago_label(crate::hosts::now_ms() - session.orphaned_at);
    match session.transcript_bytes {
        Some(bytes) => format!("{age}  {}", fmt_mem(bytes)),
        None => age,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebula_core::{AgentKind, ProjectId};
    use std::path::PathBuf;

    fn session(id: &str, branch: &str, name: &str, bytes: Option<u64>) -> OrphanedSession {
        OrphanedSession {
            session_id: id.into(),
            project_id: ProjectId("p".into()),
            kind: AgentKind::Claude,
            name: name.into(),
            branch: branch.into(),
            worktree_path: PathBuf::from("/gone"),
            created_at: 0,
            orphaned_at: 0,
            transcript_bytes: bytes,
        }
    }

    fn view_with(sessions: Vec<OrphanedSession>) -> OrphansView {
        let mut view = OrphansView::new(WorktreeId("w".into()));
        view.sessions = sessions;
        view.loaded = true;
        view.apply_filter();
        view
    }

    /// The filter reads both halves of the row, because the branch is what
    /// the user remembers about a deleted worktree and the name is what
    /// they remember about the conversation.
    #[test]
    fn the_query_matches_the_branch_and_the_name() {
        let mut view = view_with(vec![
            session("a", "features", "hook-status", None),
            session("b", "slot-ports", "review-flow", None),
        ]);
        assert_eq!(view.matches.len(), 2, "an empty query keeps everything");

        view.query.set_text("feat");
        view.apply_filter();
        assert_eq!(view.selected_session().unwrap().session_id, "a");

        view.query.set_text("review");
        view.apply_filter();
        assert_eq!(view.selected_session().unwrap().session_id, "b");
    }

    /// Narrowing the list under a cursor that was further down must not
    /// leave the cursor pointing past the end.
    #[test]
    fn filtering_pulls_the_cursor_back_into_range() {
        let mut view = view_with(vec![
            session("a", "one", "x", None),
            session("b", "two", "y", None),
            session("c", "three", "z", None),
        ]);
        view.selected = 2;

        view.query.set_text("one");
        view.apply_filter();

        assert_eq!(view.matches.len(), 1);
        assert_eq!(view.selected, 0);
        assert_eq!(view.selected_session().unwrap().session_id, "a");
    }

    /// A conversation the store remembers but whose transcript is gone has
    /// no size to show — and must not claim one.
    #[test]
    fn a_row_without_a_transcript_shows_no_size() {
        let with = right_column(&session("a", "b", "n", Some(4096)));
        let without = right_column(&session("a", "b", "n", None));
        assert!(with.contains("KB"), "{with}");
        assert!(!without.contains('B'), "{without}");
    }
}
