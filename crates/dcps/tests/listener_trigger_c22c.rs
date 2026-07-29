//! Integration tests for C2.2-c: hot-path triggers for all 12
//! listener status kinds. Spec §2.2.4.2.{4,5,6,7}.
//!
//! Per status kind at least 2 tests that show:
//! 1. The detector path increments the counter (runtime side),
//! 2. The listener bubble-up via `dispatch_*` fires correctly.
//!
//! Tests run offline (no UDP) — we drive the counters directly
//! through runtime state or via the dispatcher module.

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
    DataReaderListener, DataWriterListener, DomainParticipantListener, TopicListener,
};
use zerodds_dcps::psm_constants::status as bits;
use zerodds_dcps::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};
use zerodds_dcps::*;

// ----------------------------------------------------------------------------
// Counter listener for all 12 statuses.
// ----------------------------------------------------------------------------

#[derive(Default)]
struct ReaderC {
    requested_deadline: AtomicU32,
    requested_incompat: AtomicU32,
    liveliness_changed: AtomicU32,
    sample_lost: AtomicU32,
    sample_rejected: AtomicU32,
    last_policy: AtomicU32,
    last_lost_total: AtomicU32,
}

impl DataReaderListener for ReaderC {
    fn on_requested_deadline_missed(&self, _r: InstanceHandle, _s: RequestedDeadlineMissedStatus) {
        self.requested_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_requested_incompatible_qos(&self, _r: InstanceHandle, s: RequestedIncompatibleQosStatus) {
        self.requested_incompat.fetch_add(1, Ordering::Relaxed);
        self.last_policy.store(s.last_policy_id, Ordering::Relaxed);
    }
    fn on_liveliness_changed(&self, _r: InstanceHandle, _s: LivelinessChangedStatus) {
        self.liveliness_changed.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_lost(&self, _r: InstanceHandle, s: SampleLostStatus) {
        self.sample_lost.fetch_add(1, Ordering::Relaxed);
        self.last_lost_total
            .store(s.total_count as u32, Ordering::Relaxed);
    }
    fn on_sample_rejected(&self, _r: InstanceHandle, _s: SampleRejectedStatus) {
        self.sample_rejected.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct WriterC {
    offered_deadline: AtomicU32,
    offered_incompat: AtomicU32,
    liveliness_lost: AtomicU32,
    last_policy: AtomicU32,
}

impl DataWriterListener for WriterC {
    fn on_offered_deadline_missed(&self, _w: InstanceHandle, _s: OfferedDeadlineMissedStatus) {
        self.offered_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_offered_incompatible_qos(&self, _w: InstanceHandle, s: OfferedIncompatibleQosStatus) {
        self.offered_incompat.fetch_add(1, Ordering::Relaxed);
        self.last_policy.store(s.last_policy_id, Ordering::Relaxed);
    }
    fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
        self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct TopicC {
    inconsistent: AtomicU32,
}
impl TopicListener for TopicC {
    fn on_inconsistent_topic(&self, _t: InstanceHandle, s: InconsistentTopicStatus) {
        // Only count when delta > 0 (should already be filtered by the dispatcher).
        if s.total_count_change > 0 {
            self.inconsistent.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[derive(Default)]
struct PartC {
    inconsistent: AtomicU32,
    sub_matched: AtomicU32,
    pub_matched: AtomicU32,
    requested_deadline: AtomicU32,
    offered_deadline: AtomicU32,
    liveliness_lost: AtomicU32,
    liveliness_changed: AtomicU32,
    requested_incompat: AtomicU32,
    offered_incompat: AtomicU32,
    sample_lost: AtomicU32,
    sample_rejected: AtomicU32,
}
impl DomainParticipantListener for PartC {
    fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
        self.inconsistent.fetch_add(1, Ordering::Relaxed);
    }
    fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
        self.sub_matched.fetch_add(1, Ordering::Relaxed);
    }
    fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
        self.pub_matched.fetch_add(1, Ordering::Relaxed);
    }
    fn on_requested_deadline_missed(&self, _r: InstanceHandle, _s: RequestedDeadlineMissedStatus) {
        self.requested_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_offered_deadline_missed(&self, _w: InstanceHandle, _s: OfferedDeadlineMissedStatus) {
        self.offered_deadline.fetch_add(1, Ordering::Relaxed);
    }
    fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
        self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
    }
    fn on_liveliness_changed(&self, _r: InstanceHandle, _s: LivelinessChangedStatus) {
        self.liveliness_changed.fetch_add(1, Ordering::Relaxed);
    }
    fn on_requested_incompatible_qos(
        &self,
        _r: InstanceHandle,
        _s: RequestedIncompatibleQosStatus,
    ) {
        self.requested_incompat.fetch_add(1, Ordering::Relaxed);
    }
    fn on_offered_incompatible_qos(&self, _w: InstanceHandle, _s: OfferedIncompatibleQosStatus) {
        self.offered_incompat.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
        self.sample_lost.fetch_add(1, Ordering::Relaxed);
    }
    fn on_sample_rejected(&self, _r: InstanceHandle, _s: SampleRejectedStatus) {
        self.sample_rejected.fetch_add(1, Ordering::Relaxed);
    }
}

fn mk() -> (DomainParticipant, Topic<RawBytes>, Publisher, Subscriber) {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<RawBytes>("ChatterC22c", TopicQos::default())
        .expect("create_topic");
    let pubr = p.create_publisher(PublisherQos::default());
    let subr = p.create_subscriber(SubscriberQos::default());
    (p, t, pubr, subr)
}

// ============================================================================
// 1. on_inconsistent_topic
// ============================================================================

#[test]
fn inconsistent_topic_listener_fires_on_type_mismatch() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t1 = p
        .create_topic::<RawBytes>("DupT", TopicQos::default())
        .unwrap();
    let pc = Arc::new(TopicC::default());
    t1.set_listener(Some(pc.clone()), bits::ANY);
    // A second create_topic with the same name but a different type fails.
    use zerodds_dcps::dds_type::{DdsType, DecodeError, EncodeError};
    #[derive(Debug, Clone, Default)]
    struct Other;
    impl DdsType for Other {
        const TYPE_NAME: &'static str = "Other";
        const HAS_KEY: bool = false;
        fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
            Ok(())
        }
        fn decode(_: &[u8]) -> core::result::Result<Self, DecodeError> {
            Ok(Self)
        }
    }
    let _ = p.create_topic::<Other>("DupT", TopicQos::default());
    let s = t1.inconsistent_topic_status();
    assert!(s.total_count >= 1);
    assert_eq!(pc.inconsistent.load(Ordering::Relaxed), 1);
}

#[test]
fn inconsistent_topic_bubbles_to_participant_when_topic_listener_unset() {
    let p = DomainParticipantFactory::instance()
        .create_participant_offline(0, DomainParticipantQos::default());
    let t = p
        .create_topic::<RawBytes>("DupT2", TopicQos::default())
        .unwrap();
    let pc = Arc::new(PartC::default());
    p.set_listener(Some(pc.clone()), bits::ANY);
    // Manual trigger via record_inconsistent_topic.
    t.record_inconsistent_topic();
    let _ = t.inconsistent_topic_status();
    assert_eq!(pc.inconsistent.load(Ordering::Relaxed), 1);
}

#[test]
fn inconsistent_topic_no_delta_no_listener() {
    let (_, t, _, _) = mk();
    let pc = Arc::new(TopicC::default());
    t.set_listener(Some(pc.clone()), bits::ANY);
    // No increment → no listener.
    let _ = t.inconsistent_topic_status();
    let _ = t.inconsistent_topic_status();
    assert_eq!(pc.inconsistent.load(Ordering::Relaxed), 0);
}

// ============================================================================
// 2. on_offered_deadline_missed (Writer)
// ============================================================================

#[test]
fn offered_deadline_missed_via_runtime_counter() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
    };
    use zerodds_rtps::wire_types::GuidPrefix;
    // Tick period shorter than the deadline period so that under
    // CI container throttling (llvm-cov + serialized tests) enough
    // tick slots fit into the 250ms sleep window.
    let cfg = RuntimeConfig {
        tick_period: core::time::Duration::from_millis(5),
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([1; 12]), cfg).unwrap();
    let eid = rt
        .register_user_writer(UserWriterConfig {
            topic_name: "T".into(),
            type_name: "RawBytes".into(),
            reliable: false,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(50_i32),
            },
            lifespan: LifespanQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            ownership: OwnershipKind::Shared,
            ownership_strength: 0,
            presentation: Default::default(),
            partition: vec![],
            user_data: vec![],
            topic_data: vec![],
            group_data: vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            data_representation_offer: None,
        })
        .unwrap();
    // DDS 1.4 §2.2.3.1: deadline = maximum interval between *two*
    // consecutive writes. The counter only starts after the first
    // write — until then `last_write = None` is held.
    rt.write_user_sample(eid, b"first".to_vec())
        .expect("first write");
    // Then wait 250ms (= 5x deadline period) → at least one
    // miss tick.
    std::thread::sleep(std::time::Duration::from_millis(250));
    let n = rt.user_writer_offered_deadline_missed(eid);
    assert!(
        n > 0,
        "deadline counter expected > 0 after first write + sleep, got {n}"
    );
    rt.shutdown();
}

#[test]
fn offered_deadline_missed_listener_fires_on_delta() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let cnt = Arc::new(WriterC::default());
    w.set_listener(Some(cnt.clone()), bits::ANY);
    // In offline mode the counter stays at 0; we test the
    // delta-detection semantics directly via the dispatcher.
    use zerodds_dcps::listener_dispatch::{WriterListenerChain, dispatch_offered_deadline_missed};
    let chain = WriterListenerChain {
        writer: Some((cnt.clone(), bits::ANY)),
        publisher: None,
        participant: None,
    };
    dispatch_offered_deadline_missed(
        &chain,
        InstanceHandle::from_raw(7),
        OfferedDeadlineMissedStatus {
            total_count: 1,
            total_count_change: 1,
            last_instance_handle: InstanceHandle::from_raw(0),
        },
    );
    assert_eq!(cnt.offered_deadline.load(Ordering::Relaxed), 1);
}

