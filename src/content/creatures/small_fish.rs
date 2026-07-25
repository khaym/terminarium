//! The small fish: the third tier, darting in a tight beat close to its reef.

use ratatui::style::Color;

use super::SwimmerDef;

/// A light orange, clear of the big fish's warmer tone.
const FISH: Color = Color::Indexed(215);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "><>",
    left: "<><",
    slowdown: 1,
    radius: 5,
    reef_bias: true,
    color: FISH,
};
