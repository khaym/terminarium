//! Invariant tests — the executable form of the business rules fixed in
//! work/economy-model.md. Parameter values are placeholders; these tests are
//! the constraints they must keep satisfying while being tuned. The two
//! exceptions are frozen rather than tunable: the ecosystem's return w, and the
//! collection band each placement cost buys (`COLLECTION_BAND`).

use terminarium::engine::{Params, ReefKind, Species, State, ANCHOR_POS_MAX, MICRO};

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

/// Invariant 1 — conservation: with reefs placed, three things and only three
/// create living matter — photosynthesis, reef output, and the ecosystem's
/// return (what the life above the algae hands back, in proportion to the reef
/// it lives in). Exact equality, every tick.
#[test]
fn conservation_per_tick() {
    let p = Params::default();
    let rock = kind_index("rock");
    let mut s = populated_state();
    assert!(s.place_reef(rock, 0, &p));
    assert!(s.start_run(&p));

    // One reef, and it houses every tier of this tank — so every individual
    // above the algae returns that one reef's cost, with no housing choice to
    // make.
    let housed: u32 = s.population[1..].iter().sum();
    for i in 1..4 {
        assert!(s.population[i] <= p.reef_kinds[rock].capacity[i]);
    }
    let returned = p
        .ecosystem_return
        .apply(u128::from(housed) * u128::from(p.reef_kinds[rock].cost) * MICRO);
    assert!(returned > 0, "the third source must actually be live here");

    for _ in 0..10_000 {
        let before = s.biomass();
        s.tick(&p);
        let created =
            u128::from(s.population[0]) * p.photosynthesis + p.reef_kinds[rock].output + returned;
        assert_eq!(s.biomass(), before + created);
    }
}

/// Invariant 1 (operations) — place allocates budget without touching biomass;
/// collect moves matter to currency without creating or destroying any. The
/// ecosystem's return is ordinary surplus once it lands: a tick's worth of it
/// sits in the same collectable pile and leaves on the same collect, so the
/// third source adds income without adding a resource to keep track of.
#[test]
fn conservation_across_operations() {
    let p = Params::default();
    let mut s = populated_state();

    let before = s.biomass();
    assert!(s.place_reef(kind_index("rock"), 0, &p));
    assert_eq!(s.biomass(), before, "placement moves no biomass");

    assert!(s.start_run(&p));
    let pile = s.collectable;
    s.tick(&p);
    assert!(
        s.collectable > pile,
        "a tick of the three sources adds to the collectable pile"
    );

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
    assert!(whole.place_reef(0, 0, &p));
    assert!(split.place_reef(0, 0, &p));
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
        s.place_reef(0, 0, &p);
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
    assert!(s.place_reef(0, 0, &p));
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
            "first algae must be affordable within 15s of reef output"
        );
    }
    println!("first algae bought after {seconds}s of reef output");
}

