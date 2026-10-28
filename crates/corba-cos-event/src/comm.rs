// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosEventComm — spec §1.5.
//!
//! Four trait definitions for the push/pull model:
//!
//! | Mode | Initiator | Trait | Counterpart |
//! |---|---|---|---|
//! | Push | Supplier | PushConsumer | PushSupplier |
//! | Pull | Consumer | PullSupplier | PullConsumer |
//!
//! In the push model the supplier pushes events; in the pull model
//! the consumer pulls events. Both endpoints can be disconnected.

use alloc::vec::Vec;

/// Opaque event container — spec §1.4: `any` as a CDR encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnyEvent {
    /// Repository ID of the event type (or empty for "any").
    pub type_id: alloc::string::String,
    /// CDR encapsulation of the event body.
    pub data: Vec<u8>,
}

impl AnyEvent {
    /// Constructor.
    #[must_use]
    pub fn new(type_id: alloc::string::String, data: Vec<u8>) -> Self {
        Self { type_id, data }
    }
}

/// `Disconnected` — spec §1.5.1 normative: when a push/pull endpoint
/// is disconnected, all operations throw `Disconnected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// `ConnectError` (spec §1.6 normative "AlreadyConnected" + "TypeError").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError {
    /// Endpoint is already connected (spec §1.6.1.1).
    AlreadyConnected,
    /// Type mismatch between consumer and supplier (spec §1.6.1.2).
    TypeError,
}

/// PushConsumer — spec §1.5.1.
pub trait PushConsumer: Send + Sync {
    /// `push(any)` — the supplier sends an event.
    ///
    /// # Errors
    /// `Disconnected` if the consumer is already disconnected.
    fn push(&self, event: AnyEvent) -> Result<(), Disconnected>;

    /// `disconnect_push_consumer` — irreversible terminal state.
    fn disconnect_push_consumer(&self);
}

/// PushSupplier — spec §1.5.2.
pub trait PushSupplier: Send + Sync {
    /// `disconnect_push_supplier` — spec §1.5.2.
    fn disconnect_push_supplier(&self);
}

/// PullConsumer — spec §1.5.3.
pub trait PullConsumer: Send + Sync {
    /// `disconnect_pull_consumer` — spec §1.5.3.
    fn disconnect_pull_consumer(&self);
}

/// PullSupplier — spec §1.5.4.
pub trait PullSupplier: Send + Sync {
    /// `pull` — blocking, returns the next event.
    ///
    /// # Errors
    /// `Disconnected` after disconnect.
    fn pull(&self) -> Result<AnyEvent, Disconnected>;

    /// `try_pull` — non-blocking. Returns `(event, has_event)`.
    ///
    /// # Errors
    /// `Disconnected`.
    fn try_pull(&self) -> Result<(AnyEvent, bool), Disconnected>;

    /// `disconnect_pull_supplier`.
    fn disconnect_pull_supplier(&self);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct MemoryPushConsumer {
        received: alloc::sync::Arc<core::sync::atomic::AtomicUsize>,
        connected: AtomicBool,
    }

    impl PushConsumer for MemoryPushConsumer {
        fn push(&self, _event: AnyEvent) -> Result<(), Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.received.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn disconnect_push_consumer(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }

