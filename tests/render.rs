//! TUI snapshot regression (the render layer of the Swiss cheese): both
//! layers are drawn from a fixed (state, frame) and compared cell-by-cell
//! against a frozen picture. Rendering is a pure function of state + frame,
//! so any diff here is a real rendering change — bless it by updating the
//! literals below (a failure prints the actual rows ready to paste).

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tui_game::app::{App, Phase};
use tui_game::engine::{Params, Rock, State, MICRO, SLOTS};
use tui_game::ui;

fn fixed_state() -> State {
    State {
        population: [3, 2, 2, 1],
        pool: [50 * MICRO, MICRO, MICRO, MICRO],
        nutrient: 2 * MICRO,
        collectable: 210 * MICRO,
        currency: 420 * MICRO,
        score: 0,
        // A run in progress: started, with one base rock near mid-floor and a
        // clock past zero, so housing has emerged (capacity HUD) and the reef
        // anchors the scene (rock, gathered life, sediment mound under it).
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
        started: true,
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

/// Render `state` at the given size, frame, and time-of-day phase — the phase is
/// the extra render input the palette and any visitor keys off.
fn rendered_phase_frame(
    state: State,
    width: u16,
    height: u16,
    frame: u64,
    phase: Phase,
) -> Terminal<TestBackend> {
    let mut app = App::new(state, Params::default());
    app.on_resize(width, height);
    app.frame = frame;
    app.phase = phase;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| ui::draw(&app, f)).expect("draw");
    terminal
}

/// Render a pre-built app, so a test can set fields (placement_kind,
/// new_sea_pending) before drawing. Like `rendered_terminal_with`, but the app
/// is supplied rather than built from a bare state.
fn rendered_terminal_of(mut app: App, width: u16, height: u16) -> Terminal<TestBackend> {
    app.on_resize(width, height);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| ui::draw(&app, frame)).expect("draw");
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
        "     ><)))>                             ",
        "                                        ",
        "              .                         ",
        "             .                          ",
        "                                        ",
        "         .                              ",
        "                                        ",
        "                                        ",
        "         )   ⠁(><>                      ",
        "         (   )><>                       ",
        "        ▁)▄█▄((                         ",
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
        "                     ><)))>                                                                         ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                             ⠁                                                                      ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                             .                                                                      ",
        "                                                                                                    ",
        "                         ⠂                                                                          ",
        "                                                                                                    ",
        "                                                                                                    ",
        "                         )    (><>                                                                  ",
        "                         (   )><>                                                                   ",
        "                         )▄█▄((                                                                     ",
        "────────────────────────────────────────────────────────────────────────────────────────────────────",
        " [1] Algae 3/4  $141   [2] Plankton 2/3  $565   [3] Small fish 2/2  $1129   [4] Big fish 1/1  $4480 ",
        " $ 630  +210 collected                                                                              ",
        " score 210   next reef at 12000                                                                     ",
        " [1-4] buy   [n] new sea   [q] quit                                                                 ",
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

    // A coral + kelp sea exercises every new reef color at once: coral and
    // kelp rock bodies, both base-algae tints, a big fish and a dugong.
    let reefs = State {
        population: [4, 0, 0, 2],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 30_000 * MICRO,
        rocks: vec![Rock { kind: 1, slot: 1 }, Rock { kind: 2, slot: 3 }],
        tick_count: 300,
        started: true,
    };

    // (label, terminal, w, h): the live scene at both sizes, the placement
    // screen (run not started) with its own marker/preview colors, and a
    // coral+kelp sea that brings in the reef-variant palette.
    let scenes = [
        ("wallpaper", rendered_terminal(40, 12), 40u16, 12u16),
        ("game", rendered_terminal(100, 30), 100, 30),
        (
            "placement",
            rendered_terminal_with(State::new(), 100, 30),
            100,
            30,
        ),
        ("reefs", rendered_terminal_with(reefs, 100, 30), 100, 30),
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
        score: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
        started: true,
    };
    let terminal = rendered_terminal_with(state, 100, 30);
    let buffer = terminal.backend().buffer();

    // The species segments sit on the second HUD row (rule, species, money,
    // score, hint): area.top() + tank.height + 1 = 30 - HUD_HEIGHT(5) + 1.
    let species_row = 26u16;
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

/// The placement screen (run not yet started): an empty tank with the kind
/// panel (list, budget, next unlock), the floor slots marked, and a dim ghost
/// in the selected kind's shape at the default cursor, plus the full key hint.
/// Unlike the wallpaper, this is the game layer, so it may show numbers (the
/// budget and unlock thresholds are load-bearing here).
#[test]
fn placement_snapshot_100x30() {
    let terminal = rendered_terminal_with(State::new(), 100, 30);
    let actual = rows_of(&terminal, 100, 30);
    let expected = [
        " ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~  ",
        " place your reef                                                                                    ",
        " rock(1)                                                                                            ",
        " budget 0/1                                                                                         ",
        " coral unlocks at 12000                                                                             ",
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
        "     ▁          ▁          ▁          ▁          ▄█▄         ▁          ▁          ▁          ▁     ",
        " [</>] move  [^/v] kind  [enter] place  [bksp] remove  [s] start                                    ",
    ];
    assert_snapshot(&actual, &expected);
}

/// The placement screen offers every floor slot: with no reef placed yet, each
/// free slot shows a dim floor marker except the one under the cursor, which
/// shows the reef ghost instead. So the free markers number one short of the
/// slot count — nine slots, eight markers plus the ghost. This is the 5->9 slot
/// increase (chalk #17): a wider grid to compose a reef across.
#[test]
fn placement_marks_all_nine_slots() {
    use ratatui::style::Color;

    let terminal = rendered_terminal_with(State::new(), 100, 30);
    let buffer = terminal.backend().buffer();
    // Placement reserves the bottom row for the hint, so the floor is row 28.
    let floor = 28u16;
    let markers = (0..100u16)
        .filter(|&x| {
            let cell = buffer.cell((x, floor)).expect("cell");
            cell.symbol() == "▁" && cell.style().fg == Some(Color::Indexed(236))
        })
        .count();
    assert_eq!(
        markers, 8,
        "eight free-slot markers; the ninth slot is under the cursor ghost"
    );
    assert_eq!(usize::from(SLOTS), markers + 1, "nine slots in all");
}

/// Nine slots must still fit the minimum game pane (80x20) without the reefs
/// crowding. With a rock on every slot (and one algae frond each), every 3-wide
/// body lands on its own three columns — nine whole bodies are 27 floor cells —
/// and every frond fans into a gap between bodies, nine fronds in nine distinct
/// columns. Algae draw after rocks, so a frond straying onto a neighbour's body
/// would overwrite it and drop the 27 below; that it holds proves non-overlap.
#[test]
fn nine_reefs_fit_the_min_pane_without_overlap() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    // One algae per rock: population equals the slot count, shared round-robin
    // across the rocks, so each reef shows a single frond in its nearest flank.
    let state = State {
        population: [u32::from(SLOTS), 0, 0, 0],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 0,
        rocks: (0..SLOTS).map(|slot| Rock { kind: 0, slot }).collect(),
        tick_count: 30,
        started: true,
    };
    let (w, h) = (80u16, 20u16);
    let terminal = rendered_terminal_with(state, w, h);
    let buffer = terminal.backend().buffer();
    // Game layer: the tank sits above the 5-row HUD, so its floor is row 14.
    let floor = 14u16;

    let body_cells = (0..w)
        .filter(|&x| buffer.cell((x, floor)).expect("cell").style().fg == Some(Color::Indexed(245)))
        .count();
    assert_eq!(
        body_cells,
        usize::from(SLOTS) * 3,
        "nine 3-wide rock bodies, none overlapping or overdrawn by a frond"
    );

    let frond_columns: HashSet<u16> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(35)))
        .map(|(x, _)| x)
        .collect();
    assert_eq!(
        frond_columns.len(),
        usize::from(SLOTS),
        "one frond per reef, each in its own flank column: {frond_columns:?}"
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
        score: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
        started: true,
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
/// the last slot, four algae still render as four frond columns: off-pane
/// candidate columns are skipped in favour of in-pane ones.
#[test]
fn algae_visible_at_edge_slots_in_narrow_panes() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    for w in [20u16, 30] {
        for slot in [0u8, SLOTS - 1] {
            let state = State {
                population: [4, 0, 0, 0],
                pool: [0; 4],
                nutrient: 0,
                collectable: 0,
                currency: 0,
                score: 0,
                rocks: vec![Rock { kind: 0, slot }],
                tick_count: 30,
                started: true,
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
        score: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
        started: true,
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
        score: 0,
        rocks: vec![Rock { kind: 0, slot: 2 }],
        tick_count: 30,
        started: true,
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
            score: 0,
            rocks: vec![Rock { kind: 0, slot }],
            tick_count: 30,
            started: true,
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

/// Feedback (a) at the budget-5 wall: a five-rock sea filled to housing shows
/// every bought individual. rock×5 tops out at algae 20, plankton 15, small fish
/// 10, big fish 5 — and the renderer draws each in full. The pre-#16 sprite caps
/// (12/14/8/4) clipped all four; without the raise this test goes red. Rocks sit
/// on spread slots (0,2,4,6,8) so their colonies do not cross, and static
/// sprites are counted by column while gliding fish are counted by their glyph
/// cells at the least-occluded frame of a sweep.
#[test]
fn rock_five_sea_draws_the_full_population() {
    use ratatui::style::Color;
    use std::collections::HashSet;

    let state = State {
        population: [20, 15, 10, 5],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 0, // below the anchor unlock, so no landmark tints the count
        rocks: (0..5u8)
            .map(|k| Rock {
                kind: 0,
                slot: k * 2,
            })
            .collect(),
        tick_count: 30,
        started: true,
    };
    let (w, h) = (100u16, 30u16);

    // Algae (35) and plankton (122) hold fixed columns; count distinct columns
    // of each, unioned across a frame sweep (occlusion at one frame is clear at
    // another). Fish glide, so count their glyph cells (small "><>", 3 cells;
    // big 6 cells) and take the per-frame maximum — the least-occluded frame
    // shows every cell, so the max is the true drawn count.
    let mut algae_cols: HashSet<u16> = HashSet::new();
    let mut plankton_cols: HashSet<u16> = HashSet::new();
    let mut small_cells_max = 0usize;
    let mut big_cells_max = 0usize;

    for frame in 0..200u64 {
        let terminal = rendered_at_frame(state.clone(), w, h, frame);
        let buffer = terminal.backend().buffer();
        let (mut small_cells, mut big_cells) = (0usize, 0usize);
        for (x, y) in (0..h).flat_map(|y| (0..w).map(move |x| (x, y))) {
            match buffer.cell((x, y)).expect("cell").style().fg {
                Some(Color::Indexed(35)) => {
                    algae_cols.insert(x);
                }
                Some(Color::Indexed(122)) => {
                    plankton_cols.insert(x);
                }
                Some(Color::Indexed(215)) => small_cells += 1,
                Some(Color::Indexed(209)) => big_cells += 1,
                _ => {}
            }
        }
        small_cells_max = small_cells_max.max(small_cells);
        big_cells_max = big_cells_max.max(big_cells);
    }

    assert_eq!(
        algae_cols.len(),
        20,
        "20 algae show as 20 frond columns, got {algae_cols:?}"
    );
    assert_eq!(
        plankton_cols.len(),
        15,
        "15 plankton show as 15 columns, got {plankton_cols:?}"
    );
    assert_eq!(
        small_cells_max, 30,
        "10 small fish (3 cells each) fully drawn, got {small_cells_max}"
    );
    assert_eq!(
        big_cells_max, 30,
        "5 big fish (6 cells each) fully drawn, got {big_cells_max}"
    );
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

/// The placement screen lists the unlocked kinds with their budget cost, the
/// budget used out of its ceiling, and the next reef still to unlock. With
/// score past coral (budget 2), rock and coral are selectable and kelp reads as
/// the next goal; the selected kind is highlighted.
#[test]
fn placement_lists_unlocked_kinds_budget_and_next_unlock() {
    use ratatui::style::Color;

    let mut state = State::new();
    state.score = 12_000 * MICRO; // rock + coral unlocked, kelp locked
    let terminal = rendered_terminal_with(state, 100, 30);
    let rows = rows_of(&terminal, 100, 30);
    let joined = rows.join("\n");

    assert!(
        joined.contains("rock(1)"),
        "rock listed with cost: {joined}"
    );
    assert!(joined.contains("coral(2)"), "coral listed with cost");
    assert!(
        !joined.contains("kelp(3)"),
        "kelp is locked, so not in the selectable list"
    );
    assert!(
        joined.contains("kelp unlocks at 30000"),
        "the next reef reads as a goal"
    );
    assert!(joined.contains("budget 0/2"), "budget used/total reads");

    // The selected kind (default 0 = rock) is highlighted in the buy-green.
    let buffer = terminal.backend().buffer();
    let kinds_row = 2u16; // area.top() + 2
    let row: String = (0..100)
        .map(|x| buffer.cell((x, kinds_row)).expect("cell").symbol())
        .collect();
    let rx = row.find("rock(1)").expect("rock segment") as u16;
    assert_eq!(
        buffer.cell((rx, kinds_row)).expect("cell").style().fg,
        Some(Color::Indexed(114)),
        "the selected kind is highlighted"
    );
}

/// The placement screen at the second wall (score 75,000, budget 5): the budget
/// reads out of 5, every kind is selectable with no locked goal left, and a kelp
/// dropped (cost 3) leaves room — budget 3/5, placement still open — so a kelp
/// sea is no longer the lone-reef the budget-3 wall forced.
#[test]
fn placement_budget_five_leaves_room_after_kelp() {
    // Score past the second wall: budget 5, every kind unlocked.
    let mut state = State::new();
    state.score = 75_000 * MICRO;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("budget 0/5"),
        "budget reads out of 5: {joined}"
    );
    assert!(joined.contains("kelp(3)"), "kelp is selectable at budget 5");
    assert!(
        !joined.contains("unlocks at"),
        "no locked reef remains as a goal at budget 5"
    );

    // A kelp placed (cost 3) leaves 2 of the 5 — placement continues, unlike the
    // budget-3 wall where kelp spent everything.
    let mut state = State::new();
    state.score = 75_000 * MICRO;
    state.rocks = vec![Rock { kind: 2, slot: 0 }];
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("budget 3/5"),
        "kelp spends 3 of 5, leaving room for more: {joined}"
    );
    assert!(
        joined.contains("[enter] place"),
        "the placement hint stays — more reef can still be dropped"
    );
}

/// The HUD shows the lifetime score and, while a reef is still locked, the
/// score threshold of the next one — the goal a new sea works toward. With
/// every reef unlocked the goal drops and only the score remains.
#[test]
fn hud_shows_score_and_next_reef() {
    // collectable is zeroed so entering the game layer does not fold surplus
    // into the score under test.
    let mut state = fixed_state();
    state.score = 1_234 * MICRO;
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(joined.contains("score 1234"), "score reads: {joined}");
    assert!(
        joined.contains("next reef at 12000"),
        "the next reef threshold reads"
    );

    // Crossing a threshold mid-run: the budget now exceeds what the reef spends,
    // so the line names the kind that wall unlocked and shows the unspent
    // headroom in budget terms, pointing at the new sea — the crossing announces
    // its reward and the reason to rebuild stays visible after the unlock.
    let mut state = fixed_state();
    state.score = 12_000 * MICRO; // coral's wall crossed; budget 2, one rock spends 1
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("coral unlocked, budget 1/2 - [n] new sea"),
        "the crossed wall's coral reward and unspent budget are announced: {joined}"
    );
    assert!(
        !joined.contains("next reef"),
        "the rebuild nudge replaces the next-threshold goal"
    );

    // Every reef unlocked and the budget fully spent: nothing left to work
    // toward or rebuild for — the score stands alone.
    let mut state = fixed_state();
    state.score = 40_000 * MICRO; // past every unlock
    state.rocks = vec![Rock { kind: 2, slot: 2 }]; // kelp spends the full budget 3
    state.tick_count = 300;
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("score 40000"),
        "score reads when all unlocked"
    );
    assert!(
        !joined.contains("next reef") && !joined.contains("- [n] new sea"),
        "no goal and no nudge once everything is unlocked and spent"
    );
}

/// The headroom nudge speaks the placement screen's budget vocabulary, so it
/// stays accurate at every budget step — including the budget-5 wall (score
/// 75,000) that unlocks no new kind. A kelp reef past that wall reports its
/// unspent budget instead of dressing long-unlocked coral as freshly
/// "unlocked" (symptom 1), and the budget-5 step is announced at all where
/// naming a kind said nothing (symptom 2). A fully-spent budget clears the line.
#[test]
fn headroom_nudge_reads_as_budget_at_the_second_wall() {
    // A kelp reef (spends 3) past the budget-5 wall: budget 5 > 3, headroom
    // shows. The old code named the newest absent kind — coral — as "unlocked"
    // here, though coral unlocked at 12,000; budget vocabulary cannot go stale.
    let mut state = fixed_state();
    state.score = 75_000 * MICRO;
    state.rocks = vec![Rock { kind: 2, slot: 2 }]; // kelp, spends 3 of 5
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("budget 3/5 - [n] new sea"),
        "the nudge reports unspent budget: {joined}"
    );
    assert!(
        !joined.contains("unlocked"),
        "the nudge never claims a stale unlock: {joined}"
    );

    // kelp + rock spends 4 of 5 — the budget-5 wall's headroom announced in
    // budget terms (regression guard for the silent budget-5 step).
    let mut state = fixed_state();
    state.score = 75_000 * MICRO;
    state.rocks = vec![Rock { kind: 2, slot: 2 }, Rock { kind: 0, slot: 4 }];
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        joined.contains("budget 4/5 - [n] new sea"),
        "the budget-5 headroom reads: {joined}"
    );

    // kelp + coral spends the full 5 — no headroom, so no nudge line.
    let mut state = fixed_state();
    state.score = 75_000 * MICRO;
    state.rocks = vec![Rock { kind: 2, slot: 2 }, Rock { kind: 1, slot: 4 }];
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(
        !joined.contains("- [n] new sea"),
        "a fully-spent budget clears the nudge: {joined}"
    );
}

