//! Creature content: what one living thing *is*, one file per creature.
//!
//! A creature is a look and a motion — the glyphs it wears, its color, and the
//! pace it moves at. A reef kind names the creature it houses at each economy
//! tier by reference (`ReefDef`'s four species fields), so a creature two reefs
//! share is one definition read twice rather than two copies free to drift
//! apart. Adding a creature is one new file here, its `mod` line below, and the
//! one reference in the reef that hosts it.
//!
//! Three archetypes cover the four tiers: fronds rooted to a reef (algae),
//! drifting dots (plankton), and swimmers on a patrol (the fish and the dugong).
//! The wallpaper holds one drawing routine per archetype, so a new creature of
//! an existing archetype is data alone — no renderer edit.
//!
//! A swimmer that carries itself unlike a fish states the difference in data
//! too: `Manner` collects the modifiers the one patrol routine applies on top of
//! its glide — a vertical drift, a two-frame pulse, a second sprite row — each
//! defaulting to off in `Manner::PLAIN`. So the jellyfish is a definition, not a
//! second renderer, and a modifier joining `Manner` later leaves every plain
//! swimmer's file untouched.
//!
//! The order of the modules below carries no meaning. Unlike `KINDS`, whose
//! order *is* a kind's save identity, a creature is never named by a save (a
//! save stores only its host reef's kind index), so this list is free to be
//! sorted however reads best.
//!
//! Colors follow the same ownership rule as the rest of a definition: the tint a
//! creature wears lives in that creature's file.

use ratatui::style::Color;

pub mod anglerfish;
pub mod big_fish;
pub mod coralline_fronds;
pub mod dugong;
pub mod jellyfish;
pub mod kelp_blades;
pub mod lantern_moss;
pub mod noctiluca;
pub mod plankton;
pub mod seagrass;
pub mod shrimp;
pub mod small_fish;
pub mod sparse_fronds;
pub mod squid;
pub mod teal_fronds;
pub mod turtle;

