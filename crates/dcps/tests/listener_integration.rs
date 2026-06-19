//! Integration tests for C2.2-b: listener slot + bubble-up.
//!
//! Tests the interplay of the `set_listener`/`get_listener` API
//! with the `notify_data_arrived`/`poll_*_matched` hot-path
//! wiring. Spec §2.2.4.2.3 + §2.2.2.{2,3,4,5}.x.
//!
//! These tests all run **offline** (no UDP bind) — the
//! listener path is decoupled from the transport.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use zerodds_dcps::InstanceHandle;
use zerodds_dcps::listener::{
    DataReaderListener, DataWriterListener, DomainParticipantListener, PublisherListener,
    SubscriberListener, TopicListener,
};
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};
use zerodds_dcps::*;

// ============================================================================
// Test doubles — atomic counter per callback.
// ============================================================================

#[derive(Default)]
struct ReaderCounters {
    avail: AtomicU32,
    sub_matched: AtomicU32,
    lost: AtomicU32,
    rejected: AtomicU32,
    requested_deadline: AtomicU32,
    requested_incompat: AtomicU32,
    liveliness_changed: AtomicU32,
}

impl DataReaderListener for ReaderCounters {
    fn on_data_available(&self, _r: InstanceHandle) {
        self.avail.fetch_add(1, Ordering::Relaxed);
    }
    fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
        self.sub_matched.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
        self.lost.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_rejected(&self, _r: InstanceHandle, _s: SampleRejectedStatus) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
    }
    fn on_requested_deadline_missed(&self, _r: InstanceHandle, _s: RequestedDeadlineMissedStatus) {
        self.requested_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_requested_incompatible_qos(
        &self,
        _r: InstanceHandle,
        _s: RequestedIncompatibleQosStatus,
    ) {
        self.requested_incompat.fetch_add(1, Ordering::Relaxed);
    }
    fn on_liveliness_changed(&self, _r: InstanceHandle, _s: LivelinessChangedStatus) {
        self.liveliness_changed.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct SubscriberCounters {
    avail: AtomicU32,
    on_readers: AtomicU32,
    sub_matched: AtomicU32,
    lost: AtomicU32,
}

impl SubscriberListener for SubscriberCounters {
    fn on_data_available(&self, _r: InstanceHandle) {
        self.avail.fetch_add(1, Ordering::Relaxed);
    }
    fn on_data_on_readers(&self, _s: InstanceHandle) {
        self.on_readers.fetch_add(1, Ordering::Relaxed);
    }
    fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
        self.sub_matched.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
        self.lost.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct WriterCounters {
    pub_matched: AtomicU32,
    liveliness_lost: AtomicU32,
    offered_deadline: AtomicU32,
    offered_incompat: AtomicU32,
}

impl DataWriterListener for WriterCounters {
    fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
        self.pub_matched.fetch_add(1, Ordering::Relaxed);
    }
    fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
        self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
    }
    fn on_offered_deadline_missed(&self, _w: InstanceHandle, _s: OfferedDeadlineMissedStatus) {
        self.offered_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_offered_incompatible_qos(&self, _w: InstanceHandle, _s: OfferedIncompatibleQosStatus) {
        self.offered_incompat.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct PublisherCounters {
    pub_matched: AtomicU32,
}

impl PublisherListener for PublisherCounters {
    fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
        self.pub_matched.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct ParticipantCounters {
    avail: AtomicU32,
    on_readers: AtomicU32,
    pub_matched: AtomicU32,
    sub_matched: AtomicU32,
    inconsistent: AtomicU32,
    lost: AtomicU32,
}

impl DomainParticipantListener for ParticipantCounters {
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
    fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
        self.lost.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct TopicCounters {
    inconsistent: AtomicU32,
}

impl TopicListener for TopicCounters {
    fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
        self.inconsistent.fetch_add(1, Ordering::Relaxed);
    }
}

// ============================================================================
// Helper.
// ============================================================================

fn mk_setup() -> (DomainParticipant, Topic<RawBytes>, Publisher, Subscriber) {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<RawBytes>("Chatter", TopicQos::default())
        .expect("create_topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let subr = p.create_subscriber(SubscriberQos::default());
    (p, t, pubr, subr)
}

// ============================================================================
// API form: set_listener / get_listener on every entity.
// ============================================================================

#[test]
fn participant_set_get_listener_roundtrip() {
    let (p, _, _, _) = mk_setup();
    assert!(p.get_listener().is_none());
    p.set_listener(Some(Arc::new(ParticipantCounters::default())), bits::ANY);
    assert!(p.get_listener().is_some());
    p.set_listener(None, bits::NONE);
    assert!(p.get_listener().is_none());
}

#[test]
fn publisher_set_get_listener_roundtrip() {
    let (_, _, pubr, _) = mk_setup();
    assert!(pubr.get_listener().is_none());
    pubr.set_listener(Some(Arc::new(PublisherCounters::default())), bits::ANY);
    assert!(pubr.get_listener().is_some());
    pubr.set_listener(None, bits::NONE);
    assert!(pubr.get_listener().is_none());
}

#[test]
fn subscriber_set_get_listener_roundtrip() {
    let (_, _, _, subr) = mk_setup();
    assert!(subr.get_listener().is_none());
    subr.set_listener(Some(Arc::new(SubscriberCounters::default())), bits::ANY);
    assert!(subr.get_listener().is_some());
    subr.set_listener(None, bits::NONE);
    assert!(subr.get_listener().is_none());
}

#[test]
fn topic_set_get_listener_roundtrip() {
    let (_, t, _, _) = mk_setup();
    assert!(t.get_listener().is_none());
    t.set_listener(Some(Arc::new(TopicCounters::default())), bits::ANY);
    assert!(t.get_listener().is_some());
    t.set_listener(None, bits::NONE);
    assert!(t.get_listener().is_none());
}

#[test]
fn datawriter_set_get_listener_roundtrip() {
    let (_, t, pubr, _) = mk_setup();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    assert!(w.get_listener().is_none());
    w.set_listener(Some(Arc::new(WriterCounters::default())), bits::ANY);
    assert!(w.get_listener().is_some());
}

#[test]
fn datareader_set_get_listener_roundtrip() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    assert!(r.get_listener().is_none());
    r.set_listener(Some(Arc::new(ReaderCounters::default())), bits::ANY);
    assert!(r.get_listener().is_some());
}

// ============================================================================
// Hot path: data_available + data_on_readers via __push_raw.
// ============================================================================

#[test]
fn push_raw_fires_on_data_available_at_reader() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderCounters::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);

    r.__push_raw(vec![1, 2, 3]).unwrap();
    assert_eq!(cnt.avail.load(Ordering::Relaxed), 1);
}

#[test]
fn push_raw_fires_on_data_on_readers_at_subscriber() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let sc = Arc::new(SubscriberCounters::default());
    subr.set_listener(Some(sc.clone()), bits::ANY);

    r.__push_raw(vec![1, 2, 3]).unwrap();
    assert_eq!(sc.on_readers.load(Ordering::Relaxed), 1);
    // data_available bubbles up too — the reader has no listener,
    // the subscriber has the ANY mask, so sub.on_data_available fires too.
    assert_eq!(sc.avail.load(Ordering::Relaxed), 1);
}

#[test]
fn push_raw_bubbles_to_participant_when_others_have_no_listener() {
    let (p, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let pc = Arc::new(ParticipantCounters::default());
    p.set_listener(Some(pc.clone()), bits::ANY);

    r.__push_raw(vec![1]).unwrap();
    assert_eq!(pc.on_readers.load(Ordering::Relaxed), 1);
    assert_eq!(pc.avail.load(Ordering::Relaxed), 1);
}

#[test]
fn push_raw_with_zero_mask_at_reader_bubbles_to_subscriber() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let rc = Arc::new(ReaderCounters::default());
    let sc = Arc::new(SubscriberCounters::default());
    r.set_listener(Some(rc.clone()), 0); // Mask 0 → no bit consumed.
    subr.set_listener(Some(sc.clone()), bits::ANY);

    r.__push_raw(vec![1]).unwrap();
    assert_eq!(rc.avail.load(Ordering::Relaxed), 0);
    assert_eq!(sc.avail.load(Ordering::Relaxed), 1);
    assert_eq!(sc.on_readers.load(Ordering::Relaxed), 1);
}

#[test]
fn push_raw_with_only_data_available_bit_does_not_call_reader_for_others() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let rc = Arc::new(ReaderCounters::default());
    r.set_listener(Some(rc.clone()), bits::DATA_AVAILABLE);

    r.__push_raw(vec![1]).unwrap();
    assert_eq!(rc.avail.load(Ordering::Relaxed), 1);
    // Other bits (sub_matched) stay 0, because the mask doesn't cover them.
    assert_eq!(rc.sub_matched.load(Ordering::Relaxed), 0);
}

#[test]
fn no_listener_set_anywhere_is_safe() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    // No listeners at any stage — push must not panic.
    r.__push_raw(vec![1]).unwrap();
}

