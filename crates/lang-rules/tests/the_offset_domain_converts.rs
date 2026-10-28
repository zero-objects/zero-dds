// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The reader's offset domain, and what may be put beside it.
//!
//! `crates/lang-rules/src/emit/decode.rs` keeps positions and lengths in
//! the target's own size type — `size_t` in C, `int` in the C#, Go, Java
//! and F# runtimes, `Long` in Kotlin's — and every value that comes off
//! the wire beside one is narrower or wider: a DHEADER is a `uint32`, an
//! EMHEADER's extent is a `uint32`, a PID's length is a `uint16`, a
//! sentinel's size is a literal. Fifteen of the twenty columns widen the
//! narrower side themselves. F# and Kotlin convert nothing implicitly
//! and name a type error, which is why this is measured rather than
//! assumed.
//!
//! Two sites were wrong when this was written, both in the `@mutable`
//! read walk and both the same shape:
//!
//! - `zd_ends = reader_position() + zd_h.extent`, with no conversion
//!   (`error FS0001`, "uint32 does not match int"; Kotlin
//!   `assignment type mismatch: actual type is 'Long', but 'Int' was
//!   expected`);
//! - `reader_remaining() < 4u`, in the PL_CDR1 arm, against a
//!   `ReaderRemaining() : int`.
//!
//! A third of the same family had already been fixed once, in
//! `Framing::reached` — `T::int(0)` there and not
//! `CodecTarget::unsigned_int`, with the reason written beside it. Three
//! separate patches would have been three; the walk now has one
//! entrance to the domain (`offset_plus`, and `position`/`remaining`
//! beside it) and this file is what says the entrance is used.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::common::Language;
use zerodds_lang_rules::generate::source;

/// The one type every assertion below is made about: `@mutable`, so the
/// read walk emits the EMHEADER arm under XCDR2 and the PL_CDR1 arm
/// under XCDR1, and three members, so the id chain has something in it.
const MUTABLE: &str = "module m { @mutable struct Sample { long a; double b; short c; }; };";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("the model resolves");
    ast
}

/// The whole generated column for `MUTABLE` under one encoding.
fn generated(language: Language, encoding: Encoding) -> String {
    let ast = model(MUTABLE);
    source(
        &ast,
        language,
        LayoutKey::data(encoding, Endianness::Little),
    )
    .unwrap_or_else(|why| panic!("{}: {why:?}", language.as_str()))
}

#[test]
fn the_emheader_extent_is_converted_before_it_is_added_to_a_position() {
    // F# and Kotlin are the two columns that say so out loud; the
    // assertion is on their spelling of the conversion, because a
    // conversion that is there in C and missing in these two is exactly
    // the state this file exists to catch.
    let fsharp = generated(Language::FSharp, Encoding::PlCdr2);
    assert!(
        fsharp.contains("r.ReaderPosition() + int32(zd_h.Extent)"),
        "the F# column adds `zd_h.Extent : uint32` to `ReaderPosition() : int` \
         with no conversion:\n{fsharp}"
    );
    let kotlin = generated(Language::Kotlin, Encoding::PlCdr2);
    assert!(
        kotlin.contains("r.readerPosition() + zd_h.extent.toInt()"),
        "the Kotlin column adds `zd_h.extent : Long` to `readerPosition() : Int` \
         with no conversion:\n{kotlin}"
    );
    // And C, which never needed the conversion and now writes one: the
    // cast is the walk's and not a target's, so it reaches every column
    // and this is what says it renders where it was already correct.
    let c = generated(Language::C, Encoding::PlCdr2);
    assert!(
        c.contains("zdw_reader_position(r) + (size_t)zd_h.extent"),
        "the C column's offset addition has changed shape:\n{c}"
    );
}

#[test]
fn the_pl_cdr1_sentinel_check_compares_two_values_of_one_width() {
    // `4u` is not a token in F# and is a second width where it is one.
    // `Framing::reached` already answers this with `T::int(0)` and the
    // reason beside it; this is the same answer at the other site.
    let fsharp = generated(Language::FSharp, Encoding::PlCdr1);
    assert!(
        fsharp.contains("r.ReaderRemaining() < 4 then"),
        "the F# column compares `ReaderRemaining() : int` against an unsigned \
         literal:\n{fsharp}"
    );
    assert!(
        !fsharp.contains("ReaderRemaining() < 4u"),
        "the F# column still writes the `u` suffix:\n{fsharp}"
    );
}

#[test]
fn every_column_still_renders_a_mutable_decoder_under_all_five_encodings() {
    // The cast reaches all twenty-one columns, so the floor under the
    // two assertions above is that none of them stopped rendering.
    for language in Language::ALL {
        for encoding in Encoding::ALL {
            let ast = model(MUTABLE);
            let key = LayoutKey::data(encoding, Endianness::Little);
            if language == Language::Rust {
                // The Rust column's codec is `src/encode.rs` and not a
                // `CodecTarget` backend; `generate::source` answers for
                // it only under the encodings that column plans.
                continue;
            }
            source(&ast, language, key)
                .unwrap_or_else(|why| panic!("{} under {encoding:?}: {why:?}", language.as_str()));
        }
    }
}

#[test]
fn the_fsharp_column_leaves_a_loop_body_by_raising_and_not_by_falling_out() {
    // F# has no `return`: a function's value is its body's trailing
    // expression. `emit/fsharp.rs`'s `hoist_early_returns` places every
    // early return the shared walk writes by pushing what followed it
    // down into the branches — every one but a return inside a **loop
    // body**, where the tail would then run once per iteration. The
    // `@mutable` read walk writes exactly that, for a second header
    // carrying an id already read (DDS-XTypes 1.3 §7.4.3.4.2).
    //
    // Without the exception the generated arm ends in a bare `v` inside
    // a `while`, which is `error FS0001` ("this expression should have
    // type unit but has type Sample") — measured, `fsc` 9.0.106.
    let fsharp = generated(Language::FSharp, Encoding::PlCdr2);
    assert!(
        fsharp.contains("raise (ZdWire.GiveBack())"),
        "the F# @mutable decoder writes a bare `v` inside its loop:\n{fsharp}"
    );
    assert!(
        fsharp.contains(":? ZdWire.GiveBack -> ()"),
        "the F# @mutable decoder raises GiveBack with no `try` to catch it:\n{fsharp}"
    );
    // And the guard is not written where nothing raises: a `@final`
    // struct's decoder has no loop and no early return in one.
    let plain = source(
        &model("module m { @final struct Plain { long a; }; };"),
        Language::FSharp,
        LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
    )
    .expect("the @final column renders");
    assert!(
        !plain.contains("ZdWire.GiveBack"),
        "the F# column wraps a body that never raises:\n{plain}"
    );
}
