//! The simulation state and its transitions. Pure state machine: no I/O, no
//! clock, no floats — determinism is what makes offline settlement exact.

use super::params::{Params, Species, SPECIES};

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct State {
    pub population: [u32; SPECIES],
    pub pool: [u128; SPECIES],
    pub nutrient: u128,
    pub collectable: u128,
    pub currency: u128,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Living matter in the tank. Grows only by photosynthesis and feeding;
    /// collection moves part of it into currency.
    pub fn biomass(&self) -> u128 {
        self.pool.iter().sum::<u128>() + self.nutrient + self.collectable
    }

    /// One simulation step (1 tick = 1 second). The step order is part of the
    /// spec — reordering changes results.
    pub fn tick(&mut self, p: &Params) {
        let mut detritus: u128 = 0;

        // 1. Photosynthesis — the only creation in the system.
        self.pool[0] += u128::from(self.population[0]) * p.photosynthesis;

        // 2. Nutrient uptake by algae (1:1, no remainder).
        let uptake = self
            .nutrient
            .min(u128::from(self.population[0]) * p.nutrient_uptake);
        self.nutrient -= uptake;
        self.pool[0] += uptake;

        // 3. Predation, ascending trophic order. The conversion remainder
        // settles as detritus so no mass is ever lost to rounding.
        for i in 1..SPECIES {
            let demand = u128::from(self.population[i]) * p.demand[i];
            let eaten = self.pool[i - 1].min(demand);
            self.pool[i - 1] -= eaten;
            let converted = p.conversion.apply(eaten);
            self.pool[i] += converted;
            detritus += eaten - converted;
        }

        // 4. Die-off: every pool sheds dead matter (this is also where the
        // apex predator's output goes).
        for pool in &mut self.pool {
            let dead = p.decay.apply(*pool);
            *pool -= dead;
            detritus += dead;
        }

        // 5. Detritus settles immediately.
        self.deposit(detritus, p);
    }

    /// Run `ticks` steps. Offline settlement is exactly this call — there is
    /// no separate settlement formula.
    pub fn advance(&mut self, ticks: u64, p: &Params) {
        for _ in 0..ticks {
            self.tick(p);
        }
    }

    /// Manual feeding: uneaten food sinks as detritus. There is no time lock;
    /// automation makes feeding economically irrelevant instead.
    pub fn feed(&mut self, p: &Params) {
        self.deposit(p.feed_amount, p);
    }

    /// Cost of the next purchase of `species`: base·growth^owned, computed by
    /// sequential multiplication so every platform agrees on the result.
    pub fn next_cost(&self, species: Species, p: &Params) -> u128 {
        let mut cost = p.base_cost[species as usize];
        for _ in 0..self.population[species as usize] {
            cost = p.cost_growth.apply(cost);
        }
        cost
    }

    /// Buy one unit if affordable. Affordability is the only gate — the wall
    /// is reachability, not an unlock flag.
    pub fn buy(&mut self, species: Species, p: &Params) -> bool {
        let cost = self.next_cost(species, p);
        if self.currency < cost {
            return false;
        }
        self.currency -= cost;
        self.population[species as usize] += 1;
        true
    }

    /// Move surplus detritus into currency. When to call it (the peek) is the
    /// UI layer's decision.
    pub fn collect(&mut self) {
        self.currency += self.collectable;
        self.collectable = 0;
    }

    /// Detritus split: ρ recycles to nutrient, the remainder (including the
    /// rounding remainder) becomes collectable.
    fn deposit(&mut self, detritus: u128, p: &Params) {
        let to_nutrient = p.recycle.apply(detritus);
        self.nutrient += to_nutrient;
        self.collectable += detritus - to_nutrient;
    }
}
