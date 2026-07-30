#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable endpoint demo (DDS-XRCE reliable stream) for the pure-Python
# endpoint SDK. Mirrors endpoints/rust/examples/example_reliable.rs and
# endpoints/nim/reliable_app.nim.
#
# Modes:
#   example_reliable.py <port> <n> [run]   -- reliable sender: enqueues n
#     4-byte-LE samples through the async-decoupled ReliableWriter against
#     127.0.0.1:<port> (used by the endpoint-e2e loss-recovery test).
#   example_reliable.py <port> 0 bench     -- producer latency: enqueue
#     (decoupled) vs. inline socket.send. Prints "BENCH ...".
#   example_reliable.py                    -- standalone in-process demo: a
#     lossy receiver thread + this sender; prints the recovered sequence.

import socket
import struct
import sys
import threading
import time

from zerodds_reliable import (
    ReliableReceiver,
    ReliableWriter,
    acknack_frame,
    parse_heartbeat,
    reliable_unframe,
    reliable_write_frame,
)


def _open_socket(port=None):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    if port is not None:
        sock.connect(("127.0.0.1", port))
    return sock


def run_reliable(port, n):
    """E2E sender: enqueue n samples (u32 LE index) through the decoupled
    writer, close (drains the window), then exit."""
    sock = _open_socket(port)
    writer = ReliableWriter(sock)
    writer.start()
    for i in range(n):
        writer.enqueue(struct.pack("<I", i))
    writer.close()
    print("SENT %d" % n)


def run_bench(port):
    """Producer latency: decoupled enqueue() vs. inline socket.send()."""
    iters = 20000
    sample = struct.pack("<I", 0)

    # Inline: the producer thread itself does the send syscall.
    sock = _open_socket(port)
    t0 = time.perf_counter()
    for i in range(iters):
        sock.send(reliable_write_frame(i & 0xFFFF, sample))
    inline_ns = (time.perf_counter() - t0) * 1e9 / iters
    sock.close()

    # Decoupled: the producer only enqueues; a drain thread does the I/O
    # against a sink socket (an idle-draining thread, so sends never block).
    sink = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sink.bind(("127.0.0.1", 0))
    sink_port = sink.getsockname()[1]
    sink.settimeout(0.01)
    stop_sink = threading.Event()

    def sink_loop():
        while not stop_sink.is_set():
            try:
                sink.recv(2048)
            except socket.timeout:
                continue
            except OSError:
                break

    sink_thread = threading.Thread(target=sink_loop, daemon=True)
    sink_thread.start()

    dsock = _open_socket(sink_port)
    writer = ReliableWriter(dsock, maxsize=1 << 20)
    writer.start()
    t1 = time.perf_counter()
    for _ in range(iters):
        writer.enqueue(sample)
    enqueue_ns = (time.perf_counter() - t1) * 1e9 / iters
    writer.close()
    stop_sink.set()
    sink_thread.join(timeout=1.0)
    sink.close()
    dsock.close()

    print("BENCH enqueue_ns=%d inline_send_ns=%d" % (int(enqueue_ns), int(inline_ns)))


def standalone_demo():
    """In-process demo: a lossy receiver thread + the decoupled sender; the
    receiver prints the recovered contiguous sequence."""
    n = 12
    rx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    rx.bind(("127.0.0.1", 0))
    rx_port = rx.getsockname()[1]
    rx.settimeout(0.2)

    delivered = []

    def receiver():
        state = ReliableReceiver()
        dropped_once = set()
        count = 0
        last_addr = None
        deadline = time.monotonic() + 10.0
        while len(delivered) < n and time.monotonic() < deadline:
            try:
                data, addr = rx.recvfrom(2048)
            except socket.timeout:
                continue
            last_addr = addr
            unframed = reliable_unframe(data)
            if unframed is not None:
                seq, body = unframed
                count += 1
                # Drop every 3rd distinct sample once, to force a retransmit.
                if count % 3 == 0 and seq not in dropped_once:
                    dropped_once.add(seq)
                else:
                    state.recv_data(seq, body)
                    for _seq, payload in state.drain_in_order():
                        delivered.append(struct.unpack("<I", payload)[0])
                rx.sendto(acknack_frame(1, *state.pending_acknack()), addr)
            elif parse_heartbeat(data) is not None:
                rx.sendto(acknack_frame(1, *state.pending_acknack()), addr)
        if last_addr is not None:
            for _ in range(3):
                rx.sendto(acknack_frame(1, *state.pending_acknack()), last_addr)

    rx_thread = threading.Thread(target=receiver)
    rx_thread.start()

    sock = _open_socket(rx_port)
    writer = ReliableWriter(sock)
    writer.start()
    for i in range(n):
        writer.enqueue(struct.pack("<I", i))
    writer.close()

    rx_thread.join(timeout=15.0)
    print("RECOVERED %r" % delivered)
    assert delivered == list(range(n)), "all samples must arrive gap-free in order"
    print("OK: %d samples delivered gap-free in order despite injected loss" % n)


def main():
    if len(sys.argv) < 2:
        standalone_demo()
        return 0
    port = int(sys.argv[1])
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    mode = sys.argv[3] if len(sys.argv) > 3 else "run"
    if mode == "bench":
        run_bench(port)
    else:
        run_reliable(port, n)
    return 0


if __name__ == "__main__":
    sys.exit(main())
