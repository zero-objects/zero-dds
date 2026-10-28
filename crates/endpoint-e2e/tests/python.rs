// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Python endpoint ping-pong: raw generated codegen over UDP, plus the full
//! stack (generated dataclasses + endpoint SDK, sync `Client` and
//! an asyncio reader) with XRCE framing. Gated on `python3`.
//!
//! The app marshals with the generated `zd_enc_Ping` / `zd_dec_Pong`, which
//! call the `runtime/python` wire runtime (`zerodds_wire`, pure Python), and
//! frames/transfers with the `endpoints/python` SDK (`zerodds_endpoint`).

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};

use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

fn python_available() -> bool {
    tool_available("python3", "--version")
}

/// The imports the generated codec reaches by bare name. `Reader`, `Writer`
/// and the handle types are annotations on its own functions; `utf16_units`
/// and `utf8_octets` live in `zerodds_wire.typesupport`, which the package's
/// `__init__` does not re-export. Taken from what
/// `wireprobe-zerodds-python-gen` emits, so the two columns import the same
/// runtime the same way.
const PY_RUNTIME_IMPORTS: &str = "\
from zerodds_wire import Endianness, PlCdr1Framing, Reader, Writer, DHeader, \
EmHeader, EmHeaderRead, Pid, PidRead, ReaderMark, ParameterRole, Status, \
TryConstruct, Float128, no_dheader, bit_pattern_bool, bit_pattern_f32, \
bit_pattern_f64, bit_pattern_int, WireError
from zerodds_wire.typesupport import utf16_units, utf8_octets
import zerodds_wire as zd_wire
";

/// The generated `Ping`/`Pong` module, with the runtime imports the codec needs
/// spliced in after the generator's own import block.
///
/// After, not before: the generated module opens with a docstring and
/// `from __future__ import annotations`, and a `__future__` import that is not
/// the first statement after the docstring is a `SyntaxError`. The generator's
/// block ends with `import typing`, which is what this splices behind.
fn gen_module() -> String {
    let text = source(Language::Python);
    let anchor = "\nimport typing\n";
    let at = text
        .find(anchor)
        .map(|i| i + anchor.len())
        .expect("generated Python ends its import block with `import typing`");
    format!("{}{PY_RUNTIME_IMPORTS}{}", &text[..at], &text[at..])
}

// The one app; `sys.argv[1]` selects raw / sync / async. It imports the
// generated `pingpong_gen` (same dir) plus `zerodds_endpoint` (SDK) and the
// `zerodds` runtime (both on `PYTHONPATH`).
const PY_APP: &str = r#"import sys
import socket
import time

import pingpong_gen as pg
import zerodds_wire as zd_wire
import zerodds_endpoint as ze


def encode_ping():
    """The shared Ping, through the generated encoder (XCDR2 LE, MAXALIGN 4)."""
    w = zd_wire.Writer(zd_wire.Endianness.LITTLE, 4, zd_wire.PlCdr1Framing(), False)
    pg.zd_enc_Ping(w, pg.Ping(seq=1, msg="hello from app"))
    return w.finish()


def decode_pong(body):
    """A Pong body, through the generated decoder."""
    r = zd_wire.Reader(body, zd_wire.Endianness.LITTLE, 4, zd_wire.PlCdr1Framing(), False)
    return pg.zd_dec_Pong(r, zd_wire.TryConstruct.DISCARD)


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
    sock.sendto(encode_ping(), peer)
    data, _ = sock.recvfrom(4096)
    pong = decode_pong(data)
    print("PONG seq=%d reply=%s" % (pong.seq, pong.reply))


def run_sync(port):
    sock, peer = _open(port)
    sock.settimeout(0.05)
    client = ze.Client(UdpTransport(sock, peer), ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER))
    client.write(encode_ping())
    deadline = time.time() + 10.0
    body = None
    while time.time() < deadline:
        body = client.poll()
        if body is not None:
            break
    if body is None:
        raise SystemExit("sync: no pong")
    pong = decode_pong(body)
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
        ze.Client(tr, ze.xrce_object_id(0x001, ze.OBJK_DATAWRITER)).write(encode_ping())
        reader = AsyncReader(tr)

        async def first():
            async for body in reader.stream():
                return body
            return None

        body = await asyncio.wait_for(first(), timeout=10.0)
        pong = decode_pong(body)
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
    let dir = scratch_dir(&format!("py_{mode}"));
    std::fs::write(dir.join("pingpong_gen.py"), gen_module()).expect("write gen");
    std::fs::write(dir.join("app.py"), PY_APP).expect("write app");
    // `runtime/python` FIRST, and the order is load-bearing: `endpoints/python`
    // ships its own `zerodds_wire.py` — the SDK's older, flat wire-core — which
    // would shadow the runtime package the generated codec imports
    // (`Endianness`, `PlCdr1Framing`, `Reader`, `Writer`, …). The SDK itself
    // imports no wire module, so the runtime winning costs it nothing.
    let pythonpath = format!(
        "{}:{}",
        runtime_dir("python").display(),
        endpoints_dir("python").display()
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
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = spawn("raw", port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = xcdr2_reader(&buf[..n]);
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = xcdr2_writer();
    w.write_u32(seq);
    w.write_string(&format!("pong:{msg}"));
    sock.send_to(&w.finish(), app).expect("send");
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
