// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The generated code against the runtime it calls.
//!
//! The generator and the runtime are two crates that never see each
//! other at compile time: the generator writes `w.write_i32(..)` as
//! text, and nothing checks that `Writer` has a `write_i32` until
//! someone compiles the output. These tests check it here.
//!
//! Two halves. The first reads every call the generator emits and looks
//! for it in the runtime's own source. The second writes the bytes a
//! generated encoder would write, by hand against the runtime, and
//! asserts on them — so the sequence the generator produces is one that
//! actually puts the right bytes on the wire.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::key::{Encoding, Endianness as PlanEndianness, LayoutKey};
use zerodds_lang_rules::encode::rust_codec;
use zerodds_wire::{Endianness, Reader, Writer};

/// Generate the codec for one type of `idl`.
fn codec(idl: &str, type_name: &str, encoding: Encoding) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let ast: Ast = lowered.ast;
    let id = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![type_name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{type_name} is not in the model"));

    let items =
        rust_codec(&ast, id, LayoutKey::data(encoding, PlanEndianness::Little)).expect("codec");
    let file = zerodds_lang_cst::rust::File {
        inner_attrs: Vec::new(),
        doc: zerodds_lang_cst::common::Doc::none(),
        uses: Vec::new(),
        items,
    };
    zerodds_lang_render::rust::file(&file)
}

/// Every `w.`, `r.` and `holder.` call the generator emitted.
///
/// The receiver has to start a word. Matching `r.` anywhere took the
/// last two characters of `header.` and reported `extent`,
/// `member_id` and `must_understand` — three fields of
/// `EmHeaderRead` — as runtime methods that do not exist, which is a
/// failure this test would have to be edited around rather than one
/// anybody could fix.
fn calls(generated: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in generated.lines() {
        let bytes = line.as_bytes();
        for receiver in ["w.", "r.", "holder.", "Writer::", "Reader::"] {
            let mut base = 0usize;
            while let Some(at) = line[base..].find(receiver) {
                let at = base + at;
                let starts_a_word =
                    at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
                let after = &line[at + receiver.len()..];
                if starts_a_word {
                    let name: String = after
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        out.insert(name);
                    }
                }
                base = at + receiver.len();
            }
        }
    }
    out
}

/// The methods the runtime actually has.
fn runtime_methods() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in ["../wire/src/writer.rs", "../wire/src/reader.rs"] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
        let source = std::fs::read_to_string(&path).expect("read the runtime");
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("pub fn ")
                .or_else(|| trimmed.strip_prefix("pub const fn "))
            else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            out.insert(name);
        }
    }
    out
}

#[test]
fn every_call_the_generator_emits_exists_in_the_runtime() {
    // The generator writes these as text and nothing checks them until
    // the output is compiled. A name that is close but wrong —
    // `write_int32` for `write_i32` — would pass every test the
    // generator has and fail at the user's build.
    let runtime = runtime_methods();
    let mut missing = BTreeSet::new();

    for (idl, type_name, encoding) in [
        (
            "@final struct A { boolean flag; octet b; char c; short d; long e; long long f; float g; double h; };",
            "A",
            Encoding::PlainCdr2,
        ),
        (
            "@final struct B { string s; sequence<long> q; long arr[3]; };",
            "B",
            Encoding::PlainCdr2,
        ),
        (
            "@appendable struct C { long a; };",
            "C",
            Encoding::DelimitedCdr,
        ),
        (
            "@mutable struct D { @id(1) long a; @id(2) string b; };",
            "D",
            Encoding::PlCdr2,
        ),
        (
            "@final struct E { @key long id; @key string name; };",
            "E",
            Encoding::PlainCdr2,
        ),
    ] {
        let generated = codec(idl, type_name, encoding);
        for call in calls(&generated) {
            if !runtime.contains(&call) {
                missing.insert(call);
            }
        }
    }

    assert!(
        missing.is_empty(),
        "the generator calls methods the runtime does not have: {missing:?}"
    );
}

