# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Reliable XRCE stream (spec §8.4.10/§8.4.11) for the Julia endpoint SDK.
# Self-contained: the sender/receiver state machines + the HEARTBEAT / ACKNACK
# frame codec, byte-identical to `crates/xrce` and the other endpoint SDKs.
# Importable beside `zerodds.jl`; the reliable WRITE_DATA frame is the same
# 8-byte header as `ZeroDDS.write_frame(0x80, 0x80, seq, sample)`.
#
# Frame layout (little-endian): [0]=session [1]=stream [2..4]=seq LE
#   [4]=submessage id [5]=flags [6..8]=body-len LE [8..]=body
#   WRITE_DATA id=0x07 flags=0x03 body=sample; header seq = RFC-1982 sample seq
#   HEARTBEAT  id=0x0B flags=0x01 body(5)= first i16 LE, last i16 LE, stream
#   ACKNACK    id=0x0A flags=0x01 body(9)= DHEADER u32 LE (=5), first_unacked i16 LE,
#              nack[0], nack[1], stream. ACKNACK_Payload has no @extensibility, so it is
#              APPENDABLE (DDS-XTypes 1.3 section 7.3.1.2.1.8) and carries a DHEADER in
#              XCDR2 (unannotated_types = UNANNOTATED_APPENDABLE, the default). With
#              UNANNOTATED_FINAL the body is the 5 octets of the FINAL form of the
#              reference implementation (eProsima), length field 5, 13-octet frame.
#              parse_acknack accepts both forms in either setting.
#
# Wrapped in `module Reliable` to avoid clashing with `ZeroDDS`'s own names
# (`write_frame`, `Client`, ...).

module Reliable

export UnannotatedTypes, UNANNOTATED_APPENDABLE, UNANNOTATED_FINAL,
       SESSION_NOKEY, STREAM_NONE, RELIABLE_STREAM_ID, HEARTBEAT_PERIOD_MS,
       SENDER_WINDOW, RECEIVER_BUFFER, MAX_PAYLOAD,
       SubmitStatus, SR_OK, SR_TOO_LARGE, SR_WINDOW_FULL,
       Sender, Receiver,
       seq_lt, seq_gt,
       reliable_write_frame, heartbeat_frame, acknack_frame,
       parse_heartbeat, parse_acknack,
       in_flight_count, submit!, get_in_flight, in_flight_pairs,
       pending_heartbeat!, recv_acknack!,
       expected, out_of_order_count, recv_data!, drain_in_order!, pending_acknack,
       reset!

const SESSION_NOKEY = 0x80
const STREAM_NONE = 0x00
const RELIABLE_STREAM_ID = 0x80
const HEARTBEAT_PERIOD_MS = 500
const SENDER_WINDOW = 16
const RECEIVER_BUFFER = 64
const MAX_PAYLOAD = 65535
const SM_WRITE_DATA = 0x07
const SM_ACKNACK = 0x0A
const SM_HEARTBEAT = 0x0B
# E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 --
# DDS-XRCE 1.0 8.3.5.8.
const FLAG_WRITE = 0x01
# Wire size of the BaseObjectRequest that precedes the sample (7.7.8).
const BASE_SIZE = 4
const FLAG_E_LE = 0x01

# --- RFC-1982 16-bit serial-number comparison ---

"`a < b` in RFC-1982 order (half-window = 32768)."
function seq_lt(a::UInt16, b::UInt16)
    d = (UInt32(b) - UInt32(a)) & 0xffff
    d != 0x00000000 && d < 0x00008000
end

seq_gt(a::UInt16, b::UInt16) = seq_lt(b, a)

# --- frame codec ---

"Reliable WRITE_DATA — identical bytes to `ZeroDDS.write_frame(0x80, 0x80, seq, sample)`."
function reliable_write_frame(seq::UInt16, object_id::Integer, sample::Vector{UInt8})
    n = BASE_SIZE + length(sample)
    hdr = UInt8[
        SESSION_NOKEY, RELIABLE_STREAM_ID,
        UInt8(seq & 0xff), UInt8((seq >> 8) & 0xff),
        SM_WRITE_DATA, FLAG_WRITE,
        UInt8(n & 0xff), UInt8((n >> 8) & 0xff),
        # BaseObjectRequest (7.7.8): request_id = the sample's own seq,
        # object_id = the target DataWriter.
        UInt8((seq >> 8) & 0xff), UInt8(seq & 0xff),
        UInt8((object_id >> 8) & 0xff), UInt8(object_id & 0xff),
    ]
    vcat(hdr, sample)
end

"byte-identical to `golden_heartbeat_le.bin` for (0x80, 0x00, 1, {1,3,0x80})."
function heartbeat_frame(session::UInt8, stream_hdr::UInt8, msg_seq::Integer,
                          first::UInt16, last::UInt16, stream_id::UInt8)
    UInt8[
        session, stream_hdr,
        UInt8(msg_seq & 0xff), UInt8((msg_seq >> 8) & 0xff),
        SM_HEARTBEAT, FLAG_E_LE, 0x05, 0x00,
        UInt8(first & 0xff), UInt8((first >> 8) & 0xff),
        UInt8(last & 0xff), UInt8((last >> 8) & 0xff),
        stream_id,
    ]
