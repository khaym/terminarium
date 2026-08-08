//! The simulation state and its transitions. Pure state machine: no I/O, no
//! clock, no floats — determinism is what makes offline settlement exact.

use super::params::{Params, Species, MICRO, SLOTS, SPECIES};

/// A placed reef: which kind (index into `Params::reef_kinds`) and which floor
/// slot it occupies. Absolute coordinates live in the renderer, not here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reef {
    pub kind: usize,
    pub slot: u8,
}

/// Default anchor position: 800‰, the historical fixed column (width·4/5).
pub const DEFAULT_ANCHOR_POS: u16 = 800;
/// The largest valid anchor position; positions are a millipermille in 0..=999.
pub const ANCHOR_POS_MAX: u16 = 999;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct State {
    pub population: [u32; SPECIES],
    pub pool: [u128; SPECIES],
    pub nutrient: u128,
    pub collectable: u128,
    pub currency: u128,
    /// Lifetime score: every unit ever collected, summed across runs. Survives
    /// `reset`; unlocks and budget are derived from it.
    pub score: u128,
    /// Placed reefs, all put down before the run starts.
    pub reefs: Vec<Reef>,
    /// Ticks elapsed since the run started — the run clock (persisted as the
    /// save's `age`), advanced once per `tick`.
    pub tick_count: u64,
    /// Whether the player has committed the placement and begun the run. The
    /// gate for placement/removal (only while false) and for the clock (only
    /// while true).
    pub started: bool,
    /// Where the unlocked anchor sits, as a width-independent millipermille
    /// (0..=999) of the pane; the renderer alone turns it into a column. Pure
    /// scenery: `tick` never reads it, so it stays out of the economy. Meta-
    /// persistent — like `score`, it survives `reset` (a new sea) and reload.
    pub anchor_pos: u16,
}

