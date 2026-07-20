//! TUI snapshot regression (the render layer of the Swiss cheese): both
//! layers are drawn from a fixed (state, frame) and compared cell-by-cell
//! against a frozen picture. Rendering is a pure function of state + frame,
//! so any diff here is a real rendering change — bless it by updating the
//! literals below (a failure prints the actual rows ready to paste).

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tui_game::app::App;
use tui_game::engine::{Params, Rock, State, MICRO, SLOTS};
use tui_game::ui;

fn fixed_state() -> State {
    State {
        population: [3, 2, 2, 1],
        pool: [50 * MICRO, MICRO, MICRO, MICRO],
        nutrient: 2 * MICRO,
        collectable: 210 * MICRO,
        currency: 420 * MICRO,
        // A run in progress: one base rock near mid-floor and a clock past
        // zero, so housing has emerged (capacity HUD) and the reef anchors the
        // scene (rock, gathered life, sediment mound under it).
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
    }
}

/// Render one frame of `state` at the given size.
fn rendered_terminal_with(state: State, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut app = App::new(state, Params::default());
    app.on_resize(width, height);
    app.frame = 8;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| ui::draw(&app, frame)).expect("draw");
    terminal
}

/// Render one frame of the fixed scene at the given size.
fn rendered_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    rendered_terminal_with(fixed_state(), width, height)
}

/// Render `state` at the given size on a chosen animation `frame` — like
/// `rendered_terminal_with`, but the frame is a parameter so time-varying
/// sprite positions can be sampled across the animation.
fn rendered_at_frame(state: State, width: u16, height: u16, frame: u64) -> Terminal<TestBackend> {
    let mut app = App::new(state, Params::default());
    app.on_resize(width, height);
    app.frame = frame;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| ui::draw(&app, f)).expect("draw");
    terminal
}

/// A rendered buffer as text rows.
fn rows_of(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).expect("cell").symbol())
                .collect()
        })
        .collect()
}

/// The fixed scene rendered at the given size, as text rows.
fn render_at(width: u16, height: u16) -> Vec<String> {
    rows_of(&rendered_terminal(width, height), width, height)
}

fn assert_snapshot(actual: &[String], expected: &[&str]) {
    if actual != expected {
        println!("--- actual (paste to bless) ---");
        for row in actual {
            println!("\"{row}\",");
        }
        panic!("snapshot mismatch");
    }
}

/// Thin pane: pure decoration — waves, life, sediment, and not a single
/// digit, bar, or key hint.
#[test]
fn wallpaper_snapshot_40x12() {
    let actual = render_at(40, 12);
    let expected = [
        " ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~  ",
        "              ><)))>                    ",
        "                                        ",
        "                       .                ",
        "                      .                 ",
        "                                        ",
        "                  .                     ",
        "                                        ",
        "                                        ",
        "                  )   ⠁(><>             ",
        "                  (   )><>              ",
        "                 ▁)▄█▄((                ",
    ];
    assert_snapshot(&actual, &expected);

    let joined = actual.join("");
    assert!(
        !joined.chars().any(|c| c.is_ascii_digit()),
        "the wallpaper must never show a digit"
    );
}

/// Zoomed pane: the same tank plus the HUD — populations, next costs,
/// currency with the collect flash, and key hints.
#[test]
fn game_snapshot_100x30() {
    let actual = render_at(100, 30);
    let expected = [
        " ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~  ",
        "                                            ><)))>                                                  ",
        "                                                    .                                               ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                    ⠁                                               ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                ⠂                                                   ",
        "                                                                                                    ",
        "                                                )    (><>                                           ",
        "                                                (   )><>                                            ",
        "                                                )▄█▄((                                              ",
        "────────────────────────────────────────────────────────────────────────────────────────────────────",
        " [1] Algae 3/4  $141   [2] Plankton 2/3  $565   [3] Small fish 2/2  $1129   [4] Big fish 1/1  $4480 ",
        " $ 630  +210 collected                                                                              ",
        " [1-4] buy   [q] quit                                                                               ",
    ];
    assert_snapshot(&actual, &expected);
}

/// Requirement from work/render-observations.md: the palette never leaves
/// the indexed-256 space (tmux quantizes RGB; Windows Terminal renders
/// truecolor but does not advertise COLORTERM). Any Rgb or named ANSI color
/// slipping into any screen — wallpaper, game HUD, or placement — fails here.
#[test]
fn palette_stays_in_indexed_256_space() {
    use ratatui::style::Color;

    let in_space =
        |c: Option<Color>| matches!(c, None | Some(Color::Reset) | Some(Color::Indexed(_)));

    // (label, terminal, w, h): the live scene at both sizes, plus the placement
    // screen (run not started) which introduces its own marker/preview colors.
    let scenes = [
        ("wallpaper", rendered_terminal(40, 12), 40u16, 12u16),
        ("game", rendered_terminal(100, 30), 100, 30),
        (
            "placement",
            rendered_terminal_with(State::new(), 100, 30),
            100,
            30,
        ),
    ];
    for (label, terminal, w, h) in scenes {
        let buffer = terminal.backend().buffer();
        for y in 0..h {
            for x in 0..w {
                let style = buffer.cell((x, y)).expect("cell").style();
                assert!(
                    in_space(style.fg) && in_space(style.bg),
                    "cell ({x},{y}) on {label} {w}x{h} uses a color outside indexed-256: {style:?}"
                );
            }
        }
    }
}

