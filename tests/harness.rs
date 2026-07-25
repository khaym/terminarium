//! End-to-end check of the scenario harness against the demo scenario the repo
//! ships. The scenario file is the one a developer runs by hand
//! (`cargo run --example capture -- tests/scenarios/demo.txt`), read here rather
//! than restated, so the demo and its test can never drift apart: if the demo
//! stops reaching its goal, this fails.

use std::fs;

use terminarium::harness;

const DEMO: &str = "tests/scenarios/demo.txt";

#[test]
fn the_demo_scenario_reaches_its_goal_and_dumps_both_layers_in_color() {
    let text = fs::read_to_string(DEMO).unwrap_or_else(|e| panic!("reading {DEMO}: {e}"));
    let scenario = harness::parse(&text).unwrap_or_else(|e| panic!("{DEMO}: {e}"));
    let outcome = harness::run(&scenario);

    // Every expectation the demo states is met — the scripted play really does
    // reach the state the scenario claims.
    assert!(!outcome.checks.is_empty(), "the demo checks its goal");
    for check in &outcome.checks {
        assert!(
            check.passed,
            "{DEMO} line {}: {} (actual {})",
            check.expectation.line, check.expectation.source, check.actual
        );
    }

    // Both layers are captured: the thin pane's wallpaper and the full-screen
    // game. A dump is one line per row, and carries color.
    for (label, width, height) in [("pane", 40, 12), ("full", 100, 30)] {
        let capture = outcome
            .captures
            .iter()
            .find(|c| c.label == label)
            .unwrap_or_else(|| panic!("{DEMO} captures a frame labeled {label}"));
        assert_eq!((capture.width, capture.height), (width, height));
        assert_eq!(
            capture.ansi.lines().count(),
            usize::from(height),
            "{label}: one line per row"
        );
        assert!(
            capture.ansi.contains("\x1b[0;38;5;"),
            "{label}: the dump carries indexed color, so cat shows the picture"
        );
        for line in capture.ansi.lines() {
            assert!(
                line.ends_with("\x1b[0m"),
                "{label}: every row resets, so a dump cannot bleed into the shell"
            );
        }
    }
}
