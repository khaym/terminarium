//! Invariant tests — the executable form of the business rules fixed in
//! work/economy-model.md. Parameter values are placeholders; these tests are
//! the constraints they must keep satisfying while being tuned.

use terminarium::engine::{Params, Species, State, ANCHOR_POS_MAX, MICRO};

/// A tank with every trophic level active and non-trivial stocks, so that
/// every branch of the tick (uptake, predation, decay, recycling) is live.
fn populated_state() -> State {
    let mut s = State::new();
    s.population = [3, 2, 1, 1];
    s.pool = [50 * MICRO, 10 * MICRO, 5 * MICRO, 2 * MICRO];
    s.nutrient = 7 * MICRO;
    s.collectable = 11 * MICRO;
    s
}

/// Invariant 1 — conservation: with rocks placed, the only things that create
/// living matter are photosynthesis and rock output. Exact equality, every
/// tick.
#[test]
fn conservation_per_tick() {
    let p = Params::default();
    let mut s = populated_state();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));
    let rock_output = p.rock_kinds[0].output;
    for _ in 0..10_000 {
        let before = s.biomass();
        s.tick(&p);
        let created = u128::from(s.population[0]) * p.photosynthesis + rock_output;
        assert_eq!(s.biomass(), before + created);
    }
}

/// Invariant 1 (operations) — place allocates budget without touching biomass;
/// collect moves matter to currency without creating or destroying any.
#[test]
fn conservation_across_operations() {
    let p = Params::default();
    let mut s = populated_state();

    let before = s.biomass();
    assert!(s.place_rock(0, 0, &p));
    assert_eq!(s.biomass(), before, "placement moves no biomass");

    let before_total = s.biomass() + s.currency;
    s.collect();
    assert_eq!(s.biomass() + s.currency, before_total);
    assert_eq!(s.collectable, 0);
}

/// Invariant 2 — advancing is compositional: settling a long absence in one
/// call is identical to living through it tick by tick.
#[test]
fn advance_is_compositional() {
    let p = Params::default();
    let mut whole = populated_state();
    let mut split = populated_state();
    assert!(whole.place_rock(0, 0, &p));
    assert!(split.place_rock(0, 0, &p));
    assert!(whole.start_run(&p));
    assert!(split.start_run(&p));

    whole.advance(1_000, &p);
    split.advance(400, &p);
    split.advance(600, &p);

    assert_eq!(whole, split);
}

/// Invariant 3 — determinism: the same starting state and the same operation
/// script always end in the same state.
#[test]
fn determinism_under_operation_script() {
    let p = Params::default();
    let script = |s: &mut State| {
        s.place_rock(0, 0, &p);
        s.start_run(&p);
        for t in 0..600u64 {
            s.tick(&p);
            if t % 90 == 0 {
                s.collect();
                s.buy(Species::Algae, &p);
            }
        }
    };

    let mut a = State::new();
    let mut b = State::new();
    script(&mut a);
    script(&mut b);

    assert_eq!(a, b);
}

/// Invariant 4 — cost curve: strictly increasing, with a constant growth
/// ratio step (the sequential-multiplication rule itself).
#[test]
fn cost_curve_is_exponential_and_monotone() {
    let p = Params::default();
    let mut s = State::new();
    let mut prev = s.next_cost(Species::Algae, &p);
    for owned in 1..=50u32 {
        s.population[Species::Algae as usize] = owned;
        let cost = s.next_cost(Species::Algae, &p);
        assert!(cost > prev, "cost must strictly increase (k={owned})");
        assert_eq!(
            cost,
            p.cost_growth.apply(prev),
            "growth ratio must stay constant (k={owned})"
        );
        prev = cost;
    }
}