/// The headroom nudge names the kind the latest crossed budget wall unlocked, so
/// a wall crossing announces its reward. Past the 30k wall a coral reef (spends
/// 2 of 3) reads the kelp unlock alongside its headroom. The name belongs to the
/// latest wall only — crossing the next, kind-less wall (75k) drops it, which is
/// why the second-wall test above sees no "unlocked": staleness cannot arise.
/// Rendered at the 80-column minimum, so the fuller line still fits the pane.
#[test]
fn headroom_nudge_names_the_latest_crossed_wall() {
    let mut state = fixed_state();
    state.score = 30_000 * MICRO; // kelp's wall just crossed; budget 3
    state.rocks = vec![Rock { kind: 1, slot: 2 }]; // coral, spends 2 of 3
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 80, 20), 80, 20).join("\n");
    assert!(
        joined.contains("kelp unlocked, budget 2/3 - [n] new sea"),
        "the 30k wall's kelp reward is named with the headroom, whole at 80 cols: {joined}"
    );
}

/// The HUD hint offers the new-sea action alongside buy and quit.
#[test]
fn hud_hint_includes_new_sea() {
    let mut state = fixed_state();
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 100, 30), 100, 30).join("\n");
    assert!(joined.contains("[n] new sea"), "the hint offers new sea");
    assert!(joined.contains("[1-4] buy"), "the buy hint stays");
    assert!(joined.contains("[q] quit"), "the quit hint stays");
}

