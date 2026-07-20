//! Wallpaper layer: the tank as pure decoration — no numbers, no bars, no
//! UI. Everything derives from engine state + frame number through a seeded
//! hash, so a given (state, frame) always renders the same picture (which is
//! what makes snapshot regression possible).
//!
//! The placed rock is the scene's center of mass: sediment mounds under it,
//! algae attach to its sides, detritus rains from its neighbourhood, and the
//! smaller life gathers loosely around it. So the rock -> settle -> collect
//! causality reads from position alone, without a word of text.
//!
//! Character set and palette follow work/render-observations.md: layout uses
//! ASCII + box-drawing + block + braille only, and colors stay in the
//! indexed-256 space so tmux quantization and Windows Terminal's missing
//! COLORTERM cannot degrade them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::engine::{MICRO, SLOTS};

const WATER: Color = Color::Indexed(17);
const SURFACE: Color = Color::Indexed(31);
const FISH: Color = Color::Indexed(215);
const BIG_FISH: Color = Color::Indexed(209);
const PLANKTON: Color = Color::Indexed(122);
const ALGAE: Color = Color::Indexed(35);
const DETRITUS: Color = Color::Indexed(101);
const SEDIMENT: Color = Color::Indexed(94);
/// Rock body — a neutral reef gray in the indexed ramp.
const ROCK: Color = Color::Indexed(245);

/// Collectable surplus represented by one cell of floor sediment.
const SEDIMENT_PER_CELL: u128 = 30 * MICRO;

/// On-screen sprite caps: the wallpaper suggests abundance, it does not chart
/// it — populations beyond the cap just keep the tank at full life.
const MAX_ALGAE: u64 = 12;
const MAX_PLANKTON: u64 = 14;
const MAX_SMALL_FISH: u64 = 8;
const MAX_BIG_FISH: u64 = 4;

/// Look of the base species (algae) for a given rock kind. Structured as a
/// table so a new rock kind's variant is added as data, not code — the first
/// run offers only kind 0, but the shape is ready for the food-web branches.
struct AlgaeVariant {
    /// Two sway frames of the frond glyph.
    fronds: [&'static str; 2],
    color: Color,
}

const ALGAE_VARIANTS: [AlgaeVariant; 1] = [AlgaeVariant {
    fronds: ["(", ")"],
    color: ALGAE,
}];

fn algae_variant(kind: usize) -> &'static AlgaeVariant {
    &ALGAE_VARIANTS[kind % ALGAE_VARIANTS.len()]
}

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let frame = app.frame;
    let w = u64::from(area.width);
    let h = u64::from(area.height);

    // Before a rock is placed the tank is an empty sea: waves only. Every draw
    // below keys off the rocks (or off state that is zero pre-placement), so an
    // empty state renders just the water — the wallpaper grammar is unchanged.
    fill_water(area, buf, frame);
    draw_sediment(app, area, buf);
    draw_rocks(app, area, buf);
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

/// Floor column a slot maps to: the slot's center, `(slot*2+1)*w / (2*SLOTS)`.
/// Placement and rendering share this map so the preview lands where the real
/// rock will.
pub(crate) fn slot_center_x(area: Rect, slot: u8) -> u16 {
    let w = u64::from(area.width);
    let cx = ((u64::from(slot) * 2 + 1) * w) / (2 * u64::from(SLOTS));
    area.left() + cx as u16
}

/// Mean floor column of the placed rocks (a single rock's own column in the
/// first run). Used to anchor the sediment mound directly under the reef.
fn rock_centroid_x(app: &App, area: Rect) -> u16 {
    let rocks = &app.state.rocks;
    let sum: u64 = rocks
        .iter()
        .map(|r| u64::from(slot_center_x(area, r.slot)))
        .sum();
    (sum / rocks.len() as u64) as u16
}

