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

/// Parse argument tokens (the program name already stripped). A caller mistake
/// surfaces as `Err(message)` for the binary to print and exit non-zero — a
/// mistyped flag is the user's error, not ours, so there is no silent fallback.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
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
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_flag_is_real_time() {
        assert_eq!(parse(&args(&[])).unwrap(), Options { time_scale: 1 });
    }

    #[test]
    fn time_scale_flag_sets_the_multiplier() {
        assert_eq!(
            parse(&args(&["--time-scale", "60"])).unwrap().time_scale,
            60
        );
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
}
