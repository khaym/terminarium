//! The big fish: the plain apex individual, worn by every reef that has no apex
//! of its own.

use ratatui::style::Color;

use super::{Dash, EarlyTurn, Manner, SwimmerDef};

/// A warm orange, clear of the small fish's lighter tone.
const BIG_FISH: Color = Color::Indexed(209);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "><)))>",
    left: "<(((><",
    slowdown: 2,
    radius: 10,
    reef_bias: false,
    color: BIG_FISH,
    accent: None,
    manner: Manner {
        // An apex individual laps its wider window in 40 steps at half a step a
        // frame (~16s at 5 fps), so even 1-in-12 puts its quirks some three
        // minutes apart — twice the small fish's spacing, which suits the one
        // sprite the eye already follows.
        dash: Some(Dash {
            span: 6,
            rarity: 12,
        }),
        // 6 of its 20 columns: the same share of the window the small fish
        // gives up, on a beat the eye can follow.
        early_turn: Some(EarlyTurn {
            short: 6,
            rarity: 12,
        }),
        ..Manner::PLAIN
    },
};