/// Invariant 4 (overflow guard) — the cost sequence saturates instead of
/// wrapping: a huge ownership count must never price below an earlier one.
#[test]
fn cost_curve_saturates_instead_of_wrapping() {
    let p = Params::default();
    let mut s = State::new();
    s.population[Species::Algae as usize] = 50;
    let mid = s.next_cost(Species::Algae, &p);
    s.population[Species::Algae as usize] = 5_000;
    let far = s.next_cost(Species::Algae, &p);
    assert!(far >= mid, "cost must never wrap below earlier values");
}

/// Invariant 5 — no-op bootstrap: from a single placed base rock, the passive
/// loop {advance(1) → collect → try to buy algae} affords the first algae
/// within the 15-second opening (no other intervention). The run-start seed
/// currency is what pulls this inside a single peek.
#[test]
fn first_algae_bootstraps_from_rock_within_15s() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));

    let mut seconds = 0u64;
    loop {
        s.advance(1, &p);
        seconds += 1;
        s.collect();
        if s.buy(Species::Algae, &p) {
            break;
        }
        assert!(
            seconds < 15,
            "first algae must be affordable within 15s of rock output"
        );
    }
    println!("first algae bought after {seconds}s of rock output");
}

/// Invariant 6 — first wall distance: from one base rock plus one algae, the
/// baseline play (collect on every 90s peek, no extra purchases) reaches the
/// first plankton on peek 2 or 3.
#[test]
fn first_wall_at_two_to_three_peeks() {
    let p = Params::default();
    const PEEK_SECONDS: u64 = 90;

    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));
    s.population[Species::Algae as usize] = 1;

    let mut peeks = 0u32;
    while peeks < 10 {
        s.advance(PEEK_SECONDS, &p);
        peeks += 1;
        s.collect();
        if s.currency >= s.next_cost(Species::Plankton, &p) {
            break;
        }
    }

    println!("plankton affordable at peek {peeks}");
    assert!(
        (2..=3).contains(&peeks),
        "first wall must sit at peek 2-3, got {peeks}"
    );
}

/// Invariant 7 — the detritus cycle sustains: nutrient recycling actually
/// feeds the algae, collectables keep accruing at the photosynthesis rate,
/// and no pool grows without bound.
#[test]
fn detritus_cycle_sustains_and_stays_bounded() {
    let p = Params::default();
    let mut s = State::new();
    s.population = [10, 3, 2, 1];

    // Reach steady state, then observe a second window of the same length.
    s.advance(10_000, &p);
    let mid_pools = s.pool;
    let mid_nutrient = s.nutrient;
    let mid_collectable = s.collectable;

    // The chain itself is alive: apex stock can only arrive via conversion
    // through every tier, so a positive apex pool proves the whole chain ran.
    // (Middle pools may legitimately sit at zero — underfed tiers eat their
    // entire inflow within the tick.)
    assert!(mid_pools[0] > 0, "algae pool must hold biomass");
    assert!(
        mid_pools[3] > 0,
        "apex pool must receive biomass through the full chain"
    );

    // Recycling is live: there is nutrient in the water and algae capacity to
    // take it up, so the next tick consumes some of it.
    assert!(mid_nutrient > 0, "steady state must hold recycled nutrient");
    let uptake_capacity = u128::from(s.population[0]) * p.nutrient_uptake;
    assert!(uptake_capacity > 0);

    s.advance(10_000, &p);

    // Collectables keep flowing, and in steady state the collection rate
    // converges to the photosynthesis rate (sole source, sole sink) —
    // allow 20% tolerance for the transient and integer rounding.
    let window_gain = s.collectable - mid_collectable;
    let created = 10_000u128 * u128::from(s.population[0]) * p.photosynthesis;
    assert!(window_gain > 0, "collectable must keep accruing");
    assert!(
        window_gain * 10 > created * 8 && window_gain * 10 < created * 12,
        "steady collection rate must track photosynthesis (got {window_gain} vs created {created})"
    );

    // Bounded pools: a converged tank does not keep growing. Allow 10% slack
    // over the mid-point measurement.
    for (i, &mid) in mid_pools.iter().enumerate() {
        assert!(
            s.pool[i] * 10 <= mid * 11,
            "pool[{i}] must stay bounded ({mid} -> {})",
            s.pool[i]
        );
    }
    assert!(
        s.nutrient * 10 <= mid_nutrient * 11,
        "nutrient must stay bounded ({mid_nutrient} -> {})",
        s.nutrient
    );
}

