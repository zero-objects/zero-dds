// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! P1 "Const-Eval driftet" (idlc broad-audit): array dimensions and collection
//! bounds written as a const reference (`data[CAP]`) or a const-expression
//! (`data[BASE*2]`) are resolved through the ONE central evaluator
//! (`zerodds_idl::semantics::build_symbol_table` + `eval_bound`) instead of the
//! former literal-only backend re-parse, which dropped every non-literal to `0`
//! and emitted `std::array<..., 0>`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn gen_cpp(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

/// The audit repro: `CAP = BASE * 2` must resolve to `8` in BOTH the fixed-array
/// dimension and the bounded-sequence check — never `std::array<..., 0>`.
#[test]
fn const_expr_bound_resolves_to_eight() {
    let cpp = gen_cpp(
        "const long BASE = 4; const long CAP = BASE * 2; \
         @final struct S { long data[CAP]; sequence<long, CAP> s; };",
    );
    assert!(
        cpp.contains("std::array<int32_t, 8>"),
        "array bound CAP (=BASE*2=8) must resolve to std::array<int32_t, 8>, \
         not 0:\n{cpp}"
    );
    assert!(
        !cpp.contains("std::array<int32_t, 0>"),
        "the CAP dimension must never collapse to 0:\n{cpp}"
    );
    assert!(
        cpp.contains(".size() > 8") && cpp.contains("bounded sequence length"),
        "bounded sequence<long, CAP> must check against the resolved bound 8:\n{cpp}"
    );
}

/// A bare const reference (`data[CAP]`, no arithmetic) resolves too.
#[test]
fn const_ref_array_dimension_resolves() {
    let cpp = gen_cpp("const long CAP = 5; @final struct S { long data[CAP]; };");
    assert!(
        cpp.contains("std::array<int32_t, 5>"),
        "array bound CAP (=5) must resolve to std::array<int32_t, 5>:\n{cpp}"
    );
}

/// Gegenprobe: a direct integer-literal bound keeps the exact former rendering
/// (the literal fast path is value-identical — no snapshot churn).
#[test]
fn literal_bound_unchanged() {
    let cpp = gen_cpp("@final struct S { long data[8]; sequence<long, 8> s; };");
    assert!(
        cpp.contains("std::array<int32_t, 8>"),
        "literal array bound 8 stays std::array<int32_t, 8>:\n{cpp}"
    );
    assert!(
        cpp.contains(".size() > 8"),
        "literal sequence bound 8 keeps its .size() > 8 check:\n{cpp}"
    );
}
