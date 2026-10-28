// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One declaration, written twice: once in DDS-XML and once in the IDL
//! it corresponds to. Both go through their own front end, and the two
//! models have to agree on every `TypeIdentifier`.
//!
//! This is the strongest check available for an XML loader, and it is
//! stronger than any assertion about the tree: a `TypeIdentifier` is the
//! MD5 of the serialised `CompleteTypeObject`, so it moves if a bound, a
//! member id, an extensibility, a key flag, a try-construct behaviour, a
//! label value, an array dimension or a member order differs by one bit.
//! An XML reading that is nearly right does not pass.
//!
//! The fixtures are written here rather than taken from the OMG
//! `dds-xtypes` suite: that repository's `LICENSE.md` permits use for
//! interoperability testing and debugging but not redistribution, and
//! this file is in a repository that is mirrored publicly. What they
//! carry is the same set of constructs — every primitive including
//! `int8`, `uint8` and `float128`, bounded and unbounded strings, arrays
//! of one and two dimensions, sequences, enumerations with and without
//! explicit values, bitmasks, unions with hexadecimal, enumerated and
//! `default` labels, `@key`, `@id`, `@hashid`, `@autoid(HASH)`,
//! `@must_understand` and all three `@try_construct` behaviours.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_ast::Ast;
use zerodds_types::from_ast::{Form, type_identifier};

/// Every named type in a model, by its scoped name, with the identifier
/// of its `CompleteTypeObject`.
fn identifiers(ast: &Ast) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            // Anonymous collection nodes have no name to key on; they
            // are covered through the members that hold them.
            continue;
        }
        let Some(scoped) = ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id)) else {
            continue;
        };
        let name = scoped.segments().join("::");
        if let Ok(identifier) = type_identifier(ast, id, Form::Complete) {
            out.insert(name, format!("{identifier:?}"));
        }
    }
    out
}

fn compose_xml(xml: &str) -> Ast {
    let composed = zerodds_xml_compose::compose_source("fixture.xml", xml);
    assert!(
        !composed.failed(),
        "the XML did not compose: {:#?}",
        composed.diagnostics
    );
    composed.ast
}

fn compose_idl(idl: &str) -> Ast {
    let composed = zerodds_idl_compose::compose_source(
        "fixture.idl",
        idl,
        &zerodds_idl_parse::MemoryLoader::new(),
    );
    assert!(
        !composed.failed(),
        "the IDL did not compose: {:#?}",
        composed.diagnostics
    );
    composed.ast
}

/// The whole check: both front ends, then a name-by-name comparison of
/// the identifiers.
fn assert_same_model(xml: &str, idl: &str) {
    let from_xml = identifiers(&compose_xml(xml));
    let from_idl = identifiers(&compose_idl(idl));

    let named: Vec<&String> = from_idl.keys().filter(|k| k.contains("::")).collect();
    assert!(
        !named.is_empty(),
        "the IDL fixture declared no type inside a module"
    );

    for name in named {
        let idl_identifier = from_idl.get(name).expect("in the IDL model");
        let xml_identifier = from_xml.get(name).unwrap_or_else(|| {
            panic!(
                "`{name}` is in the IDL model and not in the XML one; the XML model has {:?}",
                from_xml.keys().collect::<Vec<_>>()
            )
        });
        assert_eq!(
            xml_identifier, idl_identifier,
            "`{name}` has a different TypeIdentifier from XML than from IDL"
        );
    }
}

