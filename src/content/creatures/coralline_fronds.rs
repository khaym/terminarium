//! Coralline fronds: the teal kind's shape in a calcareous red, so a reef whose
//! rock body is borrowed still reads by its base layer alone.

use ratatui::style::Color;

use super::FrondDef;

/// A calcareous red, clear of every other base layer's green.
const FROND: Color = Color::Indexed(131);

pub const DEF: FrondDef = FrondDef {
    fronds: ["{", "}"],
    color: FROND,
};
