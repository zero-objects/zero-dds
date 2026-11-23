// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The QoS profiles of DDS for Lightweight CCM 1.1 (formal/2012-02-01):
//! the schema of Annex C and the default profile of Annex D, measured
//! against the reader of this crate.
//!
//! `xmllint` is the second implementation, as in `schema_validation.rs`:
//! it says which documents the schema of Annex C accepts and which it
//! turns down. The reader is asked with [`ReadOptions::accept_vendor_forms`],
//! the reading that takes the forms of DDS4CCM that DDS-XML 1.0 does not
//! have (a profile directly under `<dds>`, `<domainparticipant_qos>`, the
//! Boolean and hexadecimal forms of Table 7.1).
//!
//! The specification contradicts itself between the text of §7.4.2 and the
//! schema of Annex C. Every such place is a row of [`SCHEMA_INVALID`] with
//! the clause of the text: the schema turns the document down, the text
//! writes it, the reader reads it.
//!
//! Without `xmllint` the tests that need it print `[skip]` and return.

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_qos::{
    DeadlineQosPolicy, DestinationOrderKind, DestinationOrderQosPolicy, DurabilityKind,
    DurabilityQosPolicy, DurabilityServiceQosPolicy, Duration, EntityFactoryQosPolicy,
    GroupDataQosPolicy, HistoryKind, HistoryQosPolicy, LatencyBudgetQosPolicy, LifespanQosPolicy,
    LivelinessKind, LivelinessQosPolicy, OwnershipKind, OwnershipQosPolicy,
    OwnershipStrengthQosPolicy, PartitionQosPolicy, PresentationAccessScope, PresentationQosPolicy,
    ReaderDataLifecycleQosPolicy, ReaderQos, ReliabilityKind, ReliabilityQosPolicy,
    ResourceLimitsQosPolicy, TimeBasedFilterQosPolicy, TopicDataQosPolicy,
    TransportPriorityQosPolicy, UserDataQosPolicy, WriterDataLifecycleQosPolicy, WriterQos,
};
use zerodds_xml::{
    CCM_LIBRARY, DDS4CCM_DEFAULT_QOS_PROFILE_XML, DDS4CCM_QOS_PROFILE_NAMESPACE,
    DDS4CCM_QOS_PROFILE_XSD, DiagnosticKind, EntityQos, InitialResourceLimits, QosProfile,
    ReadOptions, XmlElement, XmlError, parse_dds_xml, parse_dds_xml_with, parse_xml_tree,
};

// ---------------------------------------------------------------------
// The validator
// ---------------------------------------------------------------------

/// Locates `xmllint` by running it; `ZERODDS_XMLLINT` overrides the program.
fn xmllint() -> Option<String> {
    let prog = std::env::var("ZERODDS_XMLLINT").unwrap_or_else(|_| "xmllint".into());
    match Command::new(&prog).arg("--version").output() {
        Ok(out) if out.status.success() => Some(prog),
        _ => None,
    }
}

macro_rules! xmllint_or_skip {
    () => {
        match xmllint() {
            Some(p) => p,
            None => {
                eprintln!("[skip] xmllint not runnable (set ZERODDS_XMLLINT to override)");
                return;
            }
        }
    };
}

fn schema() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas/dds4ccm_qos_profile.xsd");
    assert!(p.is_file(), "schema missing: {}", p.display());
    p
}

/// Validates `xml` against the schema of Annex C. `Err` carries what
/// `xmllint` wrote. The exit status is read from the process.
fn validate(prog: &str, name: &str, xml: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join("zerodds-xml-dds4ccm-profiles");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let doc = dir.join(format!("{name}.xml"));
    std::fs::write(&doc, xml).unwrap_or_else(|e| panic!("writing {}: {e}", doc.display()));
    let out = Command::new(prog)
        .arg("--nonet")
        .arg("--noout")
        .arg("--schema")
        .arg(schema())
        .arg(&doc)
        .output()
        .unwrap_or_else(|e| panic!("running {prog}: {e}"));
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The DDS4CCM reading: the profiles directly under `<dds>`, in document
/// order.
fn read(xml: &str) -> Result<Vec<QosProfile>, XmlError> {
    let parsed = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms())?;
    Ok(parsed
        .value
        .qos_libraries
        .into_iter()
        .filter(|l| l.name == CCM_LIBRARY)
        .flat_map(|l| l.profiles)
        .collect())
}

// ---------------------------------------------------------------------
// Annex C: the schema
// ---------------------------------------------------------------------

/// The names of the children of `parent` that are `<xs:{tag} name="…">`.
fn named<'a>(parent: &'a XmlElement, tag: &str) -> Vec<&'a str> {
    parent
        .children
        .iter()
        .filter(|c| c.name == tag)
        .filter_map(|c| c.attribute("name"))
        .collect()
}

