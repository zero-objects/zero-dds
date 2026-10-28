// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! Documents the normative schemas of DDS-XML 1.0 define, read by the
//! parsers: what the schema allows, the reader reads and keeps.
//!
//! `tests/schema_validation.rs` checks the same documents against the
//! schema with `xmllint` (`XSD_VALID`); this file checks what the reader
//! makes of them.

use zerodds_xml::{
    Diagnostic, DiagnosticKind, ReadOptions, XmlError, parse_application_libraries, parse_dds_xml,
    parse_dds_xml_with, parse_domain_libraries, parse_domain_libraries_with,
    parse_domain_participant_libraries, parse_domain_participant_libraries_with,
    parse_qos_libraries, parse_qos_libraries_with, parse_type_libraries_with, resolve_profile,
};

/// §7.3.2.4.3.1's example: `initial_samples` and `initial_instances` are
/// fields of the schema's `resourceLimitsQosPolicy`. DDS 1.4's structure
/// has no place for them; the model carries them next to the policy.
#[test]
fn resource_limits_initial_fields_are_carried() {
    let xml = r#"<qos_library name="L"><qos_profile name="StrictReliableCommunicationProfile">
        <datawriter_qos topic_filter="B*">
            <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
            <resource_limits>
                <max_samples>128</max_samples>
                <max_samples_per_instance>128</max_samples_per_instance>
                <initial_samples>64</initial_samples>
                <max_instances>1</max_instances>
                <initial_instances>1</initial_instances>
            </resource_limits>
        </datawriter_qos>
    </qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("the example parses");
    let qos = &libs[0].profiles[0].datawriter_qos[0].qos;
    let limits = qos.resource_limits.expect("limits");
    assert_eq!(limits.max_samples, 128);
    assert_eq!(limits.max_instances, 1);
    assert_eq!(limits.max_samples_per_instance, 128);
    let initial = qos.initial_resource_limits.expect("initial limits carried");
    assert_eq!(initial.initial_samples, Some(64));
    assert_eq!(initial.initial_instances, Some(1));
}

/// The numeric simple types of the schema allow the empty string; an empty
/// element has the value the DDS gives an omitted field.
#[test]
fn empty_numeric_elements_are_the_default() {
    let xml = r#"<qos_library name="L"><qos_profile name="P"><datawriter_qos>
        <deadline><period><sec></sec><nanosec/></period></deadline>
        <history><depth/></history>
        <resource_limits><max_samples/></resource_limits>
        <ownership_strength><value></value></ownership_strength>
    </datawriter_qos></qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let qos = &libs[0].profiles[0].datawriter_qos[0].qos;
    let period = qos.deadline.unwrap().period;
    assert_eq!((period.seconds, period.fraction), (0, 0));
    assert_eq!(qos.history.unwrap().depth, 1);
    assert_eq!(qos.resource_limits.unwrap().max_samples, -1);
    assert_eq!(qos.ownership_strength.unwrap().value, 0);
}

/// The schema's `userDataQosPolicy` is one `<value>` of
/// `xs:base64Binary`, empty in the OMG's own example.
#[test]
fn user_data_value_is_base64_and_may_be_empty() {
    let xml = r#"<qos_library name="L"><qos_profile name="P">
        <domain_participant_qos>
            <user_data><value>aGk=</value></user_data>
        </domain_participant_qos>
        <publisher_qos><group_data><value></value></group_data></publisher_qos>
    </qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let p = &libs[0].profiles[0];
    assert_eq!(
        p.domainparticipant_qos[0]
            .qos
            .user_data
            .as_ref()
            .unwrap()
            .value,
        b"hi"
    );
    assert!(
        p.publisher_qos[0]
            .qos
            .group_data
            .as_ref()
            .unwrap()
            .value
            .is_empty()
    );
}

/// `<partition><name></name></partition>` (the OMG's example): a sequence
/// of no strings.
#[test]
fn empty_partition_name_is_an_empty_sequence() {
    let xml = r#"<qos_library name="L"><qos_profile name="P">
        <publisher_qos><partition><name></name></partition></publisher_qos>
    </qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).unwrap();
    let part = libs[0].profiles[0].publisher_qos[0]
        .qos
        .partition
        .as_ref()
        .unwrap();
    assert!(part.names.is_empty());
}

