//! Integrationstests fuer C6.2.C — XRCE-XML-Configuration.
//!
//! Deckt das End-to-End-Szenario "XML-File → CREATE-Submessage-Stream"
//! ab und validiert die Bridge zur DDS-XML-Library.

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

use zerodds_xrce::submessages::CreatePayload;
use zerodds_xrce::{
    InMemoryQosResolver, ObjectKind, ObjectVariant, QosProfileResolver, Submessage, SubmessageId,
    XrceXmlError, load_xrce_config,
};

const SHAPES_XML: &str = r#"<dds>
  <type object_id="0x000A">
    <module name="ShapesDemoTypes">
      <struct name="ShapeType" extensibility="EXTENSIBLE_EXTENSIBILITY">
        <member name="color" type="string" key="true" stringMaxLength="128"/>
        <member name="x" type="long"/>
        <member name="y" type="long"/>
        <member name="shapesize" type="long"/>
      </struct>
    </module>
  </type>
  <participant object_id="0xCAF1" domain_id="0">
    <topic object_id="0x0102" name="Square" type_name="ShapeType" qos_profile="ShapesLib::ShapesProfile"/>
    <topic object_id="0x0112" name="Circle" type_name="ShapeType" qos_profile="ShapesLib::ShapesProfile"/>
    <publisher object_id="0x0103">
      <data_writer object_id="0x0105" topic_ref="0x0102" qos_profile="ShapesLib::ShapesProfile"/>
      <data_writer object_id="0x0115" topic_ref="0x0112" qos_profile="ShapesLib::ShapesProfile"/>
    </publisher>
    <subscriber object_id="0x0104">
      <data_reader object_id="0x0106" topic_ref="0x0102"/>
    </subscriber>
  </participant>
</dds>"#;

#[test]
fn end_to_end_load_and_emit_creates() {
    let cfg = load_xrce_config(SHAPES_XML).expect("parse");
    assert_eq!(cfg.types.len(), 1);
    assert_eq!(cfg.participants.len(), 1);
    let msgs = cfg.to_create_messages().expect("create-msgs");
    // 1 Type + 1 Participant + 2 Topics + 1 Publisher + 1 Subscriber +
    // 2 DataWriters + 1 DataReader = 9
    assert_eq!(msgs.len(), 9);
    let kinds: Vec<ObjectKind> = msgs.iter().map(|m| m.kind).collect();
    assert_eq!(kinds[0], ObjectKind::Type);
    assert_eq!(kinds[1], ObjectKind::Participant);
}

#[test]
fn create_messages_can_be_packed_into_submessages() {
    let cfg = load_xrce_config(SHAPES_XML).unwrap();
    let msgs = cfg.to_create_messages().unwrap();
    // Jedes CreatePayload muss in eine valide Submessage gepackt werden.
    let submessages: Vec<Submessage> = msgs
        .into_iter()
        .map(|m| m.payload.into_submessage().unwrap())
        .collect();
    for sm in &submessages {
        assert_eq!(sm.header.submessage_id, SubmessageId::Create);
    }
}

#[test]
fn object_variant_decode_roundtrips_xml_string() {
    let cfg = load_xrce_config(SHAPES_XML).unwrap();
    let msgs = cfg.to_create_messages().unwrap();
    // Erste Submessage (Type) muss als ObjectVariant::ByXmlString
    // dekodierbar sein.
    let payload: &CreatePayload = &msgs[0].payload;
    let (variant, _) = ObjectVariant::decode(
        &payload.representation,
        zerodds_xrce::encoding::Endianness::Little,
    )
    .expect("decode");
    match variant {
        ObjectVariant::ByXmlString(xml) => {
            assert!(xml.contains("ShapeType"));
            assert!(xml.contains("ShapesDemoTypes"));
        }
        _ => panic!("expected ByXmlString"),
    }
}

#[test]
fn qos_resolver_bridge_collects_unique_refs() {
    let cfg = load_xrce_config(SHAPES_XML).unwrap();
    let refs = cfg.qos_profile_refs();
    assert_eq!(refs, vec!["ShapesLib::ShapesProfile".to_string()]);

    let mut resolver = InMemoryQosResolver::new();
    resolver.add(
        "ShapesLib::ShapesProfile",
        "<qos_profile name=\"ShapesProfile\"/>",
    );
    let xml = resolver.resolve("ShapesLib::ShapesProfile").unwrap();
    assert!(xml.contains("ShapesProfile"));
}

#[test]
fn missing_qos_profile_in_resolver_yields_error() {
    let cfg = load_xrce_config(SHAPES_XML).unwrap();
    let resolver = InMemoryQosResolver::new();
    for r in cfg.qos_profile_refs() {
        let res = cfg.resolve_qos_profile(&r, &resolver);
        assert!(matches!(res, Err(XrceXmlError::UnresolvedQosProfile(_))));
    }
}

/// Verifies that a config with NO Type (nur Topic mit pre-shared Type)
/// rejected wird — Topic.type_name muss gegen <type>-Section matchen.
#[test]
fn topic_without_type_section_rejected() {
    let xml = r#"<dds>
        <participant object_id="0xCAF1" domain_id="0">
          <topic object_id="0x0102" name="A" type_name="ShapeType"/>
        </participant>
      </dds>"#;
    let res = load_xrce_config(xml);
    assert!(matches!(res, Err(XrceXmlError::UnresolvedTypeName { .. })));
}