// ============================================================================
// 3. on_requested_deadline_missed (Reader)
// ============================================================================

#[test]
fn requested_deadline_missed_via_runtime_counter() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig};
    use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
    use zerodds_rtps::wire_types::GuidPrefix;
    // See `offered_deadline_missed_via_runtime_counter` — tick=5ms
    // so the CI container deterministically manages ticks within the
    // 250ms window.
    let cfg = RuntimeConfig {
        tick_period: core::time::Duration::from_millis(5),
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([2; 12]), cfg).unwrap();
    let (eid, _rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "T".into(),
            type_name: "RawBytes".into(),
            reliable: false,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(50_i32),
            },
            liveliness: LivelinessQosPolicy::default(),
            ownership: OwnershipKind::Shared,
            presentation: Default::default(),
            partition: vec![],
            user_data: vec![],
            topic_data: vec![],
            group_data: vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let n = rt.user_reader_requested_deadline_missed(eid);
    assert!(n > 0);
    rt.shutdown();
}

#[test]
fn requested_deadline_missed_listener_fires_on_delta() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    // Direct dispatcher call — offline mode without a runtime counter.
    use zerodds_dcps::listener_dispatch::{
        ReaderListenerChain, dispatch_requested_deadline_missed,
    };
    let chain = ReaderListenerChain {
        reader: Some((cnt.clone(), bits::ANY)),
        subscriber: None,
        participant: None,
    };
    dispatch_requested_deadline_missed(
        &chain,
        InstanceHandle::from_raw(7),
        RequestedDeadlineMissedStatus {
            total_count: 2,
            total_count_change: 2,
            last_instance_handle: InstanceHandle::from_raw(0),
        },
    );
    assert_eq!(cnt.requested_deadline.load(Ordering::Relaxed), 1);
}

