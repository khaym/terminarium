//! Wallpaper layer: the tank as pure decoration — no numbers, no bars, no
//! UI. Everything derives from engine state + frame number through a seeded
//! hash, so a given (state, frame) always renders the same picture (which is
//! what makes snapshot regression possible).
//!
//! The placed rock is the scene's center of mass: sediment mounds under it,
//! algae attach to its sides, detritus rains from its neighbourhood, and the
//! swimming life patrols a bounded window around it (each rock its own reef).
//! So the rock -> settle -> collect causality, and where the reef sits, read
//! from position alone, without a word of text.
//!
//! Sprite positions are assigned from an individual's ordinal within its host
//! rock, not drawn from an independent hash: distinct ordinals map to distinct
//! in-pane columns (algae, plankton) or lanes/phases (fish), so every bought
//! individual shows wherever the pane has room, and no two of a kind within a
//! rock's colony share a cell. (Colonies on different rocks can still cross —
//! the first run places a single rock; multi-rock spacing is a later concern.)
//! The `mix()` hash is left only for freedoms that cannot cause overlap (frond
//! height and sway, plankton drift, patrol phase).
//!
//! Character set and palette follow work/render-observations.md: layout uses
//! ASCII + box-drawing + block + braille only, and colors stay in the
//! indexed-256 space so tmux quantization and Windows Terminal's missing
//! COLORTERM cannot degrade them.
//!
//! Two overlays give the sea its moods, both still pure functions of the render
//! inputs. Time of day (`App::phase`) recolors only the water and its surface
//! glow — every sprite color is unchanged, since all read on all four
//! backgrounds (observed). Night is the original palette, so a night render is
//! byte-for-byte the pre-phase picture. A visiting whale (`draw_whale`) is pure
//! decoration outside the economy: gated on total biomass, it crosses in only
//! 1-in-K deterministic frame windows so a sighting stays a rare, reproducible
//! treat. The sunken-anchor scenery (`draw_anchor`) is a score-unlocked
//! landmark, derived from score alone so it costs no save field. Neither touches
//! population, pool, or the save — they only read the gate quantities.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::{App, Phase};
use crate::engine::{Rock, MICRO, SLOTS};

/// Water background and surface-glow colors for a time-of-day phase — the
/// confirmed indexed-256 palette (work/render-observations.md). Night carries
/// the original sea colors, so the palette is invisible until the clock moves.
pub(crate) fn water_color(phase: Phase) -> Color {
    match phase {
        Phase::Dawn => Color::Indexed(60),
        Phase::Day => Color::Indexed(24),
        Phase::Dusk => Color::Indexed(53),
        Phase::Night => Color::Indexed(17),
    }
}

fn surface_color(phase: Phase) -> Color {
    match phase {
        Phase::Dawn => Color::Indexed(116),
        Phase::Day => Color::Indexed(80),
        // Dusk keeps the dark background; its mood is the surface glow (216).
        Phase::Dusk => Color::Indexed(216),
        Phase::Night => Color::Indexed(31),
    }
}

const FISH: Color = Color::Indexed(215);
const BIG_FISH: Color = Color::Indexed(209);
const PLANKTON: Color = Color::Indexed(122);
const ALGAE: Color = Color::Indexed(35);
const DETRITUS: Color = Color::Indexed(101);
const SEDIMENT: Color = Color::Indexed(94);
/// Rock body — a neutral reef gray in the indexed ramp.
const ROCK: Color = Color::Indexed(245);
/// Coral rock body — a warm rose, well clear of the orange fish tones.
const CORAL: Color = Color::Indexed(174);
/// Kelp holdfast — a muted olive anchoring the frond forest to the floor.
const KELP_HOLDFAST: Color = Color::Indexed(58);
/// Base algae tint for the coral reef — a denser teal-green than plain rock.
const CORAL_ALGAE: Color = Color::Indexed(37);
/// Kelp fronds — a taller, brighter forest green; the reef's whole character.
const KELP_ALGAE: Color = Color::Indexed(70);
/// The dugong: the kelp forest's apex grazer, a warm tan sea cow.
const DUGONG: Color = Color::Indexed(180);

/// Collectable surplus represented by one cell of floor sediment.
const SEDIMENT_PER_CELL: u128 = 30 * MICRO;

