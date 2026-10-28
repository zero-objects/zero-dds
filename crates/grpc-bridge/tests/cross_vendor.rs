// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against `grpcurl`.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §10 + §12.3.
//!
//! The two live tests start `zerodds-grpc-bridged` themselves on a port
//! of their own and drive the reference client against it. They are
//! `#[ignore]` and gated on `GRPC_GRPCURL=1`; with the gate set and no
//! `grpcurl` installed they fail rather than skipping. Run via:
//! ```bash
//! GRPC_GRPCURL=1 cargo test -p zerodds-grpc-bridge \
//!     --features cross-vendor-tests --test cross_vendor -- --ignored
//! ```

#![cfg(feature = "cross-vendor-tests")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use zerodds_grpc_bridge::reflection::{REFLECTION_PATH, encode_list_services};
use zerodds_grpc_bridge::service_gen::{ServiceCatalog, TopicService};

/// The `grpcurl` binary, or nothing.
fn grpcurl_available() -> bool {
    Command::new("grpcurl").arg("-version").output().is_ok()
}

/// `GRPC_GRPCURL=1` turns the two live tests from "skip when the binary
/// is missing" into "fail when the binary is missing".
///
/// Both of them used to return early on a missing `grpcurl` and the
/// runner then printed `ok`, so a host without it reported cross-vendor
/// interop that never happened. They also expected a daemon someone else
/// had started on a fixed 127.0.0.1:50051 — on a host where nothing
/// listened there, `grpcurl` failed, the second test discarded its result
/// with `let _ = out;` and the first asserted on an empty stdout.
fn require_grpcurl() {
    assert!(
        grpcurl_available(),
        "GRPC_GRPCURL is set but `grpcurl` was not found. Skipping here \
         would report interop that never happened."
    );
}

fn enabled() -> bool {
    std::env::var("GRPC_GRPCURL").is_ok()
}

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
}

/// A `zerodds-grpc-bridged` of this build, on a port of its own.
struct Daemon {
    child: Child,
    port: u16,
}

impl Daemon {
    /// Starts the daemon with reflection on and one topic mapped.
    fn start() -> Self {
        Self::start_with(&[])
    }

    /// [`Daemon::start`] with further arguments.
    fn start_with(extra: &[&str]) -> Self {
        // Bind an ephemeral port to learn a free number, then release it.
        let probe = TcpListener::bind("127.0.0.1:0").expect("an ephemeral bind");
        let port = probe.local_addr().expect("an address").port();
        drop(probe);

        let mut child = Command::new(daemon_binary())
            .args([
                "--bind",
                &format!("127.0.0.1:{port}"),
                "--reflection",
                "--topic",
                "Trade::Trade=TradeStream",
                "--idle-timeout-ms",
                "60000",
            ])
            .args(extra)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn zerodds-grpc-bridged");

        // Wait for the daemon's own `listening` line rather than for a
        // TCP connect: a connect that is opened and closed reaches the
        // accept loop as a connection with no preface, and the daemon
        // logs it as an error. Reading the line it prints when the
        // listener is up perturbs nothing and is exact.
        let stderr = child.stderr.take().expect("a piped stderr");
        let mut lines = BufReader::new(stderr).lines();
        let listening = lines
            .by_ref()
            .take(20)
            .flatten()
            .find(|line| line.contains("\"event\":\"listening\""));
        assert!(
            listening.is_some(),
            "the daemon did not report a listener on 127.0.0.1:{port}"
        );
        // Keep draining, or the daemon blocks once the pipe fills.
        thread::spawn(move || lines.for_each(drop));

        Self { child, port }
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SIGTERM rather than SIGKILL: the daemon writes its coverage
            // profile in an atexit handler, and a killed process leaves a
            // `.profraw` `llvm-profdata merge` refuses.
            //
            // SAFETY: the pid is live until the wait below, and Drop has
            // exclusive access to the child.
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `grpcurl` and returns its stdout, failing on a non-zero exit.
fn run_grpcurl(args: &[&str]) -> String {
    let out = Command::new("grpcurl")
        .args(args)
        .output()
        .expect("spawn grpcurl");
    assert!(
        out.status.success(),
        "the grpcurl reference client failed ({args:?}): status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs the `grpcurl` binary; set GRPC_GRPCURL=1"]
fn grpcurl_list_services_against_zerodds_daemon() {
    if !enabled() {
        eprintln!("[skip] GRPC_GRPCURL not set");
        return;
    }
    require_grpcurl();
    let daemon = Daemon::start();
    // grpcurl speaks HTTP/2 and the v1alpha reflection service against
    // the daemon; every octet of the request and of the parse is its own.
    let listed = run_grpcurl(&["-plaintext", &daemon.address(), "list"]);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "zerodds.bridge.v1.TradeStream"),
        "grpcurl list must name the service the `--topic` mapping declares; got: {listed:?}"
    );
}

#[test]
#[ignore = "needs the `grpcurl` binary; set GRPC_GRPCURL=1"]
fn grpcurl_lists_the_reflection_service_itself() {
    if !enabled() {
        eprintln!("[skip] GRPC_GRPCURL not set");
        return;
    }
    require_grpcurl();
    let daemon = Daemon::start();
    // `describe` needs FileContainingSymbol, which this bridge does not
    // serve; what the reflection surface does have to report is itself —
    // gRPC reflection v1alpha §ListServices. The previous test called
    // `describe` and then discarded the result with `let _ = out;`, so it
    // asserted nothing at all.
    let listed = run_grpcurl(&["-plaintext", &daemon.address(), "list"]);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "grpc.reflection.v1.ServerReflection"),
        "the reflection service must list itself; got: {listed:?}"
    );
}

