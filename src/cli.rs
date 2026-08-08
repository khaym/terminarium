//! Command-line argument parsing. All rules live in the library (see main.rs),
//! so this is a pure function from argument tokens to a result — unit-testable
//! without touching argv or the process.

/// Parsed command-line options. Currently only the time scale, a test-only
/// fast-forward that defaults to 1 (real time).
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub time_scale: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self { time_scale: 1 }
    }
}

/// What the arguments asked the binary to do. Running the sea and answering a
/// question about it are different errands, so they are different variants:
/// `Run` opens a terminal, the other two print a line and are done.
#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    Run(Options),
    Help,
    Version,
}

/// What `--help` prints. Kept ASCII: it is the first thing a fresh install
/// shows, and unlike the tank it must survive a console that is not UTF-8.
/// The wording tracks the README's "how to play" so the two never disagree.
/// `--time-scale` is deliberately absent — it is a harness fast-forward with
/// a save file of its own, not part of the game the README teaches.
pub const HELP: &str = "\
A tiny sea in a terminal pane that grows while your coding agent works.

Usage: terminarium [OPTIONS]

The window size is the interface. One binary, two layers:

  wallpaper  any pane smaller than 80x20: just the sea. No numbers, no
             input; the thing you glance at between prompts.
  game       80x20 or larger: the same sea plus the economy - your
             currency, prices, and the reef.

Start in a wide pane: place your reef, press s to start the sea, buy your
first algae, then shrink the pane and get back to work.

Placing the reef (until s):

  h l (or left/right)  move along the sea floor
  j k (or up/down)     pick a reef to place
  Enter / Backspace    drop / lift a reef
  s                    commit the reef and start the sea

During the run:

  1-4                  buy life: algae -> plankton -> small fish -> big fish
  a                    grab the sunken anchor (h l move it, Enter sets it)
  n, then y            start a new sea (prestige)

q (or Ctrl-C) quits, in either layer, at any time.

Options:

  -h, --help     print this help and exit
  -V, --version  print the version and exit";

/// What `--version` prints. Built from the package version so the manifest
/// stays the single source of truth for what release this binary is.
pub const VERSION_LINE: &str = concat!("terminarium ", env!("CARGO_PKG_VERSION"));

/// Parse argument tokens (the program name already stripped). Two outcomes,
/// told apart by where they come from: a caller mistake surfaces as
/// `Err(message)` for the binary to print on stderr and exit non-zero — a
/// mistyped flag is the user's error, not ours, so there is no silent
/// fallback — while `--help` and `--version` are requests rather than
/// mistakes, so they come back as `Ok` for the binary to answer on stdout and
/// exit zero. A request wins from where it stands: parsing stops there, and
/// whatever came before it still has to be valid.
pub fn parse(args: &[String]) -> Result<Invocation, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "-V" | "--version" => return Ok(Invocation::Version),
            "--time-scale" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--time-scale needs a value".to_string())?;
                let scale: u64 = value
                    .parse()
                    .map_err(|_| format!("--time-scale: not a number: {value}"))?;
                if scale == 0 {
                    return Err("--time-scale must be at least 1".to_string());
                }
                options.time_scale = scale;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Invocation::Run(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|s| s.to_string()).collect()
    }

    fn options(tokens: &[&str]) -> Options {
        match parse(&args(tokens)).unwrap() {
            Invocation::Run(options) => options,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn no_flag_is_real_time() {
        assert_eq!(options(&[]), Options { time_scale: 1 });
    }

    #[test]
    fn time_scale_flag_sets_the_multiplier() {
        assert_eq!(options(&["--time-scale", "60"]).time_scale, 60);
    }

    #[test]
    fn zero_scale_is_rejected() {
        assert!(parse(&args(&["--time-scale", "0"])).is_err());
    }

    #[test]
    fn non_numeric_scale_is_rejected() {
        assert!(parse(&args(&["--time-scale", "fast"])).is_err());
    }

    #[test]
    fn missing_value_is_rejected() {
        assert!(parse(&args(&["--time-scale"])).is_err());
    }

    #[test]
    fn unknown_argument_is_rejected() {
        assert!(parse(&args(&["--turbo"])).is_err());
    }

    #[test]
    fn help_is_asked_for_by_either_spelling() {
        for flag in ["-h", "--help"] {
            assert_eq!(parse(&args(&[flag])).unwrap(), Invocation::Help);
        }
    }

    #[test]
    fn the_version_is_asked_for_by_either_spelling() {
        for flag in ["-V", "--version"] {
            assert_eq!(parse(&args(&[flag])).unwrap(), Invocation::Version);
        }
    }

    #[test]
    fn a_request_wins_over_the_options_before_it() {
        assert_eq!(
            parse(&args(&["--time-scale", "60", "--help"])).unwrap(),
            Invocation::Help
        );
    }

    #[test]
    fn a_mistake_before_a_request_is_still_a_mistake() {
        assert!(parse(&args(&["--turbo", "--help"])).is_err());
    }

    #[test]
    fn help_names_both_layers_and_the_size_that_divides_them() {
        for term in ["wallpaper", "game", "80x20"] {
            assert!(HELP.contains(term), "help should name {term}");
        }
    }

    #[test]
    fn help_lists_every_key_the_sea_answers_to() {
        for key in [
            "h l",
            "j k",
            "Enter / Backspace",
            "  s ",
            "1-4",
            "  a ",
            "n, then y",
            "q (or Ctrl-C)",
        ] {
            assert!(HELP.contains(key), "help should list the key {key:?}");
        }
    }

    #[test]
    fn help_lists_every_flag_the_parser_answers_to() {
        for flag in ["-h, --help", "-V, --version"] {
            assert!(HELP.contains(flag), "help should list {flag}");
        }
        // The help is a promise; each flag it prints has to parse.
        for flag in ["-h", "--help", "-V", "--version"] {
            assert!(parse(&args(&[flag])).is_ok(), "{flag} should parse");
        }
    }

    #[test]
    fn help_stays_ascii() {
        assert!(
            HELP.is_ascii(),
            "help must survive a console that is not UTF-8"
        );
    }

    #[test]
    fn the_version_printed_is_the_version_in_the_manifest() {
        // The package section is the first table in Cargo.toml, so the first
        // line declaring a version is the crate's own.
        let manifest = include_str!("../Cargo.toml");
        let version = manifest
            .lines()
            .find_map(|line| line.trim().strip_prefix("version = "))
            .expect("Cargo.toml declares a version")
            .trim_matches('"');
        assert_eq!(VERSION_LINE, format!("terminarium {version}"));
    }
}
