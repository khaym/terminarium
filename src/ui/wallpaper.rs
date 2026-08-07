//! Wallpaper layer: the tank as pure decoration — no numbers, no bars, no
//! UI. Everything derives from engine state + frame number through a seeded
//! hash, so a given (state, frame) always renders the same picture (which is
//! what makes snapshot regression possible).
//!
//! The placed reef is the scene's center of mass: sediment mounds under it,
//! algae attach to its sides, detritus rains from its neighbourhood, and the
//! swimming life patrols a bounded window around it (one window per reef).
//! So the reef -> settle -> collect causality, and where the reef sits, read
//! from position alone, without a word of text.
//!
//! Sprite positions are assigned from an individual's ordinal within its host
//! reef, not drawn from an independent hash: distinct ordinals map to distinct
//! in-pane columns (algae, plankton) or lanes/phases (fish), so every bought
//! individual shows wherever the pane has room, and no two of a kind within a
//! reef's colony share a cell. (Colonies on different reefs can still cross —
//! the first run places a single reef; multi-reef spacing is a later concern.)
//! The `mix()` hash is left only for freedoms that cannot cause overlap (frond
//! height and sway, plankton drift, patrol phase, and the quirk windows that
//! break up a swimmer's glide — those move it only along its own lane).
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
//! decoration outside the economy: gated on the living-population biomass, it
//! crosses in only 1-in-K deterministic frame windows so a sighting stays a
//! rare, reproducible treat. The sunken-anchor scenery (`draw_anchor`) is a
//! score-unlocked landmark the player slides along the floor (App's anchor-move
//! mode), so its position is a persisted field (#15) — but it stays outside the
//! economy: the renderer only reads it, `tick` never does. The whale reads only
//! its gate quantity (living biomass). Neither visitor is a simulation input.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::{App, Phase};
use crate::content::{
    self,
    creatures::{Dash, Drift, SwimmerDef},
};
use crate::engine::{Reef, MICRO, SLOTS};

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

/// Colors of the scenery that is not alive. Every creature takes its color from
/// its own definition in `crate::content::creatures`, reached through its host
/// reef's kind, as does the rock body.
const DETRITUS: Color = Color::Indexed(101);
const SEDIMENT: Color = Color::Indexed(94);

/// Collectable surplus represented by one cell of floor sediment.
const SEDIMENT_PER_CELL: u128 = 30 * MICRO;

/// On-screen sprite caps. A bought individual always shows (feedback (a)), so
/// each cap sits at or above the largest population the default params can field
/// — any reef within the max budget and the slot count. They bite only on an
/// out-of-range state (e.g. a tampered save), never in real play. The
/// `sprite_caps_cover_every_reachable_population` test pins them to the params.
const MAX_ALGAE: u64 = 20;
const MAX_PLANKTON: u64 = 15;
const MAX_SMALL_FISH: u64 = 14;
const MAX_BIG_FISH: u64 = 7;

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
/// whale a lucky sight rather than a fixture. At WHALE_PERIOD/~5fps per window,
/// 1-in-32 averages roughly one sighting every 1.7 hours.
const WHALE_RARITY: u64 = 32;
/// Frames per column step; higher is slower. The whale ambles slower than the
/// big fish (slowdown 2) and the dugong (3).
const WHALE_SLOWDOWN: u64 = 4;
/// Hash salt for the window gate. One hash decides both rarity (its low bits)
/// and heading (its top bit — disjoint from the rarity mask for any rarity, so
/// widening the rarity never freezes the heading), keeping the cadence a pure
/// function of the window number.
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
/// The tone the whole anchor is relit in while the player is moving it — the
/// grabbed gold game.rs uses for a grabbed reef, so "picked up, shape kept"
/// reads the same across both.
const ANCHOR_GRABBED: u8 = 222;

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

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    // Every animation clock is anchored to launch time: the epoch (wall-clock
    // frames at startup) plus the in-session frame counter gives one absolute
    // frame the whole scene rides. So waves, fish, and the whale's rare windows
    // all advance with real time across restarts instead of replaying from zero
    // each launch. A bare `App` has epoch 0, so tests and snapshots stay fixed.
    let frame = app.frame_epoch.wrapping_add(app.frame);
    // The time-of-day palette recolors only the water; every sprite paints on
    // top of it, so the water color threads through every draw below.
    let water = water_color(app.phase);
    let surface = surface_color(app.phase);
    let w = u64::from(area.width);
    let h = u64::from(area.height);

    // Before a reef is placed the tank is an empty sea: waves only. Every draw
    // below keys off the reefs (or off state that is zero pre-placement), so an
    // empty state renders just the water — the wallpaper grammar is unchanged.
    // The anchor (score-gated) and whale (biomass-gated) key off their own
    // gates, not the reefs, so they can appear on an otherwise empty sea.
    fill_water(area, buf, frame, water, surface);
    draw_sediment(app, area, buf, water);
    draw_anchor(app, area, buf, water);
    draw_rocks(app, area, buf, water);
    draw_algae(app, area, buf, frame, water);
    draw_plankton(app, area, buf, frame, w, h, water);
    draw_detritus(app, area, buf, frame, w, h, water);
    draw_fish(app, area, buf, frame, water);
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
/// reef will.
pub(crate) fn slot_center_x(area: Rect, slot: u8) -> u16 {
    let w = u64::from(area.width);
    let cx = ((u64::from(slot) * 2 + 1) * w) / (2 * u64::from(SLOTS));
    area.left() + cx as u16
}

/// Mean floor column of the placed reefs (a single reef's own column in the
/// first run). Used to anchor the sediment mound directly under the reef.
fn reef_centroid_x(app: &App, area: Rect) -> u16 {
    let reefs = &app.state.reefs;
    let sum: u64 = reefs
        .iter()
        .map(|r| u64::from(slot_center_x(area, r.slot)))
        .sum();
    (sum / reefs.len() as u64) as u16
}