/// A frond rooted to its host reef: the base species (algae) of a reef. Every
/// reef's base layer looks different, so a reef reads by its greenery alone.
pub struct FrondDef {
    /// Two sway frames of the frond glyph.
    pub fronds: [&'static str; 2],
    pub color: Color,
}

/// A speck drifting near its host reef: the plankton tier, one glyph per
/// individual.
pub struct DotDef {
    /// Four dot glyphs, cycled across a colony so neighbours differ.
    pub dots: [&'static str; 4],
    pub color: Color,
}

/// A swimmer on a bounded patrol around its host reef — the fish tiers and the
/// dugong. Keyed only by the host reef's kind, so it stays a pure function of
/// (state, frame) like every other sprite.
pub struct SwimmerDef {
    pub right: &'static str,
    pub left: &'static str,
    /// Frames per column step; higher is slower (the dugong ambles).
    pub slowdown: u64,
    /// Patrol radius, in cells either side of the host reef. Small tenants stay
    /// tight to the reef; an apex swimmer sweeps a wider, statelier beat.
    pub radius: i64,
    /// Folds the swimmer's lane into the pane's lower half, keeping it down near
    /// the reef. An apex swimmer ranges over the full height instead.
    pub reef_bias: bool,
    pub color: Color,
    /// One cell of the body worn in another color — the anglerfish's lure.
    /// The index counts cells from the left of `right`; a left-facing draw
    /// mirrors it. `None` is a single-color swimmer.
    pub accent: Option<(usize, Color)>,
    /// How this swimmer carries itself on top of the shared patrol.
    /// `Manner::PLAIN` for a fish that only glides.
    pub manner: Manner,
}

/// The modifiers a swimmer wears on top of the one patrol routine: a vertical
/// drift, a two-frame pulse, and a second sprite row. They are independent —
/// any subset composes, and each is off by default — so a creature states only
/// the ones it uses and `Manner::PLAIN` says "a fish, nothing more". A new
/// modifier joins this struct rather than the renderer, and `PLAIN` absorbs it
/// for every definition that does not want it.
pub struct Manner {
    /// Vertical sway added to the swimmer's lane.
    pub drift: Drift,
    /// A second sprite row, drawn directly under the body row (the jellyfish's
    /// tentacles). `""` is a one-row swimmer. It carries no facing: the body
    /// row alone turns with the patrol.
    pub under: &'static str,
    /// A second appearance the swimmer alternates to on a timer — a pulse in
    /// time, independent of the facing `right`/`left` already carry. `None` is
    /// a swimmer whose look never changes.
    pub pulse: Option<Pulse>,
}

impl Manner {
    /// A swimmer that only patrols: no drift, no pulse, one row. What every
    /// fish before the jellyfish wears.
    pub const PLAIN: Manner = Manner {
        drift: Drift::STILL,
        under: "",
        pulse: None,
    };
}

/// A vertical sway on the swimmer's lane: it rises and falls `amplitude` rows
/// either side of that lane, one full up-and-down every `period` frames. Whole
/// rows and whole frames — the sway is a triangle wave in integers, so it stays
/// as reproducible as the rest of the picture.
pub struct Drift {
    /// Rows either side of the lane. `0` is no drift at all.
    pub amplitude: i64,
    /// Frames per full up-and-down. Never 0 (a still drift keeps 1).
    pub period: u64,
}

impl Drift {
    /// No drift — the swimmer holds its lane.
    pub const STILL: Drift = Drift {
        amplitude: 0,
        period: 1,
    };
}

/// A swimmer's second appearance and the beat it alternates on: every `period`
/// frames the sprite switches between the swimmer's own rows and these. The
/// beat is time alone, so a pulsing swimmer keeps pulsing whichever way it
/// happens to be facing.
pub struct Pulse {
    /// The rows the swimmer wears on the pulse's beat.
    pub look: Look,
    /// Frames each appearance is held. Never 0 (guarded at the draw).
    pub period: u64,
}

/// One appearance of a swimmer: the body row it wears facing each way, and the
/// row hanging under it. The swimmer's own fields are its first appearance;
/// `Pulse` carries the second.
#[derive(Clone, Copy)]
pub struct Look {
    pub right: &'static str,
    pub left: &'static str,
    pub under: &'static str,
}

impl Look {
    /// Rows this appearance draws: two once it has an under row.
    fn rows(&self) -> u16 {
        if self.under.is_empty() {
            1
        } else {
            2
        }
    }

    /// Widest row of this appearance, in cells (not bytes — every glyph is one
    /// cell, and braille takes three bytes to say so).
    fn span(&self) -> usize {
        self.right
            .chars()
            .count()
            .max(self.left.chars().count())
            .max(self.under.chars().count())
    }
}

impl SwimmerDef {
    /// This swimmer's own appearance — the rows it wears off the pulse's beat.
    fn own(&self) -> Look {
        Look {
            right: self.right,
            left: self.left,
            under: self.manner.under,
        }
    }

    /// The appearance to draw at `frame`: this swimmer's own rows, or its
    /// pulse's on the alternate beat.
    pub fn look(&self, frame: u64) -> Look {
        match &self.manner.pulse {
            Some(pulse) if (frame / pulse.period.max(1)) % 2 == 1 => pulse.look,
            _ => self.own(),
        }
    }

    /// The room every appearance needs, in cells and rows. Taken over both
    /// appearances rather than the drawn one, so the patrol window and the lane
    /// stay put while the swimmer pulses instead of shuffling with it.
    pub fn footprint(&self) -> (usize, u16) {
        let own = self.own();
        let (mut span, mut rows) = (own.span(), own.rows());
        if let Some(pulse) = &self.manner.pulse {
            span = span.max(pulse.look.span());
            rows = rows.max(pulse.look.rows());
        }
        (span, rows)
    }
}