/// A small block-glyph rock cluster sitting on the floor at column `x`. Shared
/// by the live wallpaper (real rock) and the placement screen (dim preview).
pub(crate) fn draw_rock(area: Rect, buf: &mut Buffer, x: u16, y: u16, color: Color) {
    let style = Style::new().fg(color).bg(WATER);
    let bx = i64::from(x);
    put(buf, area, bx - 1, y, "▄", style);
    put(buf, area, bx, y, "█", style);
    put(buf, area, bx + 1, y, "▄", style);
}

/// A single dim floor tick marking a free placement slot.
pub(crate) fn draw_slot_marker(area: Rect, buf: &mut Buffer, x: u16, y: u16, color: Color) {
    put(
        buf,
        area,
        i64::from(x),
        y,
        "▁",
        Style::new().fg(color).bg(WATER),
    );
}

fn draw_rocks(app: &App, area: Rect, buf: &mut Buffer) {
    let y = area.bottom() - 1;
    for rock in &app.state.rocks {
        draw_rock(area, buf, slot_center_x(area, rock.slot), y, ROCK);
    }
}

/// Collected-in-waiting surplus piles up as a mound on the tank floor, centered
/// under the reef (peeking, which collects, visibly clears it). Contiguous so
/// it reads as sediment — scattered cells read as screen noise. The rock is
/// drawn over the mound's center, so a wider mound shows detritus wings around
/// the reef base. Abundance stays readable without a single digit.
fn draw_sediment(app: &App, area: Rect, buf: &mut Buffer) {
    if app.state.rocks.is_empty() {
        return;
    }
    let max = u128::from(area.width.saturating_sub(2));
    let cells = (app.state.collectable / SEDIMENT_PER_CELL).min(max) as u16;
    if cells == 0 {
        return;
    }
    let y = area.bottom() - 1;
    let center = rock_centroid_x(app, area);
    let start = center
        .saturating_sub(cells / 2)
        .max(area.left())
        .min(area.right().saturating_sub(cells));
    for i in 0..cells {
        let edge = cells > 2 && (i == 0 || i + 1 == cells);
        put(
            buf,
            area,
            i64::from(start + i),
            y,
            if edge { "▁" } else { "▂" },
            Style::new().fg(SEDIMENT).bg(WATER),
        );
    }
}

/// Algae attach to the sides of their host rock (never dead-center, so the rock
/// still reads), fanning out with a deterministic spread. The glyph and color
/// come from the rock kind's variant table.
fn draw_algae(app: &App, area: Rect, buf: &mut Buffer, frame: u64) {
    if app.state.rocks.is_empty() {
        return;
    }
    let count = u64::from(app.state.population[0])
        .min(MAX_ALGAE)
        .min(u64::from(area.width) / 4);
    let floor = area.bottom() - 1;
    for i in 0..count {
        let rock = &app.state.rocks[(i as usize) % app.state.rocks.len()];
        let variant = algae_variant(rock.kind);
        let center = slot_center_x(area, rock.slot);
        // Flank the rock body (which spans center +-1), so the reef stays legible.
        let mag = 2 + (mix(i, 7) % 3) as i64; // 2..=4 cells off the rock
        let sign = if i % 2 == 0 { 1 } else { -1 };
        let x = i64::from(center) + sign * mag;
        let height = (2 + mix(i, 3) % 3).min(u64::from(area.height) - 1) as u16;
        for dy in 0..height {
            let y = floor.saturating_sub(dy);
            let sway = (frame / 4 + u64::from(dy) + i).is_multiple_of(2);
            put(
                buf,
                area,
                x,
                y,
                if sway {
                    variant.fronds[0]
                } else {
                    variant.fronds[1]
                },
                Style::new().fg(variant.color).bg(WATER),
            );
        }
    }
}

