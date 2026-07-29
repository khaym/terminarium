//! Noctiluca: the fifth reef's plankton, a brighter speck than the shared
//! plankton tier — two braille dots per individual where the shared kind wears
//! one, so the same drift reads as the reef's own glow rather than a borrowed
//! tenant.

use ratatui::style::Color;

use super::DotDef;

/// A cool glow, apart from the lantern moss's warm one — the reef's two lit
/// tenants pair a warm base with a cool drift.
const NOCTILUCA: Color = Color::Indexed(159);

pub const DEF: DotDef = DotDef {
    dots: ["⠃", "⠘", "⠰", "⠉"],
    color: NOCTILUCA,
};