/// `qosLibrary` does not require the `name` of a container directly in it.
#[test]
fn unnamed_container_in_a_library_is_kept() {
    let xml = r#"<qos_library name="L"><topic_qos><durability/></topic_qos></qos_library>"#;
    let libs = parse_qos_libraries(xml).unwrap();
    assert_eq!(libs[0].profiles.len(), 1);
    assert_eq!(libs[0].profiles[0].name, "");
    assert!(libs[0].profiles[0].topic_qos[0].qos.durability.is_some());
}

/// Every QoS container may repeat in a profile (`qosProfile` of the
/// schema); containers with the same filter merge, others stay apart.
#[test]
fn repeated_containers_of_every_kind() {
    let xml = r#"<qos_library name="L"><qos_profile name="P">
        <subscriber_qos><partition><name><element>a</element></name></partition></subscriber_qos>
        <subscriber_qos><entity_factory/></subscriber_qos>
        <domain_participant_qos topic_filter="X*"><entity_factory/></domain_participant_qos>
        <domain_participant_qos><user_data/></domain_participant_qos>
    </qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).unwrap();
    let p = &libs[0].profiles[0];
    assert_eq!(p.subscriber_qos.len(), 1);
    assert!(p.subscriber_qos[0].qos.partition.is_some());
    assert!(p.subscriber_qos[0].qos.entity_factory.is_some());
    assert_eq!(p.domainparticipant_qos.len(), 2);
}

/// Every QoS container has `base_name` in the schema. In a profile it
/// names a profile whose container of the same kind is the base.
#[test]
fn container_base_name_in_a_profile_inherits() {
    let xml = r#"<qos_library name="L">
        <qos_profile name="Base">
            <datawriter_qos>
                <reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability>
                <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>9</depth></history>
            </datawriter_qos>
        </qos_profile>
        <qos_profile name="Derived">
            <datawriter_qos base_name="Base">
                <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            </datawriter_qos>
        </qos_profile>
        <qos_profile name="Loop">
            <datawriter_qos base_name="Loop"/>
        </qos_profile>
    </qos_library>"#;
    let libs = parse_qos_libraries(xml).unwrap();
    let r = resolve_profile(&libs, "L::Derived").expect("resolve");
    let dw = &r.datawriter_qos[0].qos;
    assert_eq!(
        dw.reliability.unwrap().kind,
        zerodds_qos::ReliabilityKind::BestEffort,
        "inherited through the container's base_name"
    );
    assert_eq!(dw.history.unwrap().kind, zerodds_qos::HistoryKind::KeepAll);
    assert!(dw.base_name.is_none());
    // A container that names its own profile never settles.
    assert!(resolve_profile(&libs, "L::Loop").is_err());
}

/// `registerType` has a `format` attribute and a `<registered_name>` child,
/// `topic` a `<registered_name>` child.
#[test]
fn registered_name_and_format_are_carried() {
    let xml = r#"<domain_library name="DL"><domain name="D" domain_id="1">
        <register_type name="R" type_ref="T" format="xtypes">
            <registered_name>RegisteredT</registered_name>
        </register_type>
        <topic name="Tp" register_type_ref="R">
            <registered_name>Alias</registered_name>
            <topic_qos base_name="Q"/>
        </topic>
    </domain></domain_library>"#;
    let libs = parse_domain_libraries(xml).unwrap();
    let d = &libs[0].domains[0];
    assert_eq!(d.register_types[0].format.as_deref(), Some("xtypes"));
    assert_eq!(
        d.register_types[0].registered_name.as_deref(),
        Some("RegisteredT")
    );
    assert_eq!(d.topics[0].registered_name.as_deref(), Some("Alias"));
    assert!(d.topics[0].topic_qos.is_some());
}

