//! Wallpaper layer: the tank as pure decoration — no numbers, no bars, no
//! UI. Everything derives from engine state + frame number through a seeded
//! hash, so a given (state, frame) always renders the same picture (which is
//! what makes snapshot regression possible).
//!
//! Character set and palette follow work/render-observations.md: layout uses
//! ASCII + braille only, and colors stay in the indexed-256 space so tmux
//! quantization and Windows Terminal's missing COLORTERM cannot degrade them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::engine::MICRO;

const WATER: Color = Color::Indexed(17);
const SURFACE: Color = Color::Indexed(31);
const FISH: Color = Color::Indexed(215);
const BIG_FISH: Color = Color::Indexed(209);
const PLANKTON: Color = Color::Indexed(122);
const ALGAE: Color = Color::Indexed(35);
const DETRITUS: Color = Color::Indexed(101);
const SEDIMENT: Color = Color::Indexed(94);

/// Collectable surplus represented by one cell of floor sediment.
const SEDIMENT_PER_CELL: u128 = 30 * MICRO;

/// On-screen sprite caps: the wallpaper suggests abundance, it does not chart
/// it — populations beyond the cap just keep the tank at full life.
const MAX_ALGAE: u64 = 12;
const MAX_PLANKTON: u64 = 14;
const MAX_SMALL_FISH: u64 = 8;
const MAX_BIG_FISH: u64 = 4;

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let frame = app.frame;
    let w = u64::from(area.width);
    let h = u64::from(area.height);

    fill_water(area, buf, frame);
    draw_sediment(app, area, buf);
    draw_algae(app, area, buf, frame);
    draw_plankton(app, area, buf, frame, w, h);
    draw_detritus(app, area, buf, frame, w, h);
    draw_fish(app, area, buf, frame, w, h);
}

fn fill_water(area: Rect, buf: &mut Buffer, frame: u64) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(Style::new().bg(WATER));
            }
        }
    }
    // Wave crests drifting along the surface row.
    for x in area.left()..area.right() {
        if (u64::from(x - area.left()) + frame / 2) % 5 < 2 {
            if let Some(cell) = buf.cell_mut((x, area.top())) {
                cell.set_symbol("~");
                cell.set_style(Style::new().fg(SURFACE).bg(WATER));
            }
        }
    }
}

/// Collected-in-waiting surplus piles up as a mound on the tank floor;
/// peeking (which collects) visibly clears it. Contiguous and centered so it
/// reads as sediment — scattered cells read as screen noise, and a
/// left-anchored run would read as a progress bar (both break the wallpaper
/// grammar). Abundance stays readable without a single digit.
fn draw_sediment(app: &App, area: Rect, buf: &mut Buffer) {
    let max = u128::from(area.width.saturating_sub(2));
    let cells = (app.state.collectable / SEDIMENT_PER_CELL).min(max) as u16;
    if cells == 0 {
        return;
    }
    let y = area.bottom() - 1;
    let start = area.left() + (area.width - cells) / 2;
    for i in 0..cells {
        let edge = cells > 2 && (i == 0 || i + 1 == cells);
        if let Some(cell) = buf.cell_mut((start + i, y)) {
            cell.set_symbol(if edge { "▁" } else { "▂" });
            cell.set_style(Style::new().fg(SEDIMENT).bg(WATER));
        }
    }
}

fn draw_algae(app: &App, area: Rect, buf: &mut Buffer, frame: u64) {
    let count = u64::from(app.state.population[0])
        .min(MAX_ALGAE)
        .min(u64::from(area.width) / 4);
    if count == 0 {
        return;
    }
    for i in 0..count {
        let x = area.left() + 1 + ((i * (u64::from(area.width) - 2)) / count) as u16;
        let height = (2 + mix(i, 3) % 3).min(u64::from(area.height) - 1) as u16;
        for dy in 0..height {
            let y = area.bottom() - 1 - dy;
            let sway = (frame / 4 + u64::from(dy) + i).is_multiple_of(2);
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(if sway { "(" } else { ")" });
                cell.set_style(Style::new().fg(ALGAE).bg(WATER));
            }
        }
    }
}

fn draw_plankton(app: &App, area: Rect, buf: &mut Buffer, frame: u64, w: u64, h: u64) {
    const DOTS: [&str; 4] = ["⠁", "⠂", "⠄", "⠈"];
    let count = u64::from(app.state.population[1]).min(MAX_PLANKTON);
    let lane = (h - 2).max(1);
    for i in 0..count {
        let x = area.left() + (mix(i, 11) % w) as u16;
        let y = area.top() + 1 + ((mix(i, 13) + frame / 3) % lane) as u16;
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(DOTS[(i % 4) as usize]);
            cell.set_style(Style::new().fg(PLANKTON).bg(WATER));
        }
    }
}

/// Falling specks hint at the detritus rain feeding the floor.
fn draw_detritus(app: &App, area: Rect, buf: &mut Buffer, frame: u64, w: u64, h: u64) {
    let count = (app.state.collectable / (15 * MICRO)).min(5) as u64;
    let lane = (h - 2).max(1);
    for i in 0..count {
        let x = area.left() + (mix(i, 23) % w) as u16;
        let y = area.top() + 1 + ((mix(i, 29) + frame / 2) % lane) as u16;
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(".");
            cell.set_style(Style::new().fg(DETRITUS).bg(WATER));
        }
    }
}

fn draw_fish(app: &App, area: Rect, buf: &mut Buffer, frame: u64, w: u64, h: u64) {
    let small = u64::from(app.state.population[2]).min(MAX_SMALL_FISH);
    let big = u64::from(app.state.population[3]).min(MAX_BIG_FISH);
    for i in 0..small {
        swim(area, buf, frame, w, h, i, "><>", "<><", 1, FISH);
    }
    for i in 0..big {
        swim(
            area,
            buf,
            frame,
            w,
            h,
            i + 100,
            "><)))>",
            "<(((><",
            2,
            BIG_FISH,
        );
    }
}

/// One fish on its own lane, cycling across the tank. It swims fully inside
/// or not at all (no partial clipping); leaving one edge it later re-enters,
/// reading as "swam away, came back".
#[allow(clippy::too_many_arguments)]
fn swim(
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    w: u64,
    h: u64,
    id: u64,
    right_glyph: &str,
    left_glyph: &str,
    slowdown: u64,
    color: Color,
) {
    let len = right_glyph.len() as u64;
    if w <= len {
        return;
    }
    let span = w + len * 2;
    let pos = (mix(id, 17) + frame / slowdown) % span;
    if pos + len > w {
        return; // off-screen part of the cycle
    }
    let lane = (h - 2).max(1);
    let y = area.top() + 1 + (mix(id, 19) % lane) as u16;
    let (x, glyph) = if id.is_multiple_of(2) {
        (area.left() + pos as u16, right_glyph)
    } else {
        (area.left() + (w - len - pos) as u16, left_glyph)
    };
    buf.set_string(x, y, glyph, Style::new().fg(color).bg(WATER));
}

/// Deterministic position hash: the whole animation is a pure function of
/// (state, frame), never of wall clock or an RNG stream.
fn mix(i: u64, salt: u64) -> u64 {
    (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93))
        .rotate_left(31)
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
}
