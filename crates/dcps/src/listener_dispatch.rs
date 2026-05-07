// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bubble-Up-Dispatcher fuer DCPS-Listener (Spec §2.2.4.2.3).
//!
//! Spec-Kern: Wenn auf der "kleinsten" Entity (Reader/Writer/Topic)
//! kein Listener gesetzt ist — oder dessen [`StatusMask`] das Bit
//! des aktuellen Status nicht enthaelt — bubblet das Event eine
//! Stufe weiter:
//!
//! ```text
//! DataReader  → Subscriber → DomainParticipant
//! DataWriter  → Publisher  → DomainParticipant
//! Topic       →             → DomainParticipant
//! ```
//!
//! Die Dispatcher-Funktionen in diesem Modul kapseln die Resolution
//! pro Status-Kind. Die wichtigsten Eigenschaften:
//!
//! 1. **Lock-Discipline**: Wir clonen den Listener-Arc unter dem
//!    Mutex, geben ihn frei und rufen den Callback aussen — sonst
//!    Deadlock-Risiko, wenn der Callback selbst auf andere Entities
//!    zugreift.
//! 2. **Panic-Safety**: Jeder Callback laeuft in
//!    [`std::panic::catch_unwind`]; ein panickender Listener kippt
//!    nicht den Reader-Tick um. Der Panic wird verschluckt — die
//!    Spec ist agnostisch zur Frage, was bei Listener-Panic passiert,
//!    aber Cyclone/Fast-DDS verhalten sich identisch.
//! 3. **Status-Bits**: Wir nutzen die Konstanten in
//!    [`crate::psm_constants::status`] zur Mask-Pruefung.
//!
//! Pro Status-Kind gibt es eine `dispatch_<event>`-Funktion, die der
//! Hot-Path-Code (Discovery-Match, Reader-Cache-Insert, ...) aufruft.

#![allow(clippy::module_name_repetitions)]

extern crate alloc;

use alloc::sync::Arc;

use crate::entity::StatusMask;
use crate::instance_handle::InstanceHandle;
use crate::listener::{
    DataReaderListener, DataWriterListener, DomainParticipantListener, PublisherListener,
    SubscriberListener, TopicListener,
};
use crate::psm_constants::status as bits;
use crate::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};

// ============================================================================
// Listener-Bundles — pro Hot-Path-Eintrag duplizieren wir die Listener-
// Klone hier, statt einzelne Referenzen herumzureichen. Damit haben
// die Dispatch-Funktionen einen klaren, kleinen Footprint.
// ============================================================================

