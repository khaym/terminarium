//! Plankton: the drifting second tier, a single braille dot per individual.

use ratatui::style::Color;

use super::DotDef;

/// A pale cyan that reads against every water background.
const PLANKTON: Color = Color::Indexed(122);

pub const DEF: DotDef = DotDef {
    dots: ["⠁", "⠂", "⠄", "⠈"],
    color: PLANKTON,
};
