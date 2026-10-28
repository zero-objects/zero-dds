# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# DDS-XRCE 1.0 client, ported from `crates/xrce-client`. Two layers:
#
#   1. Pure wire encoders (no state, no I/O) for MessageHeader, Submessage-
#      Header, ObjectId, BaseObjectRequest, ObjectVariant, and the five
#      submessage bodies this client speaks (CREATE_CLIENT/CREATE/DELETE/
#      WRITE_DATA/READ_DATA).
#   2. `Client`, a synchronous state machine mirroring
#      `crates/xrce-client::XrceClient` 1:1 in its lifecycle rules
#      (:disconnected -> :connecting -> :connected, monotonic request ids,
#      `disconnect!` resetting only `state`).
#
# This reuses the SDK's existing `(deliver, receive)` transport shape from
# `zerodds.jl`'s `ZeroDDS.Transport` — duck-typed here rather than formally
# imported, so this file has no load-order dependency on `zerodds.jl`; any
# object with callable `.deliver`/`.receive` fields works, `ZeroDDS.Transport`
# included. It is additive: `ZeroDDS.Client`/`ZeroDDS.AsyncReader` speak an
# older, in-house, non-conformant frame on purpose; this module is the
# conformant DDS-XRCE 1.0 sibling that can talk to a real agent.
#
# One difference from the Rust reference is worth flagging up front: in
# `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
# `request_read` take unused parameters and never touch the wire — they are
# pure request-id bookkeeping, with encoding left to a caller that does not
# exist yet. This port closes that gap: every op here actually encodes its
# submessage with the Layer 1 encoders below and calls `transport.deliver`,
# because "reach a real XRCE agent over the network" is the entire point of
# the port. The state-machine *rules* (refusals, transitions, id allocation,
# what `disconnect!` resets) are unchanged.
#
# Idiom: multiple dispatch stands in for Rust's `ObjectVariant` enum match.
# `ObjectVariant` is an abstract type with one concrete subtype implemented
# here (`ByReference`, the only representation `crates/xrce-client` itself
# encodes) and `encode_variant` is a *generic* function with one method per
# subtype. `encode_create_message` calls `encode_variant(variant)` without
# knowing which concrete type it holds; Julia picks the method from the
# argument's runtime type, the same open-ended-set-of-cases role Rust's
# `match` on an enum plays. A future `ByXmlString`/`InBinary` (out of this
# port's scope, same as the Rust reference) would add one `encode_variant`
# method and touch nothing else — no case list to extend.

module XrceClient

# --- ObjectId (spec Sec 7.2.1) ------------------------------------------------

"""4-bit OBJK_* codes (spec Sec 7.2 Table 4). Values are the wire codes
themselves, not sequential — 0x07..0x09 have no spec meaning and are
deliberately absent so no value can round-trip an invalid kind."""
@enum ObjectKind::UInt8 begin
    PARTICIPANT = 1
    TOPIC = 2
    PUBLISHER = 3
    SUBSCRIBER = 4
    DATAWRITER = 5
    DATAREADER = 6
    TYPE = 10
    QOSPROFILE = 11
    APPLICATION = 12
    AGENT = 13
    CLIENT = 14
    DOMAIN = 15
end

"""The packed 16-bit word: `raw_id` (12 bits) in the high bits, `kind` (4
bits) in the low bits. Spec Sec 7.2.1."""
struct ObjectId
    word::UInt16
end

"""`word = (raw_id << 4) | kind`."""
pack_object_id(raw_id::UInt16, kind::ObjectKind) = ObjectId(UInt16((raw_id << 4) | UInt16(kind)))

"""Constructs directly from an already-packed 16-bit word (e.g. a value read
off the wire, or a well-known singleton like OBJECTID_AGENT)."""
object_id_from_raw(word::UInt16) = ObjectId(word)

"""Wire encoding: big-endian — the one field in this spec that is *not*
little-endian (everything else in a DDS-XRCE message is LE)."""
to_bytes(id::ObjectId) = UInt8[(id.word >> 8) & 0xFF, id.word & 0xFF]

