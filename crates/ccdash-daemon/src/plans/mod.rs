//! Plan markdown parser + per-project file watcher.

pub mod parser;
pub mod watcher;

pub use watcher::Manager;
