//! Binary entry: terminal lifecycle and the event loop. All rules live in the
//! library (app / engine / save); this file only wires clock, events, and
//! screen together.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{Local, Timelike};
use crossterm::event::{self, Event, KeyEventKind};
use tui_game::app::{App, Phase};
use tui_game::engine::Params;
use tui_game::{cli, save, ui};

const POLL_TIMEOUT: Duration = Duration::from_millis(50);
const FRAME_INTERVAL: Duration = Duration::from_millis(200);
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(5);

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

fn main() -> io::Result<()> {
    // A bad flag is the caller's error: surface it and exit non-zero rather
    // than falling back to real time (which would silently ignore the mistake).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match cli::parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let params = Params::default();
    let path = save::path_for(options.time_scale);
    let state = save::load(&path, &params, unix_now(), options.time_scale);
    let mut app = App::new(state, params);
    app.time_scale = options.time_scale;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, &path);
    ratatui::restore();

    save::store(&path, &app.state, unix_now())?;
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, save_path: &Path) -> io::Result<()> {
    let size = terminal.size()?;
    app.on_resize(size.width, size.height);

    // Production time is measured on the wall clock, not Instant: monotonic
    // clocks stall across suspend, which would silently confiscate the hours
    // a resident pane sleeps through (and autosave would then pin that loss).
    // Live ticking and offline settlement thus share one clock. Backward
    // jumps (NTP) saturate to zero, same as settlement.
    let mut last_wall_ms = unix_now_ms();
    let mut next_frame = Instant::now();
    let mut next_save = Instant::now() + AUTOSAVE_INTERVAL;

    while !app.should_quit {
        if event::poll(POLL_TIMEOUT)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.on_key(key.code, key.modifiers);
                }
                Event::Resize(width, height) => app.on_resize(width, height),
                _ => {}
            }
        }

        let now_ms = unix_now_ms();
        let ms = now_ms.saturating_sub(last_wall_ms);
        if ms > 0 {
            last_wall_ms = now_ms;
            app.on_elapsed(ms);
        }

        if Instant::now() >= next_frame {
            app.on_frame();
            // Refresh the time-of-day phase from the local clock each frame, so
            // the sea's color tracks the hour. Rendering stays pure: the phase
            // is an explicit input, decided here, never read inside the draw.
            app.phase = Phase::from_hour(Local::now().hour());
            terminal.draw(|frame| ui::draw(app, frame))?;
            next_frame = Instant::now() + FRAME_INTERVAL;
        }

        if Instant::now() >= next_save {
            save::store(save_path, &app.state, unix_now())?;
            next_save = Instant::now() + AUTOSAVE_INTERVAL;
        }
    }
    Ok(())
}