/// Every primitive the XTypes vocabulary spells differently from IDL,
/// over all three extensibilities. `int8` and `uint8` are the two the
/// `zerodds-xml` primitive table has no entry for at all, which is one
/// of the reasons this loader has its own.
#[test]
fn every_primitive_reaches_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="P">
    <struct name="all_final" extensibility="final">
      <member name="a" type="uint8"/>
      <member name="b" type="uint16"/>
      <member name="c" type="uint32"/>
      <member name="d" type="uint64"/>
      <member name="e" type="int8"/>
      <member name="f" type="int16"/>
      <member name="g" type="int32"/>
      <member name="h" type="int64"/>
      <member name="i" type="boolean"/>
      <member name="j" type="float32"/>
      <member name="k" type="float64"/>
      <member name="l" type="float128"/>
      <member name="m" type="byte"/>
      <member name="n" type="char8"/>
    </struct>
    <struct name="all_appendable" extensibility="appendable">
      <member name="a" type="int32"/>
    </struct>
    <struct name="all_mutable" extensibility="mutable">
      <member name="a" type="int32"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r"
module P {
  @final struct all_final {
    uint8 a; uint16 b; uint32 c; uint64 d;
    int8 e; int16 f; int32 g; int64 h;
    boolean i; float j; double k; long double l;
    octet m; char n;
  };
  @appendable struct all_appendable { int32 a; };
  @mutable struct all_mutable { int32 a; };
};
";
    assert_same_model(xml, idl);
}

/// Bounded and unbounded strings and wstrings. `stringMaxLength` is the
/// bound of the *element*, which is what makes `string<10> x[10]` and
/// `sequence<string<10>,10>` expressible with one attribute each.
#[test]
fn string_bounds_reach_the_same_type_from_both_front_ends() {
    // The struct is `Bounds` and not `s`: identifier comparison in IDL
    // is case-insensitive (§7.2.3), so a `struct s` inside `module S`
    // redefines the name of its own immediate scope and §7.5.2 refuses
    // it. The bounds this test is about are unaffected by the name.
    let xml = r#"
<dds><types>
  <module name="S">
    <struct name="Bounds" extensibility="final">
      <member name="a" type="string"/>
      <member name="b" type="string" stringMaxLength="10"/>
      <member name="c" type="wstring"/>
      <member name="d" type="wstring" stringMaxLength="20"/>
    </struct>
  </module>
</types></dds>
"#;
    // `Bounds` and not `s`: IDL compares identifiers without regard to
    // case (§7.2.3), so a `struct s` inside `module S` would redeclare
    // the scope's own name in its immediate scope, which IDL 4.2 §7.5.2
    // does not admit. The bounds are what this compares; the name is
    // free.
    let idl = r"
module S {
  @final struct Bounds {
    string a;
    string<10> b;
    wstring c;
    wstring<20> d;
  };
};
";
    assert_same_model(xml, idl);
}

/// Arrays of one and two dimensions, of a bounded string, of an
/// enumeration and of a struct. The two-dimensional case is the one that
/// distinguishes `dims = [10, 2]` from `[2, 10]`, and the identifier
/// moves if they are swapped.
#[test]
fn arrays_reach_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="A">
    <enum name="E" bitBound="32" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1"/>
    </enum>
    <struct name="inner" extensibility="final">
      <member name="x" type="uint32" arrayDimensions="20"/>
    </struct>
    <struct name="outer" extensibility="final">
      <member name="a" type="int32" arrayDimensions="10"/>
      <member name="m2" type="int32" arrayDimensions="10,2"/>
      <member name="c" type="string" stringMaxLength="10" arrayDimensions="10"/>
      <member name="m4" type="nonBasic" nonBasicTypeName="E" arrayDimensions="10"/>
      <member name="m5" type="nonBasic" nonBasicTypeName="inner" arrayDimensions="3"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r"
module A {
  @appendable enum E { @value(0) V0, @value(1) V1 };
  @final struct inner { uint32 x[20]; };
  @final struct outer {
    int32 a[10];
    int32 m2[10][2];
    string<10> c[10];
    E m4[10];
    inner m5[3];
  };
};
";
    assert_same_model(xml, idl);
}

