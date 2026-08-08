//! Kelp: the third reef, an algae-heavy forest — it houses the base species
//! many over, and its apex is a dugong rather than a fish.

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{ReefKind, MICRO};

/// Kelp holdfast — a muted olive anchoring the frond forest to the floor.
const KELP_HOLDFAST: Color = Color::Indexed(58);

pub const DEF: ReefDef = ReefDef {
    economy: ReefKind {
        name: "kelp",
        cost: 3,
        unlock: 30_000 * MICRO,
        output: 23 * MICRO,
        capacity: [9, 5, 2, 1],
    },
    // a solid, wide holdfast
    rock: RockVariant {
        body: ["▙", "█", "▟"],
        color: KELP_HOLDFAST,
    },
    // the forest of blades is this reef's signature, and its apex is a grazer
    algae: &creatures::kelp_blades::DEF,
    plankton: &creatures::plankton::DEF,
    small: &creatures::small_fish::DEF,
    big: &creatures::dugong::DEF,
};
