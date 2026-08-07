//! Lantern: the fifth reef, the next goal past the budget-5 wall — a small,
//! cheap reef (cost 1) that slots into whatever room a finished budget-5 sea
//! still has. Its rock body stays unlit; the glow is carried by its tenants alone
//! (the twinkling moss, the bright plankton, and the anglerfish's lure).

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{ReefKind, MICRO};

/// Lantern rock body — a mid gray, deliberately unlit; the reef's glow lives in its
/// tenants, not its body.
const LANTERN_ROCK: Color = Color::Indexed(243);

pub const DEF: ReefDef = ReefDef {
    economy: ReefKind {
        name: "lantern",
        cost: 1,
        unlock: 100_000 * MICRO,
        output: 4 * MICRO,
        capacity: [4, 3, 2, 1],
    },
    // a branch: two limbs meeting at a stem
    rock: RockVariant {
        body: ["\\", "Y", "/"],
        color: LANTERN_ROCK,
    },
    // the twinkling moss and the brighter plankton are this reef's own signature;
    // the small tier is the plain shared fish, and the apex is the anglerfish
    algae: &creatures::lantern_moss::DEF,
    plankton: &creatures::noctiluca::DEF,
    small: &creatures::small_fish::DEF,
    big: &creatures::anglerfish::DEF,
};
