//! Development harness: play the game from a script instead of a terminal.
//!
//! A scenario is a line-oriented script — keys, elapsed time, capture points,
//! expectations — run against the same `App` → `ui::draw` path the binary uses,
//! so what it captures is what a player would have seen. It exists so a change
//! can be judged without a real TTY: the reached state is asserted here, and the
//! frames are serialized to ANSI for a human to look at.
//!
//! Nothing in the shipped game reaches this module: only `examples/capture.rs`
//! calls it, and examples are not part of a `cargo install`. It does no I/O
//! either — reading the scenario and writing the dumps belong to that wrapper,
//! which keeps the rules here pure and unit-testable like the rest of the lib.

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, CellWidth};
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

use crate::app::{App, Phase, FRAME_INTERVAL_MS};
use crate::engine::{Params, State};
use crate::ui;

/// A parsed scenario: the steps to run, in order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scenario {
    pub steps: Vec<Step>,
}

/// One scenario line. Every step is something a player does (resize, press a
/// key, let time pass) or something the harness does at that moment (set the
/// render's time-of-day input, capture the frame, check the reached state).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Step {
    Resize { width: u16, height: u16 },
    Key(KeyCode),
    Wait { ms: u64 },
    Phase(Phase),
    Capture { label: String },
    Expect(Expectation),
}

/// State field an expectation reads. Every field yields an integer — `started`
/// as 0/1 — so one comparison rule covers them all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Score,
    Currency,
    Collectable,
    Reefs,
    Started,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Eq,
    AtLeast,
    AtMost,
}

impl Op {
    fn holds(self, actual: u128, value: u128) -> bool {
        match self {
            Op::Eq => actual == value,
            Op::AtLeast => actual >= value,
            Op::AtMost => actual <= value,
        }
    }
}

/// One `expect` line: what to read, how to compare it, and where it was written
/// (the line and its text, so a failure points back at the scenario).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Expectation {
    pub line: usize,
    pub source: String,
    pub field: Field,
    pub op: Op,
    pub value: u128,
}

/// An expectation after it ran: what the state actually held, and whether that
/// satisfied it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Check {
    pub expectation: Expectation,
    pub actual: u128,
    pub passed: bool,
}

/// One captured frame: the label it was taken under, the size it was drawn at,
/// and the frame serialized as ANSI (see `ansi_of`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Capture {
    pub label: String,
    pub width: u16,
    pub height: u16,
    pub ansi: String,
}

/// Everything a run produced: the app as the scenario left it, the frames it
/// captured, and the verdict of every expectation it met on the way.
pub struct Outcome {
    pub app: App,
    pub captures: Vec<Capture>,
    pub checks: Vec<Check>,
}

impl Outcome {
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

/// A malformed scenario. `line` is the line it was found on, or `None` when the
/// fault is the file as a whole (an empty scenario).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub line: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Parse a scenario. A malformed line is the author's mistake, so it surfaces as
/// an error naming the line rather than being skipped: a scenario that half-ran
/// would produce a picture of something nobody asked for.
pub fn parse(text: &str) -> Result<Scenario, ParseError> {
    let mut steps: Vec<Step> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        // Everything from '#' on is a comment, so a step can carry its reason.
        let body = raw.split('#').next().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        let fail = |message: String| ParseError {
            line: Some(line),
            message,
        };
        let mut words = body.split_whitespace();
        let command = words.next().expect("a non-empty body has a first word");
        // The frame size is the whole point of a capture, so no scenario gets an
        // implicit one: the first step has to name it.
        if steps.is_empty() && command != "resize" {
            return Err(fail(
                "a scenario must begin with `resize <W>x<H>`, so the frame size \
                 it captures is never implicit"
                    .to_string(),
            ));
        }
        let step = match command {
            "resize" => {
                let (width, height) = parse_size(words.next()).map_err(&fail)?;
                Step::Resize { width, height }
            }
            "key" => Step::Key(parse_key(words.next()).map_err(&fail)?),
            "wait" => Step::Wait {
                ms: parse_number(words.next(), "wait").map_err(&fail)?,
            },
            "phase" => Step::Phase(parse_phase(words.next()).map_err(&fail)?),
            "capture" => Step::Capture {
                label: words
                    .next()
                    .ok_or_else(|| fail("capture needs a label".to_string()))?
                    .to_string(),
            },
            "expect" => Step::Expect(parse_expectation(line, body, &mut words).map_err(&fail)?),
            other => return Err(fail(format!("unknown command: {other}"))),
        };
        // A leftover token means the line was not the one its author thought.
        if let Some(extra) = words.next() {
            return Err(fail(format!("{command}: unexpected extra token: {extra}")));
        }
        steps.push(step);
    }
    if steps.is_empty() {
        // No line to blame — the file as a whole says nothing.
        return Err(ParseError {
            line: None,
            message: "a scenario needs at least one step, beginning with \
                      `resize <W>x<H>`"
                .to_string(),
        });
    }
    Ok(Scenario { steps })
}

