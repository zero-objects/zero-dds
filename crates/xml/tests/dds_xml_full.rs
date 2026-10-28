//! Integration tests for DDS-XML 1.0 §7.3.4-7.3.6 (C7.C).
//!
//! Covers:
//! * §7.3.4 Domain Library: topic definitions, register_type lookup,
//!   cross-library domain_ref, topic filter, topic inline QoS vs.
//!   qos_profile_ref.
//! * §7.3.5 Domain Participant Library: domain_ref resolve, multi-level
//!   participant inheritance, inheritance cycle, publisher/subscriber
//!   hierarchy, DataWriter/Reader QoS precedence.
//! * §7.3.6 Application Library: cross-library application-to-participant.
//! * Top-level: empty <dds/>, mixed libraries, DTD acceptance, multiple
//!   domain libraries per document, snapshot resolve.

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

use zerodds_qos::{HistoryKind, ReliabilityKind};
use zerodds_xml::sample::{PrimitiveValue, SampleValue, parse_sample};
use zerodds_xml::xtypes_def::TypeDef;
use zerodds_xml::xtypes_parser::parse_type_libraries;
use zerodds_xml::{
    DdsXml, ReadOptions, ResolvedParticipant, XmlError, apply_to_factory,
    parse_application_libraries, parse_dds_xml, parse_dds_xml_with, parse_domain_libraries,
    parse_domain_libraries_with, parse_domain_participant_libraries, parse_qos_libraries,
};
use zerodds_xml::{ParticipantFactoryAdapter, topic_filter_matches};

/// A document with the vendor forms of the older reader (`qos_profile_ref`,
/// `<topic_filter>` elements); the default reading refuses them.
fn parse_dds_xml_vendor(xml: &str) -> DdsXml {
    parse_dds_xml_with(xml, &ReadOptions::accept_vendor_forms())
        .expect("parse")
        .value
}

// ============================================================================
// 1. Cyclone-style cyclone_default.xml roundtrip.
// ============================================================================
#[test]
fn all_four_libraries_roundtrip() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-XML">
  <types>
    <module name="MyTypes"><struct name="State"/></module>
  </types>
  <qos_library name="ql">
    <qos_profile name="Reliable">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
  <domain_library name="dl">
    <domain name="MyDomain" domain_id="42">
      <register_type name="StateType" type_ref="MyTypes::State"/>
      <topic name="StateTopic" register_type_ref="StateType"/>
    </domain>
  </domain_library>
  <domain_participant_library name="dpl">
    <domain_participant name="MyParticipant" domain_ref="dl::MyDomain">
      <publisher name="Pub1">
        <data_writer name="StateWriter" topic_ref="StateTopic" qos_profile_ref="ql::Reliable"/>
      </publisher>
      <subscriber name="Sub1">
        <data_reader name="StateReader" topic_ref="StateTopic"/>
      </subscriber>
    </domain_participant>
  </domain_participant_library>
  <application_library name="al">
    <application name="MyApp">
      <domain_participant name="MyParticipant" base_name="dpl::MyParticipant"/>
    </application>
  </application_library>
</dds>"#;
    let d = parse_dds_xml_vendor(xml);
    assert_eq!(d.qos_libraries.len(), 1);
    assert_eq!(d.domain_libraries.len(), 1);
    assert_eq!(d.participant_libraries.len(), 1);
    assert_eq!(d.application_libraries.len(), 1);
    let resolved = d
        .resolve_participant("dpl::MyParticipant")
        .expect("resolve");
    assert_eq!(resolved.domain_id, 42);
    assert_eq!(resolved.publishers[0].data_writers[0].name, "StateWriter");
}

// ============================================================================
// 2. Domain with two topics + register_type lookup.
// ============================================================================
#[test]
fn domain_with_two_topics_and_register_types() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="3">
          <register_type name="A" type_ref="ns::A"/>
          <register_type name="B" type_ref="ns::B"/>
          <topic name="TA" register_type_ref="A"/>
          <topic name="TB" register_type_ref="B"/>
        </domain>
      </domain_library>
    </dds>"#;
    let libs = parse_domain_libraries(xml).expect("parse");
    let d = &libs[0].domains[0];
    assert_eq!(d.topics.len(), 2);
    assert_eq!(d.register_types.len(), 2);
    assert!(d.register_type("A").is_some());
    assert!(d.topic("TB").is_some());
}