/// The schema's `dataReader` holds `content_filter`, `status_condition`,
/// `read_condition` and `query_condition` next to the QoS.
#[test]
fn data_reader_filters_and_conditions_are_read() {
    let xml = r#"<domain_participant_library name="L">
      <domain_participant name="P" domain_id="1">
        <subscriber name="S">
          <data_reader name="R" topic_ref="T">
            <content_filter name="f" kind="builtin.sql">
              <expression>x &gt; %0 AND y = %1</expression>
              <parameter_list><param>5</param><param>7</param></parameter_list>
            </content_filter>
            <status_condition>
              <status>DATA_AVAILABLE</status>
              <status>SUBSCRIPTION_MATCHED</status>
            </status_condition>
            <read_condition name="rc">
              <sample_state>NOT_READ</sample_state>
              <view_state>NEW</view_state>
              <instance_state>ALIVE</instance_state>
            </read_condition>
            <query_condition name="q" kind="builtin.stringMatch"/>
            <datareader_qos/>
          </data_reader>
        </subscriber>
      </domain_participant>
    </domain_participant_library>"#;
    let libs = parse_domain_participant_libraries(xml).unwrap();
    let r = &libs[0].participants[0].subscribers[0].data_readers[0];
    let f = r.content_filter.as_ref().unwrap();
    assert_eq!((f.name.as_str(), f.kind.as_str()), ("f", "builtin.sql"));
    assert_eq!(f.expression.as_deref(), Some("x > %0 AND y = %1"));
    assert_eq!(f.parameters, ["5", "7"]);
    assert_eq!(
        r.status_conditions,
        ["DATA_AVAILABLE", "SUBSCRIPTION_MATCHED"]
    );
    assert_eq!(r.read_conditions[0].name, "rc");
    assert_eq!(r.read_conditions[0].sample_states, ["NOT_READ"]);
    assert_eq!(r.read_conditions[0].view_states, ["NEW"]);
    assert_eq!(r.read_conditions[0].instance_states, ["ALIVE"]);
    assert_eq!(r.query_conditions[0].kind, "builtin.stringMatch");
    assert!(r.qos.is_some());

    // The enumerations of the schema are checked.
    for bad in [
        r#"<content_filter name="f" kind="sql"/>"#,
        r#"<status_condition><status>NOPE</status></status_condition>"#,
        r#"<read_condition name="c"><view_state>OLD</view_state></read_condition>"#,
    ] {
        let doc = format!(
            r#"<domain_participant_library name="L"><domain_participant name="P" domain_id="1"><subscriber name="S"><data_reader name="R" topic_ref="T">{bad}</data_reader></subscriber></domain_participant></domain_participant_library>"#
        );
        assert!(
            matches!(
                parse_domain_participant_libraries(&doc),
                Err(XmlError::BadEnum(_))
            ),
            "{bad}"
        );
    }
}

/// The participant example of the specification: elements in any order,
/// `domain_participant_qos` after the entities, `subscriber_qos` after the
/// readers.
#[test]
fn the_participant_example_of_the_specification() {
    let xml = r#"<domain_participant_library name="MyDomainLibrary">
      <domain_participant name="ShapesParticipant" domain_ref="ADomainLibrary::ShapeDomain" domain_id="127">
        <publisher name="MyPublisher">
          <data_writer name="MySquareWriter" topic_ref="Square" />
          <publisher_qos><entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory></publisher_qos>
        </publisher>
        <subscriber name="MyPublisher">
          <data_reader name="MySquareWriter" topic_ref="Square" />
          <subscriber_qos base_name="SomeQosLibrary::SomeQosProfile">
            <entity_factory><autoenable_created_entities>false</autoenable_created_entities></entity_factory>
          </subscriber_qos>
        </subscriber>
        <domain_participant_qos>
          <entity_factory><autoenable_created_entities>true</autoenable_created_entities></entity_factory>
        </domain_participant_qos>
      </domain_participant>
    </domain_participant_library>"#;
    let libs = parse_domain_participant_libraries(xml).unwrap();
    let p = &libs[0].participants[0];
    assert_eq!(p.domain_id, Some(127));
    assert!(p.publishers[0].qos.is_some());
    assert_eq!(
        p.subscribers[0].qos.as_ref().unwrap().base_name.as_deref(),
        Some("SomeQosLibrary::SomeQosProfile")
    );
    assert!(p.qos.is_some());
}

/// The application example: a participant that only inherits.
#[test]
fn application_participant_with_base_name_and_domain_id() {
    let xml = r#"<application_library name="A"><application name="X">
        <domain_participant name="G" base_name="Lib::G" domain_id="64" />
    </application></application_library>"#;
    let libs = parse_application_libraries(xml).unwrap();
    let p = &libs[0].applications[0].domain_participants[0];
    assert_eq!(p.base_name.as_deref(), Some("Lib::G"));
    assert_eq!(p.domain_id, Some(64));
}

// ---------------------------------------------------------------------
// The reading option for what the schema does not define
// ---------------------------------------------------------------------