/// A small block-glyph rock cluster sitting on the floor at column `x`. The
/// body glyphs come from `kind`'s definition so each reef has its own
/// silhouette; `color` is passed in so the live wallpaper draws the real reef
/// color while the placement screen reuses the same shape in a dim preview tone.
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
    let body = content::def(kind).rock.body;
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
    for reef in &app.state.reefs {
        let color = content::def(reef.kind).rock.color;
        draw_rock(
            area,
            buf,
            slot_center_x(area, reef.slot),
            y,
            reef.kind,
            color,
            water,
        );
    }
}

/// Collected-in-waiting surplus piles up as a mound on the tank floor, centered
/// under the reef (peeking, which collects, visibly clears it). Contiguous so
/// it reads as sediment — scattered cells read as screen noise. The rock body
/// is drawn over the mound's center, so a wider mound shows detritus wings around
/// the reef base. Abundance stays readable without a single digit.
fn draw_sediment(app: &App, area: Rect, buf: &mut Buffer, water: Color) {
    if app.state.reefs.is_empty() {
        return;
    }
    let max = u128::from(area.width.saturating_sub(2));
    let cells = (app.state.collectable / SEDIMENT_PER_CELL).min(max) as u16;
    if cells == 0 {
        return;
    }
    let y = area.bottom() - 1;
    let center = reef_centroid_x(app, area);
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

/// Algae attach to the sides of their host reef (never dead-center, so the rock
/// body still reads), each frond in its own in-pane flank column. Which frond grows
/// there comes from the creature the host kind houses at the base tier.
fn draw_algae(app: &App, area: Rect, buf: &mut Buffer, frame: u64, water: Color) {
    if app.state.reefs.is_empty() {
        return;
    }
    let n = app.state.reefs.len() as u64;
    let count = u64::from(app.state.population[0])
        .min(MAX_ALGAE)
        .min(u64::from(area.width) / 4);
    let floor = area.bottom() - 1;
    for i in 0..count {
        let reef = &app.state.reefs[(i % n) as usize];
        let ordinal = i / n; // this reef's k-th frond
        let algae = content::def(reef.kind).algae;
        let center = slot_center_x(area, reef.slot);
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
                    algae.fronds[0]
                } else {
                    algae.fronds[1]
                },
                Style::new().fg(algae.color).bg(water),
            );
        }
    }
}

/// Plankton gather around the reef, each in its own in-pane column fanned out
/// from a reef and drifting vertically. Distinct columns mean two plankton on a
/// reef never share a cell at any frame, yet the drift still reads as life, not
/// a fixed cluster. Which drifter it is comes from the host kind's definition.
fn draw_plankton(
    app: &App,
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    _w: u64,
    h: u64,
    water: Color,
) {
    if app.state.reefs.is_empty() {
        return;
    }
    let n = app.state.reefs.len() as u64;
    let count = u64::from(app.state.population[1]).min(MAX_PLANKTON);
    let lane = (h - 2).max(1);
    for i in 0..count {
        let reef = &app.state.reefs[(i % n) as usize];
        let ordinal = i / n;
        let plankton = content::def(reef.kind).plankton;
        let center = slot_center_x(area, reef.slot);
        // One in-pane column per plankton (fan out from the reef).
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
            plankton.dots[(i % plankton.dots.len() as u64) as usize],
            Style::new().fg(plankton.color).bg(water),
        );
    }
}

/// Falling specks hint at the detritus rain feeding the floor — raining from
/// the reef's neighbourhood down toward its mound. A placed reef always sheds,
/// so one speck is the baseline (the rain shows the reef is producing); the
/// count grows with the collectable stock. This keeps the founding causality
/// (reef -> settle -> collect) on screen through the pre-purchase minute, when
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
    if app.state.reefs.is_empty() {
        return;
    }
    let count = (1 + app.state.collectable / (15 * MICRO)).min(5) as u64;
    let lane = (h - 2).max(1);
    for i in 0..count {
        let reef = &app.state.reefs[(i as usize) % app.state.reefs.len()];
        let center = slot_center_x(area, reef.slot);
        let offset = (mix(i, 23) % 7) as i64 - 3; // fall near the reef
        let x = i64::from(center) + offset;
        let y = area.top() + 1 + ((mix(i, 29) + frame / 2) % lane) as u16;
        put(buf, area, x, y, ".", Style::new().fg(DETRITUS).bg(water));
    }
}

fn draw_fish(app: &App, area: Rect, buf: &mut Buffer, frame: u64, water: Color) {
    let reefs = &app.state.reefs;
    if reefs.is_empty() {
        return;
    }
    let n = reefs.len() as u64;
    let small = u64::from(app.state.population[2]).min(MAX_SMALL_FISH);
    let big = u64::from(app.state.population[3]).min(MAX_BIG_FISH);
    // Which swimmer each tier is comes from the host kind's definition, and the
    // definition carries its own beat: small fish patrol a tight window low near
    // their reef, an apex individual sweeps a wider window and ranges higher.
    // Each stays bounded to its assigned reef.
    for i in 0..small {
        let reef = &reefs[(i % n) as usize];
        patrol(
            area,
            buf,
            frame,
            reef,
            i / n,
            content::def(reef.kind).small,
            water,
        );
    }
    for i in 0..big {
        let reef = &reefs[(i % n) as usize];
        patrol(
            area,
            buf,
            frame,
            reef,
            i / n,
            content::def(reef.kind).big,
            water,
        );
    }
}