impl Default for State {
    /// A fresh sea: empty tank, and the anchor at its default column. Written by
    /// hand (not derived) only so `anchor_pos` starts at `DEFAULT_ANCHOR_POS`
    /// rather than 0, which is a valid left-edge position, not "unset".
    fn default() -> Self {
        Self {
            population: [0; SPECIES],
            pool: [0; SPECIES],
            nutrient: 0,
            collectable: 0,
            currency: 0,
            score: 0,
            reefs: Vec::new(),
            tick_count: 0,
            started: false,
            anchor_pos: DEFAULT_ANCHOR_POS,
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Living matter in the tank. Grows only by photosynthesis and reef output;
    /// collection moves part of it into currency.
    pub fn biomass(&self) -> u128 {
        self.pool.iter().sum::<u128>() + self.nutrient + self.collectable
    }

    /// Biomass held in living populations only — the species pools, excluding
    /// the uncollected sediment (`collectable`) and free nutrient. This is what
    /// a "thriving tank" means for scenery gates: a sea earns it by keeping life
    /// alive, not by letting surplus pile up unpaid. Purely derived from state,
    /// so it touches neither the economy nor the save.
    pub fn living_biomass(&self) -> u128 {
        self.pool.iter().sum()
    }

    /// A run has begun once the player commits the placement (`start_run`).
    /// Before that the caller must not `advance` — the clock only runs during a
    /// live run.
    pub fn run_started(&self) -> bool {
        self.started
    }

    /// How many individuals of `species` the placed reefs can house. Housing
    /// counts from placement: capacity is a pure sum over the placed reefs and
    /// never depends on elapsed time. Time gates no purchase — only currency
    /// (the wall) and this capacity (the ceiling) do.
    pub fn capacity(&self, species: usize, p: &Params) -> u32 {
        self.reefs
            .iter()
            .map(|r| p.reef_kinds[r.kind].capacity[species])
            .sum()
    }

    /// Place a reef while composing the run. Succeeds only before the run is
    /// started, into a free in-range slot, for a kind the score has unlocked
    /// (`score >= unlock`), and within the placement budget (placed costs +
    /// this cost <= `budget(score)`). A rejected placement leaves the state
    /// untouched and returns false — placement spends budget, never currency or
    /// any other resource.
    pub fn place_reef(&mut self, kind: usize, slot: u8, p: &Params) -> bool {
        if self.started || kind >= p.reef_kinds.len() || slot >= SLOTS {
            return false;
        }
        if self.reefs.iter().any(|r| r.slot == slot) {
            return false;
        }
        if self.score < p.reef_kinds[kind].unlock {
            return false;
        }
        let placed: u32 = self.reefs.iter().map(|r| p.reef_kinds[r.kind].cost).sum();
        if placed.saturating_add(p.reef_kinds[kind].cost) > p.budget(self.score) {
            return false;
        }
        self.reefs.push(Reef { kind, slot });
        true
    }

    /// Remove the reef at `slot` while composing the run (for placing it
    /// differently). Only while the run has not started; returns whether a reef
    /// was there to remove.
    pub fn remove_reef(&mut self, slot: u8) -> bool {
        if self.started {
            return false;
        }
        if let Some(pos) = self.reefs.iter().position(|r| r.slot == slot) {
            self.reefs.remove(pos);
            true
        } else {
            false
        }
    }

    /// Commit the placement and begin the run. Succeeds when at least one reef
    /// is placed and the run has not already started; grants the seed currency
    /// once. This is the sole seed-grant point — one grant per run.
    pub fn start_run(&mut self, p: &Params) -> bool {
        if self.started || self.reefs.is_empty() {
            return false;
        }
        self.started = true;
        self.currency += p.seed_currency;
        true
    }

    /// Start a new sea: discard the run and return to placement, keeping only
    /// the meta-persistent state — the lifetime score (and thus the unlocks and
    /// budget derived from it) and the chosen anchor position.
    pub fn reset(&mut self) {
        *self = State {
            score: self.score,
            anchor_pos: self.anchor_pos,
            ..State::default()
        };
    }

    /// One simulation step (1 tick = 1 second). The step order is part of the
    /// spec — reordering changes results. Three sources create matter (steps
    /// 1-3), the middle steps only move it between pools, and step 7 settles
    /// what came loose.
    pub fn tick(&mut self, p: &Params) {
        let mut detritus: u128 = 0;

        // 1. Reef output — the run's only income until life is bought, and a
        // steady source after. Detritus accrues from the moment a reef is
        // placed.
        for reef in &self.reefs {
            detritus += p.reef_kinds[reef.kind].output;
        }

        // 2. The ecosystem's return — a rich reef's tenants enrich the sea.
        self.collectable += self.ecosystem_return(p);

        // 3. Photosynthesis — the algae's own creation, and the only source
        // that lands as living matter rather than as surplus.
        self.pool[0] += u128::from(self.population[0]) * p.photosynthesis;

        // 4. Nutrient uptake by algae (1:1, no remainder).
        let uptake = self
            .nutrient
            .min(u128::from(self.population[0]) * p.nutrient_uptake);
        self.nutrient -= uptake;
        self.pool[0] += uptake;

        // 5. Predation, ascending trophic order. The conversion remainder
        // settles as detritus so no mass is ever lost to rounding.
        for i in 1..SPECIES {
            let demand = u128::from(self.population[i]) * p.demand[i];
            let eaten = self.pool[i - 1].min(demand);
            self.pool[i - 1] -= eaten;
            let converted = p.conversion.apply(eaten);
            self.pool[i] += converted;
            detritus += eaten - converted;
        }

        // 6. Die-off: every pool sheds dead matter (this is also where the
        // apex predator's output goes).
        for pool in &mut self.pool {
            let dead = p.decay.apply(*pool);
            *pool -= dead;
            detritus += dead;
        }

        // 7. Detritus settles immediately.
        self.deposit(detritus, p);

        // 8. Advance the run clock.
        self.tick_count += 1;
    }

    /// The ecosystem's return: `Σ_reefs cost · (individuals housed there above
    /// the algae) · w`, created fresh every tick. A reef's placement cost is how
    /// rich a piece of sea it is, so the same fish returns more from a kelp
    /// forest than from a bare rock — which is what makes the tiers above the
    /// algae worth buying at all, and a reef's cost readable as its income.
    ///
    /// Where an individual lives is not stored: housing settles here, filling
    /// the costliest reef with room first (ties by placement order, which the
    /// stable sort keeps). Sorting by cost rather than walking the placement
    /// list keeps the return a pure function of *which* reefs are down and how
    /// many creatures live in them — never of the order the player dropped them,
    /// a difference nothing on screen could explain. Individuals beyond the
    /// total housing live nowhere and return nothing; `buy` cannot reach that
    /// state, only a hand-built one can.
    ///
    /// The return lands straight in the collectable pile rather than joining the
    /// detritus that settles at step 7. That is the whole point of the choice:
    /// routed through the split, ρ of it would come back as free nutrient and
    /// shift the balance every sea converges to. Landing it in the pile leaves
    /// the nutrient budget, the living biomass the whale gate reads, and every
    /// creature on screen exactly as they were — a third source of *income*,
    /// which is what #30 asked for, and nothing else.
    fn ecosystem_return(&self, p: &Params) -> u128 {
        let mut richest: Vec<usize> = (0..self.reefs.len()).collect();
        richest.sort_by_key(|&i| std::cmp::Reverse(p.reef_kinds[self.reefs[i].kind].cost));

        let mut weighted = 0u128;
        for species in 1..SPECIES {
            let mut unhoused = self.population[species];
            for &i in &richest {
                let kind = &p.reef_kinds[self.reefs[i].kind];
                let housed = unhoused.min(kind.capacity[species]);
                unhoused -= housed;
                weighted += u128::from(housed) * u128::from(kind.cost);
            }
        }
        p.ecosystem_return.apply(weighted * MICRO)
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
