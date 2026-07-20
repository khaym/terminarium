//! Game layer: the same tank plus a HUD. Numbers live here and only here —
//! the wallpaper never shows them. Before the run starts this layer is the
//! placement screen instead: an empty tank the player seeds with one rock.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::wallpaper;
use crate::app::App;
use crate::engine::{Species, MICRO, SLOTS, SPECIES};

const HUD_HEIGHT: u16 = 5;
const LABELS: [&str; SPECIES] = ["Algae", "Plankton", "Small fish", "Big fish"];
const ORDER: [Species; SPECIES] = [
    Species::Algae,
    Species::Plankton,
    Species::SmallFish,
    Species::BigFish,
];

const RULE: Color = Color::Indexed(240);
const TEXT: Color = Color::Indexed(252);
const MONEY: Color = Color::Indexed(222);
const FLASH: Color = Color::Indexed(114);
const AFFORDABLE: Color = Color::Indexed(114);
/// Species with no housing left: dimmed, so "no point buying" reads at a glance.
const FULL: Color = Color::Indexed(240);
const HINT: Color = Color::Indexed(245);
/// Placement preview: dimmer than a placed rock, so the ghost reads as tentative.
const PREVIEW: Color = Color::Indexed(240);
/// Placement slot markers: fainter still than the preview.
const MARKER: Color = Color::Indexed(236);
/// A placed rock under the placement cursor: relit warm so it reads as grabbed
/// (what backspace would lift), its shape still telling its kind.
const GRABBED: Color = Color::Indexed(222);

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.height <= HUD_HEIGHT + 2 || area.width < 20 {
        wallpaper::render(app, area, buf);
        return;
    }
    // Before the first rock the game layer is the placement screen.
    if !app.state.run_started() {
        render_placement(app, area, buf);
        return;
    }

    let tank = Rect {
        height: area.height - HUD_HEIGHT,
        ..area
    };
    wallpaper::render(app, tank, buf);

    let left = area.left() + 1;
    let width = usize::from(area.width.saturating_sub(2));
    let y = area.top() + tank.height;

    buf.set_string(
        area.left(),
        y,
        "─".repeat(usize::from(area.width)),
        Style::new().fg(RULE),
    );

    // One segment per species. The ones the player can act on are highlighted;
    // a species with no housing left is dimmed and never lights up, so "can I
    // buy?" is answered by both money and capacity, not money alone.
    let mut x = left;
    let right_edge = left + area.width.saturating_sub(2);
    for (i, &s) in ORDER.iter().enumerate() {
        if x >= right_edge {
            break;
        }
        let cost = app.state.next_cost(s, &app.params);
        let cap = app.state.capacity(i, &app.params);
        let segment = format!(
            "[{}] {} {}/{}  ${}",
            i + 1,
            LABELS[i],
            app.state.population[i],
            cap,
            fmt_cost(cost),
        );
        let full = app.state.population[i] >= cap;
        let affordable = !full && app.state.currency >= cost;
        let style = if full {
            Style::new().fg(FULL)
        } else if affordable {
            Style::new().fg(AFFORDABLE).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(TEXT)
        };
        buf.set_stringn(x, y + 1, &segment, usize::from(right_edge - x), style);
        x = x.saturating_add(segment.len() as u16 + 3);
    }

    let money = format!("$ {}", fmt_amount(app.state.currency));
    buf.set_stringn(
        left,
        y + 2,
        &money,
        width,
        Style::new().fg(MONEY).add_modifier(Modifier::BOLD),
    );
    if let Some((gained, _)) = app.flash {
        buf.set_stringn(
            left + money.len() as u16 + 2,
            y + 2,
            format!("+{} collected", fmt_amount(gained)),
            width.saturating_sub(money.len() + 2),
            Style::new().fg(FLASH),
        );
    }

    // Lifetime score and what it means right now: the reef a rebuild could
    // already use (the standing reason to start a new sea, highlighted like the
    // other actionable segments), or the next threshold to work toward.
    let (line, actionable) = score_line(app);
    let style = if actionable {
        Style::new().fg(AFFORDABLE).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(TEXT)
    };
    buf.set_stringn(left, y + 3, line, width, style);

    // The last row is the key hint, or the new-sea confirmation when armed —
    // a pending prompt takes the row so the choice reads where the keys did.
    if app.new_sea_pending {
        buf.set_stringn(
            left,
            y + 4,
            "start a new sea? score & unlocks stay - [y] yes, any key: no",
            width,
            Style::new().fg(TEXT),
        );
    } else {
        buf.set_stringn(
            left,
            y + 4,
            "[1-4] buy   [n] new sea   [q] quit",
            width,
            Style::new().fg(HINT),
        );
    }
}