// ============================================================================
// 3. Cross-library `domain_ref="my_lib::MyDomain"` resolves. The registered
// type is defined before the domain library (§7.3.4.4.1).
// ============================================================================
#[test]
fn cross_library_domain_ref_resolves() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <domain_library name="my_lib">
        <domain name="MyDomain" domain_id="7">
          <register_type name="T" type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="my_lib::MyDomain"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(r.domain_id, 7);
    assert_eq!(r.domain.name, "MyDomain");
}

// ============================================================================
// 4. Cross-library ref not found -> UnresolvedReference.
// ============================================================================
#[test]
fn unresolved_domain_ref_errors() {
    let xml = r#"<dds>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="missing::Foo"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_participant("dpl::P").expect_err("missing");
    assert!(matches!(err, XmlError::UnresolvedReference(_)));
}

// ============================================================================
// 5. Participant inheritance: child inherits reliability from the parent, can
//    override domain_ref.
// ============================================================================
#[test]
fn participant_inheritance_one_level() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="A" domain_id="1"/>
        <domain name="B" domain_id="2"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="Parent" domain_ref="dl::A">
          <domain_participant_qos>
            <entity_factory>
              <autoenable_created_entities>true</autoenable_created_entities>
            </entity_factory>
          </domain_participant_qos>
        </domain_participant>
        <domain_participant name="Child" domain_ref="dl::B" base_name="Parent"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::Child").expect("resolve");
    // domain_ref overridden.
    assert_eq!(r.domain_id, 2);
    // QoS inherited.
    let qos = r.qos.as_ref().expect("qos");
    assert!(qos.entity_factory.is_some());
    assert!(qos.entity_factory.unwrap().autoenable_created_entities);
}

// ============================================================================
// 6. Multi-Level Participant-Inheritance (3 Generationen).
// ============================================================================
#[test]
fn participant_inheritance_three_levels() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="0"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="A" domain_ref="dl::D">
          <publisher name="P1"/>
        </domain_participant>
        <domain_participant name="B" domain_ref="dl::D" base_name="A">
          <publisher name="P2"/>
        </domain_participant>
        <domain_participant name="C" domain_ref="dl::D" base_name="B">
          <publisher name="P3"/>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::C").expect("resolve");
    assert_eq!(r.inheritance_chain.len(), 3);
    // Three publishers (all three levels)
    assert_eq!(r.publishers.len(), 3);
    let names: Vec<&str> = r.publishers.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"P1"));
    assert!(names.contains(&"P2"));
    assert!(names.contains(&"P3"));
}

// ============================================================================
// 7. Inheritance-Zyklus -> CircularInheritance.
// ============================================================================
#[test]
fn participant_inheritance_cycle_detected() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="0"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="A" domain_ref="dl::D" base_name="B"/>
        <domain_participant name="B" domain_ref="dl::D" base_name="A"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_participant("dpl::A").expect_err("cycle");
    assert!(matches!(err, XmlError::CircularInheritance(_)));
}

// ============================================================================
// 8. Application references a non-existing participant -> UnresolvedReference.
// ============================================================================
#[test]
fn application_unresolved_participant() {
    let xml = r#"<dds>
      <application_library name="al">
        <application name="A">
          <domain_participant name="P" base_name="missing::P"/>
        </application>
      </application_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_application("al::A").expect_err("missing");
    assert!(matches!(err, XmlError::UnresolvedReference(_)));
}

// ============================================================================
// 9. Topic filter glob at the topic level matches correctly.
// ============================================================================
#[test]
fn topic_filter_glob_matches() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="T" type_ref="T"/>
          <topic name="StateTopic" register_type_ref="T">
            <topic_filter>State*</topic_filter>
          </topic>
        </domain>
      </domain_library>
    </dds>"#;
    let libs = parse_domain_libraries_with(xml, &ReadOptions::accept_vendor_forms())
        .expect("parse")
        .value;
    let t = &libs[0].domains[0].topics[0];
    let filter = t.topic_filter.as_deref().expect("filter");
    assert!(topic_filter_matches(filter, "StateTopic"));
    assert!(topic_filter_matches(filter, "StateXY"));
    assert!(!topic_filter_matches(filter, "OtherTopic"));
}

