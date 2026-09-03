//! The PRs panel: the selected project's open pull requests, one pill per
//! row. Sibling of the Worktrees panel it was split out of; the row look
//! (`↗`, status pair, title, draft badge) is the one the Sessions panel's
//! link rows use.

use super::{
    draw_column, hint_line, panel_title, pill_hit_height, pr_status_spans, render_pill,
    rows_rect_at, truncate, PILL_H, STATUS_W,
};
use crate::app::{App, Focus, HitTarget};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub(super) fn draw_prs(f: &mut Frame, app: &mut App, area: Rect) {
    let th = app.theme;
    let focused = app.focus == Focus::Prs;
    let prs = app.visible_open_prs();
    // A list cut off at the fetch cap says so rather than passing itself
    // off as the whole set.
    let base = if prs.len() >= crate::pull_request::LIST_LIMIT {
        "OPEN PRS+"
    } else {
        "OPEN PRS"
    };
    let title = panel_title(app, Focus::Prs, base);
    let count = Some(prs.len()).filter(|n| *n > 0);
    let inner = draw_column(f, area, &title, count, focused, th);
    if prs.is_empty() {
        if app.selected_project().is_some() {
            f.render_widget(
                Paragraph::new(hint_line(&[("", "no open pull requests")], th)),
                inner,
            );
        }
        app.hits.push((inner, HitTarget::PanelBg(Focus::Prs)));
        return;
    }

    // ---- resolve the scroll offset ----
    // Same contract as the Worktrees panel: the cursor pulls the viewport
    // only on the frames where it moved, the wheel is free otherwise.
    let stride = PILL_H as usize;
    let view_h = inner.height as usize;
    let content_h = (prs.len() - 1) * stride + stride + 1;
    let anchor = (app.sel_project, app.sel_pr);
    if app.prs_anchor != Some(anchor) {
        app.prs_anchor = Some(anchor);
        let top = app.sel_pr * stride;
        let bottom = top + stride + 1;
        if top < app.prs_scroll {
            app.prs_scroll = top;
        } else if bottom > app.prs_scroll + view_h {
            app.prs_scroll = bottom - view_h;
        }
    }
    app.prs_scroll = app.prs_scroll.min(content_h.saturating_sub(view_h));
    let scroll = app.prs_scroll as isize;

    // ---- draw ----
    for (i, pr) in prs.iter().enumerate() {
        let top = i * stride;
        let y = top as isize - scroll;
        if y >= view_h as isize {
            break;
        }
        let next = (i + 1 < prs.len()).then(|| top + stride);
        let hit_h = pill_hit_height(top, next);
        // Only a draft earns a badge: the title already says these are
        // open, and in a narrow column the width is better spent on the
        // title.
        let badge = pr.is_draft.then(|| format!(" {}", pr.badge()));
        let badge_len = badge.as_ref().map_or(0, |b| b.chars().count());
        // Two status cells sit between the arrow and the number: reviewers
        // first, then CI, in the order you'd ask about them. They cost the
        // title `STATUS_W` columns, so they're only there when this project
        // has something to put in them.
        let status = pr_status_spans(pr, th);
        let status_w = if status.is_empty() { 0 } else { STATUS_W };
        let label_max = (inner.width as usize)
            .saturating_sub(3)
            .saturating_sub(status_w)
            .saturating_sub(badge_len);
        let mut spans = vec![Span::styled("↗ ", Style::default().fg(th.accent))];
        spans.extend(status);
        spans.push(Span::styled(
            truncate(&pr.label(), label_max),
            Style::default().fg(th.muted),
        ));
        if let Some(badge) = badge {
            spans.push(Span::styled(badge, Style::default().fg(th.dim)));
        }
        render_pill(f, inner, y, spans, i == app.sel_pr, focused, th);
        if let Some(hit) = rows_rect_at(inner, y, hit_h) {
            app.hits.push((hit, HitTarget::Pr(i)));
        }
    }
    app.hits.push((inner, HitTarget::PanelBg(Focus::Prs)));
}