const RTI_STYLE: &str = r#"<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <qos_library name="L">
    <qos_profile name="P" is_default_qos="true">
      <datawriter_qos>
        <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        <publish_mode><kind>ASYNCHRONOUS_PUBLISH_MODE_QOS</kind></publish_mode>
        <ownership_strength><value>3</value></ownership_strength>
      </datawriter_qos>
      <datareader_qos>
        <ownership_strength><value>3</value></ownership_strength>
        <history><kind>KEEP_ALL_HISTORY_QOS</kind><bogus/></history>
      </datareader_qos>
      <participant_factory_qos/>
    </qos_profile>
  </qos_library>
  <vendor_block/>
</dds>"#;

#[test]
fn unknown_elements_are_refused_by_default() {
    let err = parse_dds_xml(RTI_STYLE).unwrap_err();
    assert!(
        matches!(&err, XmlError::UnknownElement(m) if m.contains("is_default_qos")),
        "{err:?}"
    );
}

#[test]
fn report_and_skip_lists_everything_it_leaves_out() {
    let parsed = parse_dds_xml_with(RTI_STYLE, &ReadOptions::report_and_skip()).expect("parse");
    let paths: Vec<(&str, DiagnosticKind)> = parsed
        .diagnostics
        .iter()
        .map(|d| (d.path.as_str(), d.kind))
        .collect();
    assert_eq!(
        paths,
        [
            (
                "dds/qos_library[L]/qos_profile[P]@is_default_qos",
                DiagnosticKind::UnknownAttribute
            ),
            (
                "dds/qos_library[L]/qos_profile[P]/datawriter_qos/publish_mode",
                DiagnosticKind::UnknownElement
            ),
            (
                "dds/qos_library[L]/qos_profile[P]/datareader_qos/ownership_strength",
                DiagnosticKind::MisplacedElement
            ),
            (
                "dds/qos_library[L]/qos_profile[P]/datareader_qos/history/bogus",
                DiagnosticKind::UnknownElement
            ),
            (
                "dds/qos_library[L]/qos_profile[P]/participant_factory_qos",
                DiagnosticKind::UnknownElement
            ),
            ("dds/vendor_block", DiagnosticKind::UnknownElement),
        ]
    );
    // What is modelled is read.
    let dw = &parsed.value.qos_libraries[0].profiles[0].datawriter_qos[0].qos;
    assert!(dw.reliability.is_some());
    assert_eq!(dw.ownership_strength.unwrap().value, 3);
    let dr = &parsed.value.qos_libraries[0].profiles[0].datareader_qos[0].qos;
    assert!(dr.ownership_strength.is_none(), "misplaced, left out");
    assert!(dr.history.is_some());
}

#[test]
fn report_and_skip_is_silent_about_nothing_and_empty_for_clean_documents() {
    let clean = r#"<qos_library name="L"><qos_profile name="P"/></qos_library>"#;
    let parsed = parse_qos_libraries_with(clean, &ReadOptions::report_and_skip()).unwrap();
    assert!(parsed.diagnostics.is_empty());
    // Errors that are no unknown element stay errors under both settings.
    let bad = r#"<qos_library name="L"><qos_profile name="P"><datawriter_qos><deadline><period><sec>x</sec></period></deadline></datawriter_qos></qos_profile></qos_library>"#;
    assert!(parse_qos_libraries_with(bad, &ReadOptions::report_and_skip()).is_err());
}

// ---------------------------------------------------------------------
// Types: the grammar of the schema, constant expressions
// ---------------------------------------------------------------------

use zerodds_xml::{
    AutoId, Extensibility, PrimitiveType, TryConstruct, TypeDef, TypeRef, parse_type_libraries,
};