/// While a new-sea confirmation is armed the HUD shows the prompt in place of
/// the key hint, so the yes/no choice reads where the keys were.
#[test]
fn new_sea_prompt_shows_when_pending() {
    let mut state = fixed_state();
    state.collectable = 0;
    let mut app = App::new(state, Params::default());
    app.frame = 8;
    app.new_sea_pending = true;
    let terminal = rendered_terminal_of(app, 100, 30);
    let joined = rows_of(&terminal, 100, 30).join("\n");

    assert!(
        joined.contains("start a new sea?"),
        "the confirmation prompt shows: {joined}"
    );
    assert!(joined.contains("[y] yes"), "y confirms");
    assert!(
        !joined.contains("[1-4] buy"),
        "the key hint yields to the prompt"
    );
}

/// Composing a coral reef (run not started): the placed coral shows its real
/// body color, and the cursor ghost shows the selected kind's shape in the dim
/// preview tone — the ghost's shape carries the kind, since its color does not.
#[test]
fn placement_ghost_and_placed_rock_take_the_kind() {
    use ratatui::style::Color;

    let mut state = State::new();
    state.score = 12_000 * MICRO; // coral unlocked, budget 2
    state.rocks = vec![Rock { kind: 1, slot: 0 }];
    let mut app = App::new(state, Params::default());
    app.placement_kind = 1; // coral selected
    app.placement_cursor = 4; // ghost on an empty slot
    let terminal = rendered_terminal_of(app, 100, 30);
    let buffer = terminal.backend().buffer();

    // The placed coral is drawn in its real body color (174), not rock gray.
    let placed_coral = (0..30u16)
        .flat_map(|y| (0..100u16).map(move |x| (x, y)))
        .any(|(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(174)));
    assert!(placed_coral, "the placed coral shows its real color");

    // The cursor ghost is the coral shape (a branch glyph) in the dim preview
    // tone (240), so the shape tells the player which kind will drop.
    let ghost = (0..30u16)
        .flat_map(|y| (0..100u16).map(move |x| (x, y)))
        .any(|(x, y)| {
            let cell = buffer.cell((x, y)).expect("cell");
            (cell.symbol() == "╱" || cell.symbol() == "╲")
                && cell.style().fg == Some(Color::Indexed(240))
        });
    assert!(
        ghost,
        "the cursor ghost takes the selected kind's shape, dimmed"
    );
}

