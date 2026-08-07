//! Reef kind content: what a reef kind *is*, one file per kind.
//!
//! A kind is an economy row (what it costs, when it unlocks, what it sheds, how
//! much it houses), the look of its own rock body, and the creature it houses at
//! each of the four economy tiers. All of it lives in the same file — `rock.rs`,
//! `coral.rs`, `kelp.rs`, `grotto.rs`, `lantern.rs`, `lagoon.rs` — so adding a
//! kind is one new definition file plus its name in the `kinds!` manifest below,
//! with no edit to the engine or the renderer. `Params::default` builds its
//! economy rows from the manifest and the wallpaper reads the look from it, so
//! the two halves of a kind can never drift apart.
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
use crate::engine::ReefKind;

pub mod creatures;

/// Declares the kind modules and builds the manifest from one list, so
/// registering a kind and placing it in the manifest cannot drift apart — they
/// are the same edit. Each name is a `<name>.rs` beside this file exposing
/// `pub const DEF: ReefDef`.
macro_rules! kinds {
    ($($kind:ident),+ $(,)?) => {
        $(mod $kind;)+

        /// Every kind the game knows, in the order they were added — not in
        /// unlock order: a kind added later may unlock earlier than one already
        /// shipped.
        ///
        /// A kind's position here *is* its identity in a save file (`Reef::kind`
        /// is an index into this list, and so into `Params::reef_kinds`), which
        /// makes the order append-only: reordering these entries silently
        /// rewrites what every save already on disk means.
        pub const KINDS: &[&ReefDef] = &[$(&$kind::DEF),+];
    };
}

// The manifest: the one shared line a new kind adds itself to (append only).
kinds!(rock, coral, kelp, grotto, lantern, lagoon);

/// One reef kind, whole: its economy row, its rock body, and its four tenants.
/// The engine reads the economy, the wallpaper reads the body and follows the
/// tenant references, and neither needs to know how many kinds exist.
pub struct ReefDef {
    /// The row this kind contributes to `Params::reef_kinds`.
    pub economy: ReefKind,
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
/// fills `reef_kinds` with.
pub fn reef_kinds() -> Vec<ReefKind> {
    KINDS.iter().map(|def| def.economy.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::MICRO;

    /// The progression registry: what the shipped game asks a player to work
    /// toward, as one literal table — every kind's placement cost and unlock
    /// score, in manifest order.
    ///
    /// This is an *exact* match on the whole table, so adding a kind, retuning
    /// a cost or moving an unlock all turn it red on purpose. The red is the
    /// point: the standard answer is to append the ticket's agreed
    /// `(name, cost, unlock)` row here, which makes this file the one place a
    /// change to the progression design is recorded and read by a human.
    ///
    /// It reads like `shipped_kinds_keep_their_save_identity` above and guards
    /// something else. That one pins a *prefix*, so appending a kind stays
    /// green — appending cannot break a save. This one admits nothing silently.
    /// The display rules that read these numbers (the HUD goal line, the
    /// placement panel, the headroom nudge, the placement cycle) are tested
    /// against manifests those tests build themselves, so they stay green when
    /// a kind joins and this stays the one red that asks a human to look.
    #[test]
    fn progression_registry_pins_cost_and_unlock() {
        let table: Vec<(&str, u32, u128)> = KINDS
            .iter()
            .map(|def| (def.economy.name, def.economy.cost, def.economy.unlock))
            .collect();
        assert_eq!(
            table,
            vec![
                ("rock", 1, 0),
                ("coral", 2, 12_000 * MICRO),
                ("kelp", 3, 30_000 * MICRO),
                ("grotto", 3, 40_000 * MICRO),
                ("lantern", 1, 100_000 * MICRO),
                ("lagoon", 2, 60_000 * MICRO),
            ],
            "the progression design moved — record the agreed row here"
        );
    }

    /// The manifest order is the save format: a reef stores its kind as an
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

    /// `SwimmerDef::accent`'s index counts cells of `right`, and the renderer's
    /// left-facing mirror (`cells - 1 - index`) only lands correctly if `right`
    /// and `left` are the same length in cells — true of every shipped swimmer.
    /// An out-of-range index degrades to no accent rather than panicking
    /// (`patrol`, src/ui/wallpaper.rs), but shipped content should never
    /// actually take that degrade path, so both preconditions are pinned here
    /// across every kind's small and big tenants (the only tiers that are
    /// swimmers) and across both appearances, since a pulse turns over
    /// independently of the heading and its frame is mirrored the same way.
    #[test]
    fn swimmer_defs_have_matching_cell_counts_and_in_range_accents() {
        for def in KINDS {
            for swimmer in [def.small, def.big] {
                // Frame 0 is the swimmer's own look; one pulse period on is its
                // second, if it has one.
                let beat = swimmer
                    .manner
                    .pulse
                    .as_ref()
                    .map_or(0, |pulse| pulse.period.max(1));
                for frame in [0, beat] {
                    let look = swimmer.look(frame);
                    let right_cells = look.right.chars().count();
                    let left_cells = look.left.chars().count();
                    assert_eq!(
                        right_cells, left_cells,
                        "{}: right/left cell counts must match ({right_cells} vs {left_cells})",
                        def.economy.name
                    );
                    if let Some((index, _)) = swimmer.accent {
                        assert!(
                            index < right_cells,
                            "{}: accent index {index} is out of range for {right_cells} cells",
                            def.economy.name
                        );
                    }
                }
            }
        }
    }

    /// A pulsing swimmer really does wear two appearances: its own and its
    /// pulse's differ, so the beat shows on screen instead of redrawing the
    /// same sprite. (Only the jellyfish pulses today; the loop states the rule
    /// for whatever pulses next.)
    #[test]
    fn a_pulse_changes_the_sprite_it_draws() {
        for def in KINDS {
            for swimmer in [def.small, def.big] {
                let Some(pulse) = &swimmer.manner.pulse else {
                    continue;
                };
                let own = swimmer.look(0);
                assert!(
                    (own.right, own.under) != (pulse.look.right, pulse.look.under),
                    "{}: a pulse must draw something other than the body it pulses from",
                    def.economy.name
                );
                assert!(
                    pulse.period > 0,
                    "{}: a pulse needs a beat",
                    def.economy.name
                );
            }
        }
    }
}
