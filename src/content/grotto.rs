//! Grotto: the fourth reef, a hollowed stone the founding rock's own gray — its
//! cave mouth and its red base layer are what tell the two apart. It houses the
//! small tier many over, and its apex is a squid rather than a fish.

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{ReefKind, MICRO};

/// Grotto stone — the base rock's gray, shared deliberately: this reef reads by
/// its base layer, not its body (work/render-observations.md).
const GROTTO_STONE: Color = Color::Indexed(245);

pub const DEF: ReefDef = ReefDef {
    economy: ReefKind {
        name: "grotto",
        cost: 3,
        unlock: 40_000 * MICRO,
        output: 12 * MICRO,
        capacity: [2, 3, 7, 2],
    },
    // an arch: the gap under the center glyph is the mouth of the cave
    rock: RockVariant {
        body: ["▛", "▀", "▜"],
        color: GROTTO_STONE,
    },
    // the shrimp crowding the cave's shadow are this reef's signature, and its
    // apex hangs over them
    algae: &creatures::coralline_fronds::DEF,
    plankton: &creatures::plankton::DEF,
    small: &creatures::shrimp::DEF,
    big: &creatures::squid::DEF,
};
