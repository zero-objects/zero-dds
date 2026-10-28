// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the foreign-ORB tests (`omniorb_e2e.rs`, `jacorb_e2e.rs`,
//! `tao_e2e.rs`) share: the bridge daemon in this process, the DDS backend
//! of `Calc.add` and `Calc.echo`, the notify writer of `Listener.on_value`,
//! the mappings of `tests/omniorb/Calc.idl`, and a `zerodds-naming-svc`
//! child process.

#![allow(dead_code)]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zerodds_corba_dds_bridge::bridge_daemon::{self, DaemonConfig};
use zerodds_corba_dds_bridge::daemon_runtime::BridgeMetrics;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_monitor::Registry;
use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_test_domains::DomainLease;

/// `tests/omniorb/Calc.idl`, the one IDL every foreign ORB compiles.
pub fn calc_idl() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/omniorb/Calc.idl")
}

/// The domain the bridge and the backend share, held for the test's run.
pub fn lease() -> DomainLease {
    zerodds_test_domains::lease_domain()
}

/// A free TCP port on the loopback interface.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Waits until something accepts on `127.0.0.1:port`.
pub fn wait_for_listener(port: u16, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "{what} does not listen on {port}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The mappings of `Calc` (`add`, `never`, `echo`) and, with a listener
/// port, the DDS → CORBA route to a `Listener` at
/// `corbaloc:iiop:1.2@127.0.0.1:<port>/Listener`.
pub fn calc_mappings(listener_port: Option<u16>) -> String {
    let mut m = String::from(
        "mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Calc:1.0\"\n    operation: add\n    request_topic:\n      dds_name: Calc_add_Request\n    reply_topic:\n      dds_name: Calc_add_Reply\n    timeout_ms: 3000\n\
         \x20 - repo_id: \"IDL:zerodds_test/Calc:1.0\"\n    operation: never\n    request_topic:\n      dds_name: Calc_never_Request\n    reply_topic:\n      dds_name: Calc_never_Reply\n    timeout_ms: 1000\n\
         \x20 - repo_id: \"IDL:zerodds_test/Calc:1.0\"\n    operation: echo\n    request_topic:\n      dds_name: Calc_echo_Request\n    reply_topic:\n      dds_name: Calc_echo_Reply\n    timeout_ms: 5000\n",
    );
    if let Some(port) = listener_port {
        m.push_str(&format!(
            "\x20 - repo_id: \"IDL:zerodds_test/Listener:1.0\"\n    operation: on_value\n    direction: dds_to_corba\n    notify_topic:\n      dds_name: ListenerValues\n    targets:\n      - \"corbaloc:iiop:1.2@127.0.0.1:{port}/Listener\"\n"
        ));
    }
    m
}

/// The bridge daemon, run in a thread of this process.
pub struct Bridge {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    /// Starts the bridge on `yaml` and waits for `ior_file` to appear.
    pub fn start(yaml: &str, ior_file: &Path) -> Bridge {
        let mut c = zerodds_config::Config::default();
        c.push(
            zerodds_config::parse_str(yaml, "<test>").unwrap(),
            "<test>".into(),
        )
        .unwrap();
        let cfg = DaemonConfig::from_config(Some(&c)).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let registry = Registry::new();
                let metrics = BridgeMetrics::register(&registry);
                let r = bridge_daemon::run(&cfg, RuntimeConfig::default(), &stop, &metrics);
                assert!(r.is_ok(), "bridge: {r:?}");
            })
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ior_file.exists() {
            assert!(Instant::now() < deadline, "the bridge wrote no IOR file");
            std::thread::sleep(Duration::from_millis(50));
        }
        Bridge {
            stop,
            thread: Some(thread),
        }
    }

    /// Stops the bridge the way SIGTERM does (unbinding its names) and
    /// waits for it.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: topic.into(),
        ..Default::default()
    }
}

