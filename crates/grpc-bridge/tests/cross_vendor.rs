// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against `grpcurl`, the gRPC clients of
//! §10 (grpc-go, grpc-java, tonic, ghz, BloomRPC), Cyclone DDS, and the
//! OpenTelemetry Collector for the spans of §8.3.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §8.3, §10 + §12.3.
//!
//! The live tests start `zerodds-grpc-bridged` themselves on a port of
//! their own and drive the reference client against it. They are
//! `#[ignore]` in the crate tier and run in the integration pillar's job
//! `integration-bridges-vendor-grpc`, which installs the programs
//! (`tests/integration/provision.sh grpcurl otelcol cyclonedds-dev protoc
//! grpc-go grpc-java ghz bloomrpc`) and builds the clients of
//! `tests/interop/build_peers.sh` into `$ZERODDS_IT_PEERS/grpc`; a missing
//! program fails them:
//! ```bash
//! bash crates/grpc-bridge/tests/interop/build_peers.sh "$PEERS/grpc"
//! ZERODDS_IT_PEERS="$PEERS" cargo nextest run -p zerodds-grpc-bridge --test cross_vendor --run-ignored all
//! ```
//! The Compose file of §12.3 runs in `tests/compose_e2e.rs`.

#![cfg(feature = "std")]
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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, UserReaderConfig, UserSample, UserWriterConfig};

use zerodds_grpc_bridge::reflection::{REFLECTION_PATH, encode_list_services};
use zerodds_grpc_bridge::service_gen::{ServiceCatalog, TopicService};

/// Fails the test when `grpcurl` is not on the PATH.
fn require_grpcurl() {
    assert!(
        Command::new("grpcurl").arg("-version").output().is_ok(),
        "`grpcurl` is not on the PATH (tests/integration/provision.sh grpcurl)"
    );
}

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-grpc-bridged"))
}

/// A `zerodds-grpc-bridged` of this build, on a port of its own.
struct Daemon {
    child: Child,
    port: u16,
    /// The JSON log lines of a daemon of [`chat_daemon`], for the message
    /// of a failed assertion.
    log: Arc<std::sync::Mutex<Vec<String>>>,
}

impl Daemon {
    /// Starts the daemon with reflection on and one topic mapped.
    fn start() -> Self {
        Self::start_with(&[])
    }

    /// [`Daemon::start`] with further arguments.
    fn start_with(extra: &[&str]) -> Self {
        Self::start_env(extra, &[])
    }

    /// [`Daemon::start_with`] with environment variables.
    fn start_env(extra: &[&str], env: &[(&str, String)]) -> Self {
        // Bind an ephemeral port to learn a free number, then release it.
        let probe = TcpListener::bind("127.0.0.1:0").expect("an ephemeral bind");
        let port = probe.local_addr().expect("an address").port();
        drop(probe);

        let mut child = Command::new(daemon_binary())
            .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
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
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn zerodds-grpc-bridged");

        // Wait for the daemon's own `listening` line rather than for a
        // TCP connect: a connect that is opened and closed reaches the
        // accept loop as a connection with no preface, and the daemon
        // logs it as an error. Reading the line it prints when the
        // listener is up perturbs nothing and is exact.
        // Spec §8.1: the JSON log is on stdout.
        let stdout = child.stdout.take().expect("a piped stdout");
        let mut lines = BufReader::new(stdout).lines();
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

        Self {
            child,
            port,
            log: Arc::default(),
        }
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// The log lines so far, one per line.
    fn log_text(&self) -> String {
        self.log.lock().unwrap().join("\n")
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
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(std::time::Duration::from_millis(50));
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
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_4_6_grpcurl_list_services_against_zerodds_daemon() {
    require_grpcurl();
    let daemon = Daemon::start();
    // grpcurl speaks HTTP/2 and the v1alpha reflection service against
    // the daemon; every octet of the request and of the parse is its own.
    let listed = run_grpcurl(&["-plaintext", &daemon.address(), "list"]);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "zerodds.trade.v1.TradeStream"),
        "grpcurl list must name the service the `--topic` mapping declares; got: {listed:?}"
    );
}

#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_4_6_grpcurl_lists_the_reflection_service_itself() {
    require_grpcurl();
    let daemon = Daemon::start();
    // grpc-go's reflection lists itself in both names.
    let listed = run_grpcurl(&["-plaintext", &daemon.address(), "list"]);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "grpc.reflection.v1.ServerReflection"),
        "the reflection service must list itself; got: {listed:?}"
    );
}

