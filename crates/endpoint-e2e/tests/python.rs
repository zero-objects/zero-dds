// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Python endpoint ping-pong: raw generated codegen over UDP, plus the full
//! stack (generated `@idl_struct` dataclasses + endpoint SDK, sync `Client` and
//! an asyncio reader) with XRCE framing. Gated on `python3`.
//!
//! The app marshals with the GENERATED `Ping.encode()` (the `zerodds` runtime's
//! pure-Python XCDR2-LE codec — the native `_core` ext is not required) and
//! frames/transfers with the `endpoints/python` SDK (`zerodds_endpoint`).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{PythonGenOptions, generate_python_module};

fn python_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

/// Generated Ping/Pong module (`@idl_struct` dataclasses over `zerodds.idl`).
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_python_module(&ast, &PythonGenOptions::default()).expect("gen")
}

/// `endpoints/python` (the SDK, `zerodds_endpoint.py`).
fn endpoints_python() -> PathBuf {
    Path::new(&format!(
        "{}/../../endpoints/python",
        env!("CARGO_MANIFEST_DIR")
    ))
    .canonicalize()
    .expect("endpoints/python path")
}

/// `crates/py/python` (parent of the `zerodds` runtime package).
fn zerodds_pkg() -> PathBuf {
    Path::new(&format!(
        "{}/../../crates/py/python",
        env!("CARGO_MANIFEST_DIR")
    ))
    .canonicalize()
    .expect("crates/py/python path")
}

// The one app; `sys.argv[1]` selects raw / sync / async. It imports the
// generated `pingpong_gen` (same dir) plus `zerodds_endpoint` (SDK) and the
// `zerodds` runtime (both on `PYTHONPATH`).
const PY_APP: &str = r#"import sys
import socket
import time

from pingpong_gen import Ping, Pong
import zerodds_endpoint as ze


class UdpTransport(object):
    """Real-UDP transport for the endpoint SDK: deliver()/receive()."""

    def __init__(self, sock, peer):
        self.sock = sock
        self.peer = peer

    def deliver(self, frame):
        self.sock.sendto(frame, self.peer)

    def receive(self):
        try:
            data, _ = self.sock.recvfrom(4096)
            return data
        except (socket.timeout, BlockingIOError):
            return None


def _open(peer_port):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    return sock, ("127.0.0.1", peer_port)


def run_raw(port):
    # Bare XCDR2 sample straight from the generated marshal, no XRCE frame.
    sock, peer = _open(port)
    sock.settimeout(10.0)
    sock.sendto(Ping(1, "hello from app").encode(), peer)
    data, _ = sock.recvfrom(4096)
    pong = Pong.decode(data)
    print("PONG seq=%d reply=%s" % (pong.seq, pong.reply))


def run_sync(port):
    sock, peer = _open(port)
    sock.settimeout(0.05)
    client = ze.Client(UdpTransport(sock, peer))
    client.write(Ping(1, "hello from app").encode())
    deadline = time.time() + 10.0
    body = None
    while time.time() < deadline:
        body = client.poll()
        if body is not None:
            break
    if body is None:
        raise SystemExit("sync: no pong")
    pong = Pong.decode(body)
    print("PONG seq=%d reply=%s" % (pong.seq, pong.reply))


def run_async(port):
    import asyncio

    class AsyncReader(object):
        """asyncio reader over the SDK: an async generator of sample bodies."""

        def __init__(self, transport):
            self.transport = transport

        async def stream(self):
            while True:
                frame = self.transport.receive()
                if frame is None:
                    await asyncio.sleep(0.001)
                    continue
                yield ze.xrce_read_frame(frame)

    async def main():
        sock, peer = _open(port)
        sock.settimeout(0)
        tr = UdpTransport(sock, peer)
        ze.Client(tr).write(Ping(1, "hello from app").encode())
        reader = AsyncReader(tr)

        async def first():
            async for body in reader.stream():
                return body
            return None

        body = await asyncio.wait_for(first(), timeout=10.0)
        pong = Pong.decode(body)
        print("PONG seq=%d reply=%s" % (pong.seq, pong.reply))

    asyncio.run(main())


def main():
    mode = sys.argv[1]
    port = int(sys.argv[2])
    if mode == "raw":
        run_raw(port)
    elif mode == "sync":
        run_sync(port)
    elif mode == "async":
        run_async(port)
    else:
        raise SystemExit("unknown mode: " + mode)
    return 0


if __name__ == "__main__":
    sys.exit(main())
"#;

fn spawn(mode: &str, port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_py_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("pingpong_gen.py"), gen_module()).expect("write gen");
    std::fs::write(dir.join("app.py"), PY_APP).expect("write app");
    let pythonpath = format!(
        "{}:{}",
        endpoints_python().display(),
        zerodds_pkg().display()
    );
    Command::new("python3")
        .arg(dir.join("app.py"))
        .arg(mode)
        .arg(port.to_string())
        .env("PYTHONPATH", pythonpath)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3")
}

// ---- 1) raw codegen over a plain UDP socket ----

#[test]
fn python_raw_udp() {
    if !python_available() {
        eprintln!("SKIP python_raw_udp: `python3` not on PATH");
        return;
    }
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = spawn("raw", port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = BufferReader::new(&buf[..n], Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("s");
    w.write_string(&format!("pong:{msg}")).expect("r");
    sock.send_to(&w.into_bytes(), app).expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) full stack: generated types + endpoint SDK (sync/async) over XRCE ----

fn endpoint_case(mode: &str) {
    if !python_available() {
        eprintln!("SKIP python endpoint {mode}: `python3` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = spawn(mode, peer.port);
    ping_pong(&peer, child, &format!("python/{mode}"));
}

#[test]
fn python_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn python_endpoint_async() {
    endpoint_case("async");
}
