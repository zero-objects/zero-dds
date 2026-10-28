// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosTypedEventComm + CosTypedEventChannelAdmin — spec §2.
//!
//! In typed mode events have a fixed IDL interface type and the
//! channel calls the operations directly on the consumer (instead of
//! `push(any)`). The `get_typed_consumer`/`get_typed_supplier`
//! operations return object references of the IDL interface type.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use crate::comm::Disconnected;

/// Typed operation argument form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedInvocation {
    /// Operation name (wire form).
    pub operation: String,
    /// CDR-encoded arguments.
    pub args: Vec<u8>,
}

/// TypedPushConsumer trait — spec §2.4.
pub trait TypedPushConsumer: Send + Sync {
    /// Generic dispatch for an operation call.
    ///
    /// # Errors
    /// `Disconnected`.
    fn invoke(&self, op: &TypedInvocation) -> Result<Vec<u8>, Disconnected>;

    /// `disconnect_push_consumer`.
    fn disconnect(&self);
}

/// TypedPushSupplier trait — spec §2.5.
pub trait TypedPushSupplier: Send + Sync {
    /// `disconnect_push_supplier`.
    fn disconnect(&self);
}

/// TypedPullSupplier trait — spec §2.5.2.
///
/// In pull mode the consumer actively pulls operations from the
/// supplier; `try_pull` returns immediately (or an empty result),
/// `pull` blocks until an event arrives.
pub trait TypedPullSupplier: Send + Sync {
    /// `pull` — blocks until an event is available; then returns the
    /// CDR-encoded operation args.
    ///
    /// # Errors
    /// `Disconnected`.
    fn pull(&self) -> Result<TypedInvocation, Disconnected>;

    /// `try_pull` — non-blocking. `Ok(Some(_))` if an event was
    /// available, `Ok(None)` for an empty queue.
    ///
    /// # Errors
    /// `Disconnected`.
    fn try_pull(&self) -> Result<Option<TypedInvocation>, Disconnected>;

    /// `disconnect_pull_supplier`.
    fn disconnect(&self);
}

// ---------------------------------------------------------------------------
// §2.7.2-§2.7.5 Typed Admin + Proxy Interfaces
// ---------------------------------------------------------------------------

/// Spec §2.7.2 — TypedConsumerAdmin interface.
///
/// The TypedConsumerAdmin manages the subscriptions of the
/// push consumers for a typed EventChannel.
#[derive(Default)]
pub struct TypedConsumerAdmin {
    consumers: Mutex<BTreeMap<String, Vec<Arc<dyn TypedPushConsumer>>>>,
}

impl core::fmt::Debug for TypedConsumerAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedConsumerAdmin").finish()
    }
}

impl TypedConsumerAdmin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §2.7.4 — `obtain_typed_pull_supplier(repo_id)`.
    /// Here we provide the sub-slot for pull suppliers (the caller
    /// implements the `TypedPullSupplier` trait).
    pub fn register_consumer(&self, repo_id: &str, c: Arc<dyn TypedPushConsumer>) {
        if let Ok(mut g) = self.consumers.lock() {
            g.entry(repo_id.to_string()).or_default().push(c);
        }
    }

    /// Number of registered consumers for a type.
    pub fn consumer_count(&self, repo_id: &str) -> usize {
        self.consumers
            .lock()
            .map(|g| g.get(repo_id).map_or(0, Vec::len))
            .unwrap_or(0)
    }
}

/// Spec §2.7.3 — TypedSupplierAdmin interface.
#[derive(Default)]
pub struct TypedSupplierAdmin {
    pull_suppliers: Mutex<BTreeMap<String, Vec<Arc<dyn TypedPullSupplier>>>>,
}

impl core::fmt::Debug for TypedSupplierAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedSupplierAdmin").finish()
    }
}

impl TypedSupplierAdmin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §2.7.5 — `obtain_typed_pull_supplier(repo_id)` slot.
    pub fn register_pull_supplier(&self, repo_id: &str, s: Arc<dyn TypedPullSupplier>) {
        if let Ok(mut g) = self.pull_suppliers.lock() {
            g.entry(repo_id.to_string()).or_default().push(s);
        }
    }

    /// Spec §2.7.5 — `try_pull` across all registered suppliers.
    /// Returns the first available event (or `None`).
    pub fn try_pull(&self, repo_id: &str) -> Option<TypedInvocation> {
        let suppliers = match self.pull_suppliers.lock() {
            Ok(g) => g.get(repo_id).cloned().unwrap_or_default(),
            Err(_) => return None,
        };
        for s in suppliers {
            if let Ok(Some(ev)) = s.try_pull() {
                return Some(ev);
            }
        }
        None
    }

    /// Number of registered pull suppliers for a type.
    pub fn supplier_count(&self, repo_id: &str) -> usize {
        self.pull_suppliers
            .lock()
            .map(|g| g.get(repo_id).map_or(0, Vec::len))
            .unwrap_or(0)
    }
}

