//! ccdash shared library.
//!
//! Contains protocol types, domain types, auth, and path resolution
//! shared between the daemon and any client (CLI, Tauri UI).

pub mod auth;
pub mod domain;
pub mod paths;
pub mod protocol;
