// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared walk, through the D backend.
//!
//! What this proves is not that the octets are right — that is the
//! corpus sweep's, `tools/wireprobe/wireprobe-d-corpus`, against the
//! Rust column's recorded octets — but that the walk reaches the end
//! for the shapes the corpus has and that what comes out is D rather
//! than C with different punctuation.
//!
//! D adds three of its own. An IDL `long` is a D `int`, so a codec that
//! spelled the width by the IDL word would double every 32-bit member;
//! `@optional` is `Nullable!(T)` while `@optional @external` is a bare
//! `T*`, and the four external seams keep the two apart; and a union's
//! branches live in an anonymous `union`, which assignments reach and
//! struct literals do not. Every shape below therefore ends in
//! `ldc2 -o-` — parse, semantic, no object file — against the real
//! `runtime/d`, where a toolchain is on the path.
// `print_stderr`: the one `eprintln!` says that no D compiler was on
// the path and the build was therefore not checked. A test that quietly
// checks nothing is worse than one that says it did not run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;
use zerodds_ast::Ast;
use zerodds_lang_rules::default_d::d_defaulters_called_by;
use zerodds_lang_rules::emit::d::D;
use zerodds_lang_rules::{ModelGraph, apply};

/// One IDL source, as the whole D module its codec needs.
fn module(idl: &str, type_name: &str, writing: bool) -> String {
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
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    let model = ModelGraph::of(&ast);
    let applied =
        apply(&model, &zerodds_lang_rules::rule::d::catalogue()).expect("the rules apply");
    let items =
        zerodds_lang_rules::adapter::d::d_items(&ast, &applied).expect("the types materialise");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let codec = if writing {
        zerodds_lang_rules::emit::encode::codec_from::<D>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        let decoders = zerodds_lang_rules::emit::decode::decoder_from::<D>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end");
        let mut all = d_defaulters_called_by(&ast, &decoders).expect("Table 9 answers");
        all.extend(decoders);
        all
    };
    let mut imports = vec![zerodds_lang_cst::d::Import::module(&["zdwire"])];
    if items.iter().any(uses_nullable) {
        imports.push(zerodds_lang_cst::d::Import::selective(
            &["std", "typecons"],
            "Nullable",
        ));
    }
    let mut file = zerodds_lang_cst::d::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        module: zerodds_lang_cst::common::Path::new(&["zdtypes"]),
        imports,
        decls: items,
    };
    file.decls.extend(codec);
    zerodds_lang_render::d::file(&file)
}

/// Whether a declaration mentions `Nullable!(T)` anywhere.
fn uses_nullable(decl: &zerodds_lang_cst::d::Decl) -> bool {
    use zerodds_lang_cst::d;
    fn in_type(type_: &d::Type) -> bool {
        match type_ {
            d::Type::Nullable(_) => true,
            d::Type::Slice(inner) | d::Type::Pointer(inner) => in_type(inner),
            d::Type::Array { element, .. } => in_type(element),
            d::Type::Primitive(_) | d::Type::Named(_) => false,
        }
    }
    match decl {
        d::Decl::Struct(node) => node.members.iter().any(|member| match member {
            d::Member::Field(field) => in_type(&field.type_),
            d::Member::Union(fields) => fields.iter().any(|f| in_type(&f.type_)),
            d::Member::Verbatim(_) => false,
        }),
        d::Decl::Alias(node) => in_type(&node.type_),
        d::Decl::Const(node) => node.type_.as_ref().is_some_and(in_type),
        d::Decl::Interface(_)
        | d::Decl::Func(_)
        | d::Decl::Enum(_)
        | d::Decl::Comment(_)
        | d::Decl::Verbatim(_) => false,
    }
}

fn encoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, true)
}

/// The write walk's own answer for one source, refusal included.
fn try_encoder(
    idl: &str,
    type_name: &str,
) -> Result<Vec<zerodds_lang_cst::d::Decl>, zerodds_lang_rules::encode::EncodeError> {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(parsed.diagnostics().is_empty(), "the source parses");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "the source lowers");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    zerodds_lang_rules::emit::encode::codec_from::<D>(&ast, root, key, Default::default())
}

fn decoder(idl: &str, type_name: &str) -> String {
    module(idl, type_name, false)
}

/// The D compiler on the path, if any, with its analyse-only flags.
fn d_compiler() -> Option<(&'static str, &'static [&'static str])> {
    const COMPILERS: &[(&str, &[&str])] = &[
        ("ldc2", &["-o-"]),
        ("gdc", &["-fsyntax-only"]),
        ("dmd", &["-o-"]),
    ];
    for (compiler, flags) in COMPILERS {
        let found = Command::new(compiler)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success());
        if found {
            return Some((compiler, flags));
        }
    }
    None
}