/// On-screen sprite caps. A bought individual always shows (feedback (a)), so
/// each cap sits at or above the largest population the default params can field
/// — any reef within the max budget and the slot count. They bite only on an
/// out-of-range state (e.g. a tampered save), never in real play. The
/// `sprite_caps_cover_every_reachable_population` test pins them to the params.
const MAX_ALGAE: u64 = 20;
const MAX_PLANKTON: u64 = 15;
const MAX_SMALL_FISH: u64 = 12;
const MAX_BIG_FISH: u64 = 7;

/// Patrol radius (cells either side of the host rock) by fish size. Small fish
/// stay tight to the reef; big fish sweep a wider, statelier beat.
const SMALL_RADIUS: i64 = 5;
const BIG_RADIUS: i64 = 10;

/// The visiting whale — a pale blue-gray (152) that reads on every water
/// background (observed, work/render-observations.md).
const WHALE_COLOR: Color = Color::Indexed(152);
/// Nominal glyph size. The whale enters fully off one edge and exits fully off
/// the other, so the crossing spans `width + WHALE_WIDTH` columns.
const WHALE_WIDTH: i64 = 23;
/// Panes shorter than this omit the whale — a five-row sprite needs headroom.
const WHALE_MIN_HEIGHT: u16 = 8;
/// Frames per crossing window. At ~5 fps this is a few minutes, and only a
/// fraction of a window is spent on-screen, so a sighting is scarce.
const WHALE_PERIOD: u64 = 960;
/// A crossing happens in only 1 of every K windows — the rarity that makes the
/// whale a lucky sight rather than a fixture.
const WHALE_RARITY: u64 = 4;
/// Frames per column step; higher is slower. The whale ambles slower than the
/// big fish (slowdown 2) and the dugong (3).
const WHALE_SLOWDOWN: u64 = 4;
/// Hash salt for the window gate. One hash decides both rarity (low bits) and
/// heading (bit 2), keeping the cadence a pure function of the window number.
const WHALE_SALT: u64 = 41;

/// The whale, 23x5, facing left (head and eye at the left, tail flukes at the
/// right): exactly the D1 glyph the observation gate approved (WHALE_A in
/// work/termprobe.sh). Drawn char-by-char with spaces skipped, so a character's
/// index in its row is its column offset from the sprite's left edge.
const WHALE_LEFT: [&str; 5] = [
    "       .",
    "      \":\"",
    "    ___:____     |\"\\/\"|",
    "  ,'        `.    \\  /",
    "  |  O        \\___/  |",
];

/// The same whale hand-mirrored to face right (head and eye at the right, flukes
/// at the left): `/`\`\``, `,`, and `` ` `` swapped and each row reversed within
/// the 23-column field, so the spout and head stay aligned over the body.
const WHALE_RIGHT: [&str; 5] = [
    "               .",
    "              \":\"",
    "|\"\\/\"|     ____:___",
    " \\  /    .,        '`",
    " |  \\___/        O  |",
];

/// The sunken-anchor landmark, drawn as half-block pixel art (from
/// work/glyphgen.py `PIXEL_ANCHOR_S`): iron body (66), a worn highlight on the
/// lit edge (109), and rust patches (94) — clear of the game HUD's 240, each
/// holding contrast on every water background. The rust deliberately shares
/// SEDIMENT's 94: both read as the same silted-brown age, and they never sit
/// adjacent — sediment lives on the floor row, where the anchor's own bottom
/// row (iron) overdraws it. Five cells wide by four rows,
/// bottom-anchored to the floor. Every filled cell paints over the phase water
/// except the ring's two solid arcs, whose lower pixel is iron so the ring's
/// hole reads against the water between them (`iron_bg`). Half-block charset
/// (▀▄█), which the observation gate approved for a colored pixel sprite.
const ANCHOR_IRON: u8 = 66;
const ANCHOR_HIGHLIGHT: u8 = 109;
const ANCHOR_RUST: u8 = 94;

/// Panes shorter than this omit the anchor: the sprite is four rows tall and
/// needs a little headroom, and the guard also keeps the bottom-anchored row
/// math (`floor + 1 - rows`) from underflowing in a tiny pane.
const ANCHOR_MIN_HEIGHT: u16 = 6;

