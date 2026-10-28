// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One walk, two backends, and the difference between them.
//!
//! `emit/encode.rs` decides which member is written, in what order,
//! under which header and with which length code. `emit/c.rs` and
//! `emit/java.rs` decide how that is spelled. This is the test that the
//! split is where it is claimed to be: the same IDL through both, and
//! every difference in the output is a spelling and not a decision.
//!
//! What this does *not* check is the octets. Those come out of
//! `runtime/c` and `runtime/java` when the generated code is compiled
//! and run, and the two runtimes read DDS-XTypes 1.3 §7.4
//! independently. A Rust test asserting bytes here would be asserting
//! the layout plan a second time and would pass whether or not either
//! runtime agrees.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_rules::emit::c::C;
use zerodds_lang_rules::emit::encode::{CodecOptions, codec_from};
use zerodds_lang_rules::emit::java::Java;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn root(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name.as_str() == name)
        .map(|(id, _)| id)
        .expect("the type is in the model")
}

/// The C column's encoder, rendered.
fn as_c(idl: &str, type_name: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    let decls = codec_from::<C>(&ast, root(&ast, type_name), key, CodecOptions::default())
        .expect("the C encoder");
    zerodds_lang_render::c::file(&zerodds_lang_cst::c::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        guard: None,
        includes: Vec::new(),
        decls,
    })
}

/// The Java column's encoder, rendered.
fn as_java(idl: &str, type_name: &str, encoding: Encoding) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(encoding, Endianness::Little);
    let members = codec_from::<Java>(&ast, root(&ast, type_name), key, CodecOptions::default())
        .expect("the Java encoder");
    zerodds_lang_render::java::type_decl(&zerodds_lang_cst::java::TypeDecl::Class(
        zerodds_lang_cst::java::Class {
            doc: zerodds_lang_cst::common::Doc::none(),
            annotations: Vec::new(),
            modifiers: vec![zerodds_lang_cst::java::Modifier::Public],
            name: zerodds_lang_cst::common::Ident::new("Codec"),
            generics: Vec::new(),
            extends: None,
            implements: Vec::new(),
            members,
        },
    ))
}

const PRIMITIVES: &str = "\
@final struct Sample {
    boolean flag;
    long counter;
    unsigned long long wide;
    double scale;
};";

#[test]
fn both_backends_write_the_same_members_in_the_same_order() {
    let c = as_c(PRIMITIVES, "Sample", Encoding::PlainCdr2);
    let java = as_java(PRIMITIVES, "Sample", Encoding::PlainCdr2);

    // The order is the walk's and has to survive both spellings.
    let order = ["bool", "i32", "u64", "f64"];
    let position = |text: &str, of: &str| text.find(of).unwrap_or_else(|| panic!("{of} in {text}"));

    let mut last = 0;
    for op in order {
        let at = position(&c, &format!("zdw_write_{op}("));
        assert!(at > last, "the C column writes {op} out of order");
        last = at;
    }
    let mut last = 0;
    for op in order {
        let camel = match op {
            "bool" => "writeBool".to_string(),
            other => format!("write{}{}", &other[..1].to_uppercase(), &other[1..]),
        };
        let at = position(&java, &format!("{camel}("));
        assert!(at > last, "the Java column writes {op} out of order");
        last = at;
    }
}

#[test]
fn the_difference_between_the_two_is_spelling() {
    // the assertions read the IDL-to-Java shape
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        the_difference_between_the_two_is_spelling_omg,
    );
}