/// The HUD score line, and whether it calls for action. The one rule: when the
/// score's budget exceeds what the placed reef spends, a rebuild yields a
/// strictly bigger reef — so the line says what a new sea could place (the
/// newest unlocked kind absent from this reef), and reads as actionable. It
/// clears itself once a rebuild spends the full budget. Otherwise the line
/// shows the next locked threshold as the goal, or just the score once every
/// reef is unlocked. Amounts hide the micro unit, like the rest of the HUD.
fn score_line(app: &App) -> (String, bool) {
    let score = app.state.score;
    let spent: u32 = app
        .state
        .rocks
        .iter()
        .map(|r| app.params.rock_kinds[r.kind].cost)
        .sum();
    if app.params.budget(score) > spent {
        let fresh = (0..app.params.rock_kinds.len())
            .rev()
            .find(|&k| {
                app.params.rock_kinds[k].unlock <= score
                    && !app.state.rocks.iter().any(|r| r.kind == k)
            })
            .map(|k| app.params.rock_kinds[k].name)
            .unwrap_or("a bigger reef");
        return (
            format!(
                "score {}   {fresh} unlocked - [n] new sea",
                fmt_amount(score)
            ),
            true,
        );
    }
    let next = app
        .params
        .rock_kinds
        .iter()
        .map(|k| k.unlock)
        .filter(|&u| u > score)
        .min();
    let line = match next {
        Some(unlock) => format!(
            "score {}   next reef at {}",
            fmt_amount(score),
            fmt_amount(unlock)
        ),
        None => format!("score {}", fmt_amount(score)),
    };
    (line, false)
}

/// The placement screen: an empty tank the player composes a reef into. A panel
/// at the top lists the unlocked kinds with their budget cost (the selected one
/// highlighted), the budget used out of the total the score has unlocked, and
/// the next reef still to unlock. Floor slots are marked, and a dim ghost in the
/// selected kind's shape sits at the cursor. Enter drops, s commits the run.
fn render_placement(app: &App, area: Rect, buf: &mut Buffer) {
    // Reserve the bottom row for the hint; the tank fills the rest.
    let tank = Rect {
        height: area.height - 1,
        ..area
    };
    wallpaper::render(app, tank, buf);

    let left = area.left() + 1;
    let width = usize::from(area.width.saturating_sub(2));
    let right_edge = left + area.width.saturating_sub(2);

    buf.set_string(
        left,
        area.top() + 1,
        "place your reef",
        Style::new().fg(TEXT),
    );

    // Unlocked kinds with their budget cost; the selected one is highlighted so
    // "which will I drop?" reads at a glance.
    let mut x = left;
    for (kind, rk) in app.params.rock_kinds.iter().enumerate() {
        if app.state.score < rk.unlock || x >= right_edge {
            continue;
        }
        let segment = format!("{}({})", rk.name, rk.cost);
        let style = if kind == app.placement_kind {
            Style::new().fg(AFFORDABLE).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(TEXT)
        };
        buf.set_stringn(
            x,
            area.top() + 2,
            &segment,
            usize::from(right_edge - x),
            style,
        );
        x = x.saturating_add(segment.len() as u16 + 2);
    }

    // Budget spent out of the total the score unlocks — reads how much reef is
    // left to place.
    let used: u32 = app
        .state
        .rocks
        .iter()
        .map(|r| app.params.rock_kinds[r.kind].cost)
        .sum();
    let total = app.params.budget(app.state.score);
    buf.set_stringn(
        left,
        area.top() + 3,
        format!("budget {used}/{total}"),
        width,
        Style::new().fg(TEXT),
    );

    // The next reef still to unlock — a target for this run's score.
    if let Some(rk) = app
        .params
        .rock_kinds
        .iter()
        .find(|k| app.state.score < k.unlock)
    {
        buf.set_stringn(
            left,
            area.top() + 4,
            format!("{} unlocks at {}", rk.name, fmt_amount(rk.unlock)),
            width,
            Style::new().fg(HINT),
        );
    }

    // Markers tick the free slots only — a placed reef owns its column, and a
    // marker drawn over it would punch a hole through the body.
    let floor_y = tank.bottom() - 1;
    for slot in 0..SLOTS {
        if app.state.rocks.iter().any(|r| r.slot == slot) {
            continue;
        }
        wallpaper::draw_slot_marker(
            tank,
            buf,
            wallpaper::slot_center_x(tank, slot),
            floor_y,
            MARKER,
        );
    }

    // The cursor shows what acting here affects: on a free slot, a dim ghost of
    // the kind enter would drop; on an occupied slot, the rock backspace would
    // lift, relit in the grab tone (its shape keeps telling its kind).
    let cursor_x = wallpaper::slot_center_x(tank, app.placement_cursor);
    match app
        .state
        .rocks
        .iter()
        .find(|r| r.slot == app.placement_cursor)
    {
        Some(rock) => wallpaper::draw_rock(tank, buf, cursor_x, floor_y, rock.kind, GRABBED),
        None => wallpaper::draw_rock(tank, buf, cursor_x, floor_y, app.placement_kind, PREVIEW),
    }

    buf.set_stringn(
        left,
        area.bottom() - 1,
        "[</>] move  [^/v] kind  [enter] place  [bksp] remove  [s] start",
        width,
        Style::new().fg(HINT),
    );
}

/// Whole currency units; the micro fixed-point precision is an engine detail
/// the player never sees. Money rounds down…
fn fmt_amount(x: u128) -> String {
    (x / MICRO).to_string()
}

/// …and costs round up, so displayed-money ≥ displayed-cost always means the
/// purchase really goes through (never "looks buyable but isn't").
fn fmt_cost(x: u128) -> String {
    x.div_ceil(MICRO).to_string()
}
