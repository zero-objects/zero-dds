// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java binding of the Enhanced Service Mapping (DDS-RPC 1.0
//! §7.5.1.2): the PL_CDR2 codec of its topic types and the function-call
//! classes (`${I}ClientImpl`, `${I}ServiceImpl`) over them.
//!
//! What is held here is the generated text and that `javac` accepts it
//! together with the RPC runtime (`runtime/java`) under `-Xlint:all`. The
//! octets are compared with those of the clause by
//! `crates/rpc/tests/java_enhanced_function_call.rs`, which runs a program
//! only where the host runs generated programs.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_lang_rules::adapter::interface::{ServiceMapping, member_id};

/// An exception, an `inout` parameter, an operation with nothing to carry,
/// an attribute, `@Mutable`, a base interface in
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

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn model(idl: &str) -> Ast {
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
    ast
}

/// The Java the generator writes for `idl` under `mapping`, as one text.
fn generate(idl: &str, mapping: ServiceMapping) -> String {
    zerodds_lang_rules::generate::source_with_mapping(
        &model(idl),
        zerodds_lang_cst::Language::Java,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        mapping,
    )
    .expect("the Java column takes this model")
}

/// The generated units through `javac`, with the RPC runtime.
fn compiles(text: &str, tag: &str) -> Result<(), String> {
    let root = root();
    let dir = std::env::temp_dir().join(format!("zd-enh-java-{}-{tag}", std::process::id()));
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for unit in zerodds_lang_rules::generate::java_units(text) {
        let body = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text)
        } else {
            unit.text.clone()
        };
        let file = dir.join(format!("{}.java", unit.class));
        std::fs::write(&file, body).map_err(|e| e.to_string())?;
        files.push(file);
    }
    for package in [
        "runtime/java/src/main/java/org/omg/dds/rpc",
        "runtime/java/src/main/java/org/zerodds/ddsrpc",
    ] {
        for entry in std::fs::read_dir(root.join(package)).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "java") {
                files.push(path);
            }
        }
    }
    if let Ok(keep) = std::env::var("ZD_KEEP_GENERATED") {
        let _ = std::fs::write(PathBuf::from(keep).join(format!("{tag}.java.txt")), text);
    }
    let output = Command::new("javac")
        .args(["-encoding", "UTF-8", "-Xlint:all,-unchecked", "-Werror"])
        .arg("-d")
        .arg(&classes)
        .arg("-sourcepath")
        .arg(format!(
            "{}:{}:{}",
            root.join("runtime/java/src/main/java").display(),
            root.join("crates/java-omgdds/java/src/main/java").display(),
            dir.display()
        ))
        .args(&files)
        .output()
        .map_err(|e| e.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_enhanced_java_compiles() {
    if !javac_available() {
        eprintln!("SKIPPED: `javac` is not usable.");
        return;
    }
    for (tag, mapping) in [
        ("enhanced", ServiceMapping::Enhanced),
        ("enhanced-empty", ServiceMapping::EnhancedEmptyStructures),
    ] {
        compiles(&generate(ENHANCED, mapping), tag).unwrap_or_else(|why| panic!("{tag}: {why}"));
    }
}

/// What the generated codec writes is the PL_CDR2 form the Rust codec
/// writes, so the member ids and the length codes are the ones the model
/// names: 4 for a member written with a `NEXTINT`, 2 for a 32-bit member.
#[test]
fn the_codec_writes_the_member_ids_and_length_codes_of_the_clause() {
    let text = generate(ENHANCED, ServiceMapping::Enhanced);
    for (member, code) in [("add", 4), ("remoteEx", 2), ("x", 2), ("result", 4)] {
        assert!(
            text.contains(&format!(
                "writeEmheader({}, {code}, false)",
                member_id(member)
            )),
            "the member `{member}` has length code {code}"
        );
    }
    // An `In` without `@Mutable` is final: its members one after the other.
    assert!(text.contains("encode_Calc_add_In"), "{text}");
    assert!(
        !text.contains(&format!("writeEmheader({}, 2, false)", member_id("a"))),
        "`Calc_add_In` has no headers"
    );
}

#[test]
fn the_function_call_classes_are_written_over_the_choice_holders() {
    let text = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(text.contains("class CalcClientImpl"));
    assert!(text.contains("class CalcServiceImpl"));
    // The client sets the member of the operation and reads the Result of
    // that member from the reply.
    assert!(text.contains("request.add = new Calc_add_In();"), "{text}");
    assert!(text.contains("Calc_add_Result result = reply.add;"));
    assert!(text.contains("new org.omg.dds.rpc.RemoteUnknownExceptionError("));
    // The service dispatches on the member that is present.
    assert!(text.contains("if (request.add != null)"));
    assert!(text.contains("REMOTE_EX_UNKNOWN_OPERATION"));
    // The runtime is told the mapping through the params.
    assert!(text.contains("ServiceMapping.ENHANCED"));
    assert!(text.contains("EnhancedReplySupport<"));
    // No `Call` union and no header member.
    assert!(!text.contains("Calc_Call"));
}

/// §7.5.1.2.6: the request of a derived interface holds the operations of
/// its base, under the base's type names, so the derived client and service
/// answer them on the derived topics.
#[test]
fn a_derived_interface_answers_the_inherited_operation_itself() {
    let text = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(text.contains("class Derived2ClientImpl"));
    assert!(
        text.contains("Base2Codec.encode_Base2_ident_In"),
        "the codec of the base's In structure is the base's own"
    );
    assert!(text.contains("request.ident = new"), "{text}");
    assert!(
        !text.contains("registerBase"),
        "the derived client and service hold no client or service of a base"
    );
}

/// §7.5.1.2.1.1: "exactly one member shall be present at any given time".
/// The encoder refuses none and several members, and the decoder refuses a
/// second member instead of skipping over it; the generated `TypeSupport`
/// raises what the reader refused instead of returning a sample that was
/// read from a malformed one. The run-time counterpart is
/// `crates/rpc/tests/java/EnhancedRuntimeChecks.java`.
#[test]
fn a_choice_holds_exactly_one_member_on_both_sides_of_the_codec() {
    let text = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(
        text.contains("Calc_Request holds exactly one member (DDS-RPC 1.0 §7.5.1.2.1.1)"),
        "the encoder of the request"
    );
    assert!(
        text.contains("Calc_Reply holds exactly one member (DDS-RPC 1.0 §7.5.1.2.1.1)"),
        "the encoder of the reply"
    );
    assert!(
        text.contains("Calc_add_Result holds exactly one member (DDS-RPC 1.0 §7.5.1.2.1.1)"),
        "the encoder of a Result"
    );
    // After the first member has been read and skipped, a member that is
    // still in the frame refuses the stream.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains(
            "r.readerSkipTo(after); if (r.position() < end && r.readerStatus() == \
             org.zerodds.wire.Reader.Status.OK) { r.readerRefuse(\
             org.zerodds.wire.Reader.Status.OUT_OF_RANGE); }"
        ),
        "the decoder refuses a second member"
    );
    assert!(
        text.contains("Calc_Request is malformed (DDS-RPC 1.0 §7.5.1.2.1.1): "),
        "the type support raises what the reader refused"
    );
}

