//! Teal fronds: a denser base layer, curling where the sparse kind is straight.

use ratatui::style::Color;

use super::FrondDef;

/// A denser teal-green than the founding sea's.
const FROND: Color = Color::Indexed(37);

pub const DEF: FrondDef = FrondDef {
    fronds: ["{", "}"],
    color: FROND,
};
