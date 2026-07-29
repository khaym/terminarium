//! The squid: an apex that glides the full height of the pane on the widest beat
//! of any tenant, tentacles trailing behind whichever way it faces.

use ratatui::style::Color;

use super::SwimmerDef;

/// A cool blue-gray, clear of the jellyfish lavender it swims nearest in tone.
const SQUID: Color = Color::Indexed(110);

pub const DEF: SwimmerDef = SwimmerDef {
    right: "}}=:>",
    left: "<:={{",
    slowdown: 2,
    radius: 12,
    reef_bias: false,
    color: SQUID,
    accent: None,
};
