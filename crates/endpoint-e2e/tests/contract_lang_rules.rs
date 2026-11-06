// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-endpoint-e2e` with `zerodds-lang-rules`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-lang-rules` (`src/codegen.rs`,
//! `source`): `generate::source` for the `Ping`/`Pong` IDL of every endpoint
//! language under plain XCDR2 little endian, with the C++ mapping set to
//! `CppMapping::Native` (`adapter::cpp::CppMappingScope`) and the Java
//! mapping to `JavaMapping::Omg` (`adapter::java::with_mapping`), because
//! the endpoint apps read and write the public data members of the
//! generated types.
//!
//! Oracle: the classic IDL to C++ mapping of a struct as a C++ struct with
//! public data members of the member types (`int32_t` for `long`, the
//! `<cstdint>` type DDS-PSM-Cxx 1.0 Table 7.1 names), the IDL4 to Java 1.0
//! mapping of a struct as a class with public fields (§7.2.4.3.1, `long`
//! is `int`), and DDS-XTypes 1.3 §7.4.3.4.2 (a FINAL aggregate writes no
//! DHEADER: the apps' octets are the members alone).
#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use zerodds_lang_cst::Language;

#[test]
fn the_native_cpp_mapping_gives_the_apps_public_members() {
    let text = zerodds_endpoint_e2e::codegen::source(Language::Cpp);
    let ping = &text[text.find("struct Ping {").expect("struct Ping")..];
    assert!(
        ping.starts_with("struct Ping {\npublic:\n    std::int32_t seq = {};"),
        "a public data member: {text}"
    );
    assert!(
        !text.contains("int32_t seq() const"),
        "an accessor is the PSM mapping: {text}"
    );
}

#[test]
fn idl4java_7_2_4_3_1_the_omg_java_mapping_gives_the_apps_public_fields() {
    let text = zerodds_endpoint_e2e::codegen::source(Language::Java);
    assert!(text.contains("public int seq"), "{text}");
}

#[test]
fn xtypes_7_4_3_4_2_the_final_ping_is_written_without_a_dheader_in_every_endpoint_language() {
    for language in [
        Language::C,
        Language::Cpp,
        Language::Python,
        Language::Go,
        Language::Java,
        Language::CSharp,
        Language::Node,
    ] {
        let text = zerodds_endpoint_e2e::codegen::source(language);
        assert!(
            text.contains("Ping") && text.contains("Pong"),
            "{language:?}"
        );
        let normalized = text.to_lowercase().replace('_', "");
        assert!(
            !normalized.contains("writedheader"),
            "{language:?}: a @final struct carries no DHEADER"
        );
    }
}
