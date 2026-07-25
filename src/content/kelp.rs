//! Kelp: the third reef, an algae-heavy forest — it houses the base species
//! many over, and its apex is a dugong rather than a fish.

use ratatui::style::Color;

use super::{AlgaeVariant, BigVariant, ReefDef, RockVariant};
use crate::engine::{RockKind, MICRO};

/// Kelp holdfast — a muted olive anchoring the frond forest to the floor.
const KELP_HOLDFAST: Color = Color::Indexed(58);
/// Kelp fronds — a taller, brighter forest green; the reef's whole character.
const KELP_ALGAE: Color = Color::Indexed(70);
/// The dugong: the kelp forest's apex grazer, a warm tan sea cow.
const DUGONG: Color = Color::Indexed(180);

pub const DEF: ReefDef = ReefDef {
    economy: RockKind {
        name: "kelp",
        cost: 3,
        unlock: 30_000 * MICRO,
        output: 12 * MICRO,
        capacity: [9, 5, 2, 1],
    },
    // tall swaying blades — the frond forest is this reef's signature
    algae: AlgaeVariant {
        fronds: ["\\", "/"],
        color: KELP_ALGAE,
    },
    // a solid, wide holdfast
    rock: RockVariant {
        body: ["▙", "█", "▟"],
        color: KELP_HOLDFAST,
    },
    // a rounded sea cow, ambling slower than a fish
    big: BigVariant {
        right: "=(__)o",
        left: "o(__)=",
        slowdown: 3,
        color: DUGONG,
    },
};
