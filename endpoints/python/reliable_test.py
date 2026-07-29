#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Unit tests for zerodds_reliable.py (mirror crates/xrce/src/reliable.rs) +
# byte-golden assertion against golden_heartbeat_le.bin / golden_acknack_le.bin.
# usage: python3 reliable_test.py [golden_dir]

import os
import sys

from zerodds_reliable import (
    MAX_PAYLOAD,
    RECEIVER_BUFFER,
    RELIABLE_STREAM_ID,
    SENDER_WINDOW,
    ReliableReceiver,
    ReliableSender,
    acknack_frame,
    heartbeat_frame,
    parse_acknack,
    parse_heartbeat,
)

_failed = [0]


def check(cond, name):
    if cond:
        print("ok   " + name)
    else:
        print("FAIL " + name)
        _failed[0] += 1


# --- sender ------------------------------------------------------------


def test_submit_assigns_monotonic_seqnrs():
    s = ReliableSender()
    st0, seq0 = s.submit(b"\x01\x02")
    st1, seq1 = s.submit(b"\x03\x04")
    check(st0 == "ok" and seq0 == 0, "submit_assigns_monotonic_seq_0")
    check(st1 == "ok" and seq1 == 1, "submit_assigns_monotonic_seq_1")
    check(s.in_flight_count() == 2, "submit_two_in_flight")


def test_submit_rejects_payload_too_large():
    s = ReliableSender()
    status, _ = s.submit(bytes(MAX_PAYLOAD + 1))
    check(status == "too_large", "submit_rejects_payload_too_large")


def test_submit_rejects_when_window_full():
    s = ReliableSender()
    for _ in range(SENDER_WINDOW):
        s.submit(b"\x00")
    status, _ = s.submit(b"\x00")
    check(status == "window_full", "submit_rejects_when_window_full")


def test_pending_heartbeat():
    s = ReliableSender()
    s.submit(b"\x01")
    hb = s.pending_heartbeat(0.0)
    check(
        hb is not None and hb == (0, 0, RELIABLE_STREAM_ID),
        "heartbeat_body_first_last_stream",
    )
    check(s.pending_heartbeat(0.1) is None, "heartbeat_silenced_before_period")
    check(s.pending_heartbeat(0.6) is not None, "heartbeat_fires_after_period")

    s2 = ReliableSender()
    check(s2.pending_heartbeat(0.0) is None, "heartbeat_none_when_window_empty")


def test_recv_acknack_clears_acked_keeps_missing():
    s = ReliableSender()
    s.submit(b"\xa0")  # seq 0
    s.submit(b"\xa1")  # seq 1
    s.submit(b"\xa2")  # seq 2
    # base=2, bit0 set -> seq2 missing; 0+1 acked.
    s.recv_acknack(2, 0x0001)
    check(
        s.in_flight_count() == 1 and s.get_in_flight(2) is not None,
        "acknack_clears_acked_keeps_missing",
    )


def test_recv_acknack_full_clear_when_no_bits_set():
    s = ReliableSender()
    for _ in range(5):
        s.submit(b"\x00")
    s.recv_acknack(5, 0)
    check(s.in_flight_count() == 0, "acknack_full_clear_when_no_bits_set")


# --- receiver ------------------------------------------------------------


def test_recv_data_buffers_in_order():
    r = ReliableReceiver()
    r.recv_data(0, b"\x0a")
    r.recv_data(1, b"\x0b")
    d = r.drain_in_order()
    check(
        len(d) == 2 and d[0][0] == 0 and d[1][0] == 1 and r.expected() == 2,
        "recv_data_buffers_in_order",
    )


def test_recv_data_reorders_out_of_order():
    r = ReliableReceiver()
    r.recv_data(2, b"\x16")
    r.recv_data(0, b"\x14")
    d1 = r.drain_in_order()
    check(len(d1) == 1 and d1[0][0] == 0, "recv_data_reorder_blocks_on_gap")
    r.recv_data(1, b"\x15")
    d2 = r.drain_in_order()
    check(
        len(d2) == 2 and d2[0][0] == 1 and d2[1][0] == 2,
        "recv_data_reorder_delivers",
    )


def test_recv_data_drops_duplicates():
    r = ReliableReceiver()
    r.recv_data(0, b"\x01")
    r.drain_in_order()
    r.recv_data(0, b"\x63")  # duplicate < expected
    check(r.out_of_order_count() == 0, "recv_data_drops_duplicates")


