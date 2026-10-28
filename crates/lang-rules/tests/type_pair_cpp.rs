// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.3.1.4 in the generated C++ binding: a pair of types
//! marked `@RPCRequestType` / `@RPCReplyType`, generated and compiled
//! against the runtime header.
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
use zerodds_lang_rules::adapter::interface::ServiceMapping;
use zerodds_lang_rules::emit::encode::CodecOptions;

/// The `dds` module of §7.5.1.1.1, as a user's IDL gives it.
const COMMON: &str = "module dds {
    typedef octet GuidPrefix_t[12];
    struct EntityId_t { octet entityKey[3]; octet entityKind; };
    struct GUID_t { GuidPrefix_t guidPrefix; EntityId_t entityId; };
    struct SequenceNumber_t { long high; unsigned long low; };
    struct SampleIdentity { GUID_t writer_guid; SequenceNumber_t sequence_number; };
    module rpc {
      enum RemoteExceptionCode_t { REMOTE_EX_OK, REMOTE_EX_UNSUPPORTED,
        REMOTE_EX_INVALID_ARGUMENT, REMOTE_EX_OUT_OF_RESOURCES,
        REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION };
      struct RequestHeader { SampleIdentity requestId; string<255> instanceName; };
      struct ReplyHeader { SampleIdentity relatedRequestId; RemoteExceptionCode_t remoteEx; };
    };
};";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "pair.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast: Ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

fn cpp(idl: &str) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    cpp_in_force(idl)
}

/// As [`cpp`], with the mapping in force on the thread.
fn cpp_in_force(idl: &str) -> String {
    let ast = model(idl);
    let key = LayoutKey::data(Encoding::PlainCdr1, Endianness::Little);
    zerodds_lang_rules::generate::cpp_source_with_mapping(
        &ast,
        key,
        CodecOptions::default(),
        "zd_codec",
        ServiceMapping::Basic,
    )
    .expect("generate")
    .text
}

fn runtime() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/cpp/include")
}

/// `text` as a header, included by a translation unit, compiled with
/// `-fsyntax-only` the way a consumer includes it. `None` when there is no
/// C++ compiler.
fn compile(text: &str, extra: &str, name: &str) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("zd-type-pair-cpp-{name}"));
    std::fs::create_dir_all(&dir).expect("dir");
    let header = format!(
        "#pragma once\n#include <array>\n#include <cstdint>\n#include <memory>\n\
         #include <optional>\n#include <string>\n#include <utility>\n#include <vector>\n\
         #include \"zerodds/wire.hpp\"\n{text}\n"
    );
    std::fs::write(dir.join("zd_types.hpp"), header).expect("header");
    std::fs::write(
        dir.join("main.cpp"),
        format!("#include \"zd_types.hpp\"\n{extra}\nint main() {{ return 0; }}\n"),
    )
    .expect("main");
    let output = Command::new("c++")
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg("-I")
        .arg(runtime())
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