# --- ObjectVariant (spec Sec 7.7.2), via multiple dispatch -------------------

abstract type ObjectVariant end

"""Reference-by-name string — the only representation this port encodes
(ByXmlString/InBinary are out of scope, same as `crates/xrce-client`)."""
struct ByReference <: ObjectVariant
    name::String
end

"""`discriminator(1)=0x01 ++ 3 padding bytes ++ length(4, LE)=strlen+1 ++ name
bytes ++ NUL`. The length includes the NUL terminator XCDR2 strings carry.
This is the *inner* payload only, correct only when it starts 4-aligned —
not the wire form a real agent accepts on its own; see
`encode_variant_for_kind`, whose 1-byte outer ObjectKind shifts this
padding to two bytes."""
function encode_variant(v::ByReference)
    name_bytes = codeunits(v.name)
    n = UInt32(length(name_bytes) + 1)
    result = UInt8[0x01, 0x00, 0x00, 0x00, n & 0xFF, (n >> 8) & 0xFF, (n >> 16) & 0xFF, (n >> 24) & 0xFF]
    append!(result, name_bytes)
    push!(result, 0x00)
    result
end

"""The real wire ObjectVariant (spec Sec 7.7.3): a **two-level**
discriminated union — the outer ObjectKind byte (`object_id`'s own low 4
bits — the outer discriminator and the object being created always agree,
by construction), then the inner REPRESENTATION_BY_REFERENCE payload
(disc 0x01, two padding bytes — the outer kind byte already took the
first of the four — then the same XCDR2 string as `encode_variant`), then
that kind's own trailing field: `parent_object_id` (an ObjectId,
unaligned) for OBJK_TOPIC/OBJK_PUBLISHER/OBJK_SUBSCRIBER/OBJK_DATAWRITER/
OBJK_DATAREADER, `nothing` for every other kind — byte-oracled in
`crates/xrce/src/object_repr.rs` against real eProsima Micro-XRCE-DDS
Client 3.0.1 CREATE traffic (OBJK_TOPIC's own participant_id: commit
31c62df2a63). This — not `encode_variant` alone — is the only form a
real agent accepts."""
function encode_variant_for_kind(object_id::ObjectId, v::ByReference;
                                  parent_object_id::Union{ObjectId, Nothing} = nothing)
    name_bytes = codeunits(v.name)
    n = UInt32(length(name_bytes) + 1)
    result = UInt8[object_id.word & 0x0F, 0x01, 0x00, 0x00,
                   n & 0xFF, (n >> 8) & 0xFF, (n >> 16) & 0xFF, (n >> 24) & 0xFF]
    append!(result, name_bytes)
    push!(result, 0x00)
    if parent_object_id !== nothing
        append!(result, to_bytes(parent_object_id))
    end
    result
end

# --- MessageHeader (spec Sec 8.3.2) ------------------------------------------

"""Length of the ClientKey field, spec Sec 7.7."""
const CLIENT_KEY_LEN = 4

"""`session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80` does
not (4-byte header). Reserved: SESSION_NONE_WITH_KEY = 0x00,
SESSION_NONE_WITHOUT_KEY = 0x80."""
const SESSION_CARRIES_KEY_MAX = 0x7F

"""`session_id(1) stream_id(1) sequence_nr(2, LE) [client_key(4)]`. Every
operation in this port uses a session_id <= 0x7F, so the key is always
present — callers never construct a keyless header here."""
function encode_message_header(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8})
    result = UInt8[session_id, stream_id, sequence_nr & 0xFF, (sequence_nr >> 8) & 0xFF]
    if session_id <= SESSION_CARRIES_KEY_MAX
        append!(result, client_key)
    end
    result
end

# --- SubmessageHeader (spec Sec 8.3.4) ---------------------------------------

"""FLAG_E_LITTLE_ENDIAN, bit 0. Every submessage this client sends sets it —
the whole port only ever writes little-endian bodies."""
const FLAG_E_LITTLE_ENDIAN = 0x01

