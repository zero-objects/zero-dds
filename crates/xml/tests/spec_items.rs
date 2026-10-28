//! Tests for the DDS-XML 1.0 items of `docs/spec-coverage/dds-xml-1.0.md`
//! that no other test file reads: each literal of the DCPS enumerations,
//! numbers as enumeration values, fields in any order, attribute values,
//! the examples of the specification and the value forms of Data Samples.

#![allow(
    clippy::approx_constant,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use zerodds_qos::{
    DestinationOrderKind, DurabilityKind, HistoryKind, LivelinessKind, OwnershipKind,
    PresentationAccessScope, ReliabilityKind,
};
use zerodds_xml::parser::parse_xml_tree;
use zerodds_xml::{
    PrimitiveValue, SampleValue, TypeLibrary, XmlError, parse_domain_libraries,
    parse_qos_libraries, parse_sample, parse_type_libraries,
};

/// A `<qos_library>` with one profile whose `datawriter_qos` holds `body`.
fn writer(body: &str) -> String {
    format!(
        r#"<qos_library name="L"><qos_profile name="P"><datawriter_qos>{body}</datawriter_qos></qos_profile></qos_library>"#
    )
}

fn writer_qos(body: &str) -> zerodds_xml::EntityQos {
    let libs = parse_qos_libraries(&writer(body)).expect("parse");
    libs[0].profiles[0].datawriter_qos[0].qos.clone()
}

// ---------------------------------------------------------------------
// §7.2.1: every literal of the seven enumerations
// ---------------------------------------------------------------------

#[test]
fn every_literal_of_the_dcps_enumerations_is_read() {
    for (lit, want) in [
        ("VOLATILE_DURABILITY_QOS", DurabilityKind::Volatile),
        (
            "TRANSIENT_LOCAL_DURABILITY_QOS",
            DurabilityKind::TransientLocal,
        ),
        ("TRANSIENT_DURABILITY_QOS", DurabilityKind::Transient),
        ("PERSISTENT_DURABILITY_QOS", DurabilityKind::Persistent),
    ] {
        let q = writer_qos(&format!("<durability><kind>{lit}</kind></durability>"));
        assert_eq!(q.durability.expect("durability").kind, want, "{lit}");
    }
    for (lit, want) in [
        (
            "INSTANCE_PRESENTATION_QOS",
            PresentationAccessScope::Instance,
        ),
        ("TOPIC_PRESENTATION_QOS", PresentationAccessScope::Topic),
        ("GROUP_PRESENTATION_QOS", PresentationAccessScope::Group),
    ] {
        let xml = format!(
            r#"<qos_library name="L"><qos_profile name="P"><publisher_qos><presentation><access_scope>{lit}</access_scope></presentation></publisher_qos></qos_profile></qos_library>"#
        );
        let libs = parse_qos_libraries(&xml).expect("parse");
        let q = &libs[0].profiles[0].publisher_qos[0].qos;
        assert_eq!(q.presentation.expect("presentation").access_scope, want);
    }
    for (lit, want) in [
        ("SHARED_OWNERSHIP_QOS", OwnershipKind::Shared),
        ("EXCLUSIVE_OWNERSHIP_QOS", OwnershipKind::Exclusive),
    ] {
        let q = writer_qos(&format!("<ownership><kind>{lit}</kind></ownership>"));
        assert_eq!(q.ownership.expect("ownership").kind, want, "{lit}");
    }
    for (lit, want) in [
        ("AUTOMATIC_LIVELINESS_QOS", LivelinessKind::Automatic),
        (
            "MANUAL_BY_PARTICIPANT_LIVELINESS_QOS",
            LivelinessKind::ManualByParticipant,
        ),
        (
            "MANUAL_BY_TOPIC_LIVELINESS_QOS",
            LivelinessKind::ManualByTopic,
        ),
    ] {
        let q = writer_qos(&format!("<liveliness><kind>{lit}</kind></liveliness>"));
        assert_eq!(q.liveliness.expect("liveliness").kind, want, "{lit}");
    }
    for (lit, want) in [
        ("BEST_EFFORT_RELIABILITY_QOS", ReliabilityKind::BestEffort),
        ("RELIABLE_RELIABILITY_QOS", ReliabilityKind::Reliable),
    ] {
        let q = writer_qos(&format!("<reliability><kind>{lit}</kind></reliability>"));
        assert_eq!(q.reliability.expect("reliability").kind, want, "{lit}");
    }
    for (lit, want) in [
        (
            "BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS",
            DestinationOrderKind::ByReceptionTimestamp,
        ),
        (
            "BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS",
            DestinationOrderKind::BySourceTimestamp,
        ),
    ] {
        let q = writer_qos(&format!(
            "<destination_order><kind>{lit}</kind></destination_order>"
        ));
        assert_eq!(q.destination_order.expect("order").kind, want, "{lit}");
    }
    for (lit, want) in [
        ("KEEP_LAST_HISTORY_QOS", HistoryKind::KeepLast),
        ("KEEP_ALL_HISTORY_QOS", HistoryKind::KeepAll),
    ] {
        let q = writer_qos(&format!("<history><kind>{lit}</kind></history>"));
        assert_eq!(q.history.expect("history").kind, want, "{lit}");
        let q = writer_qos(&format!(
            "<durability_service><history_kind>{lit}</history_kind></durability_service>"
        ));
        assert_eq!(
            q.durability_service.expect("service").history_kind,
            want,
            "{lit} in durability_service"
        );
    }
}

