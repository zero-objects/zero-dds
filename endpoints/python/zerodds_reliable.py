#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# zerodds_reliable -- reliable XRCE stream (spec Sec 8.4.10/Sec 8.4.11) for the
# pure-Python endpoint SDK (ADR 0013). Self-contained: the sender/receiver
# state machines + the WRITE_DATA/HEARTBEAT/ACKNACK wire codec, byte-identical
# to `crates/xrce/src/reliable.rs` and every other endpoint SDK
# (`endpoints/c/src/zerodds_reliable_async.c`, `endpoints/rust/src/reliable.rs`,
# `endpoints/nim/reliable.nim`, ...).
#
# - ReliableSender: `submit()` buffers outgoing samples (window cap 16),
#   `pending_heartbeat()` announces the in-flight range, `recv_acknack()`
#   clears acknowledged seqnrs and keeps the still-missing ones.
# - ReliableReceiver: `recv_data()` buffers out-of-order (cap 64),
#   `drain_in_order()` delivers contiguously from `expected`,
#   `pending_acknack()` reports the missing seqnrs as a 16-bit bitmap.
# - ReliableWriter: the async-decoupled writer. The producer calls
#   `enqueue()`, which puts the sample on a `queue.Queue` -- a lock + list
#   append, NOT a socket call. A drain `threading.Thread` owns the
#   ReliableSender state and the UDP socket: it pulls queued samples into the
#   sender window, frames + sends WRITE_DATA, emits period-gated HEARTBEATs,
#   and retransmits on ACKNACK until the window drains.
#
#   Honesty note (this is Python, not Rust/C): `queue.Queue` is a
#   lock-protected deque, not a wait-free SPSC ring -- `enqueue()` briefly
#   acquires a lock. What *is* real is the I/O decoupling: the GIL is
#   released around the drain thread's blocking `socket.send`/`recv` calls,
#   so the producer's `enqueue()` never blocks on a syscall, and the drain
#   thread's socket I/O genuinely runs concurrently with the producer doing
#   other work. This is "thread + GIL-released-syscalls" decoupling, not a
#   lock-free data-plane.
#
# Wire (little-endian, 8-byte header + body):
#   [0]=session [1]=stream [2..4]=seq u16 LE [4]=submsg id [5]=flags
#   [6..8]=body-len u16 LE [8..]=body
#   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = the sample's
#     RFC-1982 sequence number, on the reliable stream (id 0x80).
#   HEARTBEAT id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
#   ACKNACK   id=0x0A flags=0x01 body(5)= first_unacked i16 LE, nack[0],
#     nack[1], stream
#   HEARTBEAT/ACKNACK travel on the control stream (session 0x80, stream
#   NONE 0x00); the target reliable stream id rides in the body's last byte.
#
# A signed i16 field and an unsigned u16 with the same bit pattern encode to
# the same two bytes in little-endian -- this module packs everything as
# unsigned ('H'/'B') and never needs a separate signed reinterpret-cast.

import queue
import socket
import struct
import threading
import time

# --- constants ---------------------------------------------------------

XRCE_SESSION_NOKEY = 0x80
XRCE_STREAM_NONE = 0x00
RELIABLE_STREAM_ID = 0x80

_SM_WRITE_DATA = 0x07
_SM_ACKNACK = 0x0A
_SM_HEARTBEAT = 0x0B
_WRITE_FLAGS = 0x03
_CTRL_FLAGS = 0x01

# Heartbeat period (spec recommends 100 ms; 500 ms here -- no Tx-pacing layer
# underneath, matches every other endpoint SDK's default).
HEARTBEAT_PERIOD_S = 0.5
# Sender window cap: 16 in-flight samples (matches the 16-bit NACK bitmap).
SENDER_WINDOW = 16
# Receiver out-of-order buffer cap (DoS bound).
RECEIVER_BUFFER = 64
# Per-sample payload cap (u16 submessage length limit).
MAX_PAYLOAD = 65535