end

"""
How the types of DDS-XRCE 1.0 Annex A without `@extensibility` are written
(DDS-XTypes 1.3 section 7.3.1.2.1.8). This module writes one such type,
`ACKNACK_Payload`; `HEARTBEAT_Payload` is FINAL in Annex A and does not change.
`UNANNOTATED_APPENDABLE` (default) writes the DHEADER, `UNANNOTATED_FINAL` is the
form of the reference implementation (eProsima) without one.
"""
@enum UnannotatedTypes UNANNOTATED_APPENDABLE UNANNOTATED_FINAL

"""
ACKNACK frame. With `unannotated_types = UNANNOTATED_APPENDABLE` (default) 17 octets,
byte-identical to `golden_acknack_le.bin` for (0x80, 0x00, 1, {1,0,0,0x80}); with
`UNANNOTATED_FINAL` 13 octets (body length 5, no DHEADER).
"""
function acknack_frame(session::UInt8, stream_hdr::UInt8, msg_seq::Integer,
                        first_unacked::UInt16, nack_lo::UInt8, nack_hi::UInt8, stream_id::UInt8;
                        unannotated_types::UnannotatedTypes = UNANNOTATED_APPENDABLE)
    final = unannotated_types == UNANNOTATED_FINAL
    out = UInt8[
        session, stream_hdr,
        UInt8(msg_seq & 0xff), UInt8((msg_seq >> 8) & 0xff),
        SM_ACKNACK, FLAG_E_LE, final ? 0x05 : 0x09, 0x00,
    ]
    final || append!(out, UInt8[0x05, 0x00, 0x00, 0x00])  # DHEADER: body size 5
    append!(out, UInt8[
        UInt8(first_unacked & 0xff), UInt8((first_unacked >> 8) & 0xff),
        nack_lo, nack_hi, stream_id,
    ])
    out
end

"Returns `(first, last, stream_id)` or `nothing`."
function parse_heartbeat(frame::Vector{UInt8})
    (length(frame) < 13 || frame[5] != SM_HEARTBEAT) && return nothing
    first = UInt16(frame[9]) | (UInt16(frame[10]) << 8)
    last = UInt16(frame[11]) | (UInt16(frame[12]) << 8)
    (first = first, last = last, stream_id = frame[13])
end

"Returns `(first_unacked, nack_lo, nack_hi, stream_id)` or `nothing`. Accepts body
length 9 (DHEADER + 5 octets) and the legacy FINAL form with body length 5."
function parse_acknack(frame::Vector{UInt8})
    (length(frame) < 13 || frame[5] != SM_ACKNACK) && return nothing
    o = 8  # 0-based body offset
    if (UInt16(frame[7]) | (UInt16(frame[8]) << 8)) >= 9
        length(frame) < 17 && return nothing
        o = 12  # skip the 4-octet DHEADER
    end
    first_unacked = UInt16(frame[o + 1]) | (UInt16(frame[o + 2]) << 8)
    (first_unacked = first_unacked, nack_lo = frame[o + 3], nack_hi = frame[o + 4], stream_id = frame[o + 5])
end

# --- sender ---

@enum SubmitStatus SR_OK SR_TOO_LARGE SR_WINDOW_FULL

mutable struct Sender
    next_seq::UInt16
    in_flight::Dict{UInt16,Vector{UInt8}}
    last_heartbeat_ms::Int  # -1 = never sent
end
Sender() = Sender(UInt16(0), Dict{UInt16,Vector{UInt8}}(), -1)

in_flight_count(s::Sender) = length(s.in_flight)

"Buffers a new sample. Errors when the payload is too large or the window
(16 in-flight) is full — the caller must process ACKNACKs first."
function submit!(s::Sender, payload::Vector{UInt8})
    length(payload) > MAX_PAYLOAD && return (SR_TOO_LARGE, UInt16(0))
    length(s.in_flight) >= SENDER_WINDOW && return (SR_WINDOW_FULL, UInt16(0))
    sq = s.next_seq
    s.in_flight[sq] = payload
    s.next_seq += UInt16(1)
    (SR_OK, sq)
end

get_in_flight(s::Sender, sq::UInt16) = get(s.in_flight, sq, nothing)

"Snapshot of in-flight `(seq, payload)` pairs (safe to iterate while sending)."
in_flight_pairs(s::Sender) = collect(s.in_flight)

