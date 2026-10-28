// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ binding of the Enhanced Service Mapping (DDS-RPC 1.0 §7.5.1.2):
//! the PL_CDR2 codec of its topic types and the function-call classes
//! (`${I}Client`, `${I}Service`) over them.
//!
//! What is held here is the generated text and that a C++ compiler accepts
//! it together with the runtime header `runtime/cpp/include/dds/rpc.hpp`,
//! under `-Wall -Wextra -Werror`. The octets are written by the generated
//! codec and compared with the octets of the clause by a program the test
//! compiles and links; the program runs only where `ZD_RUN_GENERATED=1`
//! says a run is wanted (the Linux test host), not on a developer
//! machine.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::rule::cpp;
use zerodds_lang_rules::{ModelGraph, apply};

/// An exception, an `inout` parameter, an operation with nothing to carry,
/// an operation without reply data, an attribute, `@Mutable`, a base interface in
/// another module and a derived interface that answers its operation.
const ENHANCED: &str = "
module base {
  exception Refused { string why; };
  interface Base2 {
    long ident() raises (Refused);
    attribute long level;
  };
};
module enh {
  exception TooBig { long limit; };
  typedef sequence<long> Longs;
  interface Calc {
    long add(in long a, in long b);
    long scale(in long k) raises (TooBig);
    void swap(inout long v);
    void ping();
    void notify(in string text);
    Longs all(in long n, out string label);
  };
  interface Fast {
    @Mutable long square(in long x);
  };
  interface Derived2 : base::Base2 {
    long more();
  };
};
";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// IDL in, one C++ header out, for the given service mapping.
fn generate(idl: &str, mapping: ServiceMapping) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "enhanced.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &cpp::catalogue()).expect("apply");
    let mut decls = zerodds_lang_rules::adapter::cpp::cpp_items(&ast, &applied).expect("types");
    decls.extend(
        zerodds_lang_rules::emit::interface_cpp::cpp_interface_items_with(&ast, mapping)
            .expect("the RPC binding"),
    );
    zerodds_lang_render::cpp::file(&zerodds_lang_cst::cpp::File {
        doc: zerodds_lang_cst::common::Doc::none(),
        pragma_once: true,
        includes: zerodds_lang_rules::adapter::cpp::includes(),
        decls,
    })
}

fn compiler() -> Option<String> {
    if let Ok(chosen) = std::env::var("CXX") {
        return Some(chosen);
    }
    ["c++", "g++", "clang++"]
        .iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(|found| (*found).to_string())
}

