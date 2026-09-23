//! `]` / `[`: step through the sessions waiting on the user (NEEDS
//! FEEDBACK) with no modal, landing the way the `/` palette's Enter does.
//! `event_loop.rs` dispatches the key; this module owns the walk.

use super::{jump_to_target, Landing};
use crate::app::{App, PaletteTarget};
use pacer_core::ClientRequest;

/// Land on the next (`step` 1) or previous (`step` -1) session in
/// [`crate::app::attention_sessions`], wrapping at both ends. From a row
/// off the ring `]` starts at the top and `[` at the bottom, so the two
/// stay each other's reverse. A session in another workspace switches this
/// instance there, as a palette pick would.
pub(super) fn jump_attention(app: &mut App, step: i64, out: &mut Vec<ClientRequest>) {
    let ring = crate::app::attention_sessions(&app.tree);
    if ring.is_empty() {
        app.flash = Some("no session needs you".into());
        return;
    }
    let len = ring.len() as i64;
    let at = app
        .selected_session()
        .and_then(|a| ring.iter().position(|id| *id == a.id));
    let next = match at {
        Some(i) => (i as i64 + step).rem_euclid(len),
        None if step > 0 => 0,
        None => len - 1,
    };
    let landing = Landing::for_enter(crate::config::Config::load().palette_enter_attaches);
    let target = PaletteTarget::Session(ring[next as usize].clone());
    jump_to_target(app, target, landing, out);
}
