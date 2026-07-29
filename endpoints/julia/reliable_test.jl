# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Unit tests for reliable.jl (mirror crates/xrce/src/reliable.rs) + byte-golden
# assertion against golden_heartbeat_le.bin / golden_acknack_le.bin.
# Run: julia reliable_test.jl [golden_dir]

include("reliable.jl")
using .Reliable

failed = Ref(0)
function check(cond::Bool, name::AbstractString)
    if cond
        println("ok   ", name)
    else
        println("FAIL ", name)
        failed[] += 1
    end
end

# --- sender ---

let
    s = Reliable.Sender()
    a = Reliable.submit!(s, UInt8[1, 2])
    b = Reliable.submit!(s, UInt8[3, 4])
    check(a[1] == Reliable.SR_OK && a[2] == UInt16(0), "submit_assigns_monotonic_seq_0")
    check(b[1] == Reliable.SR_OK && b[2] == UInt16(1), "submit_assigns_monotonic_seq_1")
    check(Reliable.in_flight_count(s) == 2, "submit_two_in_flight")
end

let
    s = Reliable.Sender()
    huge = zeros(UInt8, Reliable.MAX_PAYLOAD + 1)
    r = Reliable.submit!(s, huge)
    check(r[1] == Reliable.SR_TOO_LARGE, "submit_rejects_payload_too_large")
end

let
    s = Reliable.Sender()
    for _ in 1:Reliable.SENDER_WINDOW
        Reliable.submit!(s, UInt8[0])
    end
    r = Reliable.submit!(s, UInt8[0])
    check(r[1] == Reliable.SR_WINDOW_FULL, "submit_rejects_when_window_full")
end

let
    s = Reliable.Sender()
    Reliable.submit!(s, UInt8[1])
    check(Reliable.pending_heartbeat!(s, 0) !== nothing, "heartbeat_fires_first_time")
end

let
    s = Reliable.Sender()
    Reliable.submit!(s, UInt8[1])
    first = Reliable.pending_heartbeat!(s, 0)
    check(first !== nothing && first.first == UInt16(0) && first.last == UInt16(0) &&
          first.stream_id == Reliable.RELIABLE_STREAM_ID, "heartbeat_body_first_last_stream")
    check(Reliable.pending_heartbeat!(s, 100) === nothing, "heartbeat_silenced_before_period")
    check(Reliable.pending_heartbeat!(s, 600) !== nothing, "heartbeat_fires_after_period")
end

let
    s = Reliable.Sender()
    check(Reliable.pending_heartbeat!(s, 0) === nothing, "heartbeat_none_when_window_empty")
end

let
    s = Reliable.Sender()
    Reliable.submit!(s, UInt8[0xA0])  # seq 0
    Reliable.submit!(s, UInt8[0xA1])  # seq 1
    Reliable.submit!(s, UInt8[0xA2])  # seq 2
    # base=2, bitmap bit0 set → seq2 missing; 0+1 acked
    Reliable.recv_acknack!(s, UInt16(2), UInt8(0x01), UInt8(0x00))
    check(Reliable.in_flight_count(s) == 1 && Reliable.get_in_flight(s, UInt16(2)) !== nothing,
          "acknack_clears_acked_keeps_missing")
end

let
    s = Reliable.Sender()
    for _ in 1:5
        Reliable.submit!(s, UInt8[0])
    end
    Reliable.recv_acknack!(s, UInt16(5), UInt8(0), UInt8(0))
    check(Reliable.in_flight_count(s) == 0, "acknack_full_clear_when_no_bits_set")
end

# --- receiver ---

let
    r = Reliable.Receiver()
    Reliable.recv_data!(r, UInt16(0), UInt8[10])
    Reliable.recv_data!(r, UInt16(1), UInt8[11])
    d = Reliable.drain_in_order!(r)
    check(length(d) == 2 && d[1][1] == UInt16(0) && d[2][1] == UInt16(1) &&
          Reliable.expected(r) == UInt16(2), "recv_data_buffers_in_order")
end

let
    r = Reliable.Receiver()
    Reliable.recv_data!(r, UInt16(2), UInt8[22])
    Reliable.recv_data!(r, UInt16(0), UInt8[20])
    d1 = Reliable.drain_in_order!(r)
    check(length(d1) == 1 && d1[1][1] == UInt16(0), "recv_data_reorder_blocks_on_gap")
    Reliable.recv_data!(r, UInt16(1), UInt8[21])
    d2 = Reliable.drain_in_order!(r)
    check(length(d2) == 2 && d2[1][1] == UInt16(1) && d2[2][1] == UInt16(2),
          "recv_data_reorder_delivers")
end