_HEADER_FMT = struct.Struct("<BBHBBH")  # session,stream,seq,submsg,flags,len
_HEARTBEAT_BODY_FMT = struct.Struct("<HHB")  # first,last,stream
_ACKNACK_BODY_FMT = struct.Struct("<HBBB")  # first_unacked,nack_lo,nack_hi,stream


# --- RFC-1982 16-bit serial-number comparison ---------------------------


def seq_lt(a, b):
    """`a < b` in RFC-1982 order (half-window = 32768)."""
    diff = (b - a) & 0xFFFF
    return diff != 0 and diff < 0x8000


def seq_gt(a, b):
    return seq_lt(b, a)


# --- wire codec ----------------------------------------------------------


def reliable_write_frame(seq, sample):
    """Reliable WRITE_DATA frame; header seq carries the sample's seqnr."""
    sample = bytes(sample)
    header = _HEADER_FMT.pack(
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        seq & 0xFFFF,
        _SM_WRITE_DATA,
        _WRITE_FLAGS,
        len(sample) & 0xFFFF,
    )
    return header + sample


def reliable_unframe(frame):
    """`(seq, sample)` from a WRITE_DATA frame, or `None`."""
    if len(frame) < 8 or frame[4] != _SM_WRITE_DATA:
        return None
    seq = struct.unpack_from("<H", frame, 2)[0]
    return seq, bytes(frame[8:])


def heartbeat_frame(msg_seq, first, last, stream_id,
                     session=XRCE_SESSION_NOKEY, stream_hdr=XRCE_STREAM_NONE):
    """HEARTBEAT control frame. Byte-identical to `golden_heartbeat_le.bin`
    for `(session=0x80, stream_hdr=0x00, msg_seq=1, first=1, last=3,
    stream_id=0x80)`."""
    header = _HEADER_FMT.pack(session, stream_hdr, msg_seq & 0xFFFF,
                               _SM_HEARTBEAT, _CTRL_FLAGS, 5)
    body = _HEARTBEAT_BODY_FMT.pack(first & 0xFFFF, last & 0xFFFF, stream_id & 0xFF)
    return header + body


def parse_heartbeat(frame):
    """`(first, last, stream)` from a HEARTBEAT frame, or `None`."""
    if len(frame) < 13 or frame[4] != _SM_HEARTBEAT:
        return None
    return _HEARTBEAT_BODY_FMT.unpack_from(frame, 8)


def acknack_frame(msg_seq, first_unacked, nack_bitmap, stream_id,
                   session=XRCE_SESSION_NOKEY, stream_hdr=XRCE_STREAM_NONE):
    """ACKNACK control frame. Byte-identical to `golden_acknack_le.bin` for
    `(session=0x80, stream_hdr=0x00, msg_seq=1, first_unacked=1,
    nack_bitmap=0, stream_id=0x80)`."""
    header = _HEADER_FMT.pack(session, stream_hdr, msg_seq & 0xFFFF,
                               _SM_ACKNACK, _CTRL_FLAGS, 5)
    body = _ACKNACK_BODY_FMT.pack(
        first_unacked & 0xFFFF,
        nack_bitmap & 0xFF,
        (nack_bitmap >> 8) & 0xFF,
        stream_id & 0xFF,
    )
    return header + body


def parse_acknack(frame):
    """`(first_unacked, nack_bitmap, stream)` from an ACKNACK frame, or `None`."""
    if len(frame) < 13 or frame[4] != _SM_ACKNACK:
        return None
    first_unacked, lo, hi, stream = _ACKNACK_BODY_FMT.unpack_from(frame, 8)
    return first_unacked, lo | (hi << 8), stream


# --- sender ----------------------------------------------------------------