/// `<W>x<H>`, both positive: a zero-column pane draws nothing, which is a
/// mistake rather than a picture.
fn parse_size(token: Option<&str>) -> Result<(u16, u16), String> {
    let token = token.ok_or_else(|| "resize needs a size like 100x30".to_string())?;
    let malformed = || format!("resize needs a size like 100x30: {token}");
    let (width, height) = token.split_once('x').ok_or_else(malformed)?;
    let width: u16 = width.parse().map_err(|_| malformed())?;
    let height: u16 = height.parse().map_err(|_| malformed())?;
    if width == 0 || height == 0 {
        return Err(format!("resize needs a pane with area: {token}"));
    }
    Ok((width, height))
}

/// A visible single character (`n`, `1`, …) or a name for a key that has no
/// glyph. Modifiers are not spelled: nothing but quit reads them, and a scenario
/// that quits has nothing left to capture.
fn parse_key(token: Option<&str>) -> Result<KeyCode, String> {
    let token = token.ok_or_else(|| "key needs a key token".to_string())?;
    let code = match token {
        "space" => KeyCode::Char(' '),
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "backspace" => KeyCode::Backspace,
        _ => {
            let mut chars = token.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return Err(format!("key: unknown key token: {token}")),
            }
        }
    };
    Ok(code)
}

fn parse_number(token: Option<&str>, command: &str) -> Result<u64, String> {
    let token = token.ok_or_else(|| format!("{command} needs a number"))?;
    token
        .parse()
        .map_err(|_| format!("{command}: not a number: {token}"))
}

fn parse_phase(token: Option<&str>) -> Result<Phase, String> {
    let token = token.ok_or_else(|| "phase needs a name".to_string())?;
    match token {
        "dawn" => Ok(Phase::Dawn),
        "day" => Ok(Phase::Day),
        "dusk" => Ok(Phase::Dusk),
        "night" => Ok(Phase::Night),
        other => Err(format!(
            "phase: unknown phase: {other} (dawn, day, dusk or night)"
        )),
    }
}

/// `expect <field> <op> <value>`, except that the boolean field spells its value
/// on its own: `expect started true`. The value's form follows the field, so a
/// number where a boolean belongs is caught here rather than coerced.
fn parse_expectation<'a>(
    line: usize,
    source: &str,
    words: &mut impl Iterator<Item = &'a str>,
) -> Result<Expectation, String> {
    let field = match words.next() {
        Some("score") => Field::Score,
        Some("currency") => Field::Currency,
        Some("collectable") => Field::Collectable,
        Some("reefs") => Field::Reefs,
        Some("started") => Field::Started,
        Some(other) => return Err(format!("expect: unknown field: {other}")),
        None => return Err("expect needs a field".to_string()),
    };
    let (op, value) = if field == Field::Started {
        let value = match words.next() {
            Some("true") => 1,
            Some("false") => 0,
            Some(other) => {
                return Err(format!(
                    "expect started: takes true or false, with no operator: {other}"
                ))
            }
            None => return Err("expect started needs true or false".to_string()),
        };
        (Op::Eq, value)
    } else {
        let op = match words.next() {
            Some("==") => Op::Eq,
            Some(">=") => Op::AtLeast,
            Some("<=") => Op::AtMost,
            Some(other) => return Err(format!("expect: unknown operator: {other} (==, >= or <=)")),
            None => return Err("expect needs an operator (==, >= or <=)".to_string()),
        };
        let token = words
            .next()
            .ok_or_else(|| "expect needs a value".to_string())?;
        let value: u128 = token
            .parse()
            .map_err(|_| format!("expect: not a number: {token}"))?;
        (op, value)
    };
    Ok(Expectation {
        line,
        source: source.to_string(),
        field,
        op,
        value,
    })
}