/// §4.6 against grpcurl (grpc-go's protobuf runtime as the reader of the
/// descriptors): `describe` of the service names the four methods with
/// their streaming, `describe` of `Sample` the six fields, and a call built
/// from the descriptors alone (`-d '{}'`, JSON to protobuf and back)
/// answers.
#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_4_6_grpcurl_describe_names_the_four_methods() {
    require_grpcurl();
    let daemon = Daemon::start();
    let addr = daemon.address();
    let svc = run_grpcurl(&[
        "-plaintext",
        &addr,
        "describe",
        "zerodds.trade.v1.TradeStream",
    ]);
    for rpc in [
        "rpc Publish ( stream .zerodds.trade.v1.Sample ) returns ( .zerodds.trade.v1.PublishAck );",
        "rpc Subscribe ( .zerodds.trade.v1.SubscribeReq ) returns ( stream .zerodds.trade.v1.Sample );",
        "rpc PublishOne ( .zerodds.trade.v1.Sample ) returns ( .zerodds.trade.v1.PublishAck );",
        "rpc Catalog ( .zerodds.trade.v1.CatalogReq ) returns ( .zerodds.trade.v1.CatalogResp );",
    ] {
        assert!(svc.contains(rpc), "{rpc} not in {svc}");
    }
    let sample = run_grpcurl(&["-plaintext", &addr, "describe", "zerodds.trade.v1.Sample"]);
    for field in [
        "bytes cdr_payload = 1;",
        "uint32 flags = 2;",
        "bytes key_hash = 3;",
        "uint64 source_ts_ns = 4;",
        "string type_name = 5;",
        "string topic_name = 6;",
    ] {
        assert!(sample.contains(field), "{field} not in {sample}");
    }
    // `describe` without a symbol walks every listed service, the
    // reflection service included.
    let all = run_grpcurl(&["-plaintext", &addr, "describe"]);
    assert!(
        all.contains("grpc.reflection.v1.ServerReflection is a service"),
        "{all}"
    );
    let catalog = run_grpcurl(&[
        "-plaintext",
        "-d",
        "{}",
        &addr,
        "zerodds.trade.v1.TradeStream/Catalog",
    ]);
    assert!(
        catalog.contains("\"ddsName\": \"Trade::Trade\""),
        "{catalog}"
    );
    assert!(
        catalog.contains("\"grpcService\": \"zerodds.trade.v1.TradeStream\""),
        "{catalog}"
    );
}

/// §4.6 against grpcurl: the configured `grpc_package` is the package the
/// reflection service lists and describes.
#[test]
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_4_6_grpcurl_lists_the_configured_package() {
    require_grpcurl();
    let daemon = Daemon::start_with(&["--topic", "Quote=acme.market.v2.QuoteStream"]);
    let addr = daemon.address();
    let listed = run_grpcurl(&["-plaintext", &addr, "list"]);
    assert!(
        listed
            .lines()
            .any(|l| l.trim() == "acme.market.v2.QuoteStream"),
        "{listed}"
    );
    let methods = run_grpcurl(&["-plaintext", &addr, "list", "acme.market.v2.QuoteStream"]);
    assert!(
        methods.contains("acme.market.v2.QuoteStream.Subscribe"),
        "{methods}"
    );
}

