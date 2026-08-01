//! The dugong: a rounded sea cow grazing a kelp forest, ambling slower than any
//! fish. Six cells like the big fish, so the patrol clamp treats them alike.

use ratatui::style::Color;

use super::{Manner, SwimmerDef};

/// A warm tan.
const DUGONG: Color = Color::Indexed(180);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "=(__)o",
    left: "o(__)=",
    slowdown: 3,
    radius: 10,
    reef_bias: false,
    color: DUGONG,
    accent: None,
    manner: Manner::PLAIN,
};
