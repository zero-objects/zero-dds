// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//! The XSD Type Representation of DDS-XTypes 1.3 §7.3.3: the examples of the
//! clause read, a library written as a schema and read back, and the written
//! schema compiled and run against sample documents by `xmllint`.

use std::path::PathBuf;
use std::process::Command;

use zerodds_xml::{
    AutoId, Extensibility, PrimitiveType, TypeDef, TypeLibrary, TypeRef, bridge_library_complete,
    parse_type_libraries, parse_xsd_schema, type_library_to_xsd,
};

const XS: &str = "http://www.w3.org/2001/XMLSchema";

fn schema(body: &str) -> String {
    format!(
        r#"<xsd:schema xmlns:xsd="{XS}" xmlns:omg="http://www.omg.org/Type"
              xmlns:tns="http://www.omg.org/IDL-Mapped/"
              targetNamespace="http://www.omg.org/IDL-Mapped/">{body}</xsd:schema>"#
    )
}

fn read(body: &str) -> TypeLibrary {
    parse_xsd_schema(&schema(body)).expect("the schema reads")
}

fn library(body: &str) -> TypeLibrary {
    parse_type_libraries(&format!("<types>{body}</types>"))
        .expect("the library parses")
        .remove(0)
}

// ---------------------------------------------------------------------
// §7.3.3.1 Annotations
// ---------------------------------------------------------------------

/// §7.3.3.1.1, Table 27: a complexType whose xsd:annotation has an appinfo of
/// the source `.../Annotation/Definition` is an OMG annotation type; each
/// attribute is a member with its type and default.
#[test]
fn an_annotation_type_is_a_complex_type_with_the_definition_appinfo() {
    // The text of the clause spells the element `appInfo`; XML Schema's is
    // `appinfo`. Both are read.
    for appinfo in ["appInfo", "appinfo"] {
        let lib = read(&format!(
            r#"<xsd:complexType name="my_annotation">
                 <xsd:annotation>
                   <xsd:{appinfo} source="http://www.omg.org/Type/Annotation/Definition"/>
                 </xsd:annotation>
                 <xsd:attribute name="widgets" type="xsd:int"/>
                 <xsd:attribute name="gadgets" type="xsd:double" default="42.0"/>
               </xsd:complexType>
               <xsd:complexType name="not_an_annotation">
                 <xsd:sequence><xsd:element name="x" type="xsd:int"/></xsd:sequence>
               </xsd:complexType>"#
        ));
        let TypeDef::Annotation(a) = &lib.types[0] else {
            panic!("not an annotation type: {:?}", lib.types[0]);
        };
        assert_eq!(a.name, "my_annotation");
        assert_eq!(a.members.len(), 2);
        assert_eq!(a.members[0].name, "widgets");
        assert_eq!(
            a.members[0].type_ref,
            TypeRef::Primitive(PrimitiveType::Long)
        );
        assert_eq!(a.members[0].default_value, None);
        assert_eq!(a.members[1].name, "gadgets");
        assert_eq!(
            a.members[1].type_ref,
            TypeRef::Primitive(PrimitiveType::Double)
        );
        assert_eq!(a.members[1].default_value.as_deref(), Some("42.0"));
        assert!(matches!(&lib.types[1], TypeDef::Struct(s) if s.name == "not_an_annotation"));
    }
}

/// §7.3.3.1.2: "OMG Annotations shall be applied to a definition by
/// declaring, immediately within that definition's XML element, an XSD
/// Annotation containing an appInfo with its source attribute set to
/// `.../Annotation/Usage`. [...] The appInfo element shall contain an
/// element annotate for each OMG Annotation to be applied." The clause's
/// example applies MyAnnotation to MyStructure.
#[test]
fn annotations_are_applied_through_the_usage_appinfo() {
    let lib = read(
        r#"<xsd:complexType name="MyStructure">
             <xsd:annotation>
               <xsd:appInfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="MyAnnotation" widgets="12" gadgets="75.0"/>
                 <omg:annotate omg:type="Doc" text="hello"/>
               </xsd:appInfo>
             </xsd:annotation>
             <xsd:sequence>
               <xsd:element name="m" type="xsd:int">
                 <xsd:annotation>
                   <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                     <omg:annotate omg:type="MyAnnotation" widgets="1"/>
                   </xsd:appinfo>
                 </xsd:annotation>
               </xsd:element>
             </xsd:sequence>
           </xsd:complexType>
           <xsd:simpleType name="Color">
             <xsd:annotation>
               <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="Doc" text="on an enumeration"/>
               </xsd:appinfo>
             </xsd:annotation>
             <xsd:restriction base="xsd:string">
               <xsd:enumeration value="RED">
                 <xsd:annotation>
                   <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                     <omg:annotate omg:type="Doc" text="on a literal"/>
                   </xsd:appinfo>
                 </xsd:annotation>
               </xsd:enumeration>
             </xsd:restriction>
           </xsd:simpleType>"#,
    );
    let TypeDef::Struct(s) = &lib.types[0] else {
        panic!()
    };
    assert_eq!(s.annotations.len(), 2);
    assert_eq!(s.annotations[0].name, "MyAnnotation");
    let values: Vec<(&str, Option<&str>)> = s.annotations[0]
        .members
        .iter()
        .map(|m| (m.name.as_str(), m.value.as_deref()))
        .collect();
    assert_eq!(
        values,
        [("gadgets", Some("75.0")), ("widgets", Some("12"))],
        "every attribute but `type` is the value of a member"
    );
    assert_eq!(s.annotations[1].name, "Doc");
    assert_eq!(s.members[0].annotations[0].name, "MyAnnotation");
    let TypeDef::Enum(e) = &lib.types[1] else {
        panic!()
    };
    assert_eq!(
        e.annotations[0].members[0].value.as_deref(),
        Some("on an enumeration")
    );
    assert_eq!(
        e.enumerators[0].annotations[0].members[0].value.as_deref(),
        Some("on a literal")
    );
}

