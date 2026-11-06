// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! ZeroDDS against JacORB 3.9, both directions, each side generated from the
//! same IDL by its own compiler (`competitors/interop.idl`; JacORB's IDL
//! compiler, `build.rs` for ZeroDDS): JacORB's servants, clients and naming
//! service are the oracle of the GIOP/IIOP requests and replies ZeroDDS
//! writes and reads. The JacORB cells of the former
//! `competitors/run_interop.sh`, and JacORB's transactional client
//! propagating an OTS context to a ZeroDDS server (CORBA OTS §10.4).
//!
//! The JacORB classes come from `competitors/build_peers.sh jacorb`
//! (`ZERODDS_CORBA_PEERS`); JacORB under `JACORB_HOME` (`/opt/jacorb`) on a
//! Java 8 JDK under `JACORB_JAVA_HOME` (`/opt/jdk8`), as
//! `tests/integration/provision.sh jacorb` installs them. The job
//! `integration-bridges-vendor-corba` runs the tests.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_corba_cos_transactions::PropagationContext;
use zerodds_corba_interop::runtime::{CorbaServer, stringify_object_ref};
use zerodds_corba_rust::SkeletonResult;

#[path = "support/peers.rs"]
mod peers;

fn jacorb_home() -> PathBuf {
    std::env::var_os("JACORB_HOME").map_or_else(|| PathBuf::from("/opt/jacorb"), PathBuf::from)
}

fn java() -> PathBuf {
    std::env::var_os("JACORB_JAVA_HOME")
        .map_or_else(|| PathBuf::from("/opt/jdk8"), PathBuf::from)
        .join("bin/java")
}

/// `java` with JacORB as the ORB on loopback, the built classes and
/// JacORB's jars on the class path, its log on stdout with what it prints.
fn jacorb(main: &str, props: &[&str], args: &[&str]) -> Command {
    let lib = jacorb_home().join("lib");
    let mut command = Command::new("sh");
    command.args(["-c", "exec \"$0\" \"$@\" 2>&1"]).arg(java());
    command
        .arg(format!("-Djava.endorsed.dirs={}", lib.display()))
        .arg(format!("-Djacorb.home={}", jacorb_home().display()))
        .arg("-Dorg.omg.CORBA.ORBClass=org.jacorb.orb.ORB")
        .arg("-Dorg.omg.CORBA.ORBSingletonClass=org.jacorb.orb.ORBSingleton")
        .arg("-Dorg.jacorb.ipAddr=127.0.0.1")
        .arg("-DOAIAddr=127.0.0.1")
        .args(props.iter().map(|p| format!("-D{p}")))
        .arg("-cp")
        .arg(format!(
            "{}:{}/*",
            peers::peers().join("jacorb-classes").display(),
            lib.display()
        ))
        .arg(main)
        .args(args);
    command
}

fn jacorb_client(main: &str, args: &[&str]) {
    let output = peers::run_client(jacorb(main, &[], args));
    assert!(output.contains("ALL OK"), "{main}:\n{output}");
}

/// JacORB's NameServer, its root context's IOR written to a file of its own.
fn name_server() -> (peers::Server, String) {
    let file = peers::scratch("jacorb-ns").join("ns.ior");
    let server = peers::Server::start(jacorb(
        "org.jacorb.naming.NameServer",
        &[&format!("jacorb.naming.ior_filename={}", file.display())],
        &[],
    ));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = std::fs::read_to_string(&file)
            && text.trim().starts_with("IOR:")
            && text.trim().len() % 2 == 0
        {
            return (server, text.trim().to_string());
        }
        assert!(
            Instant::now() < deadline,
            "JacORB's NameServer wrote no IOR"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_zerodds_calls_echo_and_bench_on_a_jacorb_server() {
    let mut server = peers::Server::start(jacorb("InteropServer", &[], &[]));
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    peers::zerodds_client("echo", &echo);
    peers::zerodds_client("bench", &bench);
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_a_jacorb_client_calls_echo_and_bench_on_a_zerodds_server() {
    let mut server = peers::zerodds_server();
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    jacorb_client("InteropClient", &["echo", &echo]);
    jacorb_client("InteropClient", &["bench", &bench]);
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_a_jacorb_client_drives_a_zerodds_naming_context() {
    let mut server = peers::zerodds_server();
    let naming = server.ior("NAMING_IOR=");
    jacorb_client("NamingClient", &[&naming]);
}

#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_zerodds_binds_and_resolves_in_jacorbs_name_server() {
    let (_names, root) = name_server();
    peers::zerodds_client("naming", &root);
}

/// CORBA OTS §10.4: JacORB's transactional client begins a transaction
/// with the TransactionService it resolves through the NameService and
/// calls a ZeroDDS server; JacORB's `ClientContextTransferInterceptor` puts
/// the `PropagationContext` in service context 0, which the server takes
/// from the request and decodes. The byte-level counterpart is
/// `zerodds-corba-cos-transactions`' `jacorb_live_capture`.
#[test]
#[ignore = "needs JacORB 3.9 and a Java 8 JDK (integration-bridges-vendor-corba)"]
fn corbarust_13_a_jacorb_transactional_client_propagates_its_ots_context_to_a_zerodds_server() {
    let (_names, root) = name_server();
    let ns_file = peers::scratch("jacorb-ots").join("ns.ior");
    std::fs::write(&ns_file, format!("{root}\n")).unwrap();
    let init_ref = format!("NameService=file://{}", ns_file.display());
    let mut transactions = peers::Server::start(jacorb(
        "org.jacorb.transaction.TransactionService",
        &[],
        &["-ORBInitRef", &init_ref],
    ));
    // Registered in the NameService (`TransactionService.service`).
    transactions.wait_for_line("INFO TransactionService up");

    let captured: Arc<Mutex<Option<PropagationContext>>> = Arc::new(Mutex::new(None));
    let server = CorbaServer::new();
    server.register(b"Ots", |_op, body, _e| SkeletonResult::Reply(body.to_vec()));
    {
        let sink = Arc::clone(&captured);
        server.on_request_contexts(move |contexts| {
            for sc in &contexts.0 {
                if sc.context_id == 0
                    && let Ok(ctx) = PropagationContext::from_service_context_data(&sc.context_data)
                {
                    *sink.lock().unwrap() = Some(ctx);
                }
            }
        });
    }
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    let ior = stringify_object_ref("IDL:Ots:1.0", &addr.ip().to_string(), addr.port(), b"Ots");

    let output = peers::run_client(jacorb(
        "OtsClient",
        &[
            "org.omg.PortableInterceptor.ORBInitializerClass.transaction=org.jacorb.transaction.TransactionInitializer",
        ],
        &[&ior, "-ORBInitRef", &init_ref],
    ));
    assert!(
        output.contains("OTS_TX_BEGUN"),
        "JacORB's client began no transaction:\n{output}"
    );
    // The client's request has been answered, so the server has taken
    // its contexts.
    acceptor.shutdown();
    let ctx = captured
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| panic!("no OTS PropagationContext in service context 0:\n{output}"));
    // JacORB's live `Current.begin()`: its default timeout of 30 s, a flat
    // transaction, a coordinator reference of the OMG type.
    assert_eq!(ctx.timeout, 30, "JacORB's default transaction timeout");
    assert!(ctx.parents.is_empty(), "a flat transaction");
    assert!(
        String::from_utf8_lossy(&ctx.current.coord_ior)
            .contains("IDL:omg.org/CosTransactions/Coordinator:1.0"),
        "the coordinator reference carries the OMG Coordinator type id"
    );
}
