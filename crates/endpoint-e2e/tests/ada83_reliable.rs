// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Pre-Object **Ada 83** reliable endpoint, live: the Ada-83 app is the
//! reliable SENDER through the async-decoupled `Zerodds_Ada83_Async_Writer`
//! (a shared-variable ring the producer enqueues into and a drain task that
//! does the I/O; Ada 83 tasking, no protected objects), the shared Rust
//! [`ReliablePeer`](zerodds_endpoint_e2e) drops datagrams and replies
//! `ACKNACK` so the app retransmits until its window drains. The live run
//! builds the example with an `Ada83_Reliable_Sample` that encodes the `Ping`
//! of the codec the Ada 83 generator writes from the IDL. Gated on
//! `gprbuild`. The state machine's unit and byte-golden cases run in the
//! endpoints-gate column of Ada 83.
//!
//! Mirrors `ada_reliable.rs` but for the oldest-legacy variant. The reliability
//! CORE (`zerodds_ada83_reliable`), the writer and the sample are compiled
//! with `-gnat83`; the UDP driver
//! app (`example_ada83_reliable`, which needs GNAT.Sockets) is compiled with a
//! relaxed standard.

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

/// Builds the Ada-83 mains once with gprbuild (strict `-gnat83` core + unit
/// test, relaxed-standard UDP sender app); returns the exec dir. `None` on build
/// failure.
fn built_dir() -> Option<&'static PathBuf> {
    static BUILD: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let root = root();
            let dir = std::env::temp_dir().join(format!("ada83_reliable_{}", std::process::id()));
            std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
            // Strict `-gnat83` by default; the socket driver app gets a relaxed
            // standard (GNAT.Sockets / Command_Line / Real_Time are not Ada 83),
            // exactly the split the endpoints/ada-83 project file itself uses.
            let gpr = format!(
                "project Rel83 is\n\
                 \x20  for Languages use (\"Ada\");\n\
                 \x20  for Source_Dirs use (\"{src}\", \"{test}\");\n\
                 \x20  for Object_Dir use \"obj\";\n\
                 \x20  for Exec_Dir use \".\";\n\
                 \x20  for Main use (\"example_ada83_reliable.adb\", \"test_ada83_reliable.adb\");\n\
                 \x20  package Compiler is\n\
                 \x20     for Default_Switches (\"Ada\") use (\"-gnat83\");\n\
                 \x20     for Switches (\"example_ada83_reliable.adb\") use (\"-gnat2012\");\n\
                 \x20  end Compiler;\n\
                 end Rel83;\n",
                src = root.join("endpoints/ada-83/src").display(),
                test = root.join("endpoints/ada-83/test").display(),
            );
            std::fs::write(dir.join("rel83.gpr"), gpr).expect("write gpr");
            let ok = Command::new("gprbuild")
                .arg("-P")
                .arg(dir.join("rel83.gpr"))
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

/// The body of `Ada83_Reliable_Sample` the live run builds the example
/// with: the codec `codegen::source(Language::Ada83)` writes from the IDL
/// (package `Zd83_Types`, its specification and body as declarations of
/// this body, calling `runtime/ada-83`'s `Zd_Wire83`) encodes `Ping (Seq =>
/// I, Msg => "reliable")` as sample I. Strict Ada 83, as the rest of the
/// endpoint's core.
const GENERATED_SAMPLE_BODY: &str = r#"-- SPDX-License-Identifier: Apache-2.0
with Zd_Wire83;
with Zerodds_Ada83_Wire;

package body Ada83_Reliable_Sample is

