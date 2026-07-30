// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! REAL client↔server RPC round-trip for the native Rust PSM (F.1 item 11).
//!
//! Generates Rust for a `@service`, drops it into a throwaway crate together
//! with a handler implementation and an in-process pump, compiles it against
//! the real `zerodds-rpc` runtime, and RUNS it. A generated `CalcRequester`
//! marshals each typed call into the wire request struct; the generated
//! `CalcReplier` decodes it, dispatches to the handler, and the reply struct
//! flows back — covering a value return, an `inout` write-back, an `out`
//! parameter, and a `oneway`. This is the Rust counterpart of the C# PSM's
//! `rpc_marshalling` round-trip: it proves the generated marshalling works,
//! not merely that it compiles.
//!
//! Opt-in (like `compile_check.rs`): needs a network-free `cargo` with the
//! workspace path-deps, so it is `#[ignore]`d in the default run and executed
//! explicitly with `--include-ignored`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{RustGenOptions, generate_rust_module};

const SERVICE_IDL: &str = "@service interface Calc { \
     long add(in long a, in long b); \
     void transform(in long factor, inout long acc, out long doubled); \
     oneway void log(in string msg); \
 };";

/// The handler + in-process pump + entry point, appended after the generated
/// module. Mirrors the `zerodds-rpc` runtime's own `runtime_e2e` pump: both
/// endpoints live on one offline participant and a background thread shuttles
/// the encoded frames between the request/reply queues.
const DRIVER_RS: &str = r#"

// ---- test driver (appended by the round-trip harness) --------------------

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use zerodds_dcps::factory::DomainParticipantFactory;
use zerodds_dcps::qos::DomainParticipantQos;
use zerodds_rpc::common_types::RemoteExceptionCode;
use zerodds_rpc::qos_profile::RpcQos;

struct Handler {
    last_log: Mutex<String>,
}

impl CalcHandler for Handler {
    fn add(&self, request: Calc_add_Request) -> Result<Calc_add_Reply, RemoteExceptionCode> {
        Ok(Calc_add_Reply { return_value: request.a + request.b })
    }
    fn transform(
        &self,
        request: Calc_transform_Request,
    ) -> Result<Calc_transform_Reply, RemoteExceptionCode> {
        Ok(Calc_transform_Reply {
            acc: request.acc + request.factor,
            doubled: request.factor * 2,
        })
    }
    fn log(&self, request: Calc_log_Request) {
        *self.last_log.lock().unwrap() = request.msg;
    }
}

fn main() {
    let participant = DomainParticipantFactory::instance()
        .create_participant_offline(4321, DomainParticipantQos::default());
    let qos = RpcQos::default_basic();

    let handler = Arc::new(Handler { last_log: Mutex::new(String::new()) });
    let replier = Arc::new(CalcReplier::new(&participant, &qos, handler.clone()).unwrap());
    let requester = Arc::new(CalcRequester::new(&participant, &qos).unwrap());

    // Background pump: move request frames client -> server, tick the server,
    // move reply frames server -> client, for every operation's endpoint.
    let stop = Arc::new(AtomicBool::new(false));
    let pump = {
        let req = requester.clone();
        let rep = replier.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                for f in req.add_endpoint.__drain_request_writer() {
                    let _ = rep.add_endpoint.__push_request_raw(f);
                }
                for f in req.transform_endpoint.__drain_request_writer() {
                    let _ = rep.transform_endpoint.__push_request_raw(f);
                }
                for f in req.log_endpoint.__drain_request_writer() {
                    let _ = rep.log_endpoint.__push_request_raw(f);
                }
                let _ = rep.tick();
                for f in rep.add_endpoint.__drain_reply_writer() {
                    let _ = req.add_endpoint.__push_reply_raw(f);
                }
                for f in rep.transform_endpoint.__drain_reply_writer() {
                    let _ = req.transform_endpoint.__push_reply_raw(f);
                }
                for f in rep.log_endpoint.__drain_reply_writer() {
                    let _ = req.log_endpoint.__push_reply_raw(f);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };

    let timeout = Some(Duration::from_secs(5));

    // Value return.
    let sum = requester.add(2, 3, timeout).unwrap();
    // inout (acc) + out (doubled), void return.
    let tr = requester.transform(3, 10, timeout).unwrap();
    // Oneway.
    requester.log("hello".to_string()).unwrap();
    // Give the pump time to deliver the oneway before we stop it.
    std::thread::sleep(Duration::from_millis(100));

    stop.store(true, Ordering::Release);
    pump.join().unwrap();

    let logged = handler.last_log.lock().unwrap().clone();

    assert_eq!(sum.return_value, 5, "value return");
    assert_eq!(tr.acc, 13, "inout write-back");
    assert_eq!(tr.doubled, 6, "out parameter");
    assert_eq!(logged, "hello", "oneway delivery");

    println!("RPC_ROUNDTRIP_OK");
}
"#;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .map(std::path::Path::to_path_buf)
        .unwrap_or(manifest)
}

#[test]
#[ignore = "requires cargo offline + path-deps; run with --include-ignored"]
fn rpc_client_server_round_trip_runs() {
    let ast = zerodds_idl::parse(SERVICE_IDL, &ParserConfig::full_4_2()).expect("parse");
    let generated = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");

    let tmp = std::env::temp_dir().join("dds_idl_rust_rpc_roundtrip");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");

    let root = workspace_root();
    let cargo_toml = format!(
        r#"[package]
name = "rpc_roundtrip_probe"
version = "0.0.0"
edition = "2024"

[[bin]]
name = "rpc_roundtrip_probe"
path = "src/main.rs"

[dependencies]
zerodds-cdr = {{ path = "{root}/crates/cdr" }}
zerodds-dcps = {{ path = "{root}/crates/dcps" }}
zerodds-types = {{ path = "{root}/crates/types" }}
zerodds-rpc = {{ path = "{root}/crates/rpc" }}
"#,
        root = root.display()
    );
    std::fs::File::create(tmp.join("Cargo.toml"))
        .expect("create Cargo.toml")
        .write_all(cargo_toml.as_bytes())
        .expect("write Cargo.toml");

    // Generated module (crate-level inner attributes stay at the top) followed
    // by the handler + pump + main.
    let mut program = generated.clone();
    program.push_str(DRIVER_RS);
    std::fs::File::create(tmp.join("src/main.rs"))
        .expect("create main.rs")
        .write_all(program.as_bytes())
        .expect("write main.rs");

    let output = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(tmp.join("Cargo.toml"))
        .arg("--offline")
        .output()
        .expect("spawn cargo run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("RPC_ROUNDTRIP_OK"),
        "Rust RPC round-trip failed.\n--- generated ---\n{generated}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
}