/// Invariant 6 — first wall distance: from one base rock plus one algae, the
/// baseline play (collect on every 90s peek, no extra purchases) reaches the
/// first plankton on peek 2 or 3.
#[test]
fn first_wall_at_two_to_three_peeks() {
    let p = Params::default();
    const PEEK_SECONDS: u64 = 90;

    let mut s = State::new();
    assert!(s.place_reef(0, 0, &p));
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
/// feeds the algae, collectables keep accruing at the rate the three sources
/// create matter, and no pool grows without bound.
///
/// A filled rock sea is the tank this is read on, because all three sources are
/// live in it at once — photosynthesis from its algae, output from the reef
/// itself, and the return from the three tiers it houses above the algae.
#[test]
fn detritus_cycle_sustains_and_stays_bounded() {
    let p = Params::default();
    let rock = kind_index("rock");
    let mut s = State::new();
    assert!(s.place_reef(rock, 0, &p));
    assert!(s.start_run(&p));
    for i in 0..4 {
        s.population[i] = s.capacity(i, &p);
    }

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
    // converges to what the three sources create (they are the only sources,
    // and collection is the only sink) — allow 20% tolerance for the transient
    // and integer rounding.
    let window_gain = s.collectable - mid_collectable;
    let housed: u128 = s.population[1..].iter().map(|&n| u128::from(n)).sum();
    let per_tick = u128::from(s.population[0]) * p.photosynthesis
        + p.reef_kinds[rock].output
        + p.ecosystem_return
            .apply(housed * u128::from(p.reef_kinds[rock].cost) * MICRO);
    let created = 10_000u128 * per_tick;
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
    assert!(s.place_reef(0, 0, &p));
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
    assert!(s.place_reef(0, 0, &p));
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
    assert!(s.place_reef(kelp, 0, &p));
    assert!(s.start_run(&p));

    assert_eq!(
        s.tick_count, 0,
        "no time has passed since the new sea began"
    );
    assert_eq!(
        s.capacity(algae, &p),
        p.reef_kinds[kelp].capacity[algae],
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

/// Invariant 10 — housing is immediate: a placed reef's capacity counts from
/// the moment the run starts and never depends on elapsed time. (#22 removed
/// the emergence delay — placement is the only thing that gates housing, so a
/// new sea has full capacity at tick 0.)
#[test]
fn housing_counts_from_placement_regardless_of_time() {
    let p = Params::default();
    let mut s = State::new();
    assert!(s.place_reef(0, 0, &p)); // base rock, housing [4, 3, 2, 1]
    assert!(s.start_run(&p));

    let expected: Vec<u32> = (0..4).map(|i| p.reef_kinds[0].capacity[i]).collect();

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

/// The index a kind's name resolves to in `Params::default().reef_kinds` — which
/// is also `Reef::kind`. A reef a test names is looked up here rather than
/// written as a literal, since the manifest is append-only and a kind joining it
/// must not silently renumber the reef another test meant.
fn kind_index(name: &str) -> usize {
    Params::default()
        .reef_kinds
        .iter()
        .position(|k| k.name == name)
        .unwrap_or_else(|| panic!("no such reef kind: {name}"))
}

/// Fill every species to the housing the given reef provides, then converge and
/// measure the collectable gained over a window — the steady collection rate of
/// a filled tank.
fn steady_collection_over_window(reefs: &[(usize, u8)], score: u128, window: u64) -> u128 {
    let p = Params::default();
    let mut s = State::new();
    s.score = score;
    for &(kind, slot) in reefs {
        assert!(
            s.place_reef(kind, slot, &p),
            "reef {reefs:?} must place within unlock/budget at score {score}"
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

/// The frozen collection band (work/economy-model.md, "コスト帯ルール"): what one
/// filled reef collects per tick at steady state, per placement cost, in MICRO.
/// The design table, not a measurement — the test below measures against it.
const COLLECTION_BAND: [(u32, u128, u128); 3] = [
    (1, 10 * MICRO, 12 * MICRO),
    (2, 26 * MICRO, 30 * MICRO),
    (3, 39 * MICRO, 45 * MICRO),
];

/// The band a reef of this cost is held to.
fn band_for(cost: u32) -> (u128, u128) {
    COLLECTION_BAND
        .iter()
        .find(|&&(c, _, _)| c == cost)
        .map(|&(_, lo, hi)| (lo, hi))
        .unwrap_or_else(|| panic!("no collection band for a cost-{cost} reef"))
}

/// The score at which a kind can first be placed: its own unlock, or the first
/// budget step that affords its cost, whichever comes later. Read off the
/// schedule rather than written out, so retuning either moves with it.
fn placeable_at(kind: &ReefKind, p: &Params) -> u128 {
    let affords = p
        .budget_steps
        .iter()
        .filter(|&&(_, budget)| budget >= kind.cost)
        .map(|&(threshold, _)| threshold)
        .min()
        .unwrap_or_else(|| panic!("{} costs more than any budget step", kind.name));
    affords.max(kind.unlock)
}

/// Invariant 13 — the collection band: every kind the game ships, filled to its
/// own housing and converged, collects at a rate inside the band its placement
/// cost buys. The bands are disjoint and ascending, so what a reef costs tells
/// the player what it earns, and a costlier reef always out-collects a cheaper
/// one however its housing is shaped.
///
/// This is the rule the pair of lagoon-vs-coral / lagoon-vs-kelp asserts used to
/// state one kind at a time, and it states it for every kind at once — including
/// the kinds added after this test was written, since it walks the manifest.
/// Before the ecosystem's return (#30) it could not be written: steady collection
/// tracked algae housing and reef output almost alone, so a cost-3 grotto (2
/// algae) collected exactly what a cost-2 coral did and the cost-3 band was
/// empty. The third source is what puts a reef's upper tiers into its rate and so
/// lets the cost band mean something.
///
/// The band is frozen (`COLLECTION_BAND`); the weight w behind it is frozen in
/// `Params::ecosystem_return`. Retuning either — w, an output, a capacity — is
/// meant to turn this red: the numbers are a design agreement, and moving one
/// asks a human to re-agree it rather than to re-bless a measurement.
#[test]
fn every_kind_collects_inside_the_band_its_cost_buys() {
    const WINDOW: u64 = 5_000;
    let p = Params::default();

    for (i, kind) in p.reef_kinds.iter().enumerate() {
        let (lo, hi) = band_for(kind.cost);
        let collected = steady_collection_over_window(&[(i, 0)], placeable_at(kind, &p), WINDOW);
        let (floor, ceiling) = (lo * u128::from(WINDOW), hi * u128::from(WINDOW));
        let rate = collected as f64 / WINDOW as f64 / MICRO as f64;
        let (lo_f, hi_f) = (lo as f64 / MICRO as f64, hi as f64 / MICRO as f64);
        let outside = if collected < floor {
            (rate - lo_f) / lo_f * 100.0
        } else if collected > ceiling {
            (rate - hi_f) / hi_f * 100.0
        } else {
            0.0
        };
        println!(
            "{:<8} cost {} -> {rate:7.3}/tick   band [{lo_f:.1}, {hi_f:.1}]",
            kind.name, kind.cost
        );
        assert!(
            (floor..=ceiling).contains(&collected),
            "{} (cost {}) collects {rate:.3}/tick, outside its band [{lo_f:.1}, {hi_f:.1}] \
             by {outside:+.1}% (w = {}/{})",
            kind.name,
            kind.cost,
            p.ecosystem_return.num,
            p.ecosystem_return.den
        );
    }
}

/// Invariant 13 (housing) — an individual returns what the reef it lives in
/// costs, and which reef that is never depends on the order the player dropped
/// them. Housing is not stored: the tick settles it by filling the costliest
/// reef with room first, so the same reefs and the same populations always
/// return the same, whichever slot went down first. (Housed by placement order
/// instead, the sea below would return 1.8 or 3.6 per tick depending only on
/// which key was pressed first — a difference the player could neither see nor
/// intend.)
#[test]
fn an_individual_returns_what_the_reef_it_lives_in_costs() {
    let p = Params::default();
    let (rock, coral) = (kind_index("rock"), kind_index("coral"));

    // One coral beside one rock, composed both ways round, holding three
    // plankton — fewer than the coral alone houses (6), so where they live is a
    // real choice rather than a foregone one.
    let sea = |placements: [(usize, u8); 2]| {
        let mut s = State::new();
        s.score = 30_000 * MICRO; // budget 3 seats the coral (2) beside the rock (1)
        for &(kind, slot) in &placements {
            assert!(s.place_reef(kind, slot, &p));
        }
        assert!(s.start_run(&p));
        s.population[Species::Plankton as usize] = 3;
        s.tick(&p);
        s.collectable
    };

    // Both seas shed the same reef output and settle the same share of it; the
    // only thing that could differ is where the three plankton are housed, and
    // the costliest reef with room takes them either way.
    let shed = p.reef_kinds[rock].output + p.reef_kinds[coral].output;
    let settled = shed - p.recycle.apply(shed);
    let returned = p
        .ecosystem_return
        .apply(3 * u128::from(p.reef_kinds[coral].cost) * MICRO);

    assert_eq!(
        sea([(coral, 0), (rock, 1)]),
        settled + returned,
        "three plankton in a coral+rock sea return three coral heads' worth"
    );
    assert_eq!(
        sea([(rock, 0), (coral, 1)]),
        settled + returned,
        "and the same sea placed rock-first returns exactly the same"
    );
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
fn steady_living_biomass(reefs: &[(usize, u8)], score: u128) -> u128 {
    let p = Params::default();
    let mut s = State::new();
    s.score = score;
    for &(kind, slot) in reefs {
        assert!(
            s.place_reef(kind, slot, &p),
            "reef {reefs:?} must place within unlock/budget at score {score}"
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
    //
    // Which of them wins is not this test's business, but it did change hands
    // with the ecosystem's return (#30): the best budget-3 sea is now a single
    // kelp (41.84/tick) rather than three rocks, because a reef's upper tiers
    // finally pay and one cost-3 reef returns more per head than three cost-1
    // ones. The assert is the same either way — budget 5 must beat all of them.
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
    assert!(s.place_reef(0, 0, &p));
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
    assert!(s.place_reef(0, 0, &p));
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
    assert!(s.reefs.is_empty());
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
        assert!(s.place_reef(0, 0, &p));
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
    assert!(!s.place_reef(1, 0, &p), "coral is locked below its unlock");
    assert!(s.reefs.is_empty());

    s.score = 12_000 * MICRO; // clears coral's unlock and grants budget 2
    assert!(
        s.place_reef(1, 0, &p),
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
    assert!(s.place_reef(0, 0, &p));
    assert!(!s.place_reef(0, 1, &p), "budget 1 admits only one reef");
    assert_eq!(s.reefs.len(), 1);
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
    for rk in &p.reef_kinds {
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

    assert!(s.place_reef(0, 0, &p));
    assert!(s.place_reef(0, 1, &p));
    assert!(s.remove_reef(1), "removal works before start");
    assert_eq!(s.reefs.len(), 1);
    assert_eq!(s.currency, 0, "placement grants no currency");

    assert!(s.start_run(&p));
    assert_eq!(s.currency, p.seed_currency, "start grants the seed once");
    assert!(!s.start_run(&p), "cannot start twice");
    assert_eq!(s.currency, p.seed_currency, "no second seed");
    assert!(!s.place_reef(0, 2, &p), "no placing after start");
    assert!(!s.remove_reef(0), "no removing after start");

    // A new sea, a fresh placement, and the seed is granted again — once.
    s.reset();
    assert_eq!(s.currency, 0);
    assert!(s.place_reef(0, 0, &p));
    assert!(s.start_run(&p));
    assert_eq!(s.currency, p.seed_currency, "each run seeds exactly once");
}