const BASIC: &str = "module robot {
    @RPCRequestType @DDSRequestTopic(name=\"MoveRequests\") @service(name=\"Arm\")
    struct Req { dds::rpc::RequestHeader header; long x; double speed; string label; };
    @RPCReplyType @DDSReplyTopic(name=\"MoveReplies\")
    struct Rep { dds::rpc::ReplyHeader header; long y; };
};";

#[test]
fn a_pair_with_headers_gets_traits_and_support() {
    let out = cpp(&format!("{COMMON}\n{BASIC}"));
    assert!(
        out.contains("struct dds_type_traits<::robot::Req>"),
        "{out}"
    );
    assert!(
        out.contains("struct dds_type_traits<::robot::Rep>"),
        "{out}"
    );
    assert!(out.contains("\"robot::Req\""), "{out}");
    assert!(out.contains("\"robot::Rep\""), "{out}");
    assert!(out.contains("struct ReqSupport"), "{out}");
    assert!(out.contains("Correlation::HEADER_IN_TYPE"), "{out}");
    assert!(out.contains("\"MoveRequests\""), "{out}");
    assert!(out.contains("\"MoveReplies\""), "{out}");
    assert!(out.contains("annotated_service_name(\"Arm\")"), "{out}");
    // The runtime's headers, not the user's declarations of them.
    assert!(out.contains("#include <dds/rpc.hpp>"), "{out}");
    assert!(!out.contains("struct RequestHeader"), "{out}");
}

#[test]
fn the_generated_pair_compiles_against_the_runtime() {
    let out = cpp(&format!("{COMMON}\n{BASIC}"));
    let extra = "
using Support = robot::ReqSupport;
static_assert(sizeof(Support::Requester) > 0, \"requester\");
static_assert(sizeof(Support::Replier) > 0, \"replier\");
dds::rpc::Octets zd_probe() {
  robot::Req req;
  req.x = 1;
  return dds::rpc::dds_type_traits<robot::Req>::serialize(req);
}
void zd_use(std::shared_ptr<dds::rpc::DomainParticipant> participant) {
  Support::Requester requester(Support::requester_params(participant));
  Support::Replier replier(Support::replier_params(participant));
}
";
    if let Some(errors) = compile(&out, extra, "basic") {
        assert!(errors.is_empty(), "{errors}\n{out}");
    }
}

/// The common types of §7.5.1.1.1 as the clause places them: `dds` holds
/// the identity, `dds::rpc` the rest.
const SPEC_COMMON: &str = "module dds {
    typedef octet GuidPrefix_t[12];
    struct EntityId_t { octet entityKey[3]; octet entityKind; };
    struct GUID_t { GuidPrefix_t guidPrefix; EntityId_t entityId; };
    struct SequenceNumber_t { long high; unsigned long low; };
    struct SampleIdentity { GUID_t writer_guid; SequenceNumber_t sequence_number; };
    module rpc {
      enum RemoteExceptionCode_t { REMOTE_EX_OK, REMOTE_EX_UNSUPPORTED,
        REMOTE_EX_INVALID_ARGUMENT, REMOTE_EX_OUT_OF_RESOURCES,
        REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION };
      struct RequestHeader { SampleIdentity requestId; string<255> instanceName; };
      struct ReplyHeader { SampleIdentity relatedRequestId; RemoteExceptionCode_t remoteEx; };
    };
};";

/// A type that refers to one of the common types the runtime defines has
/// the runtime's type as its member and a codec for it: the declaration is
/// the runtime's (§7.5.1.1.1 types come with it), the codec is written for
/// the shape the runtime gives, and the codecs of the members of the
/// pair's types and of an unrelated type both call it.
#[test]
fn a_type_that_refers_to_a_common_type_has_its_codec() {
    let out = cpp(&format!(
        "{SPEC_COMMON}
         module robot {{
           struct Trace {{ dds::SampleIdentity origin; dds::rpc::RemoteExceptionCode_t code; }};
           @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; Trace route; long x; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; Trace route; }};
         }};"
    ));
    // Not declared a second time.
    assert!(!out.contains("struct SampleIdentity"), "{out}");
    assert!(!out.contains("enum RemoteExceptionCode_t"), "{out}");
    // The types the runtime does not define are declared like any other.
    assert!(out.contains("struct GUID_t"), "{out}");
    // Their namespace-level swap (DDS-PSM-Cxx 1.0 §7.12) goes with them; the
    // declared types keep theirs.
    for common in ["SampleIdentity", "RequestHeader", "ReplyHeader"] {
        assert!(
            !out.contains(&format!("swap({common} &")),
            "{common}: {out}"
        );
        assert!(!out.contains(&format!("swap({common}&")), "{common}: {out}");
    }
    assert!(
        out.contains("swap(GUID_t &") || out.contains("swap(GUID_t&"),
        "{out}"
    );
    // The hand-written codec of the identity, in the runtime's shape.
    assert!(out.contains("const ::dds::SampleIdentity& v)"), "{out}");
    assert!(out.contains("v.sequence_number.high"), "{out}");
    let extra = "
dds::rpc::Octets zd_route_probe() {
  robot::Req req;
  req.route.origin.sequence_number.high = 1;
  return dds::rpc::dds_type_traits<robot::Req>::serialize(req);
}
";
    // The codecs of the declared types that nothing in this file calls (a
    // topic's support does in a consumer's build) are not what is measured.
    let out = format!("#pragma GCC diagnostic ignored \"-Wunused-function\"\n{out}");
    if let Some(errors) = compile(&out, extra, "common") {
        assert!(errors.is_empty(), "{errors}\n{out}");
    }
}