/// The policy elements the schema declares in the `xs:all` of a QoS
/// container type, in the order of the annex.
fn policies_of(schema: &XmlElement, container_type: &str) -> Vec<String> {
    let ty = schema
        .children
        .iter()
        .find(|c| c.name == "complexType" && c.attribute("name") == Some(container_type))
        .unwrap_or_else(|| panic!("complexType {container_type} missing"));
    let all = ty.child("all").expect("xs:all of the container");
    named(all, "element")
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

/// The six QoS containers of Annex C: element name and type name.
const CONTAINERS: [(&str, &str); 6] = [
    ("datareader_qos", "datareaderQos"),
    ("datawriter_qos", "datawriterQos"),
    ("topic_qos", "topicQos"),
    ("domainparticipant_qos", "domainparticipantQos"),
    ("publisher_qos", "publisherQos"),
    ("subscriber_qos", "subscriberQos"),
];

/// Annex C — the file carries the type definitions of the annex and, added,
/// one global element: the root `<dds>` of §7.4.2.1.
#[test]
fn dds4ccm_annex_c_the_schema_file_has_the_types_of_the_annex_and_the_root_dds() {
    let doc = parse_xml_tree(DDS4CCM_QOS_PROFILE_XSD).expect("the schema is well-formed");
    let root = &doc.root;
    assert_eq!(root.name, "schema");
    assert_eq!(
        root.attribute("targetNamespace"),
        Some(DDS4CCM_QOS_PROFILE_NAMESPACE)
    );
    assert_eq!(root.attribute("elementFormDefault"), Some("qualified"));

    assert_eq!(
        named(root, "simpleType"),
        [
            "elementName",
            "topicNameFilter",
            "destinationOrderKind",
            "durabilityKind",
            "historyKind",
            "livelinessKind",
            "presentationAccessScopeKind",
            "reliabilityKind",
            "ownershipKind",
            "nonNegativeInteger_UNLIMITED",
            "nonNegativeInteger_Duration_SEC",
            "nonNegativeInteger_Duration_NSEC",
            "positiveInteger_UNLIMITED",
        ]
    );
    assert_eq!(
        named(root, "complexType"),
        [
            "duration",
            "stringSeq",
            "deadlineQosPolicy",
            "destinationOrderQosPolicy",
            "durabilityQosPolicy",
            "durabilityServiceQosPolicy",
            "entityFactoryQosPolicy",
            "groupDataQosPolicy",
            "historyQosPolicy",
            "latencyBudgetQosPolicy",
            "lifespanQosPolicy",
            "livelinessQosPolicy",
            "ownershipQosPolicy",
            "ownershipStrengthQosPolicy",
            "partitionQosPolicy",
            "presentationQosPolicy",
            "readerDataLifecycleQosPolicy",
            "reliabilityQosPolicy",
            "resourceLimitsQosPolicy",
            "timeBasedFilterQosPolicy",
            "topicDataQosPolicy",
            "transportPriorityQosPolicy",
            "userDataQosPolicy",
            "writerDataLifecycleQosPolicy",
            "domainparticipantQos",
            "publisherQos",
            "subscriberQos",
            "topicQos",
            "datareaderQos",
            "datawriterQos",
            "domainparticipantQosProfile",
            "topicQosProfile",
            "publisherQosProfile",
            "subscriberQosProfile",
            "datawriterQosProfile",
            "datareaderQosProfile",
            "qosProfile",
        ]
    );
    // The one declaration that is not in the annex.
    assert_eq!(named(root, "element"), ["dds"]);

    // The policies of each container, as the annex lists them.
    let expect: [(&str, &[&str]); 6] = [
        (
            "datareaderQos",
            &[
                "durability",
                "deadline",
                "latency_budget",
                "liveliness",
                "reliability",
                "destination_order",
                "history",
                "resource_limits",
                "user_data",
                "ownership",
                "time_based_filter",
                "reader_data_lifecycle",
            ],
        ),
        (
            "datawriterQos",
            &[
                "durability",
                "durability_service",
                "deadline",
                "latency_budget",
                "liveliness",
                "reliability",
                "destination_order",
                "history",
                "resource_limits",
                "transport_priority",
                "lifespan",
                "user_data",
                "ownership",
                "ownership_strength",
                "writer_data_lifecycle",
            ],
        ),
        (
            "topicQos",
            &[
                "topic_data",
                "durability",
                "durability_service",
                "deadline",
                "latency_budget",
                "liveliness",
                "reliability",
                "destination_order",
                "history",
                "resource_limits",
                "transport_priority",
                "lifespan",
                "ownership",
            ],
        ),
        ("domainparticipantQos", &["user_data", "entity_factory"]),
        (
            "publisherQos",
            &["presentation", "partition", "group_data", "entity_factory"],
        ),
        (
            "subscriberQos",
            &["presentation", "partition", "group_data", "entity_factory"],
        ),
    ];
    for (ty, policies) in expect {
        assert_eq!(policies_of(root, ty), policies, "{ty}");
    }
}

/// A document that sets every policy the schema declares, in each of the
/// six containers, with values that are not the defaults.
const EVERY_POLICY: &str = r#"<dds xmlns="http://www.omg.org/dds/">
<qos_profile name="Every Policy" base_name="Base">
  <datareader_qos name="reader" base_name="Base" topic_filter="Telemetry">
    <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
    <deadline><period><sec>2</sec><nanosec>500000000</nanosec></period></deadline>
    <latency_budget><duration><sec>0</sec><nanosec>250000000</nanosec></duration></latency_budget>
    <liveliness><kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind><lease_duration><sec>7</sec><nanosec>0</nanosec></lease_duration></liveliness>
    <reliability><kind>RELIABLE_RELIABILITY_QOS</kind><max_blocking_time><sec>3</sec><nanosec>0</nanosec></max_blocking_time></reliability>
    <destination_order><kind>BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS</kind></destination_order>
    <history><kind>KEEP_ALL_HISTORY_QOS</kind><depth>5</depth></history>
    <resource_limits><max_samples>100</max_samples><max_instances>10</max_instances><max_samples_per_instance>20</max_samples_per_instance><initial_samples>4</initial_samples><initial_instances>2</initial_instances></resource_limits>
    <user_data><value>aGk=</value></user_data>
    <ownership><kind>EXCLUSIVE_OWNERSHIP_QOS</kind></ownership>
    <time_based_filter><minimum_separation><sec>1</sec><nanosec>0</nanosec></minimum_separation></time_based_filter>
    <reader_data_lifecycle><autopurge_nowriter_samples_delay><sec>11</sec><nanosec>0</nanosec></autopurge_nowriter_samples_delay><autopurge_disposed_samples_delay><sec>12</sec><nanosec>0</nanosec></autopurge_disposed_samples_delay></reader_data_lifecycle>
  </datareader_qos>
  <datawriter_qos>
    <durability><kind>TRANSIENT_DURABILITY_QOS</kind></durability>
    <durability_service><service_cleanup_delay><sec>9</sec><nanosec>0</nanosec></service_cleanup_delay><history_kind>KEEP_ALL_HISTORY_QOS</history_kind><history_depth>6</history_depth><max_samples>60</max_samples><max_instances>LENGTH_UNLIMITED</max_instances><max_samples_per_instance>30</max_samples_per_instance></durability_service>
    <deadline><period><sec>DURATION_INFINITY</sec><nanosec>DURATION_INFINITY</nanosec></period></deadline>
    <latency_budget><duration><sec>4</sec></duration></latency_budget>
    <liveliness><kind>MANUAL_BY_PARTICIPANT_LIVELINESS_QOS</kind><lease_duration><sec>8</sec></lease_duration></liveliness>
    <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
    <destination_order><kind>BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS</kind></destination_order>
    <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history>
    <resource_limits><max_samples>LENGTH_UNLIMITED</max_samples><max_instances>3</max_instances><max_samples_per_instance>LENGTH_UNLIMITED</max_samples_per_instance></resource_limits>
    <transport_priority><value>5</value></transport_priority>
    <lifespan><duration><sec>13</sec><nanosec>0</nanosec></duration></lifespan>
    <user_data><value></value></user_data>
    <ownership><kind>EXCLUSIVE_OWNERSHIP_QOS</kind></ownership>
    <ownership_strength><value>42</value></ownership_strength>
    <writer_data_lifecycle><autodispose_unregistered_instances>false</autodispose_unregistered_instances></writer_data_lifecycle>
  </datawriter_qos>
  <topic_qos>
    <topic_data><value>AQID</value></topic_data>
    <durability><kind>PERSISTENT_DURABILITY_QOS</kind></durability>
    <durability_service><history_depth>2</history_depth></durability_service>
    <deadline><period><sec>1</sec></period></deadline>
    <latency_budget><duration><nanosec>1000</nanosec></duration></latency_budget>
    <liveliness><lease_duration><sec>5</sec></lease_duration></liveliness>
    <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
    <destination_order><kind>BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS</kind></destination_order>
    <history><depth>9</depth></history>
    <resource_limits><max_samples>1</max_samples></resource_limits>
    <transport_priority><value>0</value></transport_priority>
    <lifespan><duration><sec>DURATION_INFINITE_SEC</sec><nanosec>DURATION_INFINITE_NSEC</nanosec></duration></lifespan>
    <ownership><kind>SHARED_OWNERSHIP_QOS</kind></ownership>
  </topic_qos>
  <domainparticipant_qos>
    <user_data><value>/w==</value></user_data>
    <entity_factory><autoenable_created_entities>0</autoenable_created_entities></entity_factory>
  </domainparticipant_qos>
  <publisher_qos>
    <presentation><access_scope>GROUP_PRESENTATION_QOS</access_scope><coherent_access>true</coherent_access><ordered_access>1</ordered_access></presentation>
    <partition><name><element>north</element><element>south</element></name></partition>
    <group_data><value>Zw==</value></group_data>
    <entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory>
  </publisher_qos>
  <subscriber_qos>
    <presentation><access_scope>TOPIC_PRESENTATION_QOS</access_scope><coherent_access>false</coherent_access><ordered_access>true</ordered_access></presentation>
    <partition><name><element>east</element></name></partition>
    <group_data><value>c3Vi</value></group_data>
    <entity_factory><autoenable_created_entities>true</autoenable_created_entities></entity_factory>
  </subscriber_qos>
</qos_profile>
</dds>"#;

/// The children of the first `<{container}>` of the profile of
/// [`EVERY_POLICY`].
fn policies_written(container: &str) -> Vec<String> {
    let doc = parse_xml_tree(EVERY_POLICY).expect("well-formed");
    let profile = doc.root.child("qos_profile").expect("the profile");
    let el = profile.child(container).expect("the container");
    el.children.iter().map(|c| c.name.clone()).collect()
}

/// Annex C — every policy element the schema declares, in each of the six
/// containers: the schema accepts the document, the reader reads it, and
/// each field has the value the document gives it.
#[test]
fn dds4ccm_annex_c_what_the_schema_accepts_we_read() {
    // The document covers the schema: a policy added to the annex's file
    // and not to the document fails here, with or without xmllint.
    let schema_doc = parse_xml_tree(DDS4CCM_QOS_PROFILE_XSD).expect("schema");
    for (element, ty) in CONTAINERS {
        assert_eq!(
            policies_written(element),
            policies_of(&schema_doc.root, ty),
            "<{element}> of the test document against {ty} of the schema"
        );
    }

    let profiles = read(EVERY_POLICY).expect("the reader reads what the schema accepts");
    assert_eq!(profiles.len(), 1);
    let p = &profiles[0];
    assert_eq!(p.name, "Every Policy");
    assert_eq!(p.base_name.as_deref(), Some("Base"));

    let sec = Duration::from_secs;
    let sec_ns = Duration::from_sec_nanosec;

    // datareader_qos
    assert_eq!(p.datareader_qos.len(), 1);
    let slot = &p.datareader_qos[0];
    assert_eq!(slot.topic_filter.as_deref(), Some("Telemetry"));
    assert_eq!(slot.name.as_deref(), Some("reader"));
    let mut expect = EntityQos {
        base_name: Some("Base".into()),
        durability: Some(DurabilityQosPolicy {
            kind: DurabilityKind::TransientLocal,
        }),
        deadline: Some(DeadlineQosPolicy {
            period: sec_ns(2, 500_000_000),
        }),
        latency_budget: Some(LatencyBudgetQosPolicy {
            duration: sec_ns(0, 250_000_000),
        }),
        liveliness: Some(LivelinessQosPolicy {
            kind: LivelinessKind::ManualByTopic,
            lease_duration: sec(7),
        }),
        reliability: Some(ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: sec(3),
        }),
        destination_order: Some(DestinationOrderQosPolicy {
            kind: DestinationOrderKind::BySourceTimestamp,
        }),
        history: Some(HistoryQosPolicy {
            kind: HistoryKind::KeepAll,
            depth: 5,
        }),
        resource_limits: Some(ResourceLimitsQosPolicy {
            max_samples: 100,
            max_instances: 10,
            max_samples_per_instance: 20,
        }),
        user_data: Some(UserDataQosPolicy {
            value: b"hi".to_vec(),
        }),
        ownership: Some(OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        }),
        time_based_filter: Some(TimeBasedFilterQosPolicy {
            minimum_separation: sec(1),
        }),
        reader_data_lifecycle: Some(ReaderDataLifecycleQosPolicy {
            autopurge_nowriter_samples_delay: sec(11),
            autopurge_disposed_samples_delay: sec(12),
        }),
        ..EntityQos::default()
    };
    expect.initial_resource_limits = slot.qos.initial_resource_limits;
    assert_eq!(slot.qos, expect, "datareader_qos");
    let initial = slot.qos.initial_resource_limits.expect("initial limits");
    assert_eq!(
        (initial.initial_samples, initial.initial_instances),
        (Some(4), Some(2))
    );

    // datawriter_qos
    assert_eq!(p.datawriter_qos.len(), 1);
    let slot = &p.datawriter_qos[0];
    assert_eq!(
        (slot.topic_filter.as_deref(), slot.name.as_deref()),
        (None, None)
    );
    let reliability_default = ReliabilityQosPolicy::default();
    let expect = EntityQos {
        durability: Some(DurabilityQosPolicy {
            kind: DurabilityKind::Transient,
        }),
        durability_service: Some(DurabilityServiceQosPolicy {
            service_cleanup_delay: sec(9),
            history_kind: HistoryKind::KeepAll,
            history_depth: 6,
            max_samples: 60,
            max_instances: -1,
            max_samples_per_instance: 30,
        }),
        deadline: Some(DeadlineQosPolicy {
            period: Duration::INFINITE,
        }),
        latency_budget: Some(LatencyBudgetQosPolicy { duration: sec(4) }),
        liveliness: Some(LivelinessQosPolicy {
            kind: LivelinessKind::ManualByParticipant,
            lease_duration: sec(8),
        }),
        reliability: Some(ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: slot.qos.reliability.expect("reliability").max_blocking_time,
        }),
        destination_order: Some(DestinationOrderQosPolicy {
            kind: DestinationOrderKind::BySourceTimestamp,
        }),
        history: Some(HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 7,
        }),
        resource_limits: Some(ResourceLimitsQosPolicy {
            max_samples: -1,
            max_instances: 3,
            max_samples_per_instance: -1,
        }),
        transport_priority: Some(TransportPriorityQosPolicy { value: 5 }),
        lifespan: Some(LifespanQosPolicy { duration: sec(13) }),
        user_data: Some(UserDataQosPolicy { value: Vec::new() }),
        ownership: Some(OwnershipQosPolicy {
            kind: OwnershipKind::Exclusive,
        }),
        ownership_strength: Some(OwnershipStrengthQosPolicy { value: 42 }),
        writer_data_lifecycle: Some(WriterDataLifecycleQosPolicy {
            autodispose_unregistered_instances: false,
        }),
        ..EntityQos::default()
    };
    assert_eq!(slot.qos, expect, "datawriter_qos");
    // A field the document leaves out has the value DDS gives it.
    assert_eq!(
        slot.qos.reliability.expect("reliability").max_blocking_time,
        reliability_default.max_blocking_time
    );

    // topic_qos
    assert_eq!(p.topic_qos.len(), 1);
    let topic = &p.topic_qos[0].qos;
    assert_eq!(
        topic.topic_data,
        Some(TopicDataQosPolicy {
            value: vec![1, 2, 3]
        })
    );
    assert_eq!(
        topic.durability.map(|d| d.kind),
        Some(DurabilityKind::Persistent)
    );
    let service = topic.durability_service.expect("durability_service");
    assert_eq!(
        DurabilityServiceQosPolicy {
            history_depth: 2,
            ..DurabilityServiceQosPolicy::default()
        },
        service
    );
    assert_eq!(topic.deadline.map(|d| d.period), Some(sec(1)));
    assert_eq!(
        topic.latency_budget.map(|d| d.duration),
        Some(sec_ns(0, 1000))
    );
    assert_eq!(
        topic.liveliness,
        Some(LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: sec(5),
        })
    );
    assert_eq!(
        topic.reliability.map(|r| r.kind),
        Some(ReliabilityKind::Reliable)
    );
    assert_eq!(
        topic.destination_order.map(|d| d.kind),
        Some(DestinationOrderKind::ByReceptionTimestamp)
    );
    assert_eq!(
        topic.history,
        Some(HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 9,
        })
    );
    assert_eq!(
        topic.resource_limits,
        Some(ResourceLimitsQosPolicy {
            max_samples: 1,
            ..ResourceLimitsQosPolicy::default()
        })
    );
    assert_eq!(topic.transport_priority.map(|t| t.value), Some(0));
    assert_eq!(topic.lifespan.map(|l| l.duration), Some(Duration::INFINITE));
    assert_eq!(topic.ownership.map(|o| o.kind), Some(OwnershipKind::Shared));
    // Nothing the schema does not give a topic.
    assert!(topic.user_data.is_none() && topic.ownership_strength.is_none());
    assert!(topic.time_based_filter.is_none() && topic.presentation.is_none());

    // domainparticipant_qos
    assert_eq!(p.domainparticipant_qos.len(), 1);
    assert_eq!(
        p.domainparticipant_qos[0].qos,
        EntityQos {
            user_data: Some(UserDataQosPolicy { value: vec![0xff] }),
            entity_factory: Some(EntityFactoryQosPolicy {
                autoenable_created_entities: false,
            }),
            ..EntityQos::default()
        }
    );

    // publisher_qos
    assert_eq!(p.publisher_qos.len(), 1);
    assert_eq!(
        p.publisher_qos[0].qos,
        EntityQos {
            presentation: Some(PresentationQosPolicy {
                access_scope: PresentationAccessScope::Group,
                coherent_access: true,
                ordered_access: true,
            }),
            partition: Some(PartitionQosPolicy {
                names: vec!["north".into(), "south".into()],
            }),
            group_data: Some(GroupDataQosPolicy {
                value: b"g".to_vec(),
            }),
            entity_factory: Some(EntityFactoryQosPolicy {
                autoenable_created_entities: false,
            }),
            ..EntityQos::default()
        }
    );

    // subscriber_qos
    assert_eq!(p.subscriber_qos.len(), 1);
    assert_eq!(
        p.subscriber_qos[0].qos,
        EntityQos {
            presentation: Some(PresentationQosPolicy {
                access_scope: PresentationAccessScope::Topic,
                coherent_access: false,
                ordered_access: true,
            }),
            partition: Some(PartitionQosPolicy {
                names: vec!["east".into()],
            }),
            group_data: Some(GroupDataQosPolicy {
                value: b"sub".to_vec(),
            }),
            entity_factory: Some(EntityFactoryQosPolicy {
                autoenable_created_entities: true,
            }),
            ..EntityQos::default()
        }
    );

    // The oracle: the schema of Annex C accepts the document, and the other
    // shapes its root element and its profile type allow.
    let prog = xmllint_or_skip!();
    validate(&prog, "every_policy", EVERY_POLICY)
        .unwrap_or_else(|e| panic!("the schema turns the test document down:\n{e}"));
    for (name, xml, profiles_expected) in SCHEMA_VALID {
        validate(&prog, name, xml)
            .unwrap_or_else(|e| panic!("`{name}` is meant to be valid against Annex C:\n{e}"));
        let profiles = read(xml).unwrap_or_else(|e| {
            panic!("`{name}`: the schema accepts the document, the reader refuses it: {e:?}")
        });
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(&names, profiles_expected, "`{name}`");
    }
}

