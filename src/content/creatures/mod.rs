//! Creature content: what one living thing *is*, one file per creature.
//!
//! A creature is a look and a motion — the glyphs it wears, its color, and the
//! pace it moves at. A reef kind names the creature it houses at each economy
//! tier by reference (`ReefDef`'s four species fields), so a creature two reefs
//! share is one definition read twice rather than two copies free to drift
//! apart. Adding a creature is one new file here, its `mod` line below, and the
//! one reference in the reef that hosts it.
//!
//! Three archetypes cover the four tiers: fronds rooted to a rock (algae),
//! drifting dots (plankton), and swimmers on a patrol (the fish and the dugong).
//! The wallpaper holds one drawing routine per archetype, so a new creature of
//! an existing archetype is data alone — no renderer edit.
//!
//! The order of the modules below carries no meaning. Unlike `KINDS`, whose
//! order *is* a kind's save identity, a creature is never named by a save (a
//! save stores only its host reef's kind index), so this list is free to be
//! sorted however reads best.
//!
//! Colors follow the same ownership rule as the rest of a definition: the tint a
//! creature wears lives in that creature's file.

use ratatui::style::Color;

pub mod big_fish;
pub mod dugong;
pub mod kelp_blades;
pub mod plankton;
pub mod small_fish;
pub mod sparse_fronds;
pub mod teal_fronds;

/// A frond rooted to its host rock: the base species (algae) of a reef. Every
/// reef's base layer looks different, so a reef reads by its greenery alone.
pub struct FrondDef {
    /// Two sway frames of the frond glyph.
    pub fronds: [&'static str; 2],
    pub color: Color,
}

/// A speck drifting near its host rock: the plankton tier, one glyph per
/// individual.
pub struct DotDef {
    /// Four dot glyphs, cycled across a colony so neighbours differ.
    pub dots: [&'static str; 4],
    pub color: Color,
}

/// A swimmer on a bounded patrol around its host rock — the fish tiers and the
/// dugong. Keyed only by the host rock's kind, so it stays a pure function of
/// (state, frame) like every other sprite.
pub struct SwimmerDef {
    pub right: &'static str,
    pub left: &'static str,
    /// Frames per column step; higher is slower (the dugong ambles).
    pub slowdown: u64,
    /// Patrol radius, in cells either side of the host rock. Small tenants stay
    /// tight to the reef; an apex swimmer sweeps a wider, statelier beat.
    pub radius: i64,
    /// Folds the swimmer's lane into the pane's lower half, keeping it down near
    /// the reef. An apex swimmer ranges over the full height instead.
    pub reef_bias: bool,
    pub color: Color,
}
