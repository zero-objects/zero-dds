//! Integration tests for DDS-XML 1.0 §7.3.2 QoS profile library.
//!
//! Covers:
//! * Happy-path roundtrip (reliability + history policies).
//! * Single-QoS shortcut for all 6 entity types.
//! * Inheritance (1, 2, 3 generations) + override semantics.
//! * Inheritance cycle detection + depth cap.
//! * `base_name` to a non-existent profile.
//! * Topic filter glob match.
//! * 22-policy coverage (each policy at least one parse test).
//! * DURATION_INFINITY sentinel + LENGTH_UNLIMITED.
//! * Partition Vec construction, UserData Base64.
//! * Boolean-Case-Sensitivity.
//! * DTD prohibition.
//! * Minimal XML without policies (None defaults).

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
    clippy::unreachable,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_qos::{
    DestinationOrderKind, DurabilityKind, HistoryKind, LivelinessKind, OwnershipKind,
    PresentationAccessScope, ReliabilityKind,
};
use zerodds_xml::{
    EntityQos, XmlError, parse_qos_libraries, parse_qos_library, resolve_profile,
    topic_filter_matches,
};

// ---------- 1. Happy-Path -----------------------------------------------

#[test]
fn t01_happy_path_reliability_and_history_roundtrip() {
    let xml = r#"<?xml version="1.0"?>
<dds>
  <qos_library name="L">
    <qos_profile name="P">
      <datawriter_qos>
        <reliability>
          <kind>RELIABLE_RELIABILITY_QOS</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <history>
          <kind>KEEP_LAST_HISTORY_QOS</kind>
          <depth>10</depth>
        </history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    assert_eq!(lib.name, "L");
    let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
    let r = dw.reliability.expect("rel");
    assert_eq!(r.kind, ReliabilityKind::Reliable);
    assert_eq!(r.max_blocking_time.seconds, 1);
    let h = dw.history.expect("hist");
    assert_eq!(h.kind, HistoryKind::KeepLast);
    assert_eq!(h.depth, 10);
}

// ---------- 2. Single-QoS-Shortcut for all 6 Entity-Typen ---------------

#[test]
fn t02_all_six_containers_in_a_profile() {
    // A policy valid for each entity kind (DDS 1.4 §2.2.3); a policy in
    // the wrong container is rejected by `t09b`.
    for tag in &[
        "datawriter_qos",
        "datareader_qos",
        "topic_qos",
        "publisher_qos",
        "subscriber_qos",
        "domainparticipant_qos",
    ] {
        let body = match *tag {
            "datawriter_qos" | "datareader_qos" | "topic_qos" => {
                "<reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>"
            }
            "publisher_qos" | "subscriber_qos" => {
                "<partition><name><element>p</element></name></partition>"
            }
            "domainparticipant_qos" => "<user_data><valueB64>AQ==</valueB64></user_data>",
            _ => unreachable!(),
        };
        // The schema's name of the participant container.
        let element = if *tag == "domainparticipant_qos" {
            "domain_participant_qos"
        } else {
            tag
        };
        let xml = format!(
            r#"<dds><qos_library name="L"><qos_profile name="P">
              <{element}>{body}</{element}>
            </qos_profile></qos_library></dds>"#,
        );
        let lib = parse_qos_library(&xml).expect("parse");
        let p = &lib.profiles[0];
        let entity: &EntityQos = match *tag {
            "datawriter_qos" => &p.datawriter_qos.first().expect("dw").qos,
            "datareader_qos" => &p.datareader_qos.first().expect("dr").qos,
            "topic_qos" => &p.topic_qos.first().expect("topic").qos,
            "publisher_qos" => &p.publisher_qos.first().expect("pub").qos,
            "subscriber_qos" => &p.subscriber_qos.first().expect("sub").qos,
            "domainparticipant_qos" => &p.domainparticipant_qos.first().expect("dp").qos,
            _ => unreachable!(),
        };
        assert!(
            entity.reliability.is_some()
                || entity.partition.is_some()
                || entity.user_data.is_some(),
            "single-QoS-shortcut for {tag}"
        );
    }
}