/// Invariant 8 — housing ceiling: once a species fills its capacity, buy fails
/// even with currency to spare.
#[test]
fn buy_is_capped_by_capacity() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    s.currency = u128::MAX / 2; // never the limiting factor

    let cap = s.capacity(Species::Algae as usize, &p);
    assert!(cap > 0);
    for _ in 0..cap {
        assert!(
            s.buy(Species::Algae, &p),
            "buying up to capacity must succeed"
        );
    }
    assert_eq!(s.population[Species::Algae as usize], cap);
    assert!(
        !s.buy(Species::Algae, &p),
        "buy must fail at capacity even with currency"
    );
    assert_eq!(s.population[Species::Algae as usize], cap);
}

/// Invariant 9 — steadiness: with every species filled to capacity and no
/// further purchases, any advance leaves populations fixed and pools/nutrient
/// bounded, while collectable alone keeps growing.
#[test]
fn steady_state_after_capacity_is_filled() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));

    // Fill to the base rock's housing: [4, 3, 2, 1].
    for (i, sp) in [
        Species::Algae,
        Species::Plankton,
        Species::SmallFish,
        Species::BigFish,
    ]
    .into_iter()
    .enumerate()
    {
        s.population[i] = s.capacity(i, &p);
        assert!(!s.buy(sp, &p), "no room left once at capacity");
    }
    let pops = s.population;

    s.advance(20_000, &p);
    let mid_pools = s.pool;
    let mid_nutrient = s.nutrient;
    let mid_collectable = s.collectable;

    s.advance(20_000, &p);

    assert_eq!(
        s.population, pops,
        "populations stay fixed with no purchases"
    );
    assert!(
        s.collectable > mid_collectable,
        "collectable must keep accruing"
    );
    for (i, &mid) in mid_pools.iter().enumerate() {
        assert!(
            s.pool[i] * 10 <= mid * 11 + MICRO,
            "pool[{i}] must stay bounded ({mid} -> {})",
            s.pool[i]
        );
    }
    assert!(
        s.nutrient * 10 <= mid_nutrient * 11 + MICRO,
        "nutrient must stay bounded ({mid_nutrient} -> {})",
        s.nutrient
    );
}

/// Business rule — a new sea is buyable the instant currency arrives, never
/// gated by elapsed time. Placing kelp only (the kind that carried the longest
/// old emergence delay) and starting the run, its housing is already in
/// capacity at tick 0, so the first buy succeeds the moment currency covers the
/// cost. This is the executable form of "after a new sea, the first purchasable
/// moment is set by currency alone" — the death-time #22 removed.
#[test]
fn a_new_sea_is_buyable_the_instant_currency_arrives() {
    let p = Params::default();
    let kelp = 2usize; // unlock 30,000; cost 3 fits budget 3 at that score
    let algae = Species::Algae as usize;
    let mut s = State::new();
    s.score = 30_000 * MICRO;
    assert!(s.place_rock(kelp, 0, &p));
    assert!(s.start_run(&p));

    assert_eq!(
        s.tick_count, 0,
        "no time has passed since the new sea began"
    );
    assert_eq!(
        s.capacity(algae, &p),
        p.rock_kinds[kelp].capacity[algae],
        "kelp housing counts from placement, not after an emergence delay"
    );

    // Currency alone gates the first buy: give exactly the cost and it succeeds
    // at tick 0, with no advance.
    s.currency = s.next_cost(Species::Algae, &p);
    assert!(
        s.buy(Species::Algae, &p),
        "the first algae is buyable the instant currency covers it"
    );
    assert_eq!(s.population[algae], 1);
}

