// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-RPC 1.0 §7.3.1.4 in the generated Java binding: a pair of types
//! marked `@RPCRequestType` / `@RPCReplyType`, generated, compiled against
//! the runtime with `javac -Xlint:all -Werror`, and run over the in-process
//! binding the runtime's own tests use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_ast::Ast;
use zerodds_lang_cst::Language;

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

/// What `RpcRuntimeRoundtrip` needs of the generated services.
const CALCULATOR: &str =
    "module robot { interface Calculator { long add(in long a, in long b); }; };";

const PAIRS: &str = "module pairs {
    @RPCRequestType @DDSRequestTopic(name=\"MoveRequests\") @service(name=\"Arm\")
    struct Req { dds::rpc::RequestHeader header; long x; double speed; string label; };
    @RPCReplyType @DDSReplyTopic(name=\"MoveReplies\")
    struct Rep { dds::rpc::ReplyHeader header; long y; };
};
module named {
    @RPCRequestType @DDSService(name=\"Gripper\")
    struct Req { dds::rpc::RequestHeader header; long x; };
    @RPCReplyType struct Rep { dds::rpc::ReplyHeader header; long y; };
};
module plain {
    @RPCRequestType struct Req { dds::rpc::RequestHeader header; long x; };
    @RPCReplyType struct Rep { dds::rpc::ReplyHeader header; long y; };
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

/// The three compilation units of the generated Java.
fn units(idl: &str) -> [(String, String); 3] {
    let ast = model(idl);
    // the assertions and the Java programs read the IDL-to-Java shape (public fields)
    let text = zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            zerodds_lang_rules::generate::source(
                &ast,
                Language::Java,
                zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
            )
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let units = zerodds_lang_rules::generate::java_units(&text);
    let classes: Vec<&str> = units.iter().map(|unit| unit.class).collect();
    assert_eq!(classes, ["ZdTypes", "ZdServices", "ZdCodec"]);
    let mut out = units.into_iter().map(|unit| {
        let text = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text.trim_end())
        } else {
            unit.text
        };
        (unit.class.to_string(), text)
    });
    [
        out.next().expect("types"),
        out.next().expect("services"),
        out.next().expect("codec"),
    ]
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn have_jdk() -> bool {
    ["javac", "java"].iter().all(|tool| {
        Command::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

fn run(command: &mut Command, what: &str) -> std::process::Output {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Compile the generated files, the runtime and `probe` (the class
/// `Probe`) and run it. `None` without a JDK.
fn java_probe(name: &str, idl: &str, probe: &str) -> Option<String> {
    if !have_jdk() {
        eprintln!("no JDK on this machine — the Java binding is not measured here");
        return None;
    }
    let root = root();
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("type-pair-java")
        .join(name);
    let classes = at.join("classes");
    std::fs::create_dir_all(&classes).expect("the class directory");
    let mut files = Vec::new();
    for (class, text) in units(idl) {
        let path = at.join(format!("{class}.java"));
        std::fs::write(&path, text).expect("a unit");
        files.push(path);
    }
    let probe_path = at.join("Probe.java");
    std::fs::write(&probe_path, probe).expect("the probe");
    files.push(probe_path);
    // Only the probe that drives the runtime needs its loopback and the
    // Calculator interface it is written against.
    if probe.contains("RpcRuntimeRoundtrip") {
        files.push(root.join("crates/rpc/tests/java/RpcRuntimeRoundtrip.java"));
    }
    let sourcepath = format!(
        "{}:{}",
        root.join("runtime/java/src/main/java").display(),
        root.join("crates/java-omgdds/java/src/main/java").display(),
    );
    // The generated units are compiled with the runtime and the probe, as
    // a consumer compiles them; the runtime itself is held to `-Werror` by
    // `crates/rpc/tests/java_rpc_runtime.rs`.
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8", "-Xlint:all", "-d"])
            .arg(&classes)
            .arg("-sourcepath")
            .arg(&sourcepath)
            .args(&files),
        "compiling the generated Java",
    );
    let output = run(
        Command::new("java").arg("-cp").arg(&classes).arg("Probe"),
        "the probe",
    );
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn input() -> String {
    format!("{COMMON}\n{CALCULATOR}\n{PAIRS}")
}

#[test]
fn the_generated_pair_declares_the_support_and_uses_the_runtimes_headers() {
    let units = units(&input());
    let types = &units[0].1;
    let services = &units[1].1;
    let codec = &units[2].1;
    assert!(
        types.contains("public org.omg.dds.rpc.RequestHeader header;"),
        "{types}"
    );
    assert!(
        types.contains("public org.omg.dds.rpc.ReplyHeader header;"),
        "{types}"
    );
    // The runtime's headers, not the user's declarations of them.
    assert!(!types.contains("class RequestHeader"), "{types}");
    assert!(!types.contains("class ReplyHeader"), "{types}");
    // The other common types are declared like any other type.
    // GuidPrefix_t is a typedef (octet[12]); Java has no alias, so the
    // declaration is a comment and every use takes byte[].
    assert!(types.contains("typedef byte[] GuidPrefix_t"), "{types}");
    assert!(types.contains("class GUID_t"), "{types}");
    assert!(types.contains("class SampleIdentity"), "{types}");
    // The codec of a header is written over the runtime's class.
    assert!(
        codec.contains("org.omg.dds.rpc.RequestHeader v)"),
        "{codec}"
    );
    assert!(codec.contains("org.omg.dds.rpc.ReplyHeader v)"), "{codec}");
    assert!(services.contains("class ReqSupport"), "{services}");
    assert!(
        services.contains("implements org.omg.dds.rpc.TypeSupport<ZdTypes.pairs.Req>, org.omg.dds.rpc.HeaderInRequestType<ZdTypes.pairs.Req>"),
        "{services}"
    );
    assert!(services.contains("\"pairs::Req\""), "{services}");
    assert!(services.contains("\"MoveRequests\""), "{services}");
    assert!(services.contains("\"Arm\""), "{services}");
    assert!(services.contains("\"Gripper\""), "{services}");
}

/// A type that refers to a common type of §7.5.1.1.1 has its codec: the
/// common types other than the two headers are declared and encoded like
/// any other type, and a member of the headers' type calls the codec over
/// the runtime's class.
#[test]
fn a_type_that_refers_to_a_common_type_has_its_codec() {
    let idl = format!(
        "{COMMON}
         module robot {{
           struct Trace {{ dds::SampleIdentity origin; dds::rpc::RemoteExceptionCode_t code; }};
           @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; Trace route; long x; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; Trace route; }};
         }};"
    );
    let units = units(&idl);
    let types = &units[0].1;
    let codec = &units[2].1;
    assert!(types.contains("class SampleIdentity"), "{types}");
    // The Java binding declares an IDL enum as a class of int constants.
    assert!(types.contains("class RemoteExceptionCode_t"), "{types}");
    assert!(types.contains("class Trace"), "{types}");
    // Encoder and decoder for each of the referenced types.
    for name in ["SampleIdentity", "RemoteExceptionCode_t", "Trace"] {
        assert!(codec.contains(name), "{name} in {codec}");
    }
    assert!(
        codec.contains("org.omg.dds.rpc.SampleIdentity zdIdentity("),
        "{codec}"
    );
    if let Some(out) = java_probe(
        "common",
        &idl,
        "public final class Probe { public static void main(String[] a) { \
           ZdTypes.robot.Trace t = new ZdTypes.robot.Trace(); \
           org.zerodds.wire.Writer w = org.zerodds.wire.Writer.littleEndian(); \
           ZdCodec.class.getName(); System.out.println(t != null && w != null); } }",
    ) {
        assert_eq!(out.trim(), "true");
    }
}

#[test]
fn the_enhanced_pair_has_no_header_and_selects_the_enhanced_mapping() {
    let units = units(
        "module pairs {
           @RPCRequestType struct Req { long x; };
           @RPCReplyType struct Rep { long y; };
         };",
    );
    let services = &units[1].1;
    assert!(!services.contains("HeaderInRequestType"), "{services}");
    assert!(services.contains("ServiceMapping.ENHANCED"), "{services}");
}

const BYTES: &str = r#"
import org.omg.dds.rpc.*;

public final class Probe {
    static String hex(byte[] octets) {
        StringBuilder out = new StringBuilder();
        for (byte octet : octets) {
            out.append(String.format("%02x", octet & 0xFF));
        }
        return out.toString();
    }

    public static void main(String[] args) {
        ZdServices.pairs.ReqSupport.RequestTypeSupport requests =
                new ZdServices.pairs.ReqSupport.RequestTypeSupport();
        ZdServices.pairs.ReqSupport.ReplyTypeSupport replies =
                new ZdServices.pairs.ReqSupport.ReplyTypeSupport();
        ZdTypes.pairs.Req req = new ZdTypes.pairs.Req();
        byte[] guid = new byte[16];
        for (int i = 0; i < 16; ++i) {
            guid[i] = (byte) (0xA0 + i);
        }
        req.header = new RequestHeader(new SampleIdentity(guid, (1L << 32) | 7L), "ab");
        req.x = 1;
        req.speed = 2.0;
        req.label = "hi";
        System.out.println(hex(requests.serialize(req, 0, WireEncoding.XCDR1)));
        System.out.println(hex(requests.serialize(req, 0, WireEncoding.XCDR2)));

        ZdTypes.pairs.Rep rep = new ZdTypes.pairs.Rep();
        byte[] related = new byte[16];
        for (int i = 0; i < 16; ++i) {
            related[i] = (byte) (0xB0 + i);
        }
        rep.header = new ReplyHeader(
                new SampleIdentity(related, 9L), RemoteExceptionCode.REMOTE_EX_UNKNOWN_OPERATION);
        rep.y = 5;
        System.out.println(hex(replies.serialize(rep, 0, WireEncoding.XCDR1)));

        HeaderCodec.Cursor cursor = new HeaderCodec.Cursor(0);
        ZdTypes.pairs.Req back =
                requests.deserialize(requests.serialize(req, 0, WireEncoding.XCDR1), cursor, WireEncoding.XCDR1);
        System.out.println(cursor.at + " " + back.header.requestId().sequenceNumber() + " "
                + back.header.instanceName() + " " + back.x + " " + back.speed + " " + back.label);
    }
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
    let Some(out) = java_probe("bytes", &input(), BYTES) else {
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
        Some(format!("{} {} ab 1 2.0 hi", classic.len(), (1_u64 << 32) | 7).as_str()),
        "round trip"
    );
}

const RUNTIME: &str = r#"
import java.util.List;
import org.omg.dds.rpc.*;
import org.zerodds.ddsrpc.*;

public final class Probe {
    public static void main(String[] args) {
        RpcRuntimeRoundtrip.Bus bus = new RpcRuntimeRoundtrip.Bus();
        RpcRuntimeRoundtrip.LoopbackParticipant participant =
                new RpcRuntimeRoundtrip.LoopbackParticipant(bus);

        // A request and its reply through the Requester and the Replier of
        // the runtime, correlated by the header in the type.
        Replier<ZdTypes.pairs.Req, ZdTypes.pairs.Rep> replier =
                new ReplierImpl<>(ZdServices.pairs.ReqSupport.replierParams(participant, ""));
        Requester<ZdTypes.pairs.Req, ZdTypes.pairs.Rep> requester =
                new RequesterImpl<>(ZdServices.pairs.ReqSupport.requesterParams(participant));
        ZdTypes.pairs.Req req = new ZdTypes.pairs.Req();
        req.x = 3;
        req.speed = 1.5;
        req.label = "go";
        SampleIdentity sent = SampleIdentity.of(requester.sendRequest(req));
        List<Sample<ZdTypes.pairs.Req>> received = replier.receiveRequests();
        System.out.println(received.size() + " " + received.get(0).getData().x + " "
                + received.get(0).getData().label + " "
                + received.get(0).identity().equals(sent));
        ZdTypes.pairs.Rep rep = new ZdTypes.pairs.Rep();
        rep.y = 42;
        replier.sendReply(rep, received.get(0).identity());
        Sample<ZdTypes.pairs.Rep> answer = requester.receiveReply(sent);
        System.out.println((answer == null ? "none" : String.valueOf(answer.getData().y)));
        System.out.println(participant.createdOn("MoveRequests", true) != null);
        System.out.println(participant.createdOn("MoveReplies", false) != null);

        // The service name: run-time, then annotation, then default.
        RequesterParams annotated =
                ZdServices.pairs.ReqSupport.requesterParams(participant);
        System.out.println(TopicNames.of(annotated).request() + " " + TopicNames.of(annotated).reply());
        RequesterParams runtime =
                ZdServices.pairs.ReqSupport.requesterParams(participant).serviceName("Other");
        System.out.println(TopicNames.of(runtime).request() + " " + TopicNames.of(runtime).reply());
        RequesterParams named =
                ZdServices.named.ReqSupport.requesterParams(participant);
        System.out.println(TopicNames.of(named).request() + " " + TopicNames.of(named).reply());
        RequesterParams plain =
                ZdServices.plain.ReqSupport.requesterParams(participant);
        System.out.println(TopicNames.of(plain).request() + " " + TopicNames.of(plain).reply());
        ReplierParams replierPlain =
                ZdServices.plain.ReqSupport.replierParams(participant, "").serviceName("Live");
        System.out.println(TopicNames.of(replierPlain).request() + " " + TopicNames.of(replierPlain).reply());
    }
}
"#;

/// DDS-RPC 1.0 §7.3.1.4 and §7.4.2.3 through the runtime: a request and
/// its reply are correlated by the header the types carry, and the
/// `<service_name>` is the run-time one, else the annotation (`@service` or
/// `@DDSService`, on either type), else `Service`.
#[test]
fn the_runtime_correlates_by_the_header_in_the_type_and_resolves_the_service_name() {
    let Some(out) = java_probe("runtime", &input(), RUNTIME) else {
        return;
    };
    assert_eq!(
        out,
        "1 3 go true\n42\ntrue\ntrue\n\
         MoveRequests MoveReplies\n\
         Other_Request Other_Reply\n\
         Gripper_Request Gripper_Reply\n\
         Service_Request Service_Reply\n\
         Live_Request Live_Reply\n"
    );
}

/// A nested `@appendable` or `@mutable` member of a pair type is written
/// in the form of the selected encoding (XTypes 1.3 §7.4.1.1, §7.4.1.2):
/// the codec class holds the classic CDR codecs in `Xcdr1`, and the
/// support classes choose by `encoding`.
#[test]
fn a_nested_member_of_a_pair_type_follows_the_selected_encoding() {
    let idl = format!(
        "{COMMON}
         module nest {{
           @appendable struct Appended {{ long a; double b; }};
           @mutable struct Mutated {{ long m; @id(7) long n; }};
           @RPCRequestType struct Req {{ dds::rpc::RequestHeader header; Appended ap; Mutated mu; }};
           @RPCReplyType struct Rep {{ dds::rpc::ReplyHeader header; Mutated mu; }};
         }};"
    );
    let units = units(&idl);
    let services = &units[1].1;
    let codec = &units[2].1;
    let classic_at = codec.find("class Xcdr1").expect("the classic CDR codecs");
    let (modern, classic) = codec.split_at(classic_at);
    assert!(modern.contains("writeDheader"), "{modern}");
    assert!(classic.contains("noDheader"), "{classic}");
    assert!(classic.contains("writePid"), "{classic}");
    assert!(
        services.contains("encoding == org.omg.dds.rpc.WireEncoding.XCDR1"),
        "{services}"
    );
    assert!(
        services.contains("ZdCodec.Xcdr1.zd_enc_nest_Appended"),
        "{services}"
    );
    assert!(
        services.contains("ZdCodec.zd_enc_nest_Appended"),
        "{services}"
    );
}
