//! Seagrass: the lagoon's base layer, a bright ribbon of shallow water. Its two
//! sway frames shift the same braille bar from one side of its cell to the
//! other — the third use of the frond sway (after the kelp's alternating shape
//! and the lantern moss's twinkle), and the one that reads as a ribbon leaning
//! in the current.

use ratatui::style::Color;

use super::FrondDef;

/// A yellow-green of the shallows, read apart from the kelp forest's deeper
/// green (70) in a side-by-side stack.
const SEAGRASS: Color = Color::Indexed(112);

pub const DEF: FrondDef = FrondDef {
    fronds: ["⡇", "⢸"],
    color: SEAGRASS,
};