/// Invariant 10 — housing is immediate: a placed rock's capacity counts from
/// the moment the run starts and never depends on elapsed time. (#22 removed
/// the emergence delay — placement is the only thing that gates housing, so a
/// new sea has full capacity at tick 0.)
#[test]
fn housing_counts_from_placement_regardless_of_time() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p)); // base rock, housing [4, 3, 2, 1]
    assert!(s.start_run(&p));

    let expected: Vec<u32> = (0..4).map(|i| p.rock_kinds[0].capacity[i]).collect();

    // At tick 0, before any advance, full housing is already available.
    assert_eq!(s.tick_count, 0);
    for (i, &cap) in expected.iter().enumerate() {
        assert_eq!(
            s.capacity(i, &p),
            cap,
            "species {i} housing counts from placement"
        );
    }

    // Elapsed time never changes it.
    s.advance(1_000, &p);
    for (i, &cap) in expected.iter().enumerate() {
        assert_eq!(
            s.capacity(i, &p),
            cap,
            "species {i} housing is unchanged by elapsed time"
        );
    }
}

/// The index a kind's name resolves to in `Params::default().rock_kinds` — which
/// is also `Rock::kind`. A reef a test names is looked up here rather than
/// written as a literal, since the manifest is append-only and a kind joining it
/// must not silently renumber the reef another test meant.
fn kind_index(name: &str) -> usize {
    Params::default()
        .rock_kinds
        .iter()
        .position(|k| k.name == name)
        .unwrap_or_else(|| panic!("no such rock kind: {name}"))
}

/// Fill every species to the housing the given reef provides, then converge and
/// measure the collectable gained over a window — the steady collection rate of
/// a filled tank.
fn steady_collection_over_window(rocks: &[(usize, u8)], score: u128, window: u64) -> u128 {
    let p = Params::default();
    let mut s = State::new();
    s.score = score;
    for &(kind, slot) in rocks {
        assert!(
            s.place_rock(kind, slot, &p),
            "reef {rocks:?} must place within unlock/budget at score {score}"
        );
    }
    assert!(s.start_run(&p));

    // Housing is available from placement, so fill to capacity right away.
    for i in 0..4 {
        s.population[i] = s.capacity(i, &p);
    }

    // Converge, then measure a same-length window.
    s.advance(30_000, &p);
    let before = s.collectable;
    s.advance(window, &p);
    s.collectable - before
}

/// Invariant 11 — gradient: after coral unlocks (score 12,000), a re-placed run
/// that spends budget 2 (either rock×2 or a single coral) collects faster at
/// steady state than the old budget-1 rock does. This is the existence proof
/// that rebuilding pays off.
#[test]
fn regrown_reef_out_collects_the_old_one() {
    const WINDOW: u64 = 5_000;
    let t1 = 12_000 * MICRO;

    let old = steady_collection_over_window(&[(0, 0)], 0, WINDOW);
    let rock_x2 = steady_collection_over_window(&[(0, 0), (0, 1)], t1, WINDOW);
    let coral = steady_collection_over_window(&[(1, 0)], t1, WINDOW);

    assert!(
        rock_x2 > old,
        "rock×2 (budget 2) must out-collect rock×1: {rock_x2} vs {old}"
    );
    assert!(
        coral > old,
        "coral (budget 2) must out-collect rock×1: {coral} vs {old}"
    );
}

/// Fill the given reef to housing, converge, and read the living biomass (the
/// species pools) — the steady stock a filled tank holds, independent of how
/// often surplus is collected (collection never touches the pools).
fn steady_living_biomass(rocks: &[(usize, u8)], score: u128) -> u128 {
    let p = Params::default();
    let mut s = State::new();
    s.score = score;
    for &(kind, slot) in rocks {
        assert!(
            s.place_rock(kind, slot, &p),
            "reef {rocks:?} must place within unlock/budget at score {score}"
        );
    }
    assert!(s.start_run(&p));
    for i in 0..4 {
        s.population[i] = s.capacity(i, &p);
    }
    s.advance(30_000, &p); // converge
    s.living_biomass()
}