"`some(HEARTBEAT)` when in-flight samples exist and the period elapsed
(fires on the first call, then every `HEARTBEAT_PERIOD_MS`)."
function pending_heartbeat!(s::Sender, now_ms::Integer)
    isempty(s.in_flight) && return nothing
    due = s.last_heartbeat_ms < 0 || (now_ms - s.last_heartbeat_ms) >= HEARTBEAT_PERIOD_MS
    due || return nothing
    s.last_heartbeat_ms = now_ms
    # RFC-1982 window base (oldest unacked) + end (newest unacked); NOT numeric
    # minimum/maximum, which is wrong across a 16-bit wrap: window
    # 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end 0x0001, not 0x0000 /
    # 0xFFFF. Mirrors window_base / serial_max_in_flight in crates/xrce.
    mn = mx = nothing
    for k in keys(s.in_flight)
        if mn === nothing
            mn = k; mx = k
        else
            seq_lt(k, mn) && (mn = k)
            seq_gt(k, mx) && (mx = k)
        end
    end
    (first = mn, last = mx, stream_id = RELIABLE_STREAM_ID)
end

"`base = first_unacked`; everything `< base` (RFC-1982) is acknowledged and
dropped; in `[base, base+16)` a set bit means missing (keep), a clear bit
means acked (drop)."
function recv_acknack!(s::Sender, first_unacked::UInt16, nack_lo::UInt8, nack_hi::UInt8)
    base = first_unacked
    bitmap = UInt16(nack_lo) | (UInt16(nack_hi) << 8)
    to_remove = UInt16[]
    for k in keys(s.in_flight)
        diff = (UInt32(base) - UInt32(k)) & 0xffff
        if diff != 0x00000000 && diff < 0x00008000
            push!(to_remove, k)
        end
    end
    for k in to_remove
        delete!(s.in_flight, k)
    end
    for i in UInt16(0):UInt16(15)
        sq = base + i
        bit = (bitmap >> i) & UInt16(1)
        if bit == 0 && haskey(s.in_flight, sq)
            delete!(s.in_flight, sq)
        end
    end
end

# --- receiver ---

mutable struct Receiver
    expected::UInt16
    received::Dict{UInt16,Vector{UInt8}}
    unannotated_types::UnannotatedTypes
end
Receiver(expected::UInt16, received::Dict{UInt16,Vector{UInt8}}) =
    Receiver(expected, received, UNANNOTATED_APPENDABLE)
"`Receiver(; unannotated_types = UNANNOTATED_APPENDABLE)`: the form of the ACKNACK
frames `acknack_frame(r, hint)` writes."
Receiver(; unannotated_types::UnannotatedTypes = UNANNOTATED_APPENDABLE) =
    Receiver(UInt16(0), Dict{UInt16,Vector{UInt8}}(), unannotated_types)

expected(r::Receiver) = r.expected
out_of_order_count(r::Receiver) = length(r.received)

"Buffers an incoming sample. Returns `false` only when the receiver buffer
is full (DoS bound); duplicates are silently accepted as a no-op."
function recv_data!(r::Receiver, sq::UInt16, payload::Vector{UInt8})
    seq_lt(sq, r.expected) && return true  # already delivered → drop
    haskey(r.received, sq) && return true  # already in the buffer
    length(r.received) >= RECEIVER_BUFFER && return false
    r.received[sq] = payload
    true
end

"Delivers contiguously from `expected`, advancing `expected`."
function drain_in_order!(r::Receiver)
    out = Tuple{UInt16,Vector{UInt8}}[]
    while haskey(r.received, r.expected)
        push!(out, (r.expected, r.received[r.expected]))
        delete!(r.received, r.expected)
        r.expected += UInt16(1)
    end
    out
end

"Bitmap of the missing slots in `[expected, expected+16)`; slots beyond
`hint_last_seen` are not marked missing."
function pending_acknack(r::Receiver, hint_last_seen::Union{UInt16,Nothing})
    base = r.expected
    bitmap = UInt16(0)
    for i in UInt16(0):UInt16(15)
        sq = base + i
        if hint_last_seen !== nothing && seq_gt(sq, hint_last_seen)
            continue
        end
        if !haskey(r.received, sq)
            bitmap |= (UInt16(1) << i)
        end
    end
    (first_unacked = base, nack_lo = UInt8(bitmap & 0xff), nack_hi = UInt8((bitmap >> 8) & 0xff),
     stream_id = RELIABLE_STREAM_ID)
end

"The ACKNACK frame for `pending_acknack(r, hint_last_seen)`, in the form of
`r.unannotated_types`."
function acknack_frame(r::Receiver, hint_last_seen::Union{UInt16,Nothing} = nothing;
                        session::UInt8 = SESSION_NOKEY, stream_hdr::UInt8 = STREAM_NONE,
                        msg_seq::Integer = 1)
    a = pending_acknack(r, hint_last_seen)
    acknack_frame(session, stream_hdr, msg_seq, a.first_unacked, a.nack_lo, a.nack_hi, a.stream_id;
                  unannotated_types = r.unannotated_types)
end

function reset!(s::Sender)
    s.next_seq = UInt16(0)
    empty!(s.in_flight)
    s.last_heartbeat_ms = -1
end

function reset!(r::Receiver)
    r.expected = UInt16(0)
    empty!(r.received)
end

end # module