/// XTypes 1.3 §7.3.2.4: bounds and dimensions are constant expressions.
/// The examples are the ones of the text.
#[test]
fn bounds_and_dimensions_are_evaluated() {
    let xml = r#"<types>
        <const name="MY_CONSTANT" type="int32" value="3"/>
        <const name="DOUBLE" type="int32" value="MY_CONSTANT * 2"/>
        <module name="M">
            <const name="LOCAL" type="uint32" value="0x10"/>
            <struct name="S">
                <const name="IN_STRUCT" type="int32" value="LOCAL + 1"/>
                <member name="bounded_string" type="string" stringMaxLength="2 + MY_CONSTANT"/>
                <member name="seq" type="nonBasic" nonBasicTypeName="Other" sequenceMaxLength="6 * 3"/>
                <member name="arr" type="int32" arrayDimensions="2, MY_CONSTANT + 3"/>
                <member name="arr2" type="int32" arrayDimensions=" 6,2, 3 "/>
                <member name="m" type="float32" mapKeyType="string" mapKeyStringMaxLength="DOUBLE"
                        mapMaxLength="LOCAL"/>
                <member name="in_struct" type="string" stringMaxLength="IN_STRUCT"/>
                <member name="unbounded" type="string" stringMaxLength="-1"/>
            </struct>
        </module>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Module(m) = &libs[0].types[2] else {
        panic!("expected module");
    };
    let TypeDef::Struct(s) = &m.types[1] else {
        panic!("expected struct");
    };
    let by_name = |n: &str| s.members.iter().find(|x| x.name == n).unwrap();
    assert_eq!(by_name("bounded_string").string_max_length, Some(5));
    assert_eq!(by_name("seq").sequence_max_length, Some(18));
    assert_eq!(by_name("arr").array_dimensions, [2, 6]);
    assert_eq!(by_name("arr2").array_dimensions, [6, 2, 3]);
    assert_eq!(by_name("m").map_key_string_max_length, Some(6));
    assert_eq!(by_name("m").map_max_length, Some(16));
    // A constant declared in the struct is scoped to it.
    assert_eq!(by_name("in_struct").string_max_length, Some(17));
    assert_eq!(by_name("unbounded").string_max_length, None);
    assert_eq!(s.constants.len(), 1);
}