class ReliableSender(object):
    """Reliable sender: assigns seqnrs, holds in-flight samples until
    acknowledged, emits HEARTBEATs, clears/keeps samples on ACKNACK."""

    def __init__(self):
        self._next_seq = 0
        self._in_flight = {}  # seq -> bytes
        self._last_heartbeat = None  # time.monotonic() seconds, or None

    def in_flight_count(self):
        return len(self._in_flight)

    def submit(self, payload):
        """Buffers a new sample. Returns `(status, seq)` with status one of
        `'ok'`, `'too_large'`, `'window_full'` (`seq` is `None` on error).
        On `'window_full'` the caller must process ACKNACKs first."""
        payload = bytes(payload)
        if len(payload) > MAX_PAYLOAD:
            return "too_large", None
        if len(self._in_flight) >= SENDER_WINDOW:
            return "window_full", None
        seq = self._next_seq
        self._in_flight[seq] = payload
        self._next_seq = (self._next_seq + 1) & 0xFFFF
        return "ok", seq

    def get_in_flight(self, seq):
        """The in-flight payload for `seq`, or `None`."""
        return self._in_flight.get(seq & 0xFFFF)

    def in_flight_items(self):
        """Ascending `[(seq, payload), ...]` -- deterministic retransmit order."""
        return [(k, self._in_flight[k]) for k in sorted(self._in_flight.keys())]

    def pending_heartbeat(self, now):
        """`(first, last, stream_id)` if the heartbeat period elapsed and
        samples are in flight, else `None`. `now` is `time.monotonic()`."""
        if not self._in_flight:
            return None
        due = (self._last_heartbeat is None
               or (now - self._last_heartbeat) >= HEARTBEAT_PERIOD_S)
        if not due:
            return None
        self._last_heartbeat = now
        keys = sorted(self._in_flight.keys())
        return keys[0], keys[-1], RELIABLE_STREAM_ID

    def force_heartbeat(self):
        """Resets the heartbeat timer so the next `pending_heartbeat` fires
        immediately (used while draining towards shutdown)."""
        self._last_heartbeat = None

    def recv_acknack(self, first_unacked, nack_bitmap):
        """`first_unacked` = smallest seqnr the receiver still expects;
        everything strictly before it (RFC-1982) is acknowledged and
        dropped. Within `[first_unacked, first_unacked+16)` a clear bit
        means acked (drop), a set bit means missing (keep)."""
        base = first_unacked & 0xFFFF
        to_remove = [
            k for k in self._in_flight
            if (diff := (base - k) & 0xFFFF) != 0 and diff < 0x8000
        ]
        for k in to_remove:
            del self._in_flight[k]
        for i in range(16):
            seq = (base + i) & 0xFFFF
            if ((nack_bitmap >> i) & 1) == 0 and seq in self._in_flight:
                del self._in_flight[seq]

    def reset(self):
        self._next_seq = 0
        self._in_flight.clear()
        self._last_heartbeat = None


# --- receiver ----------------------------------------------------------------


class ReliableReceiver(object):
    """Reliable receiver: buffers out-of-order samples, delivers them
    contiguously, and reports the missing seqnrs for ACKNACK."""

    def __init__(self):
        self._expected = 0
        self._received = {}  # seq -> bytes

    def expected(self):
        return self._expected

    def out_of_order_count(self):
        return len(self._received)

    def recv_data(self, seq, payload):
        """Buffers `(seq, payload)`. Returns `False` only when the receiver
        buffer is full (DoS bound); duplicates are a silent no-op returning
        `True`."""
        seq = seq & 0xFFFF
        if seq_lt(seq, self._expected):
            return True  # already delivered -> drop
        if seq in self._received:
            return True  # already buffered -> drop
        if len(self._received) >= RECEIVER_BUFFER:
            return False
        self._received[seq] = bytes(payload)
        return True

    def drain_in_order(self):
        """Delivers all contiguous samples from `expected`, advancing it.
        Returns `[(seq, payload), ...]`."""
        out = []
        while self._expected in self._received:
            out.append((self._expected, self._received.pop(self._expected)))
            self._expected = (self._expected + 1) & 0xFFFF
        return out

    def pending_acknack(self, hint_last_seen=None):
        """`(first_unacked, nack_bitmap, stream_id)` marking the missing
        slots in `[expected, expected+16)`. Slots strictly after
        `hint_last_seen` (if given) are not marked missing."""
        base = self._expected
        bitmap = 0
        for i in range(16):
            seq = (base + i) & 0xFFFF
            if hint_last_seen is not None and seq_gt(seq, hint_last_seen):
                continue
            if seq not in self._received:
                bitmap |= 1 << i
        return base, bitmap, RELIABLE_STREAM_ID

    def reset(self):
        self._expected = 0
        self._received.clear()