/// §7.3.3.1.3: "those Type System concepts represented with built-in
/// annotations in the IDL Type Representation shall be represented by
/// equivalent built-in annotations in this Type Representation."
#[test]
fn built_in_annotations_are_read_from_their_equivalents() {
    let lib = read(
        r#"<xsd:complexType name="S">
             <xsd:annotation>
               <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="mutable"/>
                 <omg:annotate omg:type="autoid" value="HASH"/>
                 <omg:annotate omg:type="nested"/>
                 <omg:annotate omg:type="verbatim" language="c++"
                               placement="after-declaration" text="// x"/>
               </xsd:appinfo>
             </xsd:annotation>
             <xsd:sequence>
               <xsd:element name="a" type="xsd:int">
                 <xsd:annotation>
                   <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                     <omg:annotate omg:type="key"/>
                     <omg:annotate omg:type="id" value="7"/>
                     <omg:annotate omg:type="must_understand"/>
                     <omg:annotate omg:type="non_serialized" value="FALSE"/>
                     <omg:annotate omg:type="external"/>
                     <omg:annotate omg:type="try_construct" value="TRIM"/>
                   </xsd:appinfo>
                 </xsd:annotation>
               </xsd:element>
               <xsd:element name="b" type="xsd:int" minOccurs="0"/>
             </xsd:sequence>
           </xsd:complexType>
           <xsd:simpleType name="E">
             <xsd:annotation>
               <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="appendable"/>
                 <omg:annotate omg:type="bit_bound" value="8"/>
               </xsd:appinfo>
             </xsd:annotation>
             <xsd:restriction base="xsd:string">
               <xsd:enumeration value="A">
                 <xsd:annotation>
                   <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                     <omg:annotate omg:type="value" value="5"/>
                     <omg:annotate omg:type="default_literal"/>
                   </xsd:appinfo>
                 </xsd:annotation>
               </xsd:enumeration>
               <xsd:enumeration value="B"/>
             </xsd:restriction>
           </xsd:simpleType>
           <xsd:simpleType name="Flags">
             <xsd:annotation>
               <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="bit_bound" value="16"/>
               </xsd:appinfo>
             </xsd:annotation>
             <xsd:list>
               <xsd:simpleType>
                 <xsd:restriction base="xsd:string">
                   <xsd:enumeration value="F0"/>
                   <xsd:enumeration value="F3">
                     <xsd:annotation>
                       <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                         <omg:annotate omg:type="position" value="3"/>
                       </xsd:appinfo>
                     </xsd:annotation>
                   </xsd:enumeration>
                 </xsd:restriction>
               </xsd:simpleType>
             </xsd:list>
           </xsd:simpleType>"#,
    );
    let TypeDef::Struct(s) = &lib.types[0] else {
        panic!()
    };
    assert_eq!(s.extensibility, Some(Extensibility::Mutable));
    assert_eq!(s.auto_id, Some(AutoId::Hash));
    assert_eq!(s.nested, Some(true));
    assert_eq!(s.verbatims[0].language, "c++");
    assert_eq!(s.verbatims[0].placement, "after-declaration");
    assert_eq!(s.verbatims[0].text, "// x");
    assert!(
        s.annotations.is_empty(),
        "built-in annotations are properties, not annotations"
    );
    let a = &s.members[0];
    assert!(a.key && a.must_understand && !a.optional);
    assert_eq!(a.id, Some(7));
    assert_eq!(a.non_serialized, Some(false));
    assert_eq!(a.external, Some(true));
    assert!(a.annotations.is_empty());
    assert!(
        s.members[1].optional,
        "minOccurs=0 is @optional (§7.3.3.2.2)"
    );
    let TypeDef::Enum(e) = &lib.types[1] else {
        panic!()
    };
    assert_eq!(e.extensibility, Some(Extensibility::Appendable));
    assert_eq!(e.bit_bound, Some(8));
    assert_eq!(e.enumerators[0].value, Some(5));
    assert_eq!(e.enumerators[0].default_literal, Some(true));
    let TypeDef::Bitmask(b) = &lib.types[2] else {
        panic!()
    };
    assert_eq!(b.bit_bound, Some(16));
    assert_eq!(b.bit_values[1].position, Some(3));
}

// ---------------------------------------------------------------------
// §7.3.3.3 and §7.3.3.4
// ---------------------------------------------------------------------