/// TypedEventChannel — spec §2.6 / §2.7.1.
pub struct TypedEventChannel {
    /// Map from repository ID to registered consumers.
    consumers: Mutex<BTreeMap<String, Vec<Arc<dyn TypedPushConsumer>>>>,
    /// Spec §2.7.1 — admin splits.
    consumer_admin: Arc<TypedConsumerAdmin>,
    supplier_admin: Arc<TypedSupplierAdmin>,
    /// Spec §2.7.1 — `destroy` marker.
    destroyed: std::sync::atomic::AtomicBool,
}

impl Default for TypedEventChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for TypedEventChannel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedEventChannel").finish()
    }
}

impl TypedEventChannel {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            consumers: Mutex::new(BTreeMap::new()),
            consumer_admin: Arc::new(TypedConsumerAdmin::new()),
            supplier_admin: Arc::new(TypedSupplierAdmin::new()),
            destroyed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Registers a consumer under a `Repository-ID` type.
    pub fn register_consumer(&self, repo_id: &str, c: Arc<dyn TypedPushConsumer>) {
        if let Ok(mut g) = self.consumers.lock() {
            g.entry(repo_id.to_string()).or_default().push(c);
        }
    }

    /// Dispatches an operation to all consumers of a type.
    /// Returns the number of consumers reached.
    pub fn dispatch(&self, repo_id: &str, op: &TypedInvocation) -> usize {
        if self.destroyed.load(std::sync::atomic::Ordering::Acquire) {
            return 0;
        }
        let consumers = match self.consumers.lock() {
            Ok(g) => g.get(repo_id).cloned().unwrap_or_default(),
            Err(_) => return 0,
        };
        let mut count = 0;
        for c in consumers {
            if c.invoke(op).is_ok() {
                count += 1;
            }
        }
        count
    }

    /// Number of registered consumers for a type.
    pub fn consumer_count(&self, repo_id: &str) -> usize {
        self.consumers
            .lock()
            .map(|g| g.get(repo_id).map_or(0, Vec::len))
            .unwrap_or(0)
    }

    /// Spec §2.7.1 — `for_consumers() -> ConsumerAdmin`.
    #[must_use]
    pub fn for_consumers(&self) -> Arc<TypedConsumerAdmin> {
        Arc::clone(&self.consumer_admin)
    }

    /// Spec §2.7.1 — `for_suppliers() -> SupplierAdmin`.
    #[must_use]
    pub fn for_suppliers(&self) -> Arc<TypedSupplierAdmin> {
        Arc::clone(&self.supplier_admin)
    }

    /// Spec §2.7.1 — `destroy()`. Puts the channel into a
    /// "destroyed" state; further `dispatch` calls are no-ops.
    pub fn destroy(&self) {
        self.destroyed
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// `true` if `destroy()` has been called.
    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.destroyed.load(std::sync::atomic::Ordering::Acquire)
    }
}

// ---------------------------------------------------------------------------
// §3.1.2-§3.1.3 Lightweight Profile (CosLightweightEventComm + Channel)
// ---------------------------------------------------------------------------

/// Spec §3.1.2 — `CosLightweightEventComm` profile (subset of
/// §2.1).
///
/// We treat the lightweight profile as the same push path as
/// [`TypedPushConsumer`], but with reduced operations (no `disconnect`
/// lifecycle in the mini variant; channel destroy is enough). The
/// profile marker makes this visible at the code level.
pub mod lightweight {
    /// Spec §3.1.2 — marker constant.
    pub const PROFILE_NAME: &str = "CosLightweightEventComm";

    /// Spec §3.1.3 — `CosLightweightEventChannel`.
    pub const CHANNEL_PROFILE_NAME: &str = "CosLightweightEventChannel";