/// Sequences, bounded and unbounded, of a primitive, a bounded string, an
/// enumeration and a struct. `sequenceMaxLength="-1"` is DDS-XML's
/// `LENGTH_UNLIMITED`, which has to reach the same unbounded sequence
/// that `sequence<int32>` does.
#[test]
fn sequences_reach_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="Q">
    <enum name="E" bitBound="32" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1"/>
    </enum>
    <struct name="inner" extensibility="appendable">
      <member name="x" type="uint32" sequenceMaxLength="20"/>
    </struct>
    <struct name="outer" extensibility="final">
      <member name="a" type="int32" sequenceMaxLength="-1"/>
      <member name="m2" type="int32" sequenceMaxLength="10"/>
      <member name="c" type="string" stringMaxLength="10" sequenceMaxLength="10"/>
      <member name="m4" type="nonBasic" nonBasicTypeName="E" sequenceMaxLength="10"/>
      <member name="m5" type="nonBasic" nonBasicTypeName="inner" sequenceMaxLength="10"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r"
module Q {
  @appendable enum E { @value(0) V0, @value(1) V1 };
  @appendable struct inner { sequence<uint32,20> x; };
  @final struct outer {
    sequence<int32> a;
    sequence<int32,10> m2;
    sequence<string<10>,10> c;
    sequence<E,10> m4;
    sequence<inner,10> m5;
  };
};
";
    assert_same_model(xml, idl);
}

/// Member ids: declared, derived from the previous one, hashed by an
/// aggregate-wide `autoid`, and hashed from a name the member names
/// itself. Every one of the four is a different number and every one of
/// them is in the identifier.
#[test]
fn member_ids_reach_the_same_numbers_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="I">
    <struct name="declared" extensibility="mutable">
      <member name="x1" type="int32" id="1"/>
      <member name="x3" type="int32" id="3"/>
      <member name="x2" type="int32" id="2"/>
    </struct>
    <struct name="derived" extensibility="mutable">
      <member name="x1" type="int32"/>
      <member name="x3" type="int32"/>
      <member name="x2" type="int32"/>
    </struct>
    <struct name="auto_hash" extensibility="final" autoid="hash">
      <member name="x1" type="int32"/>
    </struct>
    <struct name="named_hash" extensibility="final" autoid="hash">
      <member name="x2" type="int32" hashid="x1"/>
    </struct>
    <struct name="mixed" extensibility="mutable">
      <member name="a" type="int32" id="7"/>
      <member name="b" type="int32"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r#"
module I {
  @mutable struct declared { @id(1) int32 x1; @id(3) int32 x3; @id(2) int32 x2; };
  @mutable struct derived { int32 x1; int32 x3; int32 x2; };
  @final @autoid(HASH) struct auto_hash { int32 x1; };
  @final @autoid(HASH) struct named_hash { @hashid("x1") int32 x2; };
  @mutable struct mixed { @id(7) int32 a; int32 b; };
};
"#;
    assert_same_model(xml, idl);
}

/// `key="true"` on a member of every kind the suite keys: a primitive, a
/// bounded string, an enumeration, a struct and a bounded sequence. The
/// key flag is a bit in `StructMemberFlag` and moves the identifier.
#[test]
fn keys_reach_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="K">
    <enum name="E" bitBound="32" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1"/>
    </enum>
    <struct name="inner" extensibility="appendable">
      <member name="k" key="true" type="string" stringMaxLength="10"/>
      <member name="v" type="string" stringMaxLength="20"/>
    </struct>
    <struct name="outer" extensibility="appendable">
      <member name="a" type="int32"/>
      <member name="m2" key="true" type="int32"/>
      <member name="c" key="true" type="string" stringMaxLength="10"/>
      <member name="m4" key="true" type="nonBasic" nonBasicTypeName="E"/>
      <member name="m5" key="true" type="nonBasic" nonBasicTypeName="inner"/>
      <member name="f" key="true" type="int32" sequenceMaxLength="10"/>
      <member name="g" mustUnderstand="true" type="int32"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r"
