//! The small fish: the third tier, darting in a tight beat close to its reef.

use ratatui::style::Color;

use super::{Dash, EarlyTurn, Manner, SwimmerDef};

/// A light orange, clear of the big fish's warmer tone.
const FISH: Color = Color::Indexed(215);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "><>",
    left: "<><",
    slowdown: 1,
    radius: 5,
    reef_bias: true,
    color: FISH,
    accent: None,
    manner: Manner {
        // A small fish laps its window in 20 steps (~4s at 5 fps), so one lap in
        // 24 is a quirk roughly every minute and a half — often enough that a
        // watched pane shows one, rare enough that it stays a surprise.
        dash: Some(Dash {
            span: 5,
            rarity: 24,
        }),
        // Turning back 3 of its 10 columns: the fish is plainly heading home
        // early, without leaving the neighbourhood of its reef.
        early_turn: Some(EarlyTurn {
            short: 3,
            rarity: 24,
        }),
        ..Manner::PLAIN
    },
};
