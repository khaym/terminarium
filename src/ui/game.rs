//! Game layer: the same tank plus a HUD. Numbers live here and only here —
//! the wallpaper never shows them. Before the run starts this layer is the
//! placement screen instead: an empty tank the player seeds with one rock.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::wallpaper;
use crate::app::App;
use crate::engine::{Species, MICRO, SLOTS, SPECIES};

const HUD_HEIGHT: u16 = 4;
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

    buf.set_stringn(
        left,
        y + 3,
        "[1-4] buy   [q] quit",
        width,
        Style::new().fg(HINT),
    );
}

/// The one-time placement screen: an empty tank with the floor slots marked and
/// a dim rock preview at the cursor. The player moves it and presses enter to
/// seed the run.
fn render_placement(app: &App, area: Rect, buf: &mut Buffer) {
    // Reserve the bottom row for the hint; the tank fills the rest.
    let tank = Rect {
        height: area.height - 1,
        ..area
    };
    wallpaper::render(app, tank, buf);

    buf.set_string(
        area.left() + 1,
        area.top() + 1,
        "place your reef",
        Style::new().fg(TEXT),
    );

    let floor_y = tank.bottom() - 1;
    for slot in 0..SLOTS {
        wallpaper::draw_slot_marker(
            tank,
            buf,
            wallpaper::slot_center_x(tank, slot),
            floor_y,
            MARKER,
        );
    }
    let cursor_x = wallpaper::slot_center_x(tank, app.placement_cursor);
    wallpaper::draw_rock(tank, buf, cursor_x, floor_y, PREVIEW);

    buf.set_string(
        area.left() + 1,
        area.bottom() - 1,
        "[<-/->] move   [enter] place",
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