/// Invariant — the whale gate marks a grown sea: a full budget-3 rock reef
/// (rock×3, every species bought to housing, converged) stays under the whale's
/// living-biomass gate, while the smallest full budget-5 reef (rock×5) clears
/// it. So crossing the second wall and filling the larger reef is what earns the
/// visit — the gate reads living populations (`living_biomass`), not surplus.
#[test]
fn whale_gate_sits_between_full_budget_three_and_five_reefs() {
    let p = Params::default();
    let full3 = steady_living_biomass(&[(0, 0), (0, 1), (0, 2)], 30_000 * MICRO);
    let full5 = steady_living_biomass(&[(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)], 75_000 * MICRO);

    assert!(
        full3 < p.whale_biomass,
        "a full budget-3 rock sea must stay under the whale gate: {full3} vs {}",
        p.whale_biomass
    );
    assert!(
        p.whale_biomass <= full5,
        "a full budget-5 rock sea must earn the whale: gate {} vs {full5}",
        p.whale_biomass
    );
}

/// Invariant 11b — the second wall pays off: after the budget-5 step (score
/// 75,000) a kelp+coral sea collects faster at steady state than the best reef
/// budget 3 can compose (kelp alone / coral+rock / rock×3 / grotto / lagoon+rock
/// / lagoon+lantern). The existence proof that crossing the new wall is worth
/// it, mirroring the budget-2-over-1 proof.
#[test]
fn budget_five_reef_out_collects_the_best_budget_three() {
    const WINDOW: u64 = 5_000;
    let (rock, coral, kelp_k, grotto_k, lagoon_k) = (
        kind_index("rock"),
        kind_index("coral"),
        kind_index("kelp"),
        kind_index("grotto"),
        kind_index("lagoon"),
    );
    let t3 = 30_000 * MICRO;
    let t4 = 40_000 * MICRO;
    let t5 = 75_000 * MICRO;
    let t6 = 60_000 * MICRO;

    // Every way to spend budget 3 in full: the three compositions open once kelp
    // unlocks at score 30,000, the grotto from its own unlock at 40,000, and the
    // cost-2 lagoon beside a cost-1 reef from 60,000 — all still budget 3, since
    // the schedule's next step is the 75,000 wall. A partial spend houses
    // strictly less, so it can never be the best.
    // (#33: lantern, cost 1, opens no budget-3 pair of its own — its unlock is
    // 100,000, past the 75,000 wall that is budget 3's own ceiling here, so
    // lagoon's partner at 60,000 can only be a rock.)
    let kelp = steady_collection_over_window(&[(kelp_k, 0)], t3, WINDOW);
    let coral_rock = steady_collection_over_window(&[(coral, 0), (rock, 1)], t3, WINDOW);
    let rock_x3 = steady_collection_over_window(&[(rock, 0), (rock, 1), (rock, 2)], t3, WINDOW);
    let grotto = steady_collection_over_window(&[(grotto_k, 0)], t4, WINDOW);
    let lagoon_rock = steady_collection_over_window(&[(lagoon_k, 0), (rock, 1)], t6, WINDOW);
    let best_three = kelp
        .max(coral_rock)
        .max(rock_x3)
        .max(grotto)
        .max(lagoon_rock);

    // Budget 5 buys kelp+coral — the composition the new wall unlocks.
    let kelp_coral = steady_collection_over_window(&[(kelp_k, 0), (coral, 1)], t5, WINDOW);

    assert!(
        kelp_coral > best_three,
        "kelp+coral (budget 5) must out-collect the best budget-3 reef: \
         {kelp_coral} vs best {best_three} \
         (kelp {kelp}, coral+rock {coral_rock}, rock×3 {rock_x3}, grotto {grotto}, \
          lagoon+rock {lagoon_rock})"
    );
}

