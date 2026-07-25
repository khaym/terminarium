//! Kelp blades: tall swaying blades — the whole character of a kelp forest.

use ratatui::style::Color;

use super::FrondDef;

/// A taller, brighter forest green.
const BLADE: Color = Color::Indexed(70);

pub const DEF: FrondDef = FrondDef {
    fronds: ["\\", "/"],
    color: BLADE,
};
