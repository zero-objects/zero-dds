//! Integration tests for the XML→TypeObject bridge (C4.5-b).
//!
//! Loader pipeline: `<types>` XML → [`xsd_loader`] pass →
//! [`TypeLibrary`] → [`bridge_library`] → `MinimalTypeObject` map.

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

extern crate alloc;

const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<types xmlns="http://www.omg.org/spec/DDS-XML">
  <struct name="Position" extensibility="final">
    <member name="x" type="float32" key="true"/>
    <member name="y" type="float32"/>
  </struct>

  <enum name="Color">
    <enumerator name="RED" value="0"/>
    <enumerator name="GREEN" value="1"/>
    <enumerator name="BLUE" value="2"/>
  </enum>

  <struct name="Sample" extensibility="append">
    <member name="id" type="int32" id="42" key="true"/>
    <member name="data" type="byte" sequenceMaxLength="1024"/>
    <member name="label" type="string" stringMaxLength="64"/>
  </struct>

  <typedef name="Vec3" type="float32" arrayDimensions="3"/>

  <bitmask name="Permissions" bitBound="8">
    <flag name="READ" position="0"/>
    <flag name="WRITE" position="1"/>
    <flag name="EXEC" position="2"/>
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

    // Position: final flag + key on x.
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

    // Sample: explicit @id on the id member, sequence wrap on data, string bound on label.
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

    // Vec3: typedef with arrayDimensions → alias of a plain array
    // (XTypes 1.3 §7.3.4.8).
    let MinimalTypeObject::Alias(vec3) = &map["Vec3"] else {
        panic!("Vec3 is not an alias");
    };
    assert!(matches!(
        &vec3.body.common.related_type,
        TypeIdentifier::PlainArraySmall { array_bounds, .. } if *array_bounds == alloc::vec![3u8]
    ));

    // Permissions: bitmask with bitBound 8.
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
    <member name="v" type="int32"/>
  </struct>
  <struct name="Outer">
    <member name="inner" type="nonBasic" nonBasicTypeName="Inner"/>
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

// ---------------------------------------------------------------------
// Types that refer to types known from earlier <types> strings
// (DDS-XRCE 1.0 §7.7.3.3.2: "may reference other Types already known to
// the Agent").
// ---------------------------------------------------------------------

use std::collections::BTreeMap;

use zerodds_types::{CompleteTypeObject, compute_complete_hash};
use zerodds_xml::{BridgeError, bridge_library_complete, bridge_library_complete_with_known};

fn complete_map(xml: &str) -> BTreeMap<String, CompleteTypeObject> {
    let libs = load_type_libraries_from_string(xml, ValidationMode::Lax).unwrap();
    bridge_library_complete(&libs[0]).unwrap()
}

fn complete_with_known(
    xml: &str,
    known: &BTreeMap<String, CompleteTypeObject>,
) -> Result<BTreeMap<String, CompleteTypeObject>, BridgeError> {
    let libs = load_type_libraries_from_string(xml, ValidationMode::Lax).unwrap();
    bridge_library_complete_with_known(&libs[0], known)
}

/// The identifier of a type is the EquivalenceHash of its Complete
/// TypeObject.
fn id_of(cto: &CompleteTypeObject) -> TypeIdentifier {
    TypeIdentifier::EquivalenceHashComplete(compute_complete_hash(cto).unwrap())
}

fn member_ids(cto: &CompleteTypeObject) -> Vec<TypeIdentifier> {
    let CompleteTypeObject::Struct(st) = cto else {
        panic!("expected a struct, got {cto:?}");
    };
    st.member_seq
        .iter()
        .map(|m| m.common.member_type_id.clone())
        .collect()
}

#[test]
fn struct_member_refers_to_a_struct_known_from_an_earlier_string() {
    let first = complete_map(
        r#"<types><struct name="Position"><member name="x" type="float32"/></struct></types>"#,
    );
    let second = complete_with_known(
        r#"<types><struct name="Pose"><member name="at" type="nonBasic" nonBasicTypeName="Position"/></struct></types>"#,
        &first,
    )
    .expect("a known type is a valid reference");

    // Only the types of the second string are produced.
    assert_eq!(second.keys().collect::<Vec<_>>(), ["Pose"]);
    // The member carries the identifier the known type has itself.
    assert_eq!(member_ids(&second["Pose"]), [id_of(&first["Position"])]);
}

#[test]
fn unknown_name_is_an_error_naming_it() {
    let known = BTreeMap::new();
    let err = complete_with_known(
        r#"<types><struct name="Pose"><member name="at" type="nonBasic" nonBasicTypeName="Missing"/></struct></types>"#,
        &known,
    )
    .expect_err("Missing is neither in the string nor known");
    match err {
        BridgeError::UnresolvedReference(msg) => {
            assert!(msg.contains("Missing"), "{msg}");
            assert!(msg.contains("Pose"), "{msg}");
        }
        other => panic!("expected UnresolvedReference, got {other:?}"),
    }
}