/// Placed reefs must stay whole on the placement screen: the free-slot markers
/// may not punch through a placed body, and a cursor over a placed rock grabs
/// it (relit in the cursor tone, keeping its own shape) instead of painting the
/// selected kind's ghost over it.
#[test]
fn placed_reefs_stay_whole_during_placement() {
    use ratatui::style::Color;

    // A rock placed at slot 0 (center column 5, floor row 28 at 100x30) with
    // the cursor elsewhere: the body keeps its center cell.
    let mut state = State::new();
    state.score = 12_000 * MICRO; // budget 2: placement continues after one rock
    state.rocks = vec![Rock { kind: 0, slot: 0 }];
    let terminal = rendered_terminal_with(state.clone(), 100, 30);
    let buffer = terminal.backend().buffer();
    let center = buffer.cell((5, 28)).expect("cell");
    assert_eq!(
        center.symbol(),
        "█",
        "a slot marker must not punch the body"
    );
    assert_eq!(
        center.style().fg,
        Some(Color::Indexed(245)),
        "rock gray body"
    );

    // Cursor on the occupied slot, with coral selected: the rock is grabbed —
    // relit, its own shape — not repainted as a coral ghost.
    let mut app = App::new(state, Params::default());
    app.placement_cursor = 0;
    app.placement_kind = 1;
    let terminal = rendered_terminal_of(app, 100, 30);
    let buffer = terminal.backend().buffer();
    let center = buffer.cell((5, 28)).expect("cell");
    assert_eq!(center.symbol(), "█", "the grabbed rock keeps its shape");
    assert_eq!(
        center.style().fg,
        Some(Color::Indexed(222)),
        "the grabbed rock is relit in the cursor tone"
    );
    let side = buffer.cell((4, 28)).expect("cell");
    assert_eq!(side.symbol(), "▄", "no coral ghost over the placed rock");
}