/// Table 7.1: an enumeration value is a string, "do not use numeric
/// values".
#[test]
fn a_number_is_no_enumeration_value() {
    for body in [
        "<durability><kind>0</kind></durability>",
        "<reliability><kind>2</kind></reliability>",
        "<history><kind>1</kind></history>",
        "<ownership><kind>0x1</kind></ownership>",
        "<liveliness><kind>2</kind></liveliness>",
        "<destination_order><kind>1</kind></destination_order>",
    ] {
        let err = parse_qos_libraries(&writer(body)).unwrap_err();
        assert!(matches!(err, XmlError::BadEnum(_)), "{body}: {err:?}");
    }
}

// ---------------------------------------------------------------------
// §7.2.3: DurabilityService, fields in any order
// ---------------------------------------------------------------------

#[test]
fn durability_service_reads_all_its_fields() {
    let q = writer_qos(
        "<durability_service><service_cleanup_delay><sec>2</sec></service_cleanup_delay>\
         <history_kind>KEEP_ALL_HISTORY_QOS</history_kind><history_depth>5</history_depth>\
         <max_samples>10</max_samples><max_instances>11</max_instances>\
         <max_samples_per_instance>12</max_samples_per_instance></durability_service>",
    );
    let s = q.durability_service.expect("service");
    assert_eq!(s.history_depth, 5);
    assert_eq!(s.max_samples, 10);
    assert_eq!(s.max_instances, 11);
    assert_eq!(s.max_samples_per_instance, 12);
}

/// §7.2.3 maps a structure with `xs:all`: its fields come in any order.
#[test]
fn fields_of_a_structure_come_in_any_order() {
    let in_idl_order = writer_qos(
        "<reliability><kind>RELIABLE_RELIABILITY_QOS</kind>\
         <max_blocking_time><sec>1</sec><nanosec>500000000</nanosec></max_blocking_time>\
         </reliability><history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history>",
    );
    let reversed = writer_qos(
        "<history><depth>7</depth><kind>KEEP_LAST_HISTORY_QOS</kind></history>\
         <reliability><max_blocking_time><nanosec>500000000</nanosec><sec>1</sec>\
         </max_blocking_time><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>",
    );
    assert_eq!(in_idl_order, reversed);
}

/// §7.2.3: one element per field; a repeated field carries two values.
#[test]
fn a_field_given_twice_is_refused() {
    let err = parse_qos_libraries(&writer(
        "<history><depth>1</depth><depth>2</depth></history>",
    ))
    .unwrap_err();
    assert!(matches!(err, XmlError::InvalidXml(_)), "{err:?}");
}

// ---------------------------------------------------------------------
// §7.1.5 Table 7.2 string: attribute values after the XML rules
// ---------------------------------------------------------------------

