//! Coral: the second reef, the first wall's reward — denser housing for the
//! middle of the chain, at twice the rock's budget.

use ratatui::style::Color;

use super::{AlgaeVariant, BigVariant, ReefDef, RockVariant, BIG_FISH};
use crate::engine::{RockKind, MICRO};

/// Coral rock body — a warm rose, well clear of the orange fish tones.
const CORAL: Color = Color::Indexed(174);
/// Base algae tint for the coral reef — a denser teal-green than plain rock.
const CORAL_ALGAE: Color = Color::Indexed(37);

pub const DEF: ReefDef = ReefDef {
    economy: RockKind {
        name: "coral",
        cost: 2,
        unlock: 12_000 * MICRO,
        output: 12 * MICRO,
        capacity: [2, 6, 5, 3],
    },
    // a denser, teal-tinged base layer
    algae: AlgaeVariant {
        fronds: ["{", "}"],
        color: CORAL_ALGAE,
    },
    // branches spreading up from a stem
    rock: RockVariant {
        body: ["╱", "█", "╲"],
        color: CORAL,
    },
    big: BigVariant {
        right: "><)))>",
        left: "<(((><",
        slowdown: 2,
        color: BIG_FISH,
    },
};