// ============================================================================
// 10. §7.3.5.4.4: a DataWriter without explicit QoS does NOT inherit the
//     Publisher's QoS — inline QoS applies only to the entity that owns it.
// ============================================================================
#[test]
fn datawriter_does_not_inherit_publisher_qos() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="T" type_ref="T"/>
          <topic name="Topic1" register_type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <publisher name="Pub1">
            <publisher_qos>
              <partition><name><element>g1</element></name></partition>
            </publisher_qos>
            <data_writer name="W1" topic_ref="Topic1"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    // The Publisher carries its own QoS …
    let pub_qos = r.publishers[0].qos.as_ref().expect("publisher qos");
    assert_eq!(
        pub_qos.partition.as_ref().expect("partition").names,
        vec!["g1".to_string()]
    );
    // … and the DataWriter does not take it over.
    let w = &r.publishers[0].data_writers[0];
    assert!(
        w.qos.is_none(),
        "writer without QoS must stay at the DDS defaults, not the publisher's policies"
    );
}

// ============================================================================
// 11. Publisher without explicit QoS — the participant QoS does not apply to
//     the publisher directly (spec: publisher QoS has its own default set), but
//     the "merge from participant" is not applied — the resolver returns
//     None, which on materialization transitions to spec defaults.
// ============================================================================
#[test]
fn publisher_without_qos_yields_none() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="0"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <publisher name="Pub1"/>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert!(r.publishers[0].qos.is_none());
}

// ============================================================================
// 12. Inline `<topic_qos>` vs `qos_profile_ref` are both resolved.
// ============================================================================
#[test]
fn topic_qos_inline_and_profile_ref_both_resolve() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <qos_library name="ql">
        <qos_profile name="P">
          <topic_qos>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          </topic_qos>
        </qos_profile>
      </qos_library>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="T" type_ref="T"/>
          <topic name="InlineQ" register_type_ref="T">
            <topic_qos>
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </topic_qos>
          </topic>
          <topic name="RefQ" register_type_ref="T" qos_profile_ref="ql::P"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="DP" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml_vendor(xml);
    let r = d.resolve_participant("dpl::DP").expect("resolve");
    let inline = r
        .topics
        .iter()
        .find(|t| t.name == "InlineQ")
        .expect("inline");
    let qos = inline.qos.as_ref().expect("inline-qos");
    assert_eq!(qos.history.unwrap().kind, HistoryKind::KeepAll);

    let refq = r.topics.iter().find(|t| t.name == "RefQ").expect("refq");
    let qos = refq.qos.as_ref().expect("ref-qos");
    assert_eq!(qos.reliability.unwrap().kind, ReliabilityKind::Reliable);
}

/// §7.3.5.4.4 / §7.3.4.4.1: an inline QoS container may inherit from a
/// QoS Profile through its `base_name` attribute. The §7.3.4.4.1 example
/// writes exactly `<topic_qos base_name="BaseQoSProfile" />`.
#[test]
fn inline_topic_qos_base_name_inherits_from_profile() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <qos_library name="ql">
        <qos_profile name="BaseProfile">
          <topic_qos>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
            <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>4</depth></history>
          </topic_qos>
        </qos_profile>
      </qos_library>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="T" type_ref="T"/>
          <topic name="T1" register_type_ref="T">
            <topic_qos base_name="ql::BaseProfile">
              <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </topic_qos>
          </topic>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="DP" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::DP").expect("resolve");
    let t = r.topics.iter().find(|t| t.name == "T1").expect("T1");
    let qos = t.qos.as_ref().expect("qos");
    // Inherited from the base profile.
    assert_eq!(qos.reliability.unwrap().kind, ReliabilityKind::Reliable);
    // Overridden inline.
    assert_eq!(qos.history.unwrap().kind, HistoryKind::KeepAll);
}