// ---------- 3. Single-Level Inheritance + Override ---------------------

#[test]
fn t03_inheritance_child_overrides_history_keeps_reliability() {
    let xml = r#"<dds><qos_library name="L">
      <qos_profile name="Base">
        <datawriter_qos>
          <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history>
        </datawriter_qos>
      </qos_profile>
      <qos_profile name="Child" base_name="Base">
        <datawriter_qos>
          <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
        </datawriter_qos>
      </qos_profile>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let r = resolve_profile(&libs, "L::Child").expect("resolve");
    let dw = &r.datawriter_qos.first().expect("dw").qos;
    // Reliability inherited.
    assert_eq!(dw.reliability.expect("rel").kind, ReliabilityKind::Reliable);
    // History overridden.
    assert_eq!(dw.history.expect("hist").kind, HistoryKind::KeepAll);
}

// ---------- 4. Multi-Level (3 Generationen) Inheritance ----------------

#[test]
fn t04_three_level_inheritance_propagates() {
    let xml = r#"<dds><qos_library name="L">
      <qos_profile name="GP">
        <datareader_qos>
          <durability><kind>VOLATILE_DURABILITY_QOS</kind></durability>
          <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
          <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>1</depth></history>
        </datareader_qos>
      </qos_profile>
      <qos_profile name="P" base_name="GP">
        <datareader_qos>
          <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
        </datareader_qos>
      </qos_profile>
      <qos_profile name="C" base_name="P">
        <datareader_qos>
          <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
        </datareader_qos>
      </qos_profile>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let r = resolve_profile(&libs, "L::C").expect("resolve");
    let dr = &r.datareader_qos.first().expect("dr").qos;
    // Durability: P-Override.
    assert_eq!(
        dr.durability.expect("dur").kind,
        DurabilityKind::TransientLocal
    );
    // Reliability: from the grandparent.
    assert_eq!(
        dr.reliability.expect("rel").kind,
        ReliabilityKind::BestEffort
    );
    // History: C-Override.
    assert_eq!(dr.history.expect("hist").kind, HistoryKind::KeepAll);
}

// ---------- 5. Cycle is rejected ---------------------------------------

#[test]
fn t05_inheritance_cycle_rejected() {
    let xml = r#"<dds><qos_library name="L">
      <qos_profile name="A" base_name="B"/>
      <qos_profile name="B" base_name="A"/>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let err = resolve_profile(&libs, "L::A").expect_err("cycle");
    assert!(matches!(err, XmlError::CircularInheritance(_)));
}

// ---------- 6. Unresolved base_name ------------------------------------

#[test]
fn t06_base_name_to_missing_profile_errors() {
    let xml = r#"<dds><qos_library name="L">
      <qos_profile name="A" base_name="DoesNotExist"/>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let err = resolve_profile(&libs, "L::A").expect_err("missing");
    assert!(matches!(err, XmlError::UnresolvedReference(_)));
}

// ---------- 7. Tiefen-Cap ----------------------------------------------

