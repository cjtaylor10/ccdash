//! Resolves XDG / HOME-relative paths used across ccdash.

use std::path::PathBuf;

/// Returns the directory where ccdash stores its data files.
/// Honors `CCDASH_HOME` env var for testing; otherwise `~/.ccdash`.
pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CCDASH_HOME") {
        return PathBuf::from(custom);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ccdash")
}

/// Returns the default socket path for the current platform.
/// macOS: `/tmp/ccdash.sock`. Linux: `$XDG_RUNTIME_DIR/ccdash.sock`
/// (falls back to `/tmp/ccdash.sock` if `XDG_RUNTIME_DIR` is unset).
/// Honors `CCDASH_SOCKET` env var for testing.
pub fn default_socket_path() -> PathBuf {
    if let Ok(custom) = std::env::var("CCDASH_SOCKET") {
        return PathBuf::from(custom);
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/tmp/ccdash.sock")
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(xdg).join("ccdash.sock")
        } else {
            PathBuf::from("/tmp/ccdash.sock")
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("/tmp/ccdash.sock")
    }
}

pub fn projects_toml() -> PathBuf {
    data_dir().join("projects.toml")
}

pub fn sessions_toml() -> PathBuf {
    data_dir().join("sessions.toml")
}

pub fn auth_token_path() -> PathBuf {
    data_dir().join("auth")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env-var tests must be serialized — Rust's test runner parallelizes by
    // default but the process env is process-global. Without this, two
    // tests setting/removing the same var race and the assertion sees
    // whatever the other test happened to leave behind.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn data_dir_honors_ccdash_home() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("CCDASH_HOME", "/tmp/ccdash-test");
        assert_eq!(data_dir(), PathBuf::from("/tmp/ccdash-test"));
        std::env::remove_var("CCDASH_HOME");
    }

    #[test]
    fn projects_toml_is_under_data_dir() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("CCDASH_HOME", "/tmp/ccdash-test");
        assert_eq!(
            projects_toml(),
            PathBuf::from("/tmp/ccdash-test/projects.toml")
        );
        std::env::remove_var("CCDASH_HOME");
    }

    #[test]
    fn socket_honors_ccdash_socket() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("CCDASH_SOCKET", "/tmp/explicit.sock");
        assert_eq!(default_socket_path(), PathBuf::from("/tmp/explicit.sock"));
        std::env::remove_var("CCDASH_SOCKET");
    }
}
