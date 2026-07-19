//! Game layer: the same tank plus a HUD. Numbers live here and only here —
//! the wallpaper never shows them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::wallpaper;
use crate::app::App;
use crate::engine::{Species, MICRO, SPECIES};

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
const HINT: Color = Color::Indexed(245);

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.height <= HUD_HEIGHT + 2 || area.width < 20 {
        wallpaper::render(app, area, buf);
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

    // One segment per species; the ones the player can afford right now are
    // highlighted, so "can I buy?" is readable at a glance.
    let mut x = left;
    let right_edge = left + area.width.saturating_sub(2);
    for (i, &s) in ORDER.iter().enumerate() {
        if x >= right_edge {
            break;
        }
        let cost = app.state.next_cost(s, &app.params);
        let segment = format!(
            "[{}] {} {}  ${}",
            i + 1,
            LABELS[i],
            app.state.population[i],
            fmt_cost(cost),
        );
        let style = if app.state.currency >= cost {
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
        "[f] feed   [1-4] buy   [q] quit",
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