/// One filled cell of the anchor pixel art: its half-block glyph, foreground
/// color index, and whether its background is the fixed iron tone. `iron_bg`
/// marks only the ring's two solid arcs; everywhere else the phase water shows
/// through.
struct AnchorCell {
    sym: &'static str,
    fg: u8,
    iron_bg: bool,
}

impl AnchorCell {
    /// A cell whose lower pixel is the phase water (the common case).
    const fn water(sym: &'static str, fg: u8) -> Option<Self> {
        Some(Self {
            sym,
            fg,
            iron_bg: false,
        })
    }
    /// A cell whose lower pixel is iron — the ring's solid arc over the hole.
    const fn iron(sym: &'static str, fg: u8) -> Option<Self> {
        Some(Self {
            sym,
            fg,
            iron_bg: true,
        })
    }
}

/// The anchor as a 4-row by 5-cell table of half-block cells (`None` = water).
/// Read top to bottom: the ring, the stock, the shank with flaring flukes, and
/// the arms curving up to the fluke tips. Its 16 filled cells are the pixel art
/// emitted by work/glyphgen.py `PIXEL_ANCHOR_S`.
const ANCHOR: [[Option<AnchorCell>; 5]; 4] = [
    // ring — the center's lower pixel stays water, so the eye reads a hole.
    [
        None,
        AnchorCell::iron("▀", ANCHOR_HIGHLIGHT),
        AnchorCell::water("▀", ANCHOR_HIGHLIGHT),
        AnchorCell::iron("▀", ANCHOR_HIGHLIGHT),
        None,
    ],
    // stock — the crossbar seated on the shank.
    [
        AnchorCell::water("▄", ANCHOR_HIGHLIGHT),
        AnchorCell::water("▄", ANCHOR_HIGHLIGHT),
        AnchorCell::water("█", ANCHOR_IRON),
        AnchorCell::water("▄", ANCHOR_RUST),
        AnchorCell::water("▄", ANCHOR_RUST),
    ],
    // shank — the spine, with the first hint of the fluke tips to each side.
    [
        AnchorCell::water("▄", ANCHOR_IRON),
        None,
        AnchorCell::water("█", ANCHOR_IRON),
        None,
        AnchorCell::water("▄", ANCHOR_RUST),
    ],
    // arms — curving up to the upturned fluke tips.
    [
        AnchorCell::water("▀", ANCHOR_IRON),
        AnchorCell::water("▄", ANCHOR_IRON),
        AnchorCell::water("█", ANCHOR_IRON),
        AnchorCell::water("▄", ANCHOR_IRON),
        AnchorCell::water("▀", ANCHOR_IRON),
    ],
];

/// Look of the base species (algae) for a given rock kind. Structured as a
/// table so a new rock kind's variant is added as data, not code. Each kind's
/// base layer looks different, so a reef reads by its greenery alone.
struct AlgaeVariant {
    /// Two sway frames of the frond glyph.
    fronds: [&'static str; 2],
    color: Color,
}

const ALGAE_VARIANTS: [AlgaeVariant; 3] = [
    // rock: the founding sea's sparse fronds (unchanged look).
    AlgaeVariant {
        fronds: ["(", ")"],
        color: ALGAE,
    },
    // coral: a denser, teal-tinged base layer.
    AlgaeVariant {
        fronds: ["{", "}"],
        color: CORAL_ALGAE,
    },
    // kelp: tall swaying blades — the frond forest is this reef's signature.
    AlgaeVariant {
        fronds: ["\\", "/"],
        color: KELP_ALGAE,
    },
];

fn algae_variant(kind: usize) -> &'static AlgaeVariant {
    &ALGAE_VARIANTS[kind % ALGAE_VARIANTS.len()]
}

/// Look of the rock body itself for a given kind — same table shape as the
/// algae and big-fish variants, so a new kind is a data row, not new code. The
/// glyph tells kinds apart even in the dim placement ghost, where color does
/// not; color carries the difference for a placed reef.
struct RockVariant {
    /// Three body glyphs drawn left / center / right of the rock column.
    body: [&'static str; 3],
    color: Color,
}

const ROCK_VARIANTS: [RockVariant; 3] = [
    // rock: a low block mound.
    RockVariant {
        body: ["▄", "█", "▄"],
        color: ROCK,
    },
    // coral: branches spreading up from a stem.
    RockVariant {
        body: ["╱", "█", "╲"],
        color: CORAL,
    },
    // kelp: a solid, wide holdfast.
    RockVariant {
        body: ["▙", "█", "▟"],
        color: KELP_HOLDFAST,
    },
];

fn rock_variant(kind: usize) -> &'static RockVariant {
    &ROCK_VARIANTS[kind % ROCK_VARIANTS.len()]
}