fn the_difference_between_the_two_is_spelling_omg() {
    let c = as_c(PRIMITIVES, "Sample", Encoding::PlainCdr2);
    let java = as_java(PRIMITIVES, "Sample", Encoding::PlainCdr2);

    // A primitive gets its own codec in both columns — the walk's
    // decision — and the two spell the handoff differently. C takes the
    // member's address and dereferences inside; Java passes the value.
    assert!(c.contains("zd_enc_p_long(w, &v->counter)"), "{c}");
    assert!(java.contains("zd_enc_p_long(w, v.counter)"), "{java}");
    assert!(c.contains("zdw_write_i32(w, *v)"), "{c}");
    assert!(java.contains("w.writeI32(v)"), "{java}");

    // The runtime's handle type is the backend's name for it. A Java
    // column that inherited C's would declare a type javac has never
    // heard of, and the whole file would fail to compile at once —
    // which is the failure to prefer, but still a failure.
    assert!(c.contains("zdw_dheader d ="), "{c}");
    assert!(java.contains("Writer.DHeader d ="), "{java}");

    // C needs prototypes ahead of the bodies so declaration order never
    // matters; a Java method may call one declared below it, so its
    // encoder appears once rather than twice.
    let declared = c.matches("static void zd_enc_Sample").count();
    assert_eq!(declared, 2, "the C column needs a prototype and a body");
    let defined = java.matches("void zd_enc_Sample").count();
    assert_eq!(defined, 1, "the Java column needs only the body");
}

/// A `sequence` is the case where the two backends differ most and the
/// walk not at all: the same counted loop over the same length, asked
/// for in two places the languages keep it.
#[test]
fn a_sequence_is_one_loop_and_two_ways_of_asking_its_length() {
    const IDL: &str = "@final struct Bag { sequence<long> items; };";
    let c = as_c(IDL, "Bag", Encoding::PlainCdr2);
    let java = as_java(IDL, "Bag", Encoding::PlainCdr2);

    assert!(c.contains("for (uint32_t i = 0;"), "{c}");
    assert!(java.contains("for (int i = 0;"), "{java}");

    // Both write the count before the elements — §7.4.3.5.3 — and both
    // step by one written out rather than by an increment node.
    assert!(c.contains("i = i + 1"), "{c}");
    assert!(java.contains("i = i + 1"), "{java}");
}

/// The `@mutable` case, where the walk emits a header per member and
/// closes it afterwards. If the seam leaked, this is where: the header
/// handle is a local whose type each backend names for itself.
#[test]
fn a_mutable_aggregate_opens_and_closes_a_header_in_both() {
    const IDL: &str = "\
@mutable struct Tagged {
    @id(7) long first;
    @id(11) long second;
};";
    let c = as_c(IDL, "Tagged", Encoding::PlCdr2);
    let java = as_java(IDL, "Tagged", Encoding::PlCdr2);

    for (column, source) in [("C", &c), ("Java", &java)] {
        let opened =
            source.matches("riteEmheader").count() + source.matches("rite_emheader").count();
        let closed =
            source.matches("inishEmheader").count() + source.matches("inish_emheader").count();
        assert_eq!(
            opened, closed,
            "the {column} column opens {opened} member headers and closes {closed}"
        );
        assert_eq!(opened, 2, "the {column} column framed the wrong number");
    }
}

/// Java has no unsigned types, and the width a codec's local is declared
/// at is where that stops being a curiosity.
///
/// `adapter/java.rs` maps a member's `octet` to `int`, and
/// `runtime/java/Writer.java` takes `writeU8(int)` because it means a
/// number in [0, 255]. A backend that declared the codec's own local
/// `byte` would hold 200 as −56 and hand the runtime a value already
/// lost — with nothing on the wire yet to compare against, and no test
/// anywhere that reads the octets, because the Java column has no probe.
///
/// So this is asserted against the runtime's signature rather than
/// against a byte.
#[test]
fn an_unsigned_local_is_wide_enough_in_java() {
    use zerodds_lang_cst::java::Primitive;
    use zerodds_lang_rules::emit::{CodecTarget, Scalar};

    let widths = [
        (Scalar::UInt8, Primitive::Int),
        (Scalar::UInt16, Primitive::Int),
        (Scalar::UInt32, Primitive::Long),
        // The one exception: no wider type exists, so the long holds a
        // pattern and writeU64(-1L) writes ffffffffffffffff.
        (Scalar::UInt64, Primitive::Long),
    ];
    for (scalar, expected) in widths {
        assert_eq!(
            Java::scalar(scalar),
            zerodds_lang_cst::java::Type::Primitive(expected),
            "{scalar:?} does not survive its own range in Java"
        );
    }
}