// ============================================================================
// 13. Empty `<dds/>` is a valid top level.
// ============================================================================
#[test]
fn empty_dds_root_is_valid() {
    let d = parse_dds_xml(r#"<dds/>"#).expect("parse");
    assert!(d.qos_libraries.is_empty());
    assert!(d.domain_libraries.is_empty());
    assert!(d.participant_libraries.is_empty());
    assert!(d.application_libraries.is_empty());
}

// ============================================================================
// 14. §7.1.1: a DTD is part of a well-formed [XML] document.
// ============================================================================
#[test]
fn dtd_declaration_accepted_in_dds_xml() {
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<dds><domain_library name="L"/></dds>"#;
    let d = parse_dds_xml(xml).expect("dtd");
    assert_eq!(d.domain_libraries[0].name, "L");
}

// ============================================================================
// 15. Multiple `<domain_library>` coexist in the same `<dds>`.
// ============================================================================
#[test]
fn multiple_domain_libraries_coexist() {
    let xml = r#"<dds>
      <domain_library name="A"><domain name="D1" domain_id="1"/></domain_library>
      <domain_library name="B"><domain name="D2" domain_id="2"/></domain_library>
    </dds>"#;
    let libs = parse_domain_libraries(xml).expect("parse");
    assert_eq!(libs.len(), 2);
    assert_eq!(libs[0].name, "A");
    assert_eq!(libs[1].name, "B");
}

// ============================================================================
// 16. Mixed top-level: QoS+Domain+Participant+Application in the same file
//     with cross-refs to each other.
// ============================================================================
#[test]
fn mixed_top_level_with_cross_refs() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <qos_library name="ql">
        <qos_profile name="Reliable">
          <datawriter_qos>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          </datawriter_qos>
          <datareader_qos>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          </datareader_qos>
        </qos_profile>
      </qos_library>
      <domain_library name="dl">
        <domain name="D" domain_id="5">
          <register_type name="T" type_ref="T"/>
          <topic name="T1" register_type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <publisher name="Pub">
            <data_writer name="W" topic_ref="T1" qos_profile_ref="ql::Reliable"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
      <application_library name="al">
        <application name="App">
          <domain_participant name="P" base_name="dpl::P"/>
        </application>
      </application_library>
    </dds>"#;
    let d = parse_dds_xml_vendor(xml);
    let apps = d.resolve_application("al::App").expect("resolve-app");
    assert_eq!(apps.len(), 1);
    let p = &apps[0];
    assert_eq!(p.domain_id, 5);
    let w = &p.publishers[0].data_writers[0];
    let q = w.qos.as_ref().expect("qos");
    assert_eq!(q.reliability.unwrap().kind, ReliabilityKind::Reliable);
}

// ============================================================================
// 17. A concrete deployment file combining
//     inheritance, cross-library refs, and QoS profile resolution.
// ============================================================================
#[test]
fn deployment_file_with_inheritance_and_cross_references() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-XML">
  <types>
    <module name="MyTypes"><struct name="State"/></module>
  </types>
  <qos_library name="qos_lib">
    <qos_profile name="Reliable">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
      </datawriter_qos>
    </qos_profile>
  </qos_library>
  <domain_library name="my_lib">
    <domain name="MyDomain" domain_id="42">
      <register_type name="StateType" type_ref="MyTypes::State"/>
      <topic name="StateTopic" register_type_ref="StateType"/>
    </domain>
  </domain_library>
  <domain_participant_library name="dp_lib">
    <domain_participant name="ParentParticipant" domain_ref="my_lib::MyDomain"/>
    <domain_participant name="MyParticipant" domain_ref="my_lib::MyDomain"
                        base_name="ParentParticipant">
      <publisher name="Pub1">
        <data_writer name="StateWriter" topic_ref="StateTopic"
                     qos_profile_ref="qos_lib::Reliable"/>
      </publisher>
    </domain_participant>
  </domain_participant_library>
  <application_library name="app_lib">
    <application name="MyApp">
      <domain_participant name="MyParticipant" base_name="dp_lib::MyParticipant"/>
    </application>
  </application_library>
</dds>"#;
    let d = parse_dds_xml_vendor(xml);
    let apps = d.resolve_application("app_lib::MyApp").expect("apps");
    assert_eq!(apps.len(), 1);
    let p = &apps[0];
    assert_eq!(p.domain_id, 42);
    // Three links: the two in `dp_lib` plus the definition the
    // `<application>` writes itself, which is the effective one.
    assert_eq!(p.inheritance_chain.len(), 3);
    assert_eq!(
        p.inheritance_chain.last().map(String::as_str),
        Some("app_lib::MyApp::MyParticipant")
    );
    let w = &p.publishers[0].data_writers[0];
    assert_eq!(w.topic.name, "StateTopic");
    assert_eq!(w.topic.type_name, "MyTypes::State");
    let qos = w.qos.as_ref().expect("qos");
    assert_eq!(qos.reliability.unwrap().kind, ReliabilityKind::Reliable);
    assert_eq!(qos.history.unwrap().kind, HistoryKind::KeepAll);
}

