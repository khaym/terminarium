//! Application shell: which layer is on screen, what input does, and how
//! wall-clock time becomes engine ticks. Pure and time-injected so every rule
//! here is unit-testable without a terminal.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::engine::{Params, Species, State};

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
    pub should_quit: bool,
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
            should_quit: false,
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
        // A wallpaper accepts no game input.
        if self.layer != Layer::Game {
            return;
        }
        match code {
            KeyCode::Char('f') => self.state.feed(&self.params),
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

    /// Wall-clock milliseconds → whole engine ticks (1 tick = 1 s); the
    /// remainder stays accumulated so no time is lost between calls.
    pub fn on_elapsed(&mut self, ms: u64) {
        self.tick_acc_ms += ms;
        while self.tick_acc_ms >= 1000 {
            self.tick_acc_ms -= 1000;
            self.state.tick(&self.params);
        }
        self.drain_surplus();
    }

    /// On the game layer, surplus is collected as it appears — silently, so
    /// feeding and production move the visible currency immediately. On the
    /// wallpaper it accrues untouched (that pile is the peek reward).
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
        let mut state = State::new();
        state.currency = 1_000 * MICRO;
        let mut app = app_with(state);
        let none = KeyModifiers::NONE;

        // Wallpaper: feed and buy are ignored.
        app.on_resize(40, 12);
        app.on_key(KeyCode::Char('f'), none);
        app.on_key(KeyCode::Char('1'), none);
        assert_eq!(app.state.biomass(), 0);
        assert_eq!(app.state.population[0], 0);

        // Game layer: feeding moves the visible currency immediately (the
        // non-recycled share is collected on the spot).
        app.on_resize(100, 30);
        app.on_key(KeyCode::Char('f'), none);
        let fed = app.params.feed_amount;
        let recycled = app.params.recycle.apply(fed);
        assert_eq!(app.state.nutrient, recycled);
        assert_eq!(app.state.currency, 1_000 * MICRO + (fed - recycled));
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