#[test]
fn the_generator_emits_a_patch_for_every_framing_header_it_opens() {
    // A DHEADER written and never patched leaves a length of zero on the
    // wire, and a reader that trusts it stops before the first member.
    let appendable = codec(
        "@appendable struct S { long a; };",
        "S",
        Encoding::DelimitedCdr,
    );
    assert_eq!(
        appendable.matches("write_dheader").count(),
        1,
        "{appendable}"
    );
    assert_eq!(
        appendable.matches("finish_dheader").count(),
        1,
        "the DHEADER was opened and not patched:\n{appendable}"
    );

    let mutable = codec(
        "@mutable struct S { @id(1) long a; @id(2) long b; };",
        "S",
        Encoding::PlCdr2,
    );
    assert_eq!(
        mutable.matches("write_emheader").count(),
        mutable.matches("finish_emheader").count(),
        "an EMHEADER was opened and not patched:\n{mutable}"
    );
}

// -- the bytes the generated sequence produces ---------------------------

#[test]
fn the_sequence_a_final_struct_generates_writes_the_bytes_the_plan_says() {
    // `@final struct Point { long x; double y; }` plans to 16 bytes:
    // the long at 0, four bytes of padding, the double at 8.
    let mut w = Writer::little_endian();
    w.write_i32(1);
    w.write_f64(2.0);
    let bytes = w.finish();

    assert_eq!(bytes.len(), 16);
    assert_eq!(&bytes[..4], &[1, 0, 0, 0]);
    assert_eq!(&bytes[4..8], &[0, 0, 0, 0], "the padding is not zeroed");

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_i32().expect("x"), 1);
    assert!((r.read_f64().expect("y") - 2.0).abs() < f64::EPSILON);
}

#[test]
fn the_sequence_an_appendable_struct_generates_frames_its_members() {
    // The DHEADER is patched with what followed it, which is what lets a
    // reader skip members it does not know.
    let mut w = Writer::little_endian();
    let header = w.write_dheader();
    w.write_i32(7);
    w.finish_dheader(header);
    let bytes = w.finish();

    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[..4], &[4, 0, 0, 0], "the DHEADER is not 4");

    let mut r = Reader::new(&bytes, Endianness::Little);
    assert_eq!(r.read_dheader().expect("dheader"), 4);
    assert_eq!(r.read_i32().expect("a"), 7);
}

#[test]
fn the_sequence_a_mutable_struct_generates_carries_its_member_ids() {
    // A reader takes members by id, in whatever order they arrive; the
    // id in the header is what makes that possible.
    let mut w = Writer::little_endian();
    let first = w
        .write_emheader(1, 4, false)
        .expect("a header the plan chose");
    w.write_i32(11);
    w.finish_emheader(first)
        .expect("the member is the length its code declares");
    let second = w
        .write_emheader(2, 4, false)
        .expect("a header the plan chose");
    w.write_i32(22);
    w.finish_emheader(second)
        .expect("the member is the length its code declares");
    let bytes = w.finish();

    let mut r = Reader::new(&bytes, Endianness::Little);
    let head = r.read_emheader().expect("first");
    assert_eq!(head.member_id, 1);
    assert_eq!(head.next_int, Some(4));
    assert_eq!(r.read_i32().expect("a"), 11);

    let head = r.read_emheader().expect("second");
    assert_eq!(head.member_id, 2);
    assert_eq!(r.read_i32().expect("b"), 22);
}

#[test]
fn the_key_hash_sequence_produces_a_stable_sixteen_bytes() {
    // The same key gives the same handle every time, and two different
    // keys give different ones — which is the whole point of it.
    let hash_of = |id: i32| {
        let mut holder = Writer::big_endian();
        holder.write_i32(id);
        holder.zero_padded()
    };
    assert_eq!(hash_of(1), hash_of(1));
    assert_ne!(hash_of(1), hash_of(2));
    assert_eq!(
        hash_of(1)[..4],
        [0, 0, 0, 1],
        "the holder is not big-endian"
    );
}