// ============================================================================
// 4. on_offered_incompatible_qos
// ============================================================================

#[test]
fn offered_incompatible_qos_default_policies_empty() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let s = w.offered_incompatible_qos_status();
    assert_eq!(s.total_count, 0);
    assert!(s.policies.is_empty());
}

#[test]
fn offered_incompatible_qos_dispatcher_fires_listener() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let cnt = Arc::new(WriterC::default());
    w.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{WriterListenerChain, dispatch_offered_incompatible_qos};
    use zerodds_dcps::psm_constants::qos_policy_id;
    use zerodds_dcps::status::QosPolicyCount;
    let chain = WriterListenerChain {
        writer: Some((cnt.clone(), bits::ANY)),
        publisher: None,
        participant: None,
    };
    dispatch_offered_incompatible_qos(
        &chain,
        InstanceHandle::from_raw(7),
        OfferedIncompatibleQosStatus {
            total_count: 1,
            total_count_change: 1,
            last_policy_id: qos_policy_id::DURABILITY,
            policies: vec![QosPolicyCount::new(qos_policy_id::DURABILITY, 1)],
        },
    );
    assert_eq!(cnt.offered_incompat.load(Ordering::Relaxed), 1);
    assert_eq!(
        cnt.last_policy.load(Ordering::Relaxed),
        qos_policy_id::DURABILITY
    );
}

