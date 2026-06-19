// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CosNotifyChannelAdmin §4 — EventChannelFactory + EventChannel +
//! Consumer/SupplierAdmin + structured-proxy hierarchy.
//!
//! Model (in-memory, analogous to CosEvent but for `StructuredEvent`): a supplier
//! pushes into the channel via a `StructuredProxyPushConsumer`; the channel
//! fans the event out to all connected `StructuredProxyPushSupplier`s
//! (each filtered) AND places it into a pull queue from which
//! `StructuredProxyPullSupplier`s (also filtered) draw.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::Mutex;

use crate::comm::{ConnectError, Disconnected, StructuredPushConsumer};
use crate::event::StructuredEvent;
use crate::filter::Filter;
use crate::qos::QoSProperties;

/// A push-connected sink plus its filter.
struct PushLink {
    consumer: Arc<dyn StructuredPushConsumer>,
    filter: Arc<Filter>,
}

struct ChannelState {
    push_links: Vec<PushLink>,
    pending: VecDeque<StructuredEvent>,
    qos: QoSProperties,
}

struct ChannelInner {
    state: Mutex<ChannelState>,
}

impl ChannelInner {
    fn deliver(&self, event: StructuredEvent) {
        if let Ok(mut s) = self.state.lock() {
            for link in &s.push_links {
                if link.filter.match_structured(&event) {
                    let _ = link.consumer.push_structured_event(event.clone());
                }
            }
            // MaxQueueLength (admin QoS): bounded pull queue (0 = unbounded).
            let max = s
                .qos
                .get(crate::qos::MAX_QUEUE_LENGTH)
                .and_then(any_as_u32)
                .unwrap_or(0);
            s.pending.push_back(event);
            if max > 0 {
                while s.pending.len() > max as usize {
                    s.pending.pop_front();
                }
            }
        }
    }
}

fn any_as_u32(a: &zerodds_cdr::CorbaAny) -> Option<u32> {
    match &a.0 {
        zerodds_cdr::AnyValue::ULong(v) => Some(*v),
        zerodds_cdr::AnyValue::Long(v) if *v >= 0 => Some(*v as u32),
        _ => None,
    }
}

/// `CosNotifyChannelAdmin::EventChannelFactory` (§4.1).
#[derive(Default)]
pub struct EventChannelFactory {
    channels: Mutex<Vec<(i32, Arc<EventChannel>)>>,
    next_id: core::sync::atomic::AtomicI32,
}

impl core::fmt::Debug for EventChannelFactory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventChannelFactory")
            .finish_non_exhaustive()
    }
}

impl EventChannelFactory {
    /// New factory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(Vec::new()),
            next_id: core::sync::atomic::AtomicI32::new(1),
        }
    }

    /// `create_channel` (§4.1) → `(channel, channel_id)`. `initial_qos`/`admin`
    /// are adopted as a QoS bag.
    pub fn create_channel(&self, initial_qos: QoSProperties) -> (Arc<EventChannel>, i32) {
        let ch = Arc::new(EventChannel::with_qos(initial_qos));
        let id = self
            .next_id
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if let Ok(mut g) = self.channels.lock() {
            g.push((id, Arc::clone(&ch)));
        }
        (ch, id)
    }

    /// `get_all_channels` (§4.1) — assigned channel IDs.
    #[must_use]
    pub fn all_channels(&self) -> Vec<i32> {
        self.channels
            .lock()
            .map(|g| g.iter().map(|(id, _)| *id).collect())
            .unwrap_or_default()
    }

    /// `get_event_channel` (§4.1).
    #[must_use]
    pub fn get_event_channel(&self, id: i32) -> Option<Arc<EventChannel>> {
        self.channels
            .lock()
            .ok()?
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, ch)| Arc::clone(ch))
    }
}

/// `CosNotifyChannelAdmin::EventChannel` (§4.3).
pub struct EventChannel {
    inner: Arc<ChannelInner>,
    consumer_admin: Arc<ConsumerAdmin>,
    supplier_admin: Arc<SupplierAdmin>,
}

impl core::fmt::Debug for EventChannel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventChannel").finish_non_exhaustive()
    }
}

impl Default for EventChannel {
    fn default() -> Self {
        Self::with_qos(QoSProperties::new())
    }
}

impl EventChannel {
    /// New channel with empty QoS.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// New channel with initial QoS.
    #[must_use]
    pub fn with_qos(qos: QoSProperties) -> Self {
        let inner = Arc::new(ChannelInner {
            state: Mutex::new(ChannelState {
                push_links: Vec::new(),
                pending: VecDeque::new(),
                qos,
            }),
        });
        Self {
            consumer_admin: Arc::new(ConsumerAdmin {
                inner: Arc::clone(&inner),
            }),
            supplier_admin: Arc::new(SupplierAdmin {
                inner: Arc::clone(&inner),
            }),
            inner,
        }
    }

    /// `for_consumers` / `default_consumer_admin` (§4.3).
    #[must_use]
    pub fn for_consumers(&self) -> Arc<ConsumerAdmin> {
        Arc::clone(&self.consumer_admin)
    }

