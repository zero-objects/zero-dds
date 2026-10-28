// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The gate that was missing: **the frame an endpoint SDK builds, parsed by
//! `crates/xrce`.**
//!
//! Every language port here builds one `WRITE_DATA` frame with its own
//! endpoint SDK, prints it as hex, and this test hands those exact octets to
//! [`zerodds_xrce::submessages::WriteDataPayload::try_from_submessage`] — the
//! same decoder an XRCE agent runs. Nothing in between re-states the layout,
//! so a port cannot drift from the crate without a red test.
//!
//! Why it did not exist: the peer in `zerodds_endpoint_e2e` had its own
//! hand-written copy of the frame layout, and `endpoints/golden-gen` built its
//! `WRITE_DATA` submessage by hand as well. Both spoke the ports' dialect
//! (`flags = 0x03`, body = the bare sample), so every port matched a reference
//! that agreed with it and the crate's own codec was never on the path. Both
//! now go through the crate's codec; this file is the direct, per-language
//! check on top of that.
//!
//! A missing toolchain skips its case with a printed reason. A toolchain that
//! is present but fails to build our own source **fails** — it never skips.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_endpoint_e2e::codegen::{endpoints_dir, scratch_dir, tool_available};
use zerodds_xrce::object_id::ObjectId;
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::submessages::{Message, SubmessageId, WriteDataPayload};

/// The `sequence_nr` every driver frames with; it doubles as the
/// `BaseObjectRequest.request_id` (§7.7.8 asks only for per-stream
/// uniqueness).
const SEQ: u16 = 1;
/// Raw id (12 bits) of the DataWriter every driver addresses.
const RAW_ID: u16 = 0x001;
/// The sample body every driver frames.
const SAMPLE: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

/// The `ObjectId` the drivers pass — raw `0x001` + `OBJK_DATAWRITER`.
fn writer_id() -> ObjectId {
    ObjectId::new(RAW_ID, ObjectKind::DataWriter).expect("raw id fits 12 bits")
}

/// Decodes `hex` and asserts the frame is a conformant `WRITE_DATA` for
/// [`SEQ`] / [`writer_id`] / [`SAMPLE`]. `label` names the language.
fn assert_conformant(label: &str, hex: &str) {
    let frame = decode_hex(label, hex);
    let message = Message::decode(&frame)
        .unwrap_or_else(|e| panic!("{label}: crates/xrce refused the message: {e}"));
    assert_eq!(
        message.header.sequence_nr.0, SEQ,
        "{label}: message header sequence_nr"
    );
    let sm = message
        .submessages
        .first()
        .unwrap_or_else(|| panic!("{label}: no submessage"));
    assert_eq!(
        sm.header.submessage_id,
        SubmessageId::WriteData,
        "{label}: submessage id"
    );
    let payload = WriteDataPayload::try_from_submessage(sm).unwrap_or_else(|e| {
        panic!(
            "{label}: crates/xrce refused the WRITE_DATA submessage \
             (flags {:#04x}, body {} octets): {e}",
            sm.header.flags,
            sm.body.len()
        )
    });
    assert_eq!(
        sm.header.flags,
        WriteDataPayload::flags(),
        "{label}: E-flag LE + DataFormat FORMAT_DATA"
    );
    assert_eq!(
        payload.base.request_id,
        SEQ.to_be_bytes(),
        "{label}: BaseObjectRequest.request_id"
    );
    assert_eq!(
        payload.base.object_id,
        writer_id(),
        "{label}: BaseObjectRequest.object_id — an agent routes on this"
    );
    assert_eq!(
        payload.serialized_data,
        SAMPLE.to_vec(),
        "{label}: SampleData.serialized_data"
    );
}

/// Parses the driver's stdout: whitespace-insensitive lowercase/uppercase hex.
fn decode_hex(label: &str, text: &str) -> Vec<u8> {
    let digits: Vec<char> = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    assert!(
        !digits.is_empty() && digits.len() % 2 == 0,
        "{label}: driver printed {digits:?} hex digits, not a byte string:\n{text}"
    );
    digits
        .chunks(2)
        .map(|pair| {
            let byte: String = pair.iter().collect();
            u8::from_str_radix(&byte, 16).expect("hex digit pair")
        })
        .collect()
}