/// §7.4.1: the generated `Support` names the interface, so a run-time
/// service name composes `<interface>_<service>_Request` (§7.4.2.3).
#[test]
fn the_support_sets_the_interface_name() {
    let text = generate(ENHANCED, ServiceMapping::Enhanced);
    assert!(text.contains("INTERFACE_NAME = \"enh_Calc\""), "{text}");
    assert!(text.contains(".interfaceName(INTERFACE_NAME)"), "{text}");
    assert!(
        text.contains(".annotatedServiceName(SERVICE_NAME)"),
        "{text}"
    );
}

#[test]
fn the_basic_mapping_is_not_touched() {
    let text = generate(ENHANCED, ServiceMapping::Basic);
    assert!(!text.contains("ServiceMapping.ENHANCED"));
    assert!(text.contains("Calc_Call"), "the Basic mapping's union");
}

#[test]
fn the_empty_structure_reading_has_no_dummy_member_in_the_codec() {
    let dummy = generate(ENHANCED, ServiceMapping::Enhanced);
    let empty = generate(ENHANCED, ServiceMapping::EnhancedEmptyStructures);
    assert!(
        dummy.contains("UnusedMember"),
        "the dummy member is an octet"
    );
    assert!(
        !empty.contains("Calc_ping_In.dummy") && !empty.contains("ping.dummy"),
        "an @Empty structure has no octet"
    );
}
