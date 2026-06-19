// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosEventChannelAdmin — spec §1.6.
//!
//! `EventChannel` is the top-level component. Suppliers connect via
//! `for_suppliers().obtain_push_consumer()` (or
//! `obtain_pull_supplier()`); consumers via `for_consumers().
//! obtain_push_supplier()` (or `obtain_pull_consumer()`).
//!
//! In-memory implementation with fan-out: every pushed event is
//! distributed to all registered consumers.

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use std::sync::Mutex;

use crate::comm::{AnyEvent, ConnectError, Disconnected, PullSupplier, PushConsumer};

/// EventChannel — spec §1.6.1.
pub struct EventChannel {
    consumer_admin: Arc<ConsumerAdmin>,
    supplier_admin: Arc<SupplierAdmin>,
}

impl Default for EventChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for EventChannel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventChannel").finish()
    }
}

impl EventChannel {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        let inner = Arc::new(ChannelInner::default());
        Self {
            consumer_admin: Arc::new(ConsumerAdmin {
                inner: Arc::clone(&inner),
            }),
            supplier_admin: Arc::new(SupplierAdmin {
                inner: Arc::clone(&inner),
            }),
        }
    }

    /// `for_consumers` — spec §1.6.1.1.
    #[must_use]
    pub fn for_consumers(&self) -> Arc<ConsumerAdmin> {
        Arc::clone(&self.consumer_admin)
    }

    /// `for_suppliers` — spec §1.6.1.2.
    #[must_use]
    pub fn for_suppliers(&self) -> Arc<SupplierAdmin> {
        Arc::clone(&self.supplier_admin)
    }

    /// `destroy` — spec §1.6.1.3.
    pub fn destroy(&self) {
        if let Ok(mut state) = self.consumer_admin.inner.state.lock() {
            state.destroyed = true;
            state.consumers.clear();
        }
    }
}

#[derive(Default)]
struct ChannelInner {
    state: Mutex<ChannelState>,
}

#[derive(Default)]
struct ChannelState {
    destroyed: bool,
    consumers: Vec<Arc<dyn PushConsumer>>,
    pending: VecDeque<AnyEvent>,
}

/// ConsumerAdmin — spec §1.6.2.
pub struct ConsumerAdmin {
    inner: Arc<ChannelInner>,
}

impl core::fmt::Debug for ConsumerAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConsumerAdmin").finish()
    }
}

impl ConsumerAdmin {
    /// `obtain_push_supplier` — spec §1.6.2.1.
    #[must_use]
    pub fn obtain_push_supplier(&self) -> Arc<ProxyPushSupplier> {
        Arc::new(ProxyPushSupplier {
            inner: Arc::clone(&self.inner),
            connected_consumer: Mutex::new(None),
        })
    }

    /// `obtain_pull_supplier` — spec §1.6.2.2.
    #[must_use]
    pub fn obtain_pull_supplier(&self) -> Arc<ProxyPullSupplier> {
        Arc::new(ProxyPullSupplier {
            inner: Arc::clone(&self.inner),
            connected: AtomicBool::new(false),
        })
    }
}

/// SupplierAdmin — spec §1.6.3.
pub struct SupplierAdmin {
    inner: Arc<ChannelInner>,
}

impl core::fmt::Debug for SupplierAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SupplierAdmin").finish()
    }
}

impl SupplierAdmin {
    /// `obtain_push_consumer` — spec §1.6.3.1.
    #[must_use]
    pub fn obtain_push_consumer(&self) -> Arc<ProxyPushConsumer> {
        Arc::new(ProxyPushConsumer {
            inner: Arc::clone(&self.inner),
            connected: AtomicBool::new(false),
        })
    }

    /// `obtain_pull_consumer` — spec §1.6.3.2.
    #[must_use]
    pub fn obtain_pull_consumer(&self) -> Arc<ProxyPullConsumer> {
        Arc::new(ProxyPullConsumer {
            inner: Arc::clone(&self.inner),
            connected: AtomicBool::new(false),
        })
    }
}

/// ProxyPushConsumer — spec §1.6.4.
pub struct ProxyPushConsumer {
    inner: Arc<ChannelInner>,
    connected: AtomicBool,
}