// ============================================================================
// 5. on_requested_incompatible_qos
// ============================================================================

#[test]
fn requested_incompatible_qos_default_zero() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let s = r.requested_incompatible_qos_status();
    assert_eq!(s.total_count, 0);
}

#[test]
fn requested_incompatible_qos_dispatcher_fires_listener() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{
        ReaderListenerChain, dispatch_requested_incompatible_qos,
    };
    use zerodds_dcps::psm_constants::qos_policy_id;
    use zerodds_dcps::status::QosPolicyCount;
    let chain = ReaderListenerChain {
        reader: Some((cnt.clone(), bits::ANY)),
        subscriber: None,
        participant: None,
    };
    dispatch_requested_incompatible_qos(
        &chain,
        InstanceHandle::from_raw(7),
        RequestedIncompatibleQosStatus {
            total_count: 1,
            total_count_change: 1,
            last_policy_id: qos_policy_id::DEADLINE,
            policies: vec![QosPolicyCount::new(qos_policy_id::DEADLINE, 1)],
        },
    );
    assert_eq!(cnt.requested_incompat.load(Ordering::Relaxed), 1);
    assert_eq!(
        cnt.last_policy.load(Ordering::Relaxed),
        qos_policy_id::DEADLINE
    );
}

// ============================================================================
// 6. on_liveliness_lost (Writer)
// ============================================================================

#[test]
fn liveliness_lost_via_runtime_writer_lease() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
    use zerodds_qos::{
        DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessKind, LivelinessQosPolicy,
        OwnershipKind,
    };
    use zerodds_rtps::wire_types::GuidPrefix;
    // See `offered_deadline_missed_via_runtime_counter` — tick=5ms
    // under CI container throttling.
    let cfg = RuntimeConfig {
        tick_period: core::time::Duration::from_millis(5),
        ..RuntimeConfig::default()
    };
    let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([3; 12]), cfg).unwrap();
    let eid = rt
        .register_user_writer(UserWriterConfig {
            topic_name: "T".into(),
            type_name: "RawBytes".into(),
            reliable: false,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy::default(),
            lifespan: LifespanQosPolicy::default(),
            liveliness: LivelinessQosPolicy {
                kind: LivelinessKind::ManualByTopic,
                lease_duration: zerodds_qos::Duration::from_millis(50_i32),
            },
            ownership: OwnershipKind::Shared,
            ownership_strength: 0,
            presentation: Default::default(),
            partition: vec![],
            user_data: vec![],
            topic_data: vec![],
            group_data: vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            data_representation_offer: None,
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    let n = rt.user_writer_liveliness_lost(eid);
    assert!(n > 0, "liveliness_lost expected > 0, got {n}");
    rt.shutdown();
}

#[test]
fn liveliness_lost_dispatcher_fires_listener() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let cnt = Arc::new(WriterC::default());
    w.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{WriterListenerChain, dispatch_liveliness_lost};
    let chain = WriterListenerChain {
        writer: Some((cnt.clone(), bits::ANY)),
        publisher: None,
        participant: None,
    };
    dispatch_liveliness_lost(
        &chain,
        InstanceHandle::from_raw(7),
        LivelinessLostStatus {
            total_count: 1,
            total_count_change: 1,
        },
    );
    assert_eq!(cnt.liveliness_lost.load(Ordering::Relaxed), 1);
}