/// Plankton gather loosely around the reef — biased toward a rock, not pinned
/// to it, so the drift still reads as life rather than a fixed cluster.
fn draw_plankton(app: &App, area: Rect, buf: &mut Buffer, frame: u64, w: u64, h: u64) {
    const DOTS: [&str; 4] = ["⠁", "⠂", "⠄", "⠈"];
    if app.state.rocks.is_empty() {
        return;
    }
    let count = u64::from(app.state.population[1]).min(MAX_PLANKTON);
    let lane = (h - 2).max(1);
    let spread = (w / 4).max(1);
    for i in 0..count {
        let rock = &app.state.rocks[(i as usize) % app.state.rocks.len()];
        let center = slot_center_x(area, rock.slot);
        let offset = (mix(i, 11) % (2 * spread + 1)) as i64 - spread as i64;
        let x = i64::from(center) + offset;
        let y = area.top() + 1 + ((mix(i, 13) + frame / 3) % lane) as u16;
        put(
            buf,
            area,
            x,
            y,
            DOTS[(i % 4) as usize],
            Style::new().fg(PLANKTON).bg(WATER),
        );
    }
}

/// Falling specks hint at the detritus rain feeding the floor — raining from
/// the reef's neighbourhood down toward its mound. A placed rock always sheds,
/// so one speck is the baseline (the rain shows the reef is producing); the
/// count grows with the collectable stock. This keeps the founding causality
/// (rock -> settle -> collect) on screen through the pre-purchase minute, when
/// the game layer collects surplus every second and the stock sits near zero.
fn draw_detritus(app: &App, area: Rect, buf: &mut Buffer, frame: u64, _w: u64, h: u64) {
    if app.state.rocks.is_empty() {
        return;
    }
    let count = (1 + app.state.collectable / (15 * MICRO)).min(5) as u64;
    let lane = (h - 2).max(1);
    for i in 0..count {
        let rock = &app.state.rocks[(i as usize) % app.state.rocks.len()];
        let center = slot_center_x(area, rock.slot);
        let offset = (mix(i, 23) % 7) as i64 - 3; // fall near the reef
        let x = i64::from(center) + offset;
        let y = area.top() + 1 + ((mix(i, 29) + frame / 2) % lane) as u16;
        put(buf, area, x, y, ".", Style::new().fg(DETRITUS).bg(WATER));
    }
}

fn draw_fish(app: &App, area: Rect, buf: &mut Buffer, frame: u64, w: u64, h: u64) {
    let small = u64::from(app.state.population[2]).min(MAX_SMALL_FISH);
    let big = u64::from(app.state.population[3]).min(MAX_BIG_FISH);
    // Small fish keep to the lower lanes near the reef; big fish roam freely.
    for i in 0..small {
        swim(area, buf, frame, w, h, i, "><>", "<><", 1, FISH, true);
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
            false,
        );
    }
}

/// One fish on its own lane, cycling across the tank. It swims fully inside
/// or not at all (no partial clipping); leaving one edge it later re-enters,
/// reading as "swam away, came back". `reef_bias` folds the lane into the
/// lower half so smaller fish keep near the reef.
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
    reef_bias: bool,
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
    let raw = mix(id, 19) % lane;
    let row = if reef_bias {
        lane.saturating_sub(1)
            .saturating_sub(raw % (lane / 2).max(1))
    } else {
        raw
    };
    let y = area.top() + 1 + row as u16;
    let (x, glyph) = if id.is_multiple_of(2) {
        (area.left() + pos as u16, right_glyph)
    } else {
        (area.left() + (w - len - pos) as u16, left_glyph)
    };
    buf.set_string(x, y, glyph, Style::new().fg(color).bg(WATER));
}

/// Set one cell, clamped to `area` (out-of-range coordinates are dropped). Lets
/// callers compute positions with signed offsets without minding the edges.
fn put(buf: &mut Buffer, area: Rect, x: i64, y: u16, sym: &str, style: Style) {
    if x < i64::from(area.left()) || x >= i64::from(area.right()) {
        return;
    }
    if y < area.top() || y >= area.bottom() {
        return;
    }
    if let Some(cell) = buf.cell_mut((x as u16, y)) {
        cell.set_symbol(sym);
        cell.set_style(style);
    }
}

/// Deterministic position hash: the whole animation is a pure function of
/// (state, frame), never of wall clock or an RNG stream.
fn mix(i: u64, salt: u64) -> u64 {
    (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93))
        .rotate_left(31)
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
}
