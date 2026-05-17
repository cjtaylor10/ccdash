//! Shared test harness — spawns the daemon binary, returns a connection handle.

use anyhow::{Context, Result};
use ccdash_core::protocol::{Request, Response};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};

#[allow(dead_code)]
pub struct Harness {
    pub dir: TempDir,
    pub socket: PathBuf,
    pub token: String,
    child: Child,
}

impl Harness {
    pub async fn spawn() -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let socket = dir.path().join("ccdash.sock");
        let data_dir = dir.path().join("home");
        std::fs::create_dir_all(&data_dir)?;

        // Cargo sets CARGO_BIN_EXE_<bin-name> for tests in the same crate.
        let bin = PathBuf::from(env!("CARGO_BIN_EXE_ccdash-daemon"));
        assert!(bin.exists(), "daemon binary not found at {}", bin.display());

        let child = Command::new(&bin)
            .arg("--socket").arg(&socket)
            .arg("--data-dir").arg(&data_dir)
            .arg("--log-level").arg("warn")
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("spawning {}", bin.display()))?;

        // Wait up to 3s for socket to appear.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !socket.exists() && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        anyhow::ensure!(socket.exists(), "daemon did not create socket within 3s");

        let token = tokio::fs::read_to_string(data_dir.join("auth")).await?.trim().to_string();
        Ok(Self { dir, socket, token, child })
    }

    pub async fn connect(&self) -> Result<Conn> {
        let stream = UnixStream::connect(&self.socket).await.context("connecting socket")?;
        let (r, w) = stream.into_split();
        Ok(Conn { reader: BufReader::new(r), writer: w, next_id: 1 })
    }

    pub async fn handshake(&self, conn: &mut Conn) -> Result<Response> {
        conn.call("handshake", serde_json::json!({"token": self.token, "client": "cli"})).await
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[allow(dead_code)]
pub struct Conn {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next_id: u64,
}

impl Conn {
    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<Response> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(id),
            method: method.into(),
            params,
        };
        let mut bytes = serde_json::to_vec(&req)?;
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        anyhow::ensure!(n > 0, "EOF before response");
        Ok(serde_json::from_str(line.trim_end())?)
    }

    /// Test-only accessor for reading the next raw line (e.g. an unsolicited notification).
    #[allow(dead_code)]
    pub fn reader(&mut self) -> &mut BufReader<tokio::net::unix::OwnedReadHalf> {
        &mut self.reader
    }
}