/// Further documents the schema accepts, with the names of the profiles the
/// reader makes of them.
const SCHEMA_VALID: &[(&str, &str, &[&str])] = &[
    (
        "empty_root",
        r#"<dds xmlns="http://www.omg.org/dds/"/>"#,
        &[],
    ),
    (
        "empty_profile",
        r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"/></dds>"#,
        &["P"],
    ),
    // §7.4.2.3.3: a single QoS stands for a profile of its name.
    (
        "single_qos",
        r#"<dds xmlns="http://www.omg.org/dds/"><datawriter_qos name="KeepAllWriter"><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos><topic_qos name="T"/><datareader_qos name="R"/><publisher_qos name="Pub"/><subscriber_qos name="Sub"/><domainparticipant_qos name="DP"/></dds>"#,
        &["KeepAllWriter", "T", "R", "Pub", "Sub", "DP"],
    ),
    // §7.4.2.3.1: inheritance, and the name pattern of the schema with a
    // space in it.
    (
        "inheritance",
        r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="Base Profile"/><qos_profile name="MyProfile" base_name="Base Profile"/></dds>"#,
        &["Base Profile", "MyProfile"],
    ),
    // The choice of qosProfile repeats: several containers of one kind,
    // in any order, each with the three attributes.
    (
        "repeated_containers",
        r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos topic_filter="A"/><topic_qos/><datawriter_qos topic_filter="B"/><domainparticipant_qos name="n" base_name="b" topic_filter="A"/><datawriter_qos/></qos_profile></dds>"#,
        &["P"],
    ),
    // The empty string is in every numeric pattern of the schema.
    (
        "empty_numbers",
        r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datareader_qos><deadline><period><sec></sec><nanosec></nanosec></period></deadline><resource_limits><max_samples></max_samples></resource_limits></datareader_qos></qos_profile></dds>"#,
        &["P"],
    ),
];

