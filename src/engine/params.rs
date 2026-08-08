//! Tunable parameters of the economy. Rules, step order, and rounding are
//! fixed by work/economy-model.md; the numbers here are placeholders bounded
//! by the invariant tests (tests/invariants.rs).

/// Fixed-point scale: every biomass/currency quantity is in micro-units.
pub const MICRO: u128 = 1_000_000;

/// Number of species in the trophic chain.
pub const SPECIES: usize = 4;

/// Discrete floor positions a reef can occupy in a run. Placement uses a slot
/// index in `0..SLOTS`; the renderer maps it to a pane column. Nine positions
/// give the placement phase real composition room while the odd count keeps a
/// true center slot (and the cursor's `SLOTS / 2` start) on the midline.
pub const SLOTS: u8 = 9;

/// Trophic chain, bottom to top. The numeric value doubles as the array index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    Algae = 0,
    Plankton = 1,
    SmallFish = 2,
    BigFish = 3,
}

/// A ratio applied as `floor(x * num / den)`. The multiplication saturates:
/// physical stocks stay bounded via decay, but the exponential cost sequence
/// can exceed u128 — it must pin at the top, never wrap back to cheap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ratio {
    pub num: u128,
    pub den: u128,
}

impl Ratio {
    pub const fn new(num: u128, den: u128) -> Self {
        Self { num, den }
    }

    pub fn apply(self, x: u128) -> u128 {
        x.saturating_mul(self.num) / self.den
    }
}

/// A reef kind: its name, what it costs to place (budget, not currency), the
/// score that unlocks it, the detritus it sheds per tick, and how many of each
/// species it can house.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReefKind {
    /// Display name (also the identity in the placement UI).
    pub name: &'static str,
    /// Placement budget this kind consumes (an allocation, never currency).
    pub cost: u32,
    /// Score at which this kind unlocks. A kind is placeable once
    /// `score >= unlock`; unlock is a pure derivation of score, not state.
    pub unlock: u128,
    /// Detritus shed per tick, effective from placement — the run's only income
    /// until life is bought (the seed grant is a one-time boost, not income).
    pub output: u128,
    /// How many individuals of each species this kind can house.
    pub capacity: [u32; SPECIES],
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Params {
    /// r: biomass created per algae per tick — the system's only source.
    pub photosynthesis: u128,
    /// g: nutrient an algae can take up per tick.
    pub nutrient_uptake: u128,
    /// c[i]: biomass an individual of species i demands per tick (index 0 unused).
    pub demand: [u128; SPECIES],
    /// E: fraction of eaten biomass converted upward; the rest settles as detritus.
    pub conversion: Ratio,
    /// d: fraction of every pool that dies off per tick.
    pub decay: Ratio,
    /// ρ: fraction of detritus recycled to nutrient; the rest is collectable.
    pub recycle: Ratio,
    /// w: what one individual living above the algae hands back per tick, as a
    /// fraction of the placement cost of the reef it lives in — the ecosystem's
    /// return, the economy's third source beside photosynthesis and reef output.
    /// A rich reef's tenants enrich the sea more than a bare rock's, which is
    /// what makes the upper tiers worth buying and a reef's cost readable as its
    /// income (the collection band, work/economy-model.md).
    pub ecosystem_return: Ratio,
    /// Base purchase cost per species.
    pub base_cost: [u128; SPECIES],
    /// Cost multiplier per owned unit.
    pub cost_growth: Ratio,
    /// Reef kinds available to place. A reef stores its kind as an index into
    /// this list, so the order is the save format (see `content::KINDS`) and
    /// append-only — which also means it is not necessarily ascending in unlock
    /// score, since a kind added later may unlock earlier than one already
    /// shipped. Whether a kind is placeable is read from its own `unlock`, never
    /// from its position.
    pub reef_kinds: Vec<ReefKind>,
    /// Placement budget schedule: `(score threshold, budget)` pairs in
    /// ascending threshold order. The budget available at a score is the one
    /// from the highest threshold at or below it (see `Params::budget`). Budget
    /// is an allocation, spent every run start up to the unlocked ceiling.
    pub budget_steps: Vec<(u128, u32)>,
    /// Currency granted once, at run start (`start_run`), so the first algae is
    /// reachable within a single peek rather than ~48s of reef output.
    /// Deliberately short of the first algae's cost — the player still watches
    /// the reef→sediment→collect→buy causal chain close over a few seconds.
    pub seed_currency: u128,
    /// Living-population biomass (the species pools, via `State::living_biomass`)
    /// at or above which a visiting whale may cross the pane. The whale is pure
    /// decoration outside the economy — this only gates the sighting, so a tank
    /// kept alive earns the lucky visit, while uncollected sediment does not buy
    /// it. Placeholder, pinned by the invariant that a full budget-3 rock sea
    /// stays under it and a full budget-5 rock sea clears it.
    pub whale_biomass: u128,
    /// Lifetime score at which the sunken-anchor scenery is unlocked. Derived
    /// from score like the reef unlocks, so no save field is added; once
    /// unlocked the anchor is drawn as a permanent landmark and hosts no events.
    pub anchor_unlock: u128,
}

impl Params {
    /// Placement budget unlocked at `score`: the budget of the highest step
    /// whose threshold is at or below the score. Budgets rise with their
    /// thresholds, so the highest qualifying step also carries the largest
    /// budget.
    pub fn budget(&self, score: u128) -> u32 {
        self.budget_steps
            .iter()
            .filter(|&&(threshold, _)| threshold <= score)
            .map(|&(_, budget)| budget)
            .max()
            .unwrap_or(0)
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            photosynthesis: MICRO,
            nutrient_uptake: 4 * MICRO / 5,
            demand: [0, 2 * MICRO, 5 * MICRO, 20 * MICRO],
            conversion: Ratio::new(1, 10),
            decay: Ratio::new(1, 100),
            recycle: Ratio::new(3, 10),
            ecosystem_return: Ratio::new(3, 5),
            base_cost: [100 * MICRO, 450 * MICRO, 900 * MICRO, 4_000 * MICRO],
            cost_growth: Ratio::new(112, 100),
            // The kinds and their order come from the content manifest, where
            // each kind's economy row sits beside its look — so adding a kind
            // never edits this file.
            reef_kinds: crate::content::reef_kinds(),
            budget_steps: vec![
                (0, 1),
                (12_000 * MICRO, 2),
                (30_000 * MICRO, 3),
                (75_000 * MICRO, 5),
            ],
            seed_currency: 80 * MICRO,
            whale_biomass: 400 * MICRO,
            anchor_unlock: 75_000 * MICRO,
        }
    }
}
