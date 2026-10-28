// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The function-call classes of DDS-RPC 1.0 §7.11.1.1.2 under
//! `--cpp-mapping psm`: where the application sees a parameter, a return
//! value or an attribute, the type is the one DDS-PSM-Cxx 1.0 Table 7.1
//! names — `std::wstring` for `wstring`, `std::map` for `map`,
//! `dds::core::array` for an array — and the generated bodies convert
//! between it and the topic structures with `zerodds::psm::Conv`.
//!
//! The topic structures themselves (the `In`, `Out`, `Result`, `Call`,
//! `Return`, `Request` and `Reply` types) are the native types the codec
//! reads and writes, in `zd_native`; the header with both is compiled.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_rules::adapter::cpp::{CppMapping, CppMappingScope};
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::emit::encode::CodecOptions;

const IDL: &str = "
module lab {
  struct Pose { double x; double y; };
  typedef long Grid[2][3];
  typedef long Four[4];
  typedef map<string, long> Table;
  exception Refused { string why; };
  interface Probe {
    wstring echo(in wstring text);
    long count(in map<string, long> table, in Four extra);
    Pose move(in Pose start, inout wstring note, out Table seen);
    long careful(in long value) raises (Refused);
    attribute wstring title;
    readonly attribute Pose origin;
    void reset();
    void fill(in Grid cells, out Grid done);
  };
};
";

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "psm.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn cpp(mapping: ServiceMapping) -> String {
    let ast = model();
    let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);
    zerodds_lang_rules::generate::cpp_source_with_mapping(
        &ast,
        key,
        CodecOptions::default(),
        "zd_codec",
        mapping,
    )
    .expect("generate")
    .text
}

fn psm(mapping: ServiceMapping) -> String {
    let _scope = CppMappingScope::enter(CppMapping::Psm);
    cpp(mapping)
}

/// The text of `class <name> ` up to the closing brace at its indent.
fn class_of<'a>(text: &'a str, name: &str) -> &'a str {
    let head = format!("\nclass {name} ");
    let start = text
        .find(&head)
        .unwrap_or_else(|| panic!("no `class {name}` in:\n{text}"));
    let rest = &text[start + 1..];
    let end = rest.find("\n};").expect("the class ends");
    &rest[..end]
}

/// DDS-PSM-Cxx 1.0 Table 7.1, in the abstract interface.
#[test]
fn the_interface_spells_the_types_of_table_7_1() {
    let text = psm(ServiceMapping::Basic);
    let interface = class_of(&text, "Probe");
    // wstring -> std::wstring.
    assert!(
        interface.contains("const std::wstring& text"),
        "{interface}"
    );
    // map<K,V> -> std::map<K,V>.
    assert!(
        interface.contains("const std::map<std::string, int32_t>& table"),
        "{interface}"
    );
    // T[N] -> dds::core::array<T, N>, which the alias `Four` is.
    assert!(
        interface.contains("const ::lab::Four& extra"),
        "{interface}"
    );
    assert!(
        text.contains("using Four = ::dds::core::array<int32_t, 4>;"),
        "{text}"
    );
    // The attribute is the PSM's too, and so is an `out` parameter.
    assert!(interface.contains("std::wstring& title()"), "{interface}");
    assert!(interface.contains("::lab::Table& seen"), "{interface}");
    // Nothing of the native spelling is where the application sees it.
    assert!(!interface.contains("std::pair"), "{interface}");
    assert!(!interface.contains("std::array"), "{interface}");
}

/// The same interface under the native mapping keeps its spelling.
#[test]
fn the_native_mapping_keeps_the_native_spelling() {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = model();
    let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);
    let text = zerodds_lang_rules::generate::cpp_source_with_mapping(
        &ast,
        key,
        CodecOptions::default(),
        "zd_codec",
        ServiceMapping::Basic,
    )
    .expect("generate")
    .text;
    let interface = class_of(&text, "Probe");
    assert!(
        interface.contains("::zerodds::Map<std::string, std::int32_t>"),
        "{interface}"
    );
    // The array is spelled by its alias, which is the native `std::array`.
    assert!(
        interface.contains("const ::lab::Four& extra"),
        "{interface}"
    );
    assert!(
        text.contains("using Four = std::array<std::int32_t, 4>;"),
        "{text}"
    );
    assert!(!interface.contains("std::wstring"), "{interface}");
    assert!(!text.contains("zerodds::psm::Conv"), "{text}");
}