"""`submessage_id(1) flags(1) submessage_length(2, always LE)`. The length
field is always little-endian regardless of the body's own endianness flag."""
function encode_submessage_header(id::UInt8, flags::UInt8, body_len::UInt16)
    UInt8[id, flags, body_len & 0xFF, (body_len >> 8) & 0xFF]
end

# Submessage ids used by this client (spec Sec 8.3.4 Table 8).
const SM_CREATE_CLIENT = 0x00
const SM_CREATE = 0x01
const SM_DELETE = 0x03
const SM_WRITE_DATA = 0x07
const SM_READ_DATA = 0x08

# --- BaseObjectRequest (spec Sec 7.7.8) --------------------------------------

"""`request_id(2 raw bytes) ++ object_id.to_bytes()(2, big-endian)`. The wire
RequestId is two raw, caller-supplied octets — not a little-endian encoding
of an integer. `crates/xrce`'s own `BaseObjectRequest::encode` copies them
through unchanged."""
function encode_base_object_request(request_id::Tuple{UInt8, UInt8}, object_id::ObjectId)
    result = UInt8[request_id[1], request_id[2]]
    append!(result, to_bytes(object_id))
    result
end

# --- Full message encoders ----------------------------------------------------
#
# A message is `header ++ submessage_header ++ body`, concatenated directly:
# this client only ever sends one submessage per message, so the
# between-submessages alignment padding rule never applies here.

function encode_message(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8},
                         submessage_id::UInt8, flags::UInt8, body::Vector{UInt8})
    result = encode_message_header(session_id, stream_id, sequence_nr, client_key)
    append!(result, encode_submessage_header(submessage_id, flags, UInt16(length(body))))
    append!(result, body)
    result
end

"""eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
module doc has the byte-oracle measurement backing it), used as this
port's own CREATE_CLIENT requested_mtu."""
const DEFAULT_MTU = UInt16(508)

"""`CLIENT_Representation` (Annex A, spec Sec 7.8.2.1): xrce_cookie(4) ++
xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1) ++ a
0 properties-absent octet ++ requested_mtu(2, LE) — eProsima's own
non-normative MTU extension, always sent by this client. No PropertySeq
support here — this client sends none.

Keyword `representation_timestamps`: `:absent` (default, Annex A / B.1 form)
or `:present` (the prose form of Sec 7.8.2.1): after xrce_vendor_id the
`Time_t client_timestamp`, `seconds` (Int32) and `nanoseconds` (UInt32),
little endian. Time_t is APPENDABLE (DDS-XTypes 1.3 Sec 7.3.1.2.1.8) with
`unannotated_types = :appendable` (default): DHEADER = 8 in front, 12
octets; `:final`: no DHEADER, 8 octets. The agent has to be set the same
way (`representation_timestamps:` in its configuration)."""
function encode_client_representation(version::Vector{UInt8}, vendor_id::Vector{UInt8},
                                       client_key::Vector{UInt8}, session_id::UInt8;
                                       requested_mtu::UInt16 = DEFAULT_MTU,
                                       representation_timestamps::Symbol = :absent,
                                       seconds::Integer = 0, nanoseconds::Integer = 0,
                                       unannotated_types::Symbol = :appendable)
    representation_timestamps in (:absent, :present) ||
        throw(ArgumentError("representation_timestamps must be :absent or :present, got $(repr(representation_timestamps))"))
    unannotated_types in (:appendable, :final) ||
        throw(ArgumentError("unannotated_types must be :appendable or :final, got $(repr(unannotated_types))"))
    result = UInt8[0x58, 0x52, 0x43, 0x45] # "XRCE"
    append!(result, version)
    append!(result, vendor_id)
    if representation_timestamps === :present
        if unannotated_types === :appendable
            append!(result, UInt8[0x08, 0x00, 0x00, 0x00]) # DHEADER of Time_t
        end
        sec = reinterpret(UInt32, Int32(seconds))
        append!(result, UInt8[(sec >> (8 * i)) & 0xFF for i in 0:3])
        nsec = UInt32(nanoseconds)
        append!(result, UInt8[(nsec >> (8 * i)) & 0xFF for i in 0:3])
    end
    append!(result, client_key)
    push!(result, session_id)
    push!(result, 0x00) # properties absent
    push!(result, requested_mtu & 0xFF)
    push!(result, (requested_mtu >> 8) & 0xFF)
    result