/// Invariant 11c — the floor of the lagoon's accepted collection band: at its
/// own unlock (60,000) a filled lagoon out-collects the coral it costs the same
/// as, so the reef a later wall hands out is worth rebuilding around. The same
/// "rebuilding pays off" proof `regrown_reef_out_collects_the_old_one` states
/// for coral, applied to the reef that now shares that budget.
///
/// The ceiling is the test below, and the two together freeze the agreed band
/// `coral <= lagoon <= kelp` — see it for what fixes those two ends.
#[test]
fn a_filled_lagoon_out_collects_the_coral_it_costs_the_same_as() {
    const WINDOW: u64 = 5_000;
    let (coral, lagoon_k) = (kind_index("coral"), kind_index("lagoon"));
    let t6 = 60_000 * MICRO;

    let lagoon = steady_collection_over_window(&[(lagoon_k, 0)], t6, WINDOW);
    let coral_only = steady_collection_over_window(&[(coral, 0)], t6, WINDOW);

    assert!(
        lagoon > coral_only,
        "a lagoon must out-collect the coral it costs the same as: {lagoon} vs {coral_only}"
    );
}

/// Invariant 11d — the ceiling of that band: a filled lagoon stays under a
/// filled kelp. Kelp is the reef the progression leans on as its collection
/// anchor — the strongest single reef budget 3 can buy — and a kind unlocking
/// into the band below it may approach that rate but not pass it, or the
/// anchor stops meaning anything and the reef ordering reads backwards.
///
/// Why *kelp* is the anchor and not the nearest reef by cost: steady collection
/// in this economy tracks algae housing and rock output almost alone. The tiers
/// above the algae move biomass around and return it as detritus, so they never
/// lift the rate — which is why the grotto, at cost 3 with 2 algae, collects
/// exactly what the cost-2 coral does (measured: both 54,600 per 5,000 ticks at
/// full housing, against the lagoon's 67,200 and kelp's 98,700). A ceiling read
/// off cost alone would therefore be an empty band; the ceiling is the anchor
/// reef instead. Whether the upper tiers should feed the rate at all is a
/// structural question, open as #30 — until it lands, this band is the line the
/// design holds, and a new reef that crosses either end turns one of these two
/// asserts red on purpose.
#[test]
fn a_filled_lagoon_stays_under_the_kelp_that_anchors_the_band() {
    const WINDOW: u64 = 5_000;
    let (kelp_k, lagoon_k) = (kind_index("kelp"), kind_index("lagoon"));
    let t6 = 60_000 * MICRO;

    let lagoon = steady_collection_over_window(&[(lagoon_k, 0)], t6, WINDOW);
    let kelp = steady_collection_over_window(&[(kelp_k, 0)], t6, WINDOW);

    assert!(
        lagoon <= kelp,
        "a lagoon must not out-collect the kelp that anchors the band: {lagoon} vs {kelp}"
    );
}