    #[test]
    fn push_increments_counter_until_disconnect() {
        let count = alloc::sync::Arc::new(AtomicUsize::new(0));
        let c = MemoryPushConsumer {
            received: alloc::sync::Arc::clone(&count),
            connected: AtomicBool::new(true),
        };
        for _ in 0..3 {
            c.push(AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![0]))
                .unwrap();
        }
        assert_eq!(count.load(Ordering::Relaxed), 3);
        c.disconnect_push_consumer();
        assert_eq!(
            c.push(AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
    }

    #[test]
    fn any_event_round_trip() {
        let e = AnyEvent::new("IDL:demo/Tick:1.0".into(), alloc::vec![1, 2, 3]);
        assert_eq!(e.type_id, "IDL:demo/Tick:1.0");
        assert_eq!(e.data, alloc::vec![1, 2, 3]);
    }

    #[test]
    fn connect_error_variants_distinct() {
        assert_ne!(ConnectError::AlreadyConnected, ConnectError::TypeError);
    }

    // -----------------------------------------------------------------
    // §2.1.2 PushSupplier
    // -----------------------------------------------------------------

    /// A `PushSupplier` that pushes into a connected consumer until its
    /// own `disconnect_push_supplier` terminates the connection.
    struct MemoryPushSupplier {
        consumer: alloc::sync::Arc<MemoryPushConsumer>,
        connected: AtomicBool,
    }

    impl MemoryPushSupplier {
        fn emit(&self) -> Result<(), Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.consumer
                .push(AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![1]))
        }
    }

    impl PushSupplier for MemoryPushSupplier {
        fn disconnect_push_supplier(&self) {
            self.connected.store(false, Ordering::Release);
            // Spec §2.1.2: the supplier releases the connection and
            // disconnects the peer consumer.
            self.consumer.disconnect_push_consumer();
        }
    }

    /// Spec §2.1.2: `disconnect_push_supplier` terminates the event
    /// communication — the supplier stops emitting and the connected
    /// consumer is disconnected too.
    #[test]
    fn push_supplier_disconnect_terminates_the_connection() {
        let count = alloc::sync::Arc::new(AtomicUsize::new(0));
        let consumer = alloc::sync::Arc::new(MemoryPushConsumer {
            received: alloc::sync::Arc::clone(&count),
            connected: AtomicBool::new(true),
        });
        let supplier = MemoryPushSupplier {
            consumer: alloc::sync::Arc::clone(&consumer),
            connected: AtomicBool::new(true),
        };

        supplier.emit().unwrap();
        assert_eq!(count.load(Ordering::Relaxed), 1);

        supplier.disconnect_push_supplier();
        assert_eq!(supplier.emit().unwrap_err(), Disconnected);
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert_eq!(
            consumer
                .push(AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
    }

    // -----------------------------------------------------------------
    // §2.1.3 PullSupplier / §2.1.4 PullConsumer
    // -----------------------------------------------------------------

    struct MemoryPullSupplier {
        queue: std::sync::Mutex<alloc::collections::VecDeque<AnyEvent>>,
        connected: AtomicBool,
    }

    impl MemoryPullSupplier {
        fn with(payloads: &[u8]) -> Self {
            Self {
                queue: std::sync::Mutex::new(
                    payloads
                        .iter()
                        .map(|b| AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![*b]))
                        .collect(),
                ),
                connected: AtomicBool::new(true),
            }
        }
    }

    impl PullSupplier for MemoryPullSupplier {
        fn pull(&self) -> Result<AnyEvent, Disconnected> {
            loop {
                let (event, has) = self.try_pull()?;
                if has {
                    return Ok(event);
                }
                std::thread::yield_now();
            }
        }

        fn try_pull(&self) -> Result<(AnyEvent, bool), Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            let mut q = self.queue.lock().map_err(|_| Disconnected)?;
            Ok(match q.pop_front() {
                Some(e) => (e, true),
                None => (
                    AnyEvent::new(alloc::string::String::new(), alloc::vec![]),
                    false,
                ),
            })
        }

        fn disconnect_pull_supplier(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }

    /// Spec §2.1.3: `try_pull` never blocks — it returns the event
    /// together with the `has_event` flag; on an empty queue the flag is
    /// `false` and the returned `any` carries no data.
    #[test]
    fn pull_supplier_try_pull_returns_the_has_event_flag() {
        let s = MemoryPullSupplier::with(&[4]);
        let (event, has) = s.try_pull().unwrap();
        assert!(has);
        assert_eq!(event.data, alloc::vec![4]);

        let (empty, has) = s.try_pull().unwrap();
        assert!(!has);
        assert!(empty.data.is_empty());
    }

    /// Spec §2.1.3: `pull` blocks until an event is available; it
    /// returns the queued event, not the empty placeholder.
    #[test]
    fn pull_supplier_pull_returns_the_queued_event() {
        let s = MemoryPullSupplier::with(&[6]);
        assert_eq!(s.pull().unwrap().data, alloc::vec![6]);
    }

    /// Spec §2.1.3 + §2.1.5: after `disconnect_pull_supplier` both
    /// `pull` and `try_pull` raise `Disconnected`.
    #[test]
    fn pull_supplier_disconnect_raises_disconnected_on_both_operations() {
        let s = MemoryPullSupplier::with(&[1]);
        s.disconnect_pull_supplier();
        assert_eq!(s.try_pull().unwrap_err(), Disconnected);
        assert_eq!(s.pull().unwrap_err(), Disconnected);
    }

    /// A `PullConsumer` that drives a connected `PullSupplier` until its
    /// own `disconnect_pull_consumer` ends the communication.
    struct MemoryPullConsumer {
        supplier: alloc::sync::Arc<MemoryPullSupplier>,
        connected: AtomicBool,
    }

    impl MemoryPullConsumer {
        fn drain_one(&self) -> Result<(AnyEvent, bool), Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.supplier.try_pull()
        }
    }

