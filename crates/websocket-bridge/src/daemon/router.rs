// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Topic router for the `zerodds-ws-bridged` daemon.
//!
//! The router holds:
//! * A list of subscribed connection IDs per topic.
//! * A sender map `connection_id → mpsc::Sender<RouterMsg>` so the
//!   DDS pump thread can notify the right connection writer threads.
//!
//! The router is `Send + Sync` and can be consumed via `Arc<Mutex<...>>`
//! by all worker threads.

use std::collections::BTreeMap;
use std::string::String;
use std::sync::mpsc;
use std::vec::Vec;

/// Message to a connection writer thread.
#[derive(Debug, Clone)]
pub enum RouterMsg {
    /// DDS sample on a topic — push as a WS frame.
    Sample {
        /// DDS topic name.
        topic: String,
        /// CDR payload (without the encap header) or JSON representation.
        payload: Vec<u8>,
    },
    /// Daemon shutdown — close the connection with code 1001.
    Shutdown,
}

/// Router-State.
#[derive(Debug, Default)]
pub struct Router {
    /// `topic → list-of-connection-ids`.
    subs: BTreeMap<String, Vec<u64>>,
    /// `connection-id → sender-channel`.
    conns: BTreeMap<u64, mpsc::Sender<RouterMsg>>,
}

impl Router {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new connection.
    pub fn register_connection(&mut self, id: u64, sender: mpsc::Sender<RouterMsg>) {
        self.conns.insert(id, sender);
    }

    /// Removes a connection (cleanup on disconnect).
    pub fn deregister_connection(&mut self, id: u64) {
        self.conns.remove(&id);
        for subs in self.subs.values_mut() {
            subs.retain(|c| *c != id);
        }
    }

    /// Subscribes a connection to a topic.
    pub fn subscribe(&mut self, conn_id: u64, topic: String) {
        let entry = self.subs.entry(topic).or_default();
        if !entry.contains(&conn_id) {
            entry.push(conn_id);
        }
    }

    /// Unsubscribe.
    pub fn unsubscribe(&mut self, conn_id: u64, topic: &str) {
        if let Some(list) = self.subs.get_mut(topic) {
            list.retain(|c| *c != conn_id);
        }
    }

    /// Pushes a sample to all subscribers.
    /// Connections whose channel is `disconnected` are automatically
    /// removed (lazy cleanup).
    pub fn dispatch(&mut self, topic: &str, payload: Vec<u8>) -> usize {
        let Some(subs) = self.subs.get(topic).cloned() else {
            return 0;
        };
        let mut delivered = 0usize;
        for conn_id in subs {
            if let Some(sender) = self.conns.get(&conn_id) {
                let msg = RouterMsg::Sample {
                    topic: topic.to_string(),
                    payload: payload.clone(),
                };
                if sender.send(msg).is_ok() {
                    delivered += 1;
                } else {
                    // Receiver dead — mark for cleanup.
                    self.conns.remove(&conn_id);
                }
            }
        }
        delivered
    }

    /// Sends `Shutdown` to all connections (graceful drain).
    pub fn broadcast_shutdown(&self) {
        for sender in self.conns.values() {
            let _ = sender.send(RouterMsg::Shutdown);
        }
    }

    /// Number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn dispatch_to_subscribed_connection() {
        let mut router = Router::new();
        let (tx, rx) = channel();
        router.register_connection(1, tx);
        router.subscribe(1, "Trade".to_string());
        let n = router.dispatch("Trade", b"PAYLOAD".to_vec());
        assert_eq!(n, 1);
        match rx.recv().unwrap() {
            RouterMsg::Sample { topic, payload } => {
                assert_eq!(topic, "Trade");
                assert_eq!(payload, b"PAYLOAD");
            }
            other => panic!("unexpected msg {other:?}"),
        }
    }

    #[test]
    fn dispatch_to_no_subscribers_is_zero() {
        let mut router = Router::new();
        let n = router.dispatch("Empty", b"x".to_vec());
        assert_eq!(n, 0);
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let mut router = Router::new();
        let (tx, rx) = channel();
        router.register_connection(2, tx);
        router.subscribe(2, "T".to_string());
        router.unsubscribe(2, "T");
        let n = router.dispatch("T", b"x".to_vec());
        assert_eq!(n, 0);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn deregister_removes_subscription() {
        let mut router = Router::new();
        let (tx, _rx) = channel();
        router.register_connection(3, tx);
        router.subscribe(3, "T".to_string());
        router.deregister_connection(3);
        assert_eq!(router.connection_count(), 0);
    }

    #[test]
    fn shutdown_broadcasts_to_all() {
        let mut router = Router::new();
        let (tx, rx) = channel();
        router.register_connection(7, tx);
        router.broadcast_shutdown();
        assert!(matches!(rx.recv().unwrap(), RouterMsg::Shutdown));
    }
}
