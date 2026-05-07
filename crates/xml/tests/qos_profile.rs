//! Integrations-Tests fuer DDS-XML 1.0 §7.3.2 QoS-Profile-Library.
//!
//! Deckt:
//! * Happy-Path Roundtrip (Reliability + History Policies).
//! * Single-QoS-Shortcut fuer alle 6 Entity-Typen.
//! * Inheritance (1, 2, 3 Generationen) + Override-Semantik.
//! * Inheritance-Zyklus-Erkennung + Tiefen-Cap.
//! * `base_name` zu nicht-existierendem Profile.
//! * Topic-Filter Glob-Match.
//! * 22-Policy-Coverage (jede Policy mind. ein Parse-Test).
//! * DURATION_INFINITY-Sentinel + LENGTH_UNLIMITED.
//! * Partition-Vec-Aufbau, UserData-Base64.
//! * Boolean-Case-Sensitivity.
//! * DTD-Verbot.
//! * Minimal-XML ohne Policies (None-Defaults).

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
          <kind>RELIABLE</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <history>
          <kind>KEEP_LAST</kind>
          <depth>10</depth>
        </history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
</dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    assert_eq!(lib.name, "L");
    let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
    let r = dw.reliability.expect("rel");
    assert_eq!(r.kind, ReliabilityKind::Reliable);
    assert_eq!(r.max_blocking_time.seconds, 1);
    let h = dw.history.expect("hist");
    assert_eq!(h.kind, HistoryKind::KeepLast);
    assert_eq!(h.depth, 10);
}

// ---------- 2. Single-QoS-Shortcut for all 6 Entity-Typen ---------------