#[test]
fn attribute_values_keep_their_characters_and_spaces() {
    let doc =
        parse_xml_tree(r#"<a v="  x &amp; y &lt;z&gt; &quot;q&quot; &#x41;  " w='it&apos;s'/>"#)
            .expect("parse");
    assert_eq!(doc.root.attribute("v"), Some("  x & y <z> \"q\" A  "));
    assert_eq!(doc.root.attribute("w"), Some("it's"));
}

// ---------------------------------------------------------------------
// The examples of the specification
// ---------------------------------------------------------------------

/// §7.3.2.4.1.1.
#[test]
fn qos_library_example() {
    let xml = r#"<qos_library name="ReliableProfilesLibrary">
    <qos_profile name="StrictReliableCommunicationProfile">
        <datawriter_qos>
            <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        </datawriter_qos>
        <datareader_qos>
            <history><kind>KEEP_ALL_HISTORY_QOS</kind></history>
            <reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>
        </datareader_qos>
    </qos_profile>
</qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let p = &libs[0].profiles[0];
    for q in [&p.datawriter_qos[0].qos, &p.datareader_qos[0].qos] {
        assert_eq!(q.history.expect("history").kind, HistoryKind::KeepAll);
        assert_eq!(
            q.reliability.expect("reliability").kind,
            ReliabilityKind::Reliable
        );
    }
}

/// §7.3.4.4.1.1, with the `base_name` of `topic_qos`.
#[test]
fn domain_example() {
    let xml = r#"<domain_library name="DL"><domain name="MyDomain" domain_id="10">
    <register_type name="MyFirstRegisterType" type_ref= "MyType" />
    <register_type name="MySecondRegisterType" type_ref="MyType" />
    <topic name="FirstTopic" register_type_ref="MyFirstRegisterType">
        <topic_qos base_name="BaseQoSProfile" />
    </topic>
    <topic name="SecondTopic" register_type_ref="MySecondRegisterType" />
</domain></domain_library>"#;
    let libs = parse_domain_libraries(xml).expect("parse");
    let d = &libs[0].domains[0];
    assert_eq!(d.domain_id, Some(10));
    assert_eq!(d.register_types.len(), 2);
    assert_eq!(d.topics.len(), 2);
    let q = d.topics[0].topic_qos.as_ref().expect("topic_qos");
    assert_eq!(q.base_name.as_deref(), Some("BaseQoSProfile"));
}

// ---------------------------------------------------------------------
// §7.3.7: Data Samples
// ---------------------------------------------------------------------

fn types(xml: &str) -> Vec<TypeLibrary> {
    parse_type_libraries(xml).expect("types")
}

fn sample(
    libs: &[TypeLibrary],
    name: &str,
    xml: &str,
) -> Result<std::collections::BTreeMap<String, SampleValue>, XmlError> {
    let def = libs[0]
        .types
        .iter()
        .find(|t| t.name() == name)
        .expect("type");
    match parse_sample(xml, def, libs)? {
        SampleValue::Struct(m) => Ok(m),
        other => panic!("not a struct: {other:?}"),
    }
}

fn prim(m: &std::collections::BTreeMap<String, SampleValue>, k: &str) -> PrimitiveValue {
    match &m[k] {
        SampleValue::Primitive(p) => p.clone(),
        other => panic!("{k}: {other:?}"),
    }
}

/// §7.3.7.4.5, the only description of the form of eight primitive types.
#[test]
fn primitive_types_example() {
    let libs = types(
        r#"<types>
        <enum name="Suit"><enumerator name="ACE"/><enumerator name="KING"/></enum>
        <struct name="aStruct">
            <member name="aShort" type="int16"/>
            <member name="aUShort" type="uint16"/>
            <member name="anEnum" type="nonBasic" nonBasicTypeName="Suit"/>
            <member name="aLong" type="int32"/>
            <member name="aULong" type="uint32"/>
            <member name="aLongLong" type="int64"/>
            <member name="aULongLong" type="uint64"/>
            <member name="aFloat" type="float32"/>
            <member name="aDouble" type="float64"/>
            <member name="aBoolean" type="boolean"/>
            <member name="aString" type="string"/>
            <member name="anotherString" type="string"/>
            <member name="anOctet" type="byte"/>
            <member name="aChar" type="char8"/>
        </struct></types>"#,
    );
    let m = sample(
        &libs,
        "aStruct",
        r#"<aStruct>
    <aShort>3</aShort>
    <aUShort>2</aUShort>
    <anEnum>ACE</anEnum>
    <aLong>2452</aLong>
    <aULong>3245</aULong>
    <aLongLong>23121</aLongLong>
    <aULongLong>2345212</aULongLong>
    <aFloat>2.3</aFloat>
    <aDouble>3.14</aDouble>
    <aBoolean>false</aBoolean>
    <aString>A string!</aString>
    <anotherString>&#xa1;El r&#xed;o mi&#xf1;o es precioso!</anotherString>
    <anOctet>0x00</anOctet>
    <aChar>a</aChar>
</aStruct>"#,
    )
    .expect("parse");
    assert_eq!(prim(&m, "aShort"), PrimitiveValue::I16(3));
    assert_eq!(prim(&m, "aUShort"), PrimitiveValue::U16(2));
    assert_eq!(m["anEnum"], SampleValue::EnumLiteral("ACE".into()));
    assert_eq!(prim(&m, "aLong"), PrimitiveValue::I32(2452));
    assert_eq!(prim(&m, "aULong"), PrimitiveValue::U32(3245));
    assert_eq!(prim(&m, "aLongLong"), PrimitiveValue::I64(23121));
    assert_eq!(prim(&m, "aULongLong"), PrimitiveValue::U64(2345212));
    assert_eq!(prim(&m, "aFloat"), PrimitiveValue::F32(2.3));
    assert_eq!(prim(&m, "aDouble"), PrimitiveValue::F64(3.14));
    assert_eq!(prim(&m, "aBoolean"), PrimitiveValue::Bool(false));
    assert_eq!(
        prim(&m, "anotherString"),
        PrimitiveValue::Str("¡El río miño es precioso!".into())
    );
    assert_eq!(prim(&m, "anOctet"), PrimitiveValue::U8(0));
    assert_eq!(prim(&m, "aChar"), PrimitiveValue::Char('a'));
}

/// §7.3.7.4.3 and §7.3.7.4.4: sequences and arrays with `item`.
#[test]
fn sequence_and_array_examples() {
    let libs = types(
        r#"<types>
        <struct name="Point"><member name="aFloat" type="float32"/></struct>
        <struct name="aStruct">
            <member name="aSequenceOfDoubles" type="float64" sequenceMaxLength="-1"/>
            <member name="aSequenceOfStructs" type="nonBasic" nonBasicTypeName="Point"
                    sequenceMaxLength="-1"/>
            <member name="anArrayOfLongs" type="int32" arrayDimensions="2"/>
            <member name="anArrayOfStructs" type="nonBasic" nonBasicTypeName="Point"
                    arrayDimensions="2"/>
        </struct></types>"#,
    );
    let m = sample(
        &libs,
        "aStruct",
        r#"<aStruct>
    <aSequenceOfDoubles><item>1.1</item><item>1.2</item></aSequenceOfDoubles>
    <aSequenceOfStructs>
        <item><aFloat>0.0</aFloat></item>
        <item><aFloat>1.0</aFloat></item>
    </aSequenceOfStructs>
    <anArrayOfLongs><item>1</item><item>2</item></anArrayOfLongs>
    <anArrayOfStructs>
        <item><aFloat>0.0</aFloat></item>
        <item><aFloat>1.0</aFloat></item>
    </anArrayOfStructs>
</aStruct>"#,
    )
    .expect("parse");
    for (k, n) in [
        ("aSequenceOfDoubles", 2),
        ("aSequenceOfStructs", 2),
        ("anArrayOfLongs", 2),
        ("anArrayOfStructs", 2),
    ] {
        let len = match &m[k] {
            SampleValue::Sequence(v) | SampleValue::Array(v) => v.len(),
            other => panic!("{k}: {other:?}"),
        };
        assert_eq!(len, n, "{k}");
    }
}

/// §7.3.7.3.1 Table 7.1 forms in a sample: `boolean`, `long` in hex and
/// as `LENGTH_UNLIMITED`, `unsigned long`, an enumeration literal that the
/// type does not declare, and the constants of §7.2.2 in a `Duration_t`.
#[test]
fn sample_values_follow_table_7_1() {
    let libs = types(
        r#"<types>
        <enum name="E"><enumerator name="A"/></enum>
        <struct name="Duration_t">
            <member name="sec" type="int32"/><member name="nanosec" type="uint32"/>
        </struct>
        <struct name="S">
            <member name="b" type="boolean"/>
            <member name="l" type="int32"/>
            <member name="u" type="uint32"/>
            <member name="e" type="nonBasic" nonBasicTypeName="E"/>
            <member name="d" type="nonBasic" nonBasicTypeName="Duration_t"/>
        </struct></types>"#,
    );
    let ok = r#"<S><b>true</b><l>0x10</l><u>0xFFFFFFFF</u><e>A</e>
        <d><sec>DURATION_INFINITE_SEC</sec><nanosec>DURATION_INFINITE_NSEC</nanosec></d></S>"#;
    let m = sample(&libs, "S", ok).expect("parse");
    assert_eq!(prim(&m, "b"), PrimitiveValue::Bool(true));
    assert_eq!(prim(&m, "l"), PrimitiveValue::I32(16));
    assert_eq!(prim(&m, "u"), PrimitiveValue::U32(u32::MAX));
    let SampleValue::Struct(d) = &m["d"] else {
        panic!("d");
    };
    assert_eq!(prim(d, "sec"), PrimitiveValue::I32(i32::MAX));
    assert_eq!(prim(d, "nanosec"), PrimitiveValue::U32(0x7fff_ffff));

    let unlimited = ok.replace("<l>0x10</l>", "<l>LENGTH_UNLIMITED</l>");
    assert_eq!(
        prim(&sample(&libs, "S", &unlimited).expect("parse"), "l"),
        PrimitiveValue::I32(-1)
    );
    let time_invalid = ok
        .replace("DURATION_INFINITE_SEC", "TIME_INVALID_SEC")
        .replace("DURATION_INFINITE_NSEC", "TIME_INVALID_NSEC");
    let m = sample(&libs, "S", &time_invalid).expect("parse");
    let SampleValue::Struct(d) = &m["d"] else {
        panic!("d");
    };
    assert_eq!(prim(d, "sec"), PrimitiveValue::I32(-1));
    assert_eq!(prim(d, "nanosec"), PrimitiveValue::U32(0xFFFF_FFFF));

    for bad in [
        ok.replace("<b>true</b>", "<b>True</b>"),
        ok.replace("<e>A</e>", "<e>B</e>"),
        ok.replace("<u>0xFFFFFFFF</u>", "<u>4294967296</u>"),
    ] {
        assert!(sample(&libs, "S", &bad).is_err(), "{bad}");
    }
}

/// §7.3.7.3.1: the fields of a structure come in any order.
#[test]
fn sample_fields_come_in_any_order() {
    let libs = types(
        r#"<types><struct name="S"><member name="a" type="int32"/>
        <member name="b" type="string"/></struct></types>"#,
    );
    let m = sample(&libs, "S", "<S><b>x</b><a>1</a></S>").expect("parse");
    assert_eq!(prim(&m, "a"), PrimitiveValue::I32(1));
    assert_eq!(prim(&m, "b"), PrimitiveValue::Str("x".into()));
}

/// §7.3.6.4.2: a participant of an application inherits only from
/// participant libraries defined before the application.
#[test]
fn application_inherits_only_from_libraries_defined_before_it() {
    let domain = r#"<domain_library name="dl"><domain name="D" domain_id="0"/></domain_library>"#;
    let participants = r#"<domain_participant_library name="pl">
        <domain_participant name="P" domain_ref="dl::D"/></domain_participant_library>"#;
    let application = r#"<application_library name="al"><application name="A">
        <domain_participant name="Mine" base_name="pl::P"/></application>
        </application_library>"#;
    let before = format!("<dds>{domain}{participants}{application}</dds>");
    let doc = zerodds_xml::parse_dds_xml(&before).expect("parse");
    assert!(doc.resolve_application("al::A").is_ok());

    let after = format!("<dds>{domain}{application}{participants}</dds>");
    let doc = zerodds_xml::parse_dds_xml(&after).expect("parse");
    let err = doc.resolve_application("al::A").unwrap_err();
    assert!(matches!(err, XmlError::InvalidXml(_)), "{err:?}");
}

/// Table 7.2 enum: a number is no value of `extensibility`.
#[test]
fn a_number_is_no_extensibility() {
    let err =
        parse_type_libraries(r#"<types><struct name="S" extensibility="1"/></types>"#).unwrap_err();
    assert!(matches!(err, XmlError::BadEnum(_)), "{err:?}");
}

/// §7.3.4.4.1: a `register_type` refers to a type previously defined.
#[test]
fn register_type_refers_to_a_type_defined_before_the_domain() {
    let types = r#"<types><struct name="T"/></types>"#;
    let domain = r#"<domain_library name="dl"><domain name="D" domain_id="0">
        <register_type name="R" type_ref="T"/><topic name="X" register_type_ref="R"/>
        </domain></domain_library>"#;
    let participants = r#"<domain_participant_library name="pl">
        <domain_participant name="P" domain_ref="dl::D"/></domain_participant_library>"#;
    let before = zerodds_xml::parse_dds_xml(&format!("<dds>{types}{domain}{participants}</dds>"))
        .expect("parse");
    assert!(before.resolve_participant("pl::P").is_ok());

    let after = zerodds_xml::parse_dds_xml(&format!("<dds>{domain}{types}{participants}</dds>"))
        .expect("parse");
    let err = after.resolve_participant("pl::P").unwrap_err();
    assert!(matches!(err, XmlError::UnresolvedReference(_)), "{err:?}");
}

/// `topic_data` and `group_data` hold their octets as `<value>` or
/// `<valueB64>`, as `user_data` does (§7.2.4.2).
#[test]
fn topic_data_and_group_data_hold_octets_in_both_forms() {
    let xml = r#"<qos_library name="L"><qos_profile name="P">
        <topic_qos><topic_data><valueB64>AQID</valueB64></topic_data></topic_qos>
        <publisher_qos><group_data><value>1,2,3</value></group_data></publisher_qos>
        </qos_profile></qos_library>"#;
    let libs = parse_qos_libraries(xml).expect("parse");
    let p = &libs[0].profiles[0];
    assert_eq!(
        p.topic_qos[0]
            .qos
            .topic_data
            .as_ref()
            .expect("topic_data")
            .value,
        vec![1, 2, 3]
    );
    assert_eq!(
        p.publisher_qos[0]
            .qos
            .group_data
            .as_ref()
            .expect("group_data")
            .value,
        vec![1, 2, 3]
    );
}

/// §7.2.2.1 and the XSD's types: `max_samples` and its siblings are
/// `positiveInteger_UNLIMITED`, a history `depth` is `xs:positiveInteger`,
/// `ownership_strength` and `transport_priority` take `xs:nonNegativeInteger`.
#[test]
fn numbers_outside_the_schemas_types_are_refused() {
    for body in [
        "<resource_limits><max_samples>0</max_samples></resource_limits>",
        "<resource_limits><max_instances>-1</max_instances></resource_limits>",
        "<resource_limits><max_samples_per_instance>0x10</max_samples_per_instance></resource_limits>",
        "<resource_limits><initial_samples>0</initial_samples></resource_limits>",
        "<durability_service><max_samples>0</max_samples></durability_service>",
        "<durability_service><history_depth>0</history_depth></durability_service>",
        "<history><depth>0</depth></history>",
        "<ownership_strength><value>-1</value></ownership_strength>",
        "<transport_priority><value>-1</value></transport_priority>",
    ] {
        let err = parse_qos_libraries(&writer(body)).unwrap_err();
        assert!(
            matches!(err, XmlError::ValueOutOfRange(_)),
            "{body}: {err:?}"
        );
    }
    let q = writer_qos(
        "<resource_limits><max_samples>5</max_samples>\
         <max_instances>LENGTH_UNLIMITED</max_instances></resource_limits>\
         <history><depth>3</depth></history>",
    );
    let rl = q.resource_limits.expect("resource_limits");
    assert_eq!((rl.max_samples, rl.max_instances), (5, -1));
    assert_eq!(q.history.expect("history").depth, 3);
}
