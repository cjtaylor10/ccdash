//! Root-directory scanner — discovers candidate git repos under given root dirs.
//! Implementation deferred to a later task in this plan (Task C2).

use std::path::PathBuf;

#[allow(dead_code)]
pub async fn scan(_roots: &[PathBuf]) -> Vec<PathBuf> {
    vec![]
}
