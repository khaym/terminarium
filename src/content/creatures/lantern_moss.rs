//! Lantern moss: the fifth reef's base layer, the first frond whose two sway
//! frames read as a twinkle rather than a wave — the sway mechanism is the one
//! every other frond already uses, only the glyph pair and the color it wears
//! turn it into a flicker instead of a drift.

use ratatui::style::Color;

use super::FrondDef;

/// A warm glow, clear of every green base layer that came before it — the
/// reef's light is carried by its tenants, not its rock.
const LANTERN_MOSS: Color = Color::Indexed(228);

pub const DEF: FrondDef = FrondDef {
    fronds: ["*", "+"],
    color: LANTERN_MOSS,
};