/// The buy highlight answers "can I act on this species?" — it needs both money
/// and free housing. With one emerged base rock (capacity [4,3,2,1]) and $200:
///   - algae 0/4 at $100     -> room + affordable   -> highlighted (green)
///   - plankton 0/3 at $450  -> room but too dear   -> plain
///   - small fish 2/2        -> housing full        -> dimmed, never highlighted
#[test]
fn buy_highlight_needs_money_and_housing() {
    use ratatui::style::Color;

    let state = State {
        population: [0, 0, 2, 0],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 200 * MICRO,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
    };
    let terminal = rendered_terminal_with(state, 100, 30);
    let buffer = terminal.backend().buffer();

    let species_row = 27u16;
    let row: String = (0..100)
        .map(|x| buffer.cell((x, species_row)).expect("cell").symbol())
        .collect();
    let x1 = row.find("[1]").expect("algae segment") as u16;
    let x2 = row.find("[2]").expect("plankton segment") as u16;
    let x3 = row.find("[3]").expect("small fish segment") as u16;

    let fg = |x: u16| buffer.cell((x, species_row)).expect("cell").style().fg;
    assert_eq!(
        fg(x1),
        Some(Color::Indexed(114)),
        "room + affordable = highlighted"
    );
    assert_eq!(
        fg(x2),
        Some(Color::Indexed(252)),
        "room but unaffordable = plain"
    );
    assert_eq!(
        fg(x3),
        Some(Color::Indexed(240)),
        "full housing = dimmed, not highlighted"
    );
}

/// The one-time placement screen (run not yet started): an empty tank with the
/// floor slots marked and a dim rock preview at the default cursor, plus the
/// move/place hint — the only screen with text that isn't the HUD.
#[test]
fn placement_snapshot_100x30() {
    let terminal = rendered_terminal_with(State::new(), 100, 30);
    let actual = rows_of(&terminal, 100, 30);
    let expected = [
        " ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~  ",
        " place your reef                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "          ▁                   ▁                  ▄█▄                  ▁                   ▁         ",
        " [<-/->] move   [enter] place                                                                       ",
    ];
    assert_snapshot(&actual, &expected);

    let joined = actual.join("");
    assert!(
        !joined.chars().any(|c| c.is_ascii_digit()),
        "the placement screen shows no digit"
    );
}

/// Feedback (a): a bought individual must show in the picture. With one rock
/// and four algae the renderer draws exactly four frond columns — population
/// equals the visible count (within the sprite cap). The old side±{2,3,4}
/// scheme collided, so four algae could render as only two fronds.
#[test]
fn algae_population_shows_one_column_each() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    let state = State {
        population: [4, 0, 0, 0],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
    };
    let (w, h) = (40u16, 12u16);
    let terminal = rendered_terminal_with(state, w, h);
    let buffer = terminal.backend().buffer();

    let columns: HashSet<u16> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(35)))
        .map(|(x, _)| x)
        .collect();
    assert_eq!(
        columns.len(),
        4,
        "four algae must show as four frond columns, got {columns:?}"
    );
}

/// The tank's home is a thin always-on side pane, so a bought individual must
/// show even there — including at an edge slot, where a naive fan runs some of
/// its columns off-pane. Across widths 20 and 30 with the rock at slot 0 and at
/// slot 4, four algae still render as four frond columns: off-pane candidate
/// columns are skipped in favour of in-pane ones.
#[test]
fn algae_visible_at_edge_slots_in_narrow_panes() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    for w in [20u16, 30] {
        for slot in [0u8, 4] {
            let state = State {
                population: [4, 0, 0, 0],
                pool: [0; 4],
                nutrient: 0,
                collectable: 0,
                currency: 0,
                rocks: vec![Rock { kind: 0, slot }],
                tick_count: 30,
            };
            let h = 12u16;
            let terminal = rendered_terminal_with(state, w, h);
            let buffer = terminal.backend().buffer();
            let columns: HashSet<u16> = (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(35))
                })
                .map(|(x, _)| x)
                .collect();
            assert_eq!(
                columns.len(),
                4,
                "four algae must show as four frond columns at width {w}, slot {slot}, got {columns:?}"
            );
        }
    }
}

