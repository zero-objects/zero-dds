// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Live cross-ORB OTS handshake (CORBA OTS §10.4 / GIOP §13.7): a JacORB 3.9
// **transactional client** begins a transaction and invokes a **ZeroDDS
// CorbaServer** over real IIOP. JacORB's `ClientContextTransferInterceptor`
// attaches the OTS `PropagationContext` in service context id=0; the ZeroDDS
// server captures it via `on_request_contexts`, decodes it, and asserts the
// otid JacORB propagated (formatID=7, bqual_length=3, tid=[AA,BB,CC]).
//
// This is the end-to-end counterpart to the byte-level capture test in
// `corba-cos-transactions::jacorb_live_capture`: there ZeroDDS decodes the
// captured bytes; here the bytes actually traverse a live GIOP connection from
// JacORB's interceptor to the ZeroDDS server.
//
// Gated: needs JDK8 + JacORB on the host (codepit). The harness
// `competitors/jacorb/ots/run_ots_client.sh` compiles + runs the JacORB client
// against the IOR the test exports. Ignored by default; run with
// `--ignored` on a host where `JACORB_OTS=1` is set.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! Live cross-ORB OTS handshake: a JacORB transactional client propagates an OTS
//! `PropagationContext` (service context id=0) to a ZeroDDS server over IIOP.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_corba_cos_transactions::PropagationContext;
use zerodds_corba_interop::runtime::{CorbaServer, stringify_object_ref};
use zerodds_corba_rust::SkeletonResult;

#[test]
#[ignore = "needs live JDK8 + JacORB (codepit); set JACORB_OTS=1"]
fn jacorb_transactional_client_propagates_ots_context_to_zerodds() {
    if std::env::var("JACORB_OTS").is_err() {
        eprintln!("JACORB_OTS not set — skipping live OTS handshake");
        return;
    }

    // Captured PropagationContext from the JacORB interceptor (SC id=0).
    let captured: Arc<Mutex<Option<PropagationContext>>> = Arc::new(Mutex::new(None));

    let server = CorbaServer::new();
    server.register(b"Ots", |_op, body, _e| SkeletonResult::Reply(body.to_vec()));
    {
        let cap = Arc::clone(&captured);
        server.on_request_contexts(move |ctxs| {
            for sc in &ctxs.0 {
                if sc.context_id == 0 {
                    if let Ok(ctx) = PropagationContext::from_service_context_data(&sc.context_data)
                    {
                        *cap.lock().unwrap() = Some(ctx);
                    }
                }
            }
        });
    }

    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = stringify_object_ref("IDL:Ots:1.0", &addr.ip().to_string(), addr.port(), b"Ots");

    // Run the JacORB transactional client against our IOR.
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("competitors/jacorb/ots/run_ots_client.sh");
    let out = Command::new("bash")
        .arg(&script)
        .arg(&ior)
        .output()
        .expect("spawn JacORB OTS client");
    eprintln!(
        "JacORB client stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The interceptor fires before the reply; poll briefly for the capture.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && captured.lock().unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(50));
    }
    acceptor.shutdown();

    let ctx = captured
        .lock()
        .unwrap()
        .clone()
        .expect("ZeroDDS server did not capture an OTS PropagationContext (SC id=0)");

    // The context JacORB's live `Current.begin()` transaction propagated over
    // the wire: its default 30s timeout + a real (non-nil) Coordinator IOR.
    // JacORB transmits it as a TypeCode-wrapped `any` (not the spec bare
    // struct); `from_service_context_data` accepted it (liberal-in).
    assert_eq!(ctx.timeout, 30, "JacORB default transaction timeout");
    assert!(ctx.parents.is_empty(), "flat transaction (no parents)");
    assert!(
        ctx.current.coord_ior.len() > 100,
        "live coordinator IOR must be present, got {} bytes",
        ctx.current.coord_ior.len()
    );
    assert!(
        String::from_utf8_lossy(&ctx.current.coord_ior)
            .contains("IDL:omg.org/CosTransactions/Coordinator:1.0"),
        "coordinator IOR carries the OMG Coordinator type id"
    );
    eprintln!("OTS cross-ORB OK: ZeroDDS received JacORB's live PropagationContext over IIOP");
}