module K {
  @appendable enum E { @value(0) V0, @value(1) V1 };
  @appendable struct inner { @key string<10> k; string<20> v; };
  @appendable struct outer {
    int32 a;
    @key int32 m2;
    @key string<10> c;
    @key E m4;
    @key inner m5;
    @key sequence<int32,10> f;
    @must_understand int32 g;
  };
};
";
    assert_same_model(xml, idl);
}

/// Unions: hexadecimal labels, a `default` label, an enumerated
/// discriminator whose labels are enumerator names, and a case carrying
/// two labels. A label read as the wrong number moves the identifier.
///
/// The suite's fourth union shape — a **bitmask** discriminator — is not
/// here, because it has no IDL to compare against: OMG IDL 4.2
/// §7.4.1.4.4.3 rule (50) writes `<switch_type_spec>` as
/// `<integer_type> | <char_type> | <boolean_type> | <scoped_name>` and
/// admits no bitmask, so `zerodds-idl-compose` refuses it by name. It is
/// checked in `tests/loader.rs` instead, as the XML-only construct it
/// is.
#[test]
fn unions_reach_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="U">
    <enum name="E" bitBound="32" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1"/>
      <enumerator name="V2" value="2"/>
    </enum>
    <union name="by_hex" extensibility="final">
      <discriminator type="uint8"/>
      <case><caseDiscriminator value="0x01"/><member name="x1" type="uint8"/></case>
      <case><caseDiscriminator value="0x0e"/><member name="x2" type="char8"/></case>
    </union>
    <union name="with_default" extensibility="appendable">
      <discriminator type="uint32"/>
      <case><caseDiscriminator value="1"/><member name="x1" type="int16"/></case>
      <case><caseDiscriminator value="default"/><member name="xd" type="uint32"/></case>
      <case><caseDiscriminator value="3"/><member name="x3" type="int32"/></case>
    </union>
    <union name="by_enum" extensibility="appendable">
      <discriminator type="nonBasic" nonBasicTypeName="E"/>
      <case><caseDiscriminator value="V0"/><member name="x0" type="int32"/></case>
      <case><caseDiscriminator value="V1"/><member name="x1" type="int32"/></case>
      <case><caseDiscriminator value="V2"/><member name="x2" type="int32"/></case>
    </union>
    <union name="two_labels" extensibility="mutable">
      <discriminator type="uint32"/>
      <case>
        <caseDiscriminator value="1"/>
        <caseDiscriminator value="2"/>
        <member name="x1" type="int16" id="4"/>
      </case>
    </union>
  </module>
</types></dds>
"#;
    let idl = r"
module U {
  @appendable enum E { @value(0) V0, @value(1) V1, @value(2) V2 };
  @final union by_hex switch (uint8) {
    case 0x01: uint8 x1;
    case 0x0e: char x2;
  };
  @appendable union with_default switch (uint32) {
    case 1: int16 x1;
    default: uint32 xd;
    case 3: int32 x3;
  };
  @appendable union by_enum switch (E) {
    case V0: int32 x0;
    case V1: int32 x1;
    case V2: int32 x2;
  };
  @mutable union two_labels switch (uint32) {
    case 1:
    case 2: @id(4) int16 x1;
  };
};
";
    assert_same_model(xml, idl);
}

/// All three `@try_construct` behaviours, on a struct member, on a union
/// case and on a union discriminator, plus `@default_literal` on an
/// enumerator. Each behaviour is two bits of the member flags and moves
/// the identifier.
#[test]
fn try_construct_reaches_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="T">
    <enum name="E" bitBound="32" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1" defaultLiteral="true"/>
      <enumerator name="V2" value="2"/>
    </enum>
    <struct name="on_members" extensibility="mutable">
      <member name="a" type="int32" sequenceMaxLength="10" tryConstruct="trim"/>
      <member name="m2" type="string" stringMaxLength="10" tryConstruct="discard"/>
      <member name="m3" type="nonBasic" nonBasicTypeName="E" tryConstruct="use_default"/>
    </struct>
    <union name="on_cases" extensibility="appendable">
      <discriminator type="uint32"/>
      <case><caseDiscriminator value="1"/><member name="x1" type="string" stringMaxLength="10" tryConstruct="trim"/></case>
      <case><caseDiscriminator value="2"/><member name="x2" type="int32" sequenceMaxLength="10" tryConstruct="discard"/></case>
    </union>
    <union name="on_discriminator" extensibility="appendable">
      <discriminator type="nonBasic" nonBasicTypeName="E" tryConstruct="use_default"/>
      <case><caseDiscriminator value="V0"/><member name="x0" type="int32"/></case>
    </union>
  </module>
