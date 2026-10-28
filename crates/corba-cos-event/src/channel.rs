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

use std::sync::{Condvar, Mutex};

use crate::comm::{
    AnyEvent, ConnectError, Disconnected, PullConsumer, PullSupplier, PushConsumer, PushSupplier,
};

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

    /// `destroy` — spec §1.6.1.3. Marks the channel as destroyed,
    /// releases every registered consumer by calling
    /// `disconnect_push_consumer` on it, and wakes every `pull` that is
    /// blocked on the channel so it observes `Disconnected`.
    pub fn destroy(&self) {
        let inner = &self.consumer_admin.inner;
        let released = match inner.state.lock() {
            Ok(mut state) => {
                state.destroyed = true;
                state.pending.clear();
                core::mem::take(&mut state.consumers)
            }
            Err(_) => Vec::new(),
        };
        for c in &released {
            c.disconnect_push_consumer();
        }
        inner.ready.notify_all();
    }

    /// `true` once [`EventChannel::destroy`] has been called.
    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.consumer_admin
            .inner
            .state
            .lock()
            .map(|s| s.destroyed)
            .unwrap_or(true)
    }
}

#[derive(Default)]
struct ChannelInner {
    state: Mutex<ChannelState>,
    /// Signalled whenever `pending` gains an event or the channel
    /// changes into a terminal state. Blocking `pull` waits on it
    /// instead of spinning.
    ready: Condvar,
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
        self.inner.ready.notify_all();