__GENERATED__

   --  XCDR2 little-endian. The PL_CDR1 fields never fire under plain XCDR2.
   function Framing return Zd_Wire83.Pl_Cdr1_Framing is
      F : Zd_Wire83.Pl_Cdr1_Framing;
   begin
      F.Pid_Flags := Zd_Wire83.Flags_Sentinel_Must_Understand;
      F.Highest_Short_Id := 16128;
      F.Pad_Declared_Length := True;
      F.Pad_Declared_Optional_Length := True;
      F.Member_Must_Understand := True;
      F.Pad_After_Value := True;
      return F;
   end Framing;

   function Sample (I : Natural) return Zerodds_Ada83_Reliable.Frame_Store is
      W  : Zd_Wire83.Writer_Access;
      P  : Zd83_Types.Ping;
      B  : Zd_Wire83.Byte_Array_Access;
      FS : Zerodds_Ada83_Reliable.Frame_Store;
   begin
      W := Zd_Wire83.New_Writer (Zd_Wire83.Little, 4, Framing, False);
      P.Seq := Long_Integer (I);
      P.Msg := new String'("reliable");
      Zd83_Types.zd_enc_Ping (W, P);
      B := Zd_Wire83.Finish (W);
      for K in Integer range 0 .. B'Length - 1 loop
         FS.Data (K) := Zerodds_Ada83_Wire.Byte (B (B'First + K));
      end loop;
      FS.Len := B'Length;
      return FS;
   end Sample;

end Ada83_Reliable_Sample;
"#;

/// Builds `example_ada83_reliable.adb` with the generated
/// `Ada83_Reliable_Sample` body instead of the in-tree one, once; returns
/// the exec dir.
fn e2e_dir() -> &'static PathBuf {
    static BUILD: OnceLock<PathBuf> = OnceLock::new();
    BUILD.get_or_init(|| {
        let root = root();
        let dir = scratch_dir("ada83_reliable_e2e");
        std::fs::create_dir_all(dir.join("obj")).expect("mkdir");
        for f in ["example_ada83_reliable.adb", "ada83_reliable_sample.ads"] {
            std::fs::copy(root.join("endpoints/ada-83/test").join(f), dir.join(f))
                .unwrap_or_else(|e| panic!("copy {f}: {e}"));
        }
        let generated = source(Language::Ada83)
            .lines()
            .map(|l| {
                if l.is_empty() {
                    String::new()
                } else {
                    format!("   {l}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(
            dir.join("ada83_reliable_sample.adb"),
            GENERATED_SAMPLE_BODY.replace("__GENERATED__", &generated),
        )
        .expect("write ada83_reliable_sample.adb");
        let gpr = format!(
            "project Rel83 is\n\
             \x20  for Languages use (\"Ada\");\n\
             \x20  for Source_Dirs use (\".\", \"{src}\", \"{rt_src}\");\n\
             \x20  for Object_Dir use \"obj\";\n\
             \x20  for Exec_Dir use \".\";\n\
             \x20  for Main use (\"example_ada83_reliable.adb\");\n\
             \x20  package Compiler is\n\
             \x20     for Default_Switches (\"Ada\") use (\"-gnat83\");\n\
             \x20     for Switches (\"example_ada83_reliable.adb\") use (\"-gnat2012\");\n\
             \x20  end Compiler;\n\
             end Rel83;\n",
            src = root.join("endpoints/ada-83/src").display(),
            rt_src = runtime_dir("ada-83").join("src").display(),
        );
        std::fs::write(dir.join("rel83.gpr"), gpr).expect("write gpr");
        let out = Command::new("gprbuild")
            .args(["-p", "-q", "-P"])
            .arg(dir.join("rel83.gpr"))
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
    Command::new(exec_dir.join("example_ada83_reliable"))
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ada-83 sender")
}

/// reliable-endpoint 1.0 §5 point 3: the Ada 83 app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Ada 83 toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_3_ada83_samples_arrive_in_order_despite_dropped_datagrams() {
    if !ada_available() {
        eprintln!("SKIP ada83 reliable loss recovery: gprbuild not on PATH");
        return;
    }
    let dir = e2e_dir();
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_sender(dir, peer.port);
    let delivered = reliable_receive(&peer, child, "ada-83/loss", RELIABLE_SAMPLES);
    assert_reliable_pings("ada-83/loss", &delivered, RELIABLE_SAMPLES);
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Ada 83 producer's enqueue
/// into the ring of the async-decoupled writer (`Zerodds_Ada83_Async_Writer`,
/// a shared-variable ring without a rendezvous) costs less than the inline
/// frame and `sendto` it replaces; both values are printed.
#[test]
#[ignore = "needs the Ada 83 toolchain; runs in integration-bindings-gnat-beam"]
fn reliable_5_4_ada83_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !ada_available() {
        eprintln!("SKIP ada83 reliable bench: gprbuild not on PATH");
        return;
    }
    let dir = built_dir().expect("ada-83 build");
    let out = Command::new(dir.join("example_ada83_reliable"))
        .arg("bench")
        .output()
        .expect("run example_ada83_reliable bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("LATENCY"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("ada-83 reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("ada-83", &stdout);
}