/// What this crate does with a document the schema of Annex C rejects.
#[derive(Debug, Clone, Copy)]
enum Verdict {
    /// The reader rejects it as well.
    Reject,
    /// The reader reads it; the string says why.
    Accept(&'static str),
}

struct Invalid {
    name: &'static str,
    /// What of the schema the document violates.
    rule: &'static str,
    xml: &'static str,
    verdict: Verdict,
}

/// Documents the schema of Annex C turns down. The `Accept` rows are the
/// places where the text of the specification and its schema disagree, or
/// where this crate reads more than the schema, each with its reason.
const SCHEMA_INVALID: &[Invalid] = &[
    Invalid {
        name: "unqualified_elements",
        rule: "elementFormDefault qualified, targetNamespace http://www.omg.org/dds/",
        xml: r#"<dds><qos_profile name="P"/></dds>"#,
        verdict: Verdict::Accept(
            "Every example of §7.4.2 is written without a namespace; only the document \
             of Annex D declares one. The reader takes the elements by their local name.",
        ),
    },
    Invalid {
        name: "topic_filter_expression",
        rule: "topicNameFilter is ([a-zA-Z0-9])+",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos topic_filter="A*"/></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "§7.4.2.3.2 writes topic_filter=\"A*\" and makes '*' the filter of a QoS \
             without the attribute; the pattern of the schema has no '*'.",
        ),
    },
    Invalid {
        name: "qualified_name",
        rule: "elementName is ([a-zA-Z0-9 ])+",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="Base_Profile"/><qos_profile name="P" base_name="Base_Profile"/></dds>"#,
        verdict: Verdict::Accept(
            "§7.4.2.2 identifies a named tag \"by its fully qualified name in C++ style\"; \
             the pattern of the schema has neither '_' nor ':'. Names are read as written.",
        ),
    },
    Invalid {
        name: "duration_zero_names",
        rule: "nonNegativeInteger_Duration_SEC and _NSEC name DURATION_INFINITY and \
               DURATION_INFINITE_* only",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datareader_qos><deadline><period><sec>DURATION_ZERO_SEC</sec><nanosec>DURATION_ZERO_NSEC</nanosec></period></deadline></datareader_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "§7.4.2.2.5 lists DURATION_ZERO_SEC and DURATION_ZERO_NSEC among the values \
             of a duration; the patterns of the schema leave them out.",
        ),
    },
    Invalid {
        name: "hexadecimal_history_depth",
        rule: "historyQosPolicy/depth is xs:positiveInteger",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datareader_qos><history><depth>0x10</depth></history></datareader_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "Table 7.1 gives a Long as \"-2147483648 to 2147483647 or 0x80000000 to \
             0x7fffffff\"; xs:positiveInteger has decimal digits only.",
        ),
    },
    Invalid {
        name: "hexadecimal_resource_limit",
        rule: "positiveInteger_UNLIMITED is LENGTH_UNLIMITED or decimal digits",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datareader_qos><resource_limits><max_samples>0x10</max_samples></resource_limits></datareader_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "ResourceLimitsQosPolicy::max_samples is a long of the DDS IDL and Table 7.1 \
             gives a Long the hexadecimal form; the pattern of the schema has decimal \
             digits only.",
        ),
    },
    Invalid {
        name: "negative_transport_priority",
        rule: "transportPriorityQosPolicy/value is xs:nonNegativeInteger",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos><transport_priority><value>-1</value></transport_priority></datawriter_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "§7.4.2.2.1 maps the fields \"1-to-1\" from the IDL of DDS, where \
             TransportPriorityQosPolicy::value is a long, and Table 7.1 gives a Long \
             -2147483648 to 2147483647; the schema types the value xs:nonNegativeInteger.",
        ),
    },
    Invalid {
        name: "participant_qos_tag",
        rule: "qosProfile declares domainparticipant_qos",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><participant_qos><entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory></participant_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "§7.4.2.2 lists the tag <participant_qos>; Annex C and Annex D write \
             <domainparticipant_qos>. Both are read into the same container.",
        ),
    },
    Invalid {
        name: "qos_library",
        rule: "the root holds qos_profile and the six QoS",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_library name="L"><qos_profile name="P"/></qos_library></dds>"#,
        verdict: Verdict::Accept(
            "<qos_library> is the form of DDS-XML 1.0 §7.3.2, which this reader reads \
             in the same document.",
        ),
    },
    Invalid {
        name: "unnamed_profile",
        rule: "qosProfile/@name is required",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile/></dds>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "policy_twice",
        rule: "xs:all: a policy at most once in a container",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos><history/><history/></datawriter_qos></qos_profile></dds>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "ownership_strength_in_a_reader",
        rule: "datareaderQos has no ownership_strength",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datareader_qos><ownership_strength><value>1</value></ownership_strength></datareader_qos></qos_profile></dds>"#,
        verdict: Verdict::Reject,
    },
    // §7.4.2.3.3 prints <writer_qos> in the profile its example is
    // "equivalent to". No other place of the specification has that tag.
    Invalid {
        name: "writer_qos_tag",
        rule: "qosProfile declares datawriter_qos",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="KeepAllWriter"><writer_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></writer_qos></qos_profile></dds>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "history_depth_zero",
        rule: "historyQosPolicy/depth is xs:positiveInteger",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos><history><depth>0</depth></history></datawriter_qos></qos_profile></dds>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "enum_as_number",
        rule: "historyKind enumerates the two names",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><datawriter_qos><history><kind>1</kind></history></datawriter_qos></qos_profile></dds>"#,
        verdict: Verdict::Reject,
    },
    Invalid {
        name: "boolean_yes",
        rule: "autoenable_created_entities is xs:boolean",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><publisher_qos><entity_factory><autoenable_created_entities>yes</autoenable_created_entities></entity_factory></publisher_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "Table 7.1 gives a Boolean the forms yes, 1, true, BOOLEAN_TRUE and no, 0, \
             false, BOOLEAN_FALSE; xs:boolean has true, false, 1 and 0.",
        ),
    },
    Invalid {
        name: "boolean_upper_case",
        rule: "autoenable_created_entities is xs:boolean",
        xml: r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><publisher_qos><entity_factory><autoenable_created_entities>TRUE</autoenable_created_entities></entity_factory></publisher_qos></qos_profile></dds>"#,
        verdict: Verdict::Accept(
            "Table 7.1 notes for a Boolean \"Not case-sensitive\"; xs:boolean is written \
             in lower case.",
        ),
    },
    // The six *QosProfile types restrict their base without repeating its
    // content model: their content is empty, a policy in them is invalid.
    // That is why the root element declares the single QoS of §7.4.2.3.3
    // with the base type (see the head of the schema file).
    Invalid {
        name: "qos_profile_type_has_empty_content",
        rule: "datawriterQosProfile restricts datawriterQos without a content model",
        xml: r#"<dds xmlns="http://www.omg.org/dds/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><datawriter_qos xsi:type="datawriterQosProfile" name="K"><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos></dds>"#,
        verdict: Verdict::Accept(
            "The document is the single QoS of §7.4.2.3.3; xsi:type only selects the \
             type of Annex C that cannot hold it. The reader does not read xsi:type.",
        ),
    },
];

/// Annex C — the rows of [`SCHEMA_INVALID`] are invalid for `xmllint`, and
/// the answer of the reader to each is the one recorded.
#[test]
fn dds4ccm_annex_c_what_the_schema_rejects_we_reject() {
    let prog = xmllint_or_skip!();
    let mut tolerated = 0;
    for case in SCHEMA_INVALID {
        assert!(
            validate(&prog, case.name, case.xml).is_err(),
            "case `{}` is meant to violate the schema ({}) and xmllint accepts it",
            case.name,
            case.rule
        );
        match (read(case.xml), case.verdict) {
            (Err(_), Verdict::Reject) => {}
            (Ok(_), Verdict::Accept(_)) => tolerated += 1,
            (Ok(_), Verdict::Reject) => panic!(
                "case `{}`: the schema rejects this document ({}), the reader reads it, \
                 and no reason is recorded",
                case.name, case.rule
            ),
            (Err(e), Verdict::Accept(reason)) => panic!(
                "case `{}` is recorded as read ({reason}) and the reader rejects it: {e:?}",
                case.name
            ),
        }
    }
    assert_eq!(
        tolerated, 12,
        "the number of documents read against the schema of Annex C changed; each needs \
         its reason"
    );

    // The type that cannot hold a policy does validate without one: the
    // row above measures the content model and not the xsi:type attribute.
    validate(
        &prog,
        "qos_profile_type_without_content",
        r#"<dds xmlns="http://www.omg.org/dds/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><datawriter_qos xsi:type="datawriterQosProfile" name="K"/></dds>"#,
    )
    .expect("datawriterQosProfile with its name and no content");
}

/// What the reader makes of the rows it reads against the schema: the value
/// the text of §7.4.2 gives the document.
#[test]
fn dds4ccm_7_4_2_the_forms_of_the_text_the_schema_rejects_have_their_values() {
    let by_name = |name: &str| -> Vec<QosProfile> {
        let case = SCHEMA_INVALID
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no case {name}"));
        read(case.xml).unwrap_or_else(|e| panic!("{name}: {e:?}"))
    };

    // §7.4.2.3.2
    let p = by_name("topic_filter_expression");
    assert_eq!(p[0].datawriter_qos[0].topic_filter.as_deref(), Some("A*"));
    // §7.4.2.2
    let p = by_name("qualified_name");
    assert_eq!(p[1].base_name.as_deref(), Some("Base_Profile"));
    // §7.4.2.2.5
    let p = by_name("duration_zero_names");
    assert_eq!(
        p[0].datareader_qos[0].qos.deadline.map(|d| d.period),
        Some(Duration::ZERO)
    );
    // Table 7.1
    let p = by_name("hexadecimal_history_depth");
    assert_eq!(
        p[0].datareader_qos[0].qos.history.map(|h| h.depth),
        Some(16)
    );
    // Table 7.1
    let p = by_name("hexadecimal_resource_limit");
    assert_eq!(
        p[0].datareader_qos[0]
            .qos
            .resource_limits
            .as_ref()
            .map(|r| r.max_samples),
        Some(16)
    );
    let p = by_name("negative_transport_priority");
    assert_eq!(
        p[0].datawriter_qos[0]
            .qos
            .transport_priority
            .as_ref()
            .map(|t| t.value),
        Some(-1)
    );
    let auto = |p: &[QosProfile]| {
        p[0].publisher_qos[0]
            .qos
            .entity_factory
            .as_ref()
            .map(|e| e.autoenable_created_entities)
    };
    assert_eq!(auto(&by_name("boolean_yes")), Some(true));
    assert_eq!(auto(&by_name("boolean_upper_case")), Some(true));
    // §7.4.2.2
    let p = by_name("participant_qos_tag");
    assert_eq!(
        p[0].domainparticipant_qos[0].qos.entity_factory,
        Some(EntityFactoryQosPolicy {
            autoenable_created_entities: false
        })
    );
    // §7.4.2.3.3
    let p = by_name("qos_profile_type_has_empty_content");
    assert_eq!(p[0].name, "K");
    assert_eq!(
        p[0].datawriter_qos[0].qos.history.map(|h| h.kind),
        Some(HistoryKind::KeepAll)
    );
}