/// Invariant 12 — unlock stride: playing the opening greedily (each 90s peek:
/// advance, collect, buy everything affordable with housing) reaches the first
/// unlock (score 12,000) *after* the tank has filled, and within twelve peeks
/// of it. The first wall comes first; the first unlock lands a handful of peeks
/// later.
#[test]
fn first_unlock_lands_within_twelve_peeks_of_the_wall() {
    let p = Params::default();
    const PEEK: u64 = 90;
    let t1 = 12_000 * MICRO;

    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));
    let cap: [u32; 4] = [
        s.capacity(0, &p),
        s.capacity(1, &p),
        s.capacity(2, &p),
        s.capacity(3, &p),
    ];

    let mut p_fill: Option<u32> = None;
    let mut p_t1: Option<u32> = None;
    for peek in 1..=200u32 {
        s.advance(PEEK, &p);
        s.collect();
        // Greedy: keep buying anything affordable with housing until a full
        // pass buys nothing.
        loop {
            let mut bought = false;
            for sp in [
                Species::Algae,
                Species::Plankton,
                Species::SmallFish,
                Species::BigFish,
            ] {
                if s.buy(sp, &p) {
                    bought = true;
                }
            }
            if !bought {
                break;
            }
        }
        if p_fill.is_none() && (0..4).all(|i| s.population[i] == cap[i]) {
            p_fill = Some(peek);
        }
        if p_t1.is_none() && s.score >= t1 {
            p_t1 = Some(peek);
        }
        if p_fill.is_some() && p_t1.is_some() {
            break;
        }
    }

    let p_fill = p_fill.expect("housing must fill");
    let p_t1 = p_t1.expect("score must reach the first unlock");
    println!("P_fill={p_fill} P_T1={p_t1}");
    assert!(
        p_fill < p_t1,
        "the wall must come before the first unlock (P_fill={p_fill}, P_T1={p_t1})"
    );
    assert!(
        p_t1 <= p_fill + 12,
        "first unlock within 12 peeks of the wall (P_fill={p_fill}, P_T1={p_t1})"
    );
}

/// Score is the running tally of collected detritus — every `collect` adds the
/// surplus to both currency and the lifetime score.
#[test]
fn collect_accumulates_lifetime_score() {
    let mut s = State::new();
    s.collectable = 100 * MICRO;
    s.collect();
    assert_eq!(s.score, 100 * MICRO);
    assert_eq!(s.currency, 100 * MICRO);
    assert_eq!(s.collectable, 0);

    s.collectable = 50 * MICRO;
    s.collect();
    assert_eq!(s.score, 150 * MICRO, "score accumulates across collects");
    assert_eq!(s.currency, 150 * MICRO);
}

/// `reset` (new sea) keeps only the meta-persistent state — the lifetime score
/// and the chosen anchor position; everything else about the run returns to the
/// initial state.
#[test]
fn reset_keeps_score_and_clears_the_run() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));
    s.advance(200, &p);
    s.collect();
    s.population = [1, 1, 0, 0];
    s.anchor_pos = 250; // moved off the default column
    let kept = s.score;
    assert!(kept > 0);

    s.reset();

    assert_eq!(s.score, kept, "score survives a new sea");
    assert_eq!(s.anchor_pos, 250, "the anchor position survives a new sea");
    assert_eq!(s.population, [0, 0, 0, 0]);
    assert_eq!(s.pool, [0; 4]);
    assert_eq!(s.nutrient, 0);
    assert_eq!(s.collectable, 0);
    assert_eq!(s.currency, 0);
    assert!(s.rocks.is_empty());
    assert_eq!(s.tick_count, 0);
    assert!(!s.run_started());
}

/// The anchor position is pure scenery: `tick` neither reads nor writes it, so
/// the economy is identical whatever the anchor's placement (experience-
/// direction principle 4 — the placement is a picture, never a simulation
/// input). Two runs advanced from opposite anchor positions end in the same
/// state once the (untouched) anchor field is set aside.
#[test]
fn anchor_position_stays_out_of_the_economy() {
    let p = Params::default();
    let base = {
        let mut s = populated_state();
        assert!(s.place_rock(0, 0, &p));
        assert!(s.start_run(&p));
        s
    };

    let mut left = base.clone();
    left.anchor_pos = 0;
    let mut right = base.clone();
    right.anchor_pos = ANCHOR_POS_MAX;

    left.advance(5_000, &p);
    right.advance(5_000, &p);

    assert_eq!(
        left.anchor_pos, 0,
        "tick leaves the anchor position untouched"
    );
    assert_eq!(
        right.anchor_pos, ANCHOR_POS_MAX,
        "tick leaves the anchor position untouched"
    );

    // Aside from the anchor field, the two states are identical — the anchor
    // fed nothing into the simulation.
    left.anchor_pos = right.anchor_pos;
    assert_eq!(
        left, right,
        "the economy is independent of the anchor position"
    );
}