/// Look of the apex individual for a given host-rock kind. A big fish over rock
/// and coral; a slower, tan dugong over kelp — the top species takes on its
/// reef's character. Keyed only by the host rock's kind, so it stays a pure
/// function of (state, frame) like every other sprite.
struct BigVariant {
    right: &'static str,
    left: &'static str,
    /// Frames per column step; higher is slower (the dugong ambles).
    slowdown: u64,
    color: Color,
}

const BIG_VARIANTS: [BigVariant; 3] = [
    // rock
    BigVariant {
        right: "><)))>",
        left: "<(((><",
        slowdown: 2,
        color: BIG_FISH,
    },
    // coral
    BigVariant {
        right: "><)))>",
        left: "<(((><",
        slowdown: 2,
        color: BIG_FISH,
    },
    // kelp: a rounded sea cow, ambling slower than a fish.
    BigVariant {
        right: "=(__)o",
        left: "o(__)=",
        slowdown: 3,
        color: DUGONG,
    },
];

fn big_variant(kind: usize) -> &'static BigVariant {
    &BIG_VARIANTS[kind % BIG_VARIANTS.len()]
}

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let frame = app.frame;
    // The time-of-day palette recolors only the water; every sprite paints on
    // top of it, so the water color threads through every draw below.
    let water = water_color(app.phase);
    let surface = surface_color(app.phase);
    let w = u64::from(area.width);
    let h = u64::from(area.height);

    // Before a rock is placed the tank is an empty sea: waves only. Every draw
    // below keys off the rocks (or off state that is zero pre-placement), so an
    // empty state renders just the water — the wallpaper grammar is unchanged.
    // The anchor (score-gated) and whale (biomass-gated) key off their own
    // gates, not the rocks, so they can appear on an otherwise empty sea.
    fill_water(area, buf, frame, water, surface);
    draw_sediment(app, area, buf, water);
    draw_anchor(app, area, buf, water);
    draw_rocks(app, area, buf, water);
    draw_algae(app, area, buf, frame, water);
    draw_plankton(app, area, buf, frame, w, h, water);
    draw_detritus(app, area, buf, frame, w, h, water);
    draw_fish(app, area, buf, frame, h, water);
    // The whale is drawn last so it crosses in front of every other sprite.
    draw_whale(app, area, buf, frame, water);
}