/// The vertical sway a `Drift` adds to a swimmer's lane at `frame`: a triangle
/// wave in whole rows, rising to `+amplitude` and falling to `-amplitude` once
/// per period. Whole integers keep it as reproducible as every other sprite
/// position.
///
/// It reads the frame and nothing else — deliberately. A per-individual phase
/// would let one drifter sink onto the lane of the one below it, and distinct
/// lanes per ordinal are exactly what keeps a colony's sprites off each other
/// (see the module note on hashed freedoms). Drifting as one bloom keeps that
/// guarantee, and matches the pulse, which is also a beat in time alone.
fn drift_offset(drift: &Drift, frame: u64) -> i64 {
    if drift.amplitude == 0 {
        return 0;
    }
    let period = drift.period.max(1);
    let span = 2 * drift.amplitude; // rows between the two extremes
    let t = (frame % period) as i64;
    // Two half-cycles: up over the first half of the period, back down over the
    // second. Integer division alone, so the wave never leaves a whole row.
    let walk = (2 * t * span) / period as i64;
    let rise = if walk <= span { walk } else { 2 * span - walk };
    rise - drift.amplitude
}

/// One swimmer's identity, folded from everything that tells it apart from the
/// rest of the tank: its host reef's kind and slot, and its ordinal within that
/// reef's tier. The quirk gates hash this together with the lap number, which is
/// what makes a quirk one fish's own rather than the whole shoal's.
fn swimmer_id(reef: &Reef, ordinal: u64) -> u64 {
    mix(reef.kind as u64, mix(u64::from(reef.slot), ordinal))
}

/// Hash salts for the quirk windows, one per motion — so a swimmer wearing both
/// does not dash and turn back on the same laps for want of a second hash.
const DASH_SALT: u64 = 71;
const EARLY_TURN_SALT: u64 = 89;

/// The one hash behind a quirk's window on `lap` for the swimmer `id`. As with
/// the whale's crossing hash, one roll carries two things in disjoint fields:
/// its low half gates the rarity (`quirk_open`), its high half carries the
/// freedom left inside an open window (where in the lap a burst falls).
///
/// The fold is what makes the low half usable. `mix` ends in a multiply, so its
/// low bits carry only the low bits of what went in: gating on them straight
/// gave 36 swimmers just 8 distinct sets of laps, on a visibly regular beat
/// (measured). Folding the high half — where every input bit has reached — down
/// over the low one gave all 36 their own.
fn quirk_roll(id: u64, lap: u64, salt: u64) -> u64 {
    let h = mix(lap, mix(id, salt));
    h ^ (h >> 32)
}

/// Whether a quirk's window is open: one lap in `rarity`, and never every lap by
/// accident (a 0 rarity would open them all, so it is read as 1).
fn quirk_open(roll: u64, rarity: u64) -> bool {
    roll.is_multiple_of(rarity.max(1))
}

/// How a dash paces the lap it falls in: the swimmer takes two columns a step
/// over the burst, then hands that ground back a column at a time over the rest
/// of the lap. The result is monotone in `step` and pinned at both ends of the
/// lap — a dashing lap opens and closes exactly where a plain one would.
///
/// Both halves are load-bearing. The burst is what the eye catches; the handing
/// back is what keeps the draw a pure function of (state, frame), since a lap
/// that left the swimmer permanently ahead could only be placed by summing every
/// window since launch. `freedom` is the high half of the window's hash, which
/// picks where in the lap the burst falls so it is not forever the same stretch
/// of the same leg.
fn dash_warp(dash: &Dash, freedom: u64, step: i64, lap_steps: i64) -> i64 {
    // The burst has to fit the lap twice over: once to run, once to repay.
    let span = dash.span.min((lap_steps / 2) as u64) as i64;
    if span == 0 {
        return step;
    }
    let start = (freedom % (lap_steps - 2 * span + 1) as u64) as i64;
    let gained = (step - start).clamp(0, span);
    // At least `span` steps are left to repay over, so the swimmer never gives
    // back more than the column it would have swum anyway — it slows, it never
    // slides backwards.
    let repaid_over = lap_steps - start - span;
    let repaid = span * (step - start - span).max(0) / repaid_over;
    step + gained - repaid
}

/// How far along its patrol window a swimmer sits at `frame`, in columns from
/// the window's left edge (`0..=travel`), and whether it faces right.
///
/// The beat is a lap: out to the far edge of the window over the first half and
/// home over the second, `2 * travel` steps in all, one step per `slowdown`
/// frames. `ordinal` offsets the phase so same-reef swimmers are never in
/// lockstep.
///
/// A lap is also the unit a quirk lives in. Each of `Manner`'s quirks hashes
/// (`id`, lap) into its own window: on an open lap the dash re-paces the swimmer
/// and the early turn shortens how far out it goes, and on every other lap the
/// beat is the plain triangle every fish has always swum. Since both leave the
/// lap's two ends alone, a window can open or close between laps without moving
/// the swimmer a cell.
fn patrol_offset(
    swimmer: &SwimmerDef,
    id: u64,
    ordinal: u64,
    travel: i64,
    frame: u64,
) -> (i64, bool) {
    if travel == 0 {
        return (0, true); // a window one column wide: nowhere to swim
    }
    let lap_steps = 2 * travel;
    let steps = frame / swimmer.slowdown + ordinal;
    let lap = steps / lap_steps as u64;
    let mut step = (steps % lap_steps as u64) as i64;
    if let Some(dash) = &swimmer.manner.dash {
        let roll = quirk_roll(id, lap, DASH_SALT);
        if quirk_open(roll, dash.rarity) {
            step = dash_warp(dash, roll >> 32, step, lap_steps);
        }
    }
    // How far out this lap goes. The window itself does not move: a short lap
    // is the same window less fully swum, and never less than half of it, so a
    // pane thin enough to squeeze the window cannot pin the swimmer to a column.
    let reach = match &swimmer.manner.early_turn {
        Some(turn) if quirk_open(quirk_roll(id, lap, EARLY_TURN_SALT), turn.rarity) => {
            (travel - turn.short).max((travel + 1) / 2)
        }
        _ => travel,
    };
    let (along, rightward) = if step < travel {
        (step, true) // gliding out
    } else {
        (lap_steps - step, false) // gliding home
    };
    (along * reach / travel, rightward)
}