#[test]
fn multi_push_increments_listener_counter() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderCounters::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);

    for _ in 0..5 {
        r.__push_raw(vec![0]).unwrap();
    }
    assert_eq!(cnt.avail.load(Ordering::Relaxed), 5);
}

// ============================================================================
// data_on_readers consumes the subscriber stage; data_available bubbles
// up to the reader stage anyway (separate bits).
// ============================================================================

#[test]
fn data_available_independent_from_data_on_readers() {
    // The reader has ONLY data_on_readers in its mask (which would be
    // nonsensical, since that is a subscriber status) — then data_available
    // bubbles up to the subscriber.
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let rc = Arc::new(ReaderCounters::default());
    let sc = Arc::new(SubscriberCounters::default());
    r.set_listener(Some(rc.clone()), bits::DATA_ON_READERS); // unusual
    subr.set_listener(Some(sc.clone()), bits::ANY);

    r.__push_raw(vec![1]).unwrap();
    // The reader did not have the DATA_AVAILABLE bit → the subscriber
    // receives it.
    assert_eq!(rc.avail.load(Ordering::Relaxed), 0);
    assert_eq!(sc.avail.load(Ordering::Relaxed), 1);
}

// ============================================================================
// Listener-drop race: the listener is overwritten during the hot path.
// We test that this happens atomically (no crash).
// ============================================================================