#[test]
fn t02_single_qos_shortcut_all_six_entity_types() {
    for tag in &[
        "datawriter_qos",
        "datareader_qos",
        "topic_qos",
        "publisher_qos",
        "subscriber_qos",
        "domainparticipant_qos",
    ] {
        let xml = format!(
            r#"<dds><qos_library name="L"><qos_profile name="P">
              <{tag}>
                <reliability><kind>RELIABLE</kind></reliability>
              </{tag}>
            </qos_profile></qos_library></dds>"#,
        );
        let lib = parse_qos_library(&xml).expect("parse");
        let p = &lib.profiles[0];
        let entity: &EntityQos = match *tag {
            "datawriter_qos" => p.datawriter_qos.as_ref().expect("dw"),
            "datareader_qos" => p.datareader_qos.as_ref().expect("dr"),
            "topic_qos" => p.topic_qos.as_ref().expect("topic"),
            "publisher_qos" => p.publisher_qos.as_ref().expect("pub"),
            "subscriber_qos" => p.subscriber_qos.as_ref().expect("sub"),
            "domainparticipant_qos" => p.domainparticipant_qos.as_ref().expect("dp"),
            _ => unreachable!(),
        };
        assert_eq!(
            entity.reliability.expect("rel").kind,
            ReliabilityKind::Reliable,
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
          <reliability><kind>RELIABLE</kind></reliability>
          <history><kind>KEEP_LAST</kind><depth>5</depth></history>
        </datawriter_qos>
      </qos_profile>
      <qos_profile name="Child" base_name="Base">
        <datawriter_qos>
          <history><kind>KEEP_ALL</kind></history>
        </datawriter_qos>
      </qos_profile>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let r = resolve_profile(&libs, "L::Child").expect("resolve");
    let dw = r.datawriter_qos.as_ref().expect("dw");
    // Reliability geerbt.
    assert_eq!(dw.reliability.expect("rel").kind, ReliabilityKind::Reliable);
    // History ueberschrieben.
    assert_eq!(dw.history.expect("hist").kind, HistoryKind::KeepAll);
}

// ---------- 4. Multi-Level (3 Generationen) Inheritance ----------------

#[test]
fn t04_three_level_inheritance_propagates() {
    let xml = r#"<dds><qos_library name="L">
      <qos_profile name="GP">
        <datareader_qos>
          <durability><kind>VOLATILE</kind></durability>
          <reliability><kind>BEST_EFFORT</kind></reliability>
          <history><kind>KEEP_LAST</kind><depth>1</depth></history>
        </datareader_qos>
      </qos_profile>
      <qos_profile name="P" base_name="GP">
        <datareader_qos>
          <durability><kind>TRANSIENT_LOCAL</kind></durability>
        </datareader_qos>
      </qos_profile>
      <qos_profile name="C" base_name="P">
        <datareader_qos>
          <history><kind>KEEP_ALL</kind></history>
        </datareader_qos>
      </qos_profile>
    </qos_library></dds>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let r = resolve_profile(&libs, "L::C").expect("resolve");
    let dr = r.datareader_qos.as_ref().expect("dr");
    // Durability: P-Override.
    assert_eq!(
        dr.durability.expect("dur").kind,
        DurabilityKind::TransientLocal
    );
    // Reliability: vom Grossparent.
    assert_eq!(
        dr.reliability.expect("rel").kind,
        ReliabilityKind::BestEffort
    );
    // History: C-Override.
    assert_eq!(dr.history.expect("hist").kind, HistoryKind::KeepAll);
}

// ---------- 5. Cycle wird abgewiesen -----------------------------------

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
    // 40 Generationen — uebersteigt MAX_INHERITANCE_DEPTH=32.
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
    let xml = r#"<dds><qos_library name="L"><qos_profile name="ALL">
      <datawriter_qos>
        <durability><kind>TRANSIENT_LOCAL</kind></durability>
        <durability_service>
          <service_cleanup_delay><sec>5</sec><nanosec>0</nanosec></service_cleanup_delay>
          <history_kind>KEEP_LAST</history_kind>
          <history_depth>3</history_depth>
          <max_samples>100</max_samples>
        </durability_service>
        <presentation>
          <access_scope>TOPIC</access_scope>
          <coherent_access>true</coherent_access>
          <ordered_access>false</ordered_access>
        </presentation>
        <deadline><period><sec>10</sec><nanosec>0</nanosec></period></deadline>
        <latency_budget><duration><sec>0</sec><nanosec>500000000</nanosec></duration></latency_budget>
        <ownership><kind>EXCLUSIVE</kind></ownership>
        <ownership_strength><value>42</value></ownership_strength>
        <liveliness>
          <kind>MANUAL_BY_TOPIC</kind>
          <lease_duration><sec>2</sec><nanosec>0</nanosec></lease_duration>
        </liveliness>
        <time_based_filter>
          <minimum_separation><sec>0</sec><nanosec>100000000</nanosec></minimum_separation>
        </time_based_filter>
        <partition><name>g1</name></partition>
        <reliability>
          <kind>RELIABLE</kind>
          <max_blocking_time><sec>1</sec><nanosec>0</nanosec></max_blocking_time>
        </reliability>
        <transport_priority><value>7</value></transport_priority>
        <lifespan><duration><sec>30</sec><nanosec>0</nanosec></duration></lifespan>
        <destination_order><kind>BY_SOURCE_TIMESTAMP</kind></destination_order>
        <history><kind>KEEP_LAST</kind><depth>20</depth></history>
        <resource_limits>
          <max_samples>200</max_samples>
          <max_instances>10</max_instances>
          <max_samples_per_instance>20</max_samples_per_instance>
        </resource_limits>
        <entity_factory>
          <autoenable_created_entities>false</autoenable_created_entities>
        </entity_factory>
        <writer_data_lifecycle>
          <autodispose_unregistered_instances>true</autodispose_unregistered_instances>
        </writer_data_lifecycle>
        <reader_data_lifecycle>
          <autopurge_nowriter_samples_delay><sec>5</sec><nanosec>0</nanosec></autopurge_nowriter_samples_delay>
          <autopurge_disposed_samples_delay><sec>10</sec><nanosec>0</nanosec></autopurge_disposed_samples_delay>
        </reader_data_lifecycle>
        <user_data><value>cmF3X2J5dGVz</value></user_data>
        <topic_data><value>dG9waWNfZGF0YQ==</value></topic_data>
        <group_data><value>Z3JvdXBfZGF0YQ==</value></group_data>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
    // Spot-check a representative subset across all 22.
    assert_eq!(
        dw.durability.expect("dur").kind,
        DurabilityKind::TransientLocal
    );
    assert_eq!(dw.durability_service.expect("ds").history_depth, 3);
    let pres = dw.presentation.expect("pres");
    assert_eq!(pres.access_scope, PresentationAccessScope::Topic);
    assert!(pres.coherent_access);
    assert!(!pres.ordered_access);
    assert_eq!(dw.deadline.expect("d").period.seconds, 10);
    assert_eq!(dw.latency_budget.expect("lb").duration.seconds, 0);
    assert_eq!(dw.ownership.expect("o").kind, OwnershipKind::Exclusive);
    assert_eq!(dw.ownership_strength.expect("os").value, 42);
    let liv = dw.liveliness.expect("liv");
    assert_eq!(liv.kind, LivelinessKind::ManualByTopic);
    assert_eq!(liv.lease_duration.seconds, 2);
    assert_eq!(
        dw.time_based_filter
            .expect("tbf")
            .minimum_separation
            .seconds,
        0
    );
    assert_eq!(dw.partition.as_ref().expect("part").names, vec!["g1"]);
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
    assert!(!dw.entity_factory.expect("ef").autoenable_created_entities);
    assert!(
        dw.writer_data_lifecycle
            .expect("wdl")
            .autodispose_unregistered_instances
    );
    let rdl = dw.reader_data_lifecycle.expect("rdl");
    assert_eq!(rdl.autopurge_nowriter_samples_delay.seconds, 5);
    assert_eq!(rdl.autopurge_disposed_samples_delay.seconds, 10);
    assert_eq!(dw.user_data.as_ref().expect("ud").value, b"raw_bytes");
    assert_eq!(dw.topic_data.as_ref().expect("td").value, b"topic_data");
    assert_eq!(dw.group_data.as_ref().expect("gd").value, b"group_data");
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
    let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
    assert!(dw.deadline.expect("d").period.is_infinite());
}

// ---------- 11. LENGTH_UNLIMITED in resource_limits -------------------

#[test]
fn t11_length_unlimited_in_resource_limits() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <resource_limits>
          <max_samples>LENGTH_UNLIMITED</max_samples>
          <max_instances>-1</max_instances>
          <max_samples_per_instance>LENGTH_UNLIMITED</max_samples_per_instance>
        </resource_limits>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let rl = lib.profiles[0]
        .datawriter_qos
        .as_ref()
        .expect("dw")
        .resource_limits
        .expect("rl");
    assert_eq!(rl.max_samples, -1);
    assert_eq!(rl.max_instances, -1);
    assert_eq!(rl.max_samples_per_instance, -1);
}

