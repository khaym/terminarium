//! Save file handling: persistence and offline settlement. The engine stays
//! clockless — this module owns wall-clock time and injects it as ticks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::engine::{Params, State, SPECIES};

const VERSION: u32 = 1;

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
    format!(
        "v={VERSION}\nsaved_at={saved_at}\npopulation={populations}\npool={pools}\n\
         nutrient={}\ncollectable={}\ncurrency={}\n",
        state.nutrient, state.collectable, state.currency,
    )
}

/// Strict parse: every key present exactly once with the right shape, or
/// nothing — a half-read save must never masquerade as a valid one.
pub fn parse(text: &str) -> Option<(State, u64)> {
    let mut version: Option<u32> = None;
    let mut saved_at: Option<u64> = None;
    let mut population: Option<[u32; SPECIES]> = None;
    let mut pool: Option<[u128; SPECIES]> = None;
    let mut nutrient: Option<u128> = None;
    let mut collectable: Option<u128> = None;
    let mut currency: Option<u128> = None;

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
            _ => return None,
        }
    }

    if version? != VERSION {
        return None;
    }
    if population?.iter().any(|&n| n > MAX_POPULATION) {
        return None;
    }
    let state = State {
        population: population?,
        pool: pool?,
        nutrient: nutrient?,
        collectable: collectable?,
        currency: currency?,
    };
    Some((state, saved_at?))
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    if slot.is_some() {
        return None; // a duplicated key means the file is not ours
    }
    *slot = Some(value);
    Some(())
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
pub fn load(path: &Path, params: &Params, now: u64) -> State {
    let Ok(text) = fs::read_to_string(path) else {
        return State::new();
    };
    match parse(&text) {
        Some((mut state, saved_at)) => {
            state.advance(now.saturating_sub(saved_at), params);
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
    use crate::engine::MICRO;

    fn sample_state() -> State {
        State {
            population: [3, 1, 0, 0],
            pool: [52 * MICRO, 7, 0, 1],
            nutrient: 1_234_567,
            collectable: 42 * MICRO,
            currency: 120 * MICRO + 1,
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
        let (parsed, saved_at) = parse(&serialize(&state, 987_654_321)).expect("round trip");
        assert_eq!(parsed, state);
        assert_eq!(saved_at, 987_654_321);
    }

    #[test]
    fn load_settles_offline_progress() {
        let path = temp_save_path("settle");
        let params = Params::default();
        let state = sample_state();
        store(&path, &state, 1_000).expect("store");

        let settled = load(&path, &params, 1_300);

        let mut expected = state;
        expected.advance(300, &params);
        assert_eq!(settled, expected);
        cleanup(&path);
    }

    #[test]
    fn clock_skew_settles_nothing() {
        let path = temp_save_path("skew");
        let params = Params::default();
        let state = sample_state();
        store(&path, &state, 1_000).expect("store");

        assert_eq!(load(&path, &params, 500), state);
        cleanup(&path);
    }

    #[test]
    fn missing_file_starts_fresh() {
        let path = temp_save_path("missing");
        assert_eq!(load(&path, &Params::default(), 42), State::new());
    }

    #[test]
    fn corrupt_save_is_set_aside_not_destroyed() {
        let path = temp_save_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "v=1\nsaved_at=oops\n").unwrap();

        let state = load(&path, &Params::default(), 42);

        assert_eq!(state, State::new());
        assert!(!path.exists(), "corrupt file must be moved away");
        let bak = sibling(&path, ".bak.42");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "v=1\nsaved_at=oops\n");
        cleanup(&path);
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let good = serialize(&sample_state(), 1);
        assert!(parse(&format!("{good}currency=999\n")).is_none());
    }

    #[test]
    fn absurd_population_is_rejected() {
        let good = serialize(&sample_state(), 1);
        let hacked = good.replace("population=3,1,0,0", "population=3,1,0,4294967295");
        assert!(parse(&hacked).is_none());
        let plausible = good.replace("population=3,1,0,0", "population=3,1,0,600");
        assert!(parse(&plausible).is_some());
    }

    #[test]
    fn unknown_key_or_wrong_shape_is_rejected() {
        let good = serialize(&sample_state(), 1);
        assert!(parse(&good).is_some());
        assert!(parse(&good.replace("v=1", "v=2")).is_none());
        assert!(parse(&format!("{good}extra=1\n")).is_none());
        assert!(parse(&good.replace("population=3,1,0,0", "population=3,1,0")).is_none());
        assert!(parse(&good.replace("population=3,1,0,0", "population=3,1,0,0,9")).is_none());
        assert!(parse("").is_none());
    }
}
