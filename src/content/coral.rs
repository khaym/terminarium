//! Coral: the second reef, the first wall's reward — denser housing for the
//! middle of the chain, at twice the rock's budget.

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{RockKind, MICRO};

/// Coral rock body — a warm rose, well clear of the orange fish tones.
const CORAL: Color = Color::Indexed(174);

pub const DEF: ReefDef = ReefDef {
    economy: RockKind {
        name: "coral",
        cost: 2,
        unlock: 12_000 * MICRO,
        output: 12 * MICRO,
        capacity: [2, 6, 5, 3],
    },
    // branches spreading up from a stem
    rock: RockVariant {
        body: ["╱", "█", "╲"],
        color: CORAL,
    },
    // a denser base layer; the swimmers above it are the plain ones, shared with
    // the founding sea rather than restated here
    algae: &creatures::teal_fronds::DEF,
    plankton: &creatures::plankton::DEF,
    small: &creatures::small_fish::DEF,
    big: &creatures::big_fish::DEF,
};