// ---------- 12. Partition Vec mit mehreren Namen ----------------------

#[test]
fn t12_partition_multiple_names_become_vec() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <publisher_qos>
        <partition>
          <name>g1</name>
          <name>g2</name>
          <name>sensor_*</name>
        </partition>
      </publisher_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let part = lib.profiles[0]
        .publisher_qos
        .as_ref()
        .expect("pub")
        .partition
        .as_ref()
        .expect("part");
    assert_eq!(part.names, vec!["g1", "g2", "sensor_*"]);
}

// ---------- 13. user_data Base64 Dekodierung --------------------------

#[test]
fn t13_user_data_base64_to_bytes() {
    // "raw_bytes" -> "cmF3X2J5dGVz".
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <datawriter_qos>
        <user_data><value>cmF3X2J5dGVz</value></user_data>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let ud = lib.profiles[0]
        .datawriter_qos
        .as_ref()
        .expect("dw")
        .user_data
        .as_ref()
        .expect("ud");
    assert_eq!(ud.value, b"raw_bytes");
}

// ---------- 14. Boolean-Case-Sensitivity ------------------------------

#[test]
fn t14_boolean_case_sensitive_strict() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P">
      <domainparticipant_qos>
        <entity_factory>
          <autoenable_created_entities>True</autoenable_created_entities>
        </entity_factory>
      </domainparticipant_qos>
    </qos_profile></qos_library></dds>"#;
    let err = parse_qos_libraries(xml).expect_err("strict");
    assert!(matches!(err, XmlError::ValueOutOfRange(_)));
    // Lowercase ist ok.
    let ok = xml.replace("True", "true");
    let lib = parse_qos_library(&ok).expect("parse");
    let ef = lib.profiles[0]
        .domainparticipant_qos
        .as_ref()
        .expect("dp")
        .entity_factory
        .expect("ef");
    assert!(ef.autoenable_created_entities);
}

// ---------- 15. DTD-Verbot bestaetigt --------------------------------

#[test]
fn t15_dtd_rejected() {
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<dds><qos_library name="L"><qos_profile name="P"/></qos_library></dds>"#;
    let err = parse_qos_libraries(xml).expect_err("dtd");
    assert!(matches!(err, XmlError::InvalidXml(_)));
}

// ---------- 16. Minimal Profile -> alle None -------------------------

#[test]
fn t16_minimal_profile_all_none() {
    let xml = r#"<dds><qos_library name="L"><qos_profile name="P"/></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let p = &lib.profiles[0];
    assert!(p.datawriter_qos.is_none());
    assert!(p.datareader_qos.is_none());
    assert!(p.topic_qos.is_none());
    assert!(p.publisher_qos.is_none());
    assert!(p.subscriber_qos.is_none());
    assert!(p.domainparticipant_qos.is_none());
    assert!(p.topic_filter.is_none());
    assert!(p.base_name.is_none());
}

// ---------- 17. Mehrere Libraries pro Dokument -----------------------

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
        <reliability><kind>BEST_EFFORT</kind></reliability>
      </datawriter_qos>
    </qos_profile></qos_library></dds>"#;
    let lib = parse_qos_library(xml).expect("parse");
    let dw = lib.profiles[0].datawriter_qos.as_ref().expect("dw");
    let wq = dw.into_writer_qos();
    // Reliability war explizit gesetzt.
    assert_eq!(wq.reliability.kind, ReliabilityKind::BestEffort);
    // History uebernimmt Spec-Default (KeepLast, depth=1).
    assert_eq!(wq.history.kind, HistoryKind::KeepLast);
    assert_eq!(wq.history.depth, 1);

    // ReaderQos auf der gleichen Profile (use case: mixed reuse).
    let rq = dw.into_reader_qos();
    assert_eq!(rq.reliability.kind, ReliabilityKind::BestEffort);
}
