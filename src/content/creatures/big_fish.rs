//! The big fish: the plain apex individual, worn by every reef that has no apex
//! of its own.

use ratatui::style::Color;

use super::SwimmerDef;

/// A warm orange, clear of the small fish's lighter tone.
const BIG_FISH: Color = Color::Indexed(209);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "><)))>",
    left: "<(((><",
    slowdown: 2,
    radius: 10,
    reef_bias: false,
    color: BIG_FISH,
    accent: None,
};
