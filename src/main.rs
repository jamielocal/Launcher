// No console window in release builds; the launcher is a pure GUI app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Orbit: a game launcher for everything installed on one machine.
//!
//! The pieces fit together like this:
//!
//! - [`discovery`] finds what is installed, by scanning folders ([`scanner`])
//!   and by reading each launcher's own records ([`sources`]).
//! - [`model`] holds what a game is, how it is launched, and the library's
//!   filtering and sorting.
//! - [`app`] owns the running state and is the only place it changes.
//! - [`ui`] draws, and asks for changes by pushing actions back to [`app`].
//! - [`runner`] starts games and measures how long they ran; [`art`] finds their
//!   cover art; [`persist`] saves what should survive a restart.

mod app;
mod art;
mod discovery;
mod model;
mod persist;
mod runner;
mod scanner;
mod sources;
mod theme;
mod ui;

use egui::Vec2;

use crate::app::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Orbit")
            .with_inner_size(Vec2::new(1280.0, 820.0))
            .with_min_inner_size(Vec2::new(880.0, 560.0))
            .with_icon(theme::window_icon()),
        ..Default::default()
    };

    eframe::run_native("Orbit", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