impl ProxyPushConsumer {
    /// `connect_push_supplier` — spec §1.6.4.1.
    ///
    /// # Errors
    /// `AlreadyConnected` if already connected.
    pub fn connect_push_supplier(&self) -> Result<(), ConnectError> {
        if self.connected.swap(true, Ordering::AcqRel) {
            return Err(ConnectError::AlreadyConnected);
        }
        Ok(())
    }
}

impl PushConsumer for ProxyPushConsumer {
    fn push(&self, event: AnyEvent) -> Result<(), Disconnected> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(Disconnected);
        }
        let mut state = self.inner.state.lock().map_err(|_| Disconnected)?;
        if state.destroyed {
            return Err(Disconnected);
        }
        // Push directly to all registered consumers.
        let consumers = state.consumers.clone();
        // We also queue for pull mode.
        state.pending.push_back(event.clone());
        drop(state);
        for c in &consumers {
            let _ = c.push(event.clone());
        }
        Ok(())
    }

    fn disconnect_push_consumer(&self) {
        self.connected.store(false, Ordering::Release);
    }
}

/// ProxyPushSupplier — spec §1.6.5.
pub struct ProxyPushSupplier {
    inner: Arc<ChannelInner>,
    connected_consumer: Mutex<Option<Arc<dyn PushConsumer>>>,
}

impl ProxyPushSupplier {
    /// `connect_push_consumer` — spec §1.6.5.1.
    ///
    /// # Errors
    /// `AlreadyConnected` if already connected.
    pub fn connect_push_consumer(
        &self,
        consumer: Arc<dyn PushConsumer>,
    ) -> Result<(), ConnectError> {
        let mut g = self
            .connected_consumer
            .lock()
            .map_err(|_| ConnectError::TypeError)?;
        if g.is_some() {
            return Err(ConnectError::AlreadyConnected);
        }
        *g = Some(Arc::clone(&consumer));
        drop(g);
        // Register in the channel so push calls arrive.
        if let Ok(mut state) = self.inner.state.lock() {
            state.consumers.push(consumer);
        }
        Ok(())
    }

    /// `disconnect_push_supplier` — spec §1.6.5.2.
    pub fn disconnect(&self) {
        if let Ok(mut g) = self.connected_consumer.lock() {
            *g = None;
        }
    }
}

/// ProxyPullSupplier — spec §1.6.7.
pub struct ProxyPullSupplier {
    inner: Arc<ChannelInner>,
    connected: AtomicBool,
}

impl ProxyPullSupplier {
    /// `connect_pull_consumer` — spec §1.6.7.1.
    ///
    /// # Errors
    /// `AlreadyConnected`.
    pub fn connect_pull_consumer(&self) -> Result<(), ConnectError> {
        if self.connected.swap(true, Ordering::AcqRel) {
            return Err(ConnectError::AlreadyConnected);
        }
        Ok(())
    }
}

impl PullSupplier for ProxyPullSupplier {
    fn pull(&self) -> Result<AnyEvent, Disconnected> {
        // Spec §1.6.7.2: blocks until an event is available.
        // In this in-memory implementation we poll actively.
        loop {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            if let Some(e) = self.try_dequeue()? {
                return Ok(e);
            }
            // Yield briefly — a production implementation would use a
            // condition variable, but we keep it simple.
            std::thread::yield_now();
        }
    }

    fn try_pull(&self) -> Result<(AnyEvent, bool), Disconnected> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(Disconnected);
        }
        match self.try_dequeue()? {
            Some(e) => Ok((e, true)),
            None => Ok((empty_event(), false)),
        }
    }

    fn disconnect_pull_supplier(&self) {
        self.connected.store(false, Ordering::Release);
    }
}

impl ProxyPullSupplier {
    fn try_dequeue(&self) -> Result<Option<AnyEvent>, Disconnected> {
        let mut state = self.inner.state.lock().map_err(|_| Disconnected)?;
        if state.destroyed {
            return Err(Disconnected);
        }
        Ok(state.pending.pop_front())
    }
}

