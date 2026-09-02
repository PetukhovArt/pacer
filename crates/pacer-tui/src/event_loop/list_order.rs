//! What order the sidebar columns stand in, and what the cursors do when
//! that order changes. `⇧S` sorts the column the cursor is in — each of
//! Projects, Worktrees and Sessions owns its own `sort_*` SETTING — and
//! every reorder here hands the cursors back to the rows they were on.
//! `event_loop.rs` dispatches the key; this module owns the ordering.

use super::save_config;
use crate::app::{App, SortModes};

/// ⇧S: advance the focused column's sort one step, through the config so
/// it survives a restart. Panels with no list of their own say so rather
/// than sorting something the user isn't looking at.
pub(super) fn cycle_focused_sort(app: &mut App) {
    let mut cfg = crate::config::Config::load();
    let Some(word) = cfg.sort_word_mut(app.focus) else {
        app.flash = Some("sort works in the sidebar lists".into());
        return;
    };
    *word = crate::config::cycle_choice(word, crate::config::LIST_SORTS, 1).into();
    let word = word.clone();
    save_config(app, &cfg);
    apply_sort(app, cfg.sort_modes());
    app.flash = Some(format!("{} sort: {word}", super::panel_name(app.focus)));
}

/// Put the columns in `sort` order and keep every cursor on the row it was
/// resting on. The cursors are indices into lists whose rows are about to
/// move, so without the anchors a sort change silently selects a different
/// project — and the panes below follow the selection.
pub(super) fn apply_sort(app: &mut App, sort: SortModes) {
    let anchors = app.cursor_anchors();
    app.sort = sort;
    app.restore_cursors(anchors);
    app.dirty = true;
}