/// A nested `@appendable` or `@mutable` member of a pair type is written
/// in the form of the selected encoding (XTypes 1.3 §7.4.1.1, §7.4.1.2):
/// the codecs exist under both, and the traits choose by `encoding`.
#[test]
fn a_nested_member_of_a_pair_type_follows_the_selected_encoding() {
    let out = cpp(&format!(
        "{SPEC_COMMON}
         module robot {{
           @appendable struct Appended {{ long a; double b; }};
           @mutable struct Mutated {{ long m; @id(7) long n; }};
           @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; Appended ap; Mutated mu; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; Mutated mu; }};
         }};"
    ));
    let classic_at = out
        .find("namespace zd_pair_xcdr1")
        .expect("classic CDR codecs");
    let modern_at = out.find("namespace zd_pair {").expect("XCDR2 codecs");
    let (modern, classic) = if modern_at < classic_at {
        (&out[modern_at..classic_at], &out[classic_at..])
    } else {
        (&out[modern_at..], &out[classic_at..modern_at])
    };
    // XCDR2 delimits the appendable member and gives the mutable one
    // EMHEADERs; classic CDR has no DHEADER and a parameter list.
    assert!(modern.contains("write_dheader"), "{modern}");
    assert!(modern.contains("write_emheader"), "{modern}");
    assert!(classic.contains("no_dheader"), "{classic}");
    assert!(classic.contains("write_pid"), "{classic}");
    assert!(
        out.contains("if (encoding == WireEncoding::XCDR1)"),
        "{out}"
    );
    assert!(
        out.contains("zd_pair_xcdr1::zd_pair_enc_robot_Req"),
        "{out}"
    );
    assert!(out.contains("zd_pair::zd_pair_enc_robot_Req"), "{out}");
    let extra = "
dds::rpc::Octets zd_nested_probe(dds::rpc::WireEncoding encoding) {
  robot::Req req;
  req.mu.n = 4;
  return dds::rpc::dds_type_traits<robot::Req>::serialize_at(req, 0, encoding);
}
";
    let out = format!("#pragma GCC diagnostic ignored \"-Wunused-function\"\n{out}");
    if let Some(errors) = compile(&out, extra, "nested") {
        assert!(errors.is_empty(), "{errors}\n{out}");
    }
}

#[test]
fn a_pair_without_headers_is_the_enhanced_mapping() {
    let out = cpp("module robot {
           @RPCRequestType struct Req { long x; };
           @RPCReplyType struct Rep { long y; };
         };");
    assert!(out.contains("Correlation::SAMPLE_IDENTITY"), "{out}");
    assert!(!out.contains("encode_request_header"), "{out}");
    if let Some(errors) = compile(&out, "", "enhanced") {
        assert!(errors.is_empty(), "{errors}\n{out}");
    }
}

/// Compile `text` with a `main` that prints what `extra` prints, run it,
/// and give its standard output. `None` when there is no C++ compiler.
fn run(text: &str, extra: &str, name: &str) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("zd-type-pair-cpp-{name}"));
    std::fs::create_dir_all(&dir).expect("dir");
    let header = format!(
        "#pragma once\n#include <array>\n#include <cstdint>\n#include <cstdio>\n\
         #include <memory>\n#include <optional>\n#include <string>\n#include <utility>\n\
         #include <vector>\n#include \"zerodds/wire.hpp\"\n{text}\n"
    );
    std::fs::write(dir.join("zd_types.hpp"), header).expect("header");
    std::fs::write(
        dir.join("main.cpp"),
        format!("#include \"zd_types.hpp\"\n{extra}\n"),
    )
    .expect("main");
    let binary = dir.join("probe");
    let built = Command::new("c++")
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(runtime())
        .arg("-I")
        .arg(&dir)
        .arg(dir.join("main.cpp"))
        .args(
            ["writer", "reader", "float128", "md5"]
                .map(|unit| runtime().join(format!("../src/{unit}.cpp"))),
        )
        .arg("-o")
        .arg(&binary)
        .output();
    let built = match built {
        Err(error) => {
            eprintln!("no C++ compiler: {error}");
            return None;
        }
        Ok(built) => built,
    };
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let ran = Command::new(&binary).output().expect("run");
    assert!(
        ran.status.success(),
        "the probe exited with {:?}",
        ran.status
    );
    Some(String::from_utf8_lossy(&ran.stdout).into_owned())
}