let
    r = Reliable.Receiver()
    Reliable.recv_data!(r, UInt16(0), UInt8[1])
    Reliable.drain_in_order!(r)
    Reliable.recv_data!(r, UInt16(0), UInt8[99])  # duplicate < expected
    check(Reliable.out_of_order_count(r) == 0, "recv_data_drops_duplicates")
end

let
    r = Reliable.Receiver()
    for i in UInt16(1):UInt16(Reliable.RECEIVER_BUFFER)  # fill (expected=0, seqs 1..64)
        Reliable.recv_data!(r, i, UInt8[1])
    end
    ok = Reliable.recv_data!(r, UInt16(Reliable.RECEIVER_BUFFER + 1), UInt8[1])
    check(ok == false, "recv_data_rejects_when_buffer_full")
end

let
    r = Reliable.Receiver()
    Reliable.recv_data!(r, UInt16(1), UInt8[1])
    Reliable.recv_data!(r, UInt16(3), UInt8[3])
    a = Reliable.pending_acknack(r, UInt16(3))
    bitmap = UInt16(a.nack_lo) | (UInt16(a.nack_hi) << 8)
    check((bitmap & UInt16(0x01)) != 0 && (bitmap & UInt16(0x04)) != 0 &&
          (bitmap & UInt16(0x02)) == 0 && (bitmap & UInt16(0x08)) == 0,
          "pending_acknack_marks_missing_slots")
end

let
    s = Reliable.Sender()
    r = Reliable.Receiver()
    Reliable.submit!(s, UInt8[1, 2])
    Reliable.recv_data!(r, UInt16(0), UInt8[3])
    Reliable.reset!(s)
    Reliable.reset!(r)
    check(Reliable.in_flight_count(s) == 0 && Reliable.out_of_order_count(r) == 0 &&
          Reliable.expected(r) == UInt16(0), "reset_clears_state")
end

# --- end-to-end loss recovery (mirror reliable.rs) ---

let
    sender = Reliable.Sender()
    receiver = Reliable.Receiver()
    s0 = Reliable.submit!(sender, UInt8[10])[2]
    s1 = Reliable.submit!(sender, UInt8[11])[2]
    s2 = Reliable.submit!(sender, UInt8[12])[2]
    Reliable.recv_data!(receiver, s0, UInt8[10])
    Reliable.recv_data!(receiver, s2, UInt8[12])  # s1 lost
    d = Reliable.drain_in_order!(receiver)
    check(length(d) == 1 && d[1][2] == UInt8[10], "e2e_drain_blocks_on_lost_s1")
    ack = Reliable.pending_acknack(receiver, s2)
    Reliable.recv_acknack!(sender, ack.first_unacked, ack.nack_lo, ack.nack_hi)
    check(Reliable.get_in_flight(sender, s1) !== nothing, "e2e_s1_retransmittable")
    Reliable.recv_data!(receiver, s1, Reliable.get_in_flight(sender, s1))
    d2 = Reliable.drain_in_order!(receiver)
    check(length(d2) == 2 && d2[1][2] == UInt8[11] && d2[2][2] == UInt8[12],
          "e2e_delivers_all_after_retransmit")
end

# --- byte-golden ---

if length(ARGS) >= 1
    dir = ARGS[1]
    hb_path = joinpath(dir, "golden_heartbeat_le.bin")
    an_path = joinpath(dir, "golden_acknack_le.bin")
    if isfile(hb_path) && isfile(an_path)
        gold_hb = read(hb_path)
        gold_an = read(an_path)
        # golden = MessageHeader(session 0x80, stream NONE, msgSeq 1) + submessage
        hb = Reliable.heartbeat_frame(0x80, Reliable.STREAM_NONE, 1, UInt16(1), UInt16(3), 0x80)
        an = Reliable.acknack_frame(0x80, Reliable.STREAM_NONE, 1, UInt16(1), UInt8(0), UInt8(0), 0x80)
        check(hb == gold_hb, "byte_golden_heartbeat")
        check(an == gold_an, "byte_golden_acknack")
        # round-trip: parse the golden and re-encode
        phb = Reliable.parse_heartbeat(gold_hb)
        check(phb !== nothing && phb.first == UInt16(1) && phb.last == UInt16(3), "golden_heartbeat_parse")
        pan = Reliable.parse_acknack(gold_an)
        check(pan !== nothing && pan.first_unacked == UInt16(1), "golden_acknack_parse")
    else
        println("skip byte-golden: goldens not found in ", dir)
    end
else
    println("skip byte-golden: no golden dir arg")
end

if failed[] == 0
    println("ALL OK")
    exit(0)
else
    println(failed[], " FAILED")
    exit(1)
end