/// One swimmer on a bounded patrol around its host reef: it glides right to the
/// window edge, then back left, staying within the reef center ± the creature's
/// radius. The window is clamped so the whole sprite fits, so the swimmer is
/// always drawn fully — never partially clipped. The creature's `reef_bias` folds
/// the lane into the lower half so smaller tenants keep near the reef.
/// `ordinal` (this reef's k-th swimmer of its tier) sets a distinct lane and a
/// distinct patrol phase, so two of a tier on one reef never share a cell.
///
/// The swimmer's `Manner` rides on top of that one glide: a `pulse` picks which
/// appearance the frame wears, an `under` row hangs a second row below the body,
/// a `drift` sways the whole sprite over its lane, and the quirks (`patrol_offset`)
/// break up the glide itself now and then. All of them are read from the
/// definition, so a new drifter is content — this routine names no creature.
#[allow(clippy::too_many_arguments)]
fn patrol(
    area: Rect,
    buf: &mut Buffer,
    frame: u64,
    reef: &Reef,
    ordinal: u64,
    swimmer: &SwimmerDef,
    water: Color,
) {
    // Both appearances are measured, so the window and the lane hold still while
    // the swimmer pulses. Cells, not bytes: a braille body is three cells and
    // nine bytes, and the pane is counted in the former.
    let (span, rows) = swimmer.footprint();
    let len = span as i64;
    let min_anchor = i64::from(area.left());
    let max_anchor = i64::from(area.right()) - len; // last x where the whole glyph fits
    if max_anchor < min_anchor {
        return; // pane too narrow to show the fish fully
    }
    // Patrol window: reef center ± radius, clamped so the glyph never clips.
    let center = i64::from(slot_center_x(area, reef.slot));
    let left = (center - swimmer.radius).max(min_anchor);
    let right = (center + swimmer.radius).min(max_anchor);
    if right < left {
        return; // window falls off the fittable strip
    }
    let travel = right - left;
    // Which appearance this frame wears — the pulse is a beat in time, so it
    // turns over independently of the heading picked just below.
    let look = swimmer.look(frame);
    let id = swimmer_id(reef, ordinal);
    let (along, rightward) = patrol_offset(swimmer, id, ordinal, travel, frame);
    let x = left + along;
    let glyph = if rightward { look.right } else { look.left };

    // The rows a sprite may start on: under the surface waves, high enough that
    // its last row clears the floor, and — where the pane has room — inset far
    // enough that the drift's whole swing stays on screen. Lanes are handed out
    // inside that band rather than clamped into it afterwards, so two drifters
    // can never be squeezed onto one row. For a one-row swimmer that does not
    // drift the band is exactly the one every fish has always had.
    let top = i64::from(area.top()) + 1;
    let bed = i64::from(area.bottom()) - 1 - i64::from(rows);
    if bed < top {
        return; // pane too short to hold the whole sprite
    }
    let amplitude = swimmer.manner.drift.amplitude;
    let (lo, hi) = if bed - top >= 2 * amplitude {
        (top + amplitude, bed - amplitude)
    } else {
        (top, bed) // too short for the swing: the drift flattens, nothing clips
    };
    // Lanes are a sprite tall, so the band holds as many as fit whole: a
    // two-row swimmer takes two rows of it, and the one below never hangs its
    // second row over the one above.
    let lanes = ((hi - lo + 1) / i64::from(rows)).max(1);

    // Lane by ordinal: distinct rows for same-reef, same-size fish so their
    // glyphs never share a cell. Small fish fold into the lower lanes (near the
    // reef); big fish range over the whole band.
    let lane = if swimmer.reef_bias {
        hi - (ordinal % (lanes / 2).max(1) as u64) as i64 * i64::from(rows)
    } else {
        lo + (ordinal % lanes as u64) as i64 * i64::from(rows)
    };
    let y = (lane + drift_offset(&swimmer.manner.drift, frame)).clamp(top, bed) as u16;

    // The accent (the anglerfish's lure) sits at a fixed cell of the body row; a
    // left-facing draw mirrors that index (`cells - 1 - index`), which only
    // lands correctly because `right` and `left` are the same length in cells
    // (pinned for every kind by content::mod::tests). An out-of-range index
    // degrades to no accent — an internal-origin fault, not a player one —
    // rather than panicking. The under row wears the body color throughout: a
    // trailing row is one shape, not a place to hide a second signal.
    let cells = glyph.chars().count();
    let accent = swimmer
        .accent
        .filter(|&(index, _)| index < cells)
        .map(|(index, color)| {
            let mirrored = if rightward { index } else { cells - 1 - index };
            (mirrored, color)
        });
    let paint = RowPaint {
        body: swimmer.color,
        accent,
        water,
    };
    draw_sprite_row(buf, area, x, y, glyph, &paint);
    if !look.under.is_empty() {
        let plain = RowPaint {
            accent: None,
            ..paint
        };
        draw_sprite_row(buf, area, x, y + 1, look.under, &plain);
    }
}

/// How a row of a swimmer's sprite is painted: its body color, the one cell (if
/// any) that wears the accent instead, and the water behind both.
#[derive(Clone, Copy)]
struct RowPaint {
    body: Color,
    accent: Option<(usize, Color)>,
    water: Color,
}