/// §7.3.3.3: "For each type T that is not a nested type, the schema shall
/// define an XML element of that type suitable for use as a document root.
/// The name of this element shall be the fully qualified name of T."
#[test]
fn a_root_element_is_declared_for_each_type_that_is_not_nested() {
    let lib = library(
        r#"<module name="MyModule">
             <struct name="MyStructure"><member name="a" type="int32"/></struct>
             <struct name="Inner" nested="true"><member name="a" type="int32"/></struct>
             <union name="U"><discriminator type="int32"/>
               <case><caseDiscriminator value="1"/><member name="x" type="int32"/></case></union>
           </module>"#,
    );
    let xsd = type_library_to_xsd(&lib).expect("writes");
    assert!(
        xsd.contains(
            r#"<xs:element name="MyModule.MyStructure" type="tns:MyModule.MyStructure"/>"#
        ),
        "the example of the clause: {xsd}"
    );
    assert!(
        xsd.contains(r#"<xs:element name="MyModule.U" type="tns:MyModule.U"/>"#),
        "{xsd}"
    );
    assert!(
        !xsd.contains(r#"<xs:element name="MyModule.Inner""#),
        "a nested type has no root element: {xsd}"
    );
    // Reading it back: the nested type is nested, the others are not.
    let back = parse_xsd_schema(&xsd).expect("reads");
    let TypeDef::Module(m) = &back.types[0] else {
        panic!("the dotted names make a module");
    };
    assert_eq!(m.name, "MyModule");
    let nested: Vec<(&str, Option<bool>)> = m
        .types
        .iter()
        .map(|t| match t {
            TypeDef::Struct(s) => (s.name.as_str(), s.nested),
            TypeDef::Union(u) => (u.name.as_str(), u.nested),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        nested,
        [("MyStructure", None), ("Inner", Some(true)), ("U", None)]
    );
}

/// A type annotated nested that also has a root element contradicts itself.
#[test]
fn a_nested_type_with_a_root_element_is_refused() {
    let result = parse_xsd_schema(&schema(
        r#"<xsd:complexType name="T">
             <xsd:annotation>
               <xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
                 <omg:annotate omg:type="nested"/>
               </xsd:appinfo>
             </xsd:annotation>
             <xsd:sequence><xsd:element name="a" type="xsd:int"/></xsd:sequence>
           </xsd:complexType>
           <xsd:element name="T" type="tns:T"/>"#,
    ));
    assert!(result.is_err());
}

/// §7.3.3.4: "the XSD sequence declaration specifies a maxOccurs
/// multiplicity equal to the bound of the map [...] The map elements are
/// represented by elements named key and value, each of which must occur
/// exactly once for each iteration of the sequence." The example is a map of
/// integers to floating-point numbers with a bound of 32.
#[test]
fn a_map_is_a_sequence_with_max_occurs_and_the_elements_key_and_value() {
    let lib = read(
        r#"<xsd:complexType name="MyMap">
             <xsd:sequence maxOccurs="32">
               <xsd:element name="key" minOccurs="1" maxOccurs="1" type="xsd:int"/>
               <xsd:element name="value" minOccurs="1" maxOccurs="1" type="xsd:double"/>
             </xsd:sequence>
           </xsd:complexType>
           <xsd:complexType name="Holder">
             <xsd:sequence>
               <xsd:element name="m" type="tns:MyMap"/>
               <xsd:element name="unbounded">
                 <xsd:complexType>
                   <xsd:sequence maxOccurs="unbounded">
                     <xsd:element name="key" type="xsd:string"/>
                     <xsd:element name="value" type="xsd:long"/>
                   </xsd:sequence>
                 </xsd:complexType>
               </xsd:element>
             </xsd:sequence>
           </xsd:complexType>"#,
    );
    let TypeDef::Typedef(map) = &lib.types[0] else {
        panic!("a map is an alias: {:?}", lib.types[0]);
    };
    assert!(map.is_map());
    assert_eq!(map.map_key, Some(TypeRef::Primitive(PrimitiveType::Long)));
    assert_eq!(map.type_ref, TypeRef::Primitive(PrimitiveType::Double));
    assert_eq!(map.map_max_length, Some(32));
    let TypeDef::Struct(holder) = &lib.types[1] else {
        panic!()
    };
    assert_eq!(holder.members[0].type_ref, TypeRef::Named("MyMap".into()));
    let unbounded = &holder.members[1];
    assert_eq!(
        unbounded.map_key,
        Some(TypeRef::Primitive(PrimitiveType::String))
    );
    assert_eq!(
        unbounded.type_ref,
        TypeRef::Primitive(PrimitiveType::LongLong)
    );
    assert_eq!(unbounded.map_max_length, None);

    // And writing the model gives the sequence with maxOccurs, key and value.
    let written = type_library_to_xsd(&library(
        r#"<typedef name="MyMap" type="float64" mapKeyType="int32" mapMaxLength="32"/>"#,
    ))
    .expect("writes");
    assert!(
        written.contains(r#"<xs:sequence maxOccurs="32">"#),
        "{written}"
    );
    assert!(
        written.contains(r#"<xs:element name="key" type="xs:int"/>"#),
        "{written}"
    );
    assert!(
        written.contains(r#"<xs:element name="value" type="xs:double"/>"#),
        "{written}"
    );
}

// ---------------------------------------------------------------------
// §7.3.3: as if the IDL mapping to XSD were applied
// ---------------------------------------------------------------------

const EVERYTHING: &str = r#"<module name="M">
      <enum name="Color" bitBound="8">
        <enumerator name="RED" value="1" defaultLiteral="true"/><enumerator name="GREEN"/>
      </enum>
      <bitmask name="Flags" bitBound="16"><flag name="A" position="0"/><flag name="B" position="3"/></bitmask>
      <typedef name="Grid" type="int16" arrayDimensions="2,3"/>
      <typedef name="Dict" type="nonBasic" nonBasicTypeName="M::Color" mapKeyType="string"
               mapMaxLength="8" mapKeyStringMaxLength="6"/>
      <typedef name="Alias" type="int32"/>
      <typedef name="ColorAlias" type="nonBasic" nonBasicTypeName="M::Color"/>
      <struct name="Base"><member name="b" type="int8"/></struct>
      <typedef name="BaseAlias" type="nonBasic" nonBasicTypeName="M::Base"/>
      <struct name="S" baseType="M::Base" extensibility="mutable" autoid="hash">
        <member name="id" type="int32" key="true" id="7"/>
        <member name="c" type="nonBasic" nonBasicTypeName="M::Color"/>
        <member name="text" type="string" stringMaxLength="12" optional="true"/>
        <member name="q" type="float64" sequenceMaxLength="-1"/>
        <member name="r" type="float64" sequenceMaxLength="4" optional="true"/>
        <member name="grid" type="int16" arrayDimensions="2,3"/>
        <member name="seqs" type="int8" sequenceMaxLength="3" arrayDimensions="2"/>
        <member name="m" type="float64" mapKeyType="int32" mapMaxLength="32"/>
        <member name="u" type="uint8"/>
        <member name="w" type="wstring" stringMaxLength="4"/>
        <member name="ch" type="char8"/>
        <member name="wc" type="char16"/>
        <member name="ld" type="float128"/>
        <member name="b" type="boolean" external="true" tryConstruct="trim" nonSerialized="true"
                mustUnderstand="true"/>
      </struct>
      <union name="U" extensibility="final">
        <discriminator type="nonBasic" nonBasicTypeName="M::Color" key="true"/>
        <case><caseDiscriminator value="RED"/><caseDiscriminator value="GREEN"/>
          <member name="x" type="int32"/></case>
        <case><caseDiscriminator value="default"/><member name="y" type="string"
                                                          stringMaxLength="3"/></case>
      </union>
    </module>"#;

/// §7.3.3: a library written as a schema and read again is the same
/// library once its names are fully qualified, and writing that gives the
/// same schema.
#[test]
fn a_library_survives_its_xsd_representation() {
    let lib = library(EVERYTHING);
    let xsd = type_library_to_xsd(&lib).expect("writes");
    let back = parse_xsd_schema(&xsd).expect("reads");
    assert_eq!(type_library_to_xsd(&back).expect("writes again"), xsd);

    // The structure kept everything that has a place in the schema.
    let TypeDef::Module(m) = &back.types[0] else {
        panic!()
    };
    let s = m
        .types
        .iter()
        .find_map(|t| match t {
            TypeDef::Struct(s) if s.name == "S" => Some(s),
            _ => None,
        })
        .expect("S");
    assert_eq!(s.base_type.as_deref(), Some("M::Base"));
    assert_eq!(s.extensibility, Some(Extensibility::Mutable));
    assert_eq!(s.auto_id, Some(AutoId::Hash));
    let by_name = |n: &str| s.members.iter().find(|m| m.name == n).expect(n);
    assert!(by_name("id").key);
    assert_eq!(by_name("id").id, Some(7));
    assert_eq!(by_name("c").type_ref, TypeRef::Named("M::Color".into()));
    assert!(by_name("text").optional);
    assert_eq!(by_name("text").string_max_length, Some(12));
    assert_eq!(by_name("q").sequence_max_length, Some(0));
    assert!(by_name("r").optional && by_name("r").sequence_max_length == Some(4));
    assert_eq!(by_name("grid").array_dimensions, [2, 3]);
    assert_eq!(by_name("seqs").sequence_max_length, Some(3));
    assert_eq!(by_name("seqs").array_dimensions, [2]);
    assert_eq!(
        by_name("m").map_key,
        Some(TypeRef::Primitive(PrimitiveType::Long))
    );
    assert_eq!(by_name("m").map_max_length, Some(32));
    assert_eq!(
        by_name("u").type_ref,
        TypeRef::Primitive(PrimitiveType::UInt8)
    );
    assert_eq!(
        by_name("w").type_ref,
        TypeRef::Primitive(PrimitiveType::WString)
    );
    assert_eq!(by_name("w").string_max_length, Some(4));
    assert_eq!(
        by_name("ch").type_ref,
        TypeRef::Primitive(PrimitiveType::Char)
    );
    assert_eq!(
        by_name("wc").type_ref,
        TypeRef::Primitive(PrimitiveType::WChar)
    );
    assert_eq!(
        by_name("ld").type_ref,
        TypeRef::Primitive(PrimitiveType::LongDouble)
    );
    assert!(by_name("b").must_understand);
    assert_eq!(by_name("b").external, Some(true));
    assert_eq!(by_name("b").non_serialized, Some(true));
    let u = m
        .types
        .iter()
        .find_map(|t| match t {
            TypeDef::Union(u) => Some(u),
            _ => None,
        })
        .expect("U");
    assert!(u.discriminator_key);
    assert_eq!(u.cases.len(), 2);
    assert_eq!(u.cases[0].discriminators.len(), 2);
}

/// The TypeObjects of the original and of its XSD representation are the
/// same (XTypes 1.3 §7.3.3 with §7.3.4).
#[test]
fn the_xsd_representation_has_the_type_objects_of_the_original() {
    let lib = library(
        r#"<module name="M">
             <enum name="Color"><enumerator name="RED"/><enumerator name="GREEN"/></enum>
             <struct name="S" extensibility="append">
               <member name="id" type="int32" key="true"/>
               <member name="c" type="nonBasic" nonBasicTypeName="Color"/>
               <member name="q" type="float64" sequenceMaxLength="8"/>
               <member name="o" type="string" stringMaxLength="5" optional="true"/>
             </struct>
           </module>"#,
    );
    let original = bridge_library_complete(&lib).expect("bridge of the original");
    let xsd = type_library_to_xsd(&lib).expect("writes");
    let back = parse_xsd_schema(&xsd).expect("reads");
    let again = bridge_library_complete(&back).expect("bridge of the XSD");
    assert_eq!(
        original.keys().collect::<Vec<_>>(),
        again.keys().collect::<Vec<_>>()
    );
    for (name, object) in &original {
        assert_eq!(object, &again[name], "{name}");
    }
}

#[test]
fn what_has_no_xsd_form_is_refused_by_name() {
    let bitset = zerodds_xml::parse_type_libraries_with(
        r#"<types><bitset name="B"><bitfield name="a" type="uint16" mask="0xFF"/></bitset></types>"#,
        &zerodds_xml::ReadOptions::accept_vendor_forms(),
    )
    .expect("the bitset is a vendor form")
    .value
    .remove(0);
    let error = type_library_to_xsd(&bitset).expect_err("a bitset has no XSD form");
    assert!(format!("{error:?}").contains("bitset `B`"), "{error:?}");

    let element = library(r#"<struct name="W"><member name="element" type="int32"/></struct>"#);
    let error = type_library_to_xsd(&element).expect_err("the reader would take it for a sequence");
    assert!(format!("{error:?}").contains("`element`"), "{error:?}");

    let unresolved = library(
        r#"<struct name="S"><member name="a" type="nonBasic" nonBasicTypeName="Nowhere"/></struct>"#,
    );
    assert!(type_library_to_xsd(&unresolved).is_err());
}

// ---------------------------------------------------------------------
// The IDL-to-XSD mapping (CORBA to WSDL/SOAP Interworking 1.2.1,
// formal/2008-08-03, clause 4.1.7), to which §7.3.3 refers
// ---------------------------------------------------------------------

/// The examples of 4.1.7.2 to 4.1.7.6, as the clause writes them (the WS-I
/// form of a sequence or an array and its SOAP-encoding twin beside it), with
/// the one addition XTypes needs: the `case` annotation of a union branch,
/// which the mapping has as a comment only.
const MAPPING_EXAMPLES: &str = r#"
  <xsd:complexType name="Example.myStruct">
    <xsd:sequence>
      <xsd:element name="str" type="xsd:string" nillable="true" maxOccurs="1" minOccurs="1"/>
      <xsd:element name="s" type="xsd:short" maxOccurs="1" minOccurs="1"/>
      <xsd:element name="ull" type="xsd:unsignedLong" maxOccurs="1" minOccurs="1"/>
      <xsd:element name="f" type="xsd:float" maxOccurs="1" minOccurs="1"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:simpleType name="Example.Number"><xsd:restriction base="xsd:int"/></xsd:simpleType>
  <xsd:simpleType name="Example.OtherNumber">
    <xsd:restriction base="Example.Number"/>
  </xsd:simpleType>
  <xsd:complexType name="S">
    <xsd:sequence>
      <xsd:element name="dummy" type="xsd:int" maxOccurs="1" minOccurs="1"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="S_t">
    <xsd:complexContent>
      <xsd:restriction base="tns:S">
        <xsd:sequence>
          <xsd:element name="dummy" type="xsd:int" maxOccurs="1" minOccurs="1"/>
        </xsd:sequence>
      </xsd:restriction>
    </xsd:complexContent>
  </xsd:complexType>
  <xsd:complexType name="Example._SE_longSeq">
    <xsd:complexContent>
      <xsd:restriction base="soapenc:Array">
        <xsd:sequence>
          <xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="xsd:int"/>
        </xsd:sequence>
        <xsd:attribute ref="soapenc:arrayType" w:arrayType="xsd:int[]"/>
      </xsd:restriction>
    </xsd:complexContent>
  </xsd:complexType>
  <xsd:complexType name="Example.longSeq">
    <xsd:sequence>
      <xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="xsd:int"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="Example.strSeq">
    <xsd:sequence>
      <xsd:element name="item" minOccurs="0" maxOccurs="10" type="xsd:string"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="Example.structSeq">
    <xsd:sequence>
      <xsd:element name="item" minOccurs="0" maxOccurs="unbounded" type="Example.myStruct"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="Example.arrayLong">
    <xsd:sequence>
      <xsd:element name="item" type="xsd:int" minOccurs="10" maxOccurs="10"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="_SE_T.field_ArrayOfint">
    <xsd:complexContent>
      <xsd:restriction base="soapenc:Array">
        <xsd:sequence>
          <xsd:element name="item" type="xsd:int" minOccurs="10" maxOccurs="10"/>
        </xsd:sequence>
        <xsd:attribute ref="soapenc:arrayType" w:arrayType="xsd:int[]"/>
      </xsd:restriction>
    </xsd:complexContent>
  </xsd:complexType>
  <xsd:complexType name="_SE_T">
    <xsd:sequence>
      <xsd:element name="field" type="_SE_T.field_ArrayOfint" nillable="true"
                   maxOccurs="1" minOccurs="1"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="T.field_ArrayOfint">
    <xsd:sequence>
      <xsd:element name="item" type="xsd:int" minOccurs="10" maxOccurs="10"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="T">
    <xsd:sequence>
      <xsd:element name="field" type="T.field_ArrayOfint" nillable="true"
                   maxOccurs="1" minOccurs="1"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="ArrayOfint">
    <xsd:sequence>
      <xsd:element name="item" type="xsd:int" minOccurs="5" maxOccurs="5"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="matrix">
    <xsd:sequence>
      <xsd:element name="item1" type="ArrayOfint" minOccurs="3" maxOccurs="3"/>
    </xsd:sequence>
  </xsd:complexType>
  <xsd:complexType name="Example.myUnion">
    <xsd:sequence>
      <xsd:element name="discriminator" type="xsd:int"/>
      <xsd:choice>
        <xsd:element name="l" type="xsd:int" minOccurs="0" maxOccurs="1">
          <xsd:annotation><xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
            <omg:annotate omg:type="case" value="0"/></xsd:appinfo></xsd:annotation>
        </xsd:element>
        <xsd:element name="f" type="xsd:float" minOccurs="0" maxOccurs="1">
          <xsd:annotation><xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
            <omg:annotate omg:type="case" value="2,3"/></xsd:appinfo></xsd:annotation>
        </xsd:element>
        <xsd:element name="o" type="xsd:byte" minOccurs="0" maxOccurs="1">
          <xsd:annotation><xsd:appinfo source="http://www.omg.org/Type/Annotation/Usage">
            <omg:annotate omg:type="case" value="default"/></xsd:appinfo></xsd:annotation>
        </xsd:element>
      </xsd:choice>
    </xsd:sequence>
  </xsd:complexType>
"#;

fn mapping_library() -> TypeLibrary {
    parse_xsd_schema(&format!(
        r#"<xsd:schema xmlns:xsd="{XS}" xmlns:omg="http://www.omg.org/Type"
              xmlns:tns="http://www.omg.org/IDL-Mapped/"
              xmlns:soapenc="http://schemas.xmlsoap.org/soap/encoding/"
              xmlns:w="http://schemas.xmlsoap.org/wsdl/"
              targetNamespace="http://www.omg.org/IDL-Mapped/">{MAPPING_EXAMPLES}</xsd:schema>"#
    ))
    .expect("the examples of the mapping read")
}

fn find<'a>(types: &'a [TypeDef], name: &str) -> &'a TypeDef {
    types.iter().find(|t| t.name() == name).unwrap_or_else(|| {
        panic!(
            "`{name}` is not declared: {:?}",
            types.iter().map(TypeDef::name).collect::<Vec<_>>()
        )
    })
}

/// 4.1.7.2 to 4.1.7.6: what the mapping writes for IDL is read as that IDL.
#[test]
fn the_examples_of_the_mapping_are_read_as_the_idl_they_come_from() {
    let lib = mapping_library();
    let names: Vec<&str> = lib.types.iter().map(TypeDef::name).collect();
    // Neither the SOAP-encoding twins nor the types the mapping writes for
    // arrays are declarations of the library.
    for gone in ["_SE_T", "ArrayOfint", "T.field_ArrayOfint"] {
        assert!(!names.contains(&gone), "{gone} is in {names:?}");
    }
    assert!(!names.iter().any(|n| n.contains("_SE_")), "{names:?}");

    let TypeDef::Module(example) = find(&lib.types, "Example") else {
        panic!("Example is a module")
    };
    // 4.1.7.2 a structure
    let TypeDef::Struct(my_struct) = find(&example.types, "myStruct") else {
        panic!()
    };
    assert_eq!(
        my_struct
            .members
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["str", "s", "ull", "f"]
    );
    assert_eq!(
        my_struct.members[2].type_ref,
        TypeRef::Primitive(PrimitiveType::ULongLong)
    );
    // 4.1.7.3 typedefs
    let TypeDef::Typedef(number) = find(&example.types, "Number") else {
        panic!()
    };
    assert_eq!(number.type_ref, TypeRef::Primitive(PrimitiveType::Long));
    let TypeDef::Typedef(other) = find(&example.types, "OtherNumber") else {
        panic!()
    };
    assert_eq!(other.type_ref, TypeRef::Named("Example::Number".into()));
    let TypeDef::Typedef(s_t) = find(&lib.types, "S_t") else {
        panic!("S_t is an alias, the restriction of S")
    };
    assert_eq!(s_t.type_ref, TypeRef::Named("S".into()));
    // 4.1.7.4 a union
    let TypeDef::Union(union) = find(&example.types, "myUnion") else {
        panic!()
    };
    assert_eq!(union.cases.len(), 3);
    assert_eq!(union.cases[1].discriminators.len(), 2);
    // 4.1.7.5 sequences
    let TypeDef::Typedef(long_seq) = find(&example.types, "longSeq") else {
        panic!()
    };
    assert_eq!(long_seq.sequence_max_length, Some(0));
    assert_eq!(long_seq.type_ref, TypeRef::Primitive(PrimitiveType::Long));
    let TypeDef::Typedef(str_seq) = find(&example.types, "strSeq") else {
        panic!()
    };
    assert_eq!(str_seq.sequence_max_length, Some(10));
    let TypeDef::Typedef(struct_seq) = find(&example.types, "structSeq") else {
        panic!()
    };
    assert_eq!(
        struct_seq.type_ref,
        TypeRef::Named("Example::myStruct".into())
    );
    // 4.1.7.6 arrays: an explicit one, one declared as a field, and
    // `long matrix[5][3]`, whose first dimension is the innermost type.
    let TypeDef::Typedef(array_long) = find(&example.types, "arrayLong") else {
        panic!()
    };
    assert_eq!(array_long.array_dimensions, [10]);
    let TypeDef::Struct(t) = find(&lib.types, "T") else {
        panic!()
    };
    assert_eq!(t.members[0].array_dimensions, [10]);
    assert_eq!(
        t.members[0].type_ref,
        TypeRef::Primitive(PrimitiveType::Long)
    );
    let TypeDef::Typedef(matrix) = find(&lib.types, "matrix") else {
        panic!()
    };
    assert_eq!(matrix.array_dimensions, [5, 3]);
    assert_eq!(matrix.type_ref, TypeRef::Primitive(PrimitiveType::Long));
}

/// The XSD of DDS-XML 1.0 and of an earlier version of this reader named the
/// one element of a collection `element`; it is read as `item` is.
#[test]
fn the_element_of_a_collection_may_still_be_named_element() {
    let lib = read(
        r#"<xsd:complexType name="Q">
             <xsd:sequence>
               <xsd:element name="element" type="xsd:int" minOccurs="0" maxOccurs="8"/>
             </xsd:sequence>
           </xsd:complexType>"#,
    );
    let TypeDef::Typedef(q) = &lib.types[0] else {
        panic!()
    };
    assert_eq!(q.sequence_max_length, Some(8));
}

#[test]
fn the_writer_writes_what_the_mapping_writes() {
    let lib = library(
        r#"<module name="Example">
             <struct name="S"><member name="dummy" type="int32"/></struct>
             <struct name="D" baseType="S"><member name="more" type="string"/></struct>
             <typedef name="S_t" type="nonBasic" nonBasicTypeName="S"/>
             <typedef name="D_t" type="nonBasic" nonBasicTypeName="D"/>
             <typedef name="LongSeq" type="int32" sequenceMaxLength="-1"/>
             <typedef name="matrix" type="int32" arrayDimensions="5,3"/>
             <struct name="T">
               <member name="field" type="int32" arrayDimensions="10"/>
               <member name="text" type="string" stringMaxLength="10"/>
               <member name="n" type="int32"/>
               <member name="grid" type="int16" arrayDimensions="2,3"/>
             </struct>
             <union name="U"><discriminator type="int32"/>
               <case><caseDiscriminator value="1"/><member name="l" type="int32"/></case>
               <case><caseDiscriminator value="default"/><member name="o" type="uint8"/></case>
             </union>
           </module>"#,
    );
    let xsd = type_library_to_xsd(&lib).expect("writes");
    // 4.1.7.5: a sequence is the one element `item`, `minOccurs` 0.
    assert!(
        xsd.contains(
            r#"<xs:element name="item" type="xs:int" minOccurs="0" maxOccurs="unbounded"/>"#
        ),
        "{xsd}"
    );
    // 4.1.7.6: `long matrix[5][3]` is ArrayOfint of 5 and `matrix` of 3.
    assert!(
        xsd.contains(r#"<xs:complexType name="ArrayOfint">"#),
        "{xsd}"
    );
    assert!(
        xsd.contains(r#"<xs:element name="item" type="xs:int" minOccurs="5" maxOccurs="5"/>"#),
        "{xsd}"
    );
    assert!(
        xsd.contains(
            r#"<xs:element name="item1" type="tns:ArrayOfint" minOccurs="3" maxOccurs="3"/>"#
        ),
        "{xsd}"
    );
    // An array declared as a field is a type named after the type and the
    // field, and the field is nillable.
    assert!(
        xsd.contains(r#"<xs:complexType name="Example.T.field_ArrayOfint">"#),
        "{xsd}"
    );
    assert!(
        xsd.contains(r#"name="field" type="tns:Example.T.field_ArrayOfint" nillable="true""#),
        "{xsd}"
    );
    // 4.1.7.2: a string member is nillable, an integer member is not; a
    // bounded string has a `maxLength` that is fixed (4.1.6).
    assert!(
        xsd.contains(r#"name="more" type="xs:string" nillable="true""#),
        "{xsd}"
    );
    assert!(
        xsd.contains(r#"<xs:element name="n" type="xs:int"/>"#),
        "{xsd}"
    );
    assert!(
        xsd.contains(r#"<xs:maxLength value="10" fixed="true"/>"#),
        "{xsd}"
    );
    // 4.1.7.4: the branches of a union are optional within the choice.
    assert!(
        xsd.contains(r#"<xs:element name="l" type="xs:int" minOccurs="0">"#),
        "{xsd}"
    );
    // 4.1.7.3: an alias of a complex type restates its content in a
    // restriction; for a derived structure the content of the base comes
    // first.
    let restriction = |name: &str| {
        let at = xsd
            .find(&format!(r#"<xs:complexType name="Example.{name}">"#))
            .unwrap_or_else(|| panic!("{name} is not written:\n{xsd}"));
        let end = xsd[at..].find("</xs:complexType>").unwrap();
        xsd[at..at + end].to_string()
    };
    let s_t = restriction("S_t");
    assert!(
        s_t.contains(r#"<xs:restriction base="tns:Example.S">"#),
        "{s_t}"
    );
    assert!(s_t.contains(r#"name="dummy""#), "{s_t}");
    let d_t = restriction("D_t");
    assert!(
        d_t.contains(r#"<xs:restriction base="tns:Example.D">"#),
        "{d_t}"
    );
    assert!(
        d_t.find(r#"name="dummy""#).unwrap() < d_t.find(r#"name="more""#).unwrap(),
        "{d_t}"
    );

    // And all of it is read back as the library it was.
    let back = parse_xsd_schema(&xsd).expect("reads");
    assert_eq!(type_library_to_xsd(&back).expect("writes again"), xsd);
    let TypeDef::Module(m) = &back.types[0] else {
        panic!()
    };
    let TypeDef::Typedef(matrix) = find(&m.types, "matrix") else {
        panic!()
    };
    assert_eq!(matrix.array_dimensions, [5, 3]);
    let TypeDef::Struct(t) = find(&m.types, "T") else {
        panic!()
    };
    assert_eq!(t.members[0].array_dimensions, [10]);
    assert_eq!(t.members[3].array_dimensions, [2, 3]);
    assert!(
        !m.types.iter().any(|d| d.name().contains("ArrayOf")),
        "the types written for the arrays are no declarations: {:?}",
        m.types.iter().map(TypeDef::name).collect::<Vec<_>>()
    );
    let TypeDef::Typedef(s_t) = find(&m.types, "S_t") else {
        panic!()
    };
    assert_eq!(s_t.type_ref, TypeRef::Named("Example::S".into()));
}

/// Two arrays of the same element and other dimensions need two intermediate
/// types: the second gets the suffix `_1` (4.1.7.6).
#[test]
fn intermediate_types_with_other_dimensions_get_a_suffix() {
    let lib = library(
        r#"<struct name="T">
             <member name="a" type="int32" arrayDimensions="5,3"/>
             <member name="b" type="int32" arrayDimensions="6,4"/>
             <member name="c" type="int32" arrayDimensions="5,7"/>
           </struct>"#,
    );
    let xsd = type_library_to_xsd(&lib).expect("writes");
    for name in ["ArrayOfint", "ArrayOfint_1"] {
        assert!(
            xsd.contains(&format!(r#"<xs:complexType name="{name}">"#)),
            "{name}:\n{xsd}"
        );
    }
    // `a` and `c` share the intermediate type of five.
    assert_eq!(
        xsd.matches(r#"<xs:complexType name="ArrayOfint">"#).count(),
        1
    );
    let back = parse_xsd_schema(&xsd).expect("reads");
    let TypeDef::Struct(t) = find(&back.types, "T") else {
        panic!()
    };
    assert_eq!(t.members[0].array_dimensions, [5, 3]);
    assert_eq!(t.members[1].array_dimensions, [6, 4]);
    assert_eq!(t.members[2].array_dimensions, [5, 7]);
}

// ---------------------------------------------------------------------
// The schema as the second opinion
// ---------------------------------------------------------------------

fn xmllint() -> Option<String> {
    let prog = std::env::var("ZERODDS_XMLLINT").unwrap_or_else(|_| "xmllint".into());
    match Command::new(&prog).arg("--version").output() {
        Ok(out) if out.status.success() => Some(prog),
        _ => None,
    }
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join("zerodds-xml-xsd-representation");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Validates `instance` against `xsd`; `Ok` or the message.
fn lint(prog: &str, name: &str, xsd: &str, instance: &str) -> Result<(), String> {
    let dir = scratch();
    let schema_path = dir.join(format!("{name}.xsd"));
    let instance_path = dir.join(format!("{name}.xml"));
    std::fs::write(&schema_path, xsd).expect("schema");
    std::fs::write(&instance_path, instance).expect("instance");
    let out = Command::new(prog)
        .arg("--nonet")
        .arg("--noout")
        .arg("--schema")
        .arg(&schema_path)
        .arg(&instance_path)
        .output()
        .expect("xmllint runs");
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// The written schema is a schema a validator compiles, and it accepts the
/// documents of its types and rejects what is not: the root element is the
/// fully qualified name, a nested type has none, and a sequence is bounded.
#[test]
#[allow(clippy::print_stderr)]
fn the_written_schema_compiles_and_validates_sample_documents() {
    let Some(prog) = xmllint() else {
        eprintln!("[skip] xmllint not runnable (set ZERODDS_XMLLINT to override)");
        return;
    };
    let lib = library(
        r#"<module name="M">
             <enum name="Color"><enumerator name="RED"/><enumerator name="GREEN"/></enum>
             <bitmask name="Flags"><flag name="A" position="0"/><flag name="B" position="1"/></bitmask>
             <struct name="Inner" nested="true"><member name="v" type="int32"/></struct>
             <struct name="S">
               <member name="id" type="int32" key="true"/>
               <member name="c" type="nonBasic" nonBasicTypeName="Color"/>
               <member name="f" type="nonBasic" nonBasicTypeName="Flags"/>
               <member name="text" type="string" stringMaxLength="5" optional="true"/>
               <member name="q" type="float64" sequenceMaxLength="2"/>
               <member name="grid" type="int16" arrayDimensions="2"/>
               <member name="grid2" type="int16" arrayDimensions="2,3"/>
               <member name="m" type="int16" mapKeyType="int32" mapMaxLength="2"/>
               <member name="in" type="nonBasic" nonBasicTypeName="Inner"/>
             </struct>
             <union name="U"><discriminator type="int32"/>
               <case><caseDiscriminator value="1"/><member name="x" type="int32"/></case>
               <case><caseDiscriminator value="2"/><member name="y" type="string"/></case>
             </union>
           </module>"#,
    );
    let xsd = type_library_to_xsd(&lib).expect("writes");
    let ns = "http://www.omg.org/IDL-Mapped/";
    let good = format!(
        r#"<M.S xmlns="{ns}"><id>1</id><c>RED</c><f>A B</f><text>abc</text>
             <q><item>1.5</item><item>2.5</item></q>
             <grid><item>1</item><item>2</item></grid>
             <grid2><item1><item>1</item><item>2</item></item1>
                    <item1><item>3</item><item>4</item></item1>
                    <item1><item>5</item><item>6</item></item1></grid2>
             <m><key>1</key><value>10</value><key>2</key><value>20</value></m>
             <in><v>3</v></in></M.S>"#
    );
    lint(&prog, "good", &xsd, &good)
        .unwrap_or_else(|message| panic!("the sample document is rejected:\n{message}\n{xsd}"));
    lint(
        &prog,
        "union",
        &xsd,
        &format!(r#"<M.U xmlns="{ns}"><discriminator>1</discriminator><x>5</x></M.U>"#),
    )
    .unwrap_or_else(|message| panic!("the union document is rejected:\n{message}"));

    let rejected = [
        (
            "an unknown literal",
            good.replace("<c>RED</c>", "<c>BLUE</c>"),
        ),
        ("an unknown flag", good.replace("<f>A B</f>", "<f>C</f>")),
        ("a string past its bound", good.replace("abc", "abcdefg")),
        (
            "a sequence past its bound",
            good.replace("</q>", "<item>3</item></q>"),
        ),
        (
            "an array of the wrong size",
            good.replace(
                "<grid><item>1</item><item>2</item></grid>",
                "<grid><item>1</item></grid>",
            ),
        ),
        (
            "a two-dimensional array with one row too few",
            good.replace("<item1><item>5</item><item>6</item></item1>", ""),
        ),
        (
            "a two-dimensional array with a row too short",
            good.replace("<item>6</item>", ""),
        ),
        (
            "a map past its bound",
            good.replace("</m>", "<key>3</key><value>30</value></m>"),
        ),
        (
            "a map entry without its value",
            good.replace("<value>20</value>", ""),
        ),
        (
            "a nested type as the root",
            format!(r#"<M.Inner xmlns="{ns}"><v>1</v></M.Inner>"#),
        ),
    ];
    for (what, instance) in rejected {
        match lint(&prog, "rejected", &xsd, &instance) {
            Ok(()) => panic!("{what}: the schema accepted it"),
            Err(message) => assert!(
                !message.contains("Schemas parser error") && !message.contains("failed to load"),
                "{what}: xmllint failed on the schema and not on the document:\n{message}"
            ),
        }
    }
}
