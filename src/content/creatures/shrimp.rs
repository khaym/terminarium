//! The shrimp: a swarm that loiters in the shadow of its reef, patrolling
//! tighter than any fish. Two cells, so what identifies it is how it crowds the
//! reef and its color — not its outline.

use ratatui::style::Color;

use super::{Manner, SwimmerDef};

/// A pale shell pink, clear of the fish oranges.
const SHRIMP: Color = Color::Indexed(217);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "~>",
    left: "<~",
    slowdown: 1,
    radius: 3,
    reef_bias: true,
    color: SHRIMP,
    accent: None,
    manner: Manner::PLAIN,
};