end

"""Spec Sec 7.8.2. `session_id` is the session id the client asks for (it is
also in `representation`). No session exists yet, so the header carries
SESSION_NONE_WITH_KEY (0x00, with the ClientKey) for a requested `session_id`
of 0..127 and SESSION_NONE_WITHOUT_KEY (0x80, no ClientKey) for 128..255
(DDS-XRCE 1.0 Sec 8.3.2.1). stream=NONE(0x00), sequence_nr=0. `representation`
is the XCDR2-encoded `CLIENT_Representation` — build it with
`encode_client_representation` — passed through byte for byte here."""
function encode_create_client_message(session_id::UInt8, client_key::Vector{UInt8}, representation::Vector{UInt8})
    header_session = session_id <= SESSION_CARRIES_KEY_MAX ? 0x00 : 0x80
    encode_message(header_session, 0x00, UInt16(0), client_key, SM_CREATE_CLIENT, FLAG_E_LITTLE_ENDIAN, representation)
end

"""Spec Sec 7.8.3. Flags = E | (reuse ? 0x02 : 0) | (replace ? 0x04 : 0). Body
= BaseObjectRequest ++ the real, two-level ObjectVariant
(`encode_variant_for_kind`)."""
function encode_create_message(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8},
                                request_id::Tuple{UInt8, UInt8}, object_id::ObjectId, variant::ObjectVariant;
                                parent_object_id::Union{ObjectId, Nothing} = nothing,
                                reuse::Bool = false, replace::Bool = false)
    flags = FLAG_E_LITTLE_ENDIAN
    reuse && (flags |= 0x02)
    replace && (flags |= 0x04)
    body = encode_base_object_request(request_id, object_id)
    append!(body, encode_variant_for_kind(object_id, variant; parent_object_id = parent_object_id))
    encode_message(session_id, stream_id, sequence_nr, client_key, SM_CREATE, flags, body)
end

"""Spec Sec 7.8.3. Body = BaseObjectRequest only — no ObjectVariant."""
function encode_delete_message(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8},
                                request_id::Tuple{UInt8, UInt8}, object_id::ObjectId)
    body = encode_base_object_request(request_id, object_id)
    encode_message(session_id, stream_id, sequence_nr, client_key, SM_DELETE, FLAG_E_LITTLE_ENDIAN, body)
end

"""Spec Sec 7.8.4. Body = BaseObjectRequest ++ serialized_data, verbatim and
with no length prefix of its own — `submessage_length` is what bounds the
sample."""
function encode_write_data_message(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8},
                                    request_id::Tuple{UInt8, UInt8}, object_id::ObjectId, serialized_data::Vector{UInt8})
    body = encode_base_object_request(request_id, object_id)
    append!(body, serialized_data)
    encode_message(session_id, stream_id, sequence_nr, client_key, SM_WRITE_DATA, FLAG_E_LITTLE_ENDIAN, body)
end

"""Spec Sec 7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This port
never sets a content filter or delivery control, so ReadSpecification is
always `preferred_stream_id, data_format=0x00, content_filter_present=0x00,
delivery_control_present=0x00`."""
function encode_read_data_message(session_id::UInt8, stream_id::UInt8, sequence_nr::UInt16, client_key::Vector{UInt8},
                                   request_id::Tuple{UInt8, UInt8}, object_id::ObjectId, preferred_stream_id::UInt8)
    body = encode_base_object_request(request_id, object_id)
    append!(body, UInt8[preferred_stream_id, 0x00, 0x00, 0x00])
    encode_message(session_id, stream_id, sequence_nr, client_key, SM_READ_DATA, FLAG_E_LITTLE_ENDIAN, body)
end

# --- Layer 2: the state machine (mirrors crates/xrce-client) ----------------

