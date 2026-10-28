// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end test for business-interface SOAP dispatch
//! (`crate::rpc_dispatch`): a SOAP call to a config-named IDL
//! `interface`'s operation, answered by a **real, `zerodds-idlc
//! generate --lang rust`-produced** `Replier` on a second, independent
//! `DomainParticipant` — the exact binding `zerodds-rpc-rust-e2e`'s own
//! `tests/e2e.rs` runs, reused here rather than a hand-rolled
//! stand-in, so this test is the interop proof item 1 asks for: one
//! DDS-RPC topic pair for the whole `Calculator` interface (not one per
//! operation), a `Call`/`Return` union multiplexing every operation,
//! and a `Result` union per operation carrying either its
//! `Out` struct or a declared `raises` exception — built dynamically
//! here, matched byte-for-byte against the generated binding's own
//! hand-written codec (same discriminators: `operation_hash`/
//! `exception_hash`; same topic name: the interface's qualified name).
//!
//! Same leased-domain-plus-real-`TcpStream` strategy as
//! `daemon_e2e.rs`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_dcps::dds_type::DdsType;
use zerodds_rpc::replier::Replier;
use zerodds_rpc_rust_e2e::calculator::demo::{
    Calculator, CalculatorDispatcher, CalculatorError, DivByZero,
};
use zerodds_webdds_bridge::daemon::config::DaemonConfig;
use zerodds_webdds_bridge::daemon::server;

/// The exact IDL text `zerodds-rpc-rust-e2e/build.rs`'s `CALCULATOR`
/// constant generates its Rust binding from — every DDS-RPC 1.0
/// §7.5.1.1 construct in one interface: a return value, `raises`, a
/// `void` operation, an `attribute` (implied getter/setter), an `inout`
/// parameter.
const CALCULATOR_IDL: &str = "
module demo {
  exception DivByZero { string msg; };
  @DDSService interface Calculator {
    long add(in long a, in long b);
    double div(in double a, in double b) raises (DivByZero);
    void reset();
    void swap(inout long v);
    attribute long memory;
  };
};
";

fn http_request(addr: &str, raw: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    // Longer than `rpc_dispatch::RPC_TIMEOUT` (10s): the SOAP handler
    // itself waits up to that long for the RPC reply before this
    // client's own read would time out first.
    s.set_read_timeout(Some(Duration::from_secs(20))).ok();
    s.write_all(raw.as_bytes()).expect("write request");
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn soap_call(addr: &str, body: &str) -> String {
    let envelope = format!(
        "<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\"><soap:Body>{body}</soap:Body></soap:Envelope>"
    );
    let req = format!(
        "POST /soap HTTP/1.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{envelope}",
        envelope.len()
    );
    http_request(addr, &req)
}

/// Ticks `replier` in the background until dropped — same pattern
/// `zerodds-rpc-rust-e2e/tests/e2e.rs`'s own `TickingReplier` uses.
struct TickingReplier<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    _phantom: core::marker::PhantomData<(TIn, TOut)>,
}

