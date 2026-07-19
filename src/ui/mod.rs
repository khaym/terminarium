//! Rendering. The wallpaper draws the tank as pure decoration; the game layer
//! adds the HUD. Which one is on screen is the app's layer decision.

pub mod game;
pub mod wallpaper;

use ratatui::Frame;

use crate::app::{App, Layer};

pub fn draw(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    match app.layer {
        Layer::Wallpaper => wallpaper::render(app, area, buf),
        Layer::Game => game::render(app, area, buf),
    }
}