/// The DDS side of `Calc.add` and `Calc.echo`: reads the request samples the
/// bridge publishes and answers on the reply topics; `never` has no backend.
pub struct CalcBackend {
    pub dds: Arc<DcpsRuntime>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl CalcBackend {
    pub fn start(domain: i32) -> CalcBackend {
        // One prefix per backend of this process: a later test can lease the
        // same domain again, and two runtimes with one GUID prefix on one
        // domain are one participant to their peers.
        static STARTED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
        prefix[4..8].copy_from_slice(&domain.to_be_bytes());
        prefix[8..11].copy_from_slice(&STARTED.fetch_add(1, Ordering::Relaxed).to_be_bytes()[1..]);
        prefix[11] = 0xc0;
        let dds = DcpsRuntime::start(
            domain,
            GuidPrefix::from_bytes(prefix),
            RuntimeConfig::default(),
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let reader = |topic: &str| {
            dds.register_user_reader_kind(
                UserReaderConfig {
                    topic_name: topic.into(),
                    type_name: topic.into(),
                    reliable: true,
                    ..Default::default()
                },
                false,
            )
            .unwrap()
            .1
        };
        let writer = |topic: &str| {
            let w = dds
                .register_user_writer_kind(writer_cfg(topic), false)
                .unwrap();
            dds.set_user_writer_data_rep_override(w, Some(vec![dr::XCDR]))
                .unwrap();
            w
        };
        let add_requests = reader("Calc_add_Request");
        let add_writer = writer("Calc_add_Reply");
        let echo_requests = reader("Calc_echo_Request");
        let echo_writer = writer("Calc_echo_Reply");

        let add = {
            let dds = Arc::clone(&dds);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok(UserSample::Alive {
                        payload,
                        big_endian,
                        ..
                    }) = add_requests.recv_timeout(Duration::from_millis(200))
                    else {
                        continue;
                    };
                    let p = payload.as_ref();
                    let rd = |b: &[u8]| {
                        let a: [u8; 8] = b.try_into().unwrap();
                        if big_endian {
                            i64::from_be_bytes(a)
                        } else {
                            i64::from_le_bytes(a)
                        }
                    };
                    let (id, a, b) = (&p[..8], rd(&p[8..16]), rd(&p[16..24]));
                    let mut reply = id.to_vec();
                    reply.extend_from_slice(&[0; 8]); // reply_status 0, reserved 0
                    reply.extend_from_slice(&if big_endian {
                        (a + b).to_be_bytes()
                    } else {
                        (a + b).to_le_bytes()
                    });
                    dds.set_user_writer_byte_order_override(add_writer, big_endian)
                        .unwrap();
                    dds.write_user_sample_borrowed(add_writer, &reply).unwrap();
                }
            })
        };
        let echo = {
            let dds = Arc::clone(&dds);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok(UserSample::Alive {
                        payload,
                        big_endian,
                        ..
                    }) = echo_requests.recv_timeout(Duration::from_millis(200))
                    else {
                        continue;
                    };
                    // request_id, then the sequence as GIOP encoded it: echo it.
                    let p = payload.as_ref();
                    let mut reply = p[..8].to_vec();
                    reply.extend_from_slice(&[0; 8]);
                    reply.extend_from_slice(&p[8..]);
                    dds.set_user_writer_byte_order_override(echo_writer, big_endian)
                        .unwrap();
                    dds.write_user_sample_borrowed(echo_writer, &reply).unwrap();
                }
            })
        };
        CalcBackend {
            dds,
            stop,
            threads: vec![add, echo],
        }
    }

    /// Lets the bridge's readers match the backend's writers and back.
    pub fn settle(&self) {
        std::thread::sleep(Duration::from_secs(2));
    }

    /// Writes `values` on the notify topic once the bridge's reader matched.
    pub fn notify(&self, values: &[i64]) {
        let w = self
            .dds
            .register_user_writer_kind(writer_cfg("ListenerValues"), false)
            .unwrap();
        self.dds
            .set_user_writer_data_rep_override(w, Some(vec![dr::XCDR]))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.dds.user_writer_matched_count(w) == 0 {
            assert!(
                Instant::now() < deadline,
                "the bridge's notify reader did not match"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        for v in values {
            self.dds
                .write_user_sample_borrowed(w, &v.to_le_bytes())
                .unwrap();
        }
    }
}

impl Drop for CalcBackend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        // The runtime's own threads hold it; without this it keeps its
        // endpoints on the domain after the test released the lease.
        self.dds.shutdown();
    }
}

/// A child process, terminated (SIGTERM, then SIGKILL) when dropped.
pub struct ChildGuard(pub Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.0.id() as i32;
            // SAFETY: `id()` names our own child, which has not been waited on.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if let Ok(Some(_)) = self.0.try_wait() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `zerodds-naming-svc` on `127.0.0.1:<port>`, built by `cargo build` into
/// this workspace's target directory.
pub fn start_naming_svc(port: u16) -> ChildGuard {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .args([
            "build",
            "-q",
            "-p",
            "zerodds-corba-cosnaming",
            "--bin",
            "zerodds-naming-svc",
            "--features",
            "daemon",
        ])
        .status()
        .expect("run cargo build for zerodds-naming-svc");
    assert!(status.success(), "building zerodds-naming-svc failed");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"))
        .parent()
        .unwrap()
        .join("zerodds-naming-svc");
    let child = Command::new(&bin)
        .args(["--bind", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()));
    let guard = ChildGuard(child);
    wait_for_listener(port, "zerodds-naming-svc");
    guard
}

/// Runs `cmd` and returns its stdout; prints the exit status, stdout and
/// stderr for the test log.
pub fn output(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    eprintln!(
        "{cmd:?} ({}):\n{stdout}{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

/// Runs a build step and fails the test with its stderr if it fails.
pub fn build_step(cmd: &mut Command) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "{cmd:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
