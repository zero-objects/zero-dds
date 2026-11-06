// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Ada reliable endpoint, live: the Ada app is the reliable SENDER
//! (async-decoupled writer: the producer enqueues, a drain task does all I/O),
//! the shared Rust `ReliablePeer` drops datagrams and replies ACKNACK so the
//! app retransmits. The live run builds the example with a `Reliable_Sample`
//! that encodes the `Ping` of the codec the Ada generator writes from the
//! IDL, so it checks IDL, generated codec and runtime together. Gated on
//! gprbuild. The state machine's unit and
//! byte-golden cases run in the endpoints-gate column of Ada.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn ada_available() -> bool {
    Command::new("gprbuild").arg("--version").output().is_ok()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// Builds both Ada mains once (Ada + the C89 wire core) with gprbuild; returns
/// the exec dir. `None` if the build fails.
fn built_dir() -> Option<&'static PathBuf> {
    static BUILD: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let root = root();
            let dir = std::env::temp_dir().join(format!("ada_reliable_{}", std::process::id()));
            std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
            let gpr = format!(
                "project Rel is\n\
                 \x20  for Languages use (\"Ada\", \"C\");\n\
                 \x20  for Source_Dirs use (\"{ada_src}\", \"{ada_test}\", \"{c_src}\", \"{c_inc}\");\n\
                 \x20  for Excluded_Source_Files use (\"zerodds_endpoint.c\", \"zerodds_reflect.c\", \"zerodds_async.c\");\n\
                 \x20  for Object_Dir use \"obj\";\n\
                 \x20  for Exec_Dir use \".\";\n\
                 \x20  for Main use (\"example_reliable.adb\", \"test_reliable_unit.adb\");\n\
                 \x20  package Compiler is\n\
                 \x20     for Default_Switches (\"Ada\") use (\"-gnat2022\");\n\
                 \x20     for Default_Switches (\"C\") use (\"-std=c89\");\n\
                 \x20  end Compiler;\n\
                 end Rel;\n",
                ada_src = root.join("endpoints/ada/src").display(),
                ada_test = root.join("endpoints/ada/test").display(),
                c_src = root.join("endpoints/c/src").display(),
                c_inc = root.join("endpoints/c/include").display(),
            );
            std::fs::write(dir.join("rel.gpr"), gpr).expect("write gpr");
            let ok = Command::new("gprbuild")
                .arg("-P")
                .arg(dir.join("rel.gpr"))
                .arg("-p")
                .arg("-q")
                .current_dir(&dir)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok { Some(dir) } else { None }
        })
        .as_ref()
}

/// The body of `Reliable_Sample` the live run builds the example with: the
/// codec `codegen::source(Language::Ada)` writes from the IDL (package
/// `Zd_Types`, its specification and body as declarations of this body,
/// calling `runtime/ada`'s `Zd_Wire`) encodes `Ping (Seq => I, Msg =>
/// "reliable")` as sample I.
const GENERATED_SAMPLE_BODY: &str = r#"-- SPDX-License-Identifier: Apache-2.0
with Interfaces;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Zd_Wire;
use type Zd_Wire.Status;
use type Zd_Wire.Try_Construct;

package body Reliable_Sample is