/// A coral and a kelp reef side by side each render in their own colors — rock
/// body, base-algae tint, and apex individual — so a player tells reefs apart
/// by color, not just position (game-design's "different sea" payoff).
#[test]
fn reef_variants_render_distinct_colors() {
    use ratatui::style::Color;

    let state = State {
        population: [4, 0, 0, 2],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 30_000 * MICRO,
        rocks: vec![Rock { kind: 1, slot: 1 }, Rock { kind: 2, slot: 3 }],
        tick_count: 300,
        started: true,
    };
    let terminal = rendered_terminal_with(state, 100, 30);
    let buffer = terminal.backend().buffer();
    let present = |c: Color| {
        (0..30u16)
            .flat_map(|y| (0..100u16).map(move |x| (x, y)))
            .any(|(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(c))
    };
    assert!(present(Color::Indexed(174)), "coral rock body shows");
    assert!(present(Color::Indexed(58)), "kelp holdfast shows");
    assert!(present(Color::Indexed(37)), "coral base algae shows");
    assert!(present(Color::Indexed(70)), "kelp fronds show");
    assert!(present(Color::Indexed(180)), "the dugong shows over kelp");
    assert!(present(Color::Indexed(209)), "a big fish shows over coral");
}

/// The apex individual takes on its host reef's character: over kelp it is a
/// dugong (its own tan color, fully drawn at 6 cells), never a plain big fish.
/// The kelp base layer shows its own frond color. A valid normal-play state:
/// kelp unlocked (score >= 30000), budget spent (cost 3 <= budget 3), housing
/// emerged (tick_count >= kelp delay 300), the one big-fish slot filled.
#[test]
fn kelp_sea_shows_dugong() {
    use ratatui::style::Color;

    let state = State {
        population: [2, 2, 1, 1],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 30_000 * MICRO,
        rocks: vec![Rock { kind: 2, slot: 2 }],
        tick_count: 300,
        started: true,
    };
    let terminal = rendered_terminal_with(state, 100, 30);
    let buffer = terminal.backend().buffer();

    let count = |c: Color| -> usize {
        (0..30u16)
            .flat_map(|y| (0..100u16).map(move |x| (x, y)))
            .filter(|&(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(c))
            .count()
    };
    assert_eq!(
        count(Color::Indexed(180)),
        6,
        "the dugong shows fully (6 cells)"
    );
    assert_eq!(
        count(Color::Indexed(209)),
        0,
        "no plain big fish over a kelp reef"
    );
    assert!(
        count(Color::Indexed(70)) > 0,
        "kelp fronds show their color"
    );
}

/// The thin side pane is the tank's home, so the dugong (6 cells) must still
/// draw fully there — even at an edge slot, where the patrol window would run
/// off-pane without the clamp. Across a frame sweep all 6 cells stay on-screen.
#[test]
fn dugong_fully_drawn_in_narrow_pane() {
    use ratatui::style::Color;

    let state = State {
        population: [0, 0, 0, 1],
        pool: [0; 4],
        nutrient: 0,
        collectable: 0,
        currency: 0,
        score: 30_000 * MICRO,
        rocks: vec![Rock {
            kind: 2,
            slot: SLOTS - 1,
        }], // edge slot: worst case for the clamp
        tick_count: 300,
        started: true,
    };
    for frame in [0u64, 5, 8, 20, 50, 100] {
        let terminal = rendered_at_frame(state.clone(), 40, 12, frame);
        let buffer = terminal.backend().buffer();
        let cells = (0..12u16)
            .flat_map(|y| (0..40u16).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(180))
            })
            .count();
        assert_eq!(
            cells, 6,
            "the dugong stays fully drawn at 40 wide, frame {frame}"
        );
    }
}

