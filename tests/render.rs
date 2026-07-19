//! TUI snapshot regression (the render layer of the Swiss cheese): both
//! layers are drawn from a fixed (state, frame) and compared cell-by-cell
//! against a frozen picture. Rendering is a pure function of state + frame,
//! so any diff here is a real rendering change — bless it by updating the
//! literals below (a failure prints the actual rows ready to paste).

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tui_game::app::App;
use tui_game::engine::{Params, State, MICRO};
use tui_game::ui;

fn fixed_state() -> State {
    State {
        population: [3, 2, 2, 1],
        pool: [50 * MICRO, MICRO, MICRO, MICRO],
        nutrient: 2 * MICRO,
        collectable: 90 * MICRO,
        currency: 420 * MICRO,
    }
}

/// Render one frame of the fixed scene at the given size.
fn rendered_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    let mut app = App::new(fixed_state(), Params::default());
    app.on_resize(width, height);
    app.frame = 8;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| ui::draw(&app, frame)).expect("draw");
    terminal
}

/// The rendered buffer as text rows.
fn render_at(width: u16, height: u16) -> Vec<String> {
    let terminal = rendered_terminal(width, height);
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).expect("cell").symbol())
                .collect()
        })
        .collect()
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
        "                                  ⠂     ",
        "                                        ",
        "             .                          ",
        ".                                       ",
        "                                        ",
        "                      .                 ",
        "                                        ",
        "                    <><                 ",
        "      ⠁      )            (             ",
        " )           (    .       ) .           ",
        " (           )    ▁▂▁     (             ",
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
        "                                                                                                <>< ",
        "                                                                                      ⠁             ",
        "                     ><)))>                                                                         ",
        "                                                                                                    ",
        "   ><>                                                                                              ",
        "                                                                                              ⠂     ",
        "                                                                                                    ",
        "                                 )                                (                                 ",
        " )                               (                                )                                 ",
        " (                               )                                (                                 ",
        "────────────────────────────────────────────────────────────────────────────────────────────────────",
        " [1] Algae 3  $141   [2] Plankton 2  $126   [3] Small fish 2  $628   [4] Big fish 1  $5600          ",
        " $ 510  +90 collected                                                                               ",
        " [f] feed   [1-4] buy   [q] quit                                                                    ",
    ];
    assert_snapshot(&actual, &expected);
}

/// Requirement from work/render-observations.md: the palette never leaves
/// the indexed-256 space (tmux quantizes RGB; Windows Terminal renders
/// truecolor but does not advertise COLORTERM). Any Rgb or named ANSI color
/// slipping into either layer fails here.
#[test]
fn palette_stays_in_indexed_256_space() {
    use ratatui::style::Color;

    let in_space =
        |c: Option<Color>| matches!(c, None | Some(Color::Reset) | Some(Color::Indexed(_)));

    for (w, h) in [(40u16, 12u16), (100, 30)] {
        let terminal = rendered_terminal(w, h);
        let buffer = terminal.backend().buffer();
        for y in 0..h {
            for x in 0..w {
                let style = buffer.cell((x, y)).expect("cell").style();
                assert!(
                    in_space(style.fg) && in_space(style.bg),
                    "cell ({x},{y}) at {w}x{h} uses a color outside indexed-256: {style:?}"
                );
            }
        }
    }
}

/// Species the player can afford right now are highlighted in the HUD.
/// With $510 on hand: algae ($140) is affordable, small fish ($627) is not.
#[test]
fn affordable_species_are_highlighted() {
    use ratatui::style::Color;

    let mut app = App::new(fixed_state(), Params::default());
    app.on_resize(100, 30);
    app.frame = 8;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| ui::draw(&app, frame)).expect("draw");
    let buffer = terminal.backend().buffer();

    let species_row = 27u16;
    let row: String = (0..100)
        .map(|x| buffer.cell((x, species_row)).expect("cell").symbol())
        .collect();
    let x1 = row.find("[1]").expect("algae segment") as u16;
    let x3 = row.find("[3]").expect("small fish segment") as u16;

    let algae_fg = buffer.cell((x1, species_row)).expect("cell").style().fg;
    let small_fish_fg = buffer.cell((x3, species_row)).expect("cell").style().fg;
    assert_eq!(
        algae_fg,
        Some(Color::Indexed(114)),
        "affordable = highlighted"
    );
    assert_eq!(
        small_fish_fg,
        Some(Color::Indexed(252)),
        "unaffordable = plain"
    );
}
