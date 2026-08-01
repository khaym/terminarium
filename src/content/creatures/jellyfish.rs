//! The jellyfish: the lagoon's small tier, and the first tenant that does not
//! swim like a fish. It wears all three of `Manner`'s modifiers at once — a
//! bell that pulses between two braille frames, tentacles hanging on a second
//! row, and a slow vertical drift over its lane — so what tells it from a fish
//! is how it carries itself, not only its glyphs. Three cells wide by two rows
//! (glyphgen.py `JELLY_G6`), the width a pulse pair needs to stay legible
//! (work/render-observations.md, 2026-07-25).

use ratatui::style::Color;

use super::{Drift, Look, Manner, Pulse, SwimmerDef};

/// A pale lavender, clear of the plankton's cyan (122), the whale's gray (152)
/// and both fish oranges (215 / 209) — readable on all four waters.
const JELLYFISH: Color = Color::Indexed(183);

pub const DEF: SwimmerDef = SwimmerDef {
    // A jellyfish has no face to turn: both headings wear the open bell, and
    // the drift is what the eye follows instead of the heading.
    right: "⢞⢛⢳",
    left: "⢞⢛⢳",
    // Slower than any fish — it is carried rather than swimming.
    slowdown: 4,
    // Tight to its reef, like the other small tenants.
    radius: 4,
    reef_bias: true,
    color: JELLYFISH,
    accent: None,
    manner: Manner {
        // Two rows either side of its lane, one full rise and fall every 40
        // frames (~8s at 5 fps): a swell, not a bob.
        drift: Drift {
            amplitude: 2,
            period: 40,
        },
        // Tentacles trailing under the open bell.
        under: "⠁⠃⠃",
        // The contracted bell, held ~0.8s at a time, so the pulse reads as a
        // pump rather than a flicker.
        pulse: Some(Pulse {
            look: Look {
                right: "⠰⡛⡆",
                left: "⠰⡛⡆",
                under: "⠈⠊",
            },
            period: 4,
        }),
    },
};