/// One row of a swimmer's sprite, glyph by glyph from `x` — so a character's
/// index in the row is its column offset — with one cell allowed to wear the
/// accent color.
fn draw_sprite_row(buf: &mut Buffer, area: Rect, x: i64, y: u16, glyphs: &str, paint: &RowPaint) {
    for (i, ch) in glyphs.chars().enumerate() {
        let fg = match paint.accent {
            Some((accent_at, accent_color)) if i == accent_at => accent_color,
            _ => paint.body,
        };
        let mut char_buf = [0u8; 4];
        put(
            buf,
            area,
            x + i as i64,
            y,
            ch.encode_utf8(&mut char_buf),
            Style::new().fg(fg).bg(paint.water),
        );
    }
}

/// Whether a whale crosses in `window`, and if so its heading (`true` =
/// rightward). Deterministic in the window number — no RNG, no clock — so the
/// sighting cadence is reproducible and snapshot-stable. One hash decides both:
/// its low bits gate the 1-in-K rarity, and its top bit — which no rarity mask
/// reaches — alternates the heading roughly evenly (a low heading bit would fall
/// inside the mask once the rarity grew and freeze the whale to one direction).
fn whale_crossing(window: u64) -> Option<bool> {
    let h = mix(window, WHALE_SALT);
    if !h.is_multiple_of(WHALE_RARITY) {
        return None;
    }
    Some(h >> 63 == 0)
}

