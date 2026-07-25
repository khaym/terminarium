//! Run a scenario and dump the frames it captures as color ANSI files.
//!
//!     cargo run --example capture -- tests/scenarios/demo.txt [--out <dir>]
//!
//! The scripted play, the expectations, and the ANSI serialization all live in
//! `terminarium::harness`; this wrapper only does the I/O the library refuses to:
//! read the scenario, write `<label>-<W>x<H>.ans` under the output directory
//! (`work/captures/<scenario stem>/` by default), and report the verdict.
//!
//! An example rather than a subcommand of the game, because examples are not
//! part of a `cargo install` — the harness ships with the repo, never with the
//! installed binary.
//!
//! Exit codes follow the game's own rule that a caller's mistake surfaces:
//! 0 when every expectation was met, 1 when the run missed one (the captures are
//! still written — that frame is how a human sees why), 2 when the scenario or
//! the command line was wrong.

use std::fs;
use std::path::{Path, PathBuf};

use terminarium::harness;

const USAGE: &str = "usage: cargo run --example capture -- <scenario.txt> [--out <dir>]";

/// Where captures land unless `--out` says otherwise.
const DEFAULT_OUT_ROOT: &str = "work/captures";

const EXIT_UNMET: i32 = 1;
const EXIT_MISUSE: i32 = 2;

struct Options {
    scenario: PathBuf,
    out: Option<PathBuf>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(problem) => misuse(&format!("{problem}\n{USAGE}")),
    };
    let scenario_path = options.scenario;

    let text = match fs::read_to_string(&scenario_path) {
        Ok(text) => text,
        Err(e) => misuse(&format!("cannot read {}: {e}", scenario_path.display())),
    };
    let scenario = match harness::parse(&text) {
        Ok(scenario) => scenario,
        Err(e) => misuse(&format!("{}: {e}", scenario_path.display())),
    };

    let outcome = harness::run(&scenario);

    let out = options.out.unwrap_or_else(|| default_out(&scenario_path));
    if let Err(e) = fs::create_dir_all(&out) {
        misuse(&format!("cannot create {}: {e}", out.display()));
    }
    for capture in &outcome.captures {
        let path = out.join(format!(
            "{}-{}x{}.ans",
            capture.label, capture.width, capture.height
        ));
        if let Err(e) = fs::write(&path, &capture.ansi) {
            misuse(&format!("cannot write {}: {e}", path.display()));
        }
        println!("wrote {}", path.display());
    }

    for check in &outcome.checks {
        let verdict = if check.passed { "ok  " } else { "FAIL" };
        println!(
            "{verdict} line {}: {} (actual {})",
            check.expectation.line, check.expectation.source, check.actual
        );
    }

    let unmet = outcome.checks.iter().filter(|c| !c.passed).count();
    let total = outcome.checks.len();
    if outcome.all_passed() {
        println!("{total} expectations met");
    } else {
        println!("{unmet} of {total} expectations unmet");
        std::process::exit(EXIT_UNMET);
    }
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut scenario: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--out needs a directory".to_string())?;
                out = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => return Err(format!("unknown argument: {other}")),
            other if scenario.is_none() => scenario = Some(PathBuf::from(other)),
            other => return Err(format!("one scenario at a time: {other}")),
        }
    }
    let scenario = scenario.ok_or_else(|| "a scenario file is required".to_string())?;
    Ok(Options { scenario, out })
}

/// `work/captures/<scenario stem>/`, so two scenarios never overwrite each
/// other's frames. A path the scenario was already read from has a stem; the
/// fallback only covers our own confusion, not a caller's.
fn default_out(scenario: &Path) -> PathBuf {
    let stem = scenario
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scenario".to_string());
    Path::new(DEFAULT_OUT_ROOT).join(stem)
}

fn misuse(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(EXIT_MISUSE);
}