#[test]
fn bad_bounds_are_errors_that_name_the_attribute() {
    for (attr, why) in [
        (r#"stringMaxLength="NOPE""#, "names no <const>"),
        (r#"stringMaxLength="-2""#, "only -1 is negative"),
        (r#"sequenceMaxLength="0""#, "0 collides with unbounded"),
        (r#"arrayDimensions="0""#, "a dimension is at least 1"),
        (r#"arrayDimensions="3,""#, "empty dimension"),
        (r#"stringMaxLength="1 +""#, "syntax"),
    ] {
        let xml = format!(
            r#"<types><struct name="S"><member name="m" type="string" {attr}/></struct></types>"#
        );
        let err = parse_type_libraries(&xml).expect_err(why);
        assert!(
            err.to_string().contains("@") || matches!(err, XmlError::UnresolvedReference(_)),
            "{attr}: {err}"
        );
    }
}

#[test]
fn enumerator_and_case_values_are_expressions() {
    let xml = r#"<types>
        <const name="BASE" type="int32" value="10"/>
        <enum name="E">
            <enumerator name="A" value="BASE"/>
            <enumerator name="B" value="BASE + 1"/>
            <enumerator name="C" value="0+1"/>
        </enum>
        <union name="U">
            <discriminator type="nonBasic" nonBasicTypeName="E"/>
            <case><caseDiscriminator value="BASE * 2"/><member name="x" type="int32"/></case>
            <case><caseDiscriminator value="B"/><member name="y" type="int32"/></case>
        </union>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let TypeDef::Enum(e) = &libs[0].types[1] else {
        panic!()
    };
    let values: Vec<_> = e.enumerators.iter().map(|l| l.value).collect();
    assert_eq!(values, [Some(10), Some(11), Some(1)]);
    let TypeDef::Union(u) = &libs[0].types[2] else {
        panic!()
    };
    // An expression is evaluated; a name that is no <const> (here the
    // enumerator B) stays as written.
    assert!(matches!(
        &u.cases[0].discriminators[0],
        zerodds_xml::UnionDiscriminator::Value(v) if v == "20"
    ));
    assert!(matches!(
        &u.cases[1].discriminators[0],
        zerodds_xml::UnionDiscriminator::Value(v) if v == "B"
    ));
}

/// `<annotate>`, `<verbatim>`, `<const>` in a struct, `<annotation>` and the
/// attributes of Annex A are read into the model.
#[test]
fn annotations_verbatims_and_flags_are_read() {
    let xml = r#"<types>
        <annotation name="MyAnn" baseType="Base">
            <member name="level" type="int32"/>
        </annotation>
        <struct name="S" extensibility="append" nested="true" autoid="hash">
            <annotate name="MyAnn"><member name="level" value="3"/></annotate>
            <verbatim language="c++" placement="after-declaration"><text>// hi</text></verbatim>
            <member name="a" type="int32" key="true" id="7" external="true"
                    tryConstruct="trim" nonSerialized="false" mustUnderstand="true">
                <annotate name="Doc"/>
            </member>
        </struct>
        <enum name="E" bitBound="8">
            <annotate name="EAnn"/>
            <verbatim><text>x</text></verbatim>
            <enumerator name="A" value="1" defaultLiteral="true"><annotate name="LAnn"/></enumerator>
        </enum>
        <bitmask name="B" bitBound="8">
            <annotate name="BAnn"/>
            <flag name="F0" position="0"><annotate name="FAnn"/></flag>
        </bitmask>
        <typedef name="T" type="int32" external="true"/>
    </types>"#;
    let libs = parse_type_libraries(xml).expect("parse");
    let t = &libs[0].types;
    let TypeDef::Annotation(a) = &t[0] else {
        panic!("expected annotation")
    };
    assert_eq!(a.base_type.as_deref(), Some("Base"));
    assert_eq!(a.members[0].name, "level");
    let TypeDef::Struct(s) = &t[1] else { panic!() };
    assert_eq!(s.extensibility, Some(Extensibility::Appendable));
    assert_eq!(s.nested, Some(true));
    assert_eq!(s.auto_id, Some(AutoId::Hash));
    assert_eq!(s.annotations[0].name, "MyAnn");
    assert_eq!(s.annotations[0].members[0].value.as_deref(), Some("3"));
    assert_eq!(s.verbatims[0].language, "c++");
    assert_eq!(s.verbatims[0].placement, "after-declaration");
    assert_eq!(s.verbatims[0].text, "// hi");
    let m = &s.members[0];
    assert!(m.key && m.must_understand);
    assert_eq!(m.id, Some(7));
    assert_eq!(m.external, Some(true));
    assert_eq!(m.try_construct, Some(TryConstruct::Trim));
    assert_eq!(m.non_serialized, Some(false));
    assert_eq!(m.annotations[0].name, "Doc");
    let TypeDef::Enum(e) = &t[2] else { panic!() };
    assert_eq!(e.annotations[0].name, "EAnn");
    assert_eq!(e.verbatims[0].language, "*");
    assert_eq!(e.verbatims[0].placement, "before-declaration");
    assert_eq!(e.enumerators[0].default_literal, Some(true));
    assert_eq!(e.enumerators[0].annotations[0].name, "LAnn");
    let TypeDef::Bitmask(b) = &t[3] else { panic!() };
    assert_eq!(b.annotations[0].name, "BAnn");
    assert_eq!(b.bit_values[0].annotations[0].name, "FAnn");
    let TypeDef::Typedef(td) = &t[4] else {
        panic!()
    };
    assert_eq!(td.external, Some(true));
    assert!(matches!(
        &td.type_ref,
        TypeRef::Primitive(PrimitiveType::Long)
    ));
}

/// What the schema's types grammar does not define is refused with its
/// path, or reported and left out.
#[test]
fn unknown_type_elements_are_refused_or_reported() {
    let xml = r#"<types>
        <struct name="S" colour="red">
            <member name="a" type="int32" bogus="1"/>
            <future/>
        </struct>
        <enum name="E"><enumerator name="A"/><extra/></enum>
    </types>"#;
    let err = parse_type_libraries(xml).unwrap_err();
    assert!(
        matches!(&err, XmlError::UnknownElement(m) if m.contains("types/struct[S]@colour")),
        "{err:?}"
    );
    let parsed = parse_type_libraries_with(xml, &ReadOptions::report_and_skip()).unwrap();
    let paths: Vec<&str> = parsed.diagnostics.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "types/struct[S]@colour",
            "types/struct[S]/member[a]@bogus",
            "types/struct[S]/future",
            "types/enum[E]/extra",
        ]
    );
    assert_eq!(parsed.value[0].types.len(), 2, "what is modelled is read");
}

/// The same holds for a `<types>` block inside `<dds>`: the options reach it.
#[test]
fn unknown_type_elements_inside_dds_follow_the_options() {
    let xml = r#"<dds><types><struct name="S"><member name="a" type="int32"/><future/></struct></types></dds>"#;
    assert!(parse_dds_xml(xml).is_err());
    let parsed = parse_dds_xml_with(xml, &ReadOptions::report_and_skip()).unwrap();
    let paths: Vec<&str> = parsed.diagnostics.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(paths, ["dds/types/struct[S]/future"]);
}

// ---------------------------------------------------------------------
// Vendor forms: refused by default, read and reported on request
// ---------------------------------------------------------------------

type Loader = fn(&str, &ReadOptions) -> Result<Vec<Diagnostic>, XmlError>;

fn qos(xml: &str, o: &ReadOptions) -> Result<Vec<Diagnostic>, XmlError> {
    parse_qos_libraries_with(xml, o).map(|p| p.diagnostics)
}
fn domains(xml: &str, o: &ReadOptions) -> Result<Vec<Diagnostic>, XmlError> {
    parse_domain_libraries_with(xml, o).map(|p| p.diagnostics)
}
fn participants(xml: &str, o: &ReadOptions) -> Result<Vec<Diagnostic>, XmlError> {
    parse_domain_participant_libraries_with(xml, o).map(|p| p.diagnostics)
}
fn types(xml: &str, o: &ReadOptions) -> Result<Vec<Diagnostic>, XmlError> {
    parse_type_libraries_with(xml, o).map(|p| p.diagnostics)
}

/// (case, loader, document, path of the element or attribute that is the
/// vendor form)
const VENDOR_FORMS: &[(&str, Loader, &str, &str)] = &[
    (
        "participant_qos",
        qos,
        r#"<qos_library name="L"><qos_profile name="P"><participant_qos/></qos_profile></qos_library>"#,
        "qos_library[L]/qos_profile[P]/participant_qos",
    ),
    (
        "domainparticipant_qos",
        qos,
        r#"<qos_library name="L"><qos_profile name="P"><domainparticipant_qos/></qos_profile></qos_library>"#,
        "qos_library[L]/qos_profile[P]/domainparticipant_qos",
    ),
    (
        "topic_filter_element_of_a_profile",
        qos,
        r#"<qos_library name="L"><qos_profile name="P"><topic_filter>A*</topic_filter></qos_profile></qos_library>"#,
        "qos_library[L]/qos_profile[P]/topic_filter",
    ),
    (
        "duration_as_text",
        qos,
        r#"<qos_library name="L"><qos_profile name="P"><datawriter_qos><deadline><period>DURATION_INFINITY</period></deadline></datawriter_qos></qos_profile></qos_library>"#,
        "qos_library[L]/qos_profile[P]/datawriter_qos/deadline/period",
    ),
    (
        "topic_qos_profile_ref",
        domains,
        r#"<domain_library name="DL"><domain name="D" domain_id="0"><register_type name="T" type_ref="T"/><topic name="X" register_type_ref="T" qos_profile_ref="L::P"/></domain></domain_library>"#,
        "domain_library[DL]/domain[D]/topic[X]@qos_profile_ref",
    ),
    (
        "topic_filter_attribute_of_a_topic",
        domains,
        r#"<domain_library name="DL"><domain name="D" domain_id="0"><register_type name="T" type_ref="T"/><topic name="X" register_type_ref="T" topic_filter="A*"/></domain></domain_library>"#,
        "domain_library[DL]/domain[D]/topic[X]@topic_filter",
    ),
    (
        "topic_filter_element_of_a_topic",
        domains,
        r#"<domain_library name="DL"><domain name="D" domain_id="0"><register_type name="T" type_ref="T"/><topic name="X" register_type_ref="T"><topic_filter>A*</topic_filter></topic></domain></domain_library>"#,
        "domain_library[DL]/domain[D]/topic[X]/topic_filter",
    ),
    (
        "data_writer_qos_profile_ref",
        participants,
        r#"<domain_participant_library name="PL"><domain_participant name="P" domain_ref="DL::D"><publisher name="Pub"><data_writer name="W" topic_ref="T" qos_profile_ref="L::P"/></publisher></domain_participant></domain_participant_library>"#,
        "domain_participant_library[PL]/domain_participant[P]/publisher[Pub]/data_writer[W]@qos_profile_ref",
    ),
    (
        "data_reader_qos_profile_ref",
        participants,
        r#"<domain_participant_library name="PL"><domain_participant name="P" domain_ref="DL::D"><subscriber name="Sub"><data_reader name="R" topic_ref="T" qos_profile_ref="L::P"/></subscriber></domain_participant></domain_participant_library>"#,
        "domain_participant_library[PL]/domain_participant[P]/subscriber[Sub]/data_reader[R]@qos_profile_ref",
    ),
    (
        "idl_name_of_a_primitive",
        types,
        r#"<types><struct name="S"><member name="m" type="long"/></struct></types>"#,
        "types/struct[S]/member[m]@type",
    ),
    (
        "type_name_without_nonbasic",
        types,
        r#"<types><struct name="S"><member name="m" type="Other"/></struct></types>"#,
        "types/struct[S]/member[m]@type",
    ),
    (
        "lower_case_nonbasic",
        types,
        r#"<types><struct name="S"><member name="m" type="nonbasic" nonBasicTypeName="X"/></struct></types>"#,
        "types/struct[S]/member[m]@type",
    ),
    (
        "discriminator_attribute",
        types,
        r#"<types><union name="U" discriminator="int32"><case><caseDiscriminator value="0"/><member name="a" type="int32"/></case></union></types>"#,
        "types/union[U]@discriminator",
    ),
    (
        "bit_value",
        types,
        r#"<types><bitmask name="B"><bit_value name="A" position="0"/></bitmask></types>"#,
        "types/bitmask[B]/bit_value[A]",
    ),
    (
        "bitset",
        types,
        r#"<types><bitset name="S"/></types>"#,
        "types/bitset[S]",
    ),
    (
        "base_type",
        types,
        r#"<types><struct name="B"/><struct name="D" base_type="B"/></types>"#,
        "types/struct[D]@base_type",
    ),
    (
        "must_understand",
        types,
        r#"<types><struct name="S"><member name="m" type="int32" must_understand="true"/></struct></types>"#,
        "types/struct[S]/member[m]@must_understand",
    ),
    (
        "forwardDcl",
        types,
        r#"<types><forwardDcl name="F" kind="struct"/></types>"#,
        "types/forwardDcl[F]",
    ),
    (
        "upper_case_forward_kind",
        types,
        r#"<types><forward_dcl name="F" kind="STRUCT"/></types>"#,
        "types/forward_dcl[F]@kind",
    ),
    (
        "forward_dcl_without_kind",
        types,
        r#"<types><forward_dcl name="F"/></types>"#,
        "types/forward_dcl[F]",
    ),
    (
        "appendable",
        types,
        r#"<types><struct name="S" extensibility="appendable"/></types>"#,
        "types/struct[S]@extensibility",
    ),
];

/// The default reading refuses every vendor form with an error that
/// names the path, the form and the specification's form; reading it
/// takes `ReadOptions::accept_vendor_forms()`, and each use comes back as a
/// diagnostic of kind `VendorForm`. Skipping unknown elements does not
/// accept vendor forms.
#[test]
fn vendor_forms_are_refused_unless_accepted() {
    for (case, load, xml, path) in VENDOR_FORMS {
        let err = load(xml, &ReadOptions::default()).expect_err(case);
        assert!(
            matches!(&err, XmlError::VendorForm(m) if m.starts_with(path) && m.contains("writes")),
            "{case}: {err:?}"
        );
        let err = load(xml, &ReadOptions::report_and_skip()).expect_err(case);
        assert!(
            matches!(&err, XmlError::VendorForm(_)),
            "{case} under ReportAndSkip: {err:?}"
        );
        let diagnostics = load(xml, &ReadOptions::accept_vendor_forms()).expect(case);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.path == *path && d.kind == DiagnosticKind::VendorForm),
            "{case}: {diagnostics:?}"
        );
    }
}

/// Both settings together: unknown elements are skipped and vendor forms
/// are read, each with its own diagnostic.
#[test]
fn the_two_settings_are_independent() {
    let xml = r#"<qos_library name="L"><qos_profile name="P">
        <participant_qos/><datawriter_qos><publish_mode/></datawriter_qos>
    </qos_profile></qos_library>"#;
    let both = ReadOptions::report_and_skip().with_vendor_forms();
    let parsed = parse_qos_libraries_with(xml, &both).expect("parse");
    let kinds: Vec<DiagnosticKind> = parsed.diagnostics.iter().map(|d| d.kind).collect();
    assert_eq!(
        kinds,
        [DiagnosticKind::VendorForm, DiagnosticKind::UnknownElement]
    );
    assert!(parse_qos_libraries_with(xml, &ReadOptions::accept_vendor_forms()).is_err());
}

/// The specification's forms need no setting.
#[test]
fn the_specifications_forms_report_nothing() {
    let xml = r#"<types>
        <struct name="B" extensibility="append"/>
        <struct name="D" baseType="B">
            <member name="a" type="int32" mustUnderstand="true"/>
            <member name="b" type="nonBasic" nonBasicTypeName="B"/>
        </struct>
        <forward_dcl name="F" kind="struct"/>
        <bitmask name="M"><flag name="X" position="0"/></bitmask>
    </types>"#;
    let parsed = parse_type_libraries_with(xml, &ReadOptions::default()).expect("parse");
    assert!(parsed.diagnostics.is_empty());
}
