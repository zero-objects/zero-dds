//! WP 1.4 T4 Akzeptanz: SEDP-Publications-Loopback
//!
//! `SedpPublicationsWriter::announce(p)` → transport channel →
//! `SedpPublicationsReader::handle_data` → `DiscoveredEndpointsCache`.
//! Expectation: the cache contains the announced publication byte-for-byte.
//!
//! Likewise for subscriptions.

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

use core::time::Duration;

use zerodds_discovery::sedp::{
    DiscoveredEndpointsCache, SedpPublicationsReader, SedpPublicationsWriter,
    SedpSubscriptionsReader, SedpSubscriptionsWriter,
};
use zerodds_rtps::datagram::{ParsedSubmessage, decode_datagram};
use zerodds_rtps::participant_data::Duration as DdsDuration;
use zerodds_rtps::publication_data::{
    DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
};
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};

const LOCAL_PREFIX: [u8; 12] = [1; 12];
const REMOTE_PREFIX: [u8; 12] = [2; 12];

fn local_reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        EntityId::SEDP_BUILTIN_PUBLICATIONS_READER,
    )
}

fn local_sub_reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER,
    )
}

fn sample_pub() -> PublicationBuiltinTopicData {
    PublicationBuiltinTopicData {
        key: Guid::new(
            GuidPrefix::from_bytes(REMOTE_PREFIX),
            EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes(REMOTE_PREFIX), EntityId::PARTICIPANT),
        topic_name: "ChatterTopic".into(),
        type_name: "std_msgs::String".into(),
        durability: DurabilityKind::TransientLocal,
        reliability: ReliabilityQos {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: DdsDuration::from_secs(10),
        },
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        presentation: zerodds_qos::PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_information: None,
        data_representation: Vec::new(),
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: Vec::new(),
        multicast_locators: Vec::new(),
    }
}

fn sample_sub() -> SubscriptionBuiltinTopicData {
    SubscriptionBuiltinTopicData {
        key: Guid::new(
            GuidPrefix::from_bytes(REMOTE_PREFIX),
            EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes(REMOTE_PREFIX), EntityId::PARTICIPANT),
        topic_name: "ChatterTopic".into(),
        type_name: "std_msgs::String".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQos::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        presentation: zerodds_qos::PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_information: None,
        data_representation: Vec::new(),
        content_filter: None,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: Vec::new(),
        multicast_locators: Vec::new(),
    }
}

#[test]
fn publication_announce_then_reader_fills_cache() {
    // Remote writer in participant REMOTE_PREFIX, local reader.
    let mut writer =
        SedpPublicationsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    // The writer must know the local reader as a proxy, otherwise no
    // fan-out. We simulate the T5 wiring manually.
    writer.add_reader_proxy(ReaderProxy::new(
        local_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        vec![],
        true,
    ));

    let mut reader = SedpPublicationsReader::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        VendorId::ZERODDS,
        GuidPrefix::from_bytes(REMOTE_PREFIX),
        vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
    );
    let mut cache = DiscoveredEndpointsCache::default();
    let now = Duration::from_secs(1);

    // Announce 1 publication
    let original = sample_pub();
    let dgs = writer.announce(&original).expect("announce");
    assert_eq!(dgs.len(), 1);

    // Loopback dispatch: unwrap datagram → DATA submessage → reader
    for dg in dgs {
        let parsed = decode_datagram(&dg.bytes).expect("decode");
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                let delta = reader
                    .handle_data(parsed.header.guid_prefix, &d)
                    .expect("handle_data");
                for s in delta.added {
                    cache.insert_publication(s, now);
                }
            }
        }
    }

    // The cache must contain the publication byte-for-byte.
    assert_eq!(cache.publications_len(), 1);
    let cached = cache.publication(original.key).expect("cached");
    assert_eq!(cached.data, original);
    assert_eq!(cached.discovered_at, now);
}