/// Placement is gated by the unlock score: a kind whose unlock the score has
/// not cleared cannot be placed, and can once it has.
#[test]
fn placement_is_gated_by_unlock_score() {
    let p = Params::default();
    let mut s = State::new();
    // Coral (kind 1) unlocks at 12,000; below that, place fails.
    assert!(!s.place_rock(1, 0, &p), "coral is locked below its unlock");
    assert!(s.rocks.is_empty());

    s.score = 12_000 * MICRO; // clears coral's unlock and grants budget 2
    assert!(
        s.place_rock(1, 0, &p),
        "coral places once unlocked and in budget"
    );
}

/// The budget rises in steps with the score, and placement respects it.
#[test]
fn budget_grows_in_steps_and_gates_placement() {
    let p = Params::default();
    assert_eq!(p.budget(0), 1);
    assert_eq!(p.budget(12_000 * MICRO - 1), 1);
    assert_eq!(p.budget(12_000 * MICRO), 2);
    assert_eq!(p.budget(30_000 * MICRO - 1), 2);
    assert_eq!(p.budget(30_000 * MICRO), 3);
    assert_eq!(p.budget(75_000 * MICRO - 1), 3);
    assert_eq!(p.budget(75_000 * MICRO), 5);
    assert_eq!(p.budget(u128::MAX), 5);

    // At score 0 the budget is 1: one base rock places, a second does not.
    let mut s = State::new();
    assert!(s.place_rock(0, 0, &p));
    assert!(!s.place_rock(0, 1, &p), "budget 1 admits only one rock");
    assert_eq!(s.rocks.len(), 1);
}

/// Invariant — every kind is placeable: a kind costing more than the largest
/// budget the schedule ever grants can never be placed at any score, so it is
/// content no player can reach. Nothing else catches that. The sprite-cap test
/// enumerates only reachable reefs, so an unplaceable kind contributes no
/// population to bound, and no render test can build a state holding it — the
/// suite stays green while the kind sits dead in the manifest.
#[test]
fn every_kind_costs_within_the_largest_budget() {
    let p = Params::default();
    let max_budget = p
        .budget_steps
        .iter()
        .map(|&(_, budget)| budget)
        .max()
        .expect("a budget schedule");
    for rk in &p.rock_kinds {
        assert!(
            rk.cost <= max_budget,
            "{} costs {} while the budget never passes {max_budget}: it can never be placed",
            rk.name,
            rk.cost
        );
    }
}

/// The placement gates close once the run starts, and the seed is granted
/// exactly once per run — at start, and again on the next run after a new sea.
#[test]
fn placement_gates_close_at_start_and_seed_is_once_per_run() {
    let p = Params::default();
    let mut s = State::new();
    s.score = 30_000 * MICRO; // budget 3, every kind unlocked

    assert!(s.place_rock(0, 0, &p));
    assert!(s.place_rock(0, 1, &p));
    assert!(s.remove_rock(1), "removal works before start");
    assert_eq!(s.rocks.len(), 1);
    assert_eq!(s.currency, 0, "placement grants no currency");

    assert!(s.start_run(&p));
    assert_eq!(s.currency, p.seed_currency, "start grants the seed once");
    assert!(!s.start_run(&p), "cannot start twice");
    assert_eq!(s.currency, p.seed_currency, "no second seed");
    assert!(!s.place_rock(0, 2, &p), "no placing after start");
    assert!(!s.remove_rock(0), "no removing after start");

    // A new sea, a fresh placement, and the seed is granted again — once.
    s.reset();
    assert_eq!(s.currency, 0);
    assert!(s.place_rock(0, 0, &p));
    assert!(s.start_run(&p));
    assert_eq!(s.currency, p.seed_currency, "each run seeds exactly once");
}