    /// `true` if the caller wants to actively signal lightweight
    /// profile mode (simplified subscription paths without an
    /// explicit `disconnect` via the channel lifecycle).
    #[must_use]
    pub fn is_lightweight(profile_name: &str) -> bool {
        profile_name == PROFILE_NAME || profile_name == CHANNEL_PROFILE_NAME
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct LoggingConsumer {
        invocations: AtomicUsize,
        connected: AtomicBool,
    }

    impl TypedPushConsumer for LoggingConsumer {
        fn invoke(&self, _op: &TypedInvocation) -> Result<Vec<u8>, Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.invocations.fetch_add(1, Ordering::Relaxed);
            Ok(alloc::vec::Vec::new())
        }

        fn disconnect(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }

    #[test]
    fn dispatch_reaches_registered_consumers() {
        let chan = TypedEventChannel::new();
        let c1 = Arc::new(LoggingConsumer {
            invocations: AtomicUsize::new(0),
            connected: AtomicBool::new(true),
        });
        let c2 = Arc::new(LoggingConsumer {
            invocations: AtomicUsize::new(0),
            connected: AtomicBool::new(true),
        });
        chan.register_consumer(
            "IDL:demo/Tick:1.0",
            Arc::clone(&c1) as Arc<dyn TypedPushConsumer>,
        );
        chan.register_consumer(
            "IDL:demo/Tick:1.0",
            Arc::clone(&c2) as Arc<dyn TypedPushConsumer>,
        );

        let count = chan.dispatch(
            "IDL:demo/Tick:1.0",
            &TypedInvocation {
                operation: "tick".into(),
                args: alloc::vec![],
            },
        );
        assert_eq!(count, 2);
        assert_eq!(c1.invocations.load(Ordering::Relaxed), 1);
        assert_eq!(c2.invocations.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn disconnected_consumers_are_skipped_in_count() {
        let chan = TypedEventChannel::new();
        let c = Arc::new(LoggingConsumer {
            invocations: AtomicUsize::new(0),
            connected: AtomicBool::new(false),
        });
        chan.register_consumer("IDL:t:1.0", Arc::clone(&c) as Arc<dyn TypedPushConsumer>);
        let count = chan.dispatch(
            "IDL:t:1.0",
            &TypedInvocation {
                operation: "x".into(),
                args: alloc::vec![],
            },
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn unknown_repo_id_dispatches_to_zero() {
        let chan = TypedEventChannel::new();
        assert_eq!(
            chan.dispatch(
                "IDL:none:1.0",
                &TypedInvocation {
                    operation: "x".into(),
                    args: alloc::vec![],
                },
            ),
            0
        );
    }

    #[test]
    fn consumer_count_reflects_registrations() {
        let chan = TypedEventChannel::new();
        for _ in 0..3 {
            let c = Arc::new(LoggingConsumer {
                invocations: AtomicUsize::new(0),
                connected: AtomicBool::new(true),
            });
            chan.register_consumer("IDL:t:1.0", c as Arc<dyn TypedPushConsumer>);
        }
        assert_eq!(chan.consumer_count("IDL:t:1.0"), 3);
        assert_eq!(chan.consumer_count("IDL:none:1.0"), 0);
    }

    // ---------------------------------------------------------------
    // §2.5.2 TypedPullSupplier
    // ---------------------------------------------------------------

    struct InMemoryPullSupplier {
        queue: Mutex<Vec<TypedInvocation>>,
        connected: AtomicBool,
    }

    impl TypedPullSupplier for InMemoryPullSupplier {
        fn pull(&self) -> Result<TypedInvocation, Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            // Non-blocking: we simulate without busy-wait via try_pull
            self.try_pull()?.ok_or(Disconnected)
        }
        fn try_pull(&self) -> Result<Option<TypedInvocation>, Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            Ok(self.queue.lock().ok().and_then(|mut g| g.pop()))
        }
        fn disconnect(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }

    #[test]
    fn pull_supplier_try_pull_returns_queued_event() {
        let s = Arc::new(InMemoryPullSupplier {
            queue: Mutex::new(alloc::vec![TypedInvocation {
                operation: "tick".into(),
                args: alloc::vec![],
            }]),
            connected: AtomicBool::new(true),
        });
        let result = s.try_pull().expect("ok");
        assert!(result.is_some());
    }

    #[test]
    fn pull_supplier_try_pull_returns_none_for_empty() {
        let s = Arc::new(InMemoryPullSupplier {
            queue: Mutex::new(alloc::vec![]),
            connected: AtomicBool::new(true),
        });
        assert!(s.try_pull().expect("ok").is_none());
    }

    #[test]
    fn pull_supplier_disconnect_returns_error() {
        let s = Arc::new(InMemoryPullSupplier {
            queue: Mutex::new(alloc::vec![]),
            connected: AtomicBool::new(true),
        });
        s.disconnect();
        assert!(s.try_pull().is_err());
    }

    // ---------------------------------------------------------------
    // §2.7.2-§2.7.5 Typed Admin + Proxy
    // ---------------------------------------------------------------

    #[test]
    fn typed_consumer_admin_register_count_roundtrip() {
        let admin = TypedConsumerAdmin::new();
        let c = Arc::new(LoggingConsumer {
            invocations: AtomicUsize::new(0),
            connected: AtomicBool::new(true),
        });
        admin.register_consumer("IDL:demo/E:1.0", c as Arc<dyn TypedPushConsumer>);
        assert_eq!(admin.consumer_count("IDL:demo/E:1.0"), 1);
    }

    #[test]
    fn typed_supplier_admin_register_count_roundtrip() {
        let admin = TypedSupplierAdmin::new();
        let s = Arc::new(InMemoryPullSupplier {
            queue: Mutex::new(alloc::vec![]),
            connected: AtomicBool::new(true),
        });
        admin.register_pull_supplier("IDL:demo/E:1.0", s as Arc<dyn TypedPullSupplier>);
        assert_eq!(admin.supplier_count("IDL:demo/E:1.0"), 1);
    }

    #[test]
    fn typed_supplier_admin_try_pull_returns_first_available() {
        let admin = TypedSupplierAdmin::new();
        let s = Arc::new(InMemoryPullSupplier {
            queue: Mutex::new(alloc::vec![TypedInvocation {
                operation: "x".into(),
                args: alloc::vec![],
            }]),
            connected: AtomicBool::new(true),
        });
        admin.register_pull_supplier("IDL:demo/E:1.0", s as Arc<dyn TypedPullSupplier>);
        assert!(admin.try_pull("IDL:demo/E:1.0").is_some());
    }

    // ---------------------------------------------------------------
    // §2.7.1 TypedEventChannel admin-splits + destroy
    // ---------------------------------------------------------------

    #[test]
    fn typed_event_channel_for_consumers_returns_admin() {
        let chan = TypedEventChannel::new();
        let admin = chan.for_consumers();
        assert_eq!(admin.consumer_count("anything"), 0);
    }

    #[test]
    fn typed_event_channel_for_suppliers_returns_admin() {
        let chan = TypedEventChannel::new();
        let admin = chan.for_suppliers();
        assert_eq!(admin.supplier_count("anything"), 0);
    }

    #[test]
    fn destroy_disables_dispatch() {
        let chan = TypedEventChannel::new();
        let c = Arc::new(LoggingConsumer {
            invocations: AtomicUsize::new(0),
            connected: AtomicBool::new(true),
        });
        chan.register_consumer("IDL:t:1.0", c as Arc<dyn TypedPushConsumer>);
        assert!(!chan.is_destroyed());
        chan.destroy();
        assert!(chan.is_destroyed());
        let count = chan.dispatch(
            "IDL:t:1.0",
            &TypedInvocation {
                operation: "x".into(),
                args: alloc::vec![],
            },
        );
        assert_eq!(count, 0);
    }

    // ---------------------------------------------------------------
    // §3.1.2-§3.1.3 Lightweight Profile
    // ---------------------------------------------------------------

    #[test]
    fn lightweight_profile_names_match_spec() {
        assert_eq!(lightweight::PROFILE_NAME, "CosLightweightEventComm");
        assert_eq!(
            lightweight::CHANNEL_PROFILE_NAME,
            "CosLightweightEventChannel"
        );
    }

    #[test]
    fn lightweight_is_lightweight_recognizes_both_profiles() {
        assert!(lightweight::is_lightweight("CosLightweightEventComm"));
        assert!(lightweight::is_lightweight("CosLightweightEventChannel"));
        assert!(!lightweight::is_lightweight("CosTypedEventComm"));
    }

    // ---------------------------------------------------------------
    // §2.5 / §2.5.1 TypedPushConsumer + TypedPushSupplier
    // ---------------------------------------------------------------

    /// A typed consumer that records the operation it was called with
    /// and answers with a reply body derived from the arguments.
    struct EchoConsumer {
        last: Mutex<Option<TypedInvocation>>,
        connected: AtomicBool,
    }

    impl EchoConsumer {
        fn new() -> Self {
            Self {
                last: Mutex::new(None),
                connected: AtomicBool::new(true),
            }
        }
        fn last_operation(&self) -> Option<String> {
            self.last
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|i| i.operation.clone()))
        }
    }

    impl TypedPushConsumer for EchoConsumer {
        fn invoke(&self, op: &TypedInvocation) -> Result<Vec<u8>, Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            if let Ok(mut g) = self.last.lock() {
                *g = Some(op.clone());
            }
            Ok(op.args.clone())
        }

        fn disconnect(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }

    /// Spec §2.5.1: in typed mode the channel calls the IDL operation
    /// on the consumer instead of `push(any)` — the operation name and
    /// its encoded arguments reach the consumer unchanged, and the
    /// consumer's reply body is returned to the caller.
    #[test]
    fn typed_push_consumer_invoke_carries_operation_and_args() {
        let c = EchoConsumer::new();
        let reply = c
            .invoke(&TypedInvocation {
                operation: "on_tick".into(),
                args: alloc::vec![1, 2, 3],
            })
            .expect("connected consumer accepts the invocation");
        assert_eq!(reply, alloc::vec![1, 2, 3]);
        assert_eq!(c.last_operation().as_deref(), Some("on_tick"));
    }

    /// Spec §2.5.1 + §2.1.5: after `disconnect_push_consumer` the typed
    /// consumer raises `Disconnected` and records no further calls.
    #[test]
    fn typed_push_consumer_disconnect_raises_disconnected() {
        let c = EchoConsumer::new();
        c.disconnect();
        assert_eq!(
            c.invoke(&TypedInvocation {
                operation: "on_tick".into(),
                args: alloc::vec![],
            })
            .unwrap_err(),
            Disconnected
        );
        assert!(c.last_operation().is_none());
    }

    /// A typed supplier that pushes into a connected typed consumer
    /// until its own `disconnect_push_supplier` ends the connection.
    struct EchoSupplier {
        consumer: Arc<EchoConsumer>,
        connected: AtomicBool,
    }

    impl EchoSupplier {
        fn emit(&self, operation: &str) -> Result<Vec<u8>, Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.consumer.invoke(&TypedInvocation {
                operation: operation.to_string(),
                args: alloc::vec![],
            })
        }
    }

    impl TypedPushSupplier for EchoSupplier {
        fn disconnect(&self) {
            self.connected.store(false, Ordering::Release);
            self.consumer.disconnect();
        }
    }

    /// Spec §2.5: `disconnect_push_supplier` on the typed supplier
    /// terminates the connection in both directions.
    #[test]
    fn typed_push_supplier_disconnect_terminates_the_connection() {
        let consumer = Arc::new(EchoConsumer::new());
        let supplier = EchoSupplier {
            consumer: Arc::clone(&consumer),
            connected: AtomicBool::new(true),
        };
        supplier.emit("on_tick").expect("still connected");
        assert_eq!(consumer.last_operation().as_deref(), Some("on_tick"));

        supplier.disconnect();
        assert_eq!(supplier.emit("on_tick").unwrap_err(), Disconnected);
        assert_eq!(
            consumer
                .invoke(&TypedInvocation {
                    operation: "on_tick".into(),
                    args: alloc::vec![],
                })
                .unwrap_err(),
            Disconnected
        );
    }

    // ---------------------------------------------------------------
    // §3.1.3 CosLightweightEventChannel
    // ---------------------------------------------------------------

    /// Spec §3.1.3: the lightweight channel profile carries no
    /// per-endpoint disconnect operation — the channel lifecycle is the
    /// only teardown. `destroy` therefore has to end the delivery for
    /// consumers that never call a disconnect of their own.
    #[test]
    fn lightweight_channel_destroy_is_the_only_teardown() {
        assert!(lightweight::is_lightweight(
            lightweight::CHANNEL_PROFILE_NAME
        ));
        let chan = TypedEventChannel::new();
        let c = Arc::new(EchoConsumer::new());
        chan.register_consumer(
            "IDL:demo/Tick:1.0",
            Arc::clone(&c) as Arc<dyn TypedPushConsumer>,
        );

        let op = TypedInvocation {
            operation: "on_tick".into(),
            args: alloc::vec![],
        };
        assert_eq!(chan.dispatch("IDL:demo/Tick:1.0", &op), 1);

        chan.destroy();
        assert!(chan.is_destroyed());
        assert_eq!(chan.dispatch("IDL:demo/Tick:1.0", &op), 0);
    }
}
