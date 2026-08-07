//! Application shell: which layer is on screen, what input does, and how
//! wall-clock time becomes engine ticks. Pure and time-injected so every rule
//! here is unit-testable without a terminal.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::engine::{Params, Species, State, ANCHOR_POS_MAX, SLOTS};

/// Below either threshold the pane is a wallpaper; at or above both it is the
/// game layer. Sized so a tmux side pane stays decorative and a zoomed pane
/// becomes playable.
pub const GAME_MIN_WIDTH: u16 = 80;
pub const GAME_MIN_HEIGHT: u16 = 20;

/// Length of one animation frame, in milliseconds (~5 fps). The unit of every
/// frame count here (`FLASH_FRAMES`, `App::frame`), so it lives beside them
/// rather than in the binary: the event loop and the scenario harness both drive
/// frames at this rhythm, and one definition keeps the two in step.
pub const FRAME_INTERVAL_MS: u64 = 200;

/// How long the collect flash stays visible, in animation frames (~5 fps).
const FLASH_FRAMES: u8 = 15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Wallpaper,
    Game,
}

pub fn layer_for(width: u16, height: u16) -> Layer {
    if width >= GAME_MIN_WIDTH && height >= GAME_MIN_HEIGHT {
        Layer::Game
    } else {
        Layer::Wallpaper
    }
}

/// Time-of-day phase, the explicit rendering input that colors the sea. Derived
/// from the local wall-clock hour in the binary and carried on `App`, so the
/// renderer stays a pure function of (state, frame, phase). Night is the
/// pre-phase palette, so a night render is byte-for-byte the old picture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl Phase {
    /// The phase for a 24-hour local hour. Boundaries are placeholders (dawn
    /// 5-8, day 8-17, dusk 17-20, night 20-5), fixed as constants here so tuning
    /// them is a one-line change. Hours outside the day span (including 0-4 and
    /// 20-23) are night.
    pub fn from_hour(hour: u32) -> Phase {
        const DAWN_START: u32 = 5;
        const DAY_START: u32 = 8;
        const DUSK_START: u32 = 17;
        const NIGHT_START: u32 = 20;
        match hour {
            h if (DAWN_START..DAY_START).contains(&h) => Phase::Dawn,
            h if (DAY_START..DUSK_START).contains(&h) => Phase::Day,
            h if (DUSK_START..NIGHT_START).contains(&h) => Phase::Dusk,
            _ => Phase::Night,
        }
    }
}

pub struct App {
    pub state: State,
    pub params: Params,
    pub layer: Layer,
    /// Time-of-day phase, an explicit render input. Defaults to `Night` (the
    /// pre-phase palette) so a bare `App` renders exactly the old picture; the
    /// binary overwrites it each frame from the local clock.
    pub phase: Phase,
    /// Animation frame counter; rendering is a pure function of state + frame.
    pub frame: u64,
    /// Wall-clock frame offset captured at launch, so animation time is anchored
    /// to real time rather than restarting from zero each run. The renderer rides
    /// `frame_epoch + frame`; the binary seeds it from the clock, and a bare
    /// `App` leaves it 0 so tests and snapshots see the unshifted timeline.
    pub frame_epoch: u64,
    /// Collected amount to flash on the HUD, with frames left to live.
    pub flash: Option<(u128, u8)>,
    /// Floor slot the placement cursor sits on, in `0..SLOTS`. Only meaningful
    /// during the placement phase (game layer, run not started).
    pub placement_cursor: u8,
    /// Reef kind the placement cursor will drop. Cycled among the kinds the
    /// score has unlocked; only meaningful during the placement phase.
    pub placement_kind: usize,
    /// Whether a "start a new sea?" confirmation is awaiting a keypress. While
    /// set, buy input is suppressed until the prompt is answered.
    pub new_sea_pending: bool,
    /// Whether the anchor-move mode owns input. A game-layer modal, the same in
    /// placement and during a run: while set, h/l and the arrows move the anchor
    /// and every other game key is swallowed. Toggled by 'a' once the score has
    /// unlocked the anchor.
    pub anchor_mode: bool,
    pub should_quit: bool,
    /// Multiplier applied to wall-clock time on the way into the engine — the
    /// one knob of the test-only fast mode. 1 is real time; the binary raises
    /// it via `--time-scale`. Kept public so `main` sets it without threading a
    /// new `App::new` argument through every test.
    pub time_scale: u64,
    tick_acc_ms: u64,
    /// Current pane width, captured on every resize. The anchor-move step is
    /// derived from it so one keypress moves the anchor at least one column at
    /// the width the player is actually looking at.
    width: u16,
}