fn fill_water(area: Rect, buf: &mut Buffer, frame: u64, water: Color, surface: Color) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(Style::new().bg(water));
            }
        }
    }
    // Wave crests drifting along the surface row.
    for x in area.left()..area.right() {
        if (u64::from(x - area.left()) + frame / 2) % 5 < 2 {
            if let Some(cell) = buf.cell_mut((x, area.top())) {
                cell.set_symbol("~");
                cell.set_style(Style::new().fg(surface).bg(water));
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

/// A small block-glyph rock cluster sitting on the floor at column `x`. The
/// body glyphs come from `kind`'s variant so each reef has its own silhouette;
/// `color` is passed in so the live wallpaper draws the real reef color while
/// the placement screen reuses the same shape in a dim preview tone.
pub(crate) fn draw_rock(
    area: Rect,
    buf: &mut Buffer,
    x: u16,
    y: u16,
    kind: usize,
    color: Color,
    water: Color,
) {
    let style = Style::new().fg(color).bg(water);
    let body = rock_variant(kind).body;
    let bx = i64::from(x);
    put(buf, area, bx - 1, y, body[0], style);
    put(buf, area, bx, y, body[1], style);
    put(buf, area, bx + 1, y, body[2], style);
}

/// A single dim floor tick marking a free placement slot.
pub(crate) fn draw_slot_marker(
    area: Rect,
    buf: &mut Buffer,
    x: u16,
    y: u16,
    color: Color,
    water: Color,
) {
    put(
        buf,
        area,
        i64::from(x),
        y,
        "▁",
        Style::new().fg(color).bg(water),
    );
}

fn draw_rocks(app: &App, area: Rect, buf: &mut Buffer, water: Color) {
    let y = area.bottom() - 1;
    for rock in &app.state.rocks {
        let color = rock_variant(rock.kind).color;
        draw_rock(
            area,
            buf,
            slot_center_x(area, rock.slot),
            y,
            rock.kind,
            color,
            water,
        );
    }
}

/// Collected-in-waiting surplus piles up as a mound on the tank floor, centered
/// under the reef (peeking, which collects, visibly clears it). Contiguous so
/// it reads as sediment — scattered cells read as screen noise. The rock is
/// drawn over the mound's center, so a wider mound shows detritus wings around
/// the reef base. Abundance stays readable without a single digit.
fn draw_sediment(app: &App, area: Rect, buf: &mut Buffer, water: Color) {
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
            Style::new().fg(SEDIMENT).bg(water),
        );
    }
}

/// Algae attach to the sides of their host rock (never dead-center, so the rock
/// still reads), each frond in its own in-pane flank column. The glyph and
/// color come from the rock kind's variant table.
fn draw_algae(app: &App, area: Rect, buf: &mut Buffer, frame: u64, water: Color) {
    if app.state.rocks.is_empty() {
        return;
    }
    let n = app.state.rocks.len() as u64;
    let count = u64::from(app.state.population[0])
        .min(MAX_ALGAE)
        .min(u64::from(area.width) / 4);
    let floor = area.bottom() - 1;
    for i in 0..count {
        let rock = &app.state.rocks[(i % n) as usize];
        let ordinal = i / n; // this rock's k-th frond
        let variant = algae_variant(rock.kind);
        let center = slot_center_x(area, rock.slot);
        // One in-pane flank column per frond; injective in the ordinal, so N
        // fronds show as N columns wherever the pane has room. `None` means no
        // usable column left (a pane too narrow to hold another).
        let Some(x) = colony_column(area, i64::from(center), ordinal) else {
            continue;
        };
        // Height and sway are collision-free freedom, so they stay hashed.
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
                Style::new().fg(variant.color).bg(water),
            );
        }
    }
}

/// Plankton gather around the reef, each in its own in-pane column fanned out
/// from a rock and drifting vertically. Distinct columns mean two plankton on a
/// rock never share a cell at any frame, yet the drift still reads as life, not
/// a fixed cluster.
fn draw_plankton(
    app: &App,
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    _w: u64,
    h: u64,
    water: Color,
) {
    const DOTS: [&str; 4] = ["⠁", "⠂", "⠄", "⠈"];
    if app.state.rocks.is_empty() {
        return;
    }
    let n = app.state.rocks.len() as u64;
    let count = u64::from(app.state.population[1]).min(MAX_PLANKTON);
    let lane = (h - 2).max(1);
    for i in 0..count {
        let rock = &app.state.rocks[(i % n) as usize];
        let ordinal = i / n;
        let center = slot_center_x(area, rock.slot);
        // One in-pane column per plankton (fan out from the rock).
        let Some(x) = colony_column(area, i64::from(center), ordinal) else {
            continue;
        };
        // Vertical drift is the collision-free freedom, so it stays hashed.
        let y = area.top() + 1 + ((mix(i, 13) + frame / 3) % lane) as u16;
        put(
            buf,
            area,
            x,
            y,
            DOTS[(i % 4) as usize],
            Style::new().fg(PLANKTON).bg(water),
        );
    }
}

/// Falling specks hint at the detritus rain feeding the floor — raining from
/// the reef's neighbourhood down toward its mound. A placed rock always sheds,
/// so one speck is the baseline (the rain shows the reef is producing); the
/// count grows with the collectable stock. This keeps the founding causality
/// (rock -> settle -> collect) on screen through the pre-purchase minute, when
/// the game layer collects surplus every second and the stock sits near zero.
fn draw_detritus(
    app: &App,
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    _w: u64,
    h: u64,
    water: Color,
) {
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
        put(buf, area, x, y, ".", Style::new().fg(DETRITUS).bg(water));
    }
}