    /// `for_suppliers` / `default_supplier_admin` (§4.3).
    #[must_use]
    pub fn for_suppliers(&self) -> Arc<SupplierAdmin> {
        Arc::clone(&self.supplier_admin)
    }

    /// `set_qos` (§2.4) — adopt QoS properties.
    pub fn set_qos(&self, props: &crate::event::PropertySeq) {
        if let Ok(mut s) = self.inner.state.lock() {
            s.qos.apply(props);
        }
    }

    /// `get_qos` (§2.4).
    #[must_use]
    pub fn get_qos(&self) -> QoSProperties {
        self.inner
            .state
            .lock()
            .map(|s| s.qos.clone())
            .unwrap_or_default()
    }

    /// `destroy` (§4.3) — clear connections and queue.
    pub fn destroy(&self) {
        if let Ok(mut s) = self.inner.state.lock() {
            s.push_links.clear();
            s.pending.clear();
        }
    }
}

/// `CosNotifyChannelAdmin::ConsumerAdmin` (§4.4) — source of consumer-side
/// proxies (StructuredProxyPush/PullSupplier).
pub struct ConsumerAdmin {
    inner: Arc<ChannelInner>,
}

impl core::fmt::Debug for ConsumerAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConsumerAdmin").finish_non_exhaustive()
    }
}

impl ConsumerAdmin {
    /// `obtain_notification_push_supplier` (structured variant, §4.4): returns
    /// a proxy to which the consumer connects as a push sink.
    #[must_use]
    pub fn obtain_structured_push_supplier(&self) -> Arc<StructuredProxyPushSupplier> {
        Arc::new(StructuredProxyPushSupplier {
            inner: Arc::clone(&self.inner),
            filter: Arc::new(Filter::new("EXTENDED_TCL")),
        })
    }

    /// `obtain_notification_pull_supplier` (structured variant, §4.4): proxy
    /// from which the consumer pulls events.
    #[must_use]
    pub fn obtain_structured_pull_supplier(&self) -> Arc<StructuredProxyPullSupplier> {
        Arc::new(StructuredProxyPullSupplier {
            inner: Arc::clone(&self.inner),
            filter: Arc::new(Filter::new("EXTENDED_TCL")),
        })
    }
}

/// `CosNotifyChannelAdmin::SupplierAdmin` (§4.5) — source of supplier-side
/// proxies (StructuredProxyPush/PullConsumer).
pub struct SupplierAdmin {
    inner: Arc<ChannelInner>,
}

impl core::fmt::Debug for SupplierAdmin {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SupplierAdmin").finish_non_exhaustive()
    }
}

impl SupplierAdmin {
    /// `obtain_notification_push_consumer` (structured variant, §4.5): proxy
    /// into which the supplier pushes events.
    #[must_use]
    pub fn obtain_structured_push_consumer(&self) -> Arc<StructuredProxyPushConsumer> {
        Arc::new(StructuredProxyPushConsumer {
            inner: Arc::clone(&self.inner),
        })
    }
}

/// `StructuredProxyPushConsumer` (§4.6): the supplier pushes events in here;
/// they are distributed within the channel.
pub struct StructuredProxyPushConsumer {
    inner: Arc<ChannelInner>,
}

impl StructuredProxyPushConsumer {
    /// `connect_structured_push_supplier` (§4.6) — no-op connect (single supplier).
    ///
    /// # Errors
    /// never (model without multi-connect tracking).
    pub fn connect_structured_push_supplier(&self) -> Result<(), ConnectError> {
        Ok(())
    }

    /// `push_structured_event` (§4.6) — feed an event into the channel.
    pub fn push_structured_event(&self, event: StructuredEvent) {
        self.inner.deliver(event);
    }
}

/// `StructuredProxyPushSupplier` (§4.7): pushes channel events to the connected
/// consumer (filtered).
pub struct StructuredProxyPushSupplier {
    inner: Arc<ChannelInner>,
    filter: Arc<Filter>,
}

impl StructuredProxyPushSupplier {
    /// `add_filter` (§4.x) — this proxy's filter (applies to the push fanout).
    #[must_use]
    pub fn filter(&self) -> Arc<Filter> {
        Arc::clone(&self.filter)
    }

    /// `connect_structured_push_consumer` (§4.7) — registers the sink in the
    /// channel; from now on it receives (filtered) all pushed events.
    ///
    /// # Errors
    /// `Disconnected` if the channel state is unreachable.
    pub fn connect_structured_push_consumer(
        &self,
        consumer: Arc<dyn StructuredPushConsumer>,
    ) -> Result<(), Disconnected> {
        let mut s = self.inner.state.lock().map_err(|_| Disconnected)?;
        s.push_links.push(PushLink {
            consumer,
            filter: Arc::clone(&self.filter),
        });
        Ok(())
    }
}

/// `StructuredProxyPullSupplier` (§4.9): the consumer pulls events here (filtered).
pub struct StructuredProxyPullSupplier {
    inner: Arc<ChannelInner>,
    filter: Arc<Filter>,
}