/// Run a scenario against a fresh game and collect what it produced. Time is the
/// scenario's alone: the app starts at frame zero with no clock scaling, so two
/// runs of one scenario draw the same frames.
pub fn run(scenario: &Scenario) -> Outcome {
    let mut app = App::new(State::new(), Params::default());
    // The parser guarantees a resize before anything else, so this is always
    // overwritten before a capture reads it.
    let (mut width, mut height) = (0, 0);
    // Milliseconds waited that have not yet added up to a whole frame; they
    // carry so many short waits animate at the rate one long wait does.
    let mut frame_debt_ms = 0;
    let mut captures = Vec::new();
    let mut checks = Vec::new();

    for step in &scenario.steps {
        match step {
            Step::Resize {
                width: w,
                height: h,
            } => {
                (width, height) = (*w, *h);
                // A resize carries the real thing with it (collecting the pile on
                // the way into the game layer, releasing a grabbed anchor on the
                // way out) — the same as a player dragging the pane.
                app.on_resize(*w, *h);
            }
            // No scenario spells modifiers: quit is all that reads them, and a
            // quit leaves nothing to capture.
            Step::Key(code) => app.on_key(*code, KeyModifiers::NONE),
            Step::Wait { ms } => {
                app.on_elapsed(*ms);
                // Frames and the economy are independent inside one wait — a
                // frame only ages the animation and the flash, never the tank —
                // so the whole elapsed time goes in first and the frames it owes
                // follow.
                frame_debt_ms += *ms;
                while frame_debt_ms >= FRAME_INTERVAL_MS {
                    frame_debt_ms -= FRAME_INTERVAL_MS;
                    app.on_frame();
                }
            }
            Step::Phase(phase) => app.phase = *phase,
            Step::Capture { label } => captures.push(Capture {
                label: label.clone(),
                width,
                height,
                ansi: ansi_frame(&app, width, height),
            }),
            Step::Expect(expectation) => {
                let actual = read_field(&app, expectation.field);
                checks.push(Check {
                    expectation: expectation.clone(),
                    actual,
                    // A failure is recorded, never fatal: the frames of a run
                    // that missed its goal are how a human sees why it missed.
                    passed: expectation.op.holds(actual, expectation.value),
                });
            }
        }
    }

    Outcome {
        app,
        captures,
        checks,
    }
}

/// Every readable field as an integer, `started` as 0/1 — so one comparison
/// rule covers them all.
fn read_field(app: &App, field: Field) -> u128 {
    match field {
        Field::Score => app.state.score,
        Field::Currency => app.state.currency,
        Field::Collectable => app.state.collectable,
        Field::Reefs => app.state.reefs.len() as u128,
        Field::Started => u128::from(app.state.started),
    }
}

/// Draw one frame of `app` at the given size and serialize it. The draw goes
/// through the same `ui::draw` the binary calls, onto ratatui's own test
/// backend — the picture is the game's, not a re-implementation of it.
fn ansi_frame(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    // Only a backend that cannot report its own size fails here, which the test
    // backend never does.
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal
        .draw(|frame| ui::draw(app, frame))
        .expect("draw into the test backend");
    ansi_of(terminal.backend().buffer())
}

