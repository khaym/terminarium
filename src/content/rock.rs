//! Plain rock: the founding reef, placeable from score 0 and the look every
//! run opens on.

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{RockKind, MICRO};

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
    // a low block mound
    rock: RockVariant {
        body: ["▄", "█", "▄"],
        color: ROCK,
    },
    // the plain tenants; only the sparse fronds are the founding sea's own
    algae: &creatures::sparse_fronds::DEF,
    plankton: &creatures::plankton::DEF,
    small: &creatures::small_fish::DEF,
    big: &creatures::big_fish::DEF,
};
