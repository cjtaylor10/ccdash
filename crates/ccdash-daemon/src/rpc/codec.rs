//! Line-delimited JSON-RPC framing. Each frame is a single JSON object
//! terminated by `\n`. Frames over 1 MiB are rejected to prevent DoS.

use anyhow::{Context, Result};
use ccdash_core::protocol::{Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::unix::OwnedWriteHalf;

const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub struct FrameReader {
    inner: BufReader<OwnedReadHalf>,
    buf: String,
}

impl FrameReader {
    pub fn new(half: OwnedReadHalf) -> Self {
        Self {
            inner: BufReader::new(half),
            buf: String::with_capacity(1024),
        }
    }

    /// Read one JSON-RPC request frame. Returns `Ok(None)` on EOF.
    pub async fn next_request(&mut self) -> Result<Option<Request>> {
        self.buf.clear();
        let n = self
            .inner
            .read_line(&mut self.buf)
            .await
            .context("reading frame")?;
        if n == 0 {
            return Ok(None);
        }
        if self.buf.len() > MAX_FRAME_BYTES {
            anyhow::bail!("frame exceeds {} bytes", MAX_FRAME_BYTES);
        }
        let req: Request =
            serde_json::from_str(self.buf.trim_end()).context("parsing JSON-RPC request")?;
        Ok(Some(req))
    }
}

pub struct FrameWriter {
    inner: OwnedWriteHalf,
}

impl FrameWriter {
    pub fn new(half: OwnedWriteHalf) -> Self {
        Self { inner: half }
    }

    pub async fn write_response(&mut self, resp: &Response) -> Result<()> {
        let mut bytes = serde_json::to_vec(resp).context("serializing response")?;
        bytes.push(b'\n');
        self.inner
            .write_all(&bytes)
            .await
            .context("writing response")?;
        self.inner.flush().await.context("flushing")?;
        Ok(())
    }

    pub async fn write_notification(
        &mut self,
        n: &ccdash_core::protocol::Notification,
    ) -> Result<()> {
        let mut bytes = serde_json::to_vec(n).context("serializing notification")?;
        bytes.push(b'\n');
        self.inner.write_all(&bytes).await?;
        self.inner.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn round_trip_one_request() {
        let (a, b) = UnixStream::pair().unwrap();
        let (a_r, a_w) = a.into_split();
        let (_b_r, mut b_w) = b.into_split();

        // Send a request from b to a.
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "handshake".into(),
            params: serde_json::json!({"token": "x", "client": "cli"}),
        };
        let mut bytes = serde_json::to_vec(&req).unwrap();
        bytes.push(b'\n');
        b_w.write_all(&bytes).await.unwrap();
        drop(b_w);

        let mut reader = FrameReader::new(a_r);
        let got = reader.next_request().await.unwrap().unwrap();
        assert_eq!(got.method, "handshake");
        assert!(
            reader.next_request().await.unwrap().is_none(),
            "EOF expected"
        );
        let _ = a_w; // keep alive
    }
}
