// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Python reliable endpoint, live: the pure-Python reliable state machine +
//! frame codec (`endpoints/python/zerodds_reliable.py`) driven by its
//! decoupled `queue.Queue` + drain-`threading.Thread` writer against the
//! shared Rust reliable peer: the peer drops, the app retransmits on ACKNACK,
//! all samples arrive gap-free. The samples are the `Ping` of the codec the
//! Python generator writes from the IDL, so the run checks IDL, generated
//! codec and runtime together. And the producer-latency
//! comparison. The unit and byte-golden suite runs in the endpoints-gate column of Python.
//! Gated on `python3`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, endpoints_dir, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn python_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

fn endpoints_python() -> PathBuf {
    Path::new(&format!(
        "{}/../../endpoints/python",
        env!("CARGO_MANIFEST_DIR")
    ))
    .canonicalize()
    .expect("endpoints/python path")
}

/// The imports the generated codec reaches by bare name; the same list
/// `tests/python.rs` splices behind the generator's import block.
const PY_RUNTIME_IMPORTS: &str = "\
from zerodds_wire import Endianness, PlCdr1Framing, Reader, Writer, DHeader, \
EmHeader, EmHeaderRead, Pid, PidRead, ReaderMark, ParameterRole, Status, \
TryConstruct, Float128, no_dheader, bit_pattern_bool, bit_pattern_f32, \
bit_pattern_f64, bit_pattern_int, WireError
from zerodds_wire.typesupport import utf16_units, utf8_octets
import zerodds_wire as zd_wire
";

/// The generated `Ping`/`Pong` module with those imports behind the
/// generator's own import block, which ends with `import typing`.
fn gen_module() -> String {
    let text = source(Language::Python);
    let anchor = "\nimport typing\n";
    let at = text
        .find(anchor)
        .map(|i| i + anchor.len())
        .expect("generated Python ends its import block with `import typing`");
    format!("{}{PY_RUNTIME_IMPORTS}{}", &text[..at], &text[at..])
}

/// The reliable app: the generated codec encodes `Ping(seq=i,
/// msg="reliable")` and the SDK's async-decoupled `ReliableWriter`
/// (`endpoints/python/zerodds_reliable.py`) sends it.
const PY_RELIABLE_APP: &str = r#"import socket
import sys

import pingpong_gen as pg
import zerodds_wire as zd_wire
from zerodds_reliable import ReliableWriter

WRITER_ID = 0x0015


def encode_ping(i):
    w = zd_wire.Writer(zd_wire.Endianness.LITTLE, 4, zd_wire.PlCdr1Framing(), False)
    pg.zd_enc_Ping(w, pg.Ping(seq=i, msg="reliable"))
    return w.finish()


def main():
    port, n = int(sys.argv[1]), int(sys.argv[2])
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    sock.connect(("127.0.0.1", port))
    writer = ReliableWriter(sock, WRITER_ID)
    writer.start()
    for i in range(n):
        writer.enqueue(encode_ping(i))
    writer.close()
    print("SENT %d" % n)
    return 0


if __name__ == "__main__":
    sys.exit(main())
"#;

fn run_loss(drop_every: Option<usize>, label: &str) {
    if !python_available() {
        eprintln!("SKIP python_reliable {label}: `python3` not on PATH");
        return;
    }
    let n = RELIABLE_SAMPLES;
    let dir = scratch_dir(&format!("py_reliable_{}", label.replace('/', "_")));
    std::fs::write(dir.join("pingpong_gen.py"), gen_module()).expect("write gen");
    std::fs::write(dir.join("app.py"), PY_RELIABLE_APP).expect("write app");
    // `runtime/python` first: `endpoints/python` ships its own flat
    // `zerodds_wire.py`, which would shadow the package the generated codec
    // imports (`tests/python.rs`).
    let pythonpath = format!(
        "{}:{}",
        runtime_dir("python").display(),
        endpoints_dir("python").display()
    );
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new("python3")
        .arg(dir.join("app.py"))
        .arg(peer.port.to_string())
        .arg(n.to_string())
        .env("PYTHONPATH", pythonpath)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3 reliable app");
    let delivered = reliable_receive(&peer, child, label, n);
    assert_reliable_pings(label, &delivered, n);
}

/// reliable-endpoint 1.0 §5 point 3: the Python app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the Python toolchain; runs in integration-bindings"]
fn reliable_5_3_python_samples_arrive_in_order_despite_dropped_datagrams() {
    run_loss(Some(3), "python/loss");
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Python producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the Python toolchain; runs in integration-bindings"]
fn reliable_5_4_python_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !python_available() {
        eprintln!("SKIP python_reliable bench: `python3` not on PATH");
        return;
    }
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let out = Command::new("python3")
        .arg(endpoints_python().join("example_reliable.py"))
        .arg(peer.port.to_string())
        .arg("0")
        .arg("bench")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("run python reliable bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("BENCH"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("python reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("python", &stdout);
}