#[test]
fn t07_inheritance_depth_cap_enforced() {
    let mut xml = String::from(r#"<dds><qos_library name="L">"#);
    // 40 generations — exceeds MAX_INHERITANCE_DEPTH=32.
    xml.push_str(r#"<qos_profile name="P0"/>"#);
    for i in 1..40 {
        let prev = i - 1;
        xml.push_str(&format!(
            r#"<qos_profile name="P{i}" base_name="P{prev}"/>"#
        ));
    }
    xml.push_str("</qos_library></dds>");
    let libs = parse_qos_libraries(&xml).expect("parse");
    let err = resolve_profile(&libs, "L::P39").expect_err("depth");
    assert!(matches!(err, XmlError::LimitExceeded(_)));
}

// ---------- 8. Topic-Filter Glob ---------------------------------------

#[test]
fn t08_topic_filter_glob_semantics() {
    assert!(topic_filter_matches("*", "anything"));
    assert!(topic_filter_matches("*", ""));
    assert!(topic_filter_matches("foo_*", "foo_bar"));
    assert!(!topic_filter_matches("foo_*", "bar_foo"));
    assert!(topic_filter_matches("s?nsor", "sensor"));
    assert!(!topic_filter_matches("s?nsor", "snsor"));
}

// ---------- 9. 22-Policy-Coverage --------------------------------------

#[test]
fn t09_all_22_policies_parse() {
    // Each policy sits in a container that is a field of it in the DDS
    // 1.4 IDL (§2.2.3); §7.2 maps the IDL 1:1, so a policy in the wrong
    // container is rejected rather than read and dropped.
    let xml = r#"<dds><qos_library name="L"><qos_profile name="ALL">
      <datawriter_qos>
        <durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability>
        <durability_service>
          <service_cleanup_delay><sec>5</sec><nanosec>0</nanosec></service_cleanup_delay>
          <history_kind>KEEP_LAST_HISTORY_QOS</history_kind>
          <history_depth>3</history_depth>
          <max_samples>100</max_samples>
        </durability_service>
        <deadline><period><sec>10</sec><nanosec>0</nanosec></period></deadline>
        <latency_budget><duration><sec>0</sec><nanosec>500000000</nanosec></duration></latency_budget>
        <liveliness>
          <kind>MANUAL_BY_TOPIC_LIVELINESS_QOS</kind>
          <lease_duration><sec>2</sec><nanosec>0</nanosec></lease_duration>
        </liveliness>
        <reliability>
          <kind>RELIABLE_RELIABILITY_QOS</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <destination_order><kind>BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS</kind></destination_order>
        <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>20</depth></history>
        <resource_limits>
          <max_samples>200</max_samples>
          <max_instances>10</max_instances>
          <max_samples_per_instance>20</max_samples_per_instance>
        </resource_limits>
        <transport_priority><value>7</value></transport_priority>
        <lifespan><duration><sec>30</sec><nanosec>0</nanosec></duration></lifespan>
        <user_data><valueB64>cmF3X2J5dGVz</valueB64></user_data>
        <ownership><kind>EXCLUSIVE_OWNERSHIP_QOS</kind></ownership>
        <ownership_strength><value>42</value></ownership_strength>
        <writer_data_lifecycle>
          <autodispose_unregistered_instances>true</autodispose_unregistered_instances>
        </writer_data_lifecycle>
      </datawriter_qos>
      <datareader_qos>
        <time_based_filter>
          <minimum_separation><sec>0</sec><nanosec>100000000</nanosec></minimum_separation>
        </time_based_filter>
        <reader_data_lifecycle>
          <autopurge_nowriter_samples_delay><sec>5</sec><nanosec>0</nanosec></autopurge_nowriter_samples_delay>
          <autopurge_disposed_samples_delay><sec>10</sec><nanosec>0</nanosec></autopurge_disposed_samples_delay>
        </reader_data_lifecycle>
      </datareader_qos>
      <publisher_qos>
        <presentation>
          <access_scope>TOPIC_PRESENTATION_QOS</access_scope>
          <coherent_access>true</coherent_access>
          <ordered_access>false</ordered_access>
        </presentation>
        <partition><name><element>g1</element></name></partition>
        <group_data><valueB64>Z3JvdXBfZGF0YQ==</valueB64></group_data>
      </publisher_qos>
      <topic_qos>
        <topic_data><valueB64>dG9waWNfZGF0YQ==</valueB64></topic_data>
      </topic_qos>
      <domain_participant_qos>
        <entity_factory>
          <autoenable_created_entities>false</autoenable_created_entities>
        </entity_factory>
      </domain_participant_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let p = &lib.profiles[0];
    let dw = &p.datawriter_qos.first().expect("dw").qos;
    assert_eq!(
        dw.durability.expect("dur").kind,
        DurabilityKind::TransientLocal
    );
    assert_eq!(dw.durability_service.expect("ds").history_depth, 3);
    assert_eq!(dw.deadline.expect("d").period.seconds, 10);
    assert_eq!(dw.latency_budget.expect("lb").duration.seconds, 0);
    assert_eq!(dw.ownership.expect("o").kind, OwnershipKind::Exclusive);
    assert_eq!(dw.ownership_strength.expect("os").value, 42);
    let liv = dw.liveliness.expect("liv");
    assert_eq!(liv.kind, LivelinessKind::ManualByTopic);
    assert_eq!(liv.lease_duration.seconds, 2);
    assert_eq!(dw.reliability.expect("r").kind, ReliabilityKind::Reliable);
    assert_eq!(dw.transport_priority.expect("tp").value, 7);
    assert_eq!(dw.lifespan.expect("lf").duration.seconds, 30);
    assert_eq!(
        dw.destination_order.expect("do").kind,
        DestinationOrderKind::BySourceTimestamp
    );
    assert_eq!(dw.history.expect("h").depth, 20);
    let rl = dw.resource_limits.expect("rl");
    assert_eq!(rl.max_samples, 200);
    assert_eq!(rl.max_instances, 10);
    assert_eq!(rl.max_samples_per_instance, 20);
    assert!(
        dw.writer_data_lifecycle
            .expect("wdl")
            .autodispose_unregistered_instances
    );
    assert_eq!(dw.user_data.as_ref().expect("ud").value, b"raw_bytes");

    let dr = &p.datareader_qos.first().expect("dr").qos;
    assert_eq!(
        dr.time_based_filter
            .expect("tbf")
            .minimum_separation
            .seconds,
        0
    );
    let rdl = dr.reader_data_lifecycle.expect("rdl");
    assert_eq!(rdl.autopurge_nowriter_samples_delay.seconds, 5);
    assert_eq!(rdl.autopurge_disposed_samples_delay.seconds, 10);

    let pres = p
        .publisher_qos
        .first()
        .expect("pub")
        .qos
        .presentation
        .expect("pres");
    assert_eq!(pres.access_scope, PresentationAccessScope::Topic);
    assert!(pres.coherent_access);
    assert!(!pres.ordered_access);
    assert_eq!(
        p.publisher_qos
            .first()
            .expect("pub")
            .qos
            .partition
            .as_ref()
            .expect("part")
            .names,
        vec!["g1"]
    );
    assert_eq!(
        p.publisher_qos
            .first()
            .expect("pub")
            .qos
            .group_data
            .as_ref()
            .expect("gd")
            .value,
        b"group_data"
    );

    assert_eq!(
        p.topic_qos
            .first()
            .expect("topic")
            .qos
            .topic_data
            .as_ref()
            .expect("td")
            .value,
        b"topic_data"
    );
    assert!(
        !p.domainparticipant_qos
            .first()
            .expect("dp")
            .qos
            .entity_factory
            .expect("ef")
            .autoenable_created_entities
    );
}

/// §7.2 maps the DDS IDL 1:1: a policy that is not a field of the
/// container's structure is not accepted.
#[test]
fn t09b_policy_outside_the_container_is_refused() {
    for (container, misplaced) in [
        ("datawriter_qos", "time_based_filter"),
        ("datawriter_qos", "reader_data_lifecycle"),
        ("datareader_qos", "ownership_strength"),
        ("datareader_qos", "lifespan"),
        ("publisher_qos", "reliability"),
    ] {
        let xml = format!(
            r#"<dds><qos_library name="L"><qos_profile name="P">
              <{container}><{misplaced}/></{container}>
            </qos_profile></qos_library></dds>"#
        );
        let err = parse_qos_libraries(&xml).expect_err("misplaced policy");
        assert!(
            matches!(err, XmlError::UnknownElement(_)),
            "{container}/{misplaced}: got {err:?}"
        );
    }
}

// ---------- 10. DURATION_INFINITY Sentinel ----------------------------

#[test]
fn t10_duration_infinity_string_parses_to_infinite() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <deadline>
          <period><sec>DURATION_INFINITY</sec></period>
        </deadline>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
    assert!(dw.deadline.expect("d").period.is_infinite());
}

/// §7.2.2 / §7.2.6: the infinite duration written with the constant's
/// numeric value (`0x7FFFFFFF` in both fields) is the same as the symbol.
#[test]
fn t10b_duration_infinity_numeric_parses_to_infinite() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <deadline>
          <period><sec>2147483647</sec><nanosec>2147483647</nanosec></period>
        </deadline>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
    assert!(dw.deadline.expect("d").period.is_infinite());
}

