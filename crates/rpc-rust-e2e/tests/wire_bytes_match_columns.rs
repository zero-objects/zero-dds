// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The three columns put the same Request and Reply on the wire, octet for
//! octet, for members of eight octets.
//!
//! DDS-RPC 1.0 §7.5.1.1: the Basic Service Mapping is classic CDR (XCDR1),
//! where an 8-octet member aligns to 8 counted from the start of the
//! sample (XTypes 1.3 §7.4.1.1), and XCDR2 aligns it to 4. The sample is
//! the header, the padding that aligns the union to 4, and the union
//! (§7.5.1.1.6 rule 2), so with instance names of different lengths the
//! union begins at 0 or 4 modulo 8, and the reply's at 28.
//!
//! `wire_bytes_match_cpp.rs` does this for two `long`s, where the encodings
//! do not differ. Here the Rust column is the generated codec through the
//! `DdsType` methods the runtime calls, the C++ column the generated
//! `serialize_at`, the Java column the generated `serialize(value, offset,
//! encoding)`; each prefixes the header its own runtime writes. Besides
//! the comparison, the lengths of four samples are given by the clause.
//!
//! The same is done for nested `@appendable` and `@mutable` members, in an
//! interface's topic types (`Store`) and in the pair of types of DDS-RPC
//! 1.0 §7.3.1.4 (`PReq`, `PRep`): under XCDR1 an `@appendable` member has
//! no DHEADER and a `@mutable` one is a parameter list; under XCDR2 they
//! have a DHEADER and EMHEADERs (XTypes 1.3 §7.4.1.1, §7.4.1.2). The Rust
//! column of the pair is the header's octets followed by the generated
//! codecs of the members, written behind it in one writer.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_dcps::dds_type::DdsType;
use zerodds_rpc::common_types::{RemoteExceptionCode, ReplyHeader, RequestHeader, SampleIdentity};
use zerodds_rpc::wire_codec::{encode_reply_frame, encode_request_frame};
use zerodds_rpc_rust_e2e::nested_bindings::nested::{
    Appended, Mutated, Store_Call, Store_Return, Store_get_Out, Store_get_Result, Store_put_In,
};
use zerodds_rpc_rust_e2e::wide_bindings::wide::{
    W_Call, W_Return, W_mix_In, W_mix_Out, W_mix_Result, W_scale_In, W_scale_Out, W_scale_Result,
};
use zerodds_wire::{Endianness, Writer};

/// The IDL of `build.rs`'s `WIDE`.
const WIDE: &str = "
module wide {
  @DDSService interface W {
    double scale(in double d, in long long n, in unsigned long long u);
    unsigned long long mix(in octet o, in unsigned long long u);
  };
};
";

