//! The sea turtle: the lagoon's apex, flippers out and shell plates showing.
//! Six cells and the dugong's unhurried beat — the two are grazers of the same
//! size, so the diamonds of its shell and its green are what tell them apart.

use ratatui::style::Color;

use super::{Manner, SwimmerDef};

/// A sea green, clear of the dugong's tan and in tune with the jellyfish
/// lavender it shares the lagoon with.
const TURTLE: Color = Color::Indexed(71);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "=(<>)o",
    left: "o(<>)=",
    slowdown: 3,
    radius: 10,
    reef_bias: false,
    color: TURTLE,
    accent: None,
    manner: Manner::PLAIN,
};