/// §7.4.2.2.2 against Annex C — a sequence of octets. The text writes "a
/// sequence of decimal / hexadecimal values between 0..255 separated with
/// commas", the schema types the element `xs:base64Binary`. The reader takes
/// a content that has the shape of the list as the list, anything else as
/// Base64; a content that is both is the list of the text.
#[test]
fn dds4ccm_7_4_2_2_2_octets_are_the_list_of_the_text_or_the_base64_of_the_schema() {
    let value = |text: &str| -> Vec<u8> {
        let xml = format!(
            "<dds><qos_profile name=\"P\"><datareader_qos><user_data><value>{text}</value>\
             </user_data></datareader_qos></qos_profile></dds>"
        );
        let p = read(&xml).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        p[0].datareader_qos[0]
            .qos
            .user_data
            .clone()
            .expect("user_data")
            .value
    };
    // The two examples of §7.4.2.2.2.
    assert_eq!(value("100,200,0,0,0,223"), [100, 200, 0, 0, 0, 223]);
    assert_eq!(
        value("0xff,0x00,0x8e,0xEE,0x78"),
        [0xff, 0x00, 0x8e, 0xee, 0x78]
    );
    // Base64, the type of Annex C.
    assert_eq!(value("aGk="), b"hi");
    // Four characters that are a list of one octet and three bytes of
    // Base64: the list.
    assert_eq!(value("0x41"), [0x41]);
    // Digits that are no octet: Base64.
    assert_eq!(value("1234"), [0xd7, 0x6d, 0xf8]);
    assert_eq!(value(""), [0u8; 0]);
}

/// §7.4.2.3 — the forms of DDS4CCM are not forms of DDS-XML 1.0: the default
/// reading refuses a profile directly under `<dds>` and the container
/// `<domainparticipant_qos>` and names them; the reading that accepts them
/// reports each one.
#[test]
fn dds4ccm_7_4_2_3_the_forms_of_dds4ccm_are_read_on_request_and_reported() {
    let xml = r#"<dds xmlns="http://www.omg.org/dds/"><qos_profile name="P"><domainparticipant_qos/></qos_profile></dds>"#;
    let refused = parse_dds_xml(xml).expect_err("DDS-XML 1.0 has no profile under <dds>");
    assert!(matches!(refused, XmlError::VendorForm(_)), "{refused:?}");

    let parsed = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms()).expect("accepted");
    let kinds: Vec<DiagnosticKind> = parsed.diagnostics.iter().map(|d| d.kind).collect();
    assert_eq!(
        kinds,
        [DiagnosticKind::VendorForm, DiagnosticKind::VendorForm],
        "{:?}",
        parsed.diagnostics
    );
    assert!(
        parsed.diagnostics[0]
            .message
            .contains("directly under <dds>")
    );
    assert!(
        parsed.diagnostics[1]
            .message
            .contains("<domainparticipant_qos>")
    );
}

/// §7.4.2.2 and §7.4.2.3 with the examples of the text, in the form of
/// DDS4CCM (profiles and single QoS directly under `<dds>`): a QoS inherits
/// by `base_name` (§7.4.2.2), a profile inherits from a profile read before
/// it and from no later one (§7.4.2.3.1), `topic_filter` selects among the
/// QoS of one kind, and one without the attribute has the filter `*` and
/// comes after the others (§7.4.2.3.2), a single QoS stands for a profile
/// of its name (§7.4.2.3.3).
#[test]
fn dds4ccm_7_4_2_3_profiles_inherit_select_by_topic_filter_and_stand_for_a_single_qos() {
    let xml = r#"<dds>
      <datawriter_qos name="BaseWriterQos">
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>3</depth></history>
      </datawriter_qos>
      <datawriter_qos name="DerivedWriterQos" base_name="BaseWriterQos">
        <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
      </datawriter_qos>
      <qos_profile name="BaseProfile">
        <datareader_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability></datareader_qos>
      </qos_profile>
      <qos_profile name="StrictReliableCommunicationProfile" base_name="BaseProfile">
        <datawriter_qos><history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history></datawriter_qos>
        <datawriter_qos topic_filter="A*">
          <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
          <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        </datawriter_qos>
        <datawriter_qos topic_filter="B*">
          <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
          <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          <resource_limits>
            <max_samples>128</max_samples>
            <max_samples_per_instance>128</max_samples_per_instance>
            <initial_samples>128</initial_samples>
            <max_instances>1</max_instances>
            <initial_instances>1</initial_instances>
          </resource_limits>
        </datawriter_qos>
        <datawriter_qos topic_filter="A?c"><history><depth>9</depth></history></datawriter_qos>
      </qos_profile>
      <qos_profile name="Early" base_name="Late"/>
      <qos_profile name="Late"/>
    </dds>"#;
    let libraries = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms())
        .expect("the document reads")
        .value
        .qos_libraries;
    let resolve = |name: &str| zerodds_xml::resolve_profile(&libraries, &format!("(dds)::{name}"));

    // §7.4.2.2: "the writer QoS named 'DerivedWriterQos' inherits the values
    // from the writer QoS 'BaseWriterQos.' The HistoryQosPolicy kind is set
    // to KEEP_ALL_HISTORY_QOS."
    let derived = resolve("DerivedWriterQos").expect("the base was read before");
    let writer = zerodds_xml::default_qos(&derived.datawriter_qos).expect("the writer QoS");
    assert_eq!(
        writer.reliability.map(|r| r.kind),
        Some(ReliabilityKind::BestEffort)
    );
    assert_eq!(writer.history.map(|h| h.kind), Some(HistoryKind::KeepAll));

    // §7.4.2.3.1: the profile has the QoS of its base.
    let profile = resolve("StrictReliableCommunicationProfile").expect("inherits");
    let reader = zerodds_xml::default_qos(&profile.datareader_qos).expect("inherited reader QoS");
    assert_eq!(
        reader.reliability.map(|r| r.kind),
        Some(ReliabilityKind::Reliable)
    );
    // "A QoS profile cannot inherit from other QoS profiles if the last one
    // has not been parsed before."
    let early = resolve("Early").expect_err("the base stands after the profile");
    assert!(
        matches!(&early, XmlError::InvalidXml(message) if message.contains("defined later")),
        "{early:?}"
    );

    // §7.4.2.3.2: the filters "will be evaluated in order; they have
    // precedence over a QoS without a topic_filter expression", which has
    // the filter '*'.
    let history = |topic: &str| {
        let qos = profile
            .datawriter_qos_for_topic(topic)
            .unwrap_or_else(|| panic!("no writer QoS for {topic}"));
        qos.history.expect("history")
    };
    assert_eq!(history("Alpha").kind, HistoryKind::KeepAll);
    // "A*" stands before "A?c" and matches first.
    assert_eq!(history("Abc").kind, HistoryKind::KeepAll);
    assert_eq!(
        profile
            .datawriter_qos_for_topic("Beta")
            .and_then(|qos| qos.resource_limits)
            .map(|limits| (limits.max_samples, limits.max_instances)),
        Some((128, 1))
    );
    // No filter matches: the QoS without the attribute, although it is the
    // first in the document.
    assert_eq!(
        (history("Gamma").kind, history("Gamma").depth),
        (HistoryKind::KeepLast, 7)
    );

    // §7.4.2.3.3: the single QoS "is equivalent to" the profile of its
    // name with that QoS.
    let single = read(
        r#"<dds><datawriter_qos name="KeepAllWriter"><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos></dds>"#,
    )
    .expect("the single QoS");
    let written_out = read(
        r#"<dds><qos_profile name="KeepAllWriter"><datawriter_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos></qos_profile></dds>"#,
    )
    .expect("the profile");
    assert_eq!(single[0].name, written_out[0].name);
    assert_eq!(
        single[0].datawriter_qos[0].qos,
        written_out[0].datawriter_qos[0].qos
    );
    assert_eq!(single[0].datawriter_qos.len(), 1);
    assert!(single[0].datareader_qos.is_empty() && single[0].topic_qos.is_empty());
}

// ---------------------------------------------------------------------
// Annex D: the default profile
// ---------------------------------------------------------------------

/// Annex D — the document of the annex is valid against the schema of
/// Annex C.
#[test]
fn dds4ccm_annex_d_the_default_profile_is_valid_against_the_schema_of_annex_c() {
    let prog = xmllint_or_skip!();
    validate(&prog, "annex_d", DDS4CCM_DEFAULT_QOS_PROFILE_XML)
        .unwrap_or_else(|e| panic!("Annex D against Annex C:\n{e}"));
}

