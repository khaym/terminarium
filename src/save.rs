//! Save file handling: persistence and offline settlement. The engine stays
//! clockless — this module owns wall-clock time and injects it as ticks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::engine::{Params, Rock, State, ANCHOR_POS_MAX, SLOTS, SPECIES};

const VERSION: u32 = 4;

/// Sanity bound on parsed populations. Legitimate play tops out near ~620 per
/// species (the cost curve saturates u128 there), while HUD cost display is
/// O(population) per frame — an absurd hand-edited count would freeze the
/// game layer, so such a file is treated as corrupt.
const MAX_POPULATION: u32 = 100_000;

/// `$XDG_DATA_HOME` (or `~/.local/share`) `/tui-game/save.txt`. The directory
/// follows the working package name and moves when the game gets its real
/// name.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local/share")
        });
    base.join("tui-game/save.txt")
}

/// Which save file to use for the given time scale. Real time (scale 1) uses
/// the real save; any fast-forward is a throwaway test run and gets its own
/// sibling file so it never clobbers a genuine save.
pub fn path_for(time_scale: u64) -> PathBuf {
    let path = default_path();
    if time_scale == 1 {
        path
    } else {
        path.with_file_name("save.test.txt")
    }
}

pub fn serialize(state: &State, saved_at: u64) -> String {
    let pools = state
        .pool
        .iter()
        .map(u128::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let populations = state
        .population
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let rocks = state
        .rocks
        .iter()
        .map(|r| format!("{}:{}", r.kind, r.slot))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "v={VERSION}\nsaved_at={saved_at}\npopulation={populations}\npool={pools}\n\
         nutrient={}\ncollectable={}\ncurrency={}\nscore={}\nrocks={rocks}\nage={}\nstarted={}\n\
         anchor={}\n",
        state.nutrient,
        state.collectable,
        state.currency,
        state.score,
        state.tick_count,
        u8::from(state.started),
        state.anchor_pos,
    )
}

/// Strict parse: every key present exactly once with the right shape, or
/// nothing — a half-read save must never masquerade as a valid one. Rock
/// validity is checked against `params` so a hand-edited kind cannot index out
/// of the rock table at load time.
pub fn parse(text: &str, params: &Params) -> Option<(State, u64)> {
    let mut version: Option<u32> = None;
    let mut saved_at: Option<u64> = None;
    let mut population: Option<[u32; SPECIES]> = None;
    let mut pool: Option<[u128; SPECIES]> = None;
    let mut nutrient: Option<u128> = None;
    let mut collectable: Option<u128> = None;
    let mut currency: Option<u128> = None;
    let mut score: Option<u128> = None;
    let mut rocks: Option<Vec<Rock>> = None;
    let mut tick_count: Option<u64> = None;
    let mut started: Option<bool> = None;
    let mut anchor_pos: Option<u16> = None;

    for line in text.lines() {
        let (key, value) = line.split_once('=')?;
        match key {
            "v" => set_once(&mut version, value.parse().ok()?)?,
            "saved_at" => set_once(&mut saved_at, value.parse().ok()?)?,
            "population" => set_once(&mut population, parse_array(value)?)?,
            "pool" => set_once(&mut pool, parse_array(value)?)?,
            "nutrient" => set_once(&mut nutrient, value.parse().ok()?)?,
            "collectable" => set_once(&mut collectable, value.parse().ok()?)?,
            "currency" => set_once(&mut currency, value.parse().ok()?)?,
            "score" => set_once(&mut score, value.parse().ok()?)?,
            "rocks" => set_once(&mut rocks, parse_rocks(value, params)?)?,
            "age" => set_once(&mut tick_count, value.parse().ok()?)?,
            "started" => set_once(&mut started, parse_flag(value)?)?,
            "anchor" => set_once(&mut anchor_pos, value.parse().ok()?)?,
            _ => return None,
        }
    }

    if version? != VERSION {
        return None;
    }
    if population?.iter().any(|&n| n > MAX_POPULATION) {
        return None;
    }
    // A position is a millipermille in 0..=999; anything past the pane could not
    // arise in play, so it is corruption.
    if anchor_pos? > ANCHOR_POS_MAX {
        return None;
    }
    let state = State {
        population: population?,
        pool: pool?,
        nutrient: nutrient?,
        collectable: collectable?,
        currency: currency?,
        score: score?,
        rocks: rocks?,
        tick_count: tick_count?,
        started: started?,
        anchor_pos: anchor_pos?,
    };
    // Reject forms a legitimate playthrough could not produce: a rock of a kind
    // the score has not unlocked, a placement over the score's budget, or a
    // clock that ran without the run having started.
    if state
        .rocks
        .iter()
        .any(|r| params.rock_kinds[r.kind].unlock > state.score)
    {
        return None;
    }
    let placed_cost: u32 = state
        .rocks
        .iter()
        .map(|r| params.rock_kinds[r.kind].cost)
        .sum();
    if placed_cost > params.budget(state.score) {
        return None;
    }
    if !state.started && state.tick_count > 0 {
        return None;
    }
    if state.started && state.rocks.is_empty() {
        return None; // starting requires a rock, and removal ends at start
    }
    Some((state, saved_at?))
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    if slot.is_some() {
        return None; // a duplicated key means the file is not ours
    }
    *slot = Some(value);
    Some(())
}