    impl PullConsumer for MemoryPullConsumer {
        fn disconnect_pull_consumer(&self) {
            self.connected.store(false, Ordering::Release);
            // Spec §2.1.4: the consumer disconnects the peer supplier.
            self.supplier.disconnect_pull_supplier();
        }
    }

    /// Spec §2.1.4: `disconnect_pull_consumer` terminates the event
    /// communication — the consumer stops pulling and the connected
    /// supplier is disconnected too.
    #[test]
    fn pull_consumer_disconnect_terminates_the_connection() {
        let supplier = alloc::sync::Arc::new(MemoryPullSupplier::with(&[1, 2]));
        let consumer = MemoryPullConsumer {
            supplier: alloc::sync::Arc::clone(&supplier),
            connected: AtomicBool::new(true),
        };

        let (event, has) = consumer.drain_one().unwrap();
        assert!(has);
        assert_eq!(event.data, alloc::vec![1]);

        consumer.disconnect_pull_consumer();
        assert_eq!(consumer.drain_one().unwrap_err(), Disconnected);
        assert_eq!(supplier.try_pull().unwrap_err(), Disconnected);
    }

    // -----------------------------------------------------------------
    // §2.1.5 Disconnection Behavior
    // -----------------------------------------------------------------

    /// Spec §2.1.5: disconnection is terminal and applies to every
    /// operation of the disconnected object — `push` on a disconnected
    /// `PushConsumer` and `pull`/`try_pull` on a disconnected
    /// `PullSupplier` all raise `Disconnected`, and a repeated
    /// disconnect stays in that state (it is not an error and does not
    /// reconnect).
    #[test]
    fn disconnection_is_terminal_for_every_operation() {
        let count = alloc::sync::Arc::new(AtomicUsize::new(0));
        let consumer = MemoryPushConsumer {
            received: alloc::sync::Arc::clone(&count),
            connected: AtomicBool::new(true),
        };
        consumer.disconnect_push_consumer();
        consumer.disconnect_push_consumer();
        assert_eq!(
            consumer
                .push(AnyEvent::new("IDL:demo/E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
        assert_eq!(count.load(Ordering::Relaxed), 0);

        let supplier = MemoryPullSupplier::with(&[1]);
        supplier.disconnect_pull_supplier();
        supplier.disconnect_pull_supplier();
        assert_eq!(supplier.pull().unwrap_err(), Disconnected);
        assert_eq!(supplier.try_pull().unwrap_err(), Disconnected);
    }
}