/// The game layer's minimum size (GAME_MIN_WIDTH x GAME_MIN_HEIGHT) must not
/// break: every HUD and placement row stays within the pane (set_stringn
/// clamps), and the new rows are present.
#[test]
fn min_game_size_80x20_renders() {
    let mut state = fixed_state();
    state.collectable = 0;
    let joined = rows_of(&rendered_terminal_with(state, 80, 20), 80, 20).join("\n");
    assert!(joined.contains("score"), "the HUD shows score at min size");
    assert!(
        joined.contains("[n] new sea"),
        "the HUD hint fits at min size"
    );

    let mut placement = State::new();
    placement.score = 30_000 * MICRO; // every kind unlocked
    let joined = rows_of(&rendered_terminal_with(placement, 80, 20), 80, 20).join("\n");
    assert!(
        joined.contains("budget"),
        "the placement budget fits at min size"
    );
    assert!(
        joined.contains("[enter] place"),
        "the placement hint fits at min size"
    );
}

/// Time of day recolors only the water and its surface glow. Each phase paints
/// its confirmed indexed-256 pair (dawn 60/116, day 24/80, dusk 53/216, night
/// 17/31, work/render-observations.md); the same scene is drawn under each. The
/// sprite glyphs are identical across phases (only color moves), which is why
/// this asserts colors, not rows.
#[test]
fn time_of_day_recolors_the_water_and_surface() {
    use ratatui::style::Color;

    // (phase, water bg, surface glow) — the four confirmed palettes.
    let cases = [
        (Phase::Dawn, 60u8, 116u8),
        (Phase::Day, 24, 80),
        (Phase::Dusk, 53, 216),
        (Phase::Night, 17, 31),
    ];
    let (w, h) = (40u16, 12u16);
    for (phase, water, surface) in cases {
        // A plain sea (fixed scene, biomass below the whale, score below the
        // anchor) so only water + waves + reef sprites paint.
        let terminal = rendered_phase_frame(fixed_state(), w, h, 8, phase);
        let buffer = terminal.backend().buffer();

        let water_cells = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                buffer.cell((x, y)).expect("cell").style().bg == Some(Color::Indexed(water))
            })
            .count();
        assert!(
            water_cells > 0,
            "{phase:?}: the water background must be indexed {water}"
        );

        // The surface waves carry the glow color for the phase.
        let wave = (0..w)
            .map(|x| buffer.cell((x, 0)).expect("cell"))
            .find(|c| c.symbol() == "~")
            .expect("a wave crest on the surface row");
        assert_eq!(
            wave.style().fg,
            Some(Color::Indexed(surface)),
            "{phase:?}: the surface glow must be indexed {surface}"
        );
    }
}

