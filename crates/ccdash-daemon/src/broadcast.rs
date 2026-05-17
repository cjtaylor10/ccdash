//! Internal event bus shared across daemon modules.
//! Each subscribed client connection receives a tokio broadcast Receiver.

use ccdash_core::domain::{Project, Session};
use ccdash_core::protocol::Topic;
use serde::Serialize;
use tokio::sync::broadcast;

/// Channel buffer. If a slow client lags this many events behind, it gets
/// `RecvError::Lagged` on next recv and we drop the slowest event.
/// 128 is enough for short bursts; sustained lag triggers reconciliation.
const BUFFER: usize = 128;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)] // some variants published only from Phase 2+ (snapshots, session_updated)
pub enum Event {
    /// Full snapshot — sent to clients on subscribe.
    ProjectsSnapshot {
        projects: Vec<Project>,
    },
    SessionsSnapshot {
        sessions: Vec<Session>,
    },
    /// Project added/removed/updated. Carries full updated project.
    ProjectUpdated {
        project: Project,
    },
    ProjectRemoved {
        id: ccdash_core::domain::ProjectId,
    },
    /// Session lifecycle.
    SessionLaunched {
        session: Session,
    },
    SessionUpdated {
        session: Session,
    },
    SessionRemoved {
        tmux_session_id: String,
    },
}

impl Event {
    /// Which subscription topic this event belongs to.
    pub fn topic(&self) -> Topic {
        match self {
            Event::ProjectsSnapshot { .. }
            | Event::ProjectUpdated { .. }
            | Event::ProjectRemoved { .. } => Topic::Projects,
            Event::SessionsSnapshot { .. }
            | Event::SessionLaunched { .. }
            | Event::SessionUpdated { .. }
            | Event::SessionRemoved { .. } => Topic::Sessions,
        }
    }
}

/// Bus handle — cheap to clone; backed by a single broadcast::Sender.
#[derive(Clone)]
pub struct Bus {
    tx: broadcast::Sender<Event>,
}

impl Bus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(BUFFER);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        // Send fails only if there are zero receivers — that's fine, log it at trace.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccdash_core::domain::ProjectId;

    #[tokio::test]
    async fn subscriber_receives_published_event() {
        let bus = Bus::new();
        let mut rx = bus.subscribe();
        bus.publish(Event::ProjectRemoved {
            id: ProjectId("abc".into()),
        });
        let evt = rx.recv().await.unwrap();
        match evt {
            Event::ProjectRemoved { id } => assert_eq!(id.0, "abc"),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn topic_classification() {
        let evt = Event::SessionRemoved {
            tmux_session_id: "$1".into(),
        };
        assert_eq!(evt.topic(), Topic::Sessions);
        let evt = Event::ProjectRemoved {
            id: ProjectId("a".into()),
        };
        assert_eq!(evt.topic(), Topic::Projects);
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_does_not_panic() {
        let bus = Bus::new();
        bus.publish(Event::SessionRemoved {
            tmux_session_id: "$0".into(),
        });
    }
}
