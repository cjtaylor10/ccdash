//! Shared-secret auth tokens.
//!
//! The daemon writes a random token to `~/.ccdash/auth` with mode 0600.
//! Clients read it and present it in the JSON-RPC handshake.

use anyhow::{Context, Result};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Length of generated tokens in bytes (32 = 256 bits -> 64 hex chars).
const TOKEN_BYTES: usize = 32;

/// Generate a new random token as a lowercase hex string.
pub fn generate_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Read the auth token from the given path. Returns `Ok(None)` if the file
/// doesn't exist; `Err` for any other failure.
pub fn read_token(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Write the given token to the path with mode 0600 (Unix). Creates the parent
/// directory if missing.
#[cfg(unix)]
pub fn write_token(path: &Path, token: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    f.write_all(token.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

/// Ensure an auth token exists at the given path. If absent, generates and
/// writes one. Returns the resulting token.
pub fn ensure_token(path: &Path) -> Result<String> {
    if let Some(existing) = read_token(path)? {
        return Ok(existing);
    }
    let token = generate_token();
    write_token(path, &token)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generate_token_returns_64_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn read_missing_returns_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing");
        assert!(read_token(&path).unwrap().is_none());
    }

    #[test]
    fn ensure_creates_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth");
        let token = ensure_token(&path).unwrap();
        assert_eq!(token.len(), 64);
        let again = ensure_token(&path).unwrap();
        assert_eq!(token, again, "second call must return same token");
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth");
        write_token(&path, "abc").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