/// A state with high biomass but nothing else, so a rendered whale stands alone
/// against the water. Rocks are empty, so every reef sprite (rock, algae,
/// plankton, detritus, fish, sediment) skips; the whale keys off biomass, not
/// the reef, so it still crosses.
fn whale_only_sea() -> State {
    State {
        population: [0, 0, 0, 0],
        pool: [0; 4],
        // Nutrient alone clears the whale's biomass gate (400 units, default).
        nutrient: 400 * MICRO,
        collectable: 0,
        currency: 0,
        score: 0, // below the anchor unlock, so no anchor either
        rocks: vec![],
        tick_count: 0,
        started: true,
    }
}

/// A crossing frame: a whale glides across the sea, drawn in its pale blue-gray
/// (152) over the water. Window 1 hosts a rightward crossing (deterministic in
/// the window hash); frame 1084 places the whale mid-pane. This is the frozen
/// picture of one crossing frame — bless it if the glyph or path changes.
#[test]
fn whale_crossing_snapshot_40x12() {
    let terminal = rendered_at_frame(whale_only_sea(), 40, 12, 1084);
    let actual = rows_of(&terminal, 40, 12);
    let expected = [
        "   ~~   ~~   ~~   ~~   ~~   ~~   ~~   ~~",
        "                       .                ",
        "                      \":\"               ",
        "        |\"\\/\"|     ____:___             ",
        "         \\  /    .,        '`           ",
        "         |  \\___/        O  |           ",
        "                                        ",
        "                                        ",
        "                                        ",
        "                                        ",
        "                                        ",
        "                                        ",
    ];
    assert_snapshot(&actual, &expected);

    // The whale is drawn in its own color, so its cells are countable.
    use ratatui::style::Color;
    let buffer = terminal.backend().buffer();
    let whale_cells = (0..12u16)
        .flat_map(|y| (0..40u16).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(152))
        })
        .count();
    assert!(whale_cells > 0, "the whale paints in indexed 152");
}

/// The whale can hang partly off-screen: put() clamps per cell, so only the
/// on-screen slice draws (unlike the patrol clamp, which fits the whole glyph).
/// Early in the same rightward window the whale is still entering from the left,
/// so its right portion is on-screen and nothing is drawn past the left edge.
#[test]
fn whale_partly_off_screen_draws_its_on_screen_slice() {
    use ratatui::style::Color;

    // Window 1 (rightward). At a small local frame the whale anchor is negative,
    // so its left columns fall off-pane and only the right slice shows.
    let terminal = rendered_at_frame(whale_only_sea(), 40, 12, 960 + 20);
    let buffer = terminal.backend().buffer();
    let whale: Vec<(u16, u16)> = (0..12u16)
        .flat_map(|y| (0..40u16).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(152))
        })
        .collect();

    assert!(!whale.is_empty(), "part of the whale is on-screen");
    // A partial crossing shows fewer cells than a full one (frame 1084).
    let full = rows_of(&rendered_at_frame(whale_only_sea(), 40, 12, 1084), 40, 12)
        .join("")
        .matches(|c: char| !c.is_whitespace())
        .count();
    assert!(
        whale.len() < full,
        "a partial whale shows fewer cells ({}) than a full one ({full})",
        whale.len()
    );
    // Nothing is drawn out of bounds — put() dropped the off-pane columns.
    assert!(
        whale.iter().all(|&(x, _)| x < 40),
        "no whale cell lands past the pane edge"
    );
}

/// Below the biomass threshold the whale never appears, even in a window that
/// would otherwise host a crossing. The visit is earned by a thriving tank.
#[test]
fn whale_stays_away_below_the_biomass_threshold() {
    use ratatui::style::Color;

    let mut state = whale_only_sea();
    state.nutrient = 400 * MICRO - 1; // one under the default gate
                                      // Frame 1084 is a crossing frame for a thriving tank; here biomass is short.
    let terminal = rendered_at_frame(state, 40, 12, 1084);
    let buffer = terminal.backend().buffer();
    let whale = (0..12u16)
        .flat_map(|y| (0..40u16).map(move |x| (x, y)))
        .any(|(x, y)| buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(152)));
    assert!(!whale, "no whale below the biomass threshold");
}

/// A pane too short (under 8 rows) omits the whale — a five-row sprite needs
/// headroom. The same crossing frame at ample height does show it.
#[test]
fn whale_omitted_in_a_short_pane() {
    use ratatui::style::Color;

    let has_whale = |h: u16| {
        let terminal = rendered_at_frame(whale_only_sea(), 40, h, 1084);
        let buffer = terminal.backend().buffer();
        (0..h)
            .flat_map(|y| (0..40u16).map(move |x| (x, y)))
            .any(|(x, y)| {
                buffer.cell((x, y)).expect("cell").style().fg == Some(Color::Indexed(152))
            })
    };
    assert!(!has_whale(7), "no whale in a 7-row pane");
    assert!(has_whale(12), "the whale shows once there is headroom");
}