// ---------- 11. LENGTH_UNLIMITED in resource_limits -------------------

#[test]
fn t11_length_unlimited_in_resource_limits() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <resource_limits>
          <max_samples>LENGTH_UNLIMITED</max_samples>
          <max_instances>LENGTH_UNLIMITED</max_instances>
          <max_samples_per_instance>LENGTH_UNLIMITED</max_samples_per_instance>
        </resource_limits>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let rl = lib.profiles[0]
        .datawriter_qos
        .first()
        .expect("dw")
        .qos
        .resource_limits
        .expect("rl");
    assert_eq!(rl.max_samples, -1);
    assert_eq!(rl.max_instances, -1);
    assert_eq!(rl.max_samples_per_instance, -1);
}

// ---------- 12. Partition Vec with multiple names ---------------------

#[test]
fn t12_partition_multiple_names_become_vec() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <publisher_qos>
        <partition>
          <name>
            <element>g1</element>
            <element>g2</element>
            <element>sensor_*</element>
          </name>
        </partition>
      </publisher_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let part = lib.profiles[0]
        .publisher_qos
        .first()
        .expect("pub")
        .qos
        .partition
        .as_ref()
        .expect("part");
    assert_eq!(part.names, vec!["g1", "g2", "sensor_*"]);
}

// ---------- 13. user_data Base64 Dekodierung --------------------------

