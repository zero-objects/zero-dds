// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! P1 "Const-Eval driftet" (idlc broad-audit): array dimensions and collection
//! bounds written as a const reference (`data[CAP]`) or a const-expression
//! (`data[BASE*2]`) are resolved through the ONE central evaluator
//! (`zerodds_idl::semantics::build_symbol_table` + `evaluate`) instead of the
//! former literal-only `array_size`, which returned `None` and aborted the Go
//! backend with `Unsupported("non-literal array size")`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

fn gen_go(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_go_module(&ast, &GoGenOptions::default()).expect("gen")
}

/// The audit repro: `CAP = BASE * 2` resolves to `8` in the fixed-array
/// dimension and the bounded-sequence check — the backend no longer aborts.
#[test]
fn const_expr_bound_resolves_to_eight() {
    let go = gen_go(
        "const long BASE = 4; const long CAP = BASE * 2; \
         @final struct S { long data[CAP]; sequence<long, CAP> s; };",
    );
    assert!(
        go.contains("[8]int32"),
        "array bound CAP (=BASE*2=8) must resolve to [8]int32:\n{go}"
    );
    assert!(
        go.contains("encoded sequence length exceeds its IDL bound (8)"),
        "bounded sequence<long, CAP> must check against the resolved bound 8:\n{go}"
    );
}

/// A bare const reference (`data[CAP]`, no arithmetic) resolves too.
#[test]
fn const_ref_array_dimension_resolves() {
    let go = gen_go("const long CAP = 5; @final struct S { long data[CAP]; };");
    assert!(
        go.contains("[5]int32"),
        "array bound CAP (=5) must resolve to [5]int32:\n{go}"
    );
}

/// Gegenprobe: a direct integer-literal bound keeps the exact former rendering.
#[test]
fn literal_bound_unchanged() {
    let go = gen_go("@final struct S { long data[8]; sequence<long, 8> s; };");
    assert!(
        go.contains("[8]int32"),
        "literal array bound 8 stays [8]int32:\n{go}"
    );
    assert!(
        go.contains("encoded sequence length exceeds its IDL bound (8)"),
        "literal sequence bound 8 keeps its check:\n{go}"
    );
}
