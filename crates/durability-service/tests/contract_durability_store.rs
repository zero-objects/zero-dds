// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The contract of `zerodds-durability-service` with
//! `zerodds-durability-store` (internal/release/rc8-test-suite.md, "Crate
//! tier"). What the service uses: `Contract::from_qos` on a discovered
//! writer's DURABILITY_SERVICE, `Contract::keep_all` and `effective_depth`
//! for the replay writer's history; a store behind `Arc<dyn
//! DurabilityStore>` that gives back each sample with the writer and
//! sequence number it came from (to keep a sample once) and its
//! representation identifier (to replay it under its header), counts what it
//! holds and keeps the topic metadata. The oracles: DDS 1.4 §2.2.3.5 (the
//! fields of DURABILITY_SERVICE and its default KEEP_LAST(1)) and DDS 1.4
//! §2.3.2 with DDSI-RTPS 2.5 §9.3.2 (a Duration's fraction in units of
//! 2^-32 s).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use zerodds_durability_store::{
    Contract, DurabilitySample, DurabilityStore, InMemoryStore, TopicMeta,
};
use zerodds_qos::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;

/// Each field of the policy is the contract's, the cleanup delay converted
/// from seconds and 2^-32 s (one half: 2^31) to a `Duration`.
#[test]
fn dcps_2_2_3_5_the_contract_is_the_writers_durability_service_policy() {
    let qos = DurabilityServiceQosPolicy {
        service_cleanup_delay: zerodds_qos::Duration::from_millis(30_500),
        history_kind: HistoryKind::KeepLast,
        history_depth: 7,
        max_samples: 100,
        max_instances: 42,
        max_samples_per_instance: 9,
    };
    assert_eq!(
        Contract::from_qos(&qos),
        Contract {
            history_kind: HistoryKind::KeepLast,
            history_depth: 7,
            max_samples: 100,
            max_instances: 42,
            max_samples_per_instance: 9,
            cleanup_delay: Duration::from_millis(30_500),
        }
    );
    // The policy's default: KEEP_LAST(1), unlimited, no delay.
    let default = Contract::default();
    assert_eq!(
        (
            default.history_kind,
            default.effective_depth(),
            default.max_samples
        ),
        (HistoryKind::KeepLast, 1, -1)
    );
    let all = Contract::keep_all();
    assert_eq!(
        (all.history_kind, all.cleanup_delay),
        (HistoryKind::KeepAll, Duration::ZERO)
    );
}

/// A sample comes back through the trait object with its source and its
/// identifier; the counts and the topic metadata follow what was stored.
#[test]
fn a_store_gives_back_a_sample_with_its_source_and_representation_identifier() {
    let store: Arc<dyn DurabilityStore> = Arc::new(InMemoryStore::default());
    let meta = TopicMeta {
        type_name: "Reading".into(),
        keyed: true,
        contract: Contract::keep_all(),
    };
    store.set_topic_meta("T", meta.clone()).unwrap();
    let sample = DurabilitySample {
        topic: "T".into(),
        instance_key: [7; 16],
        sequence: 0,
        payload: vec![1, 2, 3],
        representation_identifier: [0x00, 0x0a],
        created_at: SystemTime::UNIX_EPOCH,
        source_guid: [0xA1; 16],
        source_sequence: 4,
    };
    store.store(sample.clone()).unwrap();
    assert_eq!(store.replay_for_topic("T").unwrap(), vec![sample]);
    let stats = store.stats("T").unwrap();
    assert_eq!((stats.samples, stats.instances), (1, 1));
    assert_eq!(store.topic_meta("T").unwrap(), Some(meta));
}
