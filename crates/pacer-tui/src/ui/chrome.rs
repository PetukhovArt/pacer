//! Panel chrome the `bold_names` setting reaches: the weight every panel
//! header and the project names are drawn at. One place, so a setting
//! called "Bold names" can't leave one header bold behind the others.

use ratatui::style::{Modifier, Style};

use crate::theme::Theme;

/// BOLD or nothing, for the text `bold_names` covers.
pub(crate) fn weight(bold: bool) -> Modifier {
    if bold {
        Modifier::BOLD
    } else {
        Modifier::empty()
    }
}

/// What every panel header needs to draw itself: whether it has focus,
/// the settable weight and the palette. The three always travel together,
/// so they travel as one.
#[derive(Clone, Copy)]
pub(crate) struct Chrome {
    pub focused: bool,
    pub bold: bool,
    pub th: Theme,
}

impl Chrome {
    pub fn new(focused: bool, bold: bool, th: Theme) -> Self {
        Self { focused, bold, th }
    }

    /// A panel header's style: accent in the focused panel, muted
    /// elsewhere — the focus signal every header carries — at the
    /// settable weight.
    pub fn header_style(self) -> Style {
        Style::default()
            .fg(if self.focused {
                self.th.accent
            } else {
                self.th.muted
            })
            .add_modifier(weight(self.bold))
    }
}