/// Plankton never permanently overlap: each occupies its own column, so no two
/// ever share a cell. Eight plankton resolve to eight distinct columns — if two
/// were assigned the same column the union could only reach seven. Taken over a
/// frame sweep (a single detritus speck, drawn over the plankton, can hide one
/// at a given frame, but a different one each time), all eight columns surface.
#[test]
fn plankton_occupy_distinct_cells() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    // Eight plankton on one rock. Rendering maps population -> sprites directly
    // (housing is the engine's concern), so this drives that map above what one
    // base rock would house, to exercise its distinctness.
    let state = State {
        population: [0, 8, 0, 0],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
    };
    let mut columns_seen: HashSet<u16> = HashSet::new();
    for frame in [0u64, 3, 8, 17, 50, 100] {
        let terminal = rendered_at_frame(state.clone(), 100, 30, frame);
        let buffer = terminal.backend().buffer();
        for (x, y) in (0..30u16).flat_map(|y| (0..100u16).map(move |x| (x, y))) {
            if buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(122)) {
                columns_seen.insert(x);
            }
        }
    }
    assert_eq!(
        columns_seen.len(),
        8,
        "eight plankton occupy eight distinct columns, got {columns_seen:?}"
    );
}

/// Fish keep to their own lanes: same-species fish get distinct rows, so their
/// glyphs never share a cell — no permanent overlap by construction. Two small
/// fish plus one big fish render as three fully-drawn glyphs (3+3+6 cells) on
/// three distinct rows, every frame.
#[test]
fn fish_keep_distinct_lanes() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    let state = State {
        population: [0, 0, 2, 1],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
    };
    for frame in [0u64, 8, 17, 50, 123] {
        let terminal = rendered_at_frame(state.clone(), 100, 30, frame);
        let buffer = terminal.backend().buffer();
        let cells: Vec<(u16, u16)> = (0..30u16)
            .flat_map(|y| (0..100u16).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                matches!(
                    buffer.cell((x, y)).expect("cell").style().fg,
                    Some(Color::Indexed(215)) | Some(Color::Indexed(209))
                )
            })
            .collect();
        assert_eq!(
            cells.len(),
            12,
            "two small (len 3) + one big (len 6) fully drawn at frame {frame}"
        );
        let rows: HashSet<u16> = cells.iter().map(|&(_, y)| y).collect();
        assert_eq!(
            rows.len(),
            3,
            "three fish on three distinct lanes at frame {frame}"
        );
    }
}

/// Feedback (b): the reef's placement must move the life. A fish patrols a
/// bounded window around its rock — its anchor x stays within the rock center ±
/// the species radius for every frame, and moving the rock (slot 0 vs slot 4)
/// moves the fish with it.
#[test]
fn fish_patrol_follows_its_rock() {
    use ratatui::style::Color;

    // Small-fish patrol radius, mirroring src/ui/wallpaper.rs SMALL_RADIUS.
    const SMALL_RADIUS: i64 = 5;
    // The wallpaper area under the game HUD spans the full width from column 0,
    // so slot centers use the terminal width; mirrors wallpaper::slot_center_x.
    let tank_w = 100i64;
    let slots = i64::from(SLOTS);
    let center = |slot: i64| ((slot * 2 + 1) * tank_w) / (2 * slots);

    let fish_x = |slot: u8, frame: u64| -> i64 {
        let state = State {
            population: [0, 0, 1, 0],
            pool: [0; 4],
            nutrient: 0,
            collectable: 0,
            currency: 0,
            rocks: vec![Rock { kind: 0, slot }],
            tick_count: 30,
        };
        let terminal = rendered_at_frame(state, 100, 30, frame);
        let buffer = terminal.backend().buffer();
        (0..30u16)
            .flat_map(|y| (0..100u16).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(215))
            })
            .map(|(x, _)| i64::from(x))
            .min()
            .expect("the fish is drawn")
    };

    for frame in 0..200u64 {
        let (c0, c4) = (center(0), center(4));
        let x0 = fish_x(0, frame);
        assert!(
            (c0 - SMALL_RADIUS..=c0 + SMALL_RADIUS).contains(&x0),
            "slot-0 fish x={x0} left center {c0} +-{SMALL_RADIUS} at frame {frame}"
        );
        let x4 = fish_x(4, frame);
        assert!(
            (c4 - SMALL_RADIUS..=c4 + SMALL_RADIUS).contains(&x4),
            "slot-4 fish x={x4} left center {c4} +-{SMALL_RADIUS} at frame {frame}"
        );
        assert_ne!(
            x0, x4,
            "the fish follows its rock: slot 0 and slot 4 differ at frame {frame}"
        );
    }
}

/// Before a rock is placed the wallpaper is an empty sea: waves only, no reef,
/// no life, no text — the wallpaper grammar is unchanged by the new run gate.
#[test]
fn pre_run_wallpaper_is_an_empty_sea() {
    let actual = rows_of(&rendered_terminal_with(State::new(), 40, 12), 40, 12);
    let joined: String = actual.join("");
    assert!(
        joined.chars().all(|c| c == ' ' || c == '~'),
        "an empty tank shows only water and waves, got: {joined:?}"
    );
    assert!(joined.contains('~'), "the surface waves are still drawn");
}