"""Mirrors `crates/xrce-client::ClientError`. Thrown as an `ArgumentError`
whose message names the refusal — this SDK's own `write_frame`/`put_*!`
functions already throw `ArgumentError` for refusals (see `zerodds.jl`), so
the state machine follows the same idiom rather than introducing a Result
type."""
struct InvalidStateError <: Exception
    msg::String
end
Base.showerror(io::IO, e::InvalidStateError) = print(io, "InvalidStateError: ", e.msg)

"""DDS-XRCE 1.0 client: the reference state machine plus wire encoding.

Spec Sec 7.2 lifecycle: `state` is one of `:disconnected`, `:connecting`,
`:connected`. This is additive to `ZeroDDS.Client`/`ZeroDDS.AsyncReader`,
which speak an older, in-house, non-conformant frame on purpose — this type
is the conformant sibling that actually reaches a real DDS-XRCE agent."""
mutable struct Client
    client_key::Vector{UInt8}
    state::Symbol
    transport
    next_request_id::UInt16
    session_id::UInt8
    stream_id::UInt8
    next_seq::UInt16
    representation_timestamps::Symbol
    unannotated_types::Symbol
end

"""Constructor. `state = :disconnected`; `next_request_id` starts at 1;
default `session_id = 0x01` (the CREATE_CLIENT header itself has session 0x00), `stream_id = 0x80` (builtin-reliable — every
post-handshake operation is reliable) — these match the fixtures;
`next_seq` starts at 0 (the CREATE_CLIENT handshake always sends
`sequence_nr = 0` on `stream_id = NONE` and does not consume this
counter; the first reliable-stream operation gets sequence_nr 0)."""
function Client(client_key::Vector{UInt8}, transport;
                representation_timestamps::Symbol = :absent,
                unannotated_types::Symbol = :appendable)
    length(client_key) == CLIENT_KEY_LEN || throw(ArgumentError("client_key must be $CLIENT_KEY_LEN bytes"))
    representation_timestamps in (:absent, :present) ||
        throw(ArgumentError("representation_timestamps must be :absent or :present, got $(repr(representation_timestamps))"))
    unannotated_types in (:appendable, :final) ||
        throw(ArgumentError("unannotated_types must be :appendable or :final, got $(repr(unannotated_types))"))
    Client(client_key, :disconnected, transport, UInt16(1), UInt8(0x01), UInt8(0x80), UInt16(0),
           representation_timestamps, unannotated_types)
end

"""The `CLIENT_Representation` of this client for `connect!`, with its
`representation_timestamps` keyword of the constructor (default `:absent`).
The Time_t is the wall clock at the time of the call."""
function client_representation(c::Client;
                               version::Vector{UInt8} = UInt8[0x01, 0x00],
                               vendor_id::Vector{UInt8} = UInt8[0x00, 0x00],
                               requested_mtu::UInt16 = DEFAULT_MTU)
    t = time()
    sec = floor(Int64, t)
    encode_client_representation(version, vendor_id, c.client_key, c.session_id;
        requested_mtu = requested_mtu, representation_timestamps = c.representation_timestamps,
        seconds = sec, nanoseconds = floor(Int64, (t - sec) * 1e9),
        unannotated_types = c.unannotated_types)
end

"""The id for this call, then advance — wraps at 16 bits and skips 0 (mirrors
Rust's `next_request_id.wrapping_add(1).max(1)`)."""
function alloc_request_id!(c::Client)
    req = c.next_request_id
    next = UInt16((UInt32(c.next_request_id) + 1) % 0x10000)
    c.next_request_id = next == 0 ? UInt16(1) : next
    req
end

"""Big-endian encoding of the auto-allocated request id — matching
`to_bytes(::ObjectId)` (also big-endian) so both halves of a
BaseObjectRequest use the same byte order (id 1 -> `(0x00, 0x01)`, matching
the same choice the other ports made)."""
request_id_bytes(id::UInt16) = (UInt8((id >> 8) & 0xFF), UInt8(id & 0xFF))

