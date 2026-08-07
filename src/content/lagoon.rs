//! Lagoon: the sixth reef, a tropical shallow of pale sand where jellyfish
//! drift over the seagrass and a turtle sweeps above them. Its housing leans on
//! the small tier, so a filled lagoon is a lagoon full of drifting bells — the
//! first sea whose life moves vertically rather than back and forth.

use ratatui::style::Color;

use super::{creatures, ReefDef, RockVariant};
use crate::engine::{ReefKind, MICRO};

/// Lagoon sand — a bright shoal color, the lightest rock body of any reef.
const LAGOON_SAND: Color = Color::Indexed(187);

pub const DEF: ReefDef = ReefDef {
    economy: ReefKind {
        name: "lagoon",
        cost: 2,
        unlock: 60_000 * MICRO,
        output: 12 * MICRO,
        capacity: [4, 4, 6, 2],
    },
    // a low sandbar: the flattest body of the six, so the reef reads as shallow
    rock: RockVariant {
        body: ["▂", "▄", "▂"],
        color: LAGOON_SAND,
    },
    // the drifting jellyfish and the turtle over them are this reef's signature,
    // rooted in seagrass; the plankton tier is the shared one
    algae: &creatures::seagrass::DEF,
    plankton: &creatures::plankton::DEF,
    small: &creatures::jellyfish::DEF,
    big: &creatures::turtle::DEF,
};