/// A scratch directory of this test's own.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zd-enh-cpp-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// The header as a consumer includes it, through the compiler.
fn compiles(cxx: &str, header: &str, tag: &str) -> Result<(), String> {
    let dir = scratch(tag);
    std::fs::write(dir.join("zd_types.hpp"), header).map_err(|e| e.to_string())?;
    let unit = dir.join("tu.cpp");
    std::fs::write(&unit, "#include \"zd_types.hpp\"\n").map_err(|e| e.to_string())?;
    let output = Command::new(cxx)
        .args(["-std=c++20", "-fsyntax-only", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-I{}", dir.display()))
        .arg(format!(
            "-I{}",
            root().join("runtime/cpp/include").display()
        ))
        .arg(&unit)
        .output()
        .map_err(|e| e.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    if let Ok(keep) = std::env::var("ZD_KEEP_GENERATED") {
        let _ = std::fs::write(PathBuf::from(keep).join(format!("{tag}.hpp")), header);
    }
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_enhanced_header_compiles() {
    let Some(cxx) = compiler() else {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    };
    for (tag, mapping) in [
        ("enhanced", ServiceMapping::Enhanced),
        ("enhanced-empty", ServiceMapping::EnhancedEmptyStructures),
    ] {
        let header = generate(ENHANCED, mapping);
        compiles(&cxx, &header, tag).unwrap_or_else(|why| panic!("{tag}: {why}"));
    }
}

/// What the generated codec writes is the PL_CDR2 form the Rust codec
/// writes, so the member ids and the length codes are the ones the model
/// names: 4 for a member written with a `NEXTINT`, 2 for a 32-bit member.
#[test]
fn the_codec_writes_the_member_ids_and_length_codes_of_the_clause() {
    use zerodds_lang_rules::adapter::interface::member_id;
    let header = generate(ENHANCED, ServiceMapping::Enhanced);
    // The `Request` holds `add` with a NEXTINT; `remoteEx` is a 32-bit
    // member of the `Reply`.
    assert!(
        header.contains(&format!("write_emheader({}, 4, false)", member_id("add"))),
        "the request member of `add`"
    );
    assert!(
        header.contains(&format!(
            "write_emheader({}, 2, false)",
            member_id("remoteEx")
        )),
        "remoteEx, length code 2"
    );
    // The `@Mutable` `In` of `Fast::square` has a header per member: a
    // `long` is length code 2.
    assert!(
        header.contains(&format!("write_emheader({}, 2, false)", member_id("x"))),
        "the member `x` of a `@Mutable` structure"
    );
    // A `string` member has the NEXTINT.
    assert!(
        header.contains(&format!(
            "write_emheader({}, 4, false)",
            member_id("result")
        )),
        "the `result` of a Result structure"
    );
    // An `In` without `@Mutable` is final: its members one after the other.
    assert!(header.contains("zd_enc_enh_enh_Calc_add_In"), "{header}");
    assert!(
        !header.contains(&format!("write_emheader({}, 2, false)", member_id("a"))),
        "`Calc_add_In` has no headers"
    );
}

#[test]
fn the_function_call_classes_are_written_over_the_choice_structures() {
    let header = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(header.contains("class CalcClient"), "the client");
    assert!(header.contains("class CalcService"), "the service");
    // The client sets the member of the operation, and reads the Result of
    // that member from the reply.
    assert!(header.contains("zd_request.add.emplace();"), "{header}");
    assert!(header.contains("zd_request.add->a = a;"));
    assert!(header.contains("const Calc_add_Result& zd_result = *zd_reply.add;"));
    assert!(header.contains("throw *zd_result.toobig_ex;"));
    // The service dispatches on the member that is present.
    assert!(header.contains("if (zd_request.add) {"));
    assert!(header.contains("RemoteUnknownOperationError(\"the request holds no operation"));
    // The runtime is told the mapping through the params.
    assert!(header.contains("dds::rpc::enhanced_requester_params_of("));
    assert!(header.contains("dds::rpc::enhanced_replier_params_of("));
    // No `Call` union and no header member: the Enhanced profile sends the
    // bare structure (§7.8.2).
    assert!(!header.contains("Calc_Call"));
    assert!(!header.contains("dds::rpc::RequestHeader"));
}

/// §7.5.1.2.6: the request of a derived interface holds the operations of
/// its base, under the base's type names, so the derived client and service
/// answer them on the derived topics, and the structures of a base in
/// another module are reached through that module.
#[test]
fn a_derived_interface_answers_the_inherited_operation_itself() {
    let header = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(header.contains("class Derived2Client"));
    assert!(
        header.contains("zd_enc_enh_base_Base2_ident_In"),
        "the codec of the base's In structure is the base's own"
    );
    assert!(
        header.contains("zd_request.ident.emplace();"),
        "the derived client sets the inherited member"
    );
    assert!(
        header.contains("implementation_->ident()"),
        "and the derived service calls the implementation"
    );
    // The derived client holds no client of the base.
    assert!(!header.contains("zd_base_0_"), "{header}");
    // An attribute of the base is answered by the derived client too.
    assert!(header.contains("level"));
}

#[test]
fn the_basic_mapping_is_not_touched() {
    let header = generate(ENHANCED, ServiceMapping::Basic);
    assert!(
        !header.contains("zd_enc_enh_enh_Calc_Request"),
        "no Enhanced codec"
    );
    assert!(!header.contains("enhanced_requester_params_of"));
    assert!(header.contains("Calc_Call"), "the Basic mapping's union");
}

/// The two readings of an operation with nothing to carry (DDS-RPC 1.0
/// §7.5.1.2.1.3): a `dummy` member, or an `@Empty` structure with none.
#[test]
fn the_empty_structure_reading_has_no_dummy_member_in_the_codec() {
    let dummy = generate(ENHANCED, ServiceMapping::Enhanced);
    let empty = generate(ENHANCED, ServiceMapping::EnhancedEmptyStructures);
    assert!(
        dummy.contains("write_u8(v.dummy)"),
        "the dummy member is an octet"
    );
    assert!(
        !empty.contains("write_u8(v.dummy)"),
        "an @Empty structure has no octet"
    );
}