</types></dds>
"#;
    let idl = r"
module T {
  @appendable enum E { @value(0) V0, @default_literal @value(1) V1, @value(2) V2 };
  @mutable struct on_members {
    @try_construct(TRIM) sequence<int32,10> a;
    @try_construct(DISCARD) string<10> m2;
    @try_construct(USE_DEFAULT) E m3;
  };
  @appendable union on_cases switch (uint32) {
    case 1: @try_construct(TRIM) string<10> x1;
    case 2: @try_construct(DISCARD) sequence<int32,10> x2;
  };
  @appendable union on_discriminator switch (@try_construct(USE_DEFAULT) E) {
    case V0: int32 x0;
  };
};
";
    assert_same_model(xml, idl);
}

/// A member whose type is declared later in the document, and one
/// declared in an enclosing module. Both are resolved by the declare
/// pass rather than left pending, which is what an anonymous collection
/// at the use site needs: its node sits in the root module and no
/// enclosing-scope walk would reach the writing member's scope.
#[test]
fn forward_and_enclosing_scope_references_reach_the_same_type() {
    let xml = r#"
<dds><types>
  <module name="Outer">
    <struct name="Shared" extensibility="final">
      <member name="v" type="int32"/>
    </struct>
    <module name="Inner">
      <struct name="uses_later" extensibility="final">
        <member name="a" type="nonBasic" nonBasicTypeName="Later"/>
        <member name="b" type="nonBasic" nonBasicTypeName="Shared" sequenceMaxLength="4"/>
        <member name="c" type="nonBasic" nonBasicTypeName="Outer::Shared"/>
      </struct>
      <struct name="Later" extensibility="final">
        <member name="v" type="int32"/>
      </struct>
    </module>
  </module>
</types></dds>
"#;
    let idl = r"
module Outer {
  @final struct Shared { int32 v; };
  module Inner {
    @final struct uses_later {
      Later a;
      sequence<Outer::Shared,4> b;
      Outer::Shared c;
    };
    @final struct Later { int32 v; };
  };
};
";
    assert_same_model(xml, idl);
}

/// An enumeration whose `bitBound` is not 32, which is the case a
/// default would swallow.
#[test]
fn a_narrow_bit_bound_reaches_the_same_type_from_both_front_ends() {
    let xml = r#"
<dds><types>
  <module name="N">
    <enum name="E8" bitBound="8" extensibility="appendable">
      <enumerator name="V0" value="0"/>
      <enumerator name="V1" value="1"/>
    </enum>
    <bitmask name="B32" bitBound="32">
      <bit_value name="F1" position="0"/>
      <bit_value name="F2" position="3"/>
    </bitmask>
    <struct name="holds" extensibility="final">
      <member name="a" type="nonBasic" nonBasicTypeName="E8"/>
      <member name="m2" type="nonBasic" nonBasicTypeName="B32"/>
    </struct>
  </module>
</types></dds>
"#;
    let idl = r"
module N {
  @appendable @bit_bound(8) enum E8 { @value(0) V0, @value(1) V1 };
  @bit_bound(32) bitmask B32 { @position(0) F1, @position(3) F2 };
  @final struct holds { E8 a; B32 m2; };
};
";
    assert_same_model(xml, idl);
}
