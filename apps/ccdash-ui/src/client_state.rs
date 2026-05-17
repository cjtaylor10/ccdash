//! Wraps an optional `ccdash_core::client::Client` behind a tokio mutex
//! so all Tauri commands can share one connection.

use ccdash_core::client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ClientState {
    pub inner: Arc<Mutex<Option<Client>>>,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
