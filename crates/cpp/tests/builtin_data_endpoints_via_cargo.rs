// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 Annex E in C++: the DataWriter and DataReader classes of the built-in types
//! (`zerodds/builtin_data_endpoints.hpp`), compiled against the C ABI library and run.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

mod cxx_program;

#[test]
fn the_data_writers_and_readers_of_the_built_in_types_write_and_take() {
    cxx_program::build_compile_run(
        "crates/cpp/tests/builtin_data_endpoints.cpp",
        "zerodds_builtin_data_endpoints",
        "BUILTIN DATA ENDPOINTS OK",
    );
}
