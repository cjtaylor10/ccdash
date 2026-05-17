//! Daemon configuration: CLI args + env vars.

use ccdash_core::paths;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(name = "ccdash-daemon", version, about = "ccdash background service")]
pub struct Config {
    /// Path to the Unix socket the daemon listens on.
    #[arg(long, env = "CCDASH_SOCKET")]
    pub socket: Option<PathBuf>,

    /// Directory for state files (projects.toml, sessions.toml, auth).
    #[arg(long, env = "CCDASH_HOME")]
    pub data_dir: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error). Default: info.
    #[arg(long, env = "CCDASH_LOG", default_value = "info")]
    pub log_level: String,
}

impl Config {
    pub fn resolved_socket(&self) -> PathBuf {
        self.socket
            .clone()
            .unwrap_or_else(paths::default_socket_path)
    }
    pub fn resolved_data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(paths::data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn defaults_when_no_flags() {
        let c = Config::parse_from(["ccdash-daemon"]);
        assert!(c.socket.is_none());
        assert!(c.data_dir.is_none());
        assert_eq!(c.log_level, "info");
    }

    #[test]
    fn flags_override() {
        let c = Config::parse_from([
            "ccdash-daemon",
            "--socket",
            "/tmp/x.sock",
            "--data-dir",
            "/tmp/x",
            "--log-level",
            "debug",
        ]);
        assert_eq!(c.socket, Some(PathBuf::from("/tmp/x.sock")));
        assert_eq!(c.data_dir, Some(PathBuf::from("/tmp/x")));
        assert_eq!(c.log_level, "debug");
    }
}