def test_recv_data_rejects_when_buffer_full():
    r = ReliableReceiver()
    for i in range(1, RECEIVER_BUFFER + 1):  # expected=0, seqs 1..64
        r.recv_data(i, b"\x01")
    ok = r.recv_data(RECEIVER_BUFFER + 1, b"\x01")
    check(ok is False, "recv_data_rejects_when_buffer_full")


def test_pending_acknack_marks_missing_slots():
    r = ReliableReceiver()
    r.recv_data(1, b"\x01")
    r.recv_data(3, b"\x03")
    _base, bitmap, _sid = r.pending_acknack(3)
    check(
        (bitmap & 0x01) != 0
        and (bitmap & 0x04) != 0
        and (bitmap & 0x02) == 0
        and (bitmap & 0x08) == 0,
        "pending_acknack_marks_missing_slots",
    )


def test_reset_clears_state():
    s = ReliableSender()
    r = ReliableReceiver()
    s.submit(b"\x01\x02")
    r.recv_data(0, b"\x03")
    s.reset()
    r.reset()
    check(
        s.in_flight_count() == 0 and r.out_of_order_count() == 0 and r.expected() == 0,
        "reset_clears_state",
    )


# --- end-to-end loss recovery (mirror reliable.rs) -----------------------


def test_end_to_end_sender_receiver_with_loss_recovery():
    sender = ReliableSender()
    receiver = ReliableReceiver()
    _st0, s0 = sender.submit(b"\x0a")
    _st1, s1 = sender.submit(b"\x0b")
    _st2, s2 = sender.submit(b"\x0c")

    receiver.recv_data(s0, b"\x0a")
    receiver.recv_data(s2, b"\x0c")  # s1 lost

    drained = receiver.drain_in_order()
    check(
        len(drained) == 1 and drained[0][1] == b"\x0a",
        "e2e_drain_blocks_on_lost_s1",
    )

    base, bitmap, _sid = receiver.pending_acknack(s2)
    sender.recv_acknack(base, bitmap)
    check(sender.get_in_flight(s1) is not None, "e2e_s1_retransmittable")

    s1_payload = sender.get_in_flight(s1)
    receiver.recv_data(s1, s1_payload)

    drained2 = receiver.drain_in_order()
    check(
        len(drained2) == 2 and drained2[0][1] == b"\x0b" and drained2[1][1] == b"\x0c",
        "e2e_delivers_all_after_retransmit",
    )


# --- byte-golden ----------------------------------------------------------


def test_byte_golden(gold_dir):
    hb_path = os.path.join(gold_dir, "golden_heartbeat_le.bin")
    an_path = os.path.join(gold_dir, "golden_acknack_le.bin")
    if not (os.path.isfile(hb_path) and os.path.isfile(an_path)):
        print("skip byte-golden: goldens not found in " + gold_dir)
        return

    with open(hb_path, "rb") as f:
        gold_hb = f.read()
    with open(an_path, "rb") as f:
        gold_an = f.read()

    # golden = MessageHeader(session 0x80, stream NONE, msg_seq 1) + submessage.
    hb = heartbeat_frame(1, 1, 3, 0x80)
    an = acknack_frame(1, 1, 0, 0x80)
    check(hb == gold_hb, "byte_golden_heartbeat")
    check(an == gold_an, "byte_golden_acknack")

    # Round-trip: parse the golden and check the fields.
    phb = parse_heartbeat(gold_hb)
    check(phb is not None and phb[0] == 1 and phb[1] == 3, "golden_heartbeat_parse")
    pan = parse_acknack(gold_an)
    check(pan is not None and pan[0] == 1, "golden_acknack_parse")


def main():
    test_submit_assigns_monotonic_seqnrs()
    test_submit_rejects_payload_too_large()
    test_submit_rejects_when_window_full()
    test_pending_heartbeat()
    test_recv_acknack_clears_acked_keeps_missing()
    test_recv_acknack_full_clear_when_no_bits_set()
    test_recv_data_buffers_in_order()
    test_recv_data_reorders_out_of_order()
    test_recv_data_drops_duplicates()
    test_recv_data_rejects_when_buffer_full()
    test_pending_acknack_marks_missing_slots()
    test_reset_clears_state()
    test_end_to_end_sender_receiver_with_loss_recovery()

    if len(sys.argv) > 1:
        test_byte_golden(sys.argv[1])
    else:
        print("skip byte-golden: no golden dir arg")

    if _failed[0] == 0:
        print("ALL OK")
        return 0
    print("%d FAILED" % _failed[0])
    return 1


if __name__ == "__main__":
    sys.exit(main())
