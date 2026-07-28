//! Reef kind content: what a rock kind *is*, one file per kind.
//!
//! A kind is an economy row (what it costs, when it unlocks, what it sheds, how
//! much it houses), the look of its own rock body, and the creature it houses at
//! each of the four economy tiers. All of it lives in the same file — `rock.rs`,
//! `coral.rs`, `kelp.rs`, `grotto.rs` — so adding a kind is one new definition
//! file plus its name in the `kinds!` manifest below, with no edit to the engine
//! or the renderer. `Params::default` builds its economy rows from the manifest
//! and the wallpaper reads the look from it, so the two halves of a kind can
//! never drift apart.
//!
//! The tenants themselves are content of their own, one file per creature in
//! `creatures/`. A kind names them by reference, so the creature two kinds share
//! (the plain big fish, over both rock and coral) is one definition read twice.
//!
//! A definition is plain Rust consts, so it is compiled in: nothing to ship
//! beside the binary and nothing to parse (or fail to parse) at startup.
//!
//! Colors follow the same ownership rule as the rest of a definition: the tint a
//! creature wears lives in that creature's file, and the tint of a kind's rock
//! body lives in that kind's. Scenery that is not alive (detritus, sediment, the
//! water itself) is not content — its colors stay in the wallpaper.

use ratatui::style::Color;

use self::creatures::{DotDef, FrondDef, SwimmerDef};
use crate::engine::RockKind;

pub mod creatures;

/// Declares the kind modules and builds the manifest from one list, so
/// registering a kind and placing it in the unlock order cannot drift apart —
/// they are the same edit. Each name is a `<name>.rs` beside this file exposing
/// `pub const DEF: ReefDef`.
macro_rules! kinds {
    ($($kind:ident),+ $(,)?) => {
        $(mod $kind;)+

        /// Every kind the game knows, in unlock order.
        ///
        /// A kind's position here *is* its identity in a save file (`Rock::kind`
        /// is an index into this list, and so into `Params::rock_kinds`), which
        /// makes the order append-only: reordering these entries silently
        /// rewrites what every save already on disk means.
        pub const KINDS: &[&ReefDef] = &[$(&$kind::DEF),+];
    };
}

// The manifest: the one shared line a new kind adds itself to (append only).
kinds!(rock, coral, kelp, grotto);

/// One reef kind, whole: its economy row, its rock body, and its four tenants.
/// The engine reads the economy, the wallpaper reads the body and follows the
/// tenant references, and neither needs to know how many kinds exist.
pub struct ReefDef {
    /// The row this kind contributes to `Params::rock_kinds`.
    pub economy: RockKind,
    /// Look of this kind's rock body.
    pub rock: RockVariant,
    /// The creature this kind houses at each economy tier, in
    /// `State::population` order. References into `creatures`, so a kind picks
    /// its tenants rather than restating them — two kinds hosting the same
    /// creature share the one definition, and retuning it moves both reefs.
    pub algae: &'static FrondDef,
    pub plankton: &'static DotDef,
    pub small: &'static SwimmerDef,
    pub big: &'static SwimmerDef,
}

/// Look of the rock body itself — the one part of a kind's look that is scenery
/// rather than a creature. The glyph tells kinds apart even in the dim placement
/// ghost, where color does not; color carries the difference for a placed reef.
pub struct RockVariant {
    /// Three body glyphs drawn left / center / right of the rock column.
    pub body: [&'static str; 3],
    pub color: Color,
}

/// The definition a kind index names. An index past the end wraps rather than
/// panicking: placement and save loading both reject an out-of-range kind, so
/// the wrap only ever catches a state real play cannot reach.
pub fn def(kind: usize) -> &'static ReefDef {
    KINDS[kind % KINDS.len()]
}

/// The economy rows of every kind, in manifest order — what `Params::default`
/// fills `rock_kinds` with.
pub fn rock_kinds() -> Vec<RockKind> {
    KINDS.iter().map(|def| def.economy.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest order is the save format: a rock stores its kind as an
    /// index, so kinds may be appended but the ones already shipped can never
    /// move — a swap would silently turn every saved coral into a kelp. Pinning
    /// the shipped kinds as a *prefix* states exactly that: appending a kind
    /// stays green, reordering or dropping one goes red.
    #[test]
    fn shipped_kinds_keep_their_save_identity() {
        let names: Vec<&str> = KINDS.iter().map(|def| def.economy.name).collect();
        assert!(
            names.starts_with(&["rock", "coral", "kelp"]),
            "kind indices are save identities and must stay put, got {names:?}"
        );
    }
}
