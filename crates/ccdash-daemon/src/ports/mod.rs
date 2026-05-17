//! Port discovery: running (via `lsof`) + declared (via per-project parsers).

pub mod declared;
pub mod lsof;
pub mod registry;

pub use registry::Registry;
