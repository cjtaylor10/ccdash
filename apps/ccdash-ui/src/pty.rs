//! Per-terminal pty manager.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{debug, warn};

pub struct PtyHandle {
    #[allow(dead_code)]
    pub id: String,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    pub child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

pub struct PtyManager {
    pub handles: Mutex<HashMap<String, PtyHandle>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }

    pub async fn open(
        &self,
        app: AppHandle,
        cmd: Vec<String>,
        rows: u16,
        cols: u16,
    ) -> Result<String, String> {
        if cmd.is_empty() {
            return Err("empty command".into());
        }
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut builder = CommandBuilder::new(&cmd[0]);
        for arg in &cmd[1..] {
            builder.arg(arg);
        }
        builder.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| format!("spawn: {}", e))?;
        drop(pair.slave);

        let id = uuid::Uuid::new_v4().to_string();
        let master = Arc::new(Mutex::new(pair.master));

        // take_writer needs to happen before we move master.
        let writer = {
            let master_guard = master.lock().await;
            let w = master_guard
                .take_writer()
                .map_err(|e| format!("take_writer: {}", e))?;
            Arc::new(Mutex::new(w))
        };

        // Get a reader clone now (outside the std thread spawn).
        let reader_handle = {
            let master_guard = master.lock().await;
            master_guard
                .try_clone_reader()
                .map_err(|e| format!("try_clone_reader: {}", e))?
        };
        let reader_id = id.clone();
        let reader_app = app.clone();
        std::thread::spawn(move || {
            run_reader_loop(reader_handle, reader_id, reader_app);
        });

        let handle = PtyHandle {
            id: id.clone(),
            writer,
            master,
            child: Arc::new(Mutex::new(child)),
        };
        self.handles.lock().await.insert(id.clone(), handle);
        Ok(id)
    }

    pub async fn write(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        let handles = self.handles.lock().await;
        let h = handles
            .get(id)
            .ok_or_else(|| format!("no such terminal: {}", id))?;
        let mut w = h.writer.lock().await;
        w.write_all(bytes).map_err(|e| format!("write: {}", e))?;
        w.flush().map_err(|e| format!("flush: {}", e))?;
        Ok(())
    }

    pub async fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<(), String> {
        let handles = self.handles.lock().await;
        let h = handles
            .get(id)
            .ok_or_else(|| format!("no such terminal: {}", id))?;
        let m = h.master.lock().await;
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize: {}", e))?;
        Ok(())
    }

    pub async fn close(&self, id: &str) -> Result<(), String> {
        let mut handles = self.handles.lock().await;
        if let Some(h) = handles.remove(id) {
            let mut child = h.child.lock().await;
            let _ = child.kill();
        }
        Ok(())
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

fn run_reader_loop(mut reader: Box<dyn Read + Send>, id: String, app: AppHandle) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!(id = %id, "pty eof");
                let _ = app.emit(&format!("terminal-eof::{}", id), serde_json::json!({}));
                return;
            }
            Ok(n) => {
                let v: Vec<u8> = buf[..n].to_vec();
                if let Err(e) = app.emit(&format!("terminal-output::{}", id), v) {
                    warn!("emit terminal-output failed: {}", e);
                    return;
                }
            }
            Err(e) => {
                warn!(id = %id, error = %e, "pty read error");
                let _ = app.emit(&format!("terminal-eof::{}", id), serde_json::json!({}));
                return;
            }
        }
    }
}