/// Annex D — "a XML QoS Profile with all default values as specified in
/// DDS": the document sets every policy of the six containers, and each
/// value the reader makes of it is the default of `zerodds_qos`.
#[test]
fn dds4ccm_annex_d_the_default_profile_reads_to_the_dds_defaults() {
    let profiles = read(DDS4CCM_DEFAULT_QOS_PROFILE_XML).expect("Annex D reads");
    assert_eq!(profiles.len(), 1);
    let p = &profiles[0];
    // As printed, with the leading space.
    assert_eq!(p.name, " DDS DefaultQosProfile");
    assert_eq!(p.base_name, None);

    // The document sets every policy the schema declares for a container.
    let schema_doc = parse_xml_tree(DDS4CCM_QOS_PROFILE_XSD).expect("schema");
    let doc = parse_xml_tree(DDS4CCM_DEFAULT_QOS_PROFILE_XML).expect("Annex D is well-formed");
    assert_eq!(
        doc.root.namespace.as_deref(),
        Some(DDS4CCM_QOS_PROFILE_NAMESPACE)
    );
    let profile_el = doc.root.child("qos_profile").expect("the profile");
    for (element, ty) in CONTAINERS {
        let el = profile_el
            .child(element)
            .unwrap_or_else(|| panic!("Annex D has no <{element}>"));
        let mut written: Vec<String> = el.children.iter().map(|c| c.name.clone()).collect();
        let mut declared = policies_of(&schema_doc.root, ty);
        written.sort();
        declared.sort();
        assert_eq!(written, declared, "<{element}> of Annex D against {ty}");
    }

    let one = |slots: &[zerodds_xml::FilteredEntityQos], what: &str| -> EntityQos {
        assert_eq!(slots.len(), 1, "{what}");
        assert_eq!(slots[0].topic_filter, None, "{what}");
        slots[0].qos.clone()
    };

    // DataReader: twelve policies, each the default of a DataReader.
    let reader = one(&p.datareader_qos, "datareader_qos");
    let d = ReaderQos::default();
    assert_eq!(
        reader,
        EntityQos {
            durability: Some(d.durability),
            deadline: Some(d.deadline),
            latency_budget: Some(d.latency_budget),
            liveliness: Some(d.liveliness),
            reliability: Some(d.reliability),
            destination_order: Some(d.destination_order),
            history: Some(d.history),
            resource_limits: Some(d.resource_limits),
            user_data: Some(d.user_data.clone()),
            ownership: Some(d.ownership),
            time_based_filter: Some(d.time_based_filter),
            reader_data_lifecycle: Some(d.reader_data_lifecycle),
            ..EntityQos::default()
        },
        "datareader_qos"
    );
    assert_eq!(reader.into_reader_qos(), d);
    // The values of the annex, spelt out for the policy whose default
    // differs between the entities.
    assert_eq!(
        reader.reliability,
        Some(ReliabilityQosPolicy {
            kind: ReliabilityKind::BestEffort,
            max_blocking_time: Duration::from_sec_nanosec(0, 100_000_000),
        })
    );

    // DataWriter: fifteen policies, each the default of a DataWriter.
    let writer = one(&p.datawriter_qos, "datawriter_qos");
    let d = WriterQos::default();
    assert_eq!(
        writer,
        EntityQos {
            durability: Some(d.durability),
            durability_service: Some(d.durability_service),
            deadline: Some(d.deadline),
            latency_budget: Some(d.latency_budget),
            liveliness: Some(d.liveliness),
            reliability: Some(d.reliability),
            destination_order: Some(d.destination_order),
            history: Some(d.history),
            resource_limits: Some(d.resource_limits),
            transport_priority: Some(d.transport_priority),
            lifespan: Some(d.lifespan),
            user_data: Some(d.user_data.clone()),
            ownership: Some(d.ownership),
            ownership_strength: Some(d.ownership_strength),
            writer_data_lifecycle: Some(d.writer_data_lifecycle),
            ..EntityQos::default()
        },
        "datawriter_qos"
    );
    assert_eq!(writer.into_writer_qos(), d);
    assert_eq!(
        writer.reliability,
        Some(ReliabilityQosPolicy {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: Duration::from_sec_nanosec(0, 100_000_000),
        })
    );

    // Topic: thirteen policies. The reliability of a Topic is BEST_EFFORT
    // (DDS 1.2 §7.1.3, the table of the policies), as that of a DataReader.
    let topic = one(&p.topic_qos, "topic_qos");
    assert_eq!(
        topic,
        EntityQos {
            topic_data: Some(TopicDataQosPolicy::default()),
            durability: Some(DurabilityQosPolicy::default()),
            durability_service: Some(DurabilityServiceQosPolicy::default()),
            deadline: Some(DeadlineQosPolicy::default()),
            latency_budget: Some(LatencyBudgetQosPolicy::default()),
            liveliness: Some(LivelinessQosPolicy::default()),
            reliability: Some(ReliabilityQosPolicy {
                kind: ReliabilityKind::BestEffort,
                max_blocking_time: Duration::from_millis(100),
            }),
            destination_order: Some(DestinationOrderQosPolicy::default()),
            history: Some(HistoryQosPolicy::default()),
            resource_limits: Some(ResourceLimitsQosPolicy::default()),
            transport_priority: Some(TransportPriorityQosPolicy::default()),
            lifespan: Some(LifespanQosPolicy::default()),
            ownership: Some(OwnershipQosPolicy::default()),
            ..EntityQos::default()
        },
        "topic_qos"
    );

    // DomainParticipant, Subscriber, Publisher.
    assert_eq!(
        one(&p.domainparticipant_qos, "domainparticipant_qos"),
        EntityQos {
            user_data: Some(UserDataQosPolicy::default()),
            entity_factory: Some(EntityFactoryQosPolicy::default()),
            ..EntityQos::default()
        }
    );
    let group = EntityQos {
        presentation: Some(PresentationQosPolicy::default()),
        partition: Some(PartitionQosPolicy::default()),
        group_data: Some(GroupDataQosPolicy::default()),
        entity_factory: Some(EntityFactoryQosPolicy::default()),
        ..EntityQos::default()
    };
    assert_eq!(one(&p.subscriber_qos, "subscriber_qos"), group);
    assert_eq!(one(&p.publisher_qos, "publisher_qos"), group);

    // The defaults the comparison above rests on, as DDS gives them.
    assert_eq!(
        PresentationQosPolicy::default(),
        PresentationQosPolicy {
            access_scope: PresentationAccessScope::Instance,
            coherent_access: false,
            ordered_access: false,
        }
    );
    assert!(PartitionQosPolicy::default().names.is_empty());
    assert!(EntityFactoryQosPolicy::default().autoenable_created_entities);
    assert_eq!(
        HistoryQosPolicy::default(),
        HistoryQosPolicy {
            kind: HistoryKind::KeepLast,
            depth: 1,
        }
    );
    assert_eq!(
        ResourceLimitsQosPolicy::default(),
        ResourceLimitsQosPolicy {
            max_samples: -1,
            max_instances: -1,
            max_samples_per_instance: -1,
        }
    );
    assert_eq!(
        DurabilityQosPolicy::default().kind,
        DurabilityKind::Volatile
    );
    assert_eq!(DeadlineQosPolicy::default().period, Duration::INFINITE);
    assert_eq!(LatencyBudgetQosPolicy::default().duration, Duration::ZERO);
    assert_eq!(
        LivelinessQosPolicy::default(),
        LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            lease_duration: Duration::INFINITE,
        }
    );
    assert_eq!(
        DestinationOrderQosPolicy::default().kind,
        DestinationOrderKind::ByReceptionTimestamp
    );
    assert_eq!(OwnershipQosPolicy::default().kind, OwnershipKind::Shared);
    assert_eq!(OwnershipStrengthQosPolicy::default().value, 0);
    assert_eq!(TransportPriorityQosPolicy::default().value, 0);
    assert_eq!(LifespanQosPolicy::default().duration, Duration::INFINITE);
    assert_eq!(
        TimeBasedFilterQosPolicy::default().minimum_separation,
        Duration::ZERO
    );
    assert!(WriterDataLifecycleQosPolicy::default().autodispose_unregistered_instances);
    assert_eq!(
        ReaderDataLifecycleQosPolicy::default(),
        ReaderDataLifecycleQosPolicy {
            autopurge_nowriter_samples_delay: Duration::INFINITE,
            autopurge_disposed_samples_delay: Duration::INFINITE,
        }
    );
    assert_eq!(
        DurabilityServiceQosPolicy::default(),
        DurabilityServiceQosPolicy {
            service_cleanup_delay: Duration::ZERO,
            history_kind: HistoryKind::KeepLast,
            history_depth: 1,
            max_samples: -1,
            max_instances: -1,
            max_samples_per_instance: -1,
        }
    );
}

// ---------------------------------------------------------------------
// §7.4.2.1: the syntax of the file and the values of Table 7.1
// ---------------------------------------------------------------------

/// The DDS4CCM reading of `xml` with the paths of its vendor form
/// diagnostics, without the one of the profile directly under `<dds>`
/// (§7.4.2.3, every document of these tests has it).
fn read_reported(xml: &str) -> Result<(Vec<QosProfile>, Vec<String>), XmlError> {
    let parsed = parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms())?;
    let paths = parsed
        .diagnostics
        .iter()
        .filter(|d| d.kind == DiagnosticKind::VendorForm)
        .filter(|d| !d.message.contains("directly under <dds>"))
        .map(|d| d.path.clone())
        .collect();
    let profiles = parsed
        .value
        .qos_libraries
        .into_iter()
        .filter(|l| l.name == CCM_LIBRARY)
        .flat_map(|l| l.profiles)
        .collect();
    Ok((profiles, paths))
}

/// A profile `P` directly under `<dds>` holding `body`.
fn profile(body: &str) -> String {
    format!("<dds><qos_profile name=\"P\">{body}</qos_profile></dds>")
}

/// §7.4.2.1 — "All leading and trailing spaces are removed from the string
/// before it is processed": a document with spaces around every value reads
/// to the QoS of the document without them. The spaces around a partition
/// name, which DDS-XML 1.0 keeps as part of the string, are removed and
/// reported. The hexadecimal transport priority is a form of Table 7.1 that
/// the schema leaves out: reported with and without spaces.
#[test]
fn dds4ccm_7_4_2_1_leading_and_trailing_spaces_are_removed() {
    let body = |s: &str| {
        profile(&format!(
            "<datawriter_qos>\
               <history><kind>{s}KEEP_LAST_HISTORY_QOS{s}</kind><depth>{s}5{s}</depth></history>\
               <reliability><kind>{s}RELIABLE_RELIABILITY_QOS{s}</kind>\
                 <max_blocking_time><sec>{s}2{s}</sec><nanosec>{s}DURATION_ZERO_NSEC{s}</nanosec></max_blocking_time></reliability>\
               <resource_limits><max_samples>{s}LENGTH_UNLIMITED{s}</max_samples><max_instances>{s}7{s}</max_instances></resource_limits>\
               <transport_priority><value>{s}0x7fffffff{s}</value></transport_priority>\
               <writer_data_lifecycle><autodispose_unregistered_instances>{s}false{s}</autodispose_unregistered_instances></writer_data_lifecycle>\
             </datawriter_qos>\
             <publisher_qos><partition><name><element>{s}A{s}</element></name></partition></publisher_qos>"
        ))
    };
    let (plain, plain_reported) = read_reported(&body("")).expect("without spaces");
    assert_eq!(
        plain_reported,
        ["dds/qos_profile[P]/datawriter_qos/transport_priority/value"]
    );
    for s in [" ", "  ", "\t", "\n "] {
        let (spaced, reported) = read_reported(&body(s)).unwrap_or_else(|e| panic!("{s:?}: {e:?}"));
        assert_eq!(
            spaced[0].datawriter_qos[0].qos, plain[0].datawriter_qos[0].qos,
            "{s:?}"
        );
        assert_eq!(
            spaced[0].publisher_qos[0].qos, plain[0].publisher_qos[0].qos,
            "{s:?}"
        );
        assert_eq!(
            reported,
            [
                "dds/qos_profile[P]/datawriter_qos/transport_priority/value",
                "dds/qos_profile[P]/publisher_qos/partition/name/element"
            ],
            "{s:?}"
        );
    }
    let qos = &plain[0].datawriter_qos[0].qos;
    assert_eq!(qos.history.map(|h| h.depth), Some(5));
    assert_eq!(
        qos.transport_priority.as_ref().map(|t| t.value),
        Some(0x7fff_ffff)
    );
    assert_eq!(
        plain[0].publisher_qos[0]
            .qos
            .partition
            .as_ref()
            .map(|p| p.names.clone()),
        Some(vec!["A".to_string()])
    );

    // The default reading keeps the spaces of a string (DDS-XML 1.0 §7.1.4).
    let xml = "<dds><qos_library name=\"L\"><qos_profile name=\"P\"><publisher_qos><partition>\
               <name><element> A </element></name></partition></publisher_qos></qos_profile>\
               </qos_library></dds>";
    let parsed = parse_dds_xml(xml).expect("DDS-XML 1.0 document");
    assert_eq!(
        parsed.qos_libraries[0].profiles[0].publisher_qos[0]
            .qos
            .partition
            .as_ref()
            .map(|p| p.names.clone()),
        Some(vec![" A ".to_string()])
    );
}