#[test]
fn multiple_publications_accumulate_in_cache() {
    let mut writer =
        SedpPublicationsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    writer.add_reader_proxy(ReaderProxy::new(
        local_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        vec![],
        true,
    ));
    let mut reader = SedpPublicationsReader::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        VendorId::ZERODDS,
        GuidPrefix::from_bytes(REMOTE_PREFIX),
        vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
    );
    let mut cache = DiscoveredEndpointsCache::default();
    let now = Duration::from_secs(1);

    // 3 distinct publications
    let topics = ["ChatterA", "ChatterB", "ChatterC"];
    for (i, topic) in topics.iter().enumerate() {
        let mut p = sample_pub();
        p.key = Guid::new(
            GuidPrefix::from_bytes(REMOTE_PREFIX),
            EntityId::user_writer_with_key([0x10 + i as u8, 0, 0]),
        );
        p.topic_name = (*topic).into();
        let dgs = writer.announce(&p).expect("announce");
        for dg in dgs {
            let parsed = decode_datagram(&dg.bytes).unwrap();
            for sub in parsed.submessages {
                if let ParsedSubmessage::Data(d) = sub {
                    for s in reader
                        .handle_data(parsed.header.guid_prefix, &d)
                        .unwrap()
                        .added
                    {
                        cache.insert_publication(s, now);
                    }
                }
            }
        }
    }

    assert_eq!(cache.publications_len(), 3);
    let by_topic: Vec<_> = cache
        .publications()
        .map(|p| p.data.topic_name.clone())
        .collect();
    assert!(by_topic.contains(&"ChatterA".to_string()));
    assert!(by_topic.contains(&"ChatterB".to_string()));
    assert!(by_topic.contains(&"ChatterC".to_string()));
}

#[test]
fn subscription_announce_then_reader_fills_cache() {
    let mut writer =
        SedpSubscriptionsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    writer.add_reader_proxy(ReaderProxy::new(
        local_sub_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        vec![],
        true,
    ));
    let mut reader = SedpSubscriptionsReader::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        VendorId::ZERODDS,
        GuidPrefix::from_bytes(REMOTE_PREFIX),
        vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
    );
    let mut cache = DiscoveredEndpointsCache::default();
    let now = Duration::from_secs(1);

    let original = sample_sub();
    for dg in writer.announce(&original).unwrap() {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                for s in reader
                    .handle_data(parsed.header.guid_prefix, &d)
                    .unwrap()
                    .added
                {
                    cache.insert_subscription(s, now);
                }
            }
        }
    }

    assert_eq!(cache.subscriptions_len(), 1);
    let cached = cache.subscription(original.key).expect("cached");
    assert_eq!(cached.data, original);
}

#[test]
fn matching_publications_and_subscriptions_by_topic() {
    // End-to-end matching: publication + subscription with the same
    // (topic, type) land in the cache; `match_publications/subscriptions`
    // returns them via the lookup.
    let mut p_writer =
        SedpPublicationsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    p_writer.add_reader_proxy(ReaderProxy::new(
        local_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        vec![],
        true,
    ));
    let mut s_writer =
        SedpSubscriptionsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    s_writer.add_reader_proxy(ReaderProxy::new(
        local_sub_reader_guid(),
        vec![Locator::udp_v4([127, 0, 0, 1], 7411)],
        vec![],
        true,
    ));

    let mut p_reader = SedpPublicationsReader::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        VendorId::ZERODDS,
        GuidPrefix::from_bytes(REMOTE_PREFIX),
        vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
    );
    let mut s_reader = SedpSubscriptionsReader::new(
        GuidPrefix::from_bytes(LOCAL_PREFIX),
        VendorId::ZERODDS,
        GuidPrefix::from_bytes(REMOTE_PREFIX),
        vec![Locator::udp_v4([127, 0, 0, 2], 7411)],
    );
    let mut cache = DiscoveredEndpointsCache::default();
    let now = Duration::from_secs(1);

    for dg in p_writer.announce(&sample_pub()).unwrap() {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                for s in p_reader
                    .handle_data(parsed.header.guid_prefix, &d)
                    .unwrap()
                    .added
                {
                    cache.insert_publication(s, now);
                }
            }
        }
    }
    for dg in s_writer.announce(&sample_sub()).unwrap() {
        let parsed = decode_datagram(&dg.bytes).unwrap();
        for sub in parsed.submessages {
            if let ParsedSubmessage::Data(d) = sub {
                for s in s_reader
                    .handle_data(parsed.header.guid_prefix, &d)
                    .unwrap()
                    .added
                {
                    cache.insert_subscription(s, now);
                }
            }
        }
    }

    let matching_pubs: Vec<_> = cache
        .match_publications("ChatterTopic", "std_msgs::String")
        .collect();
    let matching_subs: Vec<_> = cache
        .match_subscriptions("ChatterTopic", "std_msgs::String")
        .collect();
    assert_eq!(matching_pubs.len(), 1);
    assert_eq!(matching_subs.len(), 1);
}

#[test]
fn announce_without_reader_proxy_produces_no_datagrams() {
    // Sanity: without a registered remote reader there is nothing to send.
    let mut writer =
        SedpPublicationsWriter::new(GuidPrefix::from_bytes(REMOTE_PREFIX), VendorId::ZERODDS);
    let dgs = writer.announce(&sample_pub()).unwrap();
    assert!(dgs.is_empty());
}