function require_connected(c::Client)
    c.state == :connected || throw(InvalidStateError("operation requires state == :connected"))
end

"""Initiates the handshake: encodes and delivers CREATE_CLIENT, then
transitions to `:connecting`.

Throws `InvalidStateError` unless `state == :disconnected`."""
connect!(c::Client) = connect!(c, client_representation(c))

function connect!(c::Client, representation::Vector{UInt8})
    c.state == :disconnected || throw(InvalidStateError("connect!: already connecting or connected"))
    frame = encode_create_client_message(c.session_id, c.client_key, representation)
    c.transport.deliver(frame)
    c.state = :connecting
    nothing
end

"""Marks the handshake complete (the caller has received STATUS_AGENT
externally). Pure bookkeeping — no wire I/O, matching the Rust reference
exactly.

Throws `InvalidStateError` unless `state == :connecting`."""
function mark_connected!(c::Client)
    c.state == :connecting || throw(InvalidStateError("mark_connected!: not connecting"))
    c.state = :connected
    nothing
end

"""Submits a CREATE operation for the given `ObjectVariant`. Returns the
allocated `request_id`.

Throws `InvalidStateError` unless `state == :connected`."""
function create_object!(c::Client, object_id::ObjectId, variant::ObjectVariant;
                         parent_object_id::Union{ObjectId, Nothing} = nothing,
                         reuse::Bool = false, replace::Bool = false)
    require_connected(c)
    req = alloc_request_id!(c)
    frame = encode_create_message(c.session_id, c.stream_id, c.next_seq, c.client_key,
                                   request_id_bytes(req), object_id, variant;
                                   parent_object_id = parent_object_id, reuse = reuse, replace = replace)
    c.transport.deliver(frame)
    c.next_seq = UInt16((UInt32(c.next_seq) + 1) % 0x10000)
    req
end

"""Submits a DELETE operation. Returns the allocated `request_id`.

Throws `InvalidStateError` unless connected."""
function delete_object!(c::Client, object_id::ObjectId)
    require_connected(c)
    req = alloc_request_id!(c)
    frame = encode_delete_message(c.session_id, c.stream_id, c.next_seq, c.client_key,
                                   request_id_bytes(req), object_id)
    c.transport.deliver(frame)
    c.next_seq = UInt16((UInt32(c.next_seq) + 1) % 0x10000)
    req
end

"""Submits a WRITE_DATA operation. Returns the allocated `request_id`.

Throws `InvalidStateError` unless connected."""
function request_write!(c::Client, writer_object_id::ObjectId, payload::Vector{UInt8})
    require_connected(c)
    req = alloc_request_id!(c)
    frame = encode_write_data_message(c.session_id, c.stream_id, c.next_seq, c.client_key,
                                       request_id_bytes(req), writer_object_id, payload)
    c.transport.deliver(frame)
    c.next_seq = UInt16((UInt32(c.next_seq) + 1) % 0x10000)
    req
end

"""Submits a READ_DATA operation (pull-based). Returns the allocated
`request_id`. `preferred_stream_id` is the client's own `stream_id`,
`data_format` is always Data (0x00).

Throws `InvalidStateError` unless connected."""
function request_read!(c::Client, reader_object_id::ObjectId)
    require_connected(c)
    req = alloc_request_id!(c)
    frame = encode_read_data_message(c.session_id, c.stream_id, c.next_seq, c.client_key,
                                      request_id_bytes(req), reader_object_id, c.stream_id)
    c.transport.deliver(frame)
    c.next_seq = UInt16((UInt32(c.next_seq) + 1) % 0x10000)
    req
end

"""Closes the session — goes back to `:disconnected`. That is all: does NOT
reset `next_request_id`, `session_id`, `stream_id`, or `next_seq` —
`crates/xrce-client::disconnect` only resets `state`. Matching this exactly
matters, per the reference's own comment: a "helpful" reset of more than
`state` is a divergence, not an improvement."""
function disconnect!(c::Client)
    c.state = :disconnected
    nothing
end

end # module XrceClient