#[test]
fn t13_user_data_base64_to_bytes() {
    // §7.2.4.2: the Base64 form carries the suffix B64.
    // "raw_bytes" -> "cmF3X2J5dGVz".
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <user_data><valueB64>cmF3X2J5dGVz</valueB64></user_data>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let ud = lib.profiles[0]
        .datawriter_qos
        .first()
        .expect("dw")
        .qos
        .user_data
        .as_ref()
        .expect("ud");
    assert_eq!(ud.value, b"raw_bytes");
}

/// §7.2.4.2: the comma-separated decimal/hex list lives in `<value>`.
/// The example of §7.2.4.2.1 spells out "The quick".
#[test]
fn t13b_user_data_comma_list_to_bytes() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <user_data>
          <value>84, 104, 101, 32, 0x71, 0x75, 0x69, 0x63</value>
        </user_data>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let ud = lib.profiles[0]
        .datawriter_qos
        .first()
        .expect("dw")
        .qos
        .user_data
        .as_ref()
        .expect("ud");
    assert_eq!(ud.value, b"The quic");
}

/// §7.2.4.2 is an `xs:choice`: both representations at once are invalid.
#[test]
fn t13c_user_data_both_forms_rejected() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <user_data>
          <value>1, 2</value>
          <valueB64>AQI=</valueB64>
        </user_data>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let err = parse_qos_libraries(xml).expect_err("both-forms");
    assert!(matches!(err, XmlError::InvalidXml(_)));
}

// ---------- 14. Boolean-Case-Sensitivity ------------------------------

#[test]
fn t14_boolean_case_sensitive_strict() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <domain_participant_qos>
        <entity_factory>
          <autoenable_created_entities>True</autoenable_created_entities>
        </entity_factory>
      </domain_participant_qos>
    </qos_profile></qos_library></dds>"#;
    let err = parse_qos_libraries(xml).expect_err("strict");
    assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    // Lowercase is ok.
    let ok = xml.replace("True", "true");
    let lib = parse_qos_library(&ok).expect("parse");
    let ef = lib.profiles[0]
        .domainparticipant_qos
        .first()
        .expect("dp")
        .qos
        .entity_factory
        .expect("ef");
    assert!(ef.autoenable_created_entities);
}

// ---------- 15. §7.1.1: a DTD is part of a well-formed document ---------