// ============================================================================
// 7. on_liveliness_changed (Reader)
// ============================================================================

#[test]
fn liveliness_changed_default_status_zero() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let (alive, ac, nc) = r.liveliness_changed_status();
    // Offline mode → no values.
    assert!(!alive);
    assert_eq!(ac, 0);
    assert_eq!(nc, 0);
}

#[test]
fn liveliness_changed_dispatcher_fires_listener() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{ReaderListenerChain, dispatch_liveliness_changed};
    let chain = ReaderListenerChain {
        reader: Some((cnt.clone(), bits::ANY)),
        subscriber: None,
        participant: None,
    };
    dispatch_liveliness_changed(
        &chain,
        InstanceHandle::from_raw(7),
        LivelinessChangedStatus {
            alive_count: 1,
            not_alive_count: 0,
            alive_count_change: 1,
            not_alive_count_change: 0,
            last_publication_handle: InstanceHandle::from_raw(0),
        },
    );
    assert_eq!(cnt.liveliness_changed.load(Ordering::Relaxed), 1);
}

// ============================================================================
// 8. on_sample_lost
// ============================================================================

#[test]
fn sample_lost_recorder_increments_counter() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig};
    use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
    use zerodds_rtps::wire_types::GuidPrefix;
    let cfg = RuntimeConfig::default();
    let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([4; 12]), cfg).unwrap();
    let (eid, _rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "T".into(),
            type_name: "RawBytes".into(),
            reliable: false,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            ownership: OwnershipKind::Shared,
            presentation: Default::default(),
            partition: vec![],
            user_data: vec![],
            topic_data: vec![],
            group_data: vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        })
        .unwrap();
    rt.record_sample_lost(eid, 3);
    rt.record_sample_lost(eid, 2);
    let n = rt.user_reader_sample_lost(eid);
    assert_eq!(n, 5);
    rt.shutdown();
}

#[test]
fn sample_lost_dispatcher_fires_listener() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{ReaderListenerChain, dispatch_sample_lost};
    let chain = ReaderListenerChain {
        reader: Some((cnt.clone(), bits::ANY)),
        subscriber: None,
        participant: None,
    };
    dispatch_sample_lost(
        &chain,
        InstanceHandle::from_raw(7),
        SampleLostStatus {
            total_count: 5,
            total_count_change: 5,
        },
    );
    assert_eq!(cnt.sample_lost.load(Ordering::Relaxed), 1);
    assert_eq!(cnt.last_lost_total.load(Ordering::Relaxed), 5);
}

// ============================================================================
// 9. on_sample_rejected
// ============================================================================

#[test]
fn sample_rejected_recorder_increments_counter() {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig};
    use zerodds_dcps::status::SampleRejectedStatusKind;
    use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
    use zerodds_rtps::wire_types::GuidPrefix;
    let cfg = RuntimeConfig::default();
    let rt = DcpsRuntime::start(0, GuidPrefix::from_bytes([5; 12]), cfg).unwrap();
    let (eid, _rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "T".into(),
            type_name: "RawBytes".into(),
            reliable: false,
            durability: DurabilityKind::Volatile,
            deadline: DeadlineQosPolicy::default(),
            liveliness: LivelinessQosPolicy::default(),
            ownership: OwnershipKind::Shared,
            presentation: Default::default(),
            partition: vec![],
            user_data: vec![],
            topic_data: vec![],
            group_data: vec![],
            type_identifier: zerodds_types::TypeIdentifier::None,
            type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
            data_representation_offer: None,
        })
        .unwrap();
    rt.record_sample_rejected(
        eid,
        SampleRejectedStatusKind::RejectedBySamplesLimit,
        InstanceHandle::from_raw(42),
    );
    let s = rt.user_reader_sample_rejected(eid);
    assert_eq!(s.total_count, 1);
    assert_eq!(
        s.last_reason,
        SampleRejectedStatusKind::RejectedBySamplesLimit
    );
    assert_eq!(s.last_instance_handle, InstanceHandle::from_raw(42));
    rt.shutdown();
}