#[test]
fn list_services_reflection_payload_is_well_formed() {
    // Pure-Rust verification of the reflection encoding without
    // requiring grpcurl. Spec §4.6 wire-shape: outer tag 6.
    let mut cat = ServiceCatalog::new();
    cat.register(TopicService::from_topic("Trade", "Trade"));
    cat.register(TopicService::from_topic("Quote", "Quote"));
    let bytes = encode_list_services(&cat);
    assert_eq!(bytes[0], 0x32, "outer tag should be (6<<3|2) = 0x32");
    // Reflection path constant must be the v1alpha-FQN that grpcurl uses.
    assert!(REFLECTION_PATH.contains("ServerReflection/ServerReflectionInfo"));
}

/// Runs `grpcurl` and returns (success, stdout + stderr).
fn try_grpcurl(args: &[&str]) -> (bool, String) {
    let out = Command::new("grpcurl")
        .args(args)
        .output()
        .expect("spawn grpcurl");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// §7.1 + §7.2 against grpc-go: grpcurl verifies the server certificate
/// against the CA, negotiates ALPN `h2` (grpc-go refuses a TLS server
/// without it), and sends the bearer token as `authorization` metadata.
/// Without the token the call ends in `Unauthenticated`.
#[test]
#[ignore = "needs the `grpcurl` binary; set GRPC_GRPCURL=1"]
fn grpcurl_over_tls_with_a_bearer_token() {
    if !enabled() {
        eprintln!("[skip] GRPC_GRPCURL not set");
        return;
    }
    require_grpcurl();
    let dir = std::env::temp_dir().join(format!("zd-grpcurl-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "zerodds grpcurl CA");
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let mut leaf = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    leaf.distinguished_name
        .push(rcgen::DnType::CommonName, "localhost");
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = leaf.signed_by(&key, &ca, &ca_key).unwrap();
    let path = |n: &str| dir.join(n).to_str().unwrap().to_string();
    std::fs::write(path("ca.pem"), ca.pem()).unwrap();
    std::fs::write(path("server.pem"), cert.pem()).unwrap();
    std::fs::write(path("server.key"), key.serialize_pem()).unwrap();

    let daemon = Daemon::start_with(&[
        "--tls-cert",
        &path("server.pem"),
        "--tls-key",
        &path("server.key"),
        "--set",
        "auth.mode=bearer",
        "--set",
        "auth.bearer_token=tk-grpcurl",
        "--set",
        "auth.bearer_subject=alice",
    ]);
    let ca_pem = path("ca.pem");
    let tls = ["-cacert", ca_pem.as_str(), "-servername", "localhost"];
    let addr = daemon.address();

    let mut with_token: Vec<&str> = tls.to_vec();
    with_token.extend(["-H", "authorization: Bearer tk-grpcurl", &addr, "list"]);
    let listed = run_grpcurl(&with_token);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "zerodds.bridge.v1.TradeStream"),
        "grpcurl over TLS with the token; got: {listed:?}"
    );

    let mut without: Vec<&str> = tls.to_vec();
    without.extend([addr.as_str(), "list"]);
    let (ok, text) = try_grpcurl(&without);
    assert!(!ok, "a call without the token succeeded: {text}");
    assert!(text.contains("Unauthenticated"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
