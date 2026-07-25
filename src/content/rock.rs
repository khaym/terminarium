//! Plain rock: the founding reef, placeable from score 0 and the look every
//! run opens on.

use ratatui::style::Color;

use super::{AlgaeVariant, BigVariant, ReefDef, RockVariant, BIG_FISH};
use crate::engine::{RockKind, MICRO};

/// Base algae tint — the founding sea's green.
const ALGAE: Color = Color::Indexed(35);
/// Rock body — a neutral reef gray in the indexed ramp.
const ROCK: Color = Color::Indexed(245);

pub const DEF: ReefDef = ReefDef {
    economy: RockKind {
        name: "rock",
        cost: 1,
        unlock: 0,
        output: 3 * MICRO,
        capacity: [4, 3, 2, 1],
    },
    // the founding sea's sparse fronds
    algae: AlgaeVariant {
        fronds: ["(", ")"],
        color: ALGAE,
    },
    // a low block mound
    rock: RockVariant {
        body: ["▄", "█", "▄"],
        color: ROCK,
    },
    big: BigVariant {
        right: "><)))>",
        left: "<(((><",
        slowdown: 2,
        color: BIG_FISH,
    },
};
