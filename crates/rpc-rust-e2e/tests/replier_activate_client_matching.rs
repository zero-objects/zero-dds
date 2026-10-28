// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.6.2.2.1, Figure 7.3, for a replier called on arrival
//! (`Replier::activate`, §7.9.2.2 way 1): a request of a client whose reply
//! reader the reply writer has not matched yet is held, and "when the
//! service-side DataWriter completes the discovery of a client DataReader,
//! the service DataReader is ready to accept requests from that specific
//! client". The discovery that completes the match delivers no request, so a
//! replier that looks at its held requests only when a request arrives keeps
//! them.
//!
//! The client is built from DCPS entities, so that its reply reader can be
//! kept out of the match (another partition) while its request is written,
//! and let in afterwards without writing again.
//!
//! Linux only: multicast loopback on macOS is unreliable for SPDP.
#![cfg(target_os = "linux")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::Entity;
use zerodds_dcps::dds_type::RawBytes;
use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::participant::DomainParticipant;
use zerodds_dcps::qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos, PartitionQosPolicy, PublisherQos,
    SubscriberQos, TopicQos,
};
use zerodds_dcps::runtime::RpcDiscoveryExt;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::endpoint_options::RpcEndpointOptions;
use zerodds_rpc::qos_profile::RpcQos;
use zerodds_rpc::replier::{FnHandler, Replier, ReplierHandler};
use zerodds_rpc::topic_naming::{AnnotatedTopics, BindingStyle};

fn participant(domain: i32) -> DomainParticipant {
    DomainParticipantFactory::instance()
        .create_participant(domain, DomainParticipantQos::default())
        .expect("a participant")
}

fn eventually(within: Duration, cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

#[test]
fn an_activated_replier_answers_a_held_request_once_its_client_is_matched() {
    let lease = zerodds_test_domains::lease_domain();
    let service_side = participant(lease.id());
    let client_side = participant(lease.id());
    let handler: Arc<dyn ReplierHandler<RawBytes, RawBytes>> = Arc::new(FnHandler::new(
        |request: RawBytes| -> Result<RawBytes, RemoteExceptionCode> { Ok(request) },
    ));
    let replier = Arc::new(
        Replier::<RawBytes, RawBytes>::with_options(
            &service_side,
            "Held",
            "",
            &RpcQos::default_enhanced(),
            BindingStyle::FunctionCall,
            &AnnotatedTopics::none(),
            &RpcEndpointOptions::enhanced(),
            handler,
        )
        .expect("the replier"),
    );
    replier.activate();

    let request_topic = client_side
        .create_topic_with_type_name::<RawBytes>(
            replier.request_topic_name(),
            replier.request_type_name(),
            TopicQos::default(),
        )
        .unwrap();
    let reply_topic = client_side
        .create_topic_with_type_name::<RawBytes>(
            replier.reply_topic_name(),
            replier.reply_type_name(),
            TopicQos::default(),
        )
        .unwrap();
    // The reply reader starts in a partition the reply writer is not in.
    let subscriber = client_side.create_subscriber(SubscriberQos {
        partition: PartitionQosPolicy {
            names: vec!["later".into()],
        },
        ..SubscriberQos::default()
    });
    let reply_reader = subscriber
        .create_datareader::<RawBytes>(&reply_topic, DataReaderQos::default())
        .unwrap();
    let request_writer = client_side
        .create_publisher(PublisherQos::default())
        .create_datawriter::<RawBytes>(&request_topic, DataWriterQos::default())
        .unwrap();
    let reply_reader_guid = reply_reader.rtps_guid();
    assert!(reply_reader_guid.is_some(), "the reply reader has a GUID");
    // §7.6.2.1: the client-side endpoints name each other, as a requester's do.
    request_writer
        .set_rpc_discovery(&RpcDiscoveryExt {
            service_instance_name: None,
            related_entity_guid: reply_reader_guid,
            topic_aliases: None,
        })
        .unwrap();
    reply_reader
        .set_rpc_discovery(&RpcDiscoveryExt {
            service_instance_name: None,
            related_entity_guid: request_writer.rtps_guid(),
            topic_aliases: None,
        })
        .unwrap();
    request_writer
        .wait_for_matched_subscription(1, Duration::from_secs(10))
        .expect("the request writer meets the service");
    assert!(
        eventually(Duration::from_secs(10), || replier
            .__request_reader()
            .matched_rpc_writers()
            .iter()
            .any(|peer| peer.ext.related_entity_guid == reply_reader_guid)),
        "the service learns which reply reader the request writer names"
    );

    request_writer.write(&RawBytes::new(vec![7])).unwrap();
    assert!(
        eventually(Duration::from_secs(5), || replier.held_requests() == 1),
        "the request is held while the reply writer has not matched the client's reply reader"
    );
    assert_eq!(replier.handled_count(), 0);

    // The reply reader joins the partition of the reply writer: discovery
    // completes the client matching, and no request arrives with it.
    subscriber.set_qos(SubscriberQos::default()).unwrap();
    assert!(
        eventually(Duration::from_secs(10), || replier
            .__reply_writer()
            .matched_rpc_readers()
            .iter()
            .any(|peer| Some(peer.key) == reply_reader_guid)),
        "the reply writer matches the client's reply reader in the shared partition"
    );
    assert!(
        eventually(Duration::from_secs(10), || replier.handled_count() == 1),
        "the held request is answered once the client is matched (held {}, handled {})",
        replier.held_requests(),
        replier.handled_count()
    );
}