fn draw_fish(app: &App, area: Rect, buf: &mut Buffer, frame: u64, h: u64, water: Color) {
    let rocks = &app.state.rocks;
    if rocks.is_empty() {
        return;
    }
    let n = rocks.len() as u64;
    let small = u64::from(app.state.population[2]).min(MAX_SMALL_FISH);
    let big = u64::from(app.state.population[3]).min(MAX_BIG_FISH);
    // Small fish patrol a tight window low near their reef; big fish sweep a
    // wider window and range higher — each bounded to its assigned rock.
    for i in 0..small {
        let rock = &rocks[(i % n) as usize];
        patrol(
            area,
            buf,
            frame,
            h,
            rock,
            i / n,
            "><>",
            "<><",
            1,
            SMALL_RADIUS,
            FISH,
            true,
            water,
        );
    }
    for i in 0..big {
        let rock = &rocks[(i % n) as usize];
        // The apex individual takes on its host reef's character: a dugong over
        // kelp, a big fish elsewhere. Same length (6 cells) as the fish, so the
        // patrol clamp keeps it fully drawn even in a thin pane.
        let v = big_variant(rock.kind);
        patrol(
            area,
            buf,
            frame,
            h,
            rock,
            i / n,
            v.right,
            v.left,
            v.slowdown,
            BIG_RADIUS,
            v.color,
            false,
            water,
        );
    }
}

/// One fish on a bounded patrol around its host rock: it glides right to the
/// window edge, then back left, staying within the rock center ± the species
/// radius. The window is clamped so the whole glyph fits, so the fish is always
/// drawn fully — never partially clipped. `reef_bias` folds the lane into the
/// lower half so smaller fish keep near the reef. `ordinal` (this rock's k-th
/// fish of its size) sets a distinct lane and a distinct patrol phase, so two
/// fish of a kind on one rock never share a cell.
#[allow(clippy::too_many_arguments)]
fn patrol(
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    h: u64,
    rock: &Rock,
    ordinal: u64,
    right_glyph: &str,
    left_glyph: &str,
    slowdown: u64,
    radius: i64,
    color: Color,
    reef_bias: bool,
    water: Color,
) {
    let len = right_glyph.len() as i64;
    let min_anchor = i64::from(area.left());
    let max_anchor = i64::from(area.right()) - len; // last x where the whole glyph fits
    if max_anchor < min_anchor {
        return; // pane too narrow to show the fish fully
    }
    // Patrol window: rock center ± radius, clamped so the glyph never clips.
    let center = i64::from(slot_center_x(area, rock.slot));
    let left = (center - radius).max(min_anchor);
    let right = (center + radius).min(max_anchor);
    if right < left {
        return; // window falls off the fittable strip
    }
    let travel = right - left;
    let (x, glyph) = if travel == 0 {
        (left, right_glyph)
    } else {
        let period = (2 * travel) as u64;
        // Phase offset per fish keeps same-rock fish out of lockstep, so even
        // two sharing a lane never coincide every frame.
        let t = ((frame / slowdown + ordinal) % period) as i64;
        if t < travel {
            (left + t, right_glyph) // gliding right
        } else {
            (left + 2 * travel - t, left_glyph) // gliding back left
        }
    };

    // Lane by ordinal: distinct rows for same-rock, same-size fish so their
    // glyphs never share a cell. Small fish fold into the lower lanes (near the
    // reef); big fish range over the full height.
    let lane = (h - 2).max(1);
    let row = if reef_bias {
        let lower = (lane / 2).max(1);
        (lane - 1) - (ordinal % lower)
    } else {
        ordinal % lane
    };
    let y = area.top() + 1 + row as u16;
    buf.set_string(x as u16, y, glyph, Style::new().fg(color).bg(water));
}

/// Whether a whale crosses in `window`, and if so its heading (`true` =
/// rightward). Deterministic in the window number — no RNG, no clock — so the
/// sighting cadence is reproducible and snapshot-stable. One hash decides both:
/// the low bits gate the 1-in-K rarity, and bit 2 (independent of that gate)
/// alternates the heading roughly evenly.
fn whale_crossing(window: u64) -> Option<bool> {
    let h = mix(window, WHALE_SALT);
    if !h.is_multiple_of(WHALE_RARITY) {
        return None;
    }
    Some((h >> 2) & 1 == 0)
}

