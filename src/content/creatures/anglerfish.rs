//! The anglerfish: the fifth reef's apex, and the first swimmer to wear a
//! second color. Its lure — the trailing `*` — glows apart from the rest of
//! its body; a single-color fallback would let the lure read as just one more
//! body cell, so the color difference alone is what tells a reader it is
//! there. Seven cells wide, a size up from every other apex.

use ratatui::style::Color;

use super::SwimmerDef;

/// A deep-water body, clear of every other apex's warmer tone.
const ANGLERFISH: Color = Color::Indexed(102);
/// The lure, lit the same glow as the reef's own moss and plankton — the
/// whole reef reads as one light.
const LURE: Color = Color::Indexed(228);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "><((>^*",
    left: "*^<))><",
    slowdown: 3,
    radius: 8,
    reef_bias: false,
    color: ANGLERFISH,
    // `right`'s 7th cell (index 6, the trailing `*`) is the lure.
    accent: Some((6, LURE)),
};