/// The client packs the topic structure from the PSM's values and unpacks
/// the reply into them; the service does the same the other way round.
#[test]
fn the_bodies_convert_between_the_psm_types_and_the_topic_structures() {
    let text = psm(ServiceMapping::Basic);
    let client = class_of(&text, "ProbeClient");
    assert!(
        client.contains(
            "zd_request.data.echo.text = zerodds::psm::Conv<std::wstring>::to_native(text);"
        ),
        "{client}"
    );
    assert!(
        client.contains(
            "zd_request.data.count.table = zerodds::psm::Conv<std::map<std::string, int32_t>>::to_native(table);"
        ),
        "{client}"
    );
    assert!(
        client.contains("note = zerodds::psm::Conv<std::wstring>::from_native(zd_out.note);"),
        "an inout parameter goes out and comes back: {client}"
    );
    assert!(
        client
            .contains("cxx_return = zerodds::psm::Conv<::lab::Pose>::from_native(zd_out.return_);"),
        "{client}"
    );
    let service = class_of(&text, "ProbeService");
    assert!(
        service.contains(
            "std::wstring zd_arg_note = zerodds::psm::Conv<std::wstring>::from_native(zd_in.note);"
        ),
        "an inout argument is a local of the PSM's type: {service}"
    );
    assert!(
        service.contains("zd_out.note = zerodds::psm::Conv<std::wstring>::to_native(zd_arg_note);"),
        "{service}"
    );
    assert!(
        service.contains("zd_out.seen = zerodds::psm::Conv<::lab::Table>::to_native(zd_arg_seen);"),
        "{service}"
    );
}

/// §7.11.1.1.2 rule 7.h and 7.i: the `future` of an asynchronous call holds
/// the return value, or the `Out` structure, as the application sees them:
/// `future<std::wstring>` for a `wstring` return, and an `Out` structure of
/// the IDL's namespace whose members have Table 7.1's spellings, built from the
/// native one the reply holds.
#[test]
fn the_futures_hold_the_applications_types() {
    let text = psm(ServiceMapping::Basic);
    let asynchronous = class_of(&text, "ProbeAsync");
    assert!(
        asynchronous
            .contains("dds::rpc::future<std::wstring> echo_async(const std::wstring& text)"),
        "{asynchronous}"
    );
    assert!(
        asynchronous.contains("dds::rpc::future<Probe_move_Out> move_async("),
        "{asynchronous}"
    );
    // The application's `Out`: in the IDL's namespace, with the PSM types.
    let own = text
        .match_indices("struct Probe_move_Out {")
        .map(|(at, _)| &text[at..])
        .map(|rest| &rest[..rest.find("\n};").expect("the struct ends")])
        .find(|body| body.contains("std::wstring note;"))
        .expect("the application's Out structure");
    assert!(own.contains("::lab::Table seen;"), "{own}");
    assert!(own.contains("::lab::Pose return_;"), "{own}");
    // The native one stays in zd_native, with the native spellings.
    assert!(
        text.contains("const ::zd_native::lab::Probe_move_Out& zd_native = result_move(zd_reply);"),
        "{text}"
    );
    let client = class_of(&text, "ProbeClient");
    assert!(
        client.contains(
            "zd_out.note = zerodds::psm::Conv<std::wstring>::from_native(zd_native.note);"
        ),
        "{client}"
    );
    assert!(
        client.contains("static std::wstring unpack_echo("),
        "{client}"
    );
    // The native mapping keeps both as they were.
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = model();
    let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);
    let native = zerodds_lang_rules::generate::cpp_source_with_mapping(
        &ast,
        key,
        CodecOptions::default(),
        "zd_codec",
        ServiceMapping::Basic,
    )
    .expect("generate")
    .text;
    assert!(!native.contains("zd_native"), "{native}");
}