/// Listener-Klone fuer einen DataWriter inkl. Bubble-Up-Targets.
/// Jedes Feld ist die Tupel `(Listener, Mask)`. Wenn ein Eintrag
/// `None` ist, gibt es auf der Stufe keinen Listener und das Event
/// bubblet weiter.
pub struct WriterListenerChain {
    /// DataWriter-Stufe.
    pub writer: Option<(Arc<dyn DataWriterListener>, StatusMask)>,
    /// Publisher-Stufe.
    pub publisher: Option<(Arc<dyn PublisherListener>, StatusMask)>,
    /// DomainParticipant-Stufe.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

/// Wie [`WriterListenerChain`], nur fuer Reader-Seite.
pub struct ReaderListenerChain {
    /// DataReader-Stufe.
    pub reader: Option<(Arc<dyn DataReaderListener>, StatusMask)>,
    /// Subscriber-Stufe.
    pub subscriber: Option<(Arc<dyn SubscriberListener>, StatusMask)>,
    /// DomainParticipant-Stufe.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

/// Bubble-Up-Kette eines Topics (Topic → Participant).
pub struct TopicListenerChain {
    /// Topic-Stufe.
    pub topic: Option<(Arc<dyn TopicListener>, StatusMask)>,
    /// DomainParticipant-Stufe.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

// ============================================================================
// Panic-Wrapper
// ============================================================================

/// Ruft den Closure-Body und verschluckt einen ggf. auftretenden
/// Panic. So crasht ein fehlerhafter Listener nicht den Reader-Tick.
///
/// Wir wrappen mit [`std::panic::AssertUnwindSafe`], weil `dyn`-
/// Listener-Traits keine [`std::panic::UnwindSafe`]-Bound haben —
/// der Listener-Body uebernimmt die Verantwortung fuer eigenen State,
/// und wir wollen Process-Safety, nicht Listener-internal-Safety.
#[inline]
fn safe_call<F: FnOnce()>(f: F) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

// ============================================================================
// Reader-Pfad — Reader → Subscriber → Participant
// ============================================================================

/// Pruefe-und-rufe-Helper fuer den Reader-Pfad. Liefert `true`, wenn
/// das Event auf einer Stufe konsumiert wurde.
#[inline]
fn try_call_reader_stage<F>(
    listener: &Option<(Arc<dyn DataReaderListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DataReaderListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_subscriber_stage<F>(
    listener: &Option<(Arc<dyn SubscriberListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn SubscriberListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_participant_stage<F>(
    listener: &Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DomainParticipantListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_writer_stage<F>(
    listener: &Option<(Arc<dyn DataWriterListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DataWriterListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_publisher_stage<F>(
    listener: &Option<(Arc<dyn PublisherListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn PublisherListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_topic_stage<F>(
    listener: &Option<(Arc<dyn TopicListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn TopicListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

// ============================================================================
// Reader-Pfad-Dispatcher
// ============================================================================

/// Bubble-Up fuer `on_data_available` (Reader → Subscriber → Participant).
/// Spec §2.2.4.2.6.1 + §2.2.4.2.3.
pub fn dispatch_data_available(chain: &ReaderListenerChain, reader: InstanceHandle) {
    if try_call_reader_stage(&chain.reader, bits::DATA_AVAILABLE, move |l| {
        l.on_data_available(reader);
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::DATA_AVAILABLE, move |l| {
        l.on_data_available(reader);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::DATA_AVAILABLE, move |l| {
        l.on_data_available(reader);
    });
}

/// Bubble-Up fuer `on_data_on_readers` (Subscriber → Participant).
/// Spec §2.2.4.2.7.1 — kein Reader-Stage, weil das Event Subscriber-
/// scoped ist.
pub fn dispatch_data_on_readers(chain: &ReaderListenerChain, subscriber: InstanceHandle) {
    if try_call_subscriber_stage(&chain.subscriber, bits::DATA_ON_READERS, move |l| {
        l.on_data_on_readers(subscriber);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::DATA_ON_READERS, move |l| {
        l.on_data_on_readers(subscriber);
    });
}

/// Bubble-Up fuer `on_subscription_matched`.
/// Spec §2.2.4.2.6.7.
pub fn dispatch_subscription_matched(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SubscriptionMatchedStatus,
) {
    if try_call_reader_stage(&chain.reader, bits::SUBSCRIPTION_MATCHED, move |l| {
        l.on_subscription_matched(reader, status);
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SUBSCRIPTION_MATCHED, move |l| {
        l.on_subscription_matched(reader, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::SUBSCRIPTION_MATCHED, move |l| {
        l.on_subscription_matched(reader, status);
    });
}

/// Bubble-Up fuer `on_sample_lost`. Spec §2.2.4.2.6.2.
pub fn dispatch_sample_lost(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SampleLostStatus,
) {
    if try_call_reader_stage(&chain.reader, bits::SAMPLE_LOST, move |l| {
        l.on_sample_lost(reader, status);
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SAMPLE_LOST, move |l| {
        l.on_sample_lost(reader, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::SAMPLE_LOST, move |l| {
        l.on_sample_lost(reader, status);
    });
}

/// Bubble-Up fuer `on_sample_rejected`. Spec §2.2.4.2.6.3.
pub fn dispatch_sample_rejected(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SampleRejectedStatus,
) {
    if try_call_reader_stage(&chain.reader, bits::SAMPLE_REJECTED, move |l| {
        l.on_sample_rejected(reader, status);
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SAMPLE_REJECTED, move |l| {
        l.on_sample_rejected(reader, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::SAMPLE_REJECTED, move |l| {
        l.on_sample_rejected(reader, status);
    });
}

/// Bubble-Up fuer `on_requested_deadline_missed`. Spec §2.2.4.2.6.4.
pub fn dispatch_requested_deadline_missed(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: RequestedDeadlineMissedStatus,
) {
    if try_call_reader_stage(&chain.reader, bits::REQUESTED_DEADLINE_MISSED, move |l| {
        l.on_requested_deadline_missed(reader, status);
    }) {
        return;
    }
    if try_call_subscriber_stage(
        &chain.subscriber,
        bits::REQUESTED_DEADLINE_MISSED,
        move |l| {
            l.on_requested_deadline_missed(reader, status);
        },
    ) {
        return;
    }
    let _ = try_call_participant_stage(
        &chain.participant,
        bits::REQUESTED_DEADLINE_MISSED,
        move |l| {
            l.on_requested_deadline_missed(reader, status);
        },
    );
}

/// Bubble-Up fuer `on_requested_incompatible_qos`. Spec §2.2.4.2.6.5.
pub fn dispatch_requested_incompatible_qos(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: RequestedIncompatibleQosStatus,
) {
    let s = status.clone();
    if try_call_reader_stage(&chain.reader, bits::REQUESTED_INCOMPATIBLE_QOS, move |l| {
        l.on_requested_incompatible_qos(reader, s);
    }) {
        return;
    }
    let s = status.clone();
    if try_call_subscriber_stage(
        &chain.subscriber,
        bits::REQUESTED_INCOMPATIBLE_QOS,
        move |l| {
            l.on_requested_incompatible_qos(reader, s);
        },
    ) {
        return;
    }
    let s = status;
    let _ = try_call_participant_stage(
        &chain.participant,
        bits::REQUESTED_INCOMPATIBLE_QOS,
        move |l| {
            l.on_requested_incompatible_qos(reader, s);
        },
    );
}

/// Bubble-Up fuer `on_liveliness_changed`. Spec §2.2.4.2.6.6.
pub fn dispatch_liveliness_changed(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: LivelinessChangedStatus,
) {
    if try_call_reader_stage(&chain.reader, bits::LIVELINESS_CHANGED, move |l| {
        l.on_liveliness_changed(reader, status);
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::LIVELINESS_CHANGED, move |l| {
        l.on_liveliness_changed(reader, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::LIVELINESS_CHANGED, move |l| {
        l.on_liveliness_changed(reader, status);
    });
}

// ============================================================================
// Writer-Pfad-Dispatcher
// ============================================================================

/// Bubble-Up fuer `on_publication_matched`. Spec §2.2.4.2.4.4.
pub fn dispatch_publication_matched(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: PublicationMatchedStatus,
) {
    if try_call_writer_stage(&chain.writer, bits::PUBLICATION_MATCHED, move |l| {
        l.on_publication_matched(writer, status);
    }) {
        return;
    }
    if try_call_publisher_stage(&chain.publisher, bits::PUBLICATION_MATCHED, move |l| {
        l.on_publication_matched(writer, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::PUBLICATION_MATCHED, move |l| {
        l.on_publication_matched(writer, status);
    });
}

/// Bubble-Up fuer `on_offered_deadline_missed`. Spec §2.2.4.2.4.1.
pub fn dispatch_offered_deadline_missed(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: OfferedDeadlineMissedStatus,
) {
    if try_call_writer_stage(&chain.writer, bits::OFFERED_DEADLINE_MISSED, move |l| {
        l.on_offered_deadline_missed(writer, status);
    }) {
        return;
    }
    if try_call_publisher_stage(&chain.publisher, bits::OFFERED_DEADLINE_MISSED, move |l| {
        l.on_offered_deadline_missed(writer, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(
        &chain.participant,
        bits::OFFERED_DEADLINE_MISSED,
        move |l| {
            l.on_offered_deadline_missed(writer, status);
        },
    );
}

/// Bubble-Up fuer `on_offered_incompatible_qos`. Spec §2.2.4.2.4.2.
pub fn dispatch_offered_incompatible_qos(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: OfferedIncompatibleQosStatus,
) {
    let s = status.clone();
    if try_call_writer_stage(&chain.writer, bits::OFFERED_INCOMPATIBLE_QOS, move |l| {
        l.on_offered_incompatible_qos(writer, s);
    }) {
        return;
    }
    let s = status.clone();
    if try_call_publisher_stage(&chain.publisher, bits::OFFERED_INCOMPATIBLE_QOS, move |l| {
        l.on_offered_incompatible_qos(writer, s);
    }) {
        return;
    }
    let s = status;
    let _ = try_call_participant_stage(
        &chain.participant,
        bits::OFFERED_INCOMPATIBLE_QOS,
        move |l| {
            l.on_offered_incompatible_qos(writer, s);
        },
    );
}

/// Bubble-Up fuer `on_liveliness_lost`. Spec §2.2.4.2.4.3.
pub fn dispatch_liveliness_lost(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: LivelinessLostStatus,
) {
    if try_call_writer_stage(&chain.writer, bits::LIVELINESS_LOST, move |l| {
        l.on_liveliness_lost(writer, status);
    }) {
        return;
    }
    if try_call_publisher_stage(&chain.publisher, bits::LIVELINESS_LOST, move |l| {
        l.on_liveliness_lost(writer, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::LIVELINESS_LOST, move |l| {
        l.on_liveliness_lost(writer, status);
    });
}

// ============================================================================
// Topic-Pfad-Dispatcher
// ============================================================================

/// Bubble-Up fuer `on_inconsistent_topic`. Spec §2.2.4.2.5.
pub fn dispatch_inconsistent_topic(
    chain: &TopicListenerChain,
    topic: InstanceHandle,
    status: InconsistentTopicStatus,
) {
    if try_call_topic_stage(&chain.topic, bits::INCONSISTENT_TOPIC, move |l| {
        l.on_inconsistent_topic(topic, status);
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::INCONSISTENT_TOPIC, move |l| {
        l.on_inconsistent_topic(topic, status);
    });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU32, Ordering};

    // ---------- Test-Doubles ----------

    struct CountingReader {
        avail: AtomicU32,
        lost: AtomicU32,
        matched: AtomicU32,
    }
    impl CountingReader {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                lost: AtomicU32::new(0),
                matched: AtomicU32::new(0),
            }
        }
    }
    impl DataReaderListener for CountingReader {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
            self.lost.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingSubscriber {
        avail: AtomicU32,
        on_readers: AtomicU32,
        lost: AtomicU32,
        matched: AtomicU32,
    }
    impl CountingSubscriber {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                on_readers: AtomicU32::new(0),
                lost: AtomicU32::new(0),
                matched: AtomicU32::new(0),
            }
        }
    }
    impl SubscriberListener for CountingSubscriber {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_data_on_readers(&self, _s: InstanceHandle) {
            self.on_readers.fetch_add(1, Ordering::Relaxed);
        }
        fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
            self.lost.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingParticipant {
        avail: AtomicU32,
        on_readers: AtomicU32,
        pub_matched: AtomicU32,
        sub_matched: AtomicU32,
        inconsistent: AtomicU32,
        liveliness_lost: AtomicU32,
    }
    impl CountingParticipant {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                on_readers: AtomicU32::new(0),
                pub_matched: AtomicU32::new(0),
                sub_matched: AtomicU32::new(0),
                inconsistent: AtomicU32::new(0),
                liveliness_lost: AtomicU32::new(0),
            }
        }
    }
    impl DomainParticipantListener for CountingParticipant {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_data_on_readers(&self, _s: InstanceHandle) {
            self.on_readers.fetch_add(1, Ordering::Relaxed);
        }
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.pub_matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.sub_matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
            self.inconsistent.fetch_add(1, Ordering::Relaxed);
        }
        fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
            self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingWriter {
        matched: AtomicU32,
        liveliness_lost: AtomicU32,
    }
    impl CountingWriter {
        fn new() -> Self {
            Self {
                matched: AtomicU32::new(0),
                liveliness_lost: AtomicU32::new(0),
            }
        }
    }
    impl DataWriterListener for CountingWriter {
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
            self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingPublisher {
        matched: AtomicU32,
    }
    impl CountingPublisher {
        fn new() -> Self {
            Self {
                matched: AtomicU32::new(0),
            }
        }
    }
    impl PublisherListener for CountingPublisher {
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingTopic {
        n: AtomicU32,
    }
    impl CountingTopic {
        fn new() -> Self {
            Self {
                n: AtomicU32::new(0),
            }
        }
    }
    impl TopicListener for CountingTopic {
        fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
            self.n.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn h() -> InstanceHandle {
        InstanceHandle::from_raw(7)
    }

    // ---------- Reader-Pfad ----------

    #[test]
    fn data_available_reader_consumes_when_mask_set() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 1);
        assert_eq!(s.avail.load(Ordering::Relaxed), 0);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_available_bubbles_to_subscriber_when_reader_mask_zero() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.avail.load(Ordering::Relaxed), 1);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_available_bubbles_all_the_way_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(p.avail.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn data_available_skipped_if_no_listener_anywhere() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        // No-Op: darf nicht panicken.
        dispatch_data_available(&chain, h());
    }

    #[test]
    fn data_available_skipped_if_all_masks_zero() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), 0)),
            participant: Some((p.clone(), 0)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.avail.load(Ordering::Relaxed), 0);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_on_readers_does_not_call_reader_stage() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_data_on_readers(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.on_readers.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn data_on_readers_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_on_readers(&chain, h());
        assert_eq!(p.on_readers.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn subscription_matched_consumed_at_reader() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_subscription_matched(&chain, h(), SubscriptionMatchedStatus::default());
        assert_eq!(r.matched.load(Ordering::Relaxed), 1);
        assert_eq!(s.matched.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn subscription_matched_bubbles_to_subscriber_then_participant() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());

        let chain1 = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_subscription_matched(&chain1, h(), SubscriptionMatchedStatus::default());
        assert_eq!(s.matched.load(Ordering::Relaxed), 1);
        assert_eq!(p.sub_matched.load(Ordering::Relaxed), 0);

        let chain2 = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_subscription_matched(&chain2, h(), SubscriptionMatchedStatus::default());
        assert_eq!(p.sub_matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_lost_chain() {
        let r = Arc::new(CountingReader::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: None,
            participant: None,
        };
        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        assert_eq!(r.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_lost_bubbles_to_subscriber() {
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        assert_eq!(s.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_rejected_dispatcher_runs_without_listener() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_sample_rejected(&chain, h(), SampleRejectedStatus::default());
    }

    #[test]
    fn requested_deadline_missed_dispatcher_runs() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_requested_deadline_missed(&chain, h(), RequestedDeadlineMissedStatus::default());
    }

    #[test]
    fn requested_incompatible_qos_dispatcher_clones_status() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_requested_incompatible_qos(&chain, h(), RequestedIncompatibleQosStatus::default());
    }

    #[test]
    fn liveliness_changed_dispatcher_runs() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_liveliness_changed(&chain, h(), LivelinessChangedStatus::default());
    }

    // ---------- Writer-Pfad ----------

    #[test]
    fn publication_matched_consumed_at_writer() {
        let w = Arc::new(CountingWriter::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), bits::ANY)),
            publisher: None,
            participant: None,
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn publication_matched_bubbles_to_publisher() {
        let w = Arc::new(CountingWriter::new());
        let pubr = Arc::new(CountingPublisher::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), 0)),
            publisher: Some((pubr.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 0);
        assert_eq!(pubr.matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn publication_matched_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(p.pub_matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn liveliness_lost_bubble_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_liveliness_lost(&chain, h(), LivelinessLostStatus::default());
        assert_eq!(p.liveliness_lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn offered_deadline_missed_dispatcher_runs() {
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: None,
        };
        dispatch_offered_deadline_missed(&chain, h(), OfferedDeadlineMissedStatus::default());
    }

    #[test]
    fn offered_incompatible_qos_dispatcher_clones_status() {
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: None,
        };
        dispatch_offered_incompatible_qos(&chain, h(), OfferedIncompatibleQosStatus::default());
    }

    // ---------- Topic-Pfad ----------

    #[test]
    fn inconsistent_topic_consumed_at_topic() {
        let t = Arc::new(CountingTopic::new());
        let chain = TopicListenerChain {
            topic: Some((t.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_inconsistent_topic(&chain, h(), InconsistentTopicStatus::default());
        assert_eq!(t.n.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn inconsistent_topic_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = TopicListenerChain {
            topic: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_inconsistent_topic(&chain, h(), InconsistentTopicStatus::default());
        assert_eq!(p.inconsistent.load(Ordering::Relaxed), 1);
    }

    // ---------- Panic-Safety ----------

    #[test]
    fn panic_in_listener_does_not_propagate() {
        struct Panicky;
        impl DataReaderListener for Panicky {
            fn on_data_available(&self, _r: InstanceHandle) {
                panic!("intentional panic in listener");
            }
        }
        let chain = ReaderListenerChain {
            reader: Some((Arc::new(Panicky), bits::ANY)),
            subscriber: None,
            participant: None,
        };
        // Wenn das nicht swallowed wuerde, panickt der ganze Test.
        dispatch_data_available(&chain, h());
    }

    #[test]
    fn panic_in_participant_listener_does_not_propagate() {
        struct Panicky;
        impl DomainParticipantListener for Panicky {
            fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
                panic!("boom");
            }
        }
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((Arc::new(Panicky), bits::ANY)),
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
    }

    // ---------- Bit-spezifisches Mask-Verhalten ----------

    #[test]
    fn reader_with_only_data_available_bit_passes_other_events_up() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::DATA_AVAILABLE)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 1);

        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        // Reader-Listener hat das Bit NICHT — bubblet zum Subscriber.
        assert_eq!(r.lost.load(Ordering::Relaxed), 0);
        assert_eq!(s.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn writer_with_only_publication_matched_bit_passes_other_events_up() {
        let w = Arc::new(CountingWriter::new());
        let pubr = Arc::new(CountingPublisher::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), bits::PUBLICATION_MATCHED)),
            publisher: Some((pubr.clone(), 0)),
            participant: None,
        };
        // PUBLICATION_MATCHED → Writer.
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 1);
        // LIVELINESS_LOST → Bubble durch (nirgendwo gesetzt).
        dispatch_liveliness_lost(&chain, h(), LivelinessLostStatus::default());
        assert_eq!(w.liveliness_lost.load(Ordering::Relaxed), 0);
    }
}