impl StructuredProxyPullSupplier {
    /// This pull proxy's filter.
    #[must_use]
    pub fn filter(&self) -> Arc<Filter> {
        Arc::clone(&self.filter)
    }

    /// `connect_structured_pull_consumer` (§4.9) — no-op connect.
    ///
    /// # Errors
    /// never.
    pub fn connect_structured_pull_consumer(&self) -> Result<(), ConnectError> {
        Ok(())
    }

    /// `try_pull_structured_event` (§4.9): next matching event without blocking;
    /// `bool` = `has_event`.
    ///
    /// # Errors
    /// `Disconnected`.
    pub fn try_pull_structured_event(&self) -> Result<(StructuredEvent, bool), Disconnected> {
        let mut s = self.inner.state.lock().map_err(|_| Disconnected)?;
        while let Some(ev) = s.pending.pop_front() {
            if self.filter.match_structured(&ev) {
                return Ok((ev, true));
            }
        }
        Ok((StructuredEvent::default(), false))
    }

    /// `pull_structured_event` (§4.9): non-blocking here (model); `Disconnected`
    /// if no matching event is currently available.
    ///
    /// # Errors
    /// `Disconnected`.
    pub fn pull_structured_event(&self) -> Result<StructuredEvent, Disconnected> {
        match self.try_pull_structured_event()? {
            (ev, true) => Ok(ev),
            (_, false) => Err(Disconnected),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::event::EventType;
    use crate::filter::ConstraintExp;
    use alloc::string::ToString;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use zerodds_cdr::{AnyValue, CorbaAny};

    struct Recorder {
        n: AtomicUsize,
        last: Mutex<Option<StructuredEvent>>,
    }
    impl StructuredPushConsumer for Recorder {
        fn push_structured_event(&self, event: StructuredEvent) -> Result<(), Disconnected> {
            self.n.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut g) = self.last.lock() {
                *g = Some(event);
            }
            Ok(())
        }
        fn disconnect(&self) {}
    }

    fn ev(t: &str, body: &str) -> StructuredEvent {
        StructuredEvent::new("D", t, "n", CorbaAny(AnyValue::Str(body.into())))
    }

    #[test]
    fn push_fanout_to_connected_consumer() {
        let ch = EventChannel::new();
        let rec = Arc::new(Recorder {
            n: AtomicUsize::new(0),
            last: Mutex::new(None),
        });
        let sup = ch.for_consumers().obtain_structured_push_supplier();
        sup.connect_structured_push_consumer(rec.clone()).unwrap();
        let pc = ch.for_suppliers().obtain_structured_push_consumer();
        pc.connect_structured_push_supplier().unwrap();
        pc.push_structured_event(ev("T", "hello"));
        assert_eq!(rec.n.load(Ordering::Relaxed), 1);
        assert_eq!(
            rec.last.lock().unwrap().as_ref().unwrap().remainder_of_body,
            CorbaAny(AnyValue::Str("hello".into()))
        );
    }

    #[test]
    fn pull_supplier_filters_by_event_type() {
        let ch = EventChannel::new();
        let pull = ch.for_consumers().obtain_structured_pull_supplier();
        pull.filter().add_constraints(alloc::vec![ConstraintExp {
            event_types: alloc::vec![EventType::new("D", "Wanted")],
            constraint_expr: alloc::string::String::new(),
        }]);
        let pc = ch.for_suppliers().obtain_structured_push_consumer();
        pc.push_structured_event(ev("Unwanted", "x"));
        pc.push_structured_event(ev("Wanted", "y"));
        // Only the matching event gets through.
        let (got, has) = pull.try_pull_structured_event().unwrap();
        assert!(has);
        assert_eq!(got.event_type().type_name, "Wanted");
        let (_, has2) = pull.try_pull_structured_event().unwrap();
        assert!(!has2, "queue empty (Unwanted filtered/consumed)");
    }

    #[test]
    fn factory_create_and_lookup() {
        let fac = EventChannelFactory::new();
        let (_ch, id) = fac.create_channel(QoSProperties::new());
        assert!(fac.all_channels().contains(&id));
        assert!(fac.get_event_channel(id).is_some());
        assert!(fac.get_event_channel(9999).is_none());
    }

    #[test]
    fn max_queue_length_caps_pending() {
        let mut qos = QoSProperties::new();
        qos.set(crate::qos::MAX_QUEUE_LENGTH, CorbaAny(AnyValue::ULong(2)));
        let ch = EventChannel::with_qos(qos);
        let pull = ch.for_consumers().obtain_structured_pull_supplier();
        let pc = ch.for_suppliers().obtain_structured_push_consumer();
        for i in 0..5 {
            pc.push_structured_event(ev("T", &alloc::format!("e{i}")));
        }
        // Only the last 2 remain.
        let mut got = Vec::new();
        while let Ok((e, true)) = pull.try_pull_structured_event() {
            if let AnyValue::Str(s) = &e.remainder_of_body.0 {
                got.push(s.clone());
            }
        }
        assert_eq!(got, alloc::vec!["e3".to_string(), "e4".to_string()]);
    }
}