#[test]
fn sample_rejected_dispatcher_fires_listener() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    use zerodds_dcps::listener_dispatch::{ReaderListenerChain, dispatch_sample_rejected};
    use zerodds_dcps::status::SampleRejectedStatusKind;
    let chain = ReaderListenerChain {
        reader: Some((cnt.clone(), bits::ANY)),
        subscriber: None,
        participant: None,
    };
    dispatch_sample_rejected(
        &chain,
        InstanceHandle::from_raw(7),
        SampleRejectedStatus {
            total_count: 1,
            total_count_change: 1,
            last_reason: SampleRejectedStatusKind::RejectedByInstancesLimit,
            last_instance_handle: InstanceHandle::from_raw(0),
        },
    );
    assert_eq!(cnt.sample_rejected.load(Ordering::Relaxed), 1);
}

// ============================================================================
// 10. Integration: drive_listeners + bubble-up to the participant.
// ============================================================================

#[test]
fn participant_receives_inconsistent_topic_via_bubble_up() {
    let (p, t, _, _) = mk();
    let pc = Arc::new(PartC::default());
    p.set_listener(Some(pc.clone()), bits::ANY);
    t.record_inconsistent_topic();
    let _ = t.inconsistent_topic_status();
    assert_eq!(pc.inconsistent.load(Ordering::Relaxed), 1);
}

#[test]
fn drive_listeners_no_op_offline_mode() {
    let (_, t, pubr, subr) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    // Offline mode: drive_listeners must not panic and fires
    // no listeners (all counters stay 0).
    let cnt_w = Arc::new(WriterC::default());
    let cnt_r = Arc::new(ReaderC::default());
    w.set_listener(Some(cnt_w.clone()), bits::ANY);
    r.set_listener(Some(cnt_r.clone()), bits::ANY);
    w.drive_listeners();
    r.drive_listeners();
    assert_eq!(cnt_w.offered_deadline.load(Ordering::Relaxed), 0);
    assert_eq!(cnt_r.requested_deadline.load(Ordering::Relaxed), 0);
}

// ============================================================================
// 11. Idempotenz / Delta-Detection.
// ============================================================================

#[test]
fn deadline_listener_does_not_fire_twice_for_same_count() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let cnt = Arc::new(WriterC::default());
    w.set_listener(Some(cnt.clone()), bits::ANY);
    // Two calls with the same counter (offline = 0) → no fire.
    let _ = w.offered_deadline_missed_count();
    let _ = w.offered_deadline_missed_count();
    assert_eq!(cnt.offered_deadline.load(Ordering::Relaxed), 0);
}

#[test]
fn liveliness_lost_listener_idempotent() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    let cnt = Arc::new(WriterC::default());
    w.set_listener(Some(cnt.clone()), bits::ANY);
    let _ = w.liveliness_lost_count();
    let _ = w.liveliness_lost_count();
    assert_eq!(cnt.liveliness_lost.load(Ordering::Relaxed), 0);
}

#[test]
fn requested_incompat_listener_only_on_change() {
    let (_, t, _, subr) = mk();
    let r = subr
        .create_datareader::<RawBytes>(&t, DataReaderQos::default())
        .unwrap();
    let cnt = Arc::new(ReaderC::default());
    r.set_listener(Some(cnt.clone()), bits::ANY);
    let _ = r.requested_incompatible_qos_status();
    let _ = r.requested_incompatible_qos_status();
    assert_eq!(cnt.requested_incompat.load(Ordering::Relaxed), 0);
}

// ============================================================================
// 12. Manual-Liveliness-Assert auf Writer (Spec §2.2.2.4.2.20).
// ============================================================================

#[test]
fn assert_liveliness_offline_no_op() {
    let (_, t, pubr, _) = mk();
    let w = pubr
        .create_datawriter::<RawBytes>(&t, DataWriterQos::default())
        .unwrap();
    // Offline → no-op, must not panic.
    w.assert_liveliness();
    assert_eq!(w.liveliness_lost_count(), 0);
}