/// §7.11.1.1.2 rule 7.i under the Enhanced Service Mapping: the `future` of an
/// operation with `out` parameters holds the application's `Out` structure,
/// built from the topic structure of §7.5.1.2.3, as for the Basic mapping.
#[test]
fn the_enhanced_futures_hold_the_applications_types() {
    let text = psm(ServiceMapping::Enhanced);
    let asynchronous = class_of(&text, "ProbeAsync");
    assert!(
        asynchronous.contains("dds::rpc::future<Probe_move_Out> move_async("),
        "{asynchronous}"
    );
    let own = text
        .match_indices("struct Probe_move_Out {")
        .map(|(at, _)| &text[at..])
        .map(|rest| &rest[..rest.find("\n};").expect("the struct ends")])
        .find(|body| body.contains("std::wstring note;"))
        .expect("the application's Out structure");
    assert!(own.contains("::lab::Table seen;"), "{own}");
    let client = class_of(&text, "ProbeClient");
    assert!(
        client.contains("dds::rpc::future<Probe_move_Out> move_async("),
        "{client}"
    );
    assert!(
        client.contains(
            "zd_out.note = zerodds::psm::Conv<std::wstring>::from_native(zd_native.note);"
        ),
        "{client}"
    );
    assert!(
        !client.contains("future<::zd_native"),
        "no future holds a native topic structure:\n{client}"
    );
}

/// The topic types stay what the codec reads and writes: native, in
/// `zd_native`; the application's classes see them through `using`.
#[test]
fn the_topic_types_are_native_and_the_classes_are_the_applications() {
    let text = psm(ServiceMapping::Basic);
    assert!(text.contains("namespace zd_native"), "{text}");
    assert!(
        text.contains("struct dds_type_traits<::zd_native::lab::Probe_Request>"),
        "{text}"
    );
    assert!(
        text.contains("using ::zd_native::lab::Probe_Request;"),
        "{text}"
    );
    assert!(
        text.contains("using ::zd_native::lab::Probe_echo_Hash;"),
        "{text}"
    );
    // The abstract class is in the namespace of the IDL, not in zd_native.
    let before = &text[..text.find("\nclass Probe ").expect("the interface")];
    let open_native = before.matches("namespace zd_native {").count();
    let close_native = before.matches("} // namespace zd_native").count();
    assert_eq!(open_native, close_native, "Probe is outside zd_native");
}

fn compile(text: &str, name: &str) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("zd-interface-cpp-psm-{name}"));
    std::fs::create_dir_all(&dir).expect("dir");
    let header = format!(
        "#pragma once\n#include <array>\n#include <cstdint>\n#include <map>\n#include <memory>\n\
         #include <optional>\n#include <string>\n#include <utility>\n#include <vector>\n\
         #include \"zerodds/wire.hpp\"\n#include \"zerodds/psm/convert.hpp\"\n\
         #include <dds/rpc.hpp>\n{text}\n"
    );
    std::fs::write(dir.join("zd_types.hpp"), header).expect("header");
    std::fs::write(
        dir.join("main.cpp"),
        "#include \"zd_types.hpp\"\nint main() { return 0; }\n",
    )
    .expect("main");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("c++")
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/cpp/include"))
        .arg("-I")
        .arg(&dir)
        .arg(dir.join("main.cpp"))
        .output();
    match output {
        Err(error) => {
            eprintln!("no C++ compiler: {error}");
            None
        }
        Ok(output) if output.status.success() => Some(String::new()),
        Ok(output) => Some(String::from_utf8_lossy(&output.stderr).into_owned()),
    }
}

/// The header, with the PSM classes, the topic types and the function-call
/// classes in it, is C++ that compiles — under both service mappings.
#[test]
fn the_header_compiles() {
    for (mapping, name) in [
        (ServiceMapping::Basic, "basic"),
        (ServiceMapping::Enhanced, "enhanced"),
    ] {
        let text = psm(mapping);
        if let Some(errors) = compile(&text, name) {
            assert!(errors.is_empty(), "{name}: {errors}\n{text}");
        }
    }
}