/// A visiting whale gliding across the sea — pure decoration outside the
/// economy. It appears only when total biomass has reached the threshold, and
/// only in the rare windows `whale_crossing` opens; within a window the frame
/// maps to an x position, so the whale enters fully off one edge and exits fully
/// off the other. `put` clamps each cell, so a partly off-screen whale draws
/// just its on-screen slice (unlike patrol, which fits the whole glyph). Reads
/// only the biomass gate — never population, pool, or the save.
fn draw_whale(app: &App, area: Rect, buf: &mut Buffer, frame: u64, water: Color) {
    if area.height < WHALE_MIN_HEIGHT {
        return;
    }
    if app.state.biomass() < app.params.whale_biomass {
        return;
    }
    let Some(rightward) = whale_crossing(frame / WHALE_PERIOD) else {
        return;
    };
    // Columns advanced since the window opened; the whale moves one column per
    // WHALE_SLOWDOWN frames, slower than any fish.
    let step = ((frame % WHALE_PERIOD) / WHALE_SLOWDOWN) as i64;
    let w = i64::from(area.width);
    let (glyph, x0) = if rightward {
        (&WHALE_RIGHT, -WHALE_WIDTH + step) // in from the left, out the right
    } else {
        (&WHALE_LEFT, w - step) // in from the right, out the left
    };
    let style = Style::new().fg(WHALE_COLOR).bg(water);
    let y0 = area.top() + 1; // spout just under the surface waves
    for (r, row) in glyph.iter().enumerate() {
        let y = y0 + r as u16;
        for (col, ch) in row.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            put(buf, area, x0 + col as i64, y, &ch.to_string(), style);
        }
    }
}