/// §7.4.2.1 — "All values are case-sensitive unless otherwise stated", and
/// Table 7.1 for an Enum: "Must be specified as a string. (Do not use
/// numeric values.)" An enumeration literal in another case and a number
/// are refused, under both readings.
#[test]
fn dds4ccm_7_4_2_1_enums_are_case_sensitive_strings() {
    for (policy, written) in [
        (
            "<history><kind>keep_all_history_qos</kind></history>",
            "keep_all_history_qos",
        ),
        (
            "<history><kind>Keep_All_History_Qos</kind></history>",
            "Keep_All_History_Qos",
        ),
        ("<history><kind>1</kind></history>", "1"),
        (
            "<reliability><kind>reliable_reliability_qos</kind></reliability>",
            "reliable_reliability_qos",
        ),
        ("<reliability><kind>2</kind></reliability>", "2"),
        ("<durability><kind>0</kind></durability>", "0"),
        (
            "<ownership><kind>Exclusive_Ownership_Qos</kind></ownership>",
            "Exclusive_Ownership_Qos",
        ),
    ] {
        let xml = profile(&format!("<datawriter_qos>{policy}</datawriter_qos>"));
        let err = read(&xml).expect_err(written);
        assert!(
            matches!(&err, XmlError::BadEnum(e) if e == written),
            "{written}: {err:?}"
        );
        let library = format!(
            "<dds><qos_library name=\"L\"><qos_profile name=\"P\"><datawriter_qos>{policy}\
             </datawriter_qos></qos_profile></qos_library></dds>"
        );
        let err = parse_dds_xml(&library).expect_err(written);
        assert!(matches!(&err, XmlError::BadEnum(_)), "{written}: {err:?}");
    }
    // The literal as written is read.
    let p = read(&profile(
        "<datawriter_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>",
    ))
    .expect("the literal");
    assert_eq!(
        p[0].datawriter_qos[0].qos.history.map(|h| h.kind),
        Some(HistoryKind::KeepAll)
    );
}

/// Table 7.1 — Boolean: "yes, 1, true or BOOLEAN_TRUE" and "no, 0, false or
/// BOOLEAN_FALSE", "Not case-sensitive", in each of the four Boolean
/// fields of a QoS profile. Each form outside DDS-XML 1.0 is one diagnostic
/// with the path of its element; `true`, `1`, `false` and `0` give none.
#[test]
fn dds4ccm_7_4_2_1_table_7_1_boolean_forms_are_read_without_regard_to_case() {
    let doc = |b: &str| {
        profile(&format!(
            "<publisher_qos><presentation><coherent_access>{b}</coherent_access>\
               <ordered_access>{b}</ordered_access></presentation>\
               <entity_factory><autoenable_created_entities>{b}</autoenable_created_entities></entity_factory>\
             </publisher_qos>\
             <datawriter_qos><writer_data_lifecycle><autodispose_unregistered_instances>{b}\
               </autodispose_unregistered_instances></writer_data_lifecycle></datawriter_qos>"
        ))
    };
    let paths = [
        "dds/qos_profile[P]/publisher_qos/presentation/coherent_access",
        "dds/qos_profile[P]/publisher_qos/presentation/ordered_access",
        "dds/qos_profile[P]/publisher_qos/entity_factory/autoenable_created_entities",
        "dds/qos_profile[P]/datawriter_qos/writer_data_lifecycle/autodispose_unregistered_instances",
    ];
    let cases: &[(&str, bool, bool)] = &[
        // (text, value, a form of DDS-XML 1.0)
        ("true", true, true),
        ("1", true, true),
        ("false", false, true),
        ("0", false, true),
        ("yes", true, false),
        ("Yes", true, false),
        ("YES", true, false),
        ("TRUE", true, false),
        ("True", true, false),
        ("BOOLEAN_TRUE", true, false),
        ("boolean_true", true, false),
        ("no", false, false),
        ("NO", false, false),
        ("False", false, false),
        ("FALSE", false, false),
        ("BOOLEAN_FALSE", false, false),
        ("Boolean_False", false, false),
    ];
    for &(text, value, dds_xml) in cases {
        let (p, reported) = read_reported(&doc(text)).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        let publisher = &p[0].publisher_qos[0].qos;
        let presentation = publisher.presentation.as_ref().expect("presentation");
        assert_eq!(presentation.coherent_access, value, "{text}");
        assert_eq!(presentation.ordered_access, value, "{text}");
        assert_eq!(
            publisher
                .entity_factory
                .as_ref()
                .map(|e| e.autoenable_created_entities),
            Some(value),
            "{text}"
        );
        assert_eq!(
            p[0].datawriter_qos[0]
                .qos
                .writer_data_lifecycle
                .as_ref()
                .map(|w| w.autodispose_unregistered_instances),
            Some(value),
            "{text}"
        );
        if dds_xml {
            assert!(reported.is_empty(), "{text}: {reported:?}");
        } else {
            assert_eq!(reported, paths, "{text}");
        }
    }
}

/// Table 7.1 — a text that is none of the eight Boolean forms is refused,
/// under both readings, with `ValueOutOfRange`.
#[test]
fn dds4ccm_7_4_2_1_table_7_1_other_boolean_text_is_refused() {
    for text in [
        "on", "off", "y", "n", "2", "-1", "", "yes no", "BOOLEAN", "truth",
    ] {
        let xml = profile(&format!(
            "<publisher_qos><entity_factory><autoenable_created_entities>{text}\
             </autoenable_created_entities></entity_factory></publisher_qos>"
        ));
        let err = read(&xml).expect_err(text);
        assert!(
            matches!(err, XmlError::ValueOutOfRange(_)),
            "{text}: {err:?}"
        );
        let library = format!(
            "<dds><qos_library name=\"L\"><qos_profile name=\"P\"><publisher_qos><entity_factory>\
             <autoenable_created_entities>{text}</autoenable_created_entities></entity_factory>\
             </publisher_qos></qos_profile></qos_library></dds>"
        );
        let err = parse_dds_xml(&library).expect_err(text);
        assert!(
            matches!(err, XmlError::ValueOutOfRange(_)),
            "{text}: {err:?}"
        );
    }
}

