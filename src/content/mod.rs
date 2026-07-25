//! Reef kind content: what a rock kind *is*, one file per kind.
//!
//! A kind is an economy row (what it costs, when it unlocks, what it sheds, who
//! it houses) plus the sprite variants that give it a look of its own. Both
//! halves live in the same file — `rock.rs`, `coral.rs`, `kelp.rs` — so adding a
//! kind is one new definition file plus its name in the `kinds!` manifest below,
//! with no edit to the engine or the renderer. `Params::default` builds its
//! economy rows from the manifest and the wallpaper reads its variants from it,
//! so the two halves of a kind can never drift apart.
//!
//! A definition is plain Rust consts, so it is compiled in: nothing to ship
//! beside the binary and nothing to parse (or fail to parse) at startup.
//!
//! Colors follow the same ownership rule as the rest of a definition: a tint
//! only one kind wears lives in that kind's file, and a tint shared by two or
//! more kinds lives here. Sprites with no per-kind look (small fish, plankton,
//! detritus, sediment) are not content — their colors stay in the wallpaper.

use ratatui::style::Color;

use crate::engine::RockKind;

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
kinds!(rock, coral, kelp);

/// The plain big fish, worn by every kind that has no apex of its own — a warm
/// orange, clear of the small fish's lighter tone.
const BIG_FISH: Color = Color::Indexed(209);

/// One reef kind, whole: its economy row and its three sprite variants. The
/// engine reads the economy, the wallpaper reads the variants, and neither
/// needs to know how many kinds exist.
pub struct ReefDef {
    /// The row this kind contributes to `Params::rock_kinds`.
    pub economy: RockKind,
    /// Look of the base species growing on this kind.
    pub algae: AlgaeVariant,
    /// Look of this kind's rock body.
    pub rock: RockVariant,
    /// Look of the apex individual this kind hosts.
    pub big: BigVariant,
}

/// Look of the base species (algae) for a rock kind. Carried by the kind's own
/// definition, so a new kind's base layer is data in a new file, not renderer
/// code. Each kind's base layer looks different, so a reef reads by its
/// greenery alone.
pub struct AlgaeVariant {
    /// Two sway frames of the frond glyph.
    pub fronds: [&'static str; 2],
    pub color: Color,
}

/// Look of the rock body itself — one of the three variants a kind's definition
/// carries. The glyph tells kinds apart even in the dim placement ghost, where
/// color does not; color carries the difference for a placed reef.
pub struct RockVariant {
    /// Three body glyphs drawn left / center / right of the rock column.
    pub body: [&'static str; 3],
    pub color: Color,
}

/// Look of the apex individual a kind hosts. A big fish over rock and coral; a
/// slower, tan dugong over kelp — the top species takes on its reef's
/// character. Keyed only by the host rock's kind, so it stays a pure function
/// of (state, frame) like every other sprite.
pub struct BigVariant {
    pub right: &'static str,
    pub left: &'static str,
    /// Frames per column step; higher is slower (the dugong ambles).
    pub slowdown: u64,
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
