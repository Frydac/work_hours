#![warn(clippy::all, rust_2018_idioms)]

// Crate entry point. This keeps the public surface small and re-exports the
// eframe app while the internal modules stay free to evolve.
mod app;
pub mod config;
pub mod logging;
pub mod supabase;
mod ui;

pub use app::TemplateApp;
