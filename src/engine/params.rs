//! Tunable parameters of the economy. Rules, step order, and rounding are
//! fixed by work/economy-model.md; the numbers here are placeholders bounded
//! by the invariant tests (tests/invariants.rs).

/// Fixed-point scale: every biomass/currency quantity is in micro-units.
pub const MICRO: u128 = 1_000_000;

/// Number of species in the trophic chain.
pub const SPECIES: usize = 4;

/// Discrete floor positions a rock can occupy in a run. Placement uses a slot
/// index in `0..SLOTS`; the renderer maps it to a pane column.
pub const SLOTS: u8 = 5;

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

/// A rock kind: what it costs to place (budget, not currency), the detritus it
/// sheds per tick, how long before its housing comes online, and how many of
/// each species it can house.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RockKind {
    /// Placement budget this kind consumes (an allocation, never currency).
    pub cost: u32,
    /// Detritus shed per tick. Effective from placement — the emergence delay
    /// gates housing only, never output (this is the run's first currency).
    pub output: u128,
    /// Ticks after run start before this kind's housing counts toward capacity.
    pub delay: u64,
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
    /// Base purchase cost per species.
    pub base_cost: [u128; SPECIES],
    /// Cost multiplier per owned unit.
    pub cost_growth: Ratio,
    /// Rock kinds available to place. The first run offers only the base rock.
    pub rock_kinds: Vec<RockKind>,
    /// Placement budget available at the start of a run (allocation type).
    pub placement_budget: u32,
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
            base_cost: [100 * MICRO, 450 * MICRO, 900 * MICRO, 4_000 * MICRO],
            cost_growth: Ratio::new(112, 100),
            rock_kinds: vec![RockKind {
                cost: 1,
                output: 3 * MICRO,
                delay: 0,
                capacity: [4, 3, 2, 1],
            }],
            placement_budget: 1,
        }
    }
}