/// A reference two levels away must carry the identifier the referenced
/// type has in the result, which itself holds a reference to a known type.
#[test]
fn identifiers_hold_across_a_chain_of_references() {
    let known =
        complete_map(r#"<types><struct name="C"><member name="v" type="int32"/></struct></types>"#);
    let out = complete_with_known(
        r#"<types>
             <struct name="A"><member name="b" type="nonBasic" nonBasicTypeName="B"/></struct>
             <struct name="B"><member name="c" type="nonBasic" nonBasicTypeName="C"/></struct>
           </types>"#,
        &known,
    )
    .unwrap();
    assert_eq!(member_ids(&out["B"]), [id_of(&known["C"])]);
    assert_eq!(member_ids(&out["A"]), [id_of(&out["B"])]);
}

/// Names are looked up like IDL scoped names, from the module of the
/// referring type outwards; `::` names the global scope.
#[test]
fn scoped_lookup_reaches_modules_and_known_types() {
    let known = complete_map(
        r#"<types><module name="Base"><struct name="Id"><member name="v" type="int32"/></struct></module></types>"#,
    );
    let mut known_scoped = BTreeMap::new();
    // The key of a module's type is its scoped name.
    assert!(known.contains_key("Base::Id"));
    known_scoped.extend(known.clone());
    let out = complete_with_known(
        r#"<types>
             <module name="App">
               <struct name="Inner"><member name="v" type="int32"/></struct>
               <struct name="Outer">
                 <member name="inner" type="nonBasic" nonBasicTypeName="Inner"/>
                 <member name="id" type="nonBasic" nonBasicTypeName="::Base::Id"/>
               </struct>
             </module>
           </types>"#,
        &known_scoped,
    )
    .unwrap();
    assert_eq!(
        member_ids(&out["App::Outer"]),
        [id_of(&out["App::Inner"]), id_of(&known["Base::Id"])]
    );
}

#[test]
fn alias_union_discriminator_and_base_refer_to_known_types() {
    let known = complete_map(
        r#"<types>
             <enum name="Kind"><enumerator name="A"/><enumerator name="B"/></enum>
             <struct name="Base"><member name="v" type="int32"/></struct>
           </types>"#,
    );
    let out = complete_with_known(
        r#"<types>
             <typedef name="Kinds" type="nonBasic" nonBasicTypeName="Kind"/>
             <union name="U">
               <discriminator type="nonBasic" nonBasicTypeName="Kind"/>
               <case><caseDiscriminator value="0"/><member name="i" type="int32"/></case>
             </union>
             <struct name="Derived" baseType="Base"><member name="w" type="int32"/></struct>
           </types>"#,
        &known,
    )
    .unwrap();
    let CompleteTypeObject::Alias(alias) = &out["Kinds"] else {
        panic!("expected an alias");
    };
    assert_eq!(alias.body.related_type, id_of(&known["Kind"]));
    let CompleteTypeObject::Union(union) = &out["U"] else {
        panic!("expected a union");
    };
    assert_eq!(union.discriminator.common.type_id, id_of(&known["Kind"]));
    let CompleteTypeObject::Struct(derived) = &out["Derived"] else {
        panic!("expected a struct");
    };
    assert_eq!(derived.header.base_type, id_of(&known["Base"]));
}

#[test]
fn cycle_between_the_types_of_the_string_is_refused() {
    let err = complete_with_known(
        r#"<types>
             <struct name="A"><member name="b" type="nonBasic" nonBasicTypeName="B"/></struct>
             <struct name="B"><member name="a" type="nonBasic" nonBasicTypeName="A"/></struct>
           </types>"#,
        &BTreeMap::new(),
    )
    .expect_err("a cycle needs an SCC identifier");
    match err {
        BridgeError::UnsupportedXsdConstruct(msg) => {
            assert!(msg.contains('A') && msg.contains('B'), "{msg}");
        }
        other => panic!("expected UnsupportedXsdConstruct, got {other:?}"),
    }
}

/// The existing function is untouched: a string without references gives
/// the same types with or without known types.
#[test]
fn without_references_the_result_equals_bridge_library_complete() {
    let xml = r#"<types>
        <struct name="P"><member name="x" type="float32"/></struct>
        <enum name="E"><enumerator name="A"/></enum>
    </types>"#;
    let plain = complete_map(xml);
    let with_known = complete_with_known(xml, &BTreeMap::new()).unwrap();
    assert_eq!(plain, with_known);
}