#[test]
fn listener_replacement_during_dispatch_is_safe() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let first = Arc::new(ReaderCounters::default());
    r.set_listener(Some(first.clone()), bits::ANY);
    r.__push_raw(vec![1]).unwrap();
    assert_eq!(first.avail.load(Ordering::Relaxed), 1);

    // Swap the listener.
    let second = Arc::new(ReaderCounters::default());
    r.set_listener(Some(second.clone()), bits::ANY);
    r.__push_raw(vec![1]).unwrap();
    assert_eq!(first.avail.load(Ordering::Relaxed), 1); // unchanged
    assert_eq!(second.avail.load(Ordering::Relaxed), 1);

    // Remove the listener.
    r.set_listener(None, bits::NONE);
    r.__push_raw(vec![1]).unwrap();
    assert_eq!(first.avail.load(Ordering::Relaxed), 1);
    assert_eq!(second.avail.load(Ordering::Relaxed), 1);
}

// ============================================================================
// Bubble-up stop-on-hit: if the reader consumes the bit, it does not
// bubble up further.
// ============================================================================

#[test]
fn data_available_consumed_at_reader_does_not_bubble() {
    let (p, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let rc = Arc::new(ReaderCounters::default());
    let sc = Arc::new(SubscriberCounters::default());
    let pc = Arc::new(ParticipantCounters::default());
    r.set_listener(Some(rc.clone()), bits::ANY);
    subr.set_listener(Some(sc.clone()), bits::ANY);
    p.set_listener(Some(pc.clone()), bits::ANY);

    r.__push_raw(vec![1]).unwrap();
    assert_eq!(rc.avail.load(Ordering::Relaxed), 1);
    // data_available was consumed at the reader, does not bubble up.
    assert_eq!(sc.avail.load(Ordering::Relaxed), 0);
    assert_eq!(pc.avail.load(Ordering::Relaxed), 0);
    // BUT: data_on_readers is subscriber-only, has no reader
    // stage — it is consumed at the subscriber.
    assert_eq!(sc.on_readers.load(Ordering::Relaxed), 1);
    assert_eq!(pc.on_readers.load(Ordering::Relaxed), 0);
}

// ============================================================================
// Topic-Listener-Slot.
// ============================================================================

#[test]
fn topic_listener_can_be_attached_and_detached() {
    let (_, t, _, _) = mk_setup();
    let tc = Arc::new(TopicCounters::default());
    t.set_listener(Some(tc.clone()), bits::ANY);
    assert!(t.get_listener().is_some());
    t.set_listener(None, bits::NONE);
    assert!(t.get_listener().is_none());
}

// ============================================================================
// Listener replacement at subscriber + participant.
// ============================================================================

#[test]
fn subscriber_listener_replacement_works() {
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let s1 = Arc::new(SubscriberCounters::default());
    subr.set_listener(Some(s1.clone()), bits::ANY);
    r.__push_raw(vec![1]).unwrap();
    assert_eq!(s1.on_readers.load(Ordering::Relaxed), 1);

    let s2 = Arc::new(SubscriberCounters::default());
    subr.set_listener(Some(s2.clone()), bits::ANY);
    r.__push_raw(vec![2]).unwrap();
    assert_eq!(s1.on_readers.load(Ordering::Relaxed), 1);
    assert_eq!(s2.on_readers.load(Ordering::Relaxed), 1);
}

#[test]
fn participant_listener_replacement_works() {
    let (p, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let p1 = Arc::new(ParticipantCounters::default());
    p.set_listener(Some(p1.clone()), bits::ANY);
    r.__push_raw(vec![1]).unwrap();
    assert_eq!(p1.on_readers.load(Ordering::Relaxed), 1);

    let p2 = Arc::new(ParticipantCounters::default());
    p.set_listener(Some(p2.clone()), bits::ANY);
    r.__push_raw(vec![2]).unwrap();
    assert_eq!(p2.on_readers.load(Ordering::Relaxed), 1);
}

// ============================================================================
// Mixed: Reader + Subscriber + Participant, each with different
// bits, only one hit per event.
// ============================================================================

#[test]
fn three_stage_chain_each_gets_their_bit() {
    let (p, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let rc = Arc::new(ReaderCounters::default());
    let sc = Arc::new(SubscriberCounters::default());
    let pc = Arc::new(ParticipantCounters::default());

    // Reader does ONLY data_available; subscriber ONLY data_on_readers;
    // participant ALL.
    r.set_listener(Some(rc.clone()), bits::DATA_AVAILABLE);
    subr.set_listener(Some(sc.clone()), bits::DATA_ON_READERS);
    p.set_listener(Some(pc.clone()), bits::ANY);

    r.__push_raw(vec![1]).unwrap();
    assert_eq!(rc.avail.load(Ordering::Relaxed), 1);
    assert_eq!(sc.on_readers.load(Ordering::Relaxed), 1);
    // Both events were consumed at the narrower stages; the
    // participant must not have seen anything.
    assert_eq!(pc.avail.load(Ordering::Relaxed), 0);
    assert_eq!(pc.on_readers.load(Ordering::Relaxed), 0);
}

// ============================================================================
// A panic in the reader listener does not crash the push.
// ============================================================================

#[test]
fn panicking_listener_does_not_break_push() {
    struct Panicky;
    impl DataReaderListener for Panicky {
        fn on_data_available(&self, _r: InstanceHandle) {
            panic!("test-induced");
        }
    }
    let (_, t, _, subr) = mk_setup();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    r.set_listener(Some(Arc::new(Panicky)), bits::ANY);
    // If the panic slips through, the test fails.
    r.__push_raw(vec![1, 2, 3]).unwrap();
    // The inbox still contains the sample — push filled it
    // successfully before the listener threw.
    let n = r.take().unwrap().len();
    assert_eq!(n, 1);
}