/// Table 7.1 — the default reading refuses each form of the table that
/// DDS-XML 1.0 does not take in the field with `VendorForm`, naming the
/// element, the text and what DDS-XML 1.0 writes. The Boolean forms of
/// DDS4CCM are no form of DDS-XML 1.0. The hexadecimal and the negative Long
/// are forms of Table 7.1 of DDS-XML 1.0 §7.1.4 as well, which the schema of
/// DDS-XML 1.0 leaves out of the field; the message says so and names the
/// schema type. That holds for every numeric field of a QoS policy.
#[test]
fn dds4ccm_7_4_2_1_table_7_1_the_default_reading_names_the_form() {
    const NOT_DDS_XML: &str = "is not a DDS-XML 1.0 form";
    const SCHEMA: &str = "Table 7.1 (DDS-XML 1.0 §7.1.4, DDS4CCM 1.1 §7.4.2.1) that the schema \
                          of DDS-XML 1.0 leaves out of this field";
    let base = "dds/qos_library[L]/qos_profile[P]/";
    for (body, path, text, says) in [
        (
            "<publisher_qos><entity_factory><autoenable_created_entities>yes\
             </autoenable_created_entities></entity_factory></publisher_qos>",
            "publisher_qos/entity_factory/autoenable_created_entities",
            "`yes`",
            NOT_DDS_XML,
        ),
        (
            "<publisher_qos><presentation><ordered_access>BOOLEAN_TRUE</ordered_access>\
             </presentation></publisher_qos>",
            "publisher_qos/presentation/ordered_access",
            "`BOOLEAN_TRUE`",
            NOT_DDS_XML,
        ),
        (
            "<datareader_qos><resource_limits><max_instances>0x20</max_instances>\
             </resource_limits></datareader_qos>",
            "datareader_qos/resource_limits/max_instances",
            "`0x20`",
            "positiveInteger_UNLIMITED",
        ),
        (
            "<datareader_qos><resource_limits><initial_instances>0x20</initial_instances>\
             </resource_limits></datareader_qos>",
            "datareader_qos/resource_limits/initial_instances",
            "`0x20`",
            "xs:positiveInteger",
        ),
        (
            "<datareader_qos><history><depth>0x20</depth></history></datareader_qos>",
            "datareader_qos/history/depth",
            "`0x20`",
            "xs:positiveInteger",
        ),
        (
            "<topic_qos><durability_service><history_depth>0x2</history_depth>\
             </durability_service></topic_qos>",
            "topic_qos/durability_service/history_depth",
            "`0x2`",
            "xs:positiveInteger",
        ),
        (
            "<datareader_qos><deadline><period><nanosec>0x1</nanosec></period></deadline>\
             </datareader_qos>",
            "datareader_qos/deadline/period/nanosec",
            "`0x1`",
            "nonNegativeInteger_Duration_NSEC",
        ),
        (
            "<datawriter_qos><ownership_strength><value>0x7fffffff</value>\
             </ownership_strength></datawriter_qos>",
            "datawriter_qos/ownership_strength/value",
            "`0x7fffffff`",
            "xs:nonNegativeInteger",
        ),
        (
            "<datawriter_qos><transport_priority><value>-3</value>\
             </transport_priority></datawriter_qos>",
            "datawriter_qos/transport_priority/value",
            "the negative Long `-3`",
            "xs:nonNegativeInteger",
        ),
        (
            "<datawriter_qos><transport_priority><value>LENGTH_UNLIMITED</value>\
             </transport_priority></datawriter_qos>",
            "datawriter_qos/transport_priority/value",
            "the Long `LENGTH_UNLIMITED` (-1)",
            "xs:nonNegativeInteger",
        ),
    ] {
        let path = format!("{base}{path}");
        let xml = format!(
            "<dds><qos_library name=\"L\"><qos_profile name=\"P\">{body}</qos_profile>\
             </qos_library></dds>"
        );
        let err = parse_dds_xml(&xml).expect_err(&path);
        let XmlError::VendorForm(m) = &err else {
            panic!("{path}: {err:?}");
        };
        assert!(m.starts_with(&path), "{path}: {m}");
        assert!(m.contains(text), "{path}: {m}");
        assert!(m.contains(says), "{path}: {m}");
        // A number form is a form of DDS-XML 1.0 Table 7.1 and not called
        // otherwise.
        if says != NOT_DDS_XML {
            assert!(m.contains(SCHEMA), "{path}: {m}");
            assert!(!m.contains(NOT_DDS_XML), "{path}: {m}");
        }
    }
}

/// Table 7.1 — Long: "-2147483648 to 2147483647 or 0x80000000 to 0x7fffffff
/// or LENGTH_UNLIMITED"; UnsignedLong: "0 to 4294967296 or 0 to 0xffffffff".
/// Decimal and hexadecimal in every Long and UnsignedLong field of a
/// profile. The schema type of every numeric QoS field in DDS-XML 1.0 has
/// decimal digits only (`positiveInteger_UNLIMITED`, the duration patterns,
/// `xs:positiveInteger`, `xs:nonNegativeInteger`): each hexadecimal number
/// is reported, once per element, and so is a negative decimal ownership
/// strength or transport priority. A value outside the range the DDS IDL
/// gives the field is refused.
#[test]
fn dds4ccm_7_4_2_1_table_7_1_long_is_decimal_hexadecimal_or_length_unlimited() {
    let xml = profile(
        "<datareader_qos>\
           <history><depth>0x10</depth></history>\
           <resource_limits><max_samples>0x40</max_samples><max_instances>LENGTH_UNLIMITED</max_instances>\
             <max_samples_per_instance>0X7FFFFFFF</max_samples_per_instance>\
             <initial_samples>0x4</initial_samples><initial_instances>3</initial_instances></resource_limits>\
           <deadline><period><sec>0x3c</sec><nanosec>0x1F4</nanosec></period></deadline>\
         </datareader_qos>\
         <topic_qos>\
           <durability_service><history_depth>0xA</history_depth><max_samples>0x8</max_samples>\
             <max_instances>12</max_instances></durability_service>\
         </topic_qos>\
         <datawriter_qos>\
           <ownership_strength><value>0x7fffffff</value></ownership_strength>\
           <transport_priority><value>0x0</value></transport_priority>\
         </datawriter_qos>\
         <datawriter_qos topic_filter=\"N\">\
           <ownership_strength><value>0x80000000</value></ownership_strength>\
           <transport_priority><value>-2147483648</value></transport_priority>\
         </datawriter_qos>",
    );
    let (p, reported) = read_reported(&xml).expect("Table 7.1 numbers");
    let r = &p[0].datareader_qos[0].qos;
    assert_eq!(r.history.map(|h| h.depth), Some(16));
    let limits = r.resource_limits.as_ref().expect("resource_limits");
    assert_eq!(limits.max_samples, 0x40);
    assert_eq!(limits.max_instances, -1);
    assert_eq!(limits.max_samples_per_instance, i32::MAX);
    assert_eq!(
        r.initial_resource_limits,
        Some(InitialResourceLimits {
            initial_samples: Some(4),
            initial_instances: Some(3),
        })
    );
    assert_eq!(
        r.deadline.map(|d| d.period),
        Some(Duration::from_sec_nanosec(60, 500))
    );
    let ds = p[0].topic_qos[0]
        .qos
        .durability_service
        .as_ref()
        .expect("durability_service");
    assert_eq!(
        (ds.history_depth, ds.max_samples, ds.max_instances),
        (10, 8, 12)
    );
    let w = &p[0]
        .datawriter_qos
        .iter()
        .find(|q| q.topic_filter.as_deref() != Some("N"))
        .expect("the QoS without topic_filter")
        .qos;
    assert_eq!(
        w.ownership_strength.as_ref().map(|o| o.value),
        Some(i32::MAX)
    );
    assert_eq!(w.transport_priority.as_ref().map(|t| t.value), Some(0));
    assert_eq!(
        reported,
        [
            "dds/qos_profile[P]/datareader_qos/history/depth",
            "dds/qos_profile[P]/datareader_qos/resource_limits/max_samples",
            "dds/qos_profile[P]/datareader_qos/resource_limits/max_samples_per_instance",
            "dds/qos_profile[P]/datareader_qos/resource_limits/initial_samples",
            "dds/qos_profile[P]/datareader_qos/deadline/period/sec",
            "dds/qos_profile[P]/datareader_qos/deadline/period/nanosec",
            "dds/qos_profile[P]/topic_qos/durability_service/history_depth",
            "dds/qos_profile[P]/topic_qos/durability_service/max_samples",
            // The QoS without topic_filter: 0x7fffffff and 0x0.
            "dds/qos_profile[P]/datawriter_qos/ownership_strength/value",
            "dds/qos_profile[P]/datawriter_qos/transport_priority/value",
            // The QoS of topic N: 0x80000000 (one report, not two) and
            // -2147483648.
            "dds/qos_profile[P]/datawriter_qos/ownership_strength/value",
            "dds/qos_profile[P]/datawriter_qos/transport_priority/value",
        ]
    );
    // The negative half of a Long, decimal and hexadecimal, in the two
    // fields that are a long of the DDS IDL without a lower bound.
    let n = &p[0]
        .datawriter_qos
        .iter()
        .find(|q| q.topic_filter.as_deref() == Some("N"))
        .expect("the QoS of topic N")
        .qos;
    assert_eq!(
        n.ownership_strength.as_ref().map(|o| o.value),
        Some(i32::MIN)
    );
    assert_eq!(
        n.transport_priority.as_ref().map(|t| t.value),
        Some(i32::MIN)
    );

    // Out of the range of the field: a resource limit is 1 or more, a
    // duration's seconds are 0 to 0x7fffffff, and a number has 32 bits.
    for body in [
        "<resource_limits><max_samples>0x0</max_samples></resource_limits>",
        "<resource_limits><max_samples>0x80000000</max_samples></resource_limits>",
        "<resource_limits><max_samples>0xffffffff</max_samples></resource_limits>",
        "<resource_limits><max_samples>0x100000000</max_samples></resource_limits>",
        "<resource_limits><max_samples>0x</max_samples></resource_limits>",
        "<resource_limits><max_samples>0x+1</max_samples></resource_limits>",
        "<deadline><period><sec>0x80000000</sec></period></deadline>",
        "<deadline><period><nanosec>0x1ffffffff</nanosec></period></deadline>",
        "<history><depth>0x0</depth></history>",
        "<history><depth>0x80000000</depth></history>",
        "<resource_limits><initial_samples>0x0</initial_samples></resource_limits>",
    ] {
        let xml = profile(&format!("<datareader_qos>{body}</datareader_qos>"));
        let err = read(&xml).expect_err(body);
        assert!(
            matches!(err, XmlError::ValueOutOfRange(_)),
            "{body}: {err:?}"
        );
    }
}

/// §7.4.2.1 — "The root tag of the configuration file must be <dds>": a
/// profile as the root is refused; the root of Annex D, with its namespace
/// and schema location, is read.
#[test]
fn dds4ccm_7_4_2_1_the_root_is_dds() {
    let err = read(r#"<qos_profile name="P"/>"#).expect_err("a profile as the root");
    assert!(matches!(err, XmlError::InvalidXml(_)), "{err:?}");
    let err = read(r#"<datawriter_qos name="P"/>"#).expect_err("a single QoS as the root");
    assert!(matches!(err, XmlError::InvalidXml(_)), "{err:?}");
    let p = read(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- a comment before the root -->
<dds xmlns="http://www.omg.org/dds/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:schemaLocation="file://DDS_QoSProfile.xsd">
  <!-- a comment in the root -->
  <qos_profile name="P"><datawriter_qos><history><depth>2</depth></history></datawriter_qos></qos_profile>
</dds>"#,
    )
    .expect("the root of Annex D");
    assert_eq!(p[0].name, "P");
    assert_eq!(p[0].datawriter_qos[0].qos.history.map(|h| h.depth), Some(2));
}