/// §5.2 with curl as the client: `GET /idl/<dds_name>` over HTTP/1.1 on
/// the cleartext listener and with `--http2-prior-knowledge` returns the
/// IDL as `text/x-omg-idl`.
#[test]
#[ignore = "needs curl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_5_2_curl_fetches_the_idl() {
    assert!(
        Command::new("curl").arg("--version").output().is_ok(),
        "`curl` is not on the PATH"
    );
    let dir = std::env::temp_dir().join(format!("zd-curl-idl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let idl = "module Trade { struct Quote { double price; }; };\n";
    let idl_path = dir.join("quote.idl");
    std::fs::write(&idl_path, idl).unwrap();
    let config = dir.join("grpc-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            "topics:\n  - dds_name: \"Trade::Quote\"\n    idl_file: \"{}\"\n",
            idl_path.display()
        ),
    )
    .unwrap();
    let daemon = Daemon::start_with(&["--config", config.to_str().unwrap()]);
    for http in ["--http1.1", "--http2-prior-knowledge"] {
        let out = Command::new("curl")
            .args([
                "-sS",
                http,
                "-w",
                "\n%{http_code} %{content_type} %{http_version}",
                &format!("http://{}/idl/Trade::Quote", daemon.address()),
            ])
            .output()
            .expect("spawn curl");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "{http}: {text}");
        let expected_version = if http == "--http1.1" { "1.1" } else { "2" };
        assert!(
            text.ends_with(&format!("{idl}\n200 text/x-omg-idl {expected_version}")),
            "{http}: {text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes `ca.pem` (a CA named `ca_name`), `server.pem` (a certificate
/// for `localhost` the CA signed) and `server.key` into `dir`.
fn write_tls_files(dir: &std::path::Path, ca_name: &str) {
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, ca_name);
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let mut leaf = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    leaf.distinguished_name
        .push(rcgen::DnType::CommonName, "localhost");
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = leaf.signed_by(&key, &ca, &ca_key).unwrap();
    std::fs::write(dir.join("ca.pem"), ca.pem()).unwrap();
    std::fs::write(dir.join("server.pem"), cert.pem()).unwrap();
    std::fs::write(dir.join("server.key"), key.serialize_pem()).unwrap();
}

/// §5.2 over TLS with curl as the client: `GET /idl/<dds_name>` with
/// `--http1.1` (ALPN `http/1.1`) and with `--http2` (ALPN `h2`) returns the
/// IDL as `text/x-omg-idl`.
#[test]
#[ignore = "needs curl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_5_2_curl_over_tls_fetches_the_idl_with_http_1_1_and_h2() {
    assert!(
        Command::new("curl").arg("--version").output().is_ok(),
        "`curl` is not on the PATH"
    );
    let dir = std::env::temp_dir().join(format!("zd-curl-idl-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_tls_files(&dir, "zerodds curl CA");
    let path = |n: &str| dir.join(n).to_str().unwrap().to_string();
    let idl = "module Trade { struct Quote { double price; }; };\n";
    std::fs::write(path("quote.idl"), idl).unwrap();
    std::fs::write(
        path("grpc-bridged.yaml"),
        format!(
            "topics:\n  - dds_name: \"Trade::Quote\"\n    idl_file: \"{}\"\n",
            path("quote.idl")
        ),
    )
    .unwrap();
    let daemon = Daemon::start_with(&[
        "--config",
        &path("grpc-bridged.yaml"),
        "--tls-cert",
        &path("server.pem"),
        "--tls-key",
        &path("server.key"),
    ]);
    let addr = daemon.address();
    let port = addr.rsplit(':').next().unwrap().to_string();
    for (http, expected_version) in [("--http1.1", "1.1"), ("--http2", "2")] {
        let out = Command::new("curl")
            .args([
                "-sS",
                http,
                "--cacert",
                &path("ca.pem"),
                "--resolve",
                &format!("localhost:{port}:127.0.0.1"),
                "-w",
                "\n%{http_code} %{content_type} %{http_version}",
                &format!("https://localhost:{port}/idl/Trade::Quote"),
            ])
            .output()
            .expect("spawn curl");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(
            out.status.success(),
            "{http}: {text}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            text.ends_with(&format!("{idl}\n200 text/x-omg-idl {expected_version}")),
            "{http}: {text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn grpcbridge_4_6_list_services_reflection_payload_is_well_formed() {
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
#[ignore = "needs grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_7_1_grpcurl_over_tls_with_a_bearer_token() {
    require_grpcurl();
    let dir = std::env::temp_dir().join(format!("zd-grpcurl-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_tls_files(&dir, "zerodds grpcurl CA");
    let path = |n: &str| dir.join(n).to_str().unwrap().to_string();

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
            .any(|l| l.trim() == "zerodds.trade.v1.TradeStream"),
        "grpcurl over TLS with the token; got: {listed:?}"
    );

    let mut without: Vec<&str> = tls.to_vec();
    without.extend([addr.as_str(), "list"]);
    let (ok, text) = try_grpcurl(&without);
    assert!(!ok, "a call without the token succeeded: {text}");
    assert!(text.contains("Unauthenticated"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The OpenTelemetry Collector (`otelcol`, core distribution) with an
/// OTLP/HTTP receiver on a port of its own and the `debug` exporter, which
/// prints every span it decoded (trace id, parent id, name, kind,
/// attributes) on stderr.
struct Collector {
    child: Child,
    port: u16,
    log: std::sync::Arc<std::sync::Mutex<String>>,
}

impl Collector {
    fn start(dir: &std::path::Path) -> Self {
        assert!(
            Command::new("otelcol").arg("--version").output().is_ok(),
            "`otelcol` is not on the PATH (tests/integration/provision.sh otelcol)"
        );
        let probe = TcpListener::bind("127.0.0.1:0").expect("an ephemeral bind");
        let port = probe.local_addr().expect("an address").port();
        drop(probe);
        let config = dir.join("otelcol.yaml");
        std::fs::write(
            &config,
            format!(
                "receivers:\n  otlp:\n    protocols:\n      http:\n        endpoint: 127.0.0.1:{port}\n\
                 exporters:\n  debug:\n    verbosity: detailed\n\
                 service:\n  telemetry:\n    metrics:\n      level: none\n\
                 \x20 pipelines:\n    traces:\n      receivers: [otlp]\n      exporters: [debug]\n"
            ),
        )
        .unwrap();
        let mut child = Command::new("otelcol")
            .arg("--config")
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn otelcol");
        let log = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = std::sync::Arc::clone(&log);
        let stderr = child.stderr.take().expect("a piped stderr");
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut l = sink.lock().unwrap();
                l.push_str(&line);
                l.push('\n');
            }
        });
        let c = Self { child, port, log };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !c.text().contains("Everything is ready") {
            assert!(
                std::time::Instant::now() < deadline,
                "otelcol did not start: {}",
                c.text()
            );
            thread::sleep(std::time::Duration::from_millis(50));
        }
        c
    }

    fn text(&self) -> String {
        self.log.lock().unwrap().clone()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spec §8.3 against the OpenTelemetry Collector: a `PublishOne` grpcurl
/// sends with a W3C `traceparent` (Trace Context §3.2.3 example) becomes a
/// server span `zerodds.trade.v1.TradeStream/PublishOne` in the caller's
/// trace, the caller's span its parent, with the `rpc.*` attributes and a
/// `dds.publish` child, `rpc.grpc.status_code`, `network.peer.port` and
/// `dds.samples` of type int; the collector decodes them from the daemon's
/// OTLP/HTTP JSON (an OTLP request it cannot decode it answers 400 and
/// prints nothing).
#[test]
#[ignore = "needs otelcol and grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_8_3_the_opentelemetry_collector_decodes_the_spans_of_a_call() {
    require_grpcurl();
    let dir = std::env::temp_dir().join(format!("zd-grpcd-otelcol-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let collector = Collector::start(&dir);
    let daemon = Daemon::start_env(
        &[],
        &[(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://127.0.0.1:{}", collector.port),
        )],
    );
    let ack = run_grpcurl(&[
        "-plaintext",
        "-H",
        "traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        "-d",
        "{\"cdrPayload\":\"AAEAAGhp\"}",
        &daemon.address(),
        "zerodds.trade.v1.TradeStream/PublishOne",
    ]);
    assert!(ack.contains("sequenceNumber"), "{ack}");
    // The stop flushes the spans (spec §9.2).
    drop(daemon);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let text = loop {
        let text = collector.text();
        if text.contains("zerodds.trade.v1.TradeStream/PublishOne") && text.contains("dds.publish")
        {
            break text;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no span reached the collector:\n{text}"
        );
        thread::sleep(std::time::Duration::from_millis(100));
    };
    let field = |name: &str, value: &str| {
        text.lines()
            .any(|l| l.trim_start().starts_with(name) && l.trim_end().ends_with(value))
    };
    assert!(
        field("Trace ID", ": 4bf92f3577b34da6a3ce929d0e0e4736"),
        "{text}"
    );
    assert!(field("Parent ID", ": 00f067aa0ba902b7"), "{text}");
    assert!(
        field("Name", ": zerodds.trade.v1.TradeStream/PublishOne"),
        "{text}"
    );
    assert!(field("Name", ": dds.publish"), "{text}");
    assert!(field("Kind", ": Server"), "{text}");
    assert!(text.contains("rpc.system: Str(grpc)"), "{text}");
    assert!(text.contains("rpc.method: Str(PublishOne)"), "{text}");
    assert!(text.contains("dds.topic: Str(Trade::Trade)"), "{text}");
    // The conventions define the status code and the peer port as int.
    assert!(text.contains("rpc.grpc.status_code: Int(0)"), "{text}");
    assert!(text.contains("network.peer.port: Int("), "{text}");
    assert!(text.contains("dds.samples: Int(1)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Cyclone DDS participant of `tests/interop/cyclone_chat.c`, built
/// with Cyclone's `idlc` and the C compiler into the test's directory.
fn cyclone_chat() -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interop");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("grpc-cyclone-chat");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["chat.idl", "cyclone_chat.c"] {
        std::fs::copy(src.join(file), dir.join(file)).unwrap();
    }
    let idlc = Command::new("idlc")
        .arg("chat.idl")
        .current_dir(&dir)
        .status()
        .expect("idlc; install cyclonedds-dev");
    assert!(idlc.success(), "idlc failed");
    let cc = Command::new("cc")
        .args([
            "-o",
            "cyclone_chat",
            "cyclone_chat.c",
            "chat.c",
            "-I.",
            "-lddsc",
        ])
        .current_dir(&dir)
        .status()
        .expect("cc");
    assert!(cc.success(), "the Cyclone DDS participant does not build");
    dir.join("cyclone_chat")
}

/// The lines a child writes to stdout, read on a thread.
fn stdout_lines(child: &mut Child) -> std::sync::mpsc::Receiver<String> {
    let out = child.stdout.take().expect("a piped stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    rx
}

/// `Chat::Message { seq, text }` as XCDR1 little endian with its
/// encapsulation header (DDSI-RTPS 2.5 §10.2, CDR_LE).
fn chat_message_cdr(seq: i32, text: &str) -> Vec<u8> {
    let mut out = vec![0x00, 0x01, 0x00, 0x00];
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&(text.len() as u32 + 1).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    out
}

/// The `seq` and `text` of a `Chat::Message` in XCDR1 or XCDR2, little
/// endian (encapsulation CDR_LE or CDR2_LE; a final struct has no DHEADER).
fn chat_message_of(cdr: &[u8]) -> (i32, String) {
    assert!(
        cdr.len() >= 12 && matches!(cdr[..4], [0x00, 0x01, _, _] | [0x00, 0x07, _, _]),
        "not CDR_LE or CDR2_LE: {cdr:02x?}"
    );
    let body = &cdr[4..];
    let seq = i32::from_le_bytes(body[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
    let text = &body[8..8 + len - 1];
    assert_eq!(body[8 + len - 1], 0, "the string's NUL");
    (seq, String::from_utf8(text.to_vec()).unwrap())
}

/// A topic of the daemon's configuration (a YAML list item of `topics`),
/// reliable, in the package `zerodds.chat.v1`.
fn chat_topic(name: &str, ty: &str, service: &str, keyed: bool) -> String {
    format!(
        "  - dds_name: \"{name}\"\n    dds_type: \"{ty}\"\n    grpc_service: {service}\n\
         \x20   grpc_package: zerodds.chat.v1\n    keyed: {keyed}\n    qos:\n      reliability: reliable\n"
    )
}

/// A daemon of this build on `domain` with the `topics` of its
/// configuration file, h2c on a loopback port of its own, reflection on;
/// ready once it logged its listener. The file lives in the test's
/// directory under `CARGO_TARGET_TMPDIR`.
fn chat_daemon(domain: &str, topics: &str) -> Daemon {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("grpc-cross-vendor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let probe = TcpListener::bind("127.0.0.1:0").expect("an ephemeral bind");
    let port = probe.local_addr().expect("an address").port();
    drop(probe);
    let config = dir.join(format!("grpc-bridged-{port}.yaml"));
    std::fs::write(
        &config,
        format!(
            "domain: {domain}\ngrpc:\n  bind: \"127.0.0.1:{port}\"\n  reflection:\n    enabled: true\ntopics:\n{topics}"
        ),
    )
    .unwrap();
    let mut child = Command::new(daemon_binary())
        .args(["--config", config.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zerodds-grpc-bridged");
    let log = stdout_lines(&mut child);
    let daemon = Daemon {
        child,
        port,
        log: Arc::default(),
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let line = log
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the daemon's listening line");
        let listening = line.contains("\"event\":\"listening\"");
        daemon.log.lock().unwrap().push(line);
        if listening {
            break;
        }
    }
    let sink = Arc::clone(&daemon.log);
    thread::spawn(move || log.iter().for_each(|line| sink.lock().unwrap().push(line)));
    daemon
}

/// Spec §10: the daemon is a normal RTPS peer. With a Cyclone DDS 0.10
/// participant on the daemon's domain, a `PublishOne` grpcurl sends reaches
/// Cyclone's DataReader as the sample grpcurl sent, and a sample Cyclone's
/// DataWriter writes reaches a grpcurl `Subscribe` stream with Cyclone's
/// serialized bytes, the type name and the topic name — for the keyed type
/// `Chat::Message` (WITH_KEY endpoints) and for the keyless `Chat::Note`,
/// whose topic is `keyed: false` (NO_KEY endpoints, DDSI-RTPS 2.5
/// §9.3.1.2; Cyclone matches no WITH_KEY endpoint with it). Discovery
/// (SPDP, SEDP), the match and the reliable delivery are Cyclone's on its
/// side.
#[test]
#[ignore = "needs Cyclone DDS (cyclonedds-dev, idlc), a C compiler and grpcurl; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_a_grpcurl_publish_reaches_cyclone_and_a_cyclone_sample_a_grpcurl_subscribe() {
    require_grpcurl();
    let cyclone = cyclone_chat();
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id().to_string();
    let daemon = chat_daemon(
        &domain,
        &[
            chat_topic("ChatMessage", "Chat::Message", "ChatMessageStream", true),
            chat_topic("ChatNote", "Chat::Note", "ChatNoteStream", false),
        ]
        .concat(),
    );
    let address = daemon.address();
    for (kind, topic, ty, service, sent, written) in [
        (
            "message",
            "ChatMessage",
            "Chat::Message",
            "ChatMessageStream",
            7,
            9,
        ),
        ("note", "ChatNote", "Chat::Note", "ChatNoteStream", 17, 19),
    ] {
        let method = |m: &str| format!("zerodds.chat.v1.{service}/{m}");
        grpc_to_cyclone(
            &cyclone,
            &domain,
            &address,
            kind,
            &method("PublishOne"),
            sent,
        );
        let (type_name, topic_name, sample) = cyclone_to_grpc(
            &cyclone,
            &domain,
            &address,
            kind,
            &method("Subscribe"),
            written,
        );
        assert_eq!((type_name.as_str(), topic_name.as_str()), (ty, topic));
        assert_eq!(sample, (written, format!("hello from cyclone {kind}")));
    }
}

/// gRPC → DDS: grpcurl calls `publish_one` with `Chat::Message`/`Note`
/// `{ seq, "hello from grpcurl <kind>" }` until Cyclone's reader of `kind`
/// takes it, as that sample.
fn grpc_to_cyclone(
    cyclone: &std::path::Path,
    domain: &str,
    address: &str,
    kind: &str,
    publish_one: &str,
    seq: i32,
) {
    use base64::Engine as _;
    let text = format!("hello from grpcurl {kind}");
    let mut sub = Command::new(cyclone)
        .args(["sub", domain, "20", kind])
        .stdout(Stdio::piped())
        .spawn()
        .expect("cyclone sub");
    let taken = stdout_lines(&mut sub);
    assert_eq!(
        taken
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("ready"),
        "{\"event\":\"ready\"}"
    );
    let request = format!(
        "{{\"cdrPayload\":\"{}\"}}",
        base64::engine::general_purpose::STANDARD.encode(chat_message_cdr(seq, &text))
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let line = loop {
        // Until the daemon's DataWriter and Cyclone's DataReader matched.
        let ack = run_grpcurl(&["-plaintext", "-d", &request, address, publish_one]);
        assert!(ack.contains("sequenceNumber"), "{ack}");
        if let Ok(line) = taken.recv_timeout(std::time::Duration::from_millis(500)) {
            break line;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no sample of {kind} reached Cyclone DDS"
        );
    };
    assert_eq!(line, format!("{{\"seq\":{seq},\"text\":\"{text}\"}}"));
    let _ = sub.kill();
    let _ = sub.wait();
}

/// DDS → gRPC: Cyclone's writer of `kind` writes `{ seq, "hello from
/// cyclone <kind>" }`, a grpcurl `subscribe` stream gets it: its
/// `typeName`, `topicName` and the `seq` and `text` of its `cdrPayload`.
fn cyclone_to_grpc(
    cyclone: &std::path::Path,
    domain: &str,
    address: &str,
    kind: &str,
    subscribe: &str,
    seq: i32,
) -> (String, String, (i32, String)) {
    use base64::Engine as _;
    let mut stream = Command::new("grpcurl")
        .args([
            "-plaintext",
            "-max-time",
            "20",
            "-d",
            "{}",
            address,
            subscribe,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("grpcurl Subscribe");
    let streamed = stdout_lines(&mut stream);
    let mut publisher = Command::new(cyclone)
        .args([
            "pub",
            domain,
            &seq.to_string(),
            &format!("hello from cyclone {kind}"),
            kind,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("cyclone pub");
    let mut message = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !message.contains('}') {
        let line = streamed
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .unwrap_or_else(|_| panic!("no message of {kind} on the grpcurl Subscribe stream"));
        message.push_str(&line);
        message.push('\n');
    }
    let _ = stream.kill();
    let _ = stream.wait();
    let _ = publisher.kill();
    let _ = publisher.wait();
    let field = |name: &str| -> String {
        let at = message
            .find(&format!("\"{name}\": \""))
            .unwrap_or_else(|| panic!("{name} in {message}"))
            + name.len()
            + 5;
        message[at..].split('"').next().unwrap().to_string()
    };
    let cdr = base64::engine::general_purpose::STANDARD
        .decode(field("cdrPayload"))
        .expect("base64");
    (field("typeName"), field("topicName"), chat_message_of(&cdr))
}

/// A gRPC client `tests/interop/build_peers.sh` built into
/// `$ZERODDS_IT_PEERS/grpc` (the integration pillar's `{peers}/grpc`).
fn grpc_peer(name: &str) -> PathBuf {
    let dir = std::env::var_os("ZERODDS_IT_PEERS").map(PathBuf::from).unwrap_or_else(|| {
        panic!(
            "ZERODDS_IT_PEERS is not set: bash crates/grpc-bridge/tests/interop/build_peers.sh <dir>/grpc, \
             then ZERODDS_IT_PEERS=<dir>"
        )
    });
    let program = dir.join("grpc").join(name);
    assert!(
        program.is_file(),
        "{} was not built (crates/grpc-bridge/tests/interop/build_peers.sh)",
        program.display()
    );
    program
}

/// A DDS participant of the test on `domain`; `n` tells two of one test
/// apart.
fn dds_peer(domain: i32, n: u8) -> Arc<DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[8] = 0x12;
    prefix[10] = n;
    prefix[11] = 0xc5;
    DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        zerodds_dcps::runtime::RuntimeConfig::default(),
    )
    .unwrap()
}

/// The reliable DataReader of `ChatMessage` (`Chat::Message`) on `rt`, once
/// it matched the daemon's DataWriter.
fn chat_reader(
    rt: &DcpsRuntime,
) -> (
    zerodds_rtps::wire_types::EntityId,
    mpsc::Receiver<UserSample>,
) {
    let (reader, rx) = rt
        .register_user_reader(UserReaderConfig {
            topic_name: "ChatMessage".into(),
            type_name: "Chat::Message".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.user_reader_matched_count(reader) < 1 {
        assert!(
            Instant::now() < deadline,
            "the daemon's DataWriter did not match"
        );
        rt.wait_match_event(Duration::from_millis(200));
    }
    (reader, rx)
}

/// The next Alive sample of `rx`: its representation identifier and its
/// CDR bytes.
fn next_alive(rx: &mpsc::Receiver<UserSample>) -> ([u8; 2], Vec<u8>) {
    loop {
        match rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a sample from the daemon")
        {
            UserSample::Alive {
                payload,
                representation_identifier,
                ..
            } => return (representation_identifier, payload.to_vec()),
            _ => continue,
        }
    }
}

/// CDR bytes with every octet value, so that a byte the bridge or the
/// client changed, dropped or added shows.
fn every_octet(seed: u8) -> Vec<u8> {
    (0..=255u8).map(|b| b.wrapping_add(seed)).collect()
}

/// `body` behind the encapsulation header `identifier`, options zero.
fn encapsulated(identifier: [u8; 2], body: &[u8]) -> Vec<u8> {
    let mut out = vec![identifier[0], identifier[1], 0, 0];
    out.extend_from_slice(body);
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Runs a client of `tests/interop` to its end: its exit code and the lines
/// of its stdout.
fn run_client(program: &Path, address: &str, args: &[&str]) -> (Option<i32>, Vec<String>) {
    let out = Command::new(program)
        .arg(address)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", program.display()));
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    if !out.status.success() {
        eprintln!(
            "{} {args:?}: {}",
            program.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    (out.status.code(), lines)
}

/// [`run_client`] of a call that ends OK.
fn client_ok(program: &Path, address: &str, args: &[&str]) -> Vec<String> {
    let (code, lines) = run_client(program, address, args);
    assert_eq!(code, Some(0), "{} {args:?}: {lines:?}", program.display());
    lines
}

/// Spec §10, §10a: a gRPC client built from the `.proto` the crate ships
/// (`proto/zerodds/chat/v1/bridge.proto`) by its own generator, its own
/// protobuf runtime and its own HTTP/2 transport, against the daemon, with a
/// DDS participant of the test on the other side, on a leased domain:
/// - a client-streaming `Publish` of two samples (CDR_LE) reaches the DDS
///   DataReader octet for octet, the encapsulation header as its
///   representation identifier; the `PublishAck` names the sequence number
///   of the second;
/// - a `PublishOne` (CDR2_LE) likewise, sequence number 3;
/// - a `PublishOne` whose `cdr_payload` has no encapsulation header ends
///   `INVALID_ARGUMENT` (3) with the daemon's `grpc-message` (§4.5), as the
///   client decodes them;
/// - `Catalog` names the topic, its type, the full service name and the
///   `idl_url` of the authority the client sent (§5.2);
/// - a server-streaming `Subscribe` gets the two samples a DataWriter of the
///   test writes (XCDR2, big endian) with the encapsulation header
///   `CDR2_BE`, the type and the topic name.
fn client_matrix(client: &Path) {
    let lease = zerodds_test_domains::lease_domain();
    let idl = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interop/chat.idl");
    let daemon = chat_daemon(
        &lease.id().to_string(),
        &format!(
            "{}    idl_file: \"{}\"\n",
            chat_topic("ChatMessage", "Chat::Message", "ChatMessageStream", true),
            idl.display()
        ),
    );
    let address = daemon.address();
    let dds = dds_peer(lease.id(), 1);
    let (reader, samples) = chat_reader(&dds);

    let (first, second) = (every_octet(0), every_octet(0x55));
    let ack = client_ok(
        client,
        &address,
        &[
            "publish",
            &hex(&encapsulated([0x00, 0x01], &first)),
            &hex(&encapsulated([0x00, 0x01], &second)),
        ],
    );
    assert_eq!(ack, ["ack seq=2"]);
    assert_eq!(next_alive(&samples), ([0x00, 0x01], first));
    assert_eq!(next_alive(&samples), ([0x00, 0x01], second));

    let third = every_octet(0xaa);
    let ack = client_ok(
        client,
        &address,
        &["publish-one", &hex(&encapsulated([0x00, 0x07], &third))],
    );
    assert_eq!(ack, ["ack seq=3"]);
    assert_eq!(next_alive(&samples), ([0x00, 0x07], third));

    let (code, lines) = run_client(client, &address, &["publish-one", "0001"]);
    assert_eq!(code, Some(3), "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("status code=3 message=Sample.cdr_payload: "),
        "{lines:?}"
    );

    assert_eq!(
        client_ok(client, &address, &["catalog"]),
        [format!(
            "topic dds_name=ChatMessage dds_type=Chat::Message \
             grpc_service=zerodds.chat.v1.ChatMessageStream idl_url=http://{address}/idl/ChatMessage"
        )]
    );

    // The DataReader above would count as a match of the DataWriter.
    dds.remove_user_reader(reader);
    let writer = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "ChatMessage".into(),
            type_name: "Chat::Message".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let mut stream = Command::new(client)
        .args([address.as_str(), "subscribe", "2"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the Subscribe client");
    let streamed = stdout_lines(&mut stream);
    // The stream's DataReader exists once the call is open.
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(writer) < 1 {
        assert!(
            Instant::now() < deadline,
            "the DataReader of the client's Subscribe did not match"
        );
        dds.wait_match_event(Duration::from_millis(200));
    }
    dds.set_user_writer_data_rep_override(
        writer,
        Some(vec![
            zerodds_rtps::publication_data::data_representation::XCDR2,
        ]),
    )
    .unwrap();
    dds.set_user_writer_byte_order_override(writer, true)
        .unwrap();
    let (fourth, fifth) = (every_octet(0x11), every_octet(0x22));
    dds.write_user_sample_borrowed(writer, &fourth).unwrap();
    dds.write_user_sample_borrowed(writer, &fifth).unwrap();
    let got: Vec<String> = (0..2)
        .map(|_| {
            streamed
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| {
                    panic!(
                        "no sample on the client's Subscribe stream; the daemon logged:\n{}",
                        daemon.log_text()
                    )
                })
        })
        .collect();
    let status = stream.wait().expect("the Subscribe client ends");
    assert!(status.success(), "{status:?}");
    let line = |body: &[u8]| {
        format!(
            "sample type=Chat::Message topic=ChatMessage flags=0 payload={}",
            hex(&encapsulated([0x00, 0x06], body))
        )
    };
    assert_eq!(got, [line(&fourth), line(&fifth)]);
    dds.shutdown();
}

/// Spec §10a: grpc-go (Debian's golang-google-grpc-dev, stubs of
/// protoc-gen-go and protoc-gen-go-grpc) — see [`client_matrix`].
#[test]
#[ignore = "needs the grpc-go client of tests/interop/build_peers.sh; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_grpc_go_publishes_subscribes_and_reads_the_catalog() {
    client_matrix(&grpc_peer("grpc-go-client"));
}

/// Spec §10a: grpc-java (Debian's libgrpc-java with the Netty transport,
/// stubs of protoc's grpc-java plugin) — see [`client_matrix`].
#[test]
#[ignore = "needs the grpc-java client of tests/interop/build_peers.sh; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_grpc_java_publishes_subscribes_and_reads_the_catalog() {
    client_matrix(&grpc_peer("grpc-java-client"));
}

/// Spec §10a: tonic (with prost, stubs of tonic-prost-build; hyper and h2
/// as the HTTP/2 transport) — see [`client_matrix`].
#[test]
#[ignore = "needs the tonic client of tests/interop/build_peers.sh; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_tonic_publishes_subscribes_and_reads_the_catalog() {
    client_matrix(&grpc_peer("tonic-client"));
}

/// Spec §12.3 with processes: the tonic client and a Cyclone DDS 0.10
/// participant on the two sides of the daemon. A `Publish` stream of tonic
/// with `Chat::Message { seq, text }` (XCDR1 little endian) reaches
/// Cyclone's DataReader as that sample; a sample Cyclone's DataWriter
/// writes reaches a tonic `Subscribe` stream with Cyclone's bytes, the type
/// and the topic name. The Compose file of §12.3 runs the three in
/// containers (`tests/compose_e2e.rs`).
#[test]
#[ignore = "needs the tonic client of tests/interop/build_peers.sh and Cyclone DDS (cyclonedds-dev); runs in integration-bridges-vendor-grpc"]
fn grpcbridge_12_3_a_tonic_publish_reaches_cyclone_and_a_cyclone_sample_a_tonic_subscribe() {
    let tonic = grpc_peer("tonic-client");
    let cyclone = cyclone_chat();
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id().to_string();
    let daemon = chat_daemon(
        &domain,
        &chat_topic("ChatMessage", "Chat::Message", "ChatMessageStream", true),
    );
    let address = daemon.address();

    let text = "hello from tonic";
    let mut sub = Command::new(&cyclone)
        .args(["sub", &domain, "20", "message"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("cyclone sub");
    let taken = stdout_lines(&mut sub);
    assert_eq!(
        taken.recv_timeout(Duration::from_secs(10)).expect("ready"),
        "{\"event\":\"ready\"}"
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let line = loop {
        // Until the daemon's DataWriter and Cyclone's DataReader matched.
        let ack = client_ok(
            &tonic,
            &address,
            &["publish", &hex(&chat_message_cdr(23, text))],
        );
        assert_eq!(ack.len(), 1, "{ack:?}");
        assert!(ack[0].starts_with("ack seq="), "{ack:?}");
        if let Ok(line) = taken.recv_timeout(Duration::from_millis(500)) {
            break line;
        }
        assert!(Instant::now() < deadline, "no sample reached Cyclone DDS");
    };
    assert_eq!(line, format!("{{\"seq\":23,\"text\":\"{text}\"}}"));
    let _ = sub.kill();
    let _ = sub.wait();

    let mut stream = Command::new(&tonic)
        .args([address.as_str(), "subscribe", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("tonic subscribe");
    let streamed = stdout_lines(&mut stream);
    let mut publisher = Command::new(&cyclone)
        .args(["pub", &domain, "29", "hello from cyclone", "message"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("cyclone pub");
    let line = streamed
        .recv_timeout(Duration::from_secs(15))
        .expect("a sample on the tonic Subscribe stream");
    let _ = publisher.kill();
    let _ = publisher.wait();
    assert!(stream.wait().expect("tonic ends").success());
    let payload = line
        .strip_prefix("sample type=Chat::Message topic=ChatMessage flags=0 payload=")
        .unwrap_or_else(|| panic!("{line}"));
    assert_eq!(
        chat_message_of(&unhex(payload)),
        (29, "hello from cyclone".to_string())
    );
}

/// Spec §10a against ghz (the gRPC load tester, built on grpc-go): 2000
/// `PublishOne` calls from the `.proto` the crate ships, 20 at a time over
/// two connections, all end OK (ghz's own count and status distribution),
/// and all 2000 samples reach the DDS DataReader of the test, each octet for
/// octet.
#[test]
#[ignore = "needs ghz; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_ghz_2000_publish_one_calls_all_ok_and_all_reach_dds() {
    use base64::Engine as _;
    assert!(
        Command::new("ghz").arg("--version").output().is_ok(),
        "`ghz` is not on the PATH (tests/integration/provision.sh ghz)"
    );
    let lease = zerodds_test_domains::lease_domain();
    let daemon = chat_daemon(
        &lease.id().to_string(),
        &chat_topic("ChatMessage", "Chat::Message", "ChatMessageStream", true),
    );
    let dds = dds_peer(lease.id(), 1);
    let (_, samples) = chat_reader(&dds);
    let proto = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proto");
    let body = every_octet(0x3c);
    let data = format!(
        "{{\"cdrPayload\":\"{}\"}}",
        base64::engine::general_purpose::STANDARD.encode(encapsulated([0x00, 0x01], &body))
    );
    let out = Command::new("ghz")
        .arg("--insecure")
        .arg("--proto")
        .arg(proto.join("zerodds/chat/v1/bridge.proto"))
        .arg("--import-paths")
        .arg(&proto)
        .args([
            "--call",
            "zerodds.chat.v1.ChatMessageStream.PublishOne",
            "-d",
            &data,
            "-n",
            "2000",
            "-c",
            "20",
            "--connections",
            "2",
            "--timeout",
            "10s",
            "--format",
            "json",
            &daemon.address(),
        ])
        .output()
        .expect("spawn ghz");
    let report: String = String::from_utf8_lossy(&out.stdout)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        out.status.success(),
        "ghz: {:?} {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(report.contains("\"count\":2000,"), "{report:.600}");
    assert!(
        report.contains("\"statusCodeDistribution\":{\"OK\":2000}"),
        "{report:.600}"
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut taken = 0;
    while taken < 2000 {
        let left = deadline.saturating_duration_since(Instant::now());
        match samples.recv_timeout(left) {
            Ok(UserSample::Alive {
                payload,
                representation_identifier,
                ..
            }) => {
                assert_eq!(representation_identifier, [0x00, 0x01]);
                assert_eq!(&payload[..], &body[..]);
                taken += 1;
            }
            Ok(_) => {}
            Err(_) => panic!(
                "{taken} of 2000 samples reached the DDS reader; the daemon logged:\n{}",
                daemon
                    .log_text()
                    .lines()
                    .filter(|l| !l.contains("\"grpc_status\":0"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        }
    }
    dds.shutdown();
}

/// Spec §10a against BloomRPC 1.5.3, the gRPC GUI client (Electron, its
/// gRPC the native Node package `grpc` 1.24 on gRPC C-core, the `.proto`
/// loaded by its own protobufjs loader): `tests/interop/bloomrpc_drive.py`
/// starts it under Xvfb with the shipped `.proto` imported and the tabs
/// PublishOne and Catalog open, presses each play button over the DevTools
/// protocol and reads the response pane. The `PublishOne` sample reaches the
/// DDS DataReader of the test octet for octet; BloomRPC shows the
/// `PublishAck` with sequence number 1 and the `CatalogResp` of the topic.
#[test]
#[ignore = "needs BloomRPC (/opt/bloomrpc or ZERODDS_BLOOMRPC), xvfb-run and Python websockets; runs in integration-bridges-vendor-grpc"]
fn grpcbridge_10_bloomrpc_calls_publish_one_and_catalog() {
    use base64::Engine as _;
    let bloomrpc = std::env::var_os("ZERODDS_BLOOMRPC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/bloomrpc/bloomrpc"));
    assert!(
        bloomrpc.is_file(),
        "{} is not there (tests/integration/provision.sh bloomrpc)",
        bloomrpc.display()
    );
    let lease = zerodds_test_domains::lease_domain();
    let daemon = chat_daemon(
        &lease.id().to_string(),
        &chat_topic("ChatMessage", "Chat::Message", "ChatMessageStream", true),
    );
    let dds = dds_peer(lease.id(), 1);
    let (_, samples) = chat_reader(&dds);
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let body = every_octet(0x5a);
    let out = Command::new("python3")
        .arg(manifest.join("tests/interop/bloomrpc_drive.py"))
        .arg(&bloomrpc)
        .arg(daemon.address())
        .arg(manifest.join("proto"))
        .arg(base64::engine::general_purpose::STANDARD.encode(encapsulated([0x00, 0x01], &body)))
        .output()
        .expect("spawn python3");
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert!(
        out.status.success(),
        "bloomrpc_drive.py: {:?} {lines:?} {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with("publish-one {\"sequence_number\":\"1\",\"acked_at_ns\":\""),
        "{lines:?}"
    );
    assert_eq!(next_alive(&samples), ([0x00, 0x01], body));
    assert_eq!(
        lines[1],
        "catalog {\"topics\":[{\"dds_name\":\"ChatMessage\",\"dds_type\":\"Chat::Message\",\
         \"grpc_service\":\"zerodds.chat.v1.ChatMessageStream\",\"idl_url\":\"\"}]}"
    );
    dds.shutdown();
}