const PROBE: &str = r#"
static void zd_print(const dds::rpc::Octets& octets) {
  for (const std::uint8_t octet : octets) {
    std::printf("%02x", octet);
  }
  std::printf("\n");
}

int main() {
  using dds::rpc::WireEncoding;
  robot::Req req;
  for (int i = 0; i < 16; ++i) {
    req.header.requestId.writer_guid[i] = static_cast<std::uint8_t>(0xA0 + i);
  }
  req.header.requestId.sequence_number.high = 1;
  req.header.requestId.sequence_number.low = 7;
  req.header.instanceName = "ab";
  req.x = 1;
  req.speed = 2.0;
  req.label = "hi";
  zd_print(dds::rpc::dds_type_traits<robot::Req>::serialize_at(req, 0, WireEncoding::XCDR1));
  zd_print(dds::rpc::dds_type_traits<robot::Req>::serialize_at(req, 0, WireEncoding::XCDR2));

  robot::Rep rep;
  for (int i = 0; i < 16; ++i) {
    rep.header.relatedRequestId.writer_guid[i] = static_cast<std::uint8_t>(0xB0 + i);
  }
  rep.header.relatedRequestId.sequence_number.low = 9;
  rep.header.remoteEx = dds::rpc::REMOTE_EX_UNKNOWN_OPERATION;
  rep.y = 5;
  zd_print(dds::rpc::dds_type_traits<robot::Rep>::serialize_at(rep, 0, WireEncoding::XCDR1));

  // Back again.
  const dds::rpc::Octets wire =
      dds::rpc::dds_type_traits<robot::Req>::serialize_at(req, 0, WireEncoding::XCDR1);
  robot::Req back;
  std::size_t at = 0;
  const bool ok = dds::rpc::dds_type_traits<robot::Req>::deserialize(wire, at, back,
                                                                    WireEncoding::XCDR1);
  std::printf("%d %zu %d %u %s %d %g %s\n", ok ? 1 : 0, at,
              back.header.requestId.sequence_number.high,
              back.header.requestId.sequence_number.low,
              back.header.instanceName.c_str(), back.x, back.speed, back.label.c_str());
  return 0;
}
"#;

fn hex(octets: &[u8]) -> String {
    octets.iter().map(|octet| format!("{octet:02x}")).collect()
}

/// DDS-RPC 1.0 §7.5.1.1.1 and §7.5.1.1: the header is the first member,
/// sixteen octets of GUID, the sequence number as `long` and
/// `unsigned long`, and `instanceName` as a CDR string; the members after
/// it align from the start of the sample, to eight under classic CDR and
/// to four under XCDR2 (XTypes 1.3 §7.4.1).
#[test]
fn the_pair_is_the_octets_the_clause_gives_in_both_encodings() {
    let text = cpp(&format!("{COMMON}\n{BASIC}"));
    let Some(out) = run(&text, PROBE, "bytes") else {
        return;
    };
    let mut lines = out.lines();
    let mut head: Vec<u8> = (0..16).map(|i| 0xA0 + i).collect();
    head.extend_from_slice(&1_i32.to_le_bytes());
    head.extend_from_slice(&7_u32.to_le_bytes());
    head.extend_from_slice(&3_u32.to_le_bytes());
    head.extend_from_slice(b"ab\0");
    // 31 octets so far: `x` pads to 32.
    let mut classic = head.clone();
    classic.push(0);
    classic.extend_from_slice(&1_i32.to_le_bytes());
    // 36 octets: the `double` pads to 40 under classic CDR, to 36 under XCDR2.
    let mut xcdr2 = classic.clone();
    classic.extend_from_slice(&[0; 4]);
    classic.extend_from_slice(&2.0_f64.to_le_bytes());
    xcdr2.extend_from_slice(&2.0_f64.to_le_bytes());
    for tail in [&mut classic, &mut xcdr2] {
        // The string `label` is four-aligned either way.
        while tail.len() % 4 != 0 {
            tail.push(0);
        }
        tail.extend_from_slice(&3_u32.to_le_bytes());
        tail.extend_from_slice(b"hi\0");
    }
    assert_eq!(lines.next(), Some(hex(&classic).as_str()), "classic CDR");
    assert_eq!(lines.next(), Some(hex(&xcdr2).as_str()), "XCDR2");
    let mut reply: Vec<u8> = (0..16).map(|i| 0xB0 + i).collect();
    reply.extend_from_slice(&0_i32.to_le_bytes());
    reply.extend_from_slice(&9_u32.to_le_bytes());
    reply.extend_from_slice(&4_u32.to_le_bytes());
    reply.extend_from_slice(&5_i32.to_le_bytes());
    assert_eq!(lines.next(), Some(hex(&reply).as_str()), "reply");
    assert_eq!(
        lines.next(),
        Some(format!("1 {} 1 7 ab 1 2 hi", classic.len()).as_str()),
        "round trip"
    );
}

