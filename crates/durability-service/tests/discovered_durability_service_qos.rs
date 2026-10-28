// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! e2e: `enable_auto_discovery` derives the retention [`Contract`] from the
//! discovered writer's own `DurabilityServiceQosPolicy` (`PID_DURABILITY_SERVICE`
//! in its DATA(w), DDS 1.4 §2.2.3.5) — not the daemon's own default — whenever
//! that policy differs from the DDS default. Previously the daemon always
//! used its own `default_contract` and ignored the writer's announced policy
//! entirely (the policy was not even decoded off the wire).
//!
//! Own test binary = isolated process (process-global DDS state).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use zerodds_dcps::{
    DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes, TopicQos,
};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::{Contract, DurabilityStore};
use zerodds_durability_store_sqlite::SqliteStore;
use zerodds_qos::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

/// The writer's own `DurabilityServiceQosPolicy` — deliberately NOT the DDS
/// default (`KEEP_LAST(1)`, all caps unlimited), so it is unambiguous on the
/// wire that the writer set it on purpose.
fn writer_durability_service() -> DurabilityServiceQosPolicy {
    DurabilityServiceQosPolicy {
        service_cleanup_delay: zerodds_qos::Duration::ZERO,
        history_kind: HistoryKind::KeepLast,
        history_depth: 7,
        max_samples: LENGTH_UNLIMITED,
        max_instances: 42,
        max_samples_per_instance: LENGTH_UNLIMITED,
    }
}

#[test]
fn auto_discovery_derives_contract_from_writer_durability_service_qos() {
    let domain = zerodds_test_domains::unique_domain();
    let topic = "QosDerivedContract";
    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open_in_memory(keep_all()).unwrap());
    let service = Arc::new(DurabilityService::start(domain, Arc::clone(&store)).unwrap());
    // The daemon's OWN default — deliberately different from the writer's
    // announced policy, so a test that passed by coincidence (falling back
    // to the default) would be caught.
    service.enable_auto_discovery(keep_all()).unwrap();

    let _app_and_writer = {
        let factory = DomainParticipantFactory::instance();
        let app = factory
            .create_participant(domain, DomainParticipantQos::default())
            .unwrap();
        let publisher = app.create_publisher(PublisherQos::default());
        let topic_h = app
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        let mut qos = transient_service_writer_qos();
        qos.durability_service = writer_durability_service();
        let writer = publisher
            .create_datawriter::<RawBytes>(&topic_h, qos)
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(15);
        while service.contract_of(topic).is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        (app, writer)
    };

    let expected = Contract::from_qos(&writer_durability_service());
    let got = service
        .contract_of(topic)
        .expect("service should have resolved a contract for the discovered writer");
    assert_eq!(
        got, expected,
        "the contract must come from the writer's DurabilityServiceQosPolicy, \
         not the daemon's own default"
    );
    assert_ne!(
        got,
        keep_all(),
        "sanity: the derived contract must differ from the daemon's own default \
         (otherwise this test cannot tell the two apart)"
    );

    service.shutdown();
}
