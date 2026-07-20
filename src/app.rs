//! Application shell: which layer is on screen, what input does, and how
//! wall-clock time becomes engine ticks. Pure and time-injected so every rule
//! here is unit-testable without a terminal.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::engine::{Params, Species, State, SLOTS};

/// Below either threshold the pane is a wallpaper; at or above both it is the
/// game layer. Sized so a tmux side pane stays decorative and a zoomed pane
/// becomes playable.
pub const GAME_MIN_WIDTH: u16 = 80;
pub const GAME_MIN_HEIGHT: u16 = 20;

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

pub struct App {
    pub state: State,
    pub params: Params,
    pub layer: Layer,
    /// Animation frame counter; rendering is a pure function of state + frame.
    pub frame: u64,
    /// Collected amount to flash on the HUD, with frames left to live.
    pub flash: Option<(u128, u8)>,
    /// Floor slot the placement cursor sits on, in `0..SLOTS`. Only meaningful
    /// during the placement phase (game layer, run not started).
    pub placement_cursor: u8,
    pub should_quit: bool,
    /// Multiplier applied to wall-clock time on the way into the engine — the
    /// one knob of the test-only fast mode. 1 is real time; the binary raises
    /// it via `--time-scale`. Kept public so `main` sets it without threading a
    /// new `App::new` argument through every test.
    pub time_scale: u64,
    tick_acc_ms: u64,
}

impl App {
    pub fn new(state: State, params: Params) -> Self {
        Self {
            state,
            params,
            layer: Layer::Wallpaper,
            frame: 0,
            flash: None,
            // Start mid-floor so the first move goes either way.
            placement_cursor: SLOTS / 2,
            should_quit: false,
            time_scale: 1,
            tick_acc_ms: 0,
        }
    }

    /// Peeking = entering the game layer, which collects what accumulated
    /// while away (with a flash). While the player stays on the game layer,
    /// new surplus is collected as it appears (see `drain_surplus`), so the
    /// HUD currency always moves live.
    pub fn on_resize(&mut self, width: u16, height: u16) {
        let layer = layer_for(width, height);
        if layer == Layer::Game && self.layer == Layer::Wallpaper {
            let gained = self.state.collectable;
            self.state.collect();
            if gained > 0 {
                self.flash = Some((gained, FLASH_FRAMES));
            }
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
        // Before the first rock lands the game layer is the placement screen.
        if !self.state.run_started() {
            self.handle_placement(code);
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
            _ => {}
        }
        self.drain_surplus();
    }

    /// Placement-phase input: walk the cursor across the floor slots (wrapping
    /// at the ends) and drop the base rock, which starts the run.
    fn handle_placement(&mut self, code: KeyCode) {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.placement_cursor = (self.placement_cursor + SLOTS - 1) % SLOTS;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.placement_cursor = (self.placement_cursor + 1) % SLOTS;
            }
            KeyCode::Enter => {
                self.state
                    .place_rock(0, self.placement_cursor, &self.params);
            }
            _ => {}
        }
    }

    /// Wall-clock milliseconds → whole engine ticks (1 tick = 1 s); the
    /// remainder stays accumulated so no time is lost between calls. The clock
    /// only runs during a live run — before the first rock is placed there is
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
    use crate::engine::MICRO;

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
        // A placed rock gives algae housing, so the buy gate is capacity, not
        // the missing placement UI (that lands in the next slice).
        assert!(state.place_rock(0, 0, &params));
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
        // On the placement screen (game layer, no rock yet) the clock must not
        // run: ticking here would advance tick_count and slam the placement
        // gate shut before the player ever places a rock.
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        assert!(!app.state.run_started());

        app.on_elapsed(10_000);
        assert_eq!(
            app.state.tick_count, 0,
            "no rock means no run means no ticks"
        );
        assert!(!app.state.run_started());
    }

    #[test]
    fn placement_cursor_moves_and_wraps() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;

        // Default cursor sits mid-floor.
        assert_eq!(app.placement_cursor, SLOTS / 2);
        assert_eq!(app.placement_cursor, 2);

        // Right (and l) walks up and wraps past the last slot.
        app.on_key(KeyCode::Right, none);
        assert_eq!(app.placement_cursor, 3);
        app.on_key(KeyCode::Char('l'), none);
        assert_eq!(app.placement_cursor, 4);
        app.on_key(KeyCode::Right, none);
        assert_eq!(
            app.placement_cursor, 0,
            "past the last slot wraps to the first"
        );

        // Left (and h) walks down and wraps past the first slot.
        app.on_key(KeyCode::Left, none);
        assert_eq!(
            app.placement_cursor,
            SLOTS - 1,
            "before the first wraps to the last"
        );
        app.on_key(KeyCode::Char('h'), none);
        assert_eq!(app.placement_cursor, 3);
    }

    #[test]
    fn enter_places_the_cursor_rock_and_starts_the_run() {
        let mut app = app_with(State::new());
        app.on_resize(100, 30);
        let none = KeyModifiers::NONE;
        assert!(!app.state.run_started());

        app.on_key(KeyCode::Right, none); // cursor 2 -> 3
        app.on_key(KeyCode::Enter, none);

        assert!(
            app.state.run_started(),
            "Enter drops the rock and begins the run"
        );
        assert_eq!(app.state.rocks.len(), 1);
        assert_eq!(app.state.rocks[0].kind, 0);
        assert_eq!(app.state.rocks[0].slot, 3);

        // Once the run has started the clock runs again.
        app.on_elapsed(3_000);
        assert!(app.state.tick_count > 0, "time flows after placement");
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
        // A live run is the precondition for ticking; place a rock first.
        assert!(state.place_rock(0, 0, &Params::default()));
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
        assert!(state.place_rock(0, 0, &Params::default()));
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
        assert!(state.place_rock(0, 0, &Params::default()));
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
}
