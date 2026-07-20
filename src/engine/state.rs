//! The simulation state and its transitions. Pure state machine: no I/O, no
//! clock, no floats — determinism is what makes offline settlement exact.

use super::params::{Params, Species, SLOTS, SPECIES};

/// A placed rock: which kind (index into `Params::rock_kinds`) and which floor
/// slot it occupies. Absolute coordinates live in the renderer, not here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rock {
    pub kind: usize,
    pub slot: u8,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct State {
    pub population: [u32; SPECIES],
    pub pool: [u128; SPECIES],
    pub nutrient: u128,
    pub collectable: u128,
    pub currency: u128,
    /// Lifetime score: every unit ever collected, summed across runs. Survives
    /// `reset`; unlocks and budget are derived from it.
    pub score: u128,
    /// Placed rocks, all put down before the run starts.
    pub rocks: Vec<Rock>,
    /// Ticks elapsed since the run started — the basis for emergence delay.
    pub tick_count: u64,
    /// Whether the player has committed the placement and begun the run. The
    /// gate for placement/removal (only while false) and for the clock (only
    /// while true).
    pub started: bool,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Living matter in the tank. Grows only by photosynthesis and rock output;
    /// collection moves part of it into currency.
    pub fn biomass(&self) -> u128 {
        self.pool.iter().sum::<u128>() + self.nutrient + self.collectable
    }

    /// A run has begun once the player commits the placement (`start_run`).
    /// Before that the caller must not `advance` — the clock only runs during a
    /// live run.
    pub fn run_started(&self) -> bool {
        self.started
    }

    /// How many individuals of `species` the placed rocks can house right now.
    /// A rock's housing counts only once it has emerged (`tick_count >= delay`);
    /// this is the single place the emergence delay is applied.
    pub fn capacity(&self, species: usize, p: &Params) -> u32 {
        self.rocks
            .iter()
            .filter(|r| self.tick_count >= p.rock_kinds[r.kind].delay)
            .map(|r| p.rock_kinds[r.kind].capacity[species])
            .sum()
    }

    /// Place a rock while composing the run. Succeeds only before the run is
    /// started, into a free in-range slot, for a kind the score has unlocked
    /// (`score >= unlock`), and within the placement budget (placed costs +
    /// this cost <= `budget(score)`). A rejected placement leaves the state
    /// untouched and returns false — placement spends budget, never currency or
    /// any other resource.
    pub fn place_rock(&mut self, kind: usize, slot: u8, p: &Params) -> bool {
        if self.started || kind >= p.rock_kinds.len() || slot >= SLOTS {
            return false;
        }
        if self.rocks.iter().any(|r| r.slot == slot) {
            return false;
        }
        if self.score < p.rock_kinds[kind].unlock {
            return false;
        }
        let placed: u32 = self.rocks.iter().map(|r| p.rock_kinds[r.kind].cost).sum();
        if placed.saturating_add(p.rock_kinds[kind].cost) > p.budget(self.score) {
            return false;
        }
        self.rocks.push(Rock { kind, slot });
        true
    }

    /// Remove the rock at `slot` while composing the run (for placing it
    /// differently). Only while the run has not started; returns whether a rock
    /// was there to remove.
    pub fn remove_rock(&mut self, slot: u8) -> bool {
        if self.started {
            return false;
        }
        if let Some(pos) = self.rocks.iter().position(|r| r.slot == slot) {
            self.rocks.remove(pos);
            true
        } else {
            false
        }
    }

    /// Commit the placement and begin the run. Succeeds when at least one rock
    /// is placed and the run has not already started; grants the seed currency
    /// once. This is the sole seed-grant point — one grant per run.
    pub fn start_run(&mut self, p: &Params) -> bool {
        if self.started || self.rocks.is_empty() {
            return false;
        }
        self.started = true;
        self.currency += p.seed_currency;
        true
    }

    /// Start a new sea: discard the run and return to placement, keeping only
    /// the lifetime score (and thus the unlocks and budget derived from it).
    pub fn reset(&mut self) {
        *self = State {
            score: self.score,
            ..State::default()
        };
    }

    /// One simulation step (1 tick = 1 second). The step order is part of the
    /// spec — reordering changes results.
    pub fn tick(&mut self, p: &Params) {
        let mut detritus: u128 = 0;

        // 1. Rock output — the run's only income until life is bought, and a
        // steady source after. The emergence delay gates housing only (see
        // `capacity`), never output, so detritus accrues from the moment a
        // rock is placed.
        for rock in &self.rocks {
            detritus += p.rock_kinds[rock.kind].output;
        }

        // 2. Photosynthesis — the only biological creation in the system.
        self.pool[0] += u128::from(self.population[0]) * p.photosynthesis;

        // 3. Nutrient uptake by algae (1:1, no remainder).
        let uptake = self
            .nutrient
            .min(u128::from(self.population[0]) * p.nutrient_uptake);
        self.nutrient -= uptake;
        self.pool[0] += uptake;

        // 4. Predation, ascending trophic order. The conversion remainder
        // settles as detritus so no mass is ever lost to rounding.
        for i in 1..SPECIES {
            let demand = u128::from(self.population[i]) * p.demand[i];
            let eaten = self.pool[i - 1].min(demand);
            self.pool[i - 1] -= eaten;
            let converted = p.conversion.apply(eaten);
            self.pool[i] += converted;
            detritus += eaten - converted;
        }

        // 5. Die-off: every pool sheds dead matter (this is also where the
        // apex predator's output goes).
        for pool in &mut self.pool {
            let dead = p.decay.apply(*pool);
            *pool -= dead;
            detritus += dead;
        }

        // 6. Detritus settles immediately.
        self.deposit(detritus, p);

        // 7. Advance the run clock.
        self.tick_count += 1;
    }

    /// Run `ticks` steps. Offline settlement is exactly this call — there is
    /// no separate settlement formula.
    pub fn advance(&mut self, ticks: u64, p: &Params) {
        for _ in 0..ticks {
            self.tick(p);
        }
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

    /// Buy one unit if affordable and there is housing for it. Two gates only:
    /// the wall is reachability (currency), the ceiling is housing (capacity).
    pub fn buy(&mut self, species: Species, p: &Params) -> bool {
        let i = species as usize;
        let cost = self.next_cost(species, p);
        if self.currency < cost || self.population[i] >= self.capacity(i, p) {
            return false;
        }
        self.currency -= cost;
        self.population[i] += 1;
        true
    }

    /// Move surplus detritus into currency, and count it toward the lifetime
    /// score. Score is a tally of the same quantity, not a duplicate resource.
    /// When to call it (the peek) is the UI layer's decision.
    pub fn collect(&mut self) {
        self.score += self.collectable;
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