impl<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> TickingReplier<TIn, TOut> {
    fn spawn(replier: Replier<TIn, TOut>) -> Self {
        let replier = Arc::new(replier);
        let stop = Arc::new(AtomicBool::new(false));
        let replier_for_thread = Arc::clone(&replier);
        let stop_for_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::Acquire) {
                let _ = replier_for_thread.tick();
                thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            handle: Some(handle),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<TIn: DdsType + Send + 'static, TOut: DdsType + Send + 'static> Drop
    for TickingReplier<TIn, TOut>
{
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The `Calculator` implementation under test — real, mutable memory,
/// so `swap`/`get_attribute_memory`/`set_attribute_memory`/`reset` have something to prove.
struct CalculatorImpl {
    memory: Mutex<i32>,
}

impl Calculator for CalculatorImpl {
    fn add(&self, a: i32, b: i32) -> Result<i32, CalculatorError> {
        Ok(a + b)
    }

    fn div(&self, a: f64, b: f64) -> Result<f64, CalculatorError> {
        if b == 0.0 {
            return Err(CalculatorError::DivByZero(DivByZero {
                msg: "division by zero".to_string(),
            }));
        }
        Ok(a / b)
    }

    fn reset(&self) -> Result<(), CalculatorError> {
        *self.memory.lock().expect("memory lock") = 0;
        Ok(())
    }

    fn swap(&self, v: &mut i32) -> Result<(), CalculatorError> {
        let mut memory = self.memory.lock().expect("memory lock");
        std::mem::swap(&mut *memory, v);
        Ok(())
    }

    fn get_attribute_memory(&self) -> Result<i32, CalculatorError> {
        Ok(*self.memory.lock().expect("memory lock"))
    }

    fn set_attribute_memory(&self, memory: i32) -> Result<(), CalculatorError> {
        *self.memory.lock().expect("memory lock") = memory;
        Ok(())
    }
}

/// A SOAP call to a config-named interface's operation reaches a real
/// generated `Replier` on a second, independent `DomainParticipant`
/// over real RTPS/UDP, and its reply comes back through the SOAP
/// response — the return-value path (`add`).
#[test]
fn soap_add_reaches_a_generated_replier_and_returns_its_reply() {
    let dir = std::env::temp_dir().join(format!("zd-webdds-rpc-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let idl_path = dir.join("calculator.idl");
    std::fs::write(&idl_path, CALCULATOR_IDL).expect("write idl");

    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.domain = domain;
    cfg.interfaces.push(idl_path.to_string_lossy().to_string());
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    let replier_participant = zerodds_dcps::DomainParticipantFactory::instance()
        .create_participant(domain, zerodds_dcps::DomainParticipantQos::default())
        .expect("replier participant");
    let dispatcher = CalculatorDispatcher::new(CalculatorImpl {
        memory: Mutex::new(0),
    });
    let replier = dispatcher
        .serve(
            &replier_participant,
            &zerodds_rpc::qos_profile::RpcQos::default(),
        )
        .expect("a replier");
    let _ticking = TickingReplier::spawn(replier);

    let resp = soap_call(&addr, "<add><a>3</a><b>4</b></add>");
    assert!(resp.contains("addResponse"), "{resp}");
    assert!(resp.contains("<return_>7</return_>"), "{resp}");

    stop_and_cleanup(&mut handle, &dir);
}

/// The `Result` union's declared-exception case: `div` by zero raises
/// `DivByZero`, and the gateway surfaces it as a named SOAP fault
/// naming the exception and its fields, not a silently empty or wrong
/// reply.
#[test]
fn soap_div_by_zero_surfaces_the_declared_exception() {
    let dir = std::env::temp_dir().join(format!("zd-webdds-rpc-e2e-exc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let idl_path = dir.join("calculator.idl");
    std::fs::write(&idl_path, CALCULATOR_IDL).expect("write idl");

    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();

    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    cfg.domain = domain;
    cfg.interfaces.push(idl_path.to_string_lossy().to_string());
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    let replier_participant = zerodds_dcps::DomainParticipantFactory::instance()
        .create_participant(domain, zerodds_dcps::DomainParticipantQos::default())
        .expect("replier participant");
    let dispatcher = CalculatorDispatcher::new(CalculatorImpl {
        memory: Mutex::new(0),
    });
    let replier = dispatcher
        .serve(
            &replier_participant,
            &zerodds_rpc::qos_profile::RpcQos::default(),
        )
        .expect("a replier");
    let _ticking = TickingReplier::spawn(replier);

    let resp = soap_call(&addr, "<div><a>1</a><b>0</b></div>");
    assert!(resp.contains("env:Receiver"), "{resp}");
    assert!(resp.contains("DivByZero"), "{resp}");
    assert!(resp.contains("division by zero"), "{resp}");

    stop_and_cleanup(&mut handle, &dir);
}

fn stop_and_cleanup(handle: &mut server::DaemonHandle, dir: &std::path::Path) {
    handle.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

/// An operation naming no configured `interfaces:` file (and not a
/// DDS-WEB object-model operation either) gets a named fault rather
/// than a silently empty/ignored SOAP body.
#[test]
fn soap_call_to_an_unconfigured_operation_is_a_named_fault() {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.listen = "127.0.0.1:0".to_string();
    let mut handle = server::start(cfg).expect("start daemon");
    let addr = handle.local_addr.clone();

    let resp = soap_call(&addr, "<add><a>3</a><b>4</b></add>");
    assert!(resp.contains("env:Receiver"), "{resp}");
    assert!(resp.contains("configured"), "{resp}");

    handle.shutdown();
}