/// A 0/1 flag; anything else means the file is not ours.
fn parse_flag(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_array<T: std::str::FromStr + Copy + Default>(value: &str) -> Option<[T; SPECIES]> {
    let mut out = [T::default(); SPECIES];
    let mut parts = value.split(',');
    for slot in &mut out {
        *slot = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

/// `kind:slot,kind:slot,...`; empty value is an empty list. Rejects any rock a
/// live run could not have produced: an out-of-range kind, a slot at or beyond
/// `SLOTS`, a duplicated slot, or more rocks than there are slots.
fn parse_rocks(value: &str, params: &Params) -> Option<Vec<Rock>> {
    if value.is_empty() {
        return Some(Vec::new());
    }
    let mut rocks = Vec::new();
    let mut used = [false; SLOTS as usize];
    for part in value.split(',') {
        let (kind, slot) = part.split_once(':')?;
        let kind: usize = kind.parse().ok()?;
        let slot: u8 = slot.parse().ok()?;
        if kind >= params.rock_kinds.len() || slot >= SLOTS {
            return None;
        }
        let seat = &mut used[usize::from(slot)];
        if *seat {
            return None; // a duplicated slot means the file is not ours
        }
        *seat = true;
        rocks.push(Rock { kind, slot });
    }
    Some(rocks)
}

/// Write atomically (tmp + rename) so a crash mid-write cannot corrupt the
/// previous save.
pub fn store(path: &Path, state: &State, now: u64) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = sibling(path, ".tmp");
    fs::write(&tmp, serialize(state, now))?;
    fs::rename(&tmp, path)
}

/// Load and settle the absence. Missing file → fresh start. A file that
/// fails to parse is our own corruption (internal origin) → set it aside as
/// .bak and degrade to a fresh start rather than erroring or destroying data.
///
/// `time_scale` is the fast-mode multiplier — this is the offline counterpart
/// of `App::on_elapsed`, the other place wall-clock time enters the engine.
/// The elapsed seconds are scaled (saturating) so the same knob speeds up both
/// live ticking and settlement; 1 is real time.
pub fn load(path: &Path, params: &Params, now: u64, time_scale: u64) -> State {
    let Ok(text) = fs::read_to_string(path) else {
        return State::new();
    };
    match parse(&text, params) {
        Some((mut state, saved_at)) => {
            // Before the first rock the clock does not run, so an absence
            // settles nothing (see State::run_started).
            if state.run_started() {
                let elapsed = now.saturating_sub(saved_at).saturating_mul(time_scale);
                state.advance(elapsed, params);
            }
            state
        }
        None => {
            // Timestamped so a second corruption never overwrites the (likely
            // more valuable) earlier backup.
            let _ = fs::rename(path, sibling(path, &format!(".bak.{now}")));
            State::new()
        }
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{RockKind, MICRO};

    fn sample_state() -> State {
        State {
            population: [3, 1, 0, 0],
            pool: [52 * MICRO, 7, 0, 1],
            nutrient: 1_234_567,
            collectable: 42 * MICRO,
            currency: 120 * MICRO + 1,
            // Two base rocks (Σcost 2) need budget 2, so the score must clear
            // the first budget step; the clock has run, so the run is started.
            score: 12_000 * MICRO,
            rocks: vec![Rock { kind: 0, slot: 0 }, Rock { kind: 0, slot: 2 }],
            tick_count: 137,
            started: true,
            // A non-default position, off the anchor's home column, so the
            // round trip exercises a value that is neither 0 nor the default.
            anchor_pos: 250,
        }
    }

    /// Each test gets its own directory so parallel tests never collide.
    fn temp_save_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("tui-game-save-tests-{}", std::process::id()))
            .join(name)
            .join("save.txt")
    }

    fn cleanup(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn round_trip_is_exact() {
        let state = sample_state();
        let (parsed, saved_at) =
            parse(&serialize(&state, 987_654_321), &Params::default()).expect("round trip");
        assert_eq!(parsed, state);
        assert_eq!(saved_at, 987_654_321);
    }

    #[test]
    fn load_settles_offline_progress() {
        let path = temp_save_path("settle");
        let params = Params::default();
        let state = sample_state();
        store(&path, &state, 1_000).expect("store");

        let settled = load(&path, &params, 1_300, 1);

        let mut expected = state;
        expected.advance(300, &params);
        assert_eq!(settled, expected);
        cleanup(&path);
    }

    #[test]
    fn load_scales_offline_progress() {
        let path = temp_save_path("scaled");
        let params = Params::default();
        let state = sample_state();
        store(&path, &state, 1_000).expect("store");

        // 10 wall-clock seconds at 60x settle as 600 ticks.
        let settled = load(&path, &params, 1_010, 60);

        let mut expected = state;
        expected.advance(600, &params);
        assert_eq!(settled, expected);
        cleanup(&path);
    }

    #[test]
    fn test_scale_uses_a_sibling_save_file() {
        let real = path_for(1);
        assert_eq!(real, default_path());
        assert_eq!(real.file_name().unwrap(), "save.txt");

        let test = path_for(60);
        assert_eq!(test.file_name().unwrap(), "save.test.txt");
        assert_eq!(
            test.parent(),
            real.parent(),
            "the test save sits beside the real one"
        );
    }

    #[test]
    fn clock_skew_settles_nothing() {
        let path = temp_save_path("skew");
        let params = Params::default();
        let state = sample_state();
        store(&path, &state, 1_000).expect("store");

        assert_eq!(load(&path, &params, 500, 1), state);
        cleanup(&path);
    }

    #[test]
    fn pre_run_save_is_not_settled() {
        let path = temp_save_path("prerun");
        let params = Params::default();
        // No rocks placed yet: the run has not started, so an absence must
        // settle nothing even though this pool would otherwise decay away.
        let mut state = State::new();
        state.pool[0] = 10 * MICRO;
        store(&path, &state, 1_000).expect("store");

        assert_eq!(load(&path, &params, 1_000_000, 1), state);
        cleanup(&path);
    }

    #[test]
    fn placement_in_progress_save_is_not_settled() {
        let path = temp_save_path("placing");
        let params = Params::default();
        // Rocks placed but the run not yet committed (started=0): the clock is
        // not running, so an absence settles nothing — the player returns to
        // the placement screen with the reef intact.
        let mut state = State::new();
        assert!(state.place_rock(0, 0, &params));
        assert!(!state.run_started());
        store(&path, &state, 1_000).expect("store");

        assert_eq!(load(&path, &params, 1_000_000, 1), state);
        cleanup(&path);
    }

    #[test]
    fn missing_file_starts_fresh() {
        let path = temp_save_path("missing");
        assert_eq!(load(&path, &Params::default(), 42, 1), State::new());
    }

    #[test]
    fn corrupt_save_is_set_aside_not_destroyed() {
        let path = temp_save_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "v=1\nsaved_at=oops\n").unwrap();

        let state = load(&path, &Params::default(), 42, 1);

        assert_eq!(state, State::new());
        assert!(!path.exists(), "corrupt file must be moved away");
        let bak = sibling(&path, ".bak.42");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "v=1\nsaved_at=oops\n");
        cleanup(&path);
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let good = serialize(&sample_state(), 1);
        assert!(parse(&format!("{good}currency=999\n"), &Params::default()).is_none());
    }

    #[test]
    fn absurd_population_is_rejected() {
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        let hacked = good.replace("population=3,1,0,0", "population=3,1,0,4294967295");
        assert!(parse(&hacked, &p).is_none());
        let plausible = good.replace("population=3,1,0,0", "population=3,1,0,600");
        assert!(parse(&plausible, &p).is_some());
    }

    #[test]
    fn tampered_rocks_are_rejected() {
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good, &p).is_some());
        // Kind out of range (the default table has kinds 0..=2).
        assert!(parse(&good.replace("rocks=0:0,0:2", "rocks=3:0"), &p).is_none());
        // Slot at or beyond SLOTS.
        assert!(parse(&good.replace("rocks=0:0,0:2", "rocks=0:0,0:9"), &p).is_none());
        // Duplicated slot.
        assert!(parse(&good.replace("rocks=0:0,0:2", "rocks=0:0,0:0"), &p).is_none());
        // Malformed pair (missing the kind:slot separator).
        assert!(parse(&good.replace("rocks=0:0,0:2", "rocks=00,0:2"), &p).is_none());
    }

    #[test]
    fn old_five_slot_saves_still_load() {
        // A save written under the old 5-slot grid (slot indices 0..=4) stays
        // valid under the 9-slot grid: the indices are still in range, so the
        // reef is read back intact. The renderer maps those indices onto the
        // denser grid, so the reef shows further left than before — an accepted
        // position shift (the save stores the slot index, not a pane column),
        // not a load failure. #17 does not re-map old slots.
        let p = Params::default();
        let old = State {
            // Budget 2 (score past the first step) admits the two base rocks.
            score: 12_000 * MICRO,
            rocks: vec![Rock { kind: 0, slot: 0 }, Rock { kind: 0, slot: 4 }],
            tick_count: 42,
            started: true,
            ..State::new()
        };
        let (parsed, _) =
            parse(&serialize(&old, 1), &p).expect("a save from the old 5-slot grid still loads");
        assert_eq!(
            parsed.rocks, old.rocks,
            "slot indices 0..=4 survive the 5->9 slot increase unchanged"
        );
    }

    #[test]
    fn v3_saves_are_rejected() {
        // The version is a breaking bump to v4 for the anchor position: a v3
        // save (which had no anchor key) must be set aside, not read as if it
        // were current — reading it as v4 would invent a position it never held.
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good, &p).is_some());
        assert!(parse(&good.replace("v=4", "v=3"), &p).is_none());
    }

    #[test]
    fn anchor_position_out_of_range_is_rejected() {
        // The anchor position is a millipermille, 0..=999: 999 is the last valid
        // column-fraction, 1000 is past the pane and cannot arise in play.
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good, &p).is_some());
        assert!(parse(&good.replace("anchor=250", "anchor=999"), &p).is_some());
        assert!(parse(&good.replace("anchor=250", "anchor=1000"), &p).is_none());
    }

    #[test]
    fn a_save_missing_the_anchor_key_is_rejected() {
        // Every key is required (the strict all-or-nothing rule): a save without
        // the anchor position is not one of ours.
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        let without = good
            .lines()
            .filter(|l| !l.starts_with("anchor="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse(&without, &p).is_none());
    }

    #[test]
    fn a_rock_locked_above_the_score_is_rejected() {
        // A cost-1 kind (always within budget 1) that only unlocks at 5,000:
        // this isolates the unlock check from the budget check.
        let p = Params {
            rock_kinds: vec![
                RockKind {
                    name: "base",
                    cost: 1,
                    unlock: 0,
                    output: MICRO,
                    delay: 0,
                    capacity: [1, 0, 0, 0],
                },
                RockKind {
                    name: "gated",
                    cost: 1,
                    unlock: 5_000 * MICRO,
                    output: MICRO,
                    delay: 0,
                    capacity: [1, 0, 0, 0],
                },
            ],
            budget_steps: vec![(0, 1)],
            ..Params::default()
        };
        // Score 0: the gated kind is not yet unlocked → rejected.
        let locked = State {
            rocks: vec![Rock { kind: 1, slot: 0 }],
            ..State::new()
        };
        assert!(parse(&serialize(&locked, 1), &p).is_none());
        // Score past the unlock (and within budget): accepted.
        let cleared = State {
            score: 5_000 * MICRO,
            rocks: vec![Rock { kind: 1, slot: 0 }],
            ..State::new()
        };
        assert!(parse(&serialize(&cleared, 1), &p).is_some());
    }

    #[test]
    fn rocks_over_budget_are_rejected() {
        // Score 12,000 unlocks coral and grants budget 2. A rock (cost 1) plus a
        // coral (cost 2) is Σcost 3 — over budget — although both kinds are
        // unlocked, so only the budget check catches it.
        let p = Params::default();
        let over = State {
            score: 12_000 * MICRO,
            started: true,
            rocks: vec![Rock { kind: 0, slot: 0 }, Rock { kind: 1, slot: 1 }],
            tick_count: 5,
            ..State::new()
        };
        assert!(parse(&serialize(&over, 1), &p).is_none());
    }

    #[test]
    fn a_started_run_without_rocks_is_rejected() {
        // start_run demands at least one rock and removal ends at start, so a
        // started run with an empty reef cannot arise in play. A fresh save
        // (not started, empty reef) stays valid.
        let p = Params::default();
        let started_empty = State {
            started: true,
            ..State::new()
        };
        assert!(parse(&serialize(&started_empty, 1), &p).is_none());
        assert!(parse(&serialize(&State::new(), 1), &p).is_some());
    }

    #[test]
    fn a_started_flag_contradicting_the_clock_is_rejected() {
        // The clock only runs after start, so started=0 with a non-zero clock
        // cannot arise in play.
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good, &p).is_some());
        assert!(parse(&good.replace("started=1", "started=0"), &p).is_none());
        // A bare flag value that is neither 0 nor 1 is not ours.
        assert!(parse(&good.replace("started=1", "started=yes"), &p).is_none());
    }

    #[test]
    fn unknown_key_or_wrong_shape_is_rejected() {
        let p = Params::default();
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good, &p).is_some());
        assert!(parse(&good.replace("v=4", "v=1"), &p).is_none());
        assert!(parse(&format!("{good}extra=1\n"), &p).is_none());
        assert!(parse(&good.replace("population=3,1,0,0", "population=3,1,0"), &p).is_none());
        assert!(parse(
            &good.replace("population=3,1,0,0", "population=3,1,0,0,9"),
            &p
        )
        .is_none());
        assert!(parse(&good.replace("age=137", "age=nan"), &p).is_none());
        assert!(parse("", &p).is_none());
    }
}