const NAMES: &str = r#"
int main() {
  std::shared_ptr<dds::rpc::DomainParticipant> none;
  // The annotations alone.
  auto annotated = robot::ReqSupport::requester_params(none);
  std::printf("%s %s\n", annotated.topic_names().request.c_str(),
              annotated.topic_names().reply.c_str());
  // A run-time service name takes precedence over the annotations.
  auto runtime = robot::ReqSupport::requester_params(none);
  runtime.service_name("Other");
  std::printf("%s %s\n", runtime.topic_names().request.c_str(),
              runtime.topic_names().reply.c_str());
  // The service name an annotation gives, with no topic annotation.
  auto named = arm::PlainSupport::requester_params(none);
  std::printf("%s %s\n", named.topic_names().request.c_str(),
              named.topic_names().reply.c_str());
  return 0;
}
"#;

/// DDS-RPC 1.0 §7.4.2.3: "the run-time specification shall take precedence
/// over the IDL annotations and the default mechanism in that order".
#[test]
fn the_service_name_is_run_time_then_annotation_then_default() {
    let text = cpp(&format!(
        "{COMMON}\n{BASIC}\nmodule arm {{
           @RPCRequestType @service(name=\"Arm\")
           struct Plain {{ dds::rpc::RequestHeader header; long x; }};
           @RPCReplyType struct PlainRep {{ dds::rpc::ReplyHeader header; long y; }};
         }};"
    ));
    let Some(out) = run(&text, NAMES, "names") else {
        return;
    };
    assert_eq!(
        out,
        "MoveRequests MoveReplies\nOther_Request Other_Reply\nArm_Request Arm_Reply\n"
    );
}

/// Under `--cpp-mapping psm` a model with a pair takes the common types from the runtime as
/// well: no native declaration and no `swap` of them in `zd_native`, no PSM class, no
/// `swap` and no conversion in the PSM layer (DDS-PSM-Cxx 1.0 §7.12 adds the `swap` to every
/// structure the model declares, and these three the runtime defines without a member
/// `swap`). The native namespace names them again, so the native structures that hold one
/// find the runtime's type; the types the model declares keep class and `swap`.
#[test]
fn the_psm_layer_leaves_out_the_common_types_of_a_pair() {
    let _scope = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Psm,
    );
    let out = cpp_in_force(&format!(
        "{SPEC_COMMON}
         module robot {{
           struct Trace {{ dds::SampleIdentity origin; dds::rpc::RemoteExceptionCode_t code; }};
           @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; Trace route; long x; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; Trace route; }};
         }};"
    ));
    for common in ["SampleIdentity", "RequestHeader", "ReplyHeader"] {
        for spelling in [
            format!("swap({common}&"),
            format!("swap({common} &"),
            format!("class {common}"),
            format!("struct {common} "),
        ] {
            assert!(!out.contains(&spelling), "{spelling}: {out}");
        }
        assert!(
            out.contains(&format!("using ::dds::{common};"))
                || out.contains(&format!("using ::dds::rpc::{common};")),
            "{common}: {out}"
        );
    }
    // The types the model declares keep their PSM class and `swap`.
    assert!(out.contains("class Trace"), "{out}");
    assert!(
        out.contains("swap(Trace&") || out.contains("swap(Trace &"),
        "{out}"
    );
    assert!(
        out.contains("swap(GUID_t&") || out.contains("swap(GUID_t &"),
        "{out}"
    );
}