/// Hand the module to the D compiler, against the real `runtime/d`.
fn compiles(text: &str) {
    let Some((compiler, flags)) = d_compiler() else {
        eprintln!("no D compiler on the path: the generated module was not compiled");
        return;
    };
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../runtime/d/zdwire.d")
        .canonicalize()
        .expect("runtime/d");
    let dir =
        std::env::temp_dir().join(format!("zd-d-codec-{}-{}", std::process::id(), text.len()));
    std::fs::create_dir_all(&dir).expect("the build directory");
    let file = dir.join("zd_types.d");
    std::fs::write(&file, text).expect("the module");
    let output = Command::new(compiler)
        .args(flags)
        .arg(&file)
        .arg(&runtime)
        .current_dir(&dir)
        .output()
        .expect("the D compiler runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "the generated module is not D:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn a_structure_of_primitives_becomes_a_d_encoder() {
    let text = encoder(
        "@final struct Pose { long x; double y; boolean active; };",
        "Pose",
    );
    for expected in [
        "void zd_enc_",
        "w.WriteI32",
        "w.WriteF64",
        "w.WriteBool",
        "v.x",
    ] {
        assert!(text.contains(expected), "{expected} is missing:\n{text}");
    }
    for foreign in ["zdw_", "->", "write_i32", "uint32(", "func "] {
        assert!(!text.contains(foreign), "{foreign} reached D:\n{text}");
    }
    compiles(&text);
}

#[test]
fn the_read_direction_opens_with_table_9_and_returns_the_sample() {
    let text = decoder("@final struct S { long a; string b; };", "S");
    assert!(
        text.contains("auto v = zd_def_S()"),
        "the decoder does not start from Table 9:\n{text}"
    );
    assert!(
        text.contains("return v;"),
        "the sample is not returned:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_optional_member_reads_through_zd_nullable() {
    let text = decoder("@final struct S { @optional long a; };", "S");
    assert!(
        text.contains("zdwire.zdNullable(zd_dec_p_long(r, "),
        "the optional member does not go through zdNullable:\n{text}"
    );
    assert!(
        text.contains("v.a.nullify()"),
        "the absent case does not nullify:\n{text}"
    );
    compiles(&text);
}

#[test]
fn an_external_member_reads_through_zd_ptr_and_writes_the_dereference() {
    let read = decoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        read.contains("zdwire.zdPtr(zd_dec_Inner(r, "),
        "the external member does not go through zdPtr:\n{read}"
    );
    compiles(&read);
    let write = encoder(
        "@final struct Inner { long a; }; @final struct S { @external Inner i; };",
        "S",
    );
    assert!(
        write.contains("*v.i"),
        "the external member is not dereferenced on the way out:\n{write}"
    );
    compiles(&write);
}

/// An `@external` union branch is the same `T*` a structure's
/// `@external` member is, and its codec routes through the same two
/// seams: `zdwire.zdPtr` makes the storage on the way in, the
/// dereference hands the value out on the way out. Before the mirror in
/// `idl-lower` and the case seam in the walk, the annotation lowered
/// without a diagnostic and both directions treated the branch as held
/// inline — a silent drop.
#[test]
fn an_external_union_branch_reads_through_zd_ptr_and_writes_the_dereference() {
    let idl = "@final struct Inner { long a; }; \
               @final union U switch (long) { case 1: @external Inner i; default: long other; }; \
               @final struct S { U pick; };";
    let read = decoder(idl, "S");
    assert!(
        read.contains("zdwire.zdPtr(zd_dec_Inner(r, "),
        "the external branch does not go through zdPtr:\n{read}"
    );
    compiles(&read);
    let write = encoder(idl, "S");
    assert!(
        write.contains("*v.i"),
        "the external branch is not dereferenced on the way out:\n{write}"
    );
    compiles(&write);
}

#[test]
fn a_union_assigns_through_the_anonymous_union() {
    let text = decoder(
        "@final union U switch (long) { case 1: long one; default: string other; }; \
         @final struct S { U pick; };",
        "S",
    );
    assert!(
        text.contains("v.one = ") && text.contains("v._d"),
        "the branches are not plain members:\n{text}"
    );
    compiles(&text);
}

/// Every aggregate the corpus has, through both directions, to the end
/// of the walk, each compiled against the real runtime.
#[test]
fn the_shapes_the_corpus_has_reach_the_end_of_both_walks() {
    for (idl, root) in [
        ("@final struct S { long a; };", "S"),
        ("@appendable struct S { long a; string b; };", "S"),
        (
            "@mutable struct S { @id(1) long a; @id(2) sequence<double> b; };",
            "S",
        ),
        ("@final struct S { long a[3]; };", "S"),
        ("@final struct S { long g[2][3]; };", "S"),
        ("@final struct S { sequence<string> a; };", "S"),
        ("@final struct S { map<string, long> a; };", "S"),
        (
            "@final struct S { @optional long a; @optional string b; };",
            "S",
        ),
        (
            "enum Colour { RED, GREEN, BLUE }; @final struct S { Colour tint; };",
            "S",
        ),
        (
            "@bit_bound(16) bitmask Flags { @position(0) A, @position(3) B }; \
             @final struct S { Flags set; };",
            "S",
        ),
        (
            "bitset Packed { bitfield<3> low; bitfield<5> high; }; \
             @final struct S { Packed p; };",
            "S",
        ),
        ("@final struct S { wchar c; wstring w; };", "S"),
        ("@final struct S { long double d; };", "S"),
        ("@final struct S { fixed<5,2> f; };", "S"),
        (
            "@final struct Inner { long a; }; @final struct S { Inner i; };",
            "S",
        ),
        (
            "@final union U switch (long) { case 1: long one; default: string other; }; \
             @final struct S { U pick; };",
            "S",
        ),
        (
            "module a { module b { @final struct S { long x; }; }; };",
            "S",
        ),
        (
            "typedef long Distance; @final struct S { Distance d; };",
            "S",
        ),
    ] {
        let written = encoder(idl, root);
        assert!(!written.is_empty(), "no encoder for {idl}");
        compiles(&written);
        let read = decoder(idl, root);
        assert!(!read.is_empty(), "no decoder for {idl}");
        compiles(&read);
    }
}

/// A self-reference behind `@optional @external` is carried, and the
/// generated codec is the recursion.
///
/// The cycle crosses no sequence and no map, and the planner admits it
/// anyway: what makes a value finite is a terminator, and an
/// `@optional` member is one — the member can be absent. The generated
/// function follows the *value* and not the type, calling itself inside
/// the presence guard, so a value ends where the absence is.
///
/// Measured 2026-08-24 on `matrix/cases/recursive-optional-external`:
/// Fast DDS carries the type and round-trips a two-node tree. The other
/// three implementations refuse, each for a reason of its own and none
/// of them the specification — RTI in its code generator ("external
/// members can't be marked as optional"), Cyclone's `idlc` by not
/// terminating (exit 137, no message), OpenDDS's `tao_idl` by not
/// knowing `mutable`.
#[test]
fn the_recursive_external_shape_is_carried() {
    let idl = "@final struct Node { @optional @external Node next; long v; };";
    let written = encoder(idl, "Node");
    assert!(!written.is_empty(), "no encoder for the recursive shape");
    compiles(&written);
    let read = decoder(idl, "Node");
    assert!(!read.is_empty(), "no decoder for the recursive shape");
    compiles(&read);
}

/// The same shape without `@optional` has no terminator on the cycle
/// and is still refused by name, with the clause in the sentence.
#[test]
fn a_cycle_with_no_terminator_is_refused_by_name() {
    let idl = "@final struct Node { @external Node next; long v; };";
    let error = try_encoder(idl, "Node").expect_err("the planner refuses this shape");
    let text = format!("{error:?}");
    assert!(
        text.contains("closes a type cycle with nothing on it that terminates a value"),
        "the refusal changed its reason: {text}"
    );
}

/// A type handle the model will not hand over is refused by name, not
/// answered with "this type has no D data form".
///
/// `body_of` says `Ok(None)` for a construct Nim cannot hold, and
/// `d_defaulters_over` reads that as a reason to emit no defaulter and
/// walk on. A stale handle used to take the same door: the defaulter the
/// closure asked for was simply not in the batch, while the decoder that
/// calls it by name still was, and the omission surfaced — if at all —
/// as an undeclared identifier from a compiler further down the chain.
#[test]
fn a_type_the_model_does_not_hold_refuses_rather_than_going_missing() {
    let mut ast = one_struct_model();
    let stale = take_and_remove(&mut ast);
    let error = zerodds_lang_rules::default_d::d_defaulters_over(&ast, &[stale])
        .expect_err("a handle the model does not hold has no Table 9 value");
    let text = format!("{error:?}");
    assert!(
        text.contains("a type the model does not hold"),
        "the refusal does not name the case: {text}"
    );
}

/// A model holding `@final struct Point { long x; };`.
fn one_struct_model() -> Ast {
    let parsed = zerodds_idl_parse::parse("@final struct Point { long x; };");
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("the names resolve");
    ast
}

/// `Point`'s handle, with `Point` taken back out of the model, so the
/// handle is one the arena will not answer for.
fn take_and_remove(ast: &mut Ast) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    let id = ast
        .types()
        .find(|(_, node)| node.name.as_str() == "Point")
        .map(|(id, _)| id)
        .expect("Point");
    let any = ast.any_of_type(id).expect("Point's identity");
    ast.remove(any).expect("remove Point");
    assert!(ast.type_(id).is_err(), "the handle is still answered");
    id
}