__GENERATED__

   use Zd_Types;

   --  XCDR2 little-endian. The PL_CDR1 fields never fire under plain XCDR2.
   Zd_Framing : constant Zd_Wire.Pl_Cdr1_Framing :=
     (Pid_Flags => Zd_Wire.Flags_Sentinel_Must_Understand,
      Highest_Short_Id => 16128,
      Pad_Declared_Length => True,
      Pad_Declared_Optional_Length => True,
      Member_Must_Understand => True,
      Pad_After_Value => True);

   function Sample (I : Natural) return Deep_Reading.Frame_Store is
      Zd_W : Zd_Wire.Writer :=
        Zd_Wire.New_Writer (Zd_Wire.Little, 4, Zd_Framing, False);
      FS   : Deep_Reading.Frame_Store;
   begin
      zd_enc_Ping
        (Zd_W, (Seq => Interfaces.Integer_32 (I), Msg => To_Unbounded_String ("reliable")));
      declare
         B : constant Zd_Wire.Byte_Array := Zd_Wire.Finish (Zd_W).all;
      begin
         for K in B'Range loop
            FS.Data (K - B'First) := Deep_Reading.Byte (B (K));
         end loop;
         FS.Len := B'Length;
      end;
      return FS;
   end Sample;

end Reliable_Sample;
"#;

/// Builds `example_reliable.adb` with the generated `Reliable_Sample` body
/// instead of the in-tree one, once; returns the exec dir.
fn e2e_dir() -> &'static PathBuf {
    static BUILD: OnceLock<PathBuf> = OnceLock::new();
    BUILD.get_or_init(|| {
        let root = root();
        let dir = scratch_dir("ada_reliable_e2e");
        std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
        std::fs::copy(
            root.join("endpoints/ada/test/example_reliable.adb"),
            dir.join("example_reliable.adb"),
        )
        .expect("copy example_reliable.adb");
        std::fs::copy(
            root.join("endpoints/ada/test/reliable_sample.ads"),
            dir.join("reliable_sample.ads"),
        )
        .expect("copy reliable_sample.ads");
        let generated = source(Language::Ada)
            .lines()
            .map(|l| if l.is_empty() { String::new() } else { format!("   {l}") })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(
            dir.join("reliable_sample.adb"),
            GENERATED_SAMPLE_BODY.replace("__GENERATED__", &generated),
        )
        .expect("write reliable_sample.adb");
        let gpr = format!(
            "project Rel is\n\
             \x20  for Languages use (\"Ada\", \"C\");\n\
             \x20  for Source_Dirs use (\".\", \"{ada_src}\", \"{rt_src}\", \"{c_src}\", \"{c_inc}\");\n\
             \x20  for Excluded_Source_Files use (\"zerodds_endpoint.c\", \"zerodds_reflect.c\", \"zerodds_async.c\");\n\
             \x20  for Object_Dir use \"obj\";\n\
             \x20  for Exec_Dir use \".\";\n\
             \x20  for Main use (\"example_reliable.adb\");\n\
             \x20  package Compiler is\n\
             \x20     for Default_Switches (\"Ada\") use (\"-gnat2022\");\n\
             \x20     for Default_Switches (\"C\") use (\"-std=c89\");\n\
             \x20  end Compiler;\n\
             end Rel;\n",
            ada_src = root.join("endpoints/ada/src").display(),
            rt_src = runtime_dir("ada").join("src").display(),
            c_src = root.join("endpoints/c/src").display(),
            c_inc = root.join("endpoints/c/include").display(),
        );
        std::fs::write(dir.join("rel.gpr"), gpr).expect("write gpr");
        let out = Command::new("gprbuild")
            .args(["-p", "-q", "-P"])
            .arg(dir.join("rel.gpr"))
            .current_dir(&dir)
            .output()
            .expect("run gprbuild");
        assert!(
            out.status.success(),
            "gprbuild of the example with the generated sample failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        dir
    })
}

fn spawn_sender(exec_dir: &Path, port: u16) -> Child {
    Command::new(exec_dir.join("example_reliable"))
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ada sender")
}

/// reliable-endpoint 1.0 §5 point 3: the Ada app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Ada toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_3_ada_samples_arrive_in_order_despite_dropped_datagrams() {
    if !ada_available() {
        eprintln!("SKIP ada reliable loss recovery: gprbuild not on PATH");
        return;
    }
    let dir = e2e_dir();
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada/loss", RELIABLE_SAMPLES);
    assert_reliable_pings("ada/loss", &delivered, RELIABLE_SAMPLES);
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Ada producer's enqueue into
/// the protected `Send_Ring` of the async-decoupled writer costs less than
/// the inline frame and `sendto` it replaces; both values are printed.
#[test]
#[ignore = "needs the Ada toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_4_ada_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !ada_available() {
        eprintln!("SKIP ada reliable bench: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada build");
    let out = Command::new(dir.join("example_reliable"))
        .arg("bench")
        .output()
        .expect("run ada example_reliable bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("LATENCY"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("ada reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("ada", &stdout);
}