#[test]
fn t15_dtd_declaration_accepted() {
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<dds><qos_library name="L"><qos_profile name="P"/></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("dtd");
    assert_eq!(lib.name, "L");
}

// ---------- 16. Minimal profile -> all None -------------------------

#[test]
fn t16_minimal_profile_all_none() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P"/></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let p = &lib.profiles[0];
    assert!(p.datawriter_qos.is_empty());
    assert!(p.datareader_qos.is_empty());
    assert!(p.topic_qos.is_empty());
    assert!(p.publisher_qos.is_empty());
    assert!(p.subscriber_qos.is_empty());
    assert!(p.domainparticipant_qos.is_empty());
    assert!(p.topic_filter.is_none());
    assert!(p.base_name.is_none());
}

// ---------- 17. Multiple libraries per document -----------------------

#[test]
fn t17_multiple_libraries_per_document() {
    let xml = r#"<dds>
      <qos_library name="L1">
        <qos_profile name="A"/>
      </qos_library>
      <qos_library name="L2">
        <qos_profile name="B"/>
      </qos_library>
    </dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    assert_eq!(libs.len(), 2);
    assert_eq!(libs[0].name, "L1");
    assert_eq!(libs[1].name, "L2");
}

// ---------- 18. into_writer_qos / into_reader_qos materialisieren ----

#[test]
fn t18_into_writer_and_reader_qos_use_spec_defaults() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
    let wq = dw.into_writer_qos();
    // Reliability was explicitly set.
    assert_eq!(wq.reliability.kind, ReliabilityKind::BestEffort);
    // History uses Spec-Default (KeepLast, depth=1).
    assert_eq!(wq.history.kind, HistoryKind::KeepLast);
    assert_eq!(wq.history.depth, 1);

    // ReaderQos on the same profile (use case: mixed reuse).
    let rq = dw.into_reader_qos();
    assert_eq!(rq.reliability.kind, ReliabilityKind::BestEffort);
}

// ---------- 19. Empty policies fall back to the DDS defaults -----------
//
// §7.2.3: the XSD element definition provides the DDS default, and the
// field is `minOccurs="0"`. A present-but-empty policy is therefore a
// policy at its default, not a missing required element.

#[test]
fn t19_empty_policies_use_spec_defaults() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <deadline/>
        <reliability/>
        <lifespan/>
        <history/>
        <ownership/>
        <destination_order/>
        <ownership_strength/>
        <transport_priority/>
        <writer_data_lifecycle/>
      </datawriter_qos>
      <datareader_qos>
        <deadline><period/></deadline>
        <reliability/>
      </datareader_qos>
      <topic_qos>
        <reliability/>
      </topic_qos>
      <domain_participant_qos>
        <entity_factory/>
      </domain_participant_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = &lib.profiles[0].datawriter_qos.first().expect("dw").qos;
    // Deadline: the policy default is INFINITE; an explicit empty
    // <period/> is the zero duration (§7.2.6: both fields optional).
    assert!(dw.deadline.expect("deadline").period.is_infinite());
    let dr = &lib.profiles[0].datareader_qos.first().expect("dr").qos;
    let period = dr.deadline.expect("deadline").period;
    assert_eq!((period.seconds, period.fraction), (0, 0));
    assert_eq!(
        dr.reliability.expect("reader rel").kind,
        ReliabilityKind::BestEffort
    );
    let topic = &lib.profiles[0].topic_qos.first().expect("topic").qos;
    assert_eq!(
        topic.reliability.expect("topic rel").kind,
        ReliabilityKind::BestEffort
    );
    // Reliability: §2.2.3.14 gives a DataWriter RELIABLE, a DataReader
    // and a Topic BEST_EFFORT.
    assert_eq!(dw.reliability.expect("rel").kind, ReliabilityKind::Reliable);
    assert!(dw.lifespan.expect("lifespan").duration.is_infinite());
    let h = dw.history.expect("history");
    assert_eq!(h.kind, HistoryKind::KeepLast);
    assert_eq!(h.depth, 1);
    assert_eq!(dw.ownership.expect("own").kind, OwnershipKind::Shared);
    assert_eq!(
        dw.destination_order.expect("do").kind,
        DestinationOrderKind::ByReceptionTimestamp
    );
    assert_eq!(dw.ownership_strength.expect("os").value, 0);
    assert_eq!(dw.transport_priority.expect("tp").value, 0);
    let dp = &lib.profiles[0]
        .domainparticipant_qos
        .first()
        .expect("dp")
        .qos;
    assert!(dp.entity_factory.expect("ef").autoenable_created_entities);
    assert!(
        dw.writer_data_lifecycle
            .expect("wdl")
            .autodispose_unregistered_instances
    );
}

