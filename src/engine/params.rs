//! Tunable parameters of the economy. Rules, step order, and rounding are
//! fixed by work/economy-model.md; the numbers here are placeholders bounded
//! by the invariant tests (tests/invariants.rs).

/// Fixed-point scale: every biomass/currency quantity is in micro-units.
pub const MICRO: u128 = 1_000_000;

/// Number of species in the trophic chain.
pub const SPECIES: usize = 4;

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
    /// F: biomass added per manual feed.
    pub feed_amount: u128,
    /// Base purchase cost per species.
    pub base_cost: [u128; SPECIES],
    /// Cost multiplier per owned unit.
    pub cost_growth: Ratio,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            photosynthesis: MICRO,
            nutrient_uptake: MICRO / 2,
            demand: [0, 2 * MICRO, 5 * MICRO, 20 * MICRO],
            conversion: Ratio::new(1, 10),
            decay: Ratio::new(1, 100),
            recycle: Ratio::new(3, 10),
            feed_amount: 2 * MICRO,
            base_cost: [100 * MICRO, 100 * MICRO, 500 * MICRO, 5_000 * MICRO],
            cost_growth: Ratio::new(112, 100),
        }
    }
}