        // Spec §1.5.1: a consumer that raises `Disconnected` is no
        // longer part of the communication. Dropping the event on the
        // floor while keeping the dead proxy registered would hide the
        // disconnect, so the channel unregisters it here.
        let mut dead: Vec<Arc<dyn PushConsumer>> = Vec::new();
        for c in &consumers {
            if c.push(event.clone()).is_err() {
                dead.push(Arc::clone(c));
            }
        }
        if !dead.is_empty() {
            if let Ok(mut state) = self.inner.state.lock() {
                state
                    .consumers
                    .retain(|c| !dead.iter().any(|d| Arc::ptr_eq(d, c)));
            }
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

    /// `true` while a consumer is connected to this proxy.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected_consumer
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

impl PushSupplier for ProxyPushSupplier {
    /// `disconnect_push_supplier` — spec §1.6.5.2. Terminates the
    /// connection: the consumer is unregistered from the channel (so
    /// it receives no further events) and its own
    /// `disconnect_push_consumer` is called.
    fn disconnect_push_supplier(&self) {
        let released = match self.connected_consumer.lock() {
            Ok(mut g) => g.take(),
            Err(_) => None,
        };
        if let Some(consumer) = released {
            if let Ok(mut state) = self.inner.state.lock() {
                state.consumers.retain(|c| !Arc::ptr_eq(c, &consumer));
            }
            consumer.disconnect_push_consumer();
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
        // Spec §1.6.7.2: blocks until an event is available. The wait
        // is a condition variable, not a poll loop; every producer of a
        // terminal state (`destroy`, `disconnect_pull_supplier`) and
        // every producer of an event signals it.
        let mut state = self.inner.state.lock().map_err(|_| Disconnected)?;
        loop {
            if !self.connected.load(Ordering::Acquire) || state.destroyed {
                return Err(Disconnected);
            }
            if let Some(e) = state.pending.pop_front() {
                return Ok(e);
            }
            state = self.inner.ready.wait(state).map_err(|_| Disconnected)?;
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
        // Wake a `pull` that is blocked on this proxy so it observes
        // the terminal state instead of waiting forever.
        self.inner.ready.notify_all();
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
    ///
    /// # Errors
    /// `Disconnected` before `connect_pull_supplier`, after
    /// `disconnect_pull_consumer`, and after channel `destroy`.
    pub fn forward_event(&self, event: AnyEvent) -> Result<(), Disconnected> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(Disconnected);
        }
        let mut state = self.inner.state.lock().map_err(|_| Disconnected)?;
        if state.destroyed {
            return Err(Disconnected);
        }
        state.pending.push_back(event);
        drop(state);
        self.inner.ready.notify_all();
        Ok(())
    }
}

impl PullConsumer for ProxyPullConsumer {
    /// `disconnect_pull_consumer` — spec §1.6.6.2. Terminates the
    /// connection; further `forward_event` calls raise `Disconnected`.
    fn disconnect_pull_consumer(&self) {
        self.connected.store(false, Ordering::Release);
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

    // -----------------------------------------------------------------
    // §2.3.1 EventChannel
    // -----------------------------------------------------------------

    /// Spec §2.3.1: `for_consumers`/`for_suppliers` return the admin
    /// objects of *this* channel — both views share one channel state,
    /// so an event pushed through the supplier side is visible on the
    /// consumer side.
    #[test]
    fn event_channel_admins_share_one_channel_state() {
        let chan = EventChannel::new();
        let push = chan.for_suppliers().obtain_push_consumer();
        push.connect_push_supplier().unwrap();
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();

        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![9]))
            .unwrap();
        let (e, has) = pull.try_pull().unwrap();
        assert!(has);
        assert_eq!(e.data, alloc::vec![9]);
    }

    /// Spec §2.3.1: `destroy` releases the connected consumers — each
    /// registered `PushConsumer` gets `disconnect_push_consumer`, and
    /// the channel reports the terminal state.
    #[test]
    fn destroy_disconnects_registered_consumers() {
        let chan = EventChannel::new();
        let proxy = chan.for_consumers().obtain_push_supplier();
        let c = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        proxy
            .connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
            .unwrap();

        assert!(!chan.is_destroyed());
        chan.destroy();
        assert!(chan.is_destroyed());
        // `disconnect_push_consumer` was called on the consumer itself.
        assert_eq!(
            c.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
    }

    /// Spec §2.3.1 + §2.1.5: a `pull` blocked on the channel does not
    /// hang after `destroy`; it observes `Disconnected`.
    #[test]
    fn destroy_wakes_blocked_pull_with_disconnected() {
        let chan = Arc::new(EventChannel::new());
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();

        let waiter = {
            let pull = Arc::clone(&pull);
            std::thread::spawn(move || pull.pull())
        };
        let chan2 = Arc::clone(&chan);
        std::thread::spawn(move || {
            std::thread::sleep(core::time::Duration::from_millis(20));
            chan2.destroy();
        });
        assert_eq!(waiter.join().unwrap().unwrap_err(), Disconnected);
    }

    // -----------------------------------------------------------------
    // §2.3.2 ConsumerAdmin / §2.3.3 SupplierAdmin
    // -----------------------------------------------------------------

    /// Spec §2.3.2: every `obtain_push_supplier`/`obtain_pull_supplier`
    /// call returns a *fresh* proxy; two consumers therefore never share
    /// one proxy connection.
    #[test]
    fn consumer_admin_returns_a_fresh_proxy_per_call() {
        let chan = EventChannel::new();
        let admin = chan.for_consumers();
        let p1 = admin.obtain_push_supplier();
        let p2 = admin.obtain_push_supplier();
        assert!(!Arc::ptr_eq(&p1, &p2));

        let c = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        p1.connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
            .unwrap();
        // The second proxy is unconnected and accepts its own consumer.
        assert!(p1.is_connected());
        assert!(!p2.is_connected());
        p2.connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
            .unwrap();

        let s1 = admin.obtain_pull_supplier();
        let s2 = admin.obtain_pull_supplier();
        assert!(!Arc::ptr_eq(&s1, &s2));
        s1.connect_pull_consumer().unwrap();
        // s2 is still unconnected, so its own connect succeeds.
        s2.connect_pull_consumer().unwrap();
    }

    /// Spec §2.3.3: `obtain_push_consumer`/`obtain_pull_consumer`
    /// likewise return a fresh, independently connectable proxy.
    #[test]
    fn supplier_admin_returns_a_fresh_proxy_per_call() {
        let chan = EventChannel::new();
        let admin = chan.for_suppliers();
        let p1 = admin.obtain_push_consumer();
        let p2 = admin.obtain_push_consumer();
        assert!(!Arc::ptr_eq(&p1, &p2));
        p1.connect_push_supplier().unwrap();
        p2.connect_push_supplier().unwrap();

        let c1 = admin.obtain_pull_consumer();
        let c2 = admin.obtain_pull_consumer();
        assert!(!Arc::ptr_eq(&c1, &c2));
    }

    // -----------------------------------------------------------------
    // §2.3.4 ProxyPushConsumer
    // -----------------------------------------------------------------

    /// Spec §2.3.4 + §2.1.5: before `connect_push_supplier` the proxy is
    /// not part of any communication — `push` raises `Disconnected`.
    #[test]
    fn proxy_push_consumer_push_before_connect_raises_disconnected() {
        let chan = EventChannel::new();
        let proxy = chan.for_suppliers().obtain_push_consumer();
        assert_eq!(
            proxy
                .push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
    }

    /// Spec §2.1.5: a consumer that raises `Disconnected` is removed
    /// from the channel — the event is not dropped silently while the
    /// dead proxy stays registered.
    #[test]
    fn push_unregisters_a_consumer_that_reports_disconnected() {
        let chan = EventChannel::new();
        let push = chan.for_suppliers().obtain_push_consumer();
        push.connect_push_supplier().unwrap();
        let live = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        let dead = Arc::new(CountingConsumer(
            AtomicUsize::new(0),
            AtomicBool::new(false),
        ));
        let admin = chan.for_consumers();
        admin
            .obtain_push_supplier()
            .connect_push_consumer(Arc::clone(&live) as Arc<dyn PushConsumer>)
            .unwrap();
        admin
            .obtain_push_supplier()
            .connect_push_consumer(Arc::clone(&dead) as Arc<dyn PushConsumer>)
            .unwrap();

        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![1]))
            .unwrap();
        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![2]))
            .unwrap();
        assert_eq!(cnt(&live), 2);
        assert_eq!(cnt(&dead), 0);
        // Re-arming the formerly dead consumer must not resurrect a
        // stale registration: the channel unregistered it on the first
        // `Disconnected`, so it stays at zero.
        dead.1.store(true, Ordering::Release);
        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![3]))
            .unwrap();
        assert_eq!(cnt(&dead), 0);
        assert_eq!(cnt(&live), 3);
    }

    // -----------------------------------------------------------------
    // §2.3.5 ProxyPullSupplier
    // -----------------------------------------------------------------

    /// Spec §2.3.5: `try_pull` returns the `has_event` flag — `false`
    /// with an empty event when the queue is empty, `true` with the
    /// event otherwise. Before `connect_pull_consumer` it raises
    /// `Disconnected`.
    #[test]
    fn proxy_pull_supplier_try_pull_reports_has_event_flag() {
        let chan = EventChannel::new();
        let pull = chan.for_consumers().obtain_pull_supplier();
        assert_eq!(pull.try_pull().unwrap_err(), Disconnected);
        pull.connect_pull_consumer().unwrap();

        let (empty, has) = pull.try_pull().unwrap();
        assert!(!has);
        assert!(empty.data.is_empty());
        assert!(empty.type_id.is_empty());

        let push = chan.for_suppliers().obtain_push_consumer();
        push.connect_push_supplier().unwrap();
        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![5]))
            .unwrap();
        let (e, has) = pull.try_pull().unwrap();
        assert!(has);
        assert_eq!(e.data, alloc::vec![5]);
    }

    /// Spec §2.3.5: `pull` blocks until an event is available — the
    /// call returns only after a supplier has pushed.
    #[test]
    fn proxy_pull_supplier_pull_blocks_until_an_event_arrives() {
        let chan = Arc::new(EventChannel::new());
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();
        let push = chan.for_suppliers().obtain_push_consumer();
        push.connect_push_supplier().unwrap();

        let waiter = {
            let pull = Arc::clone(&pull);
            std::thread::spawn(move || pull.pull())
        };
        std::thread::sleep(core::time::Duration::from_millis(20));
        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![42]))
            .unwrap();
        let got = waiter.join().unwrap().unwrap();
        assert_eq!(got.data, alloc::vec![42]);
    }

    /// Spec §2.3.5: a second `connect_pull_consumer` on the same proxy
    /// raises `AlreadyConnected`.
    #[test]
    fn proxy_pull_supplier_double_connect_yields_already_connected() {
        let chan = EventChannel::new();
        let pull = chan.for_consumers().obtain_pull_supplier();
        pull.connect_pull_consumer().unwrap();
        assert_eq!(
            pull.connect_pull_consumer().unwrap_err(),
            ConnectError::AlreadyConnected
        );
    }

    // -----------------------------------------------------------------
    // §2.3.6 ProxyPullConsumer
    // -----------------------------------------------------------------

    /// Spec §2.3.6: after `connect_pull_supplier` the events the proxy
    /// pulls from the supplier enter the channel — a consumer-side
    /// `ProxyPullSupplier` dequeues them.
    #[test]
    fn proxy_pull_consumer_forwards_pulled_events_into_the_channel() {
        let chan = EventChannel::new();
        let pull_consumer = chan.for_suppliers().obtain_pull_consumer();
        let source = Arc::new(QueuedPullSupplier::with(alloc::vec![7, 8]));
        pull_consumer
            .connect_pull_supplier(Arc::clone(&source) as Arc<dyn PullSupplier>)
            .unwrap();

        let sink = chan.for_consumers().obtain_pull_supplier();
        sink.connect_pull_consumer().unwrap();

        // The channel worker pulls from the supplier and forwards.
        while let Ok((event, true)) = source.try_pull() {
            pull_consumer.forward_event(event).unwrap();
        }
        let (first, has) = sink.try_pull().unwrap();
        assert!(has);
        assert_eq!(first.data, alloc::vec![7]);
        let (second, has) = sink.try_pull().unwrap();
        assert!(has);
        assert_eq!(second.data, alloc::vec![8]);
    }

    /// Spec §2.3.6 + §2.1.4: before `connect_pull_supplier` and after
    /// `disconnect_pull_consumer` the proxy raises `Disconnected`; a
    /// second connect raises `AlreadyConnected`.
    #[test]
    fn proxy_pull_consumer_connect_and_disconnect_lifecycle() {
        let chan = EventChannel::new();
        let proxy = chan.for_suppliers().obtain_pull_consumer();
        let source = Arc::new(QueuedPullSupplier::with(alloc::vec![]));

        assert_eq!(
            proxy
                .forward_event(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
        proxy
            .connect_pull_supplier(Arc::clone(&source) as Arc<dyn PullSupplier>)
            .unwrap();
        assert_eq!(
            proxy
                .connect_pull_supplier(Arc::clone(&source) as Arc<dyn PullSupplier>)
                .unwrap_err(),
            ConnectError::AlreadyConnected
        );
        proxy
            .forward_event(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![1]))
            .unwrap();
        proxy.disconnect_pull_consumer();
        assert_eq!(
            proxy
                .forward_event(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![2]))
                .unwrap_err(),
            Disconnected
        );
    }

    // -----------------------------------------------------------------
    // §2.3.7 ProxyPushSupplier
    // -----------------------------------------------------------------

    /// Spec §2.3.7: `connect_push_consumer` binds exactly one consumer;
    /// a second call raises `AlreadyConnected`.
    #[test]
    fn proxy_push_supplier_double_connect_yields_already_connected() {
        let chan = EventChannel::new();
        let proxy = chan.for_consumers().obtain_push_supplier();
        let c = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        proxy
            .connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
            .unwrap();
        assert_eq!(
            proxy
                .connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
                .unwrap_err(),
            ConnectError::AlreadyConnected
        );
    }

    /// Spec §2.3.7 + §2.1.2: `disconnect_push_supplier` terminates the
    /// connection — the consumer receives no further events and its own
    /// `disconnect_push_consumer` has been called.
    #[test]
    fn proxy_push_supplier_disconnect_stops_event_delivery() {
        let chan = EventChannel::new();
        let push = chan.for_suppliers().obtain_push_consumer();
        push.connect_push_supplier().unwrap();
        let proxy = chan.for_consumers().obtain_push_supplier();
        let c = Arc::new(CountingConsumer(AtomicUsize::new(0), AtomicBool::new(true)));
        proxy
            .connect_push_consumer(Arc::clone(&c) as Arc<dyn PushConsumer>)
            .unwrap();

        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![1]))
            .unwrap();
        assert_eq!(cnt(&c), 1);

        proxy.disconnect_push_supplier();
        assert!(!proxy.is_connected());
        push.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![2]))
            .unwrap();
        assert_eq!(cnt(&c), 1);
        assert_eq!(
            c.push(AnyEvent::new("IDL:E:1.0".into(), alloc::vec![]))
                .unwrap_err(),
            Disconnected
        );
    }

    /// Test double for §2.3.6: a `PullSupplier` with a fixed event
    /// queue, standing in for the remote supplier the channel pulls
    /// from.
    struct QueuedPullSupplier {
        queue: Mutex<VecDeque<AnyEvent>>,
        connected: AtomicBool,
    }

    impl QueuedPullSupplier {
        fn with(payloads: Vec<u8>) -> Self {
            Self {
                queue: Mutex::new(
                    payloads
                        .into_iter()
                        .map(|b| AnyEvent::new("IDL:E:1.0".into(), alloc::vec![b]))
                        .collect(),
                ),
                connected: AtomicBool::new(true),
            }
        }
    }

    impl PullSupplier for QueuedPullSupplier {
        fn pull(&self) -> Result<AnyEvent, Disconnected> {
            self.try_pull()
                .and_then(|(e, has)| if has { Ok(e) } else { Err(Disconnected) })
        }
        fn try_pull(&self) -> Result<(AnyEvent, bool), Disconnected> {
            if !self.connected.load(Ordering::Acquire) {
                return Err(Disconnected);
            }
            let mut q = self.queue.lock().map_err(|_| Disconnected)?;
            Ok(match q.pop_front() {
                Some(e) => (e, true),
                None => (empty_event(), false),
            })
        }
        fn disconnect_pull_supplier(&self) {
            self.connected.store(false, Ordering::Release);
        }
    }
}