fn empty_event() -> AnyEvent {
    AnyEvent::new(String::new(), Vec::new())
}

/// ProxyPullConsumer — spec §1.6.6. The caller (channel worker) calls
/// `pull` on the connected `PullSupplier` and forwards events to the
/// `ChannelInner` via `forward_event`.
pub struct ProxyPullConsumer {
    inner: Arc<ChannelInner>,
    connected: AtomicBool,
}

impl ProxyPullConsumer {
    /// `connect_pull_supplier` — spec §1.6.6.1.
    ///
    /// # Errors
    /// `AlreadyConnected`.
    pub fn connect_pull_supplier(
        &self,
        _supplier: Arc<dyn PullSupplier>,
    ) -> Result<(), ConnectError> {
        if self.connected.swap(true, Ordering::AcqRel) {
            return Err(ConnectError::AlreadyConnected);
        }
        Ok(())
    }

    /// The caller (channel worker) feeds in an event pulled from the
    /// supplier. Counterpart to the `push` method on the push side.
    pub fn forward_event(&self, event: AnyEvent) -> Result<(), Disconnected> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(Disconnected);
        }
        let mut state = self.inner.state.lock().map_err(|_| Disconnected)?;
        if state.destroyed {
            return Err(Disconnected);
        }
        state.pending.push_back(event);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicUsize;

    struct CountingConsumer(AtomicUsize, AtomicBool);
    impl PushConsumer for CountingConsumer {
        fn push(&self, _e: AnyEvent) -> Result<(), Disconnected> {
            if !self.1.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn disconnect_push_consumer(&self) {
            self.1.store(false, Ordering::Release);
        }
    }

    fn cnt(c: &Arc<CountingConsumer>) -> usize {
        c.0.load(Ordering::Relaxed)
    }

    #[test]
    fn push_fan_out_to_multiple_consumers() {
        let chan = EventChannel::new();
        let supplier_admin = chan.for_suppliers();
        let proxy_consumer = supplier_admin.obtain_push_consumer();
        proxy_consumer.connect_push_supplier().unwrap();

        let consumer_admin = chan.for_consumers();
        let p1 = consumer_admin.obtain_push_supplier();
        let p2 = consumer_admin.obtain_push_supplier();

        let c1 = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        let c2 = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        p1.connect_push_consumer(Arc::clone(&c1) as Arc<dyn PushConsumer>)
            .unwrap();
        p2.connect_push_consumer(Arc::clone(&c2) as Arc<dyn PushConsumer>)
            .unwrap();

        proxy_consumer
            .push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![1]))
            .unwrap();
        assert_eq!(cnt(&c1), 1);
        assert_eq!(cnt(&c2), 1);
    }

    #[test]
    fn pull_supplier_dequeues_events() {
        let chan = EventChannel::new();
        let proxy_consumer = chan.for_suppliers().obtain_push_consumer();
        proxy_consumer.connect_push_supplier().unwrap();
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();

        proxy_consumer
            .push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![7]))
            .unwrap();
        let (e, has) = pull.try_pull().unwrap();
        assert!(has);
        assert_eq!(e.data, alloc::vec![7]);
        let (_, has2) = pull.try_pull().unwrap();
        assert!(!has2);
    }

    #[test]
    fn double_connect_yields_already_connected() {
        let chan = EventChannel::new();
        let proxy = chan.for_suppliers().obtain_push_consumer();
        proxy.connect_push_supplier().unwrap();
        assert_eq!(
            proxy.connect_push_supplier().unwrap_err(),
            ConnectError::AlreadyConnected
        );
    }

    #[test]
    fn destroy_disconnects_proxies() {
        let chan = EventChannel::new();
        let proxy = chan.for_suppliers().obtain_push_consumer();
        proxy.connect_push_supplier().unwrap();
        chan.destroy();
        let err = proxy
            .push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![]))
            .unwrap_err();
        assert_eq!(err, Disconnected);
    }

    #[test]
    fn pull_supplier_disconnect_propagates() {
        let chan = EventChannel::new();
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();
        pull.disconnect_pull_supplier();
        assert_eq!(pull.try_pull().unwrap_err(), Disconnected);
    }
}
