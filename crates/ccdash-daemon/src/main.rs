//! ccdash daemon entry point.

mod broadcast;
mod config;
mod projects;
mod sessions;
mod tmux;
mod worktrees;

use clap::Parser;

fn main() {
    let cfg = config::Config::parse();
    println!(
        "ccdash-daemon {} socket={} data_dir={} log={}",
        env!("CARGO_PKG_VERSION"),
        cfg.resolved_socket().display(),
        cfg.resolved_data_dir().display(),
        cfg.log_level,
    );
}
