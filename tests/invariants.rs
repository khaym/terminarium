//! Invariant tests — the executable form of the business rules fixed in
//! work/economy-model.md. Parameter values are placeholders; these tests are
//! the constraints they must keep satisfying while being tuned.

use tui_game::engine::{Params, Species, State, MICRO};

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

/// Invariant 1 — conservation: photosynthesis is the only thing that creates
/// living matter. Exact equality, every tick.
#[test]
fn conservation_per_tick() {
    let p = Params::default();
    let mut s = populated_state();
    for _ in 0..10_000 {
        let before = s.biomass();
        s.tick(&p);
        let created = u128::from(s.population[0]) * p.photosynthesis;
        assert_eq!(s.biomass(), before + created);
    }
}

/// Invariant 1 (operations) — feed adds exactly F; collect moves matter to
/// currency without creating or destroying any.
#[test]
fn conservation_across_operations() {
    let p = Params::default();
    let mut s = populated_state();

    let before = s.biomass();
    s.feed(&p);
    assert_eq!(s.biomass(), before + p.feed_amount);

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
        for t in 0..600u64 {
            if t < 60 {
                s.feed(&p);
                s.feed(&p);
            }
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

/// Invariant 5 — automation before the first wall: mashing the feed key
/// (2 feeds/second) affords the first algae within the 60-second manual phase.
#[test]
fn first_automation_within_manual_phase() {
    let p = Params::default();
    let mut s = State::new();
    let mut seconds = 0u64;
    loop {
        s.feed(&p);
        s.feed(&p);
        s.tick(&p);
        seconds += 1;
        s.collect();
        if s.buy(Species::Algae, &p) {
            break;
        }
        assert!(
            seconds < 60,
            "first algae must be affordable within 60s of feeding"
        );
    }
    println!("first algae bought after {seconds}s of manual feeding");
}

/// Invariant 6 — first wall distance: from the first algae onward, the
/// baseline play (collect on every 90s peek, no extra purchases) reaches the
/// first plankton on peek 2 or 3.
#[test]
fn first_wall_at_two_to_three_peeks() {
    let p = Params::default();
    const PEEK_SECONDS: u64 = 90;

    let mut s = State::new();
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