/// The colors of the sunken-anchor pixel art (iron / worn highlight / rust),
/// mirroring src/ui/wallpaper.rs — a cell painted in any of them is anchor.
fn is_anchor_color(fg: Option<ratatui::style::Color>) -> bool {
    use ratatui::style::Color;
    matches!(
        fg,
        Some(Color::Indexed(66)) | Some(Color::Indexed(109)) | Some(Color::Indexed(94))
    )
}

/// The sunken anchor is a score-unlocked landmark: absent below the unlock,
/// present at or above it, drawn as floor scenery in its half-block pixel-art
/// palette (iron 66 / highlight 109 / rust 94). Rendered on the thin pane (no
/// HUD) so the colors are unambiguous.
#[test]
fn anchor_appears_only_after_its_score_unlock() {
    let anchor_cells = |score: u128| -> usize {
        let mut state = State::new();
        state.score = score;
        state.started = true; // a live sea; the anchor is not a reef, so no rock needed
        let (w, h) = (40u16, 12u16);
        let terminal = rendered_terminal_with(state, w, h);
        let buffer = terminal.backend().buffer();
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| is_anchor_color(buffer.cell((x, y)).expect("cell").style().fg))
            .count()
    };

    // Default unlock is 75,000 (the budget-5 wall). Just under: no anchor. At
    // the unlock: it appears.
    assert_eq!(
        anchor_cells(75_000 * MICRO - 1),
        0,
        "no anchor below the score unlock"
    );
    assert!(
        anchor_cells(75_000 * MICRO) > 0,
        "the anchor appears once the score unlocks it"
    );
}

/// The anchor's fixed 0.8-width column was chosen for the old 5-slot grid, where
/// it fell between the two right-hand slots and cleared every rock body. Under
/// the 9-slot grid its column now lands on slot 7's reef. At the minimum game
/// pane (80x20) the anchor spans columns 62..=66 and slot 7's body covers
/// 65..=67, so the two right anchor cells share a floor column with that body;
/// the anchor draws before the rocks, so the reef overdraws it there (it reads
/// as behind the reef). The other 14 pixel-art cells still render. This is an
/// accepted narrow-grid degradation until #15 makes the anchor position
/// configurable — it does not remove or move the anchor.
#[test]
fn anchor_overdrawn_by_the_slot_seven_reef_at_min_pane() {
    use ratatui::style::Color;

    let (w, h) = (80u16, 20u16);
    let mut state = State::new();
    state.score = 75_000 * MICRO; // anchor unlocked
    state.started = true;
    // A reef on every slot — the crowded worst case for the fixed anchor.
    state.rocks = (0..SLOTS).map(|slot| Rock { kind: 0, slot }).collect();
    let terminal = rendered_terminal_with(state, w, h);
    let buffer = terminal.backend().buffer();

    // Game layer: the tank floor sits above the 5-row HUD, at row 14.
    let floor = 14u16;
    // Slot 7's body (cols 65..=67) overdraws the anchor's two right floor cells.
    for x in [65u16, 66] {
        assert_eq!(
            buffer.cell((x, floor)).expect("cell").style().fg,
            Some(Color::Indexed(245)),
            "the slot-7 reef draws in front of the anchor at column {x}"
        );
    }
    // The rest of the anchor glyph (14 of its 16 cells) still renders.
    let anchor = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| is_anchor_color(buffer.cell((x, y)).expect("cell").style().fg))
        .count();
    assert_eq!(
        anchor, 14,
        "the two overdrawn floor cells aside, the anchor glyph renders"
    );
}

/// The anchor sprite is four rows tall, so a pane too short omits it — and the
/// height guard runs before the bottom-anchored row math, which would otherwise
/// underflow (`floor + 1 - rows` wraps when a 4-row sprite is placed in a 3-row
/// pane). Below the guard height (6) nothing is drawn and nothing panics; at the
/// guard height the anchor appears.
#[test]
fn anchor_omitted_in_a_short_pane() {
    let anchor_cells = |h: u16| -> usize {
        let mut state = State::new();
        state.score = 75_000 * MICRO; // unlocked, so only the height gates it
        state.started = true;
        let w = 40u16;
        let terminal = rendered_terminal_with(state, w, h);
        let buffer = terminal.backend().buffer();
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| is_anchor_color(buffer.cell((x, y)).expect("cell").style().fg))
            .count()
    };
    // Height 3 is the genuine underflow case; 5 is the boundary just under the
    // guard. Both must draw nothing and (by not panicking) prove the guard.
    assert_eq!(
        anchor_cells(3),
        0,
        "no anchor — and no panic — in a 3-row pane"
    );
    assert_eq!(
        anchor_cells(5),
        0,
        "no anchor in a 5-row pane just below the guard"
    );
    assert!(
        anchor_cells(6) > 0,
        "the anchor shows once there is headroom"
    );
}