// ============================================================================
// 18. Resolve snapshot: DdsXml::resolve_participant returns a flat view.
// ============================================================================
#[test]
fn resolve_participant_returns_flat_snapshot() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <domain_library name="dl">
        <domain name="D" domain_id="11">
          <register_type name="T" type_ref="T"/>
          <topic name="X" register_type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <subscriber name="S">
            <data_reader name="R" topic_ref="X"/>
          </subscriber>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(r.lookup_path, "dpl::P");
    assert_eq!(r.name, "P");
    assert_eq!(r.domain_id, 11);
    assert_eq!(r.topics.len(), 1);
    assert_eq!(r.subscribers[0].data_readers[0].name, "R");
    assert_eq!(r.subscribers[0].data_readers[0].topic.name, "X");
}

// ============================================================================
// 19. ParticipantFactoryAdapter trait skeleton: apply_to_factory calls
//     through to the adapter.
// ============================================================================
#[test]
fn factory_adapter_skeleton_invocation() {
    use core::cell::Cell;

    struct CountingAdapter {
        calls: Cell<usize>,
    }
    impl ParticipantFactoryAdapter for CountingAdapter {
        fn apply(&self, _p: &ResolvedParticipant) -> Result<(), XmlError> {
            self.calls.set(self.calls.get() + 1);
            Ok(())
        }
    }

    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="1"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    let adapter = CountingAdapter {
        calls: Cell::new(0),
    };
    apply_to_factory(&r, &adapter).expect("apply");
    assert_eq!(adapter.calls.get(), 1);
}

// ============================================================================
// 20. Standalone library loaders: parse_application_libraries and
//     parse_domain_participant_libraries also accept documents that
//     contain only one kind of library.
// ============================================================================
#[test]
fn standalone_loaders_work_in_isolation() {
    let xml1 = r#"<dds>
      <application_library name="al">
        <application name="A">
          <domain_participant name="P" base_name="dpl::P"/>
        </application>
      </application_library>
    </dds>"#;
    let apps = parse_application_libraries(xml1).expect("parse");
    assert_eq!(apps[0].applications[0].name, "A");

    let xml2 = r#"<dds>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let dps = parse_domain_participant_libraries(xml2).expect("parse");
    assert_eq!(dps[0].participants[0].name, "P");
}

// ============================================================================
// 21. Topic without a register_type_ref match -> UnresolvedReference on resolve.
// ============================================================================
#[test]
fn topic_with_unknown_register_type_ref_errors() {
    let xml = r#"<dds>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <topic name="T1" register_type_ref="MissingType"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_participant("dpl::P").expect_err("missing");
    assert!(matches!(err, XmlError::UnresolvedReference(_)));
}

// ============================================================================
// 22. DataWriter with a qos_profile_ref to a non-existent profile.
// ============================================================================
#[test]
fn datawriter_qos_profile_ref_unresolved() {
    let xml = r#"<dds>
      <types><struct name="T"/></types>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="T" type_ref="T"/>
          <topic name="T1" register_type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <publisher name="Pub">
            <data_writer name="W" topic_ref="T1" qos_profile_ref="ql::Missing"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml_vendor(xml);
    let err = d.resolve_participant("dpl::P").expect_err("missing");
    assert!(matches!(err, XmlError::UnresolvedReference(_)));
}

// ============================================================================
// 23. find_participant / find_domain low-level lookups.
// ============================================================================
#[test]
fn find_participant_and_domain() {
    let xml = r#"<dds>
      <domain_library name="dl"><domain name="D" domain_id="3"/></domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d: DdsXml = parse_dds_xml(xml).expect("parse");
    assert_eq!(d.find_domain("dl::D").expect("d").domain_id, Some(3));
    assert_eq!(d.find_participant("dpl::P").expect("p").name, "P");
    assert!(matches!(
        d.find_domain("nope::X"),
        Err(XmlError::UnresolvedReference(_))
    ));
    assert!(matches!(
        d.find_participant("X"),
        Err(XmlError::UnresolvedReference(_))
    ));
}

