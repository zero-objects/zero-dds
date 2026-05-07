// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosEventComm — Spec §1.5.
//!
//! Vier Trait-Definitions fuer das Push/Pull-Modell:
//!
//! | Mode | Initiator | Trait | Counterpart |
//! |---|---|---|---|
//! | Push | Supplier | PushConsumer | PushSupplier |
//! | Pull | Consumer | PullSupplier | PullConsumer |
//!
//! Im Push-Modell pusht der Supplier Events; im Pull-Modell zieht
//! der Consumer Events. Beide Endpunkte koennen disconnected werden.

use alloc::vec::Vec;

/// Opaque-Event-Container — Spec §1.4: `any` als CDR-Encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnyEvent {
    /// Repository-ID des Event-Types (oder leer fuer "any").
    pub type_id: alloc::string::String,
    /// CDR-Encapsulation des Event-Body.
    pub data: Vec<u8>,
}

impl AnyEvent {
    /// Konstruktor.
    #[must_use]
    pub fn new(type_id: alloc::string::String, data: Vec<u8>) -> Self {
        Self { type_id, data }
    }
}

/// `Disconnected` — Spec §1.5.1 normativ: wenn ein Push-/Pull-Endpoint
/// disconnected ist, werfen alle Operations `Disconnected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// `ConnectError` (Spec §1.6 normativ "AlreadyConnected" + "TypeError").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError {
    /// Endpoint ist bereits verbunden (Spec §1.6.1.1).
    AlreadyConnected,
    /// Type-Mismatch zwischen Consumer und Supplier (Spec §1.6.1.2).
    TypeError,
}

/// PushConsumer — Spec §1.5.1.
pub trait PushConsumer: Send + Sync {
    /// `push(any)` — Supplier sendet ein Event.
    ///
    /// # Errors
    /// `Disconnected` wenn der Consumer bereits disconnected ist.
    fn push(&self, event: AnyEvent) -> Result<(), Disconnected>;

    /// `disconnect_push_consumer` — irreversibler Endzustand.
    fn disconnect_push_consumer(&self);
}

/// PushSupplier — Spec §1.5.2.
pub trait PushSupplier: Send + Sync {
    /// `disconnect_push_supplier` — Spec §1.5.2.
    fn disconnect_push_supplier(&self);
}

/// PullConsumer — Spec §1.5.3.
pub trait PullConsumer: Send + Sync {
    /// `disconnect_pull_consumer` — Spec §1.5.3.
    fn disconnect_pull_consumer(&self);
}

/// PullSupplier — Spec §1.5.4.
pub trait PullSupplier: Send + Sync {
    /// `pull` — blockierend, liefert das naechste Event.
    ///
    /// # Errors
    /// `Disconnected` nach Disconnect.
    fn pull(&self) -> Result<AnyEvent, Disconnected>;

    /// `try_pull` — non-blocking. Liefert `(event, has_event)`.
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
}
