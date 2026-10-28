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
#   WRITE_DATA id=0x07 flags=0x01 body = BaseObjectRequest + sample; header seq = the sample's
#     RFC-1982 sequence number, on the reliable stream (id 0x80).
#   HEARTBEAT id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
#   ACKNACK   id=0x0A flags=0x01 body(9)= DHEADER u32 LE (=5), first_unacked
#     i16 LE, nack[0], nack[1], stream. ACKNACK_Payload has no @extensibility,
#     so it is APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) and carries a
#     DHEADER in XCDR2 (unannotated_types="appendable", the default). With
#     unannotated_types="final" the body is the 5 octets of the FINAL form of
#     the reference implementation (eProsima), length field 5, 13-octet
#     frame. The parser accepts both forms in either setting.
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
# E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
# DDS-XRCE 1.0 Sec 8.3.5.8.
_WRITE_FLAGS = 0x01
# Wire size of the BaseObjectRequest that precedes the sample (Sec 7.7.8).
BASE_SIZE = 4
_CTRL_FLAGS = 0x01

# Heartbeat period (spec recommends 100 ms; 500 ms here -- no Tx-pacing layer
# underneath, matches every other endpoint SDK's default).
HEARTBEAT_PERIOD_S = 0.5
# Sender window cap: 16 in-flight samples (matches the 16-bit NACK bitmap).
SENDER_WINDOW = 16
# Receiver out-of-order buffer cap (DoS bound).
RECEIVER_BUFFER = 64
# Per-sample payload cap: the largest sample whose body (BaseObjectRequest +
# sample, Sec 7.7.8) still fits the 16-bit submessage_length.
MAX_PAYLOAD = 0xFFFF - BASE_SIZE

_HEADER_FMT = struct.Struct("<BBHBBH")  # session,stream,seq,submsg,flags,len
_HEARTBEAT_BODY_FMT = struct.Struct("<HHB")  # first,last,stream
_ACKNACK_BODY_FMT = struct.Struct("<HBBB")  # first_unacked,nack_lo,nack_hi,stream
_ACKNACK_DHEADER = struct.pack("<I", _ACKNACK_BODY_FMT.size)  # APPENDABLE: DHEADER = 5

# How the types of DDS-XRCE 1.0 Annex A without @extensibility are written
# (DDS-XTypes 1.3 section 7.3.1.2.1.8). This module writes one such type,
# ACKNACK_Payload; HEARTBEAT_Payload is FINAL in Annex A and does not change.
UNANNOTATED_APPENDABLE = "appendable"  # with a DHEADER (default)
UNANNOTATED_FINAL = "final"  # without a DHEADER (reference implementation)


def _check_unannotated_types(mode):
    if mode != UNANNOTATED_APPENDABLE and mode != UNANNOTATED_FINAL:
        raise ValueError(
            "unannotated_types must be 'appendable' or 'final', got %r" % (mode,))
    return mode


# --- RFC-1982 16-bit serial-number comparison ---------------------------


def seq_lt(a, b):
    """`a < b` in RFC-1982 order (half-window = 32768)."""
    diff = (b - a) & 0xFFFF
    return diff != 0 and diff < 0x8000


def seq_gt(a, b):
    return seq_lt(b, a)


# --- wire codec ----------------------------------------------------------


def reliable_write_frame(seq, object_id, sample):
    """Reliable WRITE_DATA frame; header seq carries the sample's seqnr.

    The submessage body is the BaseObjectRequest (Sec 7.7.8: request_id then
    object_id, each two octets most significant first) followed by the sample.
    `object_id` names the target DataWriter; the request_id is the sample's own
    sequence number, unique per stream as Sec 7.7.8 requires.

    The 16-bit submessage_length field cannot describe a body larger than
    0xFFFF; raise rather than truncate the length while appending the full
    payload (mirrors the C SDK's rejection).
    """
    sample = bytes(sample)
    body_len = BASE_SIZE + len(sample)
    if body_len > 0xFFFF:
        raise ValueError("sample exceeds 16-bit submessage_length")
    header = _HEADER_FMT.pack(
        XRCE_SESSION_NOKEY,
        RELIABLE_STREAM_ID,
        seq & 0xFFFF,
        _SM_WRITE_DATA,
        _WRITE_FLAGS,
        body_len & 0xFFFF,
    )
    base = struct.pack(">HH", seq & 0xFFFF, object_id & 0xFFFF)
    return header + base + sample