/// §7.2.3: a structure field is a single element. Two occurrences carry
/// two values and neither may be dropped silently.
#[test]
fn t20_duplicate_structure_field_rejected() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <deadline>
          <period><sec>1</sec></period>
          <period><sec>2</sec></period>
        </deadline>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let err = parse_qos_libraries(xml).expect_err("duplicate");
    assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
}

/// §7.2.6: `sec` and `nanosec` are `minOccurs="0"`; a `<duration>` with
/// no fields is the zero duration, not a missing `<sec>`.
#[test]
fn t19b_duration_without_sec_is_zero() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <latency_budget><duration/></latency_budget>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let lb = lib.profiles[0]
        .datawriter_qos
        .first()
        .expect("dw")
        .qos
        .latency_budget
        .expect("lb");
    assert_eq!(lb.duration.seconds, 0);
}

// ---------- §7.3.2.4.3 Topic-name filters -------------------------------

/// §7.3.2.4.3.1's example: a profile with two `<datawriter_qos>`
/// containers, each with its own `topic_filter` attribute. The parser
/// keeps both, and the topic name selects between them; the explicit
/// filters take precedence over a filterless container, and one that
/// matches neither leaves the entity without QoS from the profile.
#[test]
fn t21_repeated_containers_select_by_topic_filter() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="StrictReliableCommunicationProfile">
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
          <max_instances>1</max_instances>
        </resource_limits>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("the §7.3.2.4.3.1 example parses");
    let r = resolve_profile(&libs, "L::StrictReliableCommunicationProfile").expect("resolve");
    assert_eq!(r.datawriter_qos.len(), 2, "both containers are kept");
    assert_eq!(r.datawriter_qos[0].topic_filter.as_deref(), Some("A*"));
    assert_eq!(r.datawriter_qos[1].topic_filter.as_deref(), Some("B*"));

    let a = r.datawriter_qos_for_topic("A1").expect("A* matches A1");
    assert_eq!(a.history.expect("h").kind, HistoryKind::KeepAll);
    // The A* container names no resource_limits (the B* one does).
    assert!(a.resource_limits.is_none());

    let b = r.datawriter_qos_for_topic("B2").expect("B* matches B2");
    assert_eq!(b.resource_limits.expect("rl").max_samples, 128);

    // Neither explicit filter matches and there is no filterless
    // container: the profile names no QoS for this topic.
    assert!(r.datawriter_qos_for_topic("C3").is_none());
}

/// A filterless container is the "*" of §7.3.2.4.3: it applies when no
/// explicit filter matches, and an explicit match takes precedence
/// over it.
#[test]
fn t22_filterless_container_is_the_fallback() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datareader_qos topic_filter="Sensor*">
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
      </datareader_qos>
      <datareader_qos>
        <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
      </datareader_qos>
    </qos_profile></qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let r = resolve_profile(&libs, "L::P").expect("resolve");
    let sensor = r
        .datareader_qos_for_topic("Sensor1")
        .expect("Sensor* matches");
    assert_eq!(
        sensor.reliability.expect("rel").kind,
        ReliabilityKind::Reliable
    );
    let other = r
        .datareader_qos_for_topic("Actuator")
        .expect("fallback applies");
    assert_eq!(
        other.reliability.expect("rel").kind,
        ReliabilityKind::BestEffort
    );
}
