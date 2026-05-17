//! JSON-RPC client over Unix socket. Used by the CLI and the Tauri UI.

use crate::auth;
use crate::paths;
use crate::protocol::{ClientKind, HandshakeParams, Request, Response, Topic};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub struct Client {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next_id: AtomicU64,
}

impl Client {
    /// Connect to the daemon socket at the given path. Does NOT handshake yet.
    pub async fn connect(socket: &PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(socket)
            .await
            .with_context(|| format!("connecting {}", socket.display()))?;
        let (r, w) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(r),
            writer: w,
            next_id: AtomicU64::new(1),
        })
    }

    /// Connect using the default socket path for this platform.
    pub async fn connect_default() -> Result<Self> {
        Self::connect(&paths::default_socket_path()).await
    }

    /// Send handshake using the token read from `~/.ccdash/auth`.
    pub async fn handshake(&mut self, kind: ClientKind) -> Result<Response> {
        let token = auth::read_token(&paths::auth_token_path())?.ok_or_else(|| {
            anyhow::anyhow!("no auth token at {}", paths::auth_token_path().display())
        })?;
        self.call(
            "handshake",
            serde_json::to_value(HandshakeParams {
                token,
                client: kind,
            })?,
        )
        .await
    }

    /// Subscribe to one or more event topics. Notifications arrive on the same stream
    /// and can be read with `next_notification`.
    pub async fn subscribe(&mut self, topics: Vec<Topic>) -> Result<Response> {
        self.call("subscribe", serde_json::json!({ "topics": topics }))
            .await
    }

    /// Send a JSON-RPC request, wait for the matching response, return it.
    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<Response> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(id),
            method: method.into(),
            params,
        };
        let mut bytes = serde_json::to_vec(&req)?;
        bytes.push(b'\n');
        self.writer
            .write_all(&bytes)
            .await
            .context("writing request")?;
        self.writer.flush().await?;

        // Read frames until we find one whose id matches; intervening frames must be notifications.
        loop {
            let line = self.read_line().await?;
            let v: serde_json::Value = serde_json::from_str(&line)
                .with_context(|| format!("parsing daemon frame: {}", line.trim()))?;
            if v.get("id").is_some() && !v["id"].is_null() {
                let resp: Response = serde_json::from_value(v).context("parsing response")?;
                if resp.id == req.id {
                    return Ok(resp);
                }
            }
            // Else: notification or unrelated id — for simple sync clients we drop it.
        }
    }

    /// Read the next notification (frame with no `id`). Blocks until one arrives.
    pub async fn next_notification(&mut self) -> Result<serde_json::Value> {
        loop {
            let line = self.read_line().await?;
            let v: serde_json::Value = serde_json::from_str(&line)?;
            if v.get("id").is_none() || v["id"].is_null() {
                return Ok(v);
            }
        }
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        let n = self
            .reader
            .read_line(&mut line)
            .await
            .context("reading line")?;
        anyhow::ensure!(n > 0, "daemon closed connection (EOF)");
        Ok(line)
    }
}