impl App {
    pub fn new(state: State, params: Params) -> Self {
        Self {
            state,
            params,
            layer: Layer::Wallpaper,
            phase: Phase::Night,
            frame: 0,
            frame_epoch: 0,
            flash: None,
            // Start mid-floor so the first move goes either way.
            placement_cursor: SLOTS / 2,
            placement_kind: 0,
            new_sea_pending: false,
            anchor_mode: false,
            should_quit: false,
            time_scale: 1,
            tick_acc_ms: 0,
            width: 0,
        }
    }

    /// Peeking = entering the game layer, which collects what accumulated
    /// while away (with a flash). While the player stays on the game layer,
    /// new surplus is collected as it appears (see `drain_surplus`), so the
    /// HUD currency always moves live.
    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.width = width;
        let layer = layer_for(width, height);
        if layer == Layer::Game && self.layer == Layer::Wallpaper {
            let gained = self.state.collectable;
            self.state.collect();
            if gained > 0 {
                self.flash = Some((gained, FLASH_FRAMES));
            }
        }
        // A wallpaper is a still picture that takes no input, so a grabbed
        // anchor must not linger on it — shrinking to it releases the move mode.
        // Re-expanding does not resume it; the player re-enters with 'a'.
        if layer == Layer::Wallpaper {
            self.anchor_mode = false;
        }
        self.layer = layer;
    }

    pub fn on_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if code == KeyCode::Char('q')
            || (code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL))
        {
            self.should_quit = true;
            return;
        }
        // A wallpaper accepts no game input — neither placement nor buying.
        if self.layer != Layer::Game {
            return;
        }
        // Anchor-move mode is a game-layer modal, the same in placement and
        // during a run: while active it owns h/l and the arrows and swallows
        // every other game key. 'a' toggles it, gated on the score having
        // unlocked the anchor.
        if self.anchor_mode {
            self.handle_anchor_mode(code);
            return;
        }
        if code == KeyCode::Char('a') && self.anchor_unlocked() {
            self.anchor_mode = true;
            // A mode change dismisses any half-armed new-sea prompt.
            self.new_sea_pending = false;
            return;
        }
        // Before the run is committed the game layer is the placement screen.
        if !self.state.run_started() {
            self.handle_placement(code);
            return;
        }
        // A pending "new sea?" prompt swallows the next key: 'y' confirms,
        // anything else cancels — and either way no buy fires this keystroke.
        if self.new_sea_pending {
            self.new_sea_pending = false;
            if code == KeyCode::Char('y') {
                self.state.reset();
                self.placement_cursor = SLOTS / 2;
                self.placement_kind = 0;
            }
            return;
        }
        match code {
            KeyCode::Char('1') => {
                self.state.buy(Species::Algae, &self.params);
            }
            KeyCode::Char('2') => {
                self.state.buy(Species::Plankton, &self.params);
            }
            KeyCode::Char('3') => {
                self.state.buy(Species::SmallFish, &self.params);
            }
            KeyCode::Char('4') => {
                self.state.buy(Species::BigFish, &self.params);
            }
            // Arm the new-sea confirmation; nothing changes until it is answered.
            KeyCode::Char('n') => {
                self.new_sea_pending = true;
            }
            _ => {}
        }
        self.drain_surplus();
    }

    /// Placement-phase input: move the cursor across the floor slots (h/l or
    /// arrows, wrapping), cycle the reef kind among those unlocked (j/k or
    /// arrows), drop the selected kind (enter) or lift the one under the cursor
    /// (backspace/delete), and commit the run (s).
    fn handle_placement(&mut self, code: KeyCode) {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.placement_cursor = (self.placement_cursor + SLOTS - 1) % SLOTS;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.placement_cursor = (self.placement_cursor + 1) % SLOTS;
            }
            KeyCode::Up | KeyCode::Char('k') => self.cycle_kind(1),
            KeyCode::Down | KeyCode::Char('j') => self.cycle_kind(-1),
            KeyCode::Enter => {
                self.state
                    .place_reef(self.placement_kind, self.placement_cursor, &self.params);
            }
            KeyCode::Backspace | KeyCode::Delete => {
                self.state.remove_reef(self.placement_cursor);
            }
            KeyCode::Char('s') => {
                self.state.start_run(&self.params);
            }
            _ => {}
        }
    }

    /// Move `placement_kind` by `delta` steps through the unlocked kinds
    /// (wrapping). Each kind is tested against its own unlock rather than a cut
    /// through the list: the manifest is append-only, so a kind added later can
    /// unlock earlier and the unlocked ones are not always a prefix of it.
    fn cycle_kind(&mut self, delta: i32) {
        let unlocked: Vec<usize> = (0..self.params.reef_kinds.len())
            .filter(|&k| self.state.score >= self.params.reef_kinds[k].unlock)
            .collect();
        if unlocked.is_empty() {
            return;
        }
        let here = unlocked
            .iter()
            .position(|&k| k == self.placement_kind)
            .unwrap_or(0) as i32;
        let n = unlocked.len() as i32;
        let next = (here + delta).rem_euclid(n) as usize;
        self.placement_kind = unlocked[next];
    }

    /// Anchor-move input: slide the anchor along the floor (h/l or arrows,
    /// clamped to 0..=999), or leave the mode (Esc/Enter/a). Movement is
    /// immediate — there is no staged position to commit or cancel, the same as
    /// placing a reef. Every other key is swallowed so the mode owns input.
    fn handle_anchor_mode(&mut self, code: KeyCode) {
        let step = self.anchor_step();
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.state.anchor_pos = self.state.anchor_pos.saturating_sub(step);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.state.anchor_pos = self
                    .state
                    .anchor_pos
                    .saturating_add(step)
                    .min(ANCHOR_POS_MAX);
            }
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('a') => {
                self.anchor_mode = false;
            }
            _ => {}
        }
    }

    /// Position delta that moves the anchor at least one pane column per press.
    /// The renderer places it at `left + width·pos/1000`, so a delta of
    /// `ceil(1000/width)` shifts that column by at least one at the current
    /// width (a wider pane needs a bigger delta to clear a whole column).
    fn anchor_step(&self) -> u16 {
        let w = u32::from(self.width.max(1));
        1000u32.div_ceil(w) as u16
    }

    /// Whether the lifetime score has unlocked the anchor — the gate for
    /// entering anchor-move mode, and for adding its key to the hint row.
    pub fn anchor_unlocked(&self) -> bool {
        self.state.score >= self.params.anchor_unlock
    }

    /// Wall-clock milliseconds → whole engine ticks (1 tick = 1 s); the
    /// remainder stays accumulated so no time is lost between calls. The clock
    /// only runs during a live run — before the first reef is placed there is
    /// nothing to advance (mirroring the engine contract that the caller must
    /// not `advance` pre-placement), so accumulated time and the run clock both
    /// start fresh at placement.
    ///
    /// This is one of the two places wall-clock time enters the engine (the
    /// other is offline settlement in `save::load`); both scale it by
    /// `time_scale` for the test-only fast mode. The multiply saturates so an
    /// extreme scale cannot overflow the accumulator.
    pub fn on_elapsed(&mut self, ms: u64) {
        if self.state.run_started() {
            self.tick_acc_ms += ms.saturating_mul(self.time_scale);
            while self.tick_acc_ms >= 1000 {
                self.tick_acc_ms -= 1000;
                self.state.tick(&self.params);
            }
        }
        self.drain_surplus();
    }

    /// On the game layer, surplus is collected as it appears — silently, so
    /// production moves the visible currency immediately. On the wallpaper it
    /// accrues untouched (that pile is the peek reward).
    fn drain_surplus(&mut self) {
        if self.layer == Layer::Game {
            self.state.collect();
        }
    }

    /// Advance one animation frame and age the flash message.
    pub fn on_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        if let Some((_, ttl)) = &mut self.flash {
            *ttl -= 1;
            if *ttl == 0 {
                self.flash = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{ReefKind, MICRO, SPECIES};

    fn app_with(state: State) -> App {
        App::new(state, Params::default())
    }

    #[test]
    fn layer_is_decided_by_size() {
        assert_eq!(layer_for(79, 30), Layer::Wallpaper);
        assert_eq!(layer_for(200, 19), Layer::Wallpaper);
        assert_eq!(layer_for(80, 20), Layer::Game);
        assert_eq!(layer_for(188, 45), Layer::Game);
    }

    #[test]
    fn entering_game_layer_collects_with_flash_then_drains_silently() {
        let mut state = State::new();
        state.collectable = 420 * MICRO;
        let mut app = app_with(state);

        app.on_resize(100, 30);
        assert_eq!(app.layer, Layer::Game);
        assert_eq!(app.state.currency, 420 * MICRO);
        assert_eq!(app.state.collectable, 0);
        assert_eq!(app.flash, Some((420 * MICRO, 15)));

        // While staying on the game layer, new surplus is collected as time
        // passes — silently, without a new flash.
        app.flash = None;
        app.state.collectable = 7;
        app.on_elapsed(1_000);
        assert_eq!(app.state.collectable, 0);
        assert_eq!(app.state.currency, 420 * MICRO + 7);
        assert_eq!(app.flash, None);

        // Back on the wallpaper the surplus accrues untouched.
        app.on_resize(40, 12);
        app.state.collectable = 9;
        app.on_elapsed(1_000);
        assert_eq!(app.state.collectable, 9);
        assert_eq!(app.state.currency, 420 * MICRO + 7);

        // The next peek collects the pile again, with a flash.
        app.on_resize(100, 30);
        assert_eq!(app.state.collectable, 0);
        assert_eq!(app.state.currency, 420 * MICRO + 7 + 9);
        assert_eq!(app.flash, Some((9, 15)));
    }

    #[test]
    fn entering_with_nothing_to_collect_shows_no_flash() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        assert_eq!(app.flash, None);
    }

    #[test]
    fn game_input_works_only_in_game_layer() {
        let params = Params::default();
        let mut state = State::new();
        // A committed run with algae housing, so the buy gate is capacity, not
        // the placement screen. currency is set after start so the seed grant
        // does not perturb the fixed amount.
        assert!(state.place_reef(0, 0, &params));
        assert!(state.start_run(&params));
        state.currency = 1_000 * MICRO;
        let mut app = App::new(state, params);
        let none = KeyModifiers::NONE;

        // Wallpaper: buy is ignored.
        app.on_resize(40, 12);
        app.on_key(KeyCode::Char('1'), none);
        assert_eq!(app.state.population[0], 0);

        // Game layer: buying works.
        app.on_resize(100, 30);
        app.on_key(KeyCode::Char('1'), none);
        assert_eq!(app.state.population[0], 1);
    }

    #[test]
    fn no_time_flows_before_the_run_starts() {
        // On the placement screen (game layer, run not committed) the clock must
        // not run: reef output is anchored to the start, so nothing should age
        // while the player is still composing.
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        assert!(!app.state.run_started());

        app.on_elapsed(10_000);
        assert_eq!(app.state.tick_count, 0, "an uncommitted run means no ticks");
        assert!(!app.state.run_started());
    }

    #[test]
    fn placement_cursor_moves_and_wraps() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        // Default cursor sits mid-floor.
        assert_eq!(app.placement_cursor, SLOTS / 2);
        assert_eq!(app.placement_cursor, 4);

        // Right (and l) walk up by one.
        app.on_key(KeyCode::Right, none);
        assert_eq!(app.placement_cursor, 5);
        app.on_key(KeyCode::Char('l'), none);
        assert_eq!(app.placement_cursor, 6);

        // From the last slot, stepping right wraps to the first.
        app.placement_cursor = SLOTS - 1;
        app.on_key(KeyCode::Right, none);
        assert_eq!(
            app.placement_cursor, 0,
            "past the last slot wraps to the first"
        );

        // From the first slot, stepping left wraps to the last.
        app.on_key(KeyCode::Left, none);
        assert_eq!(
            app.placement_cursor,
            SLOTS - 1,
            "before the first wraps to the last"
        );
        app.on_key(KeyCode::Char('h'), none);
        assert_eq!(app.placement_cursor, SLOTS - 2);
    }

    #[test]
    fn enter_places_a_reef_and_s_starts_the_run() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;
        assert!(!app.state.run_started());

        app.on_key(KeyCode::Right, none); // cursor 4 -> 5
        app.on_key(KeyCode::Enter, none);

        assert!(
            !app.state.run_started(),
            "Enter drops a reef but no longer begins the run"
        );
        assert_eq!(app.state.reefs.len(), 1);
        assert_eq!(app.state.reefs[0].kind, 0);
        assert_eq!(app.state.reefs[0].slot, 5);

        // The clock stays still until the run is committed.
        app.on_elapsed(3_000);
        assert_eq!(app.state.tick_count, 0, "no clock before start");

        // 's' commits the placement and starts the run.
        app.on_key(KeyCode::Char('s'), none);
        assert!(app.state.run_started(), "s begins the run");
        assert_eq!(
            app.state.currency, app.params.seed_currency,
            "start grants the seed once"
        );

        // Once the run has started the clock runs.
        app.on_elapsed(3_000);
        assert!(app.state.tick_count > 0, "time flows after start");
    }

    #[test]
    fn placement_input_is_ignored_on_the_wallpaper() {
        let mut app = app_with(State::new());
        app.on_resize(40, 12); // wallpaper: no game input at all
        let none = KeyModifiers::NONE;
        let start = app.placement_cursor;

        app.on_key(KeyCode::Right, none);
        assert_eq!(
            app.placement_cursor, start,
            "the wallpaper takes no placement input"
        );
        app.on_key(KeyCode::Enter, none);
        assert!(
            !app.state.run_started(),
            "Enter on the wallpaper places nothing"
        );
    }

    /// The kind cycle walks the kinds the score has unlocked, no others, and
    /// wraps at both ends. It is a rule about unlock scores rather than about
    /// the reefs that ship, so it is stated against a manifest of this test's
    /// own: two kinds with made-up names, which keeps adding a reef kind from
    /// moving this test (the shipped progression is pinned once, by
    /// `content::tests::progression_registry_pins_cost_and_unlock`).
    #[test]
    fn placement_cycles_only_unlocked_kinds() {
        let kind = |name: &'static str, unlock: u128| ReefKind {
            name,
            cost: 1,
            unlock,
            output: 0,
            capacity: [0; SPECIES],
        };
        let params = Params {
            reef_kinds: vec![kind("amber", 0), kind("cobalt", 500 * MICRO)],
            ..Params::default()
        };
        let mut app = App::new(State::new(), params);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        // Score 0: only the first kind is unlocked, so cycling is a no-op.
        assert_eq!(app.placement_kind, 0);
        app.on_key(KeyCode::Up, none);
        assert_eq!(app.placement_kind, 0, "only amber unlocked at score 0");

        // Score past the second unlock brings it into the cycle.
        app.state.score = 500 * MICRO;
        app.on_key(KeyCode::Up, none);
        assert_eq!(app.placement_kind, 1, "cobalt now selectable");
        app.on_key(KeyCode::Up, none);
        assert_eq!(app.placement_kind, 0, "the cycle wraps back to amber");
        app.on_key(KeyCode::Char('j'), none);
        assert_eq!(app.placement_kind, 1, "j walks the other way");
    }

    #[test]
    fn place_several_reefs_remove_one_then_start() {
        let mut state = State::new();
        state.score = 12_000 * MICRO; // budget 2 admits two base reefs
        let mut app = app_with(state);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        app.on_key(KeyCode::Enter, none); // reef at cursor 4
        app.on_key(KeyCode::Right, none); // cursor -> 5
        app.on_key(KeyCode::Enter, none); // reef at 5
        assert_eq!(app.state.reefs.len(), 2);
        assert!(!app.state.run_started(), "placing does not start the run");

        // Backspace lifts the reef under the cursor (slot 5).
        app.on_key(KeyCode::Backspace, none);
        assert_eq!(app.state.reefs.len(), 1);
        assert_eq!(app.state.reefs[0].slot, 4);

        app.on_key(KeyCode::Char('s'), none);
        assert!(app.state.run_started());
        assert_eq!(
            app.state.currency, app.params.seed_currency,
            "the seed is granted once, at start"
        );
    }

    #[test]
    fn new_sea_needs_confirmation_and_a_stray_key_cancels() {
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.score = 500 * MICRO;
        state.population[0] = 2;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        // 'n' arms the prompt but changes nothing yet.
        app.on_key(KeyCode::Char('n'), none);
        assert!(app.new_sea_pending);
        assert!(app.state.run_started());
        assert_eq!(app.state.population[0], 2);

        // Any non-'y' key cancels, leaving the run untouched.
        app.on_key(KeyCode::Char('x'), none);
        assert!(!app.new_sea_pending, "the prompt is dismissed");
        assert!(app.state.run_started());
        assert_eq!(app.state.population[0], 2);

        // Arm again and confirm: reset to a fresh placement, keeping score.
        app.on_key(KeyCode::Char('n'), none);
        app.on_key(KeyCode::Char('y'), none);
        assert!(!app.state.run_started(), "y returns to placement");
        assert!(app.state.reefs.is_empty());
        assert_eq!(app.state.population, [0, 0, 0, 0]);
        assert_eq!(app.state.score, 500 * MICRO, "score survives the new sea");
        assert_eq!(app.placement_cursor, SLOTS / 2);
        assert_eq!(app.placement_kind, 0);
    }

    #[test]
    fn buy_is_suppressed_while_new_sea_is_pending() {
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.currency = 1_000 * MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        app.on_key(KeyCode::Char('n'), none); // arm the prompt
        app.on_key(KeyCode::Char('1'), none); // would buy algae — but cancels instead
        assert_eq!(
            app.state.population[0], 0,
            "a buy key does not fire while the prompt is pending"
        );
        assert!(!app.new_sea_pending, "and it dismisses the prompt");

        // With the prompt gone the same key buys.
        app.on_key(KeyCode::Char('1'), none);
        assert_eq!(app.state.population[0], 1);
    }

    #[test]
    fn quit_works_in_both_layers() {
        let mut app = app_with(State::new());
        app.on_resize(40, 12);
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.should_quit);

        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        app.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.should_quit);
    }

    #[test]
    fn elapsed_milliseconds_accumulate_into_whole_ticks() {
        let mut state = State::new();
        // A live run is the precondition for ticking; place and start first.
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.population[0] = 1;
        let mut app = app_with(state.clone());

        app.on_elapsed(500);
        assert_eq!(app.state, state, "half a second is not a tick yet");

        app.on_elapsed(500);
        let mut expected = state.clone();
        expected.tick(&app.params);
        assert_eq!(app.state, expected, "two halves make one tick");

        app.on_elapsed(2_300);
        expected.advance(2, &app.params);
        assert_eq!(app.state, expected, "2300ms = 2 ticks + 300ms carried");

        app.on_elapsed(700);
        expected.tick(&app.params);
        assert_eq!(app.state, expected, "the carried 300ms completes a tick");
    }

    #[test]
    fn time_scale_defaults_to_real_time() {
        let app = app_with(State::new());
        assert_eq!(app.time_scale, 1);
    }

    #[test]
    fn time_scale_multiplies_elapsed_before_ticking() {
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.population[0] = 1;
        let mut app = app_with(state.clone());
        app.time_scale = 60;

        // At 60x, one wall-clock second is a full minute of ticks.
        app.on_elapsed(1_000);
        let mut expected = state.clone();
        expected.advance(60, &app.params);
        assert_eq!(app.state, expected, "scale=60: 1000ms = 60 ticks");
    }

    #[test]
    fn time_scale_carries_the_sub_tick_remainder() {
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.population[0] = 1;
        let mut app = app_with(state.clone());
        app.time_scale = 60;

        // 10ms * 60 = 600ms scaled: not a full tick yet, so it carries.
        app.on_elapsed(10);
        assert_eq!(app.state, state, "600ms scaled is still sub-tick");

        // Another 10ms * 60 = 600ms, 1200ms scaled total: one tick, 200 carried.
        app.on_elapsed(10);
        let mut expected = state.clone();
        expected.tick(&app.params);
        assert_eq!(
            app.state, expected,
            "the carried remainder completes a tick"
        );
    }

    #[test]
    fn phase_boundaries_map_hours_to_time_of_day() {
        // Placeholder boundaries: dawn 5-8, day 8-17, dusk 17-20, night 20-5.
        // Boundary hours belong to the later phase (half-open intervals).
        assert_eq!(Phase::from_hour(4), Phase::Night, "pre-dawn is night");
        assert_eq!(Phase::from_hour(5), Phase::Dawn, "dawn opens at 5");
        assert_eq!(Phase::from_hour(7), Phase::Dawn);
        assert_eq!(Phase::from_hour(8), Phase::Day, "day opens at 8");
        assert_eq!(Phase::from_hour(12), Phase::Day);
        assert_eq!(Phase::from_hour(16), Phase::Day);
        assert_eq!(Phase::from_hour(17), Phase::Dusk, "dusk opens at 17");
        assert_eq!(Phase::from_hour(19), Phase::Dusk);
        assert_eq!(Phase::from_hour(20), Phase::Night, "night opens at 20");
        assert_eq!(Phase::from_hour(23), Phase::Night);
        assert_eq!(Phase::from_hour(0), Phase::Night, "midnight is night");
    }

    #[test]
    fn a_bare_app_is_night_so_old_snapshots_are_the_regression_guard() {
        // Night is the pre-phase palette; defaulting to it keeps every existing
        // snapshot byte-for-byte, which is what makes them the phase regression
        // guard. The binary overwrites the phase from the clock at runtime.
        let app = app_with(State::new());
        assert_eq!(app.phase, Phase::Night);
    }

    #[test]
    fn flash_expires_after_its_frames() {
        let mut state = State::new();
        state.collectable = MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        assert!(app.flash.is_some());
        for _ in 0..15 {
            app.on_frame();
        }
        assert_eq!(app.flash, None);
    }

    #[test]
    fn anchor_mode_toggles_only_once_the_score_unlocks_the_anchor() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        // Below the unlock 'a' does nothing.
        assert!(app.state.score < app.params.anchor_unlock);
        app.on_key(KeyCode::Char('a'), none);
        assert!(!app.anchor_mode, "locked: 'a' cannot enter anchor mode");

        // Once unlocked 'a' toggles the mode on, and again off.
        app.state.score = app.params.anchor_unlock;
        app.on_key(KeyCode::Char('a'), none);
        assert!(app.anchor_mode, "unlocked: 'a' enters anchor mode");
        app.on_key(KeyCode::Char('a'), none);
        assert!(!app.anchor_mode, "'a' again leaves anchor mode");
    }

    #[test]
    fn anchor_mode_moves_at_least_a_column_per_press_and_clamps() {
        let mut app = app_with(State::new());
        app.state.score = app.params.anchor_unlock;
        let w = 100u16;
        app.on_resize(w, 30);
        let none = KeyModifiers::NONE;
        // The renderer's column for a position (see wallpaper::draw_anchor).
        let col = |pos: u16| u32::from(w) * u32::from(pos) / 1000;

        app.on_key(KeyCode::Char('a'), none);
        assert!(app.anchor_mode);

        // One press moves the anchor by at least one pane column, either way.
        let start = app.state.anchor_pos;
        app.on_key(KeyCode::Right, none);
        assert!(
            col(app.state.anchor_pos) > col(start),
            "one press right moves at least one column"
        );
        let here = app.state.anchor_pos;
        app.on_key(KeyCode::Char('h'), none); // 'h' moves left
        assert!(
            col(here) > col(app.state.anchor_pos),
            "one press left moves at least one column"
        );

        // Clamped to 0..=999: from the right edge right stays, from the left
        // edge left stays.
        app.state.anchor_pos = 999;
        app.on_key(KeyCode::Char('l'), none);
        assert_eq!(app.state.anchor_pos, 999, "clamped at the right edge");
        app.state.anchor_pos = 0;
        app.on_key(KeyCode::Left, none);
        assert_eq!(app.state.anchor_pos, 0, "clamped at the left edge");
    }

    #[test]
    fn anchor_mode_works_the_same_in_placement_and_during_a_run() {
        let none = KeyModifiers::NONE;

        // Placement phase (run not started).
        let mut app = app_with(State::new());
        app.state.score = app.params.anchor_unlock;
        app.on_resize(100, 30);
        assert!(!app.state.run_started());
        app.on_key(KeyCode::Char('a'), none);
        assert!(app.anchor_mode, "anchor mode opens on the placement screen");
        let before = app.state.anchor_pos;
        app.on_key(KeyCode::Left, none);
        assert_ne!(app.state.anchor_pos, before, "and moves the anchor there");
        app.on_key(KeyCode::Esc, none);
        assert!(!app.anchor_mode, "Esc leaves the mode");

        // During a live run.
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.score = 75_000 * MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        assert!(app.state.run_started());
        app.on_key(KeyCode::Char('a'), none);
        assert!(app.anchor_mode, "anchor mode opens during a run too");
        let before = app.state.anchor_pos;
        app.on_key(KeyCode::Right, none);
        assert_ne!(app.state.anchor_pos, before, "and moves the anchor");
        app.on_key(KeyCode::Enter, none);
        assert!(!app.anchor_mode, "Enter leaves the mode");
    }

    #[test]
    fn anchor_mode_swallows_other_game_keys() {
        // A live run with money and housing, so a buy key would otherwise fire.
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.score = 75_000 * MICRO;
        state.currency = 1_000 * MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        app.on_key(KeyCode::Char('a'), none);
        assert!(app.anchor_mode);

        // A buy key does not buy while the mode owns input.
        app.on_key(KeyCode::Char('1'), none);
        assert_eq!(
            app.state.population[0], 0,
            "buy is swallowed in anchor mode"
        );
        assert!(app.anchor_mode, "a swallowed key does not leave the mode");

        // 'n' does not arm the new-sea prompt either.
        app.on_key(KeyCode::Char('n'), none);
        assert!(!app.new_sea_pending, "new sea is swallowed in anchor mode");
        assert!(app.anchor_mode);
    }

    #[test]
    fn entering_anchor_mode_dismisses_a_pending_new_sea_prompt() {
        let mut state = State::new();
        assert!(state.place_reef(0, 0, &Params::default()));
        assert!(state.start_run(&Params::default()));
        state.score = 75_000 * MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        app.on_key(KeyCode::Char('n'), none); // arm the prompt
        assert!(app.new_sea_pending);
        app.on_key(KeyCode::Char('a'), none); // enter anchor mode instead
        assert!(app.anchor_mode);
        assert!(!app.new_sea_pending, "the mode change dismisses the prompt");
    }

    #[test]
    fn quit_works_even_in_anchor_mode() {
        let mut state = State::new();
        state.score = 75_000 * MICRO;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.anchor_mode);
        app.on_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(
            app.should_quit,
            "quit is global, even while moving the anchor"
        );
    }

    #[test]
    fn shrinking_to_the_wallpaper_leaves_anchor_mode() {
        // The wallpaper is a still picture that takes no input, so a grabbed
        // anchor must not linger there — shrinking below the game size drops the
        // mode. Re-expanding does not resume it (the player re-enters with 'a').
        let mut state = State::new();
        state.score = Params::default().anchor_unlock;
        let mut app = app_with(state);
        app.on_resize(100, 30);
        app.on_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(app.anchor_mode, "the mode is on at game size");

        app.on_resize(40, 12);
        assert_eq!(app.layer, Layer::Wallpaper);
        assert!(
            !app.anchor_mode,
            "the wallpaper takes no input, so the anchor is released"
        );

        app.on_resize(100, 30);
        assert!(
            !app.anchor_mode,
            "the mode does not resume on re-expand; 'a' re-enters it"
        );
    }
}