/// The sunken-anchor landmark, once the lifetime score has unlocked it. It sits
/// at a fixed floor column, 0.8 of the pane width — a position chosen for the
/// old 5-slot grid, where it fell between the two right-hand slots and cleared
/// every rock body. Under the 9-slot grid that column lands on slot 7's reef:
/// they share a column or two, and — because the anchor draws before the rocks —
/// the reef overdraws it there, so the anchor reads as sitting behind that reef
/// rather than punching through it. This is an accepted degradation until #15
/// makes the anchor position configurable. It hosts no events. Bottom-anchored
/// to the floor, four rows tall. Derived from score alone, so it adds no save
/// field.
fn draw_anchor(app: &App, area: Rect, buf: &mut Buffer, water: Color) {
    // Guard the height before the bottom-anchored row math (`floor + 1 - rows`),
    // which would underflow if a 4-row sprite were placed in a shorter pane.
    if area.height < ANCHOR_MIN_HEIGHT {
        return;
    }
    if app.state.score < app.params.anchor_unlock {
        return;
    }
    let center = (i64::from(area.width) * 4) / 5; // 0.8 * width, on slot 7's reef (9-slot grid)
    let floor = area.bottom() - 1;
    let rows = ANCHOR.len() as u16;
    for (r, cells) in ANCHOR.iter().enumerate() {
        // Bottom row on the floor, earlier rows stacked above it.
        let y = floor + r as u16 + 1 - rows;
        for (col, cell) in cells.iter().enumerate() {
            let Some(cell) = cell else { continue };
            let bg = if cell.iron_bg {
                Color::Indexed(ANCHOR_IRON)
            } else {
                water
            };
            let style = Style::new().fg(Color::Indexed(cell.fg)).bg(bg);
            // Cell column 2 sits on the center, so the 5-wide anchor is centered.
            put(buf, area, center - 2 + col as i64, y, cell.sym, style);
        }
    }
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

/// Column for the `k`-th member of a rock's colony. Candidate columns fan out
/// from the rock (nearest first, right then left: +2, -2, +3, -3, …), and only
/// those on-screen and clear of the rock body (center ±1) are kept; the k-th
/// kept column is the answer. Skipping off-pane candidates for in-pane ones is
/// what lets N members show as N columns even in a thin edge pane. Injective in
/// `k` within a rock. `None` when the pane has fewer than `k+1` usable columns.
fn colony_column(area: Rect, center: i64, k: u64) -> Option<i64> {
    let lo = i64::from(area.left());
    let hi = i64::from(area.right()); // exclusive
    let mut remaining = k;
    // Any usable column is within `area.width` of the center, so that bounds the
    // fan. Magnitude starts at 2, which is what excludes the rock body.
    for mag in 2..=i64::from(area.width) {
        for x in [center + mag, center - mag] {
            if (lo..hi).contains(&x) {
                if remaining == 0 {
                    return Some(x);
                }
                remaining -= 1;
            }
        }
    }
    None
}

/// Deterministic position hash: the whole animation is a pure function of
/// (state, frame), never of wall clock or an RNG stream.
fn mix(i: u64, salt: u64) -> u64 {
    (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93))
        .rotate_left(31)
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window gate opens on roughly 1 in K windows — the rarity that makes a
    /// whale a lucky sight. Scanning a large window range, the crossing count
    /// stays inside a band around the design fraction (1/WHALE_RARITY).
    #[test]
    fn whale_crossings_are_roughly_one_in_k_windows() {
        const WINDOWS: u64 = 10_000;
        let crossings = (0..WINDOWS)
            .filter(|&w| whale_crossing(w).is_some())
            .count() as u64;
        let expected = WINDOWS / WHALE_RARITY;
        // A good hash lands near expected; allow a generous +-20% band so the
        // test asserts "rare, not never / not always" without being brittle.
        assert!(
            crossings > expected * 4 / 5 && crossings < expected * 6 / 5,
            "crossings {crossings} outside the design band around {expected} (1/{WHALE_RARITY})"
        );
    }

    /// The heading alternates rather than sticking one way: among the windows
    /// that host a crossing, both directions occur in comparable numbers. (A
    /// naive low-bit heading would collide with the rarity gate and never turn.)
    #[test]
    fn whale_heading_is_not_stuck_one_way() {
        const WINDOWS: u64 = 10_000;
        let (mut right, mut left) = (0u64, 0u64);
        for w in 0..WINDOWS {
            match whale_crossing(w) {
                Some(true) => right += 1,
                Some(false) => left += 1,
                None => {}
            }
        }
        assert!(right > 0 && left > 0, "both headings must occur");
        // Neither heading dominates the crossings (each within 40-60%).
        let total = right + left;
        assert!(
            right * 5 > total * 2 && left * 5 > total * 2,
            "headings should be balanced, got right {right} / left {left}"
        );
    }

    /// Sprite caps must never clip a bought individual (feedback (a)): for every
    /// reef the default params can compose — any multiset of kinds within the max
    /// budget and the slot count — each species' total housing stays within its
    /// on-screen cap. Enumerating all such reefs keeps the bound honest: widen a
    /// budget step or a capacity past a cap and this goes red at the cap to raise
    /// (which is exactly how #16's budget-5 step surfaced the old caps).
    #[test]
    fn sprite_caps_cover_every_reachable_population() {
        use crate::engine::Params;

        let p = Params::default();
        let max_budget = p
            .budget_steps
            .iter()
            .map(|&(_, b)| b)
            .max()
            .expect("a budget schedule");
        let species = p.rock_kinds[0].capacity.len();

        // Depth-first over multisets of kinds (a non-decreasing start index counts
        // each composition once), bounded by remaining budget and slots; track the
        // largest per-species housing any reachable reef reaches.
        fn walk(p: &Params, start: usize, budget: u32, slots: u32, caps: &[u32], best: &mut [u32]) {
            for (b, &c) in best.iter_mut().zip(caps.iter()) {
                *b = (*b).max(c);
            }
            if slots == 0 {
                return;
            }
            for kind in start..p.rock_kinds.len() {
                let rk = &p.rock_kinds[kind];
                if rk.cost <= budget {
                    let next: Vec<u32> = caps
                        .iter()
                        .zip(rk.capacity.iter())
                        .map(|(a, c)| a + c)
                        .collect();
                    walk(p, kind, budget - rk.cost, slots - 1, &next, best);
                }
            }
        }

        let zero = vec![0u32; species];
        let mut best = zero.clone();
        walk(&p, 0, max_budget, u32::from(SLOTS), &zero, &mut best);

        let caps = [MAX_ALGAE, MAX_PLANKTON, MAX_SMALL_FISH, MAX_BIG_FISH];
        for (sp, (&reach, &cap)) in best.iter().zip(caps.iter()).enumerate() {
            assert!(
                u64::from(reach) <= cap,
                "species {sp}: reachable population {reach} exceeds sprite cap {cap}"
            );
        }
    }
}
