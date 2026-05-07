//! Integration-Tests fuer die XML→TypeObject-Bridge (C4.5-b).
//!
//! Lader-Pipeline: `<types>`-XML → [`xsd_loader`]-Pass →
//! [`TypeLibrary`] → [`bridge_library`] → `MinimalTypeObject`-Map.

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

use zerodds_types::compute_minimal_hash;
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::{MinimalTypeObject, TypeIdentifier};
use zerodds_xml::xsd_loader::{ValidationMode, load_type_libraries_from_string};
use zerodds_xml::{bridge_library, xml_type_to_typeobject};

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<types xmlns="http://www.omg.org/spec/DDS-XML">
  <struct name="Position" extensibility="final">
    <member name="x" type="float" key="true"/>
    <member name="y" type="float"/>
  </struct>

  <enum name="Color">
    <enumerator name="RED" value="0"/>
    <enumerator name="GREEN" value="1"/>
    <enumerator name="BLUE" value="2"/>
  </enum>

  <struct name="Sample" extensibility="appendable">
    <member name="id" type="long" id="42" key="true"/>
    <member name="data" type="octet" sequenceMaxLength="1024"/>
    <member name="label" type="string" stringMaxLength="64"/>
  </struct>

  <typedef name="Vec3" type="float" arrayDimensions="3"/>

  <bitmask name="Permissions" bitBound="8">
    <bit_value name="READ" position="0"/>
    <bit_value name="WRITE" position="1"/>
    <bit_value name="EXEC" position="2"/>
  </bitmask>
</types>
"#;

#[test]
fn end_to_end_xml_string_to_typeobject_map() {
    let libs = load_type_libraries_from_string(SAMPLE, ValidationMode::Strict).unwrap();
    assert_eq!(libs.len(), 1);
    let map = bridge_library(&libs[0]).unwrap();
    assert!(map.contains_key("Position"));
    assert!(map.contains_key("Color"));
    assert!(map.contains_key("Sample"));
    assert!(map.contains_key("Vec3"));
    assert!(map.contains_key("Permissions"));

    // Position: final-Flag + key auf x.
    if let MinimalTypeObject::Struct(st) = &map["Position"] {
        assert!(st.struct_flags.has(StructTypeFlag::IS_FINAL));
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(StructMemberFlag::IS_KEY)
        );
    } else {
        panic!("Position not a struct");
    }

    // Sample: explicit @id auf id-Member, sequence-Wrap auf data, string-Bound auf label.
    if let MinimalTypeObject::Struct(st) = &map["Sample"] {
        assert!(st.struct_flags.has(StructTypeFlag::IS_APPENDABLE));
        assert_eq!(st.member_seq[0].common.member_id, 42);
        assert!(matches!(
            st.member_seq[1].common.member_type_id,
            TypeIdentifier::PlainSequenceLarge { bound: 1024, .. }
        ));
        assert_eq!(
            st.member_seq[2].common.member_type_id,
            TypeIdentifier::String8Small { bound: 64 }
        );
    } else {
        panic!("Sample not a struct");
    }

    // Vec3: typedef mit arrayDimensions → MinimalArray.
    assert!(matches!(map["Vec3"], MinimalTypeObject::Array(_)));
    if let MinimalTypeObject::Array(a) = &map["Vec3"] {
        assert_eq!(a.bound_seq, alloc::vec![3]);
    }

    // Permissions: Bitmask mit bitBound 8.
    if let MinimalTypeObject::Bitmask(bm) = &map["Permissions"] {
        assert_eq!(bm.bit_bound, 8);
        assert_eq!(bm.flag_seq.len(), 3);
    } else {
        panic!("Permissions not a bitmask");
    }
}

#[test]
fn end_to_end_typeobject_hashes_are_deterministic() {
    let libs = load_type_libraries_from_string(SAMPLE, ValidationMode::Strict).unwrap();
    let map1 = bridge_library(&libs[0]).unwrap();
    let map2 = bridge_library(&libs[0]).unwrap();
    let h1 = compute_minimal_hash(&map1["Position"]).unwrap();
    let h2 = compute_minimal_hash(&map2["Position"]).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn cross_reference_between_two_xml_structs_resolves() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<types xmlns="http://www.omg.org/spec/DDS-XML">
  <struct name="Inner">
    <member name="v" type="long"/>
  </struct>
  <struct name="Outer">
    <member name="inner" type="Inner"/>
  </struct>
</types>"#;
    let libs = load_type_libraries_from_string(xml, ValidationMode::Lax).unwrap();
    let map = bridge_library(&libs[0]).unwrap();
    let outer = map.get("Outer").unwrap();
    let inner = map.get("Inner").unwrap();
    let inner_hash = compute_minimal_hash(inner).unwrap();
    if let MinimalTypeObject::Struct(st) = outer {
        let m = &st.member_seq[0];
        assert_eq!(
            m.common.member_type_id,
            TypeIdentifier::EquivalenceHashMinimal(inner_hash)
        );
    } else {
        panic!();
    }
}

#[test]
fn single_type_bridge_via_xml_type_to_typeobject() {
    let libs = load_type_libraries_from_string(SAMPLE, ValidationMode::Strict).unwrap();
    let pos = libs[0].find("Position").unwrap();
    let to = xml_type_to_typeobject(pos).unwrap();
    let h = zerodds_types::compute_hash(&to).unwrap();
    // 14-byte EquivalenceHash.
    assert_eq!(h.0.len(), 14);
}

extern crate alloc;
