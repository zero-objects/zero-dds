// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 Annex E in C++: the TypeSupport classes of the built-in types
//! (`zerodds/builtin_type_support.hpp`), compiled against the C ABI library and run.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

mod cxx_program;

#[test]
fn the_type_supports_of_the_built_in_types_name_and_register_their_types() {
    cxx_program::build_compile_run(
        "crates/cpp/tests/builtin_type_support.cpp",
        "zerodds_builtin_type_support",
        "BUILTIN TYPE SUPPORT OK",
    );
}