def reliable_unframe(frame):
    """`(seq, sample)` from a WRITE_DATA frame, or `None`. The sample starts
    after the BaseObjectRequest; a body too short to hold one is refused
    rather than handed back truncated."""
    if len(frame) < 8 + BASE_SIZE or frame[4] != _SM_WRITE_DATA:
        return None
    sm_len = struct.unpack_from("<H", frame, 6)[0]
    if sm_len < BASE_SIZE or 8 + sm_len > len(frame):
        return None
    seq = struct.unpack_from("<H", frame, 2)[0]
    return seq, bytes(frame[8 + BASE_SIZE:8 + sm_len])


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
                   session=XRCE_SESSION_NOKEY, stream_hdr=XRCE_STREAM_NONE,
                   unannotated_types=UNANNOTATED_APPENDABLE):
    """ACKNACK control frame. With `unannotated_types="appendable"` (default)
    17 octets, byte-identical to `golden_acknack_le.bin` for
    `(session=0x80, stream_hdr=0x00, msg_seq=1, first_unacked=1,
    nack_bitmap=0, stream_id=0x80)`; with `"final"` 13 octets (body length 5,
    no DHEADER)."""
    final = _check_unannotated_types(unannotated_types) == UNANNOTATED_FINAL
    header = _HEADER_FMT.pack(session, stream_hdr, msg_seq & 0xFFFF,
                               _SM_ACKNACK, _CTRL_FLAGS,
                               _ACKNACK_BODY_FMT.size if final else 9)
    body = (b"" if final else _ACKNACK_DHEADER) + _ACKNACK_BODY_FMT.pack(
        first_unacked & 0xFFFF,
        nack_bitmap & 0xFF,
        (nack_bitmap >> 8) & 0xFF,
        stream_id & 0xFF,
    )
    return header + body


def parse_acknack(frame):
    """`(first_unacked, nack_bitmap, stream)` from an ACKNACK frame, or `None`.
    Accepts body length 9 (DHEADER + 5 octets) and the FINAL form (5)."""
    if len(frame) < 13 or frame[4] != _SM_ACKNACK:
        return None
    off = 8
    if frame[6] | (frame[7] << 8) >= 9:
        if len(frame) < 17:
            return None
        off = 12  # skip the 4-octet DHEADER
    first_unacked, lo, hi, stream = _ACKNACK_BODY_FMT.unpack_from(frame, off)
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

    def __init__(self, unannotated_types=UNANNOTATED_APPENDABLE):
        """`unannotated_types`: `"appendable"` (default) or `"final"`, the
        form of the ACKNACK frames `acknack_frame()` writes."""
        self._unannotated_types = _check_unannotated_types(unannotated_types)
        self._expected = 0
        self._received = {}  # seq -> bytes

    @property
    def unannotated_types(self):
        return self._unannotated_types

    def acknack_frame(self, hint_last_seen=None, msg_seq=1,
                      session=XRCE_SESSION_NOKEY, stream_hdr=XRCE_STREAM_NONE):
        """The ACKNACK frame for `pending_acknack(hint_last_seen)`, in the
        form of this receiver's `unannotated_types`."""
        first, bitmap, stream_id = self.pending_acknack(hint_last_seen)
        return acknack_frame(msg_seq, first, bitmap, stream_id, session,
                             stream_hdr, self._unannotated_types)

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

    def __init__(self, sock, object_id, window=SENDER_WINDOW, maxsize=1024):
        self._sock = sock
        # ObjectId of the target DataWriter (Sec 7.7.8 BaseObjectRequest).
        self._object_id = object_id
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
                    self._safe_send(
                        reliable_write_frame(seq, self._object_id, item))

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
                            self._safe_send(
                                reliable_write_frame(seq, self._object_id, payload))

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