# --- async-decoupled writer --------------------------------------------------


class ReliableWriter(object):
    """Async-decoupled reliable writer (see module docstring). `enqueue()` is
    the producer-side call: it never touches the socket. `start()`/`close()`
    manage the drain thread's lifecycle.

    The "no more samples" signal is a `threading.Event`, deliberately NOT a
    sentinel pushed through the payload queue: once the sender window (16
    in-flight) is full, the drain thread stops pulling from the queue until
    ACKNACKs free up slots, so a queue-sentinel could sit behind an unconsumed
    backlog forever if the peer stalls. The `Event` is checked independently
    of the queue on every loop iteration, exactly like the Rust/C writer's
    separate `running` atomic (not routed through their ring either)."""

    def __init__(self, sock, window=SENDER_WINDOW, maxsize=1024):
        self._sock = sock
        self._window = window
        self._queue = queue.Queue(maxsize=maxsize)
        self._closing = threading.Event()
        self._thread = None

    def start(self):
        self._thread = threading.Thread(target=self._drain, daemon=True)
        self._thread.start()

    def enqueue(self, payload):
        """Producer call: enqueues one sample body. Blocks only on queue
        backpressure (`maxsize` reached) -- never on socket I/O."""
        self._queue.put(bytes(payload))

    def close(self, timeout=30.0):
        """Signals the drain thread that no more samples are coming, and
        joins it (blocks until the send window drains, or `timeout`
        elapses as a safety valve if the peer stops ACKing)."""
        self._closing.set()
        if self._thread is not None:
            self._thread.join(timeout)

    def _drain(self):
        # Short timeout: the drain loop must keep servicing the heartbeat
        # timer and the queue even with no ACKNACK traffic.
        self._sock.settimeout(0.005)
        sender = ReliableSender()
        ctl_seq = 1
        close_deadline = None

        while True:
            # 1) Pull queued samples into the sender window; send WRITE_DATA.
            while sender.in_flight_count() < self._window:
                try:
                    item = self._queue.get_nowait()
                except queue.Empty:
                    break
                status, seq = sender.submit(item)
                if status == "ok":
                    self._safe_send(reliable_write_frame(seq, item))

            # 2) Period-gated HEARTBEAT.
            hb = sender.pending_heartbeat(time.monotonic())
            if hb is not None:
                first, last, stream_id = hb
                self._safe_send(heartbeat_frame(ctl_seq, first, last, stream_id))
                ctl_seq = (ctl_seq + 1) & 0xFFFF

            # 3) Drain incoming ACKNACKs; retransmit whatever is still
            #    marked missing.
            while True:
                try:
                    data = self._sock.recv(2048)
                except (socket.timeout, BlockingIOError):
                    break
                except OSError:
                    break
                ack = parse_acknack(data)
                if ack is None:
                    continue
                first_unacked, bitmap, _stream_id = ack
                sender.recv_acknack(first_unacked, bitmap)
                for i in range(16):
                    if (bitmap >> i) & 1:
                        seq = (first_unacked + i) & 0xFFFF
                        payload = sender.get_in_flight(seq)
                        if payload is not None:
                            self._safe_send(reliable_write_frame(seq, payload))

            # 4) Exit once closed AND the window (+ queue backlog) has
            #    drained -- or, as a safety valve, once the post-close
            #    deadline elapses (a vanished peer must never wedge close()).
            if self._closing.is_set():
                if close_deadline is None:
                    close_deadline = time.monotonic() + 5.0
                if sender.in_flight_count() == 0 and self._queue.empty():
                    break
                if time.monotonic() > close_deadline:
                    break
                # Keep heartbeating so the peer keeps ACKing until drained.
                sender.force_heartbeat()

    def _safe_send(self, frame):
        try:
            self._sock.send(frame)
        except OSError:
            pass