// ============================================================================
// 25. §7.3.5.4.2 — a participant names its Domain with `domain_ref`, and a
//     `domain_id` of its own overrides the id of that Domain.
// ============================================================================
#[test]
fn participant_domain_id_overrides_the_referenced_domain() {
    let xml = r#"<dds>
      <domain_library name="dl"><domain name="D" domain_id="3"/></domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="Plain" domain_ref="dl::D"/>
        <domain_participant name="Override" domain_ref="dl::D" domain_id="11"/>
      </domain_participant_library>
    </dds>"#;
    let doc: DdsXml = parse_dds_xml(xml).expect("parse");
    // Without an override the participant takes the Domain's id.
    assert_eq!(
        doc.resolve_participant("dpl::Plain")
            .expect("plain")
            .domain_id,
        3
    );
    // With an override the participant's own id wins, and the Domain it
    // refers to is still the one named by `domain_ref`.
    let overridden = doc.resolve_participant("dpl::Override").expect("override");
    assert_eq!(overridden.domain_id, 11);
    assert_eq!(overridden.domain.name, "D");

    // A participant that names no Domain at all cannot be resolved — the
    // missing reference is reported rather than read as domain 0.
    let no_ref = r#"<dds>
      <domain_participant_library name="dpl">
        <domain_participant name="P"/>
      </domain_participant_library>
    </dds>"#;
    let doc2: DdsXml = parse_dds_xml(no_ref).expect("parse");
    assert!(matches!(
        doc2.resolve_participant("dpl::P"),
        Err(XmlError::UnresolvedReference(_))
    ));
}

// ============================================================================
// 26. §7.3.2.2 / §7.3.3.2 / §7.3.7.2 — the QoS, Types and Data Samples
//     building blocks have no dependency on any other building block, so a
//     document that contains only their own elements parses completely.
// ============================================================================
#[test]
fn independent_building_blocks_parse_without_the_other_blocks() {
    // Building Block QoS on its own — no <types>, <domain_library>,
    // <domain_participant_library> or <application_library> anywhere.
    let qos_only = r#"<dds>
      <qos_library name="ql">
        <qos_profile name="P">
          <datawriter_qos>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
          </datawriter_qos>
        </qos_profile>
      </qos_library>
    </dds>"#;
    let libs = parse_qos_libraries(qos_only).expect("QoS building block alone");
    assert_eq!(libs[0].profiles[0].name, "P");
    assert_eq!(
        libs[0].profiles[0]
            .datawriter_qos
            .first()
            .expect("dw qos")
            .qos
            .reliability
            .expect("reliability")
            .kind,
        ReliabilityKind::Reliable
    );

    // Building Block Types on its own.
    let types_only = r#"<dds>
      <types>
        <struct name="ShapeType">
          <member name="color" type="string" key="true"/>
          <member name="x" type="int32"/>
        </struct>
      </types>
    </dds>"#;
    let type_libs = parse_type_libraries(types_only).expect("Types building block alone");
    let TypeDef::Struct(shape) = &type_libs[0].types[0] else {
        panic!("expected a struct");
    };
    assert_eq!(shape.name, "ShapeType");
    assert!(shape.members[0].key);

    // Building Block Data Samples on its own: the sample document names no
    // other building block, the type comes in as a parameter.
    let sample_only = r#"<sample type_ref="ShapeType"><color>BLUE</color><x>7</x></sample>"#;
    let value = parse_sample(sample_only, &type_libs[0].types[0], &[])
        .expect("Data Samples building block alone");
    let SampleValue::Struct(members) = value else {
        panic!("expected a struct sample");
    };
    assert!(matches!(
        members.get("x"),
        Some(SampleValue::Primitive(PrimitiveValue::I32(7)))
    ));
}

// ============================================================================
// 27. §7.3.5.4.1 — a misplaced or unknown child is reported, not dropped.
// ============================================================================
#[test]
fn misplaced_or_unknown_children_are_rejected() {
    // A DataReader under <publisher> (the ordering §7.3.5.4.1 forbids),
    // a misspelt QoS container, an unknown element under <domain> and a
    // misspelt <content_filtr> (the schema's <content_filter> is read).
    let cases: &[&str] = &[
        r#"<dds><domain_participant_library name="dpl">
             <domain_participant name="P" domain_ref="dl::D">
               <publisher name="Pub"><data_reader name="R" topic_ref="T"/></publisher>
             </domain_participant>
           </domain_participant_library></dds>"#,
        r#"<dds><domain_participant_library name="dpl">
             <domain_participant name="P" domain_ref="dl::D">
               <topic_qoss/>
             </domain_participant>
           </domain_participant_library></dds>"#,
        r#"<dds><domain_library name="dl">
             <domain name="D" domain_id="0"><nonsense/></domain>
           </domain_library></dds>"#,
        r#"<dds><domain_participant_library name="dpl">
             <domain_participant name="P" domain_ref="dl::D">
               <subscriber name="S">
                 <data_reader name="R" topic_ref="T"><content_filtr/></data_reader>
               </subscriber>
             </domain_participant>
           </domain_participant_library></dds>"#,
    ];
    for xml in cases {
        let err = parse_dds_xml(xml).expect_err("unknown-child");
        assert!(
            matches!(err, XmlError::UnknownElement(_)),
            "expected UnknownElement, got {err:?} for {xml}"
        );
    }
}