/// The common types of §7.5.1.1.1, nested members and a pair of types over
/// them: `module nested` is the IDL of `build.rs`'s `NESTED` plus the pair.
const NESTED: &str = "
module dds {
  typedef octet GuidPrefix_t[12];
  struct EntityId_t { octet entityKey[3]; octet entityKind; };
  struct GUID_t { GuidPrefix_t guidPrefix; EntityId_t entityId; };
  struct SequenceNumber_t { long high; unsigned long low; };
  struct SampleIdentity { GUID_t writer_guid; SequenceNumber_t sequence_number; };
  module rpc {
    enum RemoteExceptionCode_t {
      REMOTE_EX_OK, REMOTE_EX_UNSUPPORTED, REMOTE_EX_INVALID_ARGUMENT,
      REMOTE_EX_OUT_OF_RESOURCES, REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION
    };
    struct RequestHeader { SampleIdentity requestId; string<255> instanceName; };
    struct ReplyHeader { SampleIdentity relatedRequestId; RemoteExceptionCode_t remoteEx; };
  };
};
module nested {
  @appendable struct Appended { long a; double b; };
  @mutable struct Mutated { long m; @id(7) long n; };
  @DDSService interface Store {
    long put(in Appended a, in Mutated m);
    Mutated get();
  };
  @RPCRequestType struct PReq { dds::rpc::RequestHeader header; Appended ap; Mutated mu; };
  @RPCReplyType struct PRep { dds::rpc::ReplyHeader header; Appended ap; Mutated mu; };
};
";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn build_directory(name: &str) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&at).expect("the build directory");
    at
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn ast() -> zerodds_ast::Ast {
    let source = format!("{WIDE}{NESTED}");
    let parsed = zerodds_idl_parse::parse(&source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "wide.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    // The header reads the fields of the codec's structures: the native mapping.
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    ast
}

/// `<kind> <encoding> <name length> <operation>` -> the sample's octets.
type Samples = BTreeMap<String, String>;

fn parse(text: &str) -> Samples {
    text.lines()
        .map(|line| {
            let (key, sample) = line.rsplit_once(' ').expect("a line with a sample");
            (key.to_string(), sample.to_string())
        })
        .collect()
}

fn identity() -> SampleIdentity {
    SampleIdentity::new([0x42; 16], 0x0102_0304_0506_0708)
}

/// The Rust column: the generated codec behind the runtime's header, as
/// `Requester::frame_request` and `Replier::write_reply` assemble it.
fn rust_samples() -> Samples {
    let mut out = rust_nested_samples();
    let scale = W_Call::Scale(W_scale_In {
        d: 1.5,
        n: -2,
        u: 0x1122_3344_5566_7788,
    });
    let mix = W_Call::Mix(W_mix_In {
        o: 0x7f,
        u: 0x0102_0304_0506_0708,
    });
    for (name, xcdr1) in [("xcdr1", true), ("xcdr2", false)] {
        for length in [0usize, 1, 3, 4, 5] {
            let header = RequestHeader::new(identity(), "n".repeat(length));
            for (operation, call) in [("scale", &scale), ("mix", &mix)] {
                let mut frame = encode_request_frame(&header, &[]).expect("the header");
                call.encode_continuing(&mut frame, xcdr1)
                    .expect("the union");
                out.insert(format!("request {name} {length} {operation}"), hex(&frame));
            }
        }
        let reply_header = ReplyHeader::new(identity(), RemoteExceptionCode::Ok);
        let replies = [
            (
                "scale",
                W_Return::Scale(W_scale_Result::Ok(W_scale_Out { return_: 3.0 })),
            ),
            (
                "mix",
                W_Return::Mix(W_mix_Result::Ok(W_mix_Out {
                    return_: 0xAABB_CCDD_EEFF_0011,
                })),
            ),
        ];
        for (operation, reply) in replies {
            let mut frame = encode_reply_frame(&reply_header, &[]);
            reply
                .encode_continuing(&mut frame, xcdr1)
                .expect("the union");
            out.insert(format!("reply {name} 0 {operation}"), hex(&frame));
        }
    }
    out
}

fn appended() -> Appended {
    Appended { a: 1, b: 2.0 }
}

fn mutated() -> Mutated {
    Mutated { m: 3, n: 4 }
}

/// The Rust column of the nested members: the interface's topic types
/// behind the runtime's header, and the pair of types, whose octets are the
/// header's followed by the members' codecs.
fn rust_nested_samples() -> Samples {
    let mut out = Samples::new();
    let put = Store_Call::Put(Store_put_In {
        a: appended(),
        m: mutated(),
    });
    let get = Store_Return::Get(Store_get_Result::Ok(Store_get_Out { return_: mutated() }));
    for (name, xcdr1) in [("xcdr1", true), ("xcdr2", false)] {
        let align = if xcdr1 { 8 } else { 4 };
        for length in [0usize, 3] {
            let header = RequestHeader::new(identity(), "n".repeat(length));
            let mut frame = encode_request_frame(&header, &[]).expect("the header");
            put.encode_continuing(&mut frame, xcdr1).expect("the union");
            out.insert(format!("request {name} {length} put"), hex(&frame));

            let mut w = Writer::with_max_align(Endianness::Little, align);
            w.write_bytes(&header.to_cdr_le().expect("the header"));
            write_members(&mut w, xcdr1);
            out.insert(format!("request {name} {length} pair"), hex(&w.finish()));
        }
        let reply_header = ReplyHeader::new(identity(), RemoteExceptionCode::Ok);
        let mut frame = encode_reply_frame(&reply_header, &[]);
        get.encode_continuing(&mut frame, xcdr1).expect("the union");
        out.insert(format!("reply {name} 0 get"), hex(&frame));

        let mut w = Writer::with_max_align(Endianness::Little, align);
        w.write_bytes(&reply_header.to_cdr_le());
        write_members(&mut w, xcdr1);
        out.insert(format!("reply {name} 0 pair"), hex(&w.finish()));
    }
    out
}

/// The members of `PReq` and `PRep` behind the header.
fn write_members(w: &mut Writer, xcdr1: bool) {
    if xcdr1 {
        appended().encode_xcdr1(w).expect("the appendable member");
        mutated().encode_xcdr1(w).expect("the mutable member");
    } else {
        appended().encode(w).expect("the appendable member");
        mutated().encode(w).expect("the mutable member");
    }
}

fn compiler() -> Option<&'static str> {
    ["c++", "clang++", "g++"].into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

fn cpp_samples(compiler: &str) -> Samples {
    // The header reads the fields of the codec's structures: the native mapping.
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let root = repository_root();
    let at = build_directory("wire-bytes-match-columns-cpp");
    let body = zerodds_lang_rules::generate::source(
        &ast(),
        zerodds_lang_cst::Language::Cpp,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut header = String::from("#pragma once\n\n#include <dds/rpc.hpp>\n");
    for include in zerodds_lang_rules::adapter::cpp::includes() {
        header.push_str(&format!("#include <{}>\n", include.path));
    }
    header.push('\n');
    header.push_str(&body);
    header.push('\n');
    std::fs::write(at.join("wide.hpp"), header).expect("the generated header");
    let binary = at.join("wide_wire");
    run(
        Command::new(compiler)
            .arg("-std=c++17")
            .args(["-Wall", "-Wextra", "-Werror"])
            .arg(format!("-I{}", at.display()))
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(root.join("crates/rpc-rust-e2e/tests/cpp/wide_wire.cpp"))
            .args(
                ["writer", "reader", "float128", "md5"]
                    .map(|unit| root.join(format!("runtime/cpp/src/{unit}.cpp"))),
            )
            .arg("-o")
            .arg(&binary),
        "compiling wide_wire.cpp",
    );
    let output = run(&mut Command::new(&binary), "wide_wire");
    parse(&String::from_utf8_lossy(&output.stdout))
}

fn java_available() -> bool {
    let present = ["javac", "java"].iter().all(|tool| {
        Command::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    assert!(
        present || std::env::var("ZERODDS_REQUIRE_JDK").as_deref() != Ok("1"),
        "ZERODDS_REQUIRE_JDK=1 and no `javac`/`java` on PATH"
    );
    present
}

fn java_samples() -> Samples {
    let root = repository_root();
    let at = build_directory("wire-bytes-match-columns-java");
    let classes = at.join("classes");
    std::fs::create_dir_all(&classes).expect("the class directory");

    // the Java program reads the IDL-to-Java shape (public fields)
    let text = zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || {
            zerodds_lang_rules::generate::source(
                &ast(),
                zerodds_lang_cst::Language::Java,
                zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
            )
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let units = zerodds_lang_rules::generate::java_units(&text);
    let classes_found: Vec<&str> = units.iter().map(|unit| unit.class).collect();
    assert_eq!(classes_found, ["ZdTypes", "ZdServices", "ZdCodec"]);
    for unit in &units {
        let body = if unit.class == "ZdCodec" {
            format!("import org.zerodds.wire.*;\n\n{}", unit.text.trim_end())
        } else {
            unit.text.clone()
        };
        std::fs::write(at.join(format!("{}.java", unit.class)), body).expect("a generated unit");
    }

    let sourcepath = format!(
        "{}:{}:{}",
        root.join("runtime/java/src/main/java").display(),
        root.join("crates/java-omgdds/java/src/main/java").display(),
        at.display(),
    );
    run(
        Command::new("javac")
            .args(["-encoding", "UTF-8"])
            .arg("-d")
            .arg(&classes)
            .arg("-sourcepath")
            .arg(&sourcepath)
            .arg(at.join("ZdTypes.java"))
            .arg(at.join("ZdServices.java"))
            .arg(at.join("ZdCodec.java"))
            .arg(root.join("crates/rpc-rust-e2e/tests/java/WideWire.java")),
        "compiling the generated Java files and WideWire",
    );
    let output = run(
        Command::new("java")
            .arg("-cp")
            .arg(&classes)
            .arg("WideWire"),
        "WideWire",
    );
    parse(&String::from_utf8_lossy(&output.stdout))
}

fn assert_same(name: &str, left: &Samples, right: &Samples) {
    assert_eq!(
        left.keys().collect::<Vec<_>>(),
        right.keys().collect::<Vec<_>>(),
        "{name}: the same samples"
    );
    for (key, octets) in left {
        assert_eq!(octets, &right[key], "{name}: {key}");
    }
}

/// XTypes 1.3 §7.4.1.1 and DDS-RPC 1.0 §7.5.1.1.6 rule 2, worked by hand
/// for `scale(double, long long, unsigned long long)`:
///
/// * instance name of 0 octets: header 29, padded to 32; the union's
///   discriminator 32..36; the double at 40 under XCDR1 (padding 36..40)
///   and at 36 under XCDR2; the two 64-bit members follow: 64 and 60
///   octets in all;
/// * instance name of 4 octets: header 33, padded to 36; discriminator
///   36..40; the double is at 40 under both; 64 octets in all.
#[test]
fn the_rust_samples_have_the_lengths_the_clause_gives() {
    let samples = rust_samples();
    let length = |key: &str| samples[key].len() / 2;
    assert_eq!(length("request xcdr1 0 scale"), 64);
    assert_eq!(length("request xcdr2 0 scale"), 60);
    assert_eq!(length("request xcdr1 4 scale"), 64);
    assert_eq!(length("request xcdr2 4 scale"), 64);
    // The reply's union begins at 28: discriminator 28..32, the `_d` of the
    // result 32..36, the double at 40 (XCDR1) or 36 (XCDR2).
    assert_eq!(length("reply xcdr1 0 scale"), 48);
    assert_eq!(length("reply xcdr2 0 scale"), 44);
}

#[test]
fn the_rust_and_cpp_columns_put_eight_octet_members_where_the_same_octets() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the wire comparison is not measured here");
        return;
    };
    assert_same("rust against c++", &rust_samples(), &cpp_samples(compiler));
}

#[test]
fn the_rust_and_java_columns_put_eight_octet_members_where_the_same_octets() {
    if !java_available() {
        println!("no JDK on this machine — the wire comparison is not measured here");
        return;
    }
    assert_same("rust against java", &rust_samples(), &java_samples());
}

#[allow(dead_code)]
fn _paths(_: &Path) {}