/// A visiting whale gliding across the sea — pure decoration outside the
/// economy. It appears only when the living-population biomass has reached the
/// threshold, and only in the rare windows `whale_crossing` opens; within a
/// window the frame maps to an x position, so the whale enters fully off one
/// edge and exits fully off the other. `put` clamps each cell, so a partly
/// off-screen whale draws just its on-screen slice (unlike patrol, which fits
/// the whole glyph). Reads only the living-biomass gate — a sea kept alive earns
/// it, uncollected sediment does not; never touches population count or the save.
fn draw_whale(app: &App, area: Rect, buf: &mut Buffer, frame: u64, water: Color) {
    if area.height < WHALE_MIN_HEIGHT {
        return;
    }
    if app.state.living_biomass() < app.params.whale_biomass {
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

/// The sunken-anchor landmark, once the lifetime score has unlocked it. Its
/// floor column follows the player-chosen position (`state.anchor_pos`, a
/// width-independent millipermille); the millipermille → column conversion
/// lives here, since absolute coordinates are the renderer's. The default 800‰
/// reproduces the old fixed 0.8-width column. While the player is moving it
/// (`app.anchor_mode`) the whole glyph is relit in the grabbed tone, the same
/// "picked up, shape kept" cue game.rs uses for a grabbed reef. The anchor draws
/// before the reefs, so a reef sharing its column overdraws it and it reads as
/// sitting behind that reef — moving it aside is exactly what the mode is for.
/// It hosts no events. Bottom-anchored to the floor, four rows tall.
fn draw_anchor(app: &App, area: Rect, buf: &mut Buffer, water: Color) {
    // Guard the height before the bottom-anchored row math (`floor + 1 - rows`),
    // which would underflow if a 4-row sprite were placed in a shorter pane.
    if area.height < ANCHOR_MIN_HEIGHT {
        return;
    }
    if app.state.score < app.params.anchor_unlock {
        return;
    }
    let center =
        i64::from(area.left()) + (i64::from(area.width) * i64::from(app.state.anchor_pos)) / 1000;
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
            // Relit to the grabbed tone while being moved; otherwise its own
            // iron/highlight/rust palette. Only the foreground changes, so the
            // silhouette is unchanged.
            let fg = if app.anchor_mode {
                Color::Indexed(ANCHOR_GRABBED)
            } else {
                Color::Indexed(cell.fg)
            };
            let style = Style::new().fg(fg).bg(bg);
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

/// Column for the `k`-th member of a reef's colony. Candidate columns fan out
/// from the reef (nearest first, right then left: +2, -2, +3, -3, …), and only
/// those on-screen and clear of the rock body (center ±1) are kept; the k-th
/// kept column is the answer. Skipping off-pane candidates for in-pane ones is
/// what lets N members show as N columns even in a thin edge pane. Injective in
/// `k` within a reef. `None` when the pane has fewer than `k+1` usable columns.
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
    use crate::content::creatures::{Dash, Drift, EarlyTurn, Look, Manner, Pulse};

    /// A pane to patrol one swimmer in, and the reef it patrols around.
    fn pane(width: u16, height: u16) -> (Rect, Reef) {
        (
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            Reef { kind: 0, slot: 4 },
        )
    }

    /// The rows one swimmer draws at `frame`, as `(row, glyphs)` top-first —
    /// every cell painted in the swimmer's own color, which on an empty buffer
    /// is the swimmer and nothing else.
    fn sprite_rows(swimmer: &SwimmerDef, area: Rect, frame: u64) -> Vec<(u16, String)> {
        let (_, reef) = pane(area.width, area.height);
        let mut buf = Buffer::empty(area);
        patrol(area, &mut buf, frame, &reef, 0, swimmer, Color::Indexed(17));
        let mut rows: Vec<(u16, String)> = Vec::new();
        for y in area.top()..area.bottom() {
            let mut glyphs = String::new();
            for x in area.left()..area.right() {
                let cell = buf.cell((x, y)).expect("cell");
                if cell.style().fg == Some(swimmer.color) {
                    glyphs.push_str(cell.symbol());
                }
            }
            if !glyphs.is_empty() {
                rows.push((y, glyphs));
            }
        }
        rows
    }

    /// A drifter a test composes itself: the same three modifiers the jellyfish
    /// wears, at whatever amplitude, period and glyphs the test names. Its
    /// point is that a second drifter is data — a definition file could say
    /// exactly this and need no renderer change (the tests/render.rs habit of
    /// building a manifest of one's own, applied to a creature).
    fn drifter(amplitude: i64, period: u64, pulse_period: u64) -> SwimmerDef {
        SwimmerDef {
            right: "(o)",
            left: "(o)",
            slowdown: 4,
            radius: 4,
            reef_bias: true,
            color: Color::Indexed(200),
            accent: None,
            manner: Manner {
                drift: Drift { amplitude, period },
                under: "\\|/",
                pulse: Some(Pulse {
                    look: Look {
                        right: "[o]",
                        left: "[o]",
                        under: "|",
                    },
                    period: pulse_period,
                }),
                ..Manner::PLAIN
            },
        }
    }

    /// The drift is a triangle wave in whole rows: over one period it rises to
    /// `+amplitude`, falls to `-amplitude`, and comes back — never further, and
    /// never anywhere but on a row. `Drift::STILL` holds its lane forever, which
    /// is what keeps every fish's picture the one it always had.
    #[test]
    fn drift_sweeps_its_amplitude_and_returns() {
        let drift = Drift {
            amplitude: 2,
            period: 40,
        };
        let offsets: Vec<i64> = (0..40).map(|f| drift_offset(&drift, f)).collect();
        assert_eq!(
            offsets.iter().copied().min(),
            Some(-2),
            "the drift must reach its lower amplitude: {offsets:?}"
        );
        assert_eq!(
            offsets.iter().copied().max(),
            Some(2),
            "the drift must reach its upper amplitude: {offsets:?}"
        );
        // A wave, not a sawtooth and not a jitter: one rise and one fall per
        // period — the offsets climb to the peak and fall away from it, never
        // turning in between — and every step that moves moves a single row.
        let peak = offsets.iter().position(|&v| v == 2).expect("a peak");
        assert!(
            offsets[..=peak].windows(2).all(|w| w[1] >= w[0]),
            "the drift rises to its peak without turning: {offsets:?}"
        );
        assert!(
            offsets[peak..].windows(2).all(|w| w[1] <= w[0]),
            "and falls away from it without turning: {offsets:?}"
        );
        assert!(
            offsets.windows(2).all(|w| (w[1] - w[0]).abs() <= 1),
            "the drift moves a row at a time: {offsets:?}"
        );
        assert_eq!(
            offsets,
            (40..80)
                .map(|f| drift_offset(&drift, f))
                .collect::<Vec<_>>(),
            "the wave repeats every period"
        );

        for frame in 0..100 {
            assert_eq!(
                drift_offset(&Drift::STILL, frame),
                0,
                "a still swimmer never leaves its lane"
            );
        }
    }

    /// The three modifiers are data, not the jellyfish's own code: a second
    /// drifter composed here — its own glyphs, its own amplitude, its own two
    /// beats — draws two rows, pulses between its two appearances on its own
    /// period, and swings exactly its own amplitude. Change a number in the
    /// definition and the picture follows it; nothing in the renderer names a
    /// creature.
    #[test]
    fn a_second_drifter_is_data_alone() {
        let (area, _) = pane(40, 20);
        let gentle = drifter(1, 20, 2);
        let wide = drifter(3, 24, 5);

        for (swimmer, amplitude) in [(&gentle, 1), (&wide, 3)] {
            let tops: Vec<u16> = (0..48)
                .map(|f| sprite_rows(swimmer, area, f)[0].0)
                .collect();
            let (lo, hi) = (
                tops.iter().copied().min().expect("a row"),
                tops.iter().copied().max().expect("a row"),
            );
            assert_eq!(
                i64::from(hi - lo),
                2 * amplitude,
                "amplitude {amplitude} must swing {} rows, got {lo}..{hi}",
                2 * amplitude
            );

            // Two rows, adjacent, in every frame — the under row hangs directly
            // under the body row and neither is ever dropped.
            for frame in 0..48 {
                let rows = sprite_rows(swimmer, area, frame);
                assert_eq!(rows.len(), 2, "frame {frame}: a two-row sprite draws two");
                assert_eq!(rows[1].0, rows[0].0 + 1, "frame {frame}: rows are adjacent");
            }
        }

        // Each drifter alternates on its own pulse period, independent of the
        // other's and of which way it faces.
        let bodies = |swimmer: &SwimmerDef, upto: u64| -> Vec<String> {
            (0..upto)
                .map(|f| sprite_rows(swimmer, area, f)[0].1.clone())
                .collect()
        };
        assert_eq!(
            bodies(&gentle, 8),
            vec!["(o)", "(o)", "[o]", "[o]", "(o)", "(o)", "[o]", "[o]"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            "period 2 holds each appearance for two frames"
        );
        assert_eq!(
            bodies(&wide, 10),
            vec!["(o)", "(o)", "(o)", "(o)", "(o)", "[o]", "[o]", "[o]", "[o]", "[o]"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            "period 5 holds each appearance for five frames"
        );
    }

    /// A swimmer's patrol window is measured in cells, not bytes: a braille
    /// three-cell body fits a pane that its nine-byte encoding would call too
    /// narrow. Without the cell count the jellyfish simply vanishes from a thin
    /// pane — the one the tank lives in.
    #[test]
    fn a_braille_swimmer_is_measured_in_cells() {
        let (area, _) = pane(6, 12);
        let jelly = &crate::content::creatures::jellyfish::DEF;
        let rows = sprite_rows(jelly, area, 0);
        assert_eq!(
            rows.first().map(|r| r.1.chars().count()),
            Some(3),
            "all three cells of the bell must fit a six-column pane, got {rows:?}"
        );
    }

    /// A fish that quirks to order: the two motions the shipped fish opted
    /// into, at whatever span, reach and rarity the test names. `rarity: 1`
    /// opens every lap, which is how a test reads the *shape* of a quirk
    /// without depending on which laps the hash happens to pick.
    fn quirky(dash: Option<Dash>, early_turn: Option<EarlyTurn>) -> SwimmerDef {
        SwimmerDef {
            right: "><>",
            left: "<><",
            slowdown: 1,
            radius: 5,
            reef_bias: true,
            color: Color::Indexed(200),
            accent: None,
            manner: Manner {
                dash,
                early_turn,
                ..Manner::PLAIN
            },
        }
    }

    /// Where one swimmer sits along its window over `frames` consecutive frames,
    /// in columns from the window's left edge.
    fn offsets(swimmer: &SwimmerDef, id: u64, travel: i64, frames: u64) -> Vec<i64> {
        (0..frames)
            .map(|f| patrol_offset(swimmer, id, 0, travel, f).0)
            .collect()
    }

    /// The largest a swimmer's position moves between two consecutive frames.
    fn widest_step(trace: &[i64]) -> i64 {
        trace
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .max()
            .unwrap_or(0)
    }

    /// A swimmer that opted into no quirk keeps exactly the beat every fish has
    /// always had: out and back on the bare triangle, one column a step, the
    /// same for every individual. This is the jellyfish's guarantee — a bloom
    /// that drifts as one is not the place for a fish's private impulse — and
    /// it holds by the definition alone, with no renderer opt-out.
    #[test]
    fn a_swimmer_without_quirks_keeps_the_plain_beat() {
        let travel = 10;
        let plain = quirky(None, None);
        let swimmers = [&crate::content::creatures::jellyfish::DEF, &plain];
        for swimmer in swimmers {
            for id in [0u64, 7, 1_234_567] {
                for frame in 0..400u64 {
                    let step = ((frame / swimmer.slowdown) % (2 * travel) as u64) as i64;
                    let want = if step < travel {
                        (step, true)
                    } else {
                        (2 * travel - step, false)
                    };
                    assert_eq!(
                        patrol_offset(swimmer, id, 0, travel, frame),
                        want,
                        "a quirkless swimmer must hold the plain triangle at frame {frame} (id {id})"
                    );
                }
            }
        }
    }

    /// A dash is a burst inside one lap: the swimmer takes two columns in a
    /// step instead of one, and gives that ground back over the rest of the lap
    /// so the lap ends where a plain lap would. Both halves matter — the burst
    /// is what the eye sees, and the giving back is what keeps the picture a
    /// pure function of the frame (nothing accumulates across laps).
    #[test]
    fn a_dash_doubles_a_stride_and_gives_the_ground_back_inside_its_lap() {
        let travel = 10;
        let lap = 2 * travel as u64;
        let fish = quirky(
            Some(Dash {
                span: 5,
                rarity: 1, // every lap, so the shape is what is under test
            }),
            None,
        );
        let plain = quirky(None, None);

        for id in [1u64, 2, 3, 99] {
            let dashed = offsets(&fish, id, travel, 6 * lap);
            let bare = offsets(&plain, id, travel, 6 * lap);
            assert_ne!(dashed, bare, "the dash must reach the picture (id {id})");
            for l in 0..6 {
                let seam = (l * lap) as usize;
                assert_eq!(
                    dashed[seam], bare[seam],
                    "lap {l} must open where a plain lap would (id {id})"
                );
            }
            assert_eq!(
                widest_step(&dashed),
                2,
                "the burst is two columns a step, never more (id {id})"
            );
            assert!(
                dashed.iter().all(|&x| (0..=travel).contains(&x)),
                "a dash stays inside the patrol window (id {id})"
            );
            // A burst somewhere in every lap, not one burst spread thin.
            for l in 0..6 {
                let seam = (l * lap) as usize;
                let this_lap = &dashed[seam..seam + lap as usize];
                assert_eq!(
                    widest_step(this_lap),
                    2,
                    "lap {l} must carry the burst (id {id}): {this_lap:?}"
                );
            }
        }
    }

    /// An early turn gives up the far end of the lap, not the window: the
    /// swimmer turns back `short` columns early and still comes home to the
    /// same left edge, so where the reef put its life is unchanged. It swims
    /// the shorter lap rather than skipping it — never more than a column a
    /// step — and a window squeezed thin by a narrow pane keeps at least half.
    #[test]
    fn an_early_turn_stops_short_without_moving_the_window() {
        let fish = quirky(
            None,
            Some(EarlyTurn {
                short: 3,
                rarity: 1, // every lap, so the shape is what is under test
            }),
        );
        let travel = 10;
        let trace = offsets(&fish, 4, travel, 5 * (2 * travel) as u64);
        assert_eq!(
            trace.iter().copied().max(),
            Some(travel - 3),
            "the turn comes three columns early: {trace:?}"
        );
        assert_eq!(
            trace.iter().copied().min(),
            Some(0),
            "and the swimmer still comes home to the window's edge: {trace:?}"
        );
        assert_eq!(
            widest_step(&trace),
            1,
            "the shorter lap is swum, not skipped: {trace:?}"
        );

        // A window barely wider than the turn: the swimmer still gets half of
        // it, rather than being pinned to a column.
        let narrow = offsets(&fish, 4, 4, 40);
        assert_eq!(
            narrow.iter().copied().max(),
            Some(2),
            "a thin window keeps half its lap: {narrow:?}"
        );
    }

    /// A quirk is one fish's own. Two individuals of the same species — a
    /// different reef, or the next ordinal on the same reef — quirk on
    /// different laps, so the tank never twitches as one. (The drift is
    /// deliberately the other way round; see `Dash`'s note on why.)
    #[test]
    fn a_quirk_belongs_to_one_swimmer_not_the_shoal() {
        let others = [
            Reef { kind: 0, slot: 5 },
            Reef { kind: 1, slot: 4 },
            Reef { kind: 0, slot: 4 },
        ];
        let reef = Reef { kind: 0, slot: 4 };
        let mine: Vec<u64> = (0..400)
            .filter(|&lap| quirk_open(quirk_roll(swimmer_id(&reef, 0), lap, DASH_SALT), 8))
            .collect();
        assert!(!mine.is_empty(), "the scan must cover some open laps");
        for (i, other) in others.iter().enumerate() {
            // The third neighbour is the same reef, one ordinal along.
            let ordinal = u64::from(i == 2);
            let theirs: Vec<u64> = (0..400)
                .filter(|&lap| {
                    quirk_open(quirk_roll(swimmer_id(other, ordinal), lap, DASH_SALT), 8)
                })
                .collect();
            // Not merely a different set: mostly a different set. A hash whose
            // individuals only shift the same beat by a lap or two would pass an
            // inequality and still twitch the tank as one.
            let shared = mine.iter().filter(|lap| theirs.contains(lap)).count();
            assert!(
                shared * 2 < mine.len(),
                "reef {other:?} ordinal {ordinal} must quirk on its own laps, \
                 not on {shared} of my {}",
                mine.len()
            );
        }
    }

    /// The quirk windows open on roughly one lap in K — the rarity that keeps a
    /// burst a surprise rather than a tic. The rates come from the shipped
    /// definitions, so retuning a fish moves this test with it (and a fish that
    /// dropped its opt-in fails here, rather than passing silently).
    #[test]
    fn quirk_windows_open_on_roughly_one_lap_in_k() {
        const LAPS: u64 = 10_000;
        let small = &crate::content::creatures::small_fish::DEF;
        let big = &crate::content::creatures::big_fish::DEF;
        let mut cases: Vec<(&str, u64, u64)> = Vec::new();
        for (name, manner) in [("small fish", &small.manner), ("big fish", &big.manner)] {
            let dash = manner
                .dash
                .as_ref()
                .unwrap_or_else(|| panic!("the {name} must keep its dash"));
            let turn = manner
                .early_turn
                .as_ref()
                .unwrap_or_else(|| panic!("the {name} must keep its early turn"));
            cases.push((name, dash.rarity, DASH_SALT));
            cases.push((name, turn.rarity, EARLY_TURN_SALT));
        }
        for (name, rarity, salt) in cases {
            let id = swimmer_id(&Reef { kind: 0, slot: 4 }, 0);
            let open = (0..LAPS)
                .filter(|&lap| quirk_open(quirk_roll(id, lap, salt), rarity))
                .count() as u64;
            let expected = LAPS / rarity;
            // The same generous +-20% band the whale's crossings are held to:
            // "rare, not never / not always", without being brittle.
            assert!(
                open > expected * 4 / 5 && open < expected * 6 / 5,
                "{name}: {open} open laps outside the design band around {expected} (1/{rarity})"
            );
        }
    }

    /// The invariants a quirk must never break, held over the shipped fish and
    /// every window width a pane can hand them: the swimmer stays inside its
    /// patrol window, never moves more than two cells in a frame (the burst's
    /// stride, and nothing beyond it), and still sweeps the whole window over
    /// time — a quirk is a flourish, not a cage. The scan is long enough that
    /// both quirks really fire, which the open-lap counts assert outright.
    #[test]
    fn a_quirked_fish_stays_inside_its_window_and_sweeps_it_all() {
        const FRAMES: u64 = 20_000;
        let small = &crate::content::creatures::small_fish::DEF;
        let big = &crate::content::creatures::big_fish::DEF;
        for (name, swimmer) in [("small fish", small), ("big fish", big)] {
            for travel in [1i64, 3, 7, 10, 20] {
                let id = swimmer_id(&Reef { kind: 0, slot: 4 }, 0);
                let trace = offsets(swimmer, id, travel, FRAMES);
                assert!(
                    trace.iter().all(|&x| (0..=travel).contains(&x)),
                    "{name}: left its window (travel {travel})"
                );
                assert!(
                    widest_step(&trace) <= 2,
                    "{name}: moved more than two cells in a frame (travel {travel})"
                );
                assert_eq!(
                    trace.iter().copied().min(),
                    Some(0),
                    "{name}: never reached the near edge (travel {travel})"
                );
                assert_eq!(
                    trace.iter().copied().max(),
                    Some(travel),
                    "{name}: never reached the far edge (travel {travel})"
                );

                // Not a vacuous scan: both windows opened inside it.
                let laps = FRAMES / (2 * travel as u64 * swimmer.slowdown);
                for (quirk, rarity, salt) in [
                    (
                        "dash",
                        swimmer.manner.dash.as_ref().expect("a dash").rarity,
                        DASH_SALT,
                    ),
                    (
                        "early turn",
                        swimmer.manner.early_turn.as_ref().expect("a turn").rarity,
                        EARLY_TURN_SALT,
                    ),
                ] {
                    let open = (0..laps)
                        .filter(|&lap| quirk_open(quirk_roll(id, lap, salt), rarity))
                        .count();
                    assert!(
                        open > 0,
                        "{name}: the {quirk} never fired in {laps} laps (travel {travel})"
                    );
                }
            }
        }
    }

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
        let species = p.reef_kinds[0].capacity.len();

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
            for kind in start..p.reef_kinds.len() {
                let rk = &p.reef_kinds[kind];
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