// ============================================================================
// 28. §7.3.4.4.2 — a domain inherits the topics and registered types of its
//     base, not only the domain_id.
// ============================================================================
#[test]
fn domain_inheritance_carries_topics_and_types() {
    let xml = r#"<dds>
      <types>
        <module name="MyTypes"><struct name="Base"/></module>
      </types>
      <domain_library name="dl">
        <domain name="BaseDomain" domain_id="7">
          <register_type name="BaseType" type_ref="MyTypes::Base"/>
          <topic name="BaseTopic" register_type_ref="BaseType"/>
        </domain>
        <domain name="DerivedDomain" base_name="BaseDomain"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::DerivedDomain">
          <publisher name="Pub">
            <data_writer name="W" topic_ref="BaseTopic"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(r.domain_id, 7);
    let w = &r.publishers[0].data_writers[0];
    assert_eq!(w.topic.name, "BaseTopic");
    assert_eq!(w.topic.type_name, "MyTypes::Base");
}

// ============================================================================
// 29. §7.3.4.4.1 — a `type_ref` that names no type in a `<types>` block is
//     reported when the topic is resolved.
// ============================================================================
#[test]
fn register_type_with_dangling_type_ref_errors() {
    let xml = r#"<dds>
      <types><struct name="Present"/></types>
      <domain_library name="dl">
        <domain name="D" domain_id="0">
          <register_type name="Reg" type_ref="Absent"/>
          <topic name="T1" register_type_ref="Reg"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <publisher name="Pub"><data_writer name="W" topic_ref="T1"/></publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_participant("dpl::P").expect_err("dangling");
    assert!(
        matches!(err, XmlError::UnresolvedReference(_)),
        "got {err:?}"
    );
}

// ============================================================================
// 30. §7.3.5.4.2 — `domain_ref` may be unqualified, and a participant may
//     name its Domain only numerically.
// ============================================================================
#[test]
fn unqualified_domain_ref_resolves() {
    let xml = r#"<dds>
      <domain_library name="ShapesDomainLibrary">
        <domain name="ShapesDomain" domain_id="5"/>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="ShapesDomain"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(r.domain_id, 5);
    assert_eq!(r.domain.name, "ShapesDomain");
}

#[test]
fn participant_without_domain_ref_but_with_domain_id_resolves() {
    let xml = r#"<dds>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_id="32"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let r = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(r.domain_id, 32);
    assert!(r.domain.name.is_empty());
}

// ============================================================================
// 31. §7.3.5.4.3 — a participant may only inherit from participants defined
//     before it.
// ============================================================================
#[test]
fn participant_forward_base_name_rejected() {
    let xml = r#"<dds>
      <domain_library name="dl"><domain name="D" domain_id="0"/></domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="Child" domain_ref="dl::D" base_name="Base"/>
        <domain_participant name="Base" domain_ref="dl::D"/>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let err = d.resolve_participant("dpl::Child").expect_err("forward");
    assert!(matches!(err, XmlError::InvalidXml(_)), "got {err:?}");
}

// ============================================================================
// 32. §7.3.5.4.2 — a participant's own <register_type>/<topic> children are
//     definitions that join the ones its domain makes available.
// ============================================================================
#[test]
fn participant_local_definitions_resolve_writers() {
    let xml = r#"<dds>
      <types>
        <module name="MyTypes">
          <struct name="State"/>
          <struct name="Local"/>
        </module>
      </types>
      <domain_library name="dl">
        <domain name="D" domain_id="5">
          <register_type name="StateType" type_ref="MyTypes::State"/>
          <topic name="StateTopic" register_type_ref="StateType"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <register_type name="LocalType" type_ref="MyTypes::Local"/>
          <topic name="LocalTopic" register_type_ref="LocalType"/>
          <publisher name="Pub">
            <data_writer name="W1" topic_ref="LocalTopic"/>
            <data_writer name="W2" topic_ref="StateTopic"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let p = d.resolve_participant("dpl::P").expect("resolve");
    // Both sets are visible: the domain's topic and the participant's
    // own definition, each with its own registered type.
    let names: Vec<&str> = p.topics.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["StateTopic", "LocalTopic"]);
    assert_eq!(
        p.domain
            .topic("LocalTopic")
            .expect("local")
            .register_type_ref,
        "LocalType"
    );
    assert_eq!(
        p.publishers[0].data_writers[0].topic.type_name,
        "MyTypes::Local"
    );
    assert_eq!(
        p.publishers[0].data_writers[1].topic.type_name,
        "MyTypes::State"
    );
}

