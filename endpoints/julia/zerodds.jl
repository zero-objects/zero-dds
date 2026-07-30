# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Native pure-Julia endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
# byte-identical to the Rust core and the other SDKs. Sync (poll) and async
# (a Task filling a Channel — the idiomatic Julia concurrency model).

module ZeroDDS

@enum Endian LE BE

# --- Writer: a UInt8 buffer + endian; alignment relative to buffer start, cap 4 ---

mutable struct Writer
    buf::Vector{UInt8}
    endian::Endian
end
Writer(endian::Endian) = Writer(UInt8[], endian)

function align!(w::Writer, a::Int)
    cap = min(a, 4)
    pad = mod(cap - mod(length(w.buf), cap), cap)
    for _ in 1:pad
        push!(w.buf, 0x00)
    end
end

# le: bytes already encoded little-endian; reversed for BE.
function emit!(w::Writer, a::Int, le::Vector{UInt8})
    align!(w, a)
    append!(w.buf, w.endian == BE ? reverse(le) : le)
end

le_bytes(v::Unsigned, n::Int) = UInt8[UInt8((v >> (8 * (i - 1))) & 0xff) for i in 1:n]

put_u8!(w::Writer, v) = push!(w.buf, UInt8(v & 0xff))
put_u16!(w::Writer, v) = emit!(w, 2, le_bytes(UInt16(v), 2))
put_u32!(w::Writer, v) = emit!(w, 4, le_bytes(UInt32(v), 4))
put_u64!(w::Writer, v) = emit!(w, 4, le_bytes(UInt64(v), 8))
put_f32!(w::Writer, v) = emit!(w, 4, le_bytes(reinterpret(UInt32, Float32(v)), 4))
put_bytes!(w::Writer, b) = append!(w.buf, b)

function put_string!(w::Writer, s::AbstractString)
    put_u32!(w, sizeof(s) + 1)
    append!(w.buf, codeunits(s))
    put_u8!(w, 0)
end

function put_seq_u8!(w::Writer, b::Vector{UInt8})
    put_u32!(w, length(b))
    append!(w.buf, b)
end

bytes(w::Writer) = w.buf

# --- Reader: a byte cursor (pos is 0-based) ---

mutable struct Reader
    data::Vector{UInt8}
    pos::Int
    endian::Endian
end
Reader(data::Vector{UInt8}, endian::Endian) = Reader(data, 0, endian)

function take_bytes!(r::Reader, a::Int, n::Int)
    cap = min(a, 4)
    pad = mod(cap - mod(r.pos, cap), cap)
    r.pos += pad
    chunk = r.data[(r.pos + 1):(r.pos + n)]
    r.pos += n
    r.endian == BE ? reverse(chunk) : chunk
end

function get_u32(r::Reader)
    b = take_bytes!(r, 4, 4)
    UInt32(b[1]) | (UInt32(b[2]) << 8) | (UInt32(b[3]) << 16) | (UInt32(b[4]) << 24)
end

# --- primitives beyond get_u32 — the byte-exact inverse of the Writer ---

function get_u8(r::Reader)
    v = r.data[r.pos + 1]
    r.pos += 1
    v
end

function get_u16(r::Reader)
    b = take_bytes!(r, 2, 2)
    UInt16(b[1]) | (UInt16(b[2]) << 8)
end

function get_u64(r::Reader)
    b = take_bytes!(r, 4, 8)
    v = UInt64(0)
    for i in 1:8
        v |= UInt64(b[i]) << (8 * (i - 1))
    end
    v
end

get_f32(r::Reader) = reinterpret(Float32, get_u32(r))

function get_string(r::Reader)
    n = Int(get_u32(r))
    s = String(r.data[(r.pos + 1):(r.pos + n - 1)])
    r.pos += n
    s
end

function get_seq_u8(r::Reader)
    n = Int(get_u32(r))
    b = r.data[(r.pos + 1):(r.pos + n)]
    r.pos += n
    b
end

# --- XRCE WRITE_DATA framing ---

const SESSION_NOKEY = 0x80
const STREAM_BEST_EFFORT = 0x01

function write_frame(session, stream, seq, sample::Vector{UInt8})
    n = length(sample)
    # The 16-bit submessage_length field cannot describe a sample larger than
    # 0xFFFF; refuse (empty vector) rather than truncate the length while
    # appending the full payload (mirrors the C SDK's rejection).
    n > 0xFFFF && return UInt8[]
    hdr = UInt8[
        UInt8(session), UInt8(stream),
        UInt8(seq & 0xff), UInt8((seq >> 8) & 0xff),
        0x07, 0x03,
        UInt8(n & 0xff), UInt8((n >> 8) & 0xff),
    ]
    vcat(hdr, sample)
end

# Frame layout (1-based): [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
# len_hi] at indices 1..8, then the sample body from index 9. The 16-bit
# little-endian submessage_length (indices 7..8) bounds the body exactly:
# frame[9 : 8+smLen], never frame[9:end], so trailing padding or an appended
# submessage is not folded into the sample.
#
# Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
# hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
# Returns nothing for a short header, a wrong submessage id, or a declared
# length that runs past the datagram (truncation / wrong length).
function read_frame(frame::Vector{UInt8})
    (length(frame) >= 8 && (frame[5] == 0x07 || frame[5] == 0x09)) || return nothing
    sm_len = Int(frame[7]) | (Int(frame[8]) << 8)
    8 + sm_len > length(frame) && return nothing
    frame[9 : 8 + sm_len]
end

# --- transport ---

struct Transport
    deliver::Function
    receive::Function
end

# --- sync client ---

mutable struct Client
    transport::Transport
    session::UInt8
    stream::UInt8
    seq::Int
end
Client(t::Transport) = Client(t, SESSION_NOKEY, STREAM_BEST_EFFORT, 1)

function write!(c::Client, sample::Vector{UInt8})
    frame = write_frame(c.session, c.stream, c.seq, sample)
    c.transport.deliver(frame)
    c.seq = (c.seq + 1) & 0xffff
end

# One non-blocking receive: returns the sample body, or nothing.
function poll(c::Client)
    frame = c.transport.receive()
    frame === nothing ? nothing : read_frame(frame)
end

# --- async reader: a Task filling a Channel (idiomatic Julia) ---

mutable struct AsyncReader
    ch::Channel{Vector{UInt8}}
    running::Ref{Bool}
end

function start_reader(transport::Transport)
    ch = Channel{Vector{UInt8}}(32)
    running = Ref(true)
    @async begin
        while running[]
            frame = transport.receive()
            if frame === nothing
                sleep(0.001)
            else
                body = read_frame(frame)
                body !== nothing && put!(ch, body)
            end
        end
    end
    AsyncReader(ch, running)
end

recv(r::AsyncReader) = take!(r.ch)
stop!(r::AsyncReader) = (r.running[] = false)

# In-memory FIFO transport for tests + example.
function mem_transport()
    q = Vector{Vector{UInt8}}()
    Transport(
        frame -> (push!(q, copy(frame)); nothing),
        () -> isempty(q) ? nothing : popfirst!(q),
    )
end

end # module