/// Runs `command`, failing the test (never skipping) if it does not succeed.
/// Returns its stdout.
fn run(label: &str, command: &mut Command) -> String {
    let out = command
        .output()
        .unwrap_or_else(|e| panic!("{label}: spawn: {e}"));
    assert!(
        out.status.success(),
        "{label}: {command:?} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Writes `text` to `dir/name` and returns the path.
fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    path
}

// =====================================================================
// The counter-proof: the dialect the ports spoke before this gate existed
// =====================================================================

/// The exact frame the ports built until this change: `flags = 0x03`
/// (`FORMAT_SAMPLE`) and a body that is the bare sample, with no
/// `BaseObjectRequest`. `crates/xrce` refuses it — which is what makes the
/// gate above capable of going red. Without this, a gate that never rejects
/// anything would look identical to one that works.
#[test]
fn the_pre_conformance_dialect_is_refused() {
    let legacy = "8001010007030400deadbeef";
    let frame = decode_hex("legacy", legacy);
    let message = Message::decode(&frame).expect("the envelope alone still parses");
    let sm = message.submessages.first().expect("one submessage");
    assert_eq!(sm.header.flags, 0x03, "the dialect's flag byte");
    let refused = WriteDataPayload::try_from_submessage(sm);
    assert!(
        refused.is_err(),
        "the pre-conformance frame must be refused, not decoded"
    );
}

/// And the same octets fail `assert_conformant`, i.e. the gate itself is red
/// on them. Asserted by catching the panic, so the proof lives in the file
/// rather than in a reviewer's memory of a one-off experiment.
#[test]
fn the_gate_fails_on_the_pre_conformance_dialect() {
    let outcome = std::panic::catch_unwind(|| {
        assert_conformant("counter-proof", "8001010007030400deadbeef");
    });
    assert!(
        outcome.is_err(),
        "assert_conformant accepted the pre-conformance frame — the gate is inert"
    );
}

// =====================================================================
// Rust
// =====================================================================

#[test]
fn rust_endpoint_frames_conformant_write_data() {
    let frame = zerodds_endpoint_rust::xrce_write_frame(SEQ, writer_id().raw(), &SAMPLE)
        .expect("the sample fits the 16-bit submessage_length");
    let hex: String = frame.iter().map(|b| format!("{b:02x}")).collect();
    assert_conformant("rust", &hex);
}

#[test]
fn rust_reliable_frames_conformant_write_data() {
    let frame = zerodds_endpoint_rust::reliable::write_frame(SEQ, writer_id().raw(), &SAMPLE)
        .expect("the sample fits the 16-bit submessage_length");
    let hex: String = frame.iter().map(|b| format!("{b:02x}")).collect();
    assert_conformant("rust/reliable", &hex);
}

// =====================================================================
// C and C++ (the C++ SDK links the C framing)
// =====================================================================

const C_DRIVER: &str = r#"
#include <stdio.h>
#include "zerodds_endpoint.h"

int main(void) {
    unsigned char sample[4] = { 0xDE, 0xAD, 0xBE, 0xEF };
    unsigned char frame[64];
    size_t n = zdw_xrce_write_frame(frame, sizeof frame,
                                    ZDW_XRCE_SESSION_NOKEY,
                                    ZDW_XRCE_STREAM_BEST_EFFORT, 1u,
                                    ZDW_XRCE_OBJECT_ID(0x001, ZDW_OBJK_DATAWRITER),
                                    sample, sizeof sample);
    size_t i;
    if (n == 0u) { return 1; }
    for (i = 0; i < n; i++) { printf("%02x", frame[i]); }
    printf("\n");
    return 0;
}
"#;

#[test]
fn c_endpoint_frames_conformant_write_data() {
    if !tool_available("cc", "--version") {
        eprintln!("SKIP c: `cc` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_c");
    let src = write(&dir, "driver.c", C_DRIVER);
    let bin = dir.join("driver");
    let c = endpoints_dir("c");
    run(
        "c",
        Command::new("cc")
            .args(["-std=c89", "-pedantic", "-Wall", "-Wextra", "-O0"])
            .arg("-I")
            .arg(c.join("include"))
            .arg(&src)
            .arg(c.join("src/zerodds_endpoint.c"))
            .arg("-o")
            .arg(&bin),
    );
    assert_conformant("c", &run("c", &mut Command::new(&bin)));
}

#[test]
fn cpp_endpoint_frames_conformant_write_data() {
    if !tool_available("c++", "--version") {
        eprintln!("SKIP cpp: `c++` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_cpp");
    let src = write(
        &dir,
        "driver.cpp",
        &C_DRIVER.replace("<stdio.h>", "<cstdio>"),
    );
    let bin = dir.join("driver");
    let c = endpoints_dir("c");
    run(
        "cpp",
        Command::new("c++")
            .args(["-std=c++17", "-Wall", "-Wextra", "-O0"])
            .arg("-I")
            .arg(c.join("include"))
            .arg(&src)
            .arg(c.join("src/zerodds_endpoint.c"))
            .arg("-o")
            .arg(&bin),
    );
    assert_conformant("cpp", &run("cpp", &mut Command::new(&bin)));
}

// =====================================================================
// Interpreted / scripted ports
// =====================================================================

#[test]
fn python_endpoint_frames_conformant_write_data() {
    if !tool_available("python3", "-V") {
        eprintln!("SKIP python: `python3` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_python");
    let src = write(
        &dir,
        "driver.py",
        r#"
import sys
sys.path.insert(0, sys.argv[1])
import zerodds_endpoint as ze

frame = ze.xrce_write_frame(
    ze.XRCE_SESSION_NOKEY, ze.XRCE_STREAM_BEST_EFFORT, 1,
    ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER), bytes([0xDE, 0xAD, 0xBE, 0xEF]))
print("".join("%02x" % b for b in bytearray(frame)))
"#,
    );
    let out = run(
        "python",
        Command::new("python3")
            .arg(&src)
            .arg(endpoints_dir("python")),
    );
    assert_conformant("python", &out);
}

#[test]
fn node_endpoint_frames_conformant_write_data() {
    if !tool_available("node", "-v") {
        eprintln!("SKIP node: `node` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_node");
    let src = write(
        &dir,
        "driver.js",
        r#"
const z = require(process.argv[2] + '/zerodds.js');
const frame = z.xrceWriteFrame(0x80, 0x01, 1,
  z.xrceObjectId(0x001, z.OBJK_DATAWRITER), Buffer.from([0xDE, 0xAD, 0xBE, 0xEF]));
console.log(Buffer.from(frame).toString('hex'));
"#,
    );
    let out = run(
        "node",
        Command::new("node").arg(&src).arg(endpoints_dir("node")),
    );
    assert_conformant("node", &out);
}

#[test]
fn lua_endpoint_frames_conformant_write_data() {
    let Some(lua) = ["lua", "lua5.4", "lua5.3"]
        .into_iter()
        .find(|l| tool_available(l, "-v"))
    else {
        eprintln!("SKIP lua: no Lua interpreter on PATH");
        return;
    };
    let dir = scratch_dir("gate_lua");
    let src = write(
        &dir,
        "driver.lua",
        r#"
package.path = arg[1] .. "/?.lua;" .. package.path
local z = require("zerodds")
local frame = z.writeFrame(z.SESSION_NOKEY, z.STREAM_BEST_EFFORT, 1,
  z.objectId(0x001, z.OBJK_DATAWRITER), "\xDE\xAD\xBE\xEF")
print((frame:gsub(".", function(c) return string.format("%02x", c:byte()) end)))
"#,
    );
    let out = run("lua", Command::new(lua).arg(&src).arg(endpoints_dir("lua")));
    assert_conformant("lua", &out);
}

#[test]
fn julia_endpoint_frames_conformant_write_data() {
    if !tool_available("julia", "--version") {
        eprintln!("SKIP julia: `julia` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_julia");
    let src = write(
        &dir,
        "driver.jl",
        r#"
include(joinpath(ARGS[1], "zerodds.jl"))
frame = ZeroDDS.write_frame(ZeroDDS.SESSION_NOKEY, ZeroDDS.STREAM_BEST_EFFORT, 1,
    ZeroDDS.xrce_object_id(0x001, ZeroDDS.OBJK_DATAWRITER),
    UInt8[0xDE, 0xAD, 0xBE, 0xEF])
println(join([string(b, base = 16, pad = 2) for b in frame]))
"#,
    );
    let out = run(
        "julia",
        Command::new("julia").arg(&src).arg(endpoints_dir("julia")),
    );
    assert_conformant("julia", &out);
}

#[test]
fn elixir_endpoint_frames_conformant_write_data() {
    if !tool_available("elixir", "-v") {
        eprintln!("SKIP elixir: `elixir` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_elixir");
    let src = write(
        &dir,
        "driver.exs",
        r#"
frame =
  ZeroDDS.Endpoint.write_frame(
    0x80, 0x01, 1,
    ZeroDDS.Endpoint.object_id(0x001, ZeroDDS.Endpoint.objk_datawriter()),
    <<0xDE, 0xAD, 0xBE, 0xEF>>
  )

IO.puts(Base.encode16(frame, case: :lower))
"#,
    );
    let out = run(
        "elixir",
        Command::new("elixir")
            .arg("-r")
            .arg(endpoints_dir("elixir").join("lib/zerodds.ex"))
            .arg(&src),
    );
    assert_conformant("elixir", &out);
}

// =====================================================================
// Compiled ports
// =====================================================================

#[test]
fn go_endpoint_frames_conformant_write_data() {
    if !tool_available("go", "version") {
        eprintln!("SKIP go: `go` not on PATH");
        return;
    }
    let go = endpoints_dir("go");
    let dir = go.join("gate_xrce_framing");
    std::fs::create_dir_all(&dir).expect("mkdir gate dir");
    write(
        &dir,
        "main.go",
        r#"// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

// Prints one XRCE WRITE_DATA frame as hex, for
// crates/endpoint-e2e/tests/xrce_write_framing.rs.
package main

import (
	"encoding/hex"
	"fmt"

	zerodds "zeroddsendpoint"
)

func main() {
	frame := zerodds.XrceWriteFrame(
		zerodds.XrceSessionNoKey, zerodds.XrceStreamBestEffort, 1,
		zerodds.XrceObjectID(0x001, uint16(zerodds.ObjkDataWriter)),
		[]byte{0xDE, 0xAD, 0xBE, 0xEF})
	fmt.Println(hex.EncodeToString(frame))
}
"#,
    );
    let out = run(
        "go",
        Command::new("go")
            .arg("run")
            .arg("./gate_xrce_framing")
            .current_dir(&go),
    );
    assert_conformant("go", &out);
}

#[test]
fn java_endpoint_frames_conformant_write_data() {
    if !tool_available("javac", "-version") {
        eprintln!("SKIP java: `javac` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_java");
    let src = write(
        &dir,
        "GateFrame.java",
        r#"
public final class GateFrame {
    public static void main(String[] args) {
        byte[] frame = ZdwEndpoint.xrceWriteFrame(
            ZdwEndpoint.SESSION_NOKEY, ZdwEndpoint.STREAM_BEST_EFFORT, 1,
            ZdwEndpoint.objectId(0x001, ZdwEndpoint.OBJK_DATAWRITER),
            new byte[] { (byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF });
        StringBuilder sb = new StringBuilder();
        for (byte b : frame) sb.append(String.format("%02x", b & 0xFF));
        System.out.println(sb);
    }
}
"#,
    );
    run(
        "java",
        Command::new("javac")
            .args(["-encoding", "UTF-8", "-nowarn", "-d"])
            .arg(&dir)
            .arg(&src)
            .arg(endpoints_dir("java").join("ZdwEndpoint.java")),
    );
    let out = run(
        "java",
        Command::new("java").arg("-cp").arg(&dir).arg("GateFrame"),
    );
    assert_conformant("java", &out);
}

#[test]
fn kotlin_endpoint_frames_conformant_write_data() {
    if !tool_available("kotlinc", "-version") {
        eprintln!("SKIP kotlin: `kotlinc` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_kotlin");
    let src = write(
        &dir,
        "GateFrame.kt",
        r#"
import zerodds.OBJK_DATAWRITER
import zerodds.XRCE_SESSION_NOKEY
import zerodds.XRCE_STREAM_BEST_EFFORT
import zerodds.xrceObjectId
import zerodds.xrceWriteFrame

fun main() {
    val frame = xrceWriteFrame(
        XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT, 1,
        xrceObjectId(0x001, OBJK_DATAWRITER),
        byteArrayOf(0xDE.toByte(), 0xAD.toByte(), 0xBE.toByte(), 0xEF.toByte()),
    )
    println(frame.joinToString("") { "%02x".format(it) })
}
"#,
    );
    let jar = dir.join("gate.jar");
    run(
        "kotlin",
        Command::new("kotlinc")
            .arg(endpoints_dir("kotlin").join("src/Zerodds.kt"))
            .arg(&src)
            .args(["-include-runtime", "-d"])
            .arg(&jar),
    );
    let out = run("kotlin", Command::new("java").arg("-jar").arg(&jar));
    assert_conformant("kotlin", &out);
}

#[test]
fn csharp_endpoint_frames_conformant_write_data() {
    if !tool_available("dotnet", "--version") {
        eprintln!("SKIP csharp: `dotnet` not found");
        return;
    }
    let dir = scratch_dir("gate_csharp");
    write(
        &dir,
        "Gate.csproj",
        &format!(
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>disable</ImplicitUsings>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
    <StartupObject>Gate</StartupObject>
  </PropertyGroup>
  <ItemGroup>
    <Compile Include="{}" />
    <Compile Include="Gate.cs" />
  </ItemGroup>
</Project>
"#,
            endpoints_dir("csharp").join("Zerodds.cs").display()
        ),
    );
    write(
        &dir,
        "Gate.cs",
        r#"
using System;
using ZeroDDS;

public static class Gate {
    public static void Main() {
        var frame = Xrce.WriteFrame(1, Xrce.ObjectId(0x001, Xrce.ObjkDataWriter),
            new byte[] { 0xDE, 0xAD, 0xBE, 0xEF });
        Console.WriteLine(Convert.ToHexString(frame).ToLowerInvariant());
    }
}
"#,
    );
    run(
        "csharp",
        Command::new("dotnet")
            .args(["build", "-v", "q", "--nologo"])
            .arg(dir.join("Gate.csproj")),
    );
    let out = run(
        "csharp",
        Command::new("dotnet")
            .env("DOTNET_ROLL_FORWARD", "Major")
            .args(["run", "--no-build", "--project"])
            .arg(dir.join("Gate.csproj")),
    );
    assert_conformant("csharp", &out);
}

#[test]
fn fsharp_endpoint_frames_conformant_write_data() {
    if !tool_available("dotnet", "--version") {
        eprintln!("SKIP fsharp: `dotnet` not found");
        return;
    }
    let dir = scratch_dir("gate_fsharp");
    let src = write(
        &dir,
        "gate.fsx",
        &format!(
            r#"
#load @"{}"
open ZeroDDS

let frame =
    match writeFrame SessionNoKey StreamBestEffort 1 (xrceObjectId 0x001 ObjkDataWriter)
              [| 0xDEuy; 0xADuy; 0xBEuy; 0xEFuy |] with
    | Some f -> f
    | None -> failwith "the builder refused a four-octet sample"

printfn "%s" (frame |> Array.map (sprintf "%02x") |> String.concat "")
"#,
            endpoints_dir("fsharp").join("zerodds.fs").display()
        ),
    );
    let out = run(
        "fsharp",
        Command::new("dotnet")
            .env("DOTNET_ROLL_FORWARD", "Major")
            .arg("fsi")
            .arg(&src),
    );
    assert_conformant("fsharp", &out);
}

#[test]
fn swift_endpoint_frames_conformant_write_data() {
    if !tool_available("swiftc", "--version") {
        eprintln!("SKIP swift: `swiftc` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_swift");
    // Top-level statements are only allowed in `main.swift`.
    let src = write(
        &dir,
        "main.swift",
        r#"
let frame = xrceWriteFrame(
    session: xrceSessionNoKey, stream: xrceStreamBestEffort, seq: 1,
    objectId: xrceObjectId(0x001, objkDataWriter),
    sample: [0xDE, 0xAD, 0xBE, 0xEF])!
print(frame.map { String(format: "%02x", $0) }.joined())
"#,
    );
    let bin = dir.join("gate");
    run(
        "swift",
        Command::new("swiftc")
            .args(["-swift-version", "5", "-module-name", "main"])
            .arg(endpoints_dir("swift").join("Sources/Zerodds/Zerodds.swift"))
            .arg(&src)
            .arg("-o")
            .arg(&bin),
    );
    assert_conformant("swift", &run("swift", &mut Command::new(&bin)));
}

#[test]
fn ocaml_endpoint_frames_conformant_write_data() {
    if !tool_available("ocamlfind", "printconf") {
        eprintln!("SKIP ocaml: `ocamlfind` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_ocaml");
    let src = write(
        &dir,
        "gate.ml",
        r#"
let () =
  let frame =
    Zerodds.Endpoint.write_frame Zerodds.Endpoint.session_nokey
      Zerodds.Endpoint.stream_best_effort 1
      (Zerodds.Endpoint.object_id 0x001 Zerodds.Endpoint.objk_datawriter)
      (Bytes.of_string "\xDE\xAD\xBE\xEF")
  in
  Bytes.iter (fun c -> Printf.printf "%02x" (Char.code c)) frame;
  print_newline ()
"#,
    );
    // `ocamlopt` writes each unit's `.cmi`/`.cmx` next to its source, and
    // resolves `Zerodds` from the compilation directory — so the SDK is copied
    // in rather than referenced in place (which would also litter
    // `endpoints/ocaml`).
    let sdk = dir.join("zerodds.ml");
    std::fs::copy(endpoints_dir("ocaml").join("zerodds.ml"), &sdk).expect("copy zerodds.ml");
    let bin = dir.join("gate");
    run(
        "ocaml",
        Command::new("ocamlfind")
            // `-thread` and `threads.posix`, as `ocaml.rs`,
            // `ocaml_reliable.rs` and `endpoint-common.sh` all pass
            // them. Without the switch `ocamlfind` says so itself —
            // "Linking problems may arise because of the missing
            // -thread" — and on OCaml 4, where `Mutex` sits behind the
            // threads package rather than in the stdlib, the SDK does
            // not compile at all: `Error: Unbound module Mutex`. The CI
            // image carries 4.x from Debian, the measurement host 5.3.
            .args([
                "ocamlopt",
                "-thread",
                "-package",
                "unix,threads.posix",
                "-linkpkg",
            ])
            .arg(&sdk)
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .current_dir(&dir),
    );
    assert_conformant("ocaml", &run("ocaml", &mut Command::new(&bin)));
}

#[test]
fn nim_endpoint_frames_conformant_write_data() {
    if !tool_available("nim", "--version") {
        eprintln!("SKIP nim: `nim` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_nim");
    let src = write(
        &dir,
        "gate.nim",
        r#"
import std/strutils
import zerodds

let frame = writeFrame(SessionNoKey, StreamBestEffort, 1,
                       xrceObjectId(0x001'u16, ObjkDataWriter),
                       @[0xDE'u8, 0xAD'u8, 0xBE'u8, 0xEF'u8])
var hex = ""
for b in frame:
  hex.add(toHex(b.int, 2).toLowerAscii)
echo hex
"#,
    );
    let bin = dir.join("gate");
    run(
        "nim",
        Command::new("nim")
            .args(["c", "--hints:off", "--warnings:off"])
            .arg(format!("--path:{}", endpoints_dir("nim").display()))
            .arg(format!("-o:{}", bin.display()))
            .arg(&src)
            .current_dir(&dir),
    );
    assert_conformant("nim", &run("nim", &mut Command::new(&bin)));
}

// =====================================================================
// Ada — three SDKs, three separate framings
// =====================================================================

fn gprbuild_available() -> bool {
    tool_available("gprbuild", "--version")
}

fn gnatmake_available() -> bool {
    tool_available("gnatmake", "--version")
}

/// The Stage-1 Ada SDK (`endpoints/ada`) frames in `Deep_Reading.Frame`, which
/// is Ada over the C89 wire-core, so the driver is built the way
/// `endpoints/ada/zerodds_ada.gpr` builds the SDK's own tests: both languages
/// in one project, the C units Stage 1 does not bind excluded.
const ADA_GATE_ADB: &str = r#"-- SPDX-License-Identifier: Apache-2.0
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces;  use Interfaces;
with Deep_Reading;

procedure Gate is
   Hex     : constant String := "0123456789abcdef";
   Sample  : Deep_Reading.Frame_Store;
   Framed  : Deep_Reading.Frame_Store;
begin
   Sample.Data (0) := 16#DE#;
   Sample.Data (1) := 16#AD#;
   Sample.Data (2) := 16#BE#;
   Sample.Data (3) := 16#EF#;
   Sample.Len := 4;
   --  Seq 1, ObjectId 0x0015 = raw id 0x001 + OBJK_DATAWRITER (0x5).
   Framed := Deep_Reading.Frame (1, 16#0015#, Sample);
   for I in 0 .. Framed.Len - 1 loop
      Put (Hex (Integer (Framed.Data (I) / 16) + 1));
      Put (Hex (Integer (Framed.Data (I) mod 16) + 1));
   end loop;
   New_Line;
end Gate;
"#;

#[test]
fn ada_endpoint_frames_conformant_write_data() {
    if !gprbuild_available() {
        eprintln!("SKIP ada: `gprbuild` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_ada");
    write(&dir, "gate.adb", ADA_GATE_ADB);
    let ada_src = endpoints_dir("ada").join("src").display().to_string();
    let c_src = endpoints_dir("c").join("src").display().to_string();
    let c_inc = endpoints_dir("c").join("include").display().to_string();
    write(
        &dir,
        "gate.gpr",
        &format!(
            "project Gate is\n\
             \x20  for Languages use (\"Ada\", \"C\");\n\
             \x20  for Source_Dirs use (\".\", \"{ada_src}\", \"{c_src}\", \"{c_inc}\");\n\
             \x20  for Object_Dir use \"obj\";\n\
             \x20  for Exec_Dir use \".\";\n\
             \x20  for Main use (\"gate.adb\");\n\
             \x20  for Excluded_Source_Files use\n\
             \x20    (\"zerodds_endpoint.c\", \"zerodds_reflect.c\",\n\
             \x20     \"zerodds_async.c\", \"zerodds_reliable_async.c\");\n\
             \x20  package Compiler is\n\
             \x20     for Default_Switches (\"Ada\") use (\"-g\", \"-gnat2022\");\n\
             \x20     for Default_Switches (\"C\") use (\"-std=c89\", \"-O2\");\n\
             \x20  end Compiler;\n\
             \x20  package Binder is\n\
             \x20     for Default_Switches (\"Ada\") use (\"-Es\");\n\
             \x20  end Binder;\n\
             end Gate;\n"
        ),
    );
    run(
        "ada",
        Command::new("gprbuild")
            .args(["-p", "-P", "gate.gpr"])
            .current_dir(&dir),
    );
    assert_conformant("ada", &run("ada", &mut Command::new(dir.join("gate"))));
}

/// Strict Ada 83 (`-gnat83`, the switch `endpoints/ada-83/zerodds_ada83.gpr`
/// compiles the SDK with): `Text_IO` only, no `Interfaces`, no `for ... of`,
/// and `Byte` is an integer type without shifts — so the hex digits are
/// division and remainder.
const ADA83_GATE_ADB: &str = r#"-- SPDX-License-Identifier: Apache-2.0
with Text_IO;
with Zerodds_Ada83_Wire;     use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Endpoint;

procedure Gate83 is
   Hex    : constant String := "0123456789abcdef";
   Sample : constant Byte_Array (0 .. 3) :=
     (16#DE#, 16#AD#, 16#BE#, 16#EF#);
   W      : Writer;
begin
   Init (W, Little);
   Zerodds_Ada83_Endpoint.Xrce_Write_Frame
     (W, 16#80#, 16#01#, 1,
      Zerodds_Ada83_Endpoint.Xrce_Object_Id
        (16#001#, Zerodds_Ada83_Endpoint.Objk_DataWriter),
      Sample);
   for I in 0 .. Length (W) - 1 loop
      Text_IO.Put (Hex (Integer (W.Buf (I) / 16) + 1));
      Text_IO.Put (Hex (Integer (W.Buf (I) mod 16) + 1));
   end loop;
   Text_IO.New_Line;
end Gate83;
"#;

#[test]
fn ada83_endpoint_frames_conformant_write_data() {
    if !gnatmake_available() {
        eprintln!("SKIP ada-83: `gnatmake` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_ada83");
    let src = write(&dir, "gate83.adb", ADA83_GATE_ADB);
    let bin = dir.join("gate83");
    run(
        "ada-83",
        Command::new("gnatmake")
            .args(["-gnat83", "-g"])
            .arg(format!(
                "-I{}",
                endpoints_dir("ada-83").join("src").display()
            ))
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .current_dir(&dir),
    );
    assert_conformant("ada-83", &run("ada-83", &mut Command::new(&bin)));
}

/// The pure-Ada Stage-2 SDK (`endpoints/ada-native`) frames in
/// `Native_Framing.Xrce_Write_Frame` — no C, so `gnatmake` alone builds it.
const ADA_NATIVE_GATE_ADB: &str = r#"-- SPDX-License-Identifier: Apache-2.0
with Ada.Text_IO;
with Native_Framing;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

procedure Gate_Native is
   Hex    : constant String := "0123456789abcdef";
   Sample : constant Byte_Array (0 .. 3) :=
     [16#DE#, 16#AD#, 16#BE#, 16#EF#];
   Framed : constant Byte_Array :=
     Native_Framing.Xrce_Write_Frame
       (Session   => 16#80#,
        Stream    => 16#01#,
        Seq       => 1,
        Object_Id => Native_Framing.Xrce_Object_Id
                       (16#001#, Native_Framing.Objk_DataWriter),
        Sample    => Sample);
begin
   for B of Framed loop
      Ada.Text_IO.Put (Hex (Integer (B / 16) + 1));
      Ada.Text_IO.Put (Hex (Integer (B mod 16) + 1));
   end loop;
   Ada.Text_IO.New_Line;
end Gate_Native;
"#;

#[test]
fn ada_native_endpoint_frames_conformant_write_data() {
    if !gnatmake_available() {
        eprintln!("SKIP ada-native: `gnatmake` not on PATH");
        return;
    }
    let dir = scratch_dir("gate_ada_native");
    let src = write(&dir, "gate_native.adb", ADA_NATIVE_GATE_ADB);
    let bin = dir.join("gate_native");
    run(
        "ada-native",
        Command::new("gnatmake")
            .args(["-gnat2022", "-g"])
            .arg(format!(
                "-I{}",
                endpoints_dir("ada-native").join("src").display()
            ))
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .current_dir(&dir),
    );
    assert_conformant("ada-native", &run("ada-native", &mut Command::new(&bin)));
}

// =====================================================================
// D
// =====================================================================

#[test]
fn d_endpoint_frames_conformant_write_data() {
    // `endpoints/d/Makefile` defaults to `gdc`; `ldc2` builds the same
    // sources and only spells its output flag differently.
    let Some(dc) = ["gdc", "ldc2"]
        .into_iter()
        .find(|c| tool_available(c, "--version"))
    else {
        eprintln!("SKIP d: neither `gdc` nor `ldc2` on PATH");
        return;
    };
    let dir = scratch_dir("gate_d");
    let src = write(
        &dir,
        "gate.d",
        r#"
import std.stdio;
import zerodds;

void main() {
    immutable(ubyte)[] sample = [0xDE, 0xAD, 0xBE, 0xEF];
    auto frame = writeFrame(SessionNoKey, StreamBestEffort, 1,
                            xrceObjectId(0x001, ObjkDataWriter), sample);
    foreach (b; frame) writef("%02x", b);
    writeln();
}
"#,
    );
    let bin = dir.join("gate");
    let mut command = Command::new(dc);
    command
        .arg(endpoints_dir("d").join("zerodds.d"))
        .arg(&src)
        .current_dir(&dir);
    if dc == "ldc2" {
        command.arg(format!("-of={}", bin.display()));
    } else {
        command.arg("-o").arg(&bin);
    }
    run("d", &mut command);
    assert_conformant("d", &run("d", &mut Command::new(&bin)));
}

// =====================================================================
// Zig — two framings: the best-effort SDK and the reliable stream
// =====================================================================

/// Both Zig drivers are run with `zig run` and a `-M` module graph, so the
/// gate compiles the SDK source in the tree rather than a copy of it: the
/// driver is the root module and the SDK file is its only dependency.
fn zig_gate(label: &str, driver: &str, module: &str, sdk: &Path) -> String {
    let dir = scratch_dir(&format!("gate_zig_{module}"));
    let src = write(&dir, "gate.zig", driver);
    run(
        label,
        Command::new("zig")
            .arg("run")
            .arg("--dep")
            .arg(module)
            .arg(format!("-Mroot={}", src.display()))
            .arg(format!("-M{module}={}", sdk.display()))
            .current_dir(&dir),
    )
}

/// `endpoints/zig/src/zerodds.zig`'s `xrceWriteFrame` — the best-effort
/// endpoint SDK. `main` takes `std.process.Init` because Zig 0.16 routes
/// stdout through `std.Io`, the same shape `tests/zig.rs`'s drivers use.
const ZIG_GATE: &str = r#"const std = @import("std");
const z = @import("zerodds");

pub fn main(init: std.process.Init) !void {
    const sample = [_]u8{ 0xDE, 0xAD, 0xBE, 0xEF };
    var frame: [64]u8 = undefined;
    const n = try z.xrceWriteFrame(&frame, z.XRCE_SESSION_NOKEY,
                                   z.XRCE_STREAM_BEST_EFFORT, 1, &sample);
    var buf: [256]u8 = undefined;
    var out_file = std.Io.File.stdout().writer(init.io, &buf);
    const out = &out_file.interface;
    defer out.flush() catch {};
    for (frame[0..n]) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#;

/// `endpoints/zig/src/reliable.zig`'s `writeDataFrame`. It is a separate
/// framing — its own file, its own constants, no `@import` of
/// `zerodds.zig` — and it wrote the pre-conformance body as well, so it
/// gets its own column rather than riding on the one above.
const ZIG_RELIABLE_GATE: &str = r#"const std = @import("std");
const r = @import("reliable");

pub fn main(init: std.process.Init) !void {
    const sample = [_]u8{ 0xDE, 0xAD, 0xBE, 0xEF };
    var frame: [64]u8 = undefined;
    const n = r.writeDataFrame(&frame, 1, &sample);
    var buf: [256]u8 = undefined;
    var out_file = std.Io.File.stdout().writer(init.io, &buf);
    const out = &out_file.interface;
    defer out.flush() catch {};
    for (frame[0..n]) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#;

#[test]
fn zig_endpoint_frames_conformant_write_data() {
    if !tool_available("zig", "version") {
        eprintln!("SKIP zig: `zig` not on PATH");
        return;
    }
    let sdk = endpoints_dir("zig").join("src/zerodds.zig");
    assert_conformant("zig", &zig_gate("zig", ZIG_GATE, "zerodds", &sdk));
}

#[test]
fn zig_reliable_frames_conformant_write_data() {
    if !tool_available("zig", "version") {
        eprintln!("SKIP zig/reliable: `zig` not on PATH");
        return;
    }
    let sdk = endpoints_dir("zig").join("src/reliable.zig");
    assert_conformant(
        "zig/reliable",
        &zig_gate("zig/reliable", ZIG_RELIABLE_GATE, "reliable", &sdk),
    );
}