/// A definition of the same name overrides the domain's entry, the same
/// way a derived participant definition overrides its base.
#[test]
fn participant_definition_overrides_the_domains() {
    let xml = r#"<dds>
      <types>
        <module name="MyTypes">
          <struct name="A"/>
          <struct name="B"/>
        </module>
      </types>
      <domain_library name="dl">
        <domain name="D" domain_id="5">
          <register_type name="T" type_ref="MyTypes::A"/>
          <topic name="S" register_type_ref="T"/>
        </domain>
      </domain_library>
      <domain_participant_library name="dpl">
        <domain_participant name="P" domain_ref="dl::D">
          <register_type name="T" type_ref="MyTypes::B"/>
          <publisher name="Pub">
            <data_writer name="W" topic_ref="S"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let p = d.resolve_participant("dpl::P").expect("resolve");
    assert_eq!(
        p.publishers[0].data_writers[0].topic.type_name,
        "MyTypes::B"
    );
}

/// A participant that names only `domain_id` (no `domain_ref`, as in the
/// OMG's own dds-xml_domainparticipants_example.xml) still resolves
/// writers at its own topics — there is no domain to look them up in.
#[test]
fn participant_without_domain_resolves_its_own_topics() {
    let xml = r#"<dds>
      <types><struct name="Hello"/></types>
      <domain_participant_library name="dpl">
        <domain_participant name="HelloParticipant" domain_id="32">
          <register_type name="HelloWorld" type_ref="Hello"/>
          <topic name="HelloTopic" register_type_ref="HelloWorld"/>
          <publisher name="Pub">
            <data_writer name="HelloWriter" topic_ref="HelloTopic"/>
          </publisher>
        </domain_participant>
      </domain_participant_library>
    </dds>"#;
    let d = parse_dds_xml(xml).expect("parse");
    let p = d
        .resolve_participant("dpl::HelloParticipant")
        .expect("resolve");
    assert_eq!(p.domain_id, 32);
    assert_eq!(p.publishers[0].data_writers[0].topic.name, "HelloTopic");
    assert_eq!(p.publishers[0].data_writers[0].topic.type_name, "Hello");
}

// ============================================================================
// 33. §7.3.5.4.4 — inline QoS is one container per entity. A second one would
//     replace the first without a word.
// ============================================================================
#[test]
fn repeated_inline_qos_container_is_rejected() {
    let wrap = |entity: &str| {
        format!(
            r#"<dds><domain_participant_library name="dpl">
                 <domain_participant name="P" domain_id="1">{entity}</domain_participant>
               </domain_participant_library></dds>"#
        )
    };
    let history = "<history><kind>KEEP_ALL_HISTORY_QOS</kind></history>";
    let cases = [
        wrap("<domain_participant_qos/><domain_participant_qos/>"),
        wrap("<publisher name=\"Pub\"><publisher_qos/><publisher_qos/></publisher>"),
        wrap("<subscriber name=\"Sub\"><subscriber_qos/><subscriber_qos/></subscriber>"),
        wrap(&format!(
            "<publisher name=\"Pub\"><data_writer name=\"W\" topic_ref=\"T\">\
             <datawriter_qos>{history}</datawriter_qos><datawriter_qos/></data_writer></publisher>"
        )),
        wrap(&format!(
            "<subscriber name=\"Sub\"><data_reader name=\"R\" topic_ref=\"T\">\
             <datareader_qos>{history}</datareader_qos><datareader_qos/></data_reader></subscriber>"
        )),
        r#"<dds><domain_library name="dl"><domain name="D" domain_id="0">
             <topic name="T" register_type_ref="R"><topic_qos/><topic_qos/></topic>
           </domain></domain_library></dds>"#
            .to_string(),
    ];
    for xml in &cases {
        let err = parse_dds_xml(xml).expect_err("repeated container");
        assert!(
            matches!(err, XmlError::InvalidXml(_)),
            "expected InvalidXml, got {err:?} for {xml}"
        );
    }
}