/// The backend has to spell the shapes the materialiser declares, not
/// shapes of its own.
///
/// `adapter/java.rs` makes a `sequence<T>` and an IDL array both a Java
/// `T[]`, and a `map<K,V>` an array of a generated pair rather than a
/// `java.util.Map` — because the entry order is a measured axis and
/// every `java.util.Map` decides it. A codec asking a list for `.size()`
/// or allocating an `ArrayList` would generate source against types that
/// are never emitted, and nothing would say so until javac ran.
#[test]
fn the_java_backend_spells_the_shapes_the_materialiser_declares() {
    // the assertions read the IDL-to-Java shape
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        the_java_backend_spells_the_shapes_the_materialiser_declares_omg,
    );
}

fn the_java_backend_spells_the_shapes_the_materialiser_declares_omg() {
    use zerodds_lang_cst::java::Type;
    use zerodds_lang_rules::emit::CodecTarget;

    let counted = <Java as CodecTarget>::count_of("v", None);
    let rendered = format!("{counted:?}");
    assert!(
        rendered.contains("length"),
        "a sequence's count is an array's length field, not a list's size(): {rendered}"
    );

    let room = <Java as CodecTarget>::make_room(
        "v",
        Type::Primitive(zerodds_lang_cst::java::Primitive::Int),
        <Java as CodecTarget>::int(3),
        true,
        Vec::new(),
    );
    let rendered = format!("{room:?}");
    assert!(
        rendered.contains("NewArray"),
        "room for a sequence is an array allocation: {rendered}"
    );

    // The bit-set holder is the materialiser's own function, so the two
    // cannot part: twelve bits is an int in both or in neither.
    assert_eq!(
        <Java as CodecTarget>::unsigned_holder(12),
        zerodds_lang_rules::adapter::java::unsigned_holder(12)
    );
}

/// Which unary operators the shared walk ever hands a backend.
///
/// One: `!`. Both backends answer it by name and then have a
/// catch-all that writes `0 <op> value`, which is right for a negation
/// — `0 - a` is `-a` — and is not an expression for anything else. The
/// C++ backend had no other form at all until `cpp::Expr::Unary`
/// existed, so `~` went through that catch-all and produced
/// `zd_bits & 0 ~ UINT64_C(32897)`; twenty-one corpus cases stopped
/// compiling and the round trip reported them as unread. The walk no
/// longer asks for `~`: a bitmask read keeps the declared bits with a
/// plain `&`.
///
/// The catch-all is unreachable today and reads like a general answer.
/// This counts the call sites in the walk so that a second operator has
/// to arrive with a decision rather than with a rendering: adding
/// `T::unary("%", …)` turns this red, and the fix is to give each
/// backend a form for it.
#[test]
fn the_walk_asks_for_one_unary_operator_and_no_others() {
    let mut found: Vec<String> = Vec::new();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/emit");
    for entry in std::fs::read_dir(&source).expect("the emit directory") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("an emit source");
        // The walk's own calls, not a backend's implementation of the
        // seam: `T::unary` and `Self::unary` are how a walk asks.
        for prefix in ["T::unary(\"", "Self::unary(\""] {
            let mut rest = text.as_str();
            while let Some(at) = rest.find(prefix) {
                rest = &rest[at + prefix.len()..];
                let operator = rest.split('"').next().unwrap_or("").to_string();
                if !found.contains(&operator) {
                    found.push(operator);
                }
            }
        }
    }
    found.sort();
    assert_eq!(
        found,
        vec!["!".to_string()],
        "the walk asks for a unary operator no backend was written for; \
         give each backend a form for it rather than letting the \
         catch-all render `0 <op> value`"
    );
}