/// Serialize a rendered buffer as ANSI text: one line per row, an escape
/// wherever the style changes, and a reset at every row's end so a dump can be
/// `cat`ed without bleeding style into the shell.
///
/// Each escape is self-contained (it opens with a reset), so no cell depends on
/// what an earlier row did. Foreground, background, and the display modifiers
/// are carried — every cell attribute with a portable SGR encoding. The
/// underline color is not: the renderer never sets one, and its encoding is too
/// thinly supported to be worth a capture that lies about it.
pub fn ansi_of(buffer: &Buffer) -> String {
    let mut out = String::new();
    for y in buffer.area.top()..buffer.area.bottom() {
        let mut open: Option<(Color, Color, Modifier)> = None;
        let mut x = buffer.area.left();
        while x < buffer.area.right() {
            let cell = &buffer[(x, y)];
            let style = (cell.fg, cell.bg, cell.modifier);
            if open != Some(style) {
                out.push_str(&sgr(style));
                open = Some(style);
            }
            out.push_str(cell.symbol());
            // A double-width glyph already paints the column after it, where the
            // buffer parks a blank; writing that blank too would shift the rest
            // of the row. Skipping it is what a real backend does. The floor of 1
            // keeps a zero-width symbol from stalling the row.
            x += cell.cell_width().max(1);
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

/// The escape that opens a style run: a reset, then the attributes that differ
/// from the default. `Color::Reset` needs no code of its own — the leading reset
/// already restored the terminal's own color.
fn sgr((fg, bg, modifier): (Color, Color, Modifier)) -> String {
    let mut params = vec!["0".to_string()];
    for &(flag, code) in MODIFIER_CODES {
        if modifier.contains(flag) {
            params.push(code.to_string());
        }
    }
    params.extend(color_params(fg, false));
    params.extend(color_params(bg, true));
    format!("\x1b[{}m", params.join(";"))
}

/// The display modifiers and their SGR codes. The whole set ratatui can carry,
/// not just the ones the renderer uses today: a dropped attribute would make a
/// capture quietly disagree with the screen it stands in for.
const MODIFIER_CODES: &[(Modifier, &str)] = &[
    (Modifier::BOLD, "1"),
    (Modifier::DIM, "2"),
    (Modifier::ITALIC, "3"),
    (Modifier::UNDERLINED, "4"),
    (Modifier::SLOW_BLINK, "5"),
    (Modifier::RAPID_BLINK, "6"),
    (Modifier::REVERSED, "7"),
    (Modifier::HIDDEN, "8"),
    (Modifier::CROSSED_OUT, "9"),
];

/// SGR parameters for one color, as a foreground or a background. The two differ
/// by a fixed offset (30 vs 40, 38 vs 48), so one table serves both.
fn color_params(color: Color, background: bool) -> Option<String> {
    let (base, extended) = if background { (40, 48) } else { (30, 38) };
    // The eight original colors count up from the base; their bright twins sit
    // 60 above (90-97 / 100-107).
    let basic = |n: u8| (base + n).to_string();
    let bright = |n: u8| (base + 60 + n).to_string();
    Some(match color {
        Color::Reset => return None,
        Color::Black => basic(0),
        Color::Red => basic(1),
        Color::Green => basic(2),
        Color::Yellow => basic(3),
        Color::Blue => basic(4),
        Color::Magenta => basic(5),
        Color::Cyan => basic(6),
        Color::Gray => basic(7),
        Color::DarkGray => bright(0),
        Color::LightRed => bright(1),
        Color::LightGreen => bright(2),
        Color::LightYellow => bright(3),
        Color::LightBlue => bright(4),
        Color::LightMagenta => bright(5),
        Color::LightCyan => bright(6),
        Color::White => bright(7),
        Color::Indexed(n) => format!("{extended};5;{n}"),
        Color::Rgb(r, g, b) => format!("{extended};2;{r};{g};{b}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    fn parsed(text: &str) -> Scenario {
        parse(text).expect("scenario parses")
    }

    fn error(text: &str) -> ParseError {
        parse(text).expect_err("scenario is rejected")
    }

    #[test]
    fn every_command_parses_into_its_step() {
        let scenario = parsed(concat!(
            "resize 100x30\n",
            "key enter\n",
            "key s\n",
            "wait 1000\n",
            "phase dawn\n",
            "capture full\n",
            "expect score >= 1\n",
        ));
        assert_eq!(
            scenario.steps,
            vec![
                Step::Resize {
                    width: 100,
                    height: 30
                },
                Step::Key(KeyCode::Enter),
                Step::Key(KeyCode::Char('s')),
                Step::Wait { ms: 1_000 },
                Step::Phase(Phase::Dawn),
                Step::Capture {
                    label: "full".to_string()
                },
                Step::Expect(Expectation {
                    line: 7,
                    source: "expect score >= 1".to_string(),
                    field: Field::Score,
                    op: Op::AtLeast,
                    value: 1,
                }),
            ]
        );
    }

    #[test]
    fn comments_and_blank_lines_carry_no_steps() {
        let scenario = parsed(concat!(
            "# the opening pane\n",
            "\n",
            "resize 80x20   # the smallest playable size\n",
            "   \n",
            "wait 200\n",
        ));
        assert_eq!(
            scenario.steps,
            vec![
                Step::Resize {
                    width: 80,
                    height: 20
                },
                Step::Wait { ms: 200 },
            ]
        );
    }

    #[test]
    fn a_comment_is_stripped_from_an_expectations_source() {
        let scenario = parsed("resize 80x20\nexpect reefs == 2  # both reefs landed\n");
        let Step::Expect(expectation) = &scenario.steps[1] else {
            panic!("expected an expectation");
        };
        assert_eq!(expectation.source, "expect reefs == 2");
    }

    #[test]
    fn a_scenario_must_begin_with_a_resize() {
        // The frame size is the whole point of a capture, so an implicit size is
        // never assumed: the first step has to name one.
        let err = error("key s\nresize 80x20\n");
        assert_eq!(err.line, Some(1));
        assert!(err.message.contains("resize"), "{}", err.message);

        // A scenario with no steps at all has no line to point at.
        let err = error("# nothing but a comment\n");
        assert_eq!(err.line, None);
        assert!(err.message.contains("resize"), "{}", err.message);
    }

    #[test]
    fn an_unknown_command_names_its_line() {
        let err = error("resize 80x20\nwait 100\nsprint 3\n");
        assert_eq!(err.line, Some(3));
        assert!(err.message.contains("sprint"), "{}", err.message);
    }

    #[test]
    fn a_malformed_size_names_its_line() {
        assert_eq!(error("resize 80\n").line, Some(1));
        assert_eq!(error("resize eightyx20\n").line, Some(1));
        assert_eq!(error("resize 80x20x3\n").line, Some(1));
        // A zero-column pane draws nothing; that is a mistake, not a picture.
        assert_eq!(error("resize 0x20\n").line, Some(1));
        assert_eq!(error("resize 80x0\n").line, Some(1));
    }

    #[test]
    fn key_tokens_are_a_single_character_or_a_name() {
        let scenario = parsed(concat!(
            "resize 80x20\n",
            "key n\n",
            "key 1\n",
            "key space\n",
            "key enter\n",
            "key esc\n",
            "key left\n",
            "key right\n",
            "key up\n",
            "key down\n",
            "key backspace\n",
        ));
        assert_eq!(
            scenario.steps[1..],
            [
                Step::Key(KeyCode::Char('n')),
                Step::Key(KeyCode::Char('1')),
                Step::Key(KeyCode::Char(' ')),
                Step::Key(KeyCode::Enter),
                Step::Key(KeyCode::Esc),
                Step::Key(KeyCode::Left),
                Step::Key(KeyCode::Right),
                Step::Key(KeyCode::Up),
                Step::Key(KeyCode::Down),
                Step::Key(KeyCode::Backspace),
            ]
        );
    }

    #[test]
    fn an_unnamed_multi_character_key_is_rejected() {
        let err = error("resize 80x20\nkey ctrl\n");
        assert_eq!(err.line, Some(2));
        assert!(err.message.contains("ctrl"), "{}", err.message);
        assert_eq!(error("resize 80x20\nkey\n").line, Some(2));
    }

    #[test]
    fn a_bad_wait_or_phase_names_its_line() {
        assert_eq!(error("resize 80x20\nwait soon\n").line, Some(2));
        assert_eq!(error("resize 80x20\nwait\n").line, Some(2));
        assert_eq!(error("resize 80x20\nphase noon\n").line, Some(2));
        assert_eq!(error("resize 80x20\ncapture\n").line, Some(2));
    }

    #[test]
    fn every_phase_has_a_token() {
        let scenario = parsed("resize 80x20\nphase dawn\nphase day\nphase dusk\nphase night\n");
        assert_eq!(
            scenario.steps[1..],
            [
                Step::Phase(Phase::Dawn),
                Step::Phase(Phase::Day),
                Step::Phase(Phase::Dusk),
                Step::Phase(Phase::Night),
            ]
        );
    }

    #[test]
    fn every_expect_field_and_operator_parses() {
        let scenario = parsed(concat!(
            "resize 80x20\n",
            "expect score == 0\n",
            "expect currency >= 1\n",
            "expect collectable <= 2\n",
            "expect reefs == 3\n",
            "expect started true\n",
            "expect started false\n",
        ));
        let fields: Vec<(Field, Op, u128)> = scenario.steps[1..]
            .iter()
            .map(|step| {
                let Step::Expect(e) = step else {
                    panic!("expected an expectation");
                };
                (e.field, e.op, e.value)
            })
            .collect();
        assert_eq!(
            fields,
            vec![
                (Field::Score, Op::Eq, 0),
                (Field::Currency, Op::AtLeast, 1),
                (Field::Collectable, Op::AtMost, 2),
                (Field::Reefs, Op::Eq, 3),
                // `started` is read as 0/1, so one comparison rule fits every
                // field; its own token stays true/false.
                (Field::Started, Op::Eq, 1),
                (Field::Started, Op::Eq, 0),
            ]
        );
    }

    #[test]
    fn a_bad_expectation_names_its_line() {
        assert_eq!(error("resize 80x20\nexpect biomass >= 1\n").line, Some(2));
        assert_eq!(error("resize 80x20\nexpect score > 1\n").line, Some(2));
        assert_eq!(error("resize 80x20\nexpect score >= lots\n").line, Some(2));
        assert_eq!(error("resize 80x20\nexpect score >= 1 2\n").line, Some(2));
        // A boolean field takes a boolean, and a numeric field a number —
        // mixing them is a mistake worth surfacing, not a silent coercion.
        assert_eq!(error("resize 80x20\nexpect started 1\n").line, Some(2));
        assert_eq!(error("resize 80x20\nexpect score >= true\n").line, Some(2));
    }

    #[test]
    fn trailing_tokens_are_rejected() {
        assert_eq!(error("resize 80x20 tall\n").line, Some(1));
        assert_eq!(error("resize 80x20\nwait 100 200\n").line, Some(2));
        assert_eq!(error("resize 80x20\ncapture full wide\n").line, Some(2));
    }

    #[test]
    fn keys_reach_the_app_through_the_real_input_path() {
        let outcome = run(&parsed("resize 100x30\nkey enter\nkey s\n"));
        assert!(outcome.app.state.run_started(), "enter placed, s started");
        assert_eq!(outcome.app.state.reefs.len(), 1);
    }

    #[test]
    fn a_wait_advances_the_economy_and_the_animation_together() {
        // One wait is both kinds of time: engine ticks (1 s each) and animation
        // frames at the binary's frame length, so a captured frame sits where the
        // real loop would have put it.
        let outcome = run(&parsed("resize 100x30\nkey enter\nkey s\nwait 1000\n"));
        assert_eq!(outcome.app.state.tick_count, 1, "1000ms is one engine tick");
        assert_eq!(
            outcome.app.frame,
            1_000 / FRAME_INTERVAL_MS,
            "and five animation frames"
        );
    }

    #[test]
    fn the_frame_remainder_carries_into_the_next_wait() {
        // Frames are whole: what a wait leaves over stays with the runner, so
        // many short waits animate at the same rate as one long one.
        let outcome = run(&parsed("resize 100x30\nkey enter\nkey s\nwait 500\n"));
        assert_eq!(outcome.app.frame, 2, "500ms is two whole frames, 100 left");

        let outcome = run(&parsed(
            "resize 100x30\nkey enter\nkey s\nwait 500\nwait 100\n",
        ));
        assert_eq!(outcome.app.frame, 3, "the carried 100ms completes a third");
    }

    #[test]
    fn phase_sets_the_renders_time_of_day() {
        let outcome = run(&parsed("resize 100x30\nphase dawn\n"));
        assert_eq!(outcome.app.phase, Phase::Dawn);
    }

    #[test]
    fn a_capture_records_the_frame_at_the_current_size() {
        let outcome = run(&parsed(
            "resize 100x30\ncapture full\nresize 40x12\ncapture pane\n",
        ));
        let sizes: Vec<(&str, u16, u16)> = outcome
            .captures
            .iter()
            .map(|c| (c.label.as_str(), c.width, c.height))
            .collect();
        assert_eq!(sizes, vec![("full", 100, 30), ("pane", 40, 12)]);
        assert_eq!(outcome.captures[0].ansi.lines().count(), 30);
        assert_eq!(outcome.captures[1].ansi.lines().count(), 12);
    }

    #[test]
    fn expectations_are_judged_against_the_state_they_read() {
        let outcome = run(&parsed(concat!(
            "resize 100x30\n",
            "expect started false\n",
            "expect reefs == 0\n",
            "key enter\n",
            "key s\n",
            "expect started true\n",
            "expect reefs == 1\n",
            "expect currency >= 1\n",
            "expect collectable == 0\n",
        )));
        assert!(outcome.all_passed(), "checks: {:?}", outcome.checks);
        assert_eq!(outcome.checks.len(), 6);
    }

    #[test]
    fn a_failed_expectation_reports_the_actual_value_and_the_run_goes_on() {
        // A capture is wanted even from a run that missed its goal — that frame
        // is how a human sees *why* it missed. So a failure is recorded, not
        // fatal, and every later step still happens.
        let outcome = run(&parsed(concat!(
            "resize 100x30\n",
            "expect reefs == 1\n",
            "capture after-the-failure\n",
            "expect started false\n",
        )));
        assert!(!outcome.all_passed());
        assert_eq!(outcome.checks.len(), 2);
        assert!(!outcome.checks[0].passed);
        assert_eq!(outcome.checks[0].actual, 0, "no reef was ever placed");
        assert_eq!(outcome.checks[0].expectation.line, 2);
        assert_eq!(outcome.checks[0].expectation.source, "expect reefs == 1");
        assert!(
            outcome.checks[1].passed,
            "the run carried on past the failure"
        );
        assert_eq!(outcome.captures.len(), 1, "and still captured its frame");
    }

    #[test]
    fn ansi_writes_one_escape_per_style_run_and_resets_each_row() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 2));
        let teal = Style::new().fg(Color::Indexed(44));
        buffer.set_string(0, 0, "ab", teal);
        buffer.set_string(
            2,
            0,
            "C",
            Style::new()
                .bg(Color::Indexed(17))
                .add_modifier(Modifier::BOLD),
        );
        assert_eq!(
            ansi_of(&buffer),
            concat!(
                // one escape opens the teal run, none between a and b
                "\x1b[0;38;5;44mab",
                "\x1b[0;1;48;5;17mC",
                // the untouched last cell is the default style again
                "\x1b[0m \x1b[0m\n",
                "\x1b[0m    \x1b[0m\n",
            )
        );
    }

    #[test]
    fn ansi_covers_named_and_true_colors() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        buffer.set_string(0, 0, "x", Style::new().fg(Color::Red).bg(Color::White));
        buffer.set_string(1, 0, "y", Style::new().fg(Color::DarkGray).bg(Color::Blue));
        buffer.set_string(2, 0, "z", Style::new().fg(Color::Rgb(1, 2, 3)));
        assert_eq!(
            ansi_of(&buffer),
            concat!(
                "\x1b[0;31;107mx",
                "\x1b[0;90;44my",
                "\x1b[0;38;2;1;2;3mz",
                "\x1b[0m\n",
            )
        );
    }

    #[test]
    fn a_double_width_glyph_covers_the_column_after_it() {
        // The buffer parks an empty cell behind a wide glyph (the glyph already
        // paints that column). Writing that cell out too would shift the rest of
        // the row, so the dump skips it exactly as a real backend does.
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
        buffer.set_string(0, 0, "コa", Style::new());
        assert_eq!(ansi_of(&buffer), "\x1b[0mコa \x1b[0m\n");
    }
}
