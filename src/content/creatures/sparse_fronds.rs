//! Sparse fronds: the thin, widely spaced base layer of the founding sea.

use ratatui::style::Color;

use super::FrondDef;

/// The founding sea's green.
const FROND: Color = Color::Indexed(35);

pub const DEF: FrondDef = FrondDef {
    fronds: ["(", ")"],
    color: FROND,
};
