// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A layout key that asks for an axis the generated codec's runtime does
//! not carry is refused, not generated in the specification's form.
//!
//! `Dialect::Hdds` selects packed collection elements, a length before
//! each element of a sequence of a non-`@final` type, and a UTF-8
//! `wstring`. `zerodds-wire-codec` writes all three, so does the Rust
//! codec ([`axes_the_rust_codec_carries`]), and so does every one of the
//! twenty generated codecs: each target answers
//! `CodecTarget::carries_hdds_axes` `true`. The refusal is still there for
//! a caller that names no target ([`axes_the_codecs_carry`]) and for a
//! target added without the axes, which inherits the trait's `false`.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey, Purpose};
use zerodds_lang_rules::emit::CodecTarget;
use zerodds_lang_rules::emit::decode::decoder_from;
use zerodds_lang_rules::emit::encode::codec_from;
use zerodds_lang_rules::encode::{
    EncodeError, axes_the_codecs_carry, axes_the_codecs_carry_for, axes_the_rust_codec_carries,
    rust_codec,
};

const IDL: &str = "@appendable struct Sample { sequence<long> a; wstring w; };";

fn key(dialect: Dialect) -> LayoutKey {
    LayoutKey::in_dialect(
        dialect,
        Encoding::PlainCdr2,
        Endianness::Little,
        Purpose::Data,
    )
}

fn model() -> (Ast, Id<TypeNode>) {
    let parsed = zerodds_idl_parse::parse(IDL);
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "hdds.idl").ast;
    ast.resolve_all_by_name().expect("the model resolves");
    let root = ast
        .find_by_scoped_name(&ScopedName::absolute(vec!["Sample".to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("Sample is in the model");
    (ast, root)
}

#[test]
fn the_specification_key_passes_and_the_hdds_key_names_its_three_axes() {
    assert!(axes_the_codecs_carry(key(Dialect::Specification)).is_ok());
    let hdds = key(Dialect::Hdds);
    let Err(EncodeError::NoLayout { detail }) = axes_the_codecs_carry(hdds) else {
        panic!("the HDDS key passed a check it has three axes to fail");
    };
    for axis in [
        hdds.element_padding.as_str(),
        hdds.sequence_element_delimiter.as_str(),
        hdds.wstring_code_units.as_str(),
    ] {
        assert!(detail.contains(axis), "`{axis}` is not named in: {detail}");
    }
}

#[test]
fn the_rust_target_carries_the_hdds_key() {
    assert!(axes_the_rust_codec_carries(key(Dialect::Hdds)).is_ok());
    assert!(axes_the_rust_codec_carries(key(Dialect::Specification)).is_ok());
    let (ast, root) = model();
    assert!(rust_codec(&ast, root, key(Dialect::Specification)).is_ok());
    assert!(
        rust_codec(&ast, root, key(Dialect::Hdds)).is_ok(),
        "the Rust codec carries the HDDS axes"
    );
}

/// One target: it says it carries the axes, and both directions of its
/// generated codec are produced under the HDDS key.
fn carries<T: CodecTarget>(name: &str, ast: &Ast, root: Id<TypeNode>) {
    assert!(
        T::carries_hdds_axes(),
        "{name}: `carries_hdds_axes` is false, so `--dialect hdds` is refused for it"
    );
    assert!(axes_the_codecs_carry_for::<T>(key(Dialect::Hdds)).is_ok());
    let options = Default::default();
    assert!(
        codec_from::<T>(ast, root, key(Dialect::Hdds), options).is_ok(),
        "{name}: no encoder under the HDDS key"
    );
    // Both directions, because the axes are write-side and read-side.
    assert!(
        decoder_from::<T>(ast, root, key(Dialect::Hdds), options).is_ok(),
        "{name}: no decoder under the HDDS key"
    );
    assert!(codec_from::<T>(ast, root, key(Dialect::Specification), options).is_ok());
}

#[test]
fn every_generated_codec_carries_the_hdds_axes() {
    use zerodds_lang_rules::emit::{
        ada::Ada, ada83::Ada83, c::C, cpp::Cpp, csharp::CSharp, d::D, elixir::Elixir,
        fsharp::Fsharp, go::Go, java::Java, julia::Julia, kotlin::Kotlin, lua::Lua, nim::Nim,
        node::Node, ocaml::OCaml, python::Python, swift::Swift, typescript::TypeScript, zig::Zig,
    };
    let (ast, root) = model();
    carries::<Ada>("ada", &ast, root);
    carries::<Ada83>("ada-83", &ast, root);
    carries::<C>("c", &ast, root);
    carries::<Cpp>("cpp", &ast, root);
    carries::<CSharp>("csharp", &ast, root);
    carries::<D>("d", &ast, root);
    carries::<Elixir>("elixir", &ast, root);
    carries::<Fsharp>("fsharp", &ast, root);
    carries::<Go>("go", &ast, root);
    carries::<Java>("java", &ast, root);
    carries::<Julia>("julia", &ast, root);
    carries::<Kotlin>("kotlin", &ast, root);
    carries::<Lua>("lua", &ast, root);
    carries::<Nim>("nim", &ast, root);
    carries::<Node>("node", &ast, root);
    carries::<OCaml>("ocaml", &ast, root);
    carries::<Python>("python", &ast, root);
    carries::<Swift>("swift", &ast, root);
    carries::<TypeScript>("typescript", &ast, root);
    carries::<Zig>("zig", &ast, root);
}
