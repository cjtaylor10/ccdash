//! Window position clamping: when a window's position is outside any
//! available monitor's bounds, snap it to the primary monitor's center.
//!
//! macOS restores saved window positions on next launch. If the user
//! disconnects the monitor those coordinates referred to, the window comes
//! back invisible. This module runs on every `WindowEvent::Moved` and at
//! new-window creation.

use tauri::{PhysicalPosition, Runtime, Window};
use tracing::{debug, warn};

pub fn clamp_window_position<R: Runtime>(window: &Window<R>) {
    let pos = match window.outer_position() {
        Ok(p) => p,
        Err(e) => {
            warn!("clamp: outer_position failed: {}", e);
            return;
        }
    };
    let size = match window.outer_size() {
        Ok(s) => s,
        Err(e) => {
            warn!("clamp: outer_size failed: {}", e);
            return;
        }
    };
    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(e) => {
            warn!("clamp: available_monitors failed: {}", e);
            return;
        }
    };

    let any_overlap = monitors.iter().any(|mon| {
        let mp = mon.position();
        let ms = mon.size();
        let mx2 = mp.x + ms.width as i32;
        let my2 = mp.y + ms.height as i32;
        let wx2 = pos.x + size.width as i32;
        let wy2 = pos.y + size.height as i32;
        pos.x < mx2 && wx2 > mp.x && pos.y < my2 && wy2 > mp.y
    });
    if any_overlap {
        return;
    }

    let primary = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next());

    if let Some(mon) = primary {
        let mp = mon.position();
        let ms = mon.size();
        let target_x = mp.x + ((ms.width as i32 - size.width as i32) / 2).max(0);
        let target_y = mp.y + ((ms.height as i32 - size.height as i32) / 2).max(0);
        debug!(
            "clamp: window off-screen, snapping to {}x{}",
            target_x, target_y
        );
        if let Err(e) = window.set_position(PhysicalPosition::new(target_x, target_y)) {
            warn!("clamp: set_position failed: {}", e);
        }
    }
}
