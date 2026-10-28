// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 client, ported from `crates/xrce-client`. Two layers:
//
//   1. Pure wire encoders (no state, no I/O) for MessageHeader, Submessage-
//      Header, ObjectId, BaseObjectRequest, ObjectVariant::ByReference, and
//      the five submessage bodies this client speaks
//      (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA).
//   2. `XrceClient`, a synchronous state machine mirroring
//      `crates/xrce-client::XrceClient` 1:1 in its lifecycle rules
//      (Disconnected -> Connecting -> Connected, monotonic request ids,
//      `disconnect` resetting only `state`).
//
// This reuses the SDK's existing `Transport` type (see `zerodds.d`) as-is.
// It is additive: `Client`/`AsyncReader` in `zerodds.d` speak an older,
// in-house, non-conformant frame on purpose; this module is the conformant
// DDS-XRCE 1.0 sibling that can talk to a real agent.
//
// One difference from the Rust reference is worth flagging up front: in
// `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
// `request_read` take underscore-prefixed parameters and never touch the
// wire — they are pure request-id bookkeeping, with encoding left to a
// caller that does not exist yet. This port closes that gap: every op here
// actually encodes its submessage with the Layer 1 encoders below and calls
// `transport.deliver`, because "reach a real XRCE agent over the network" is
// the entire point of the port. The state-machine *rules* (refusals,
// transitions, id allocation, what `disconnect` resets) are unchanged.

module xrce_client;

import zerodds : Transport;

// --- ObjectId (spec Sec 7.2.1) ------------------------------------------------

/// 4-bit OBJK_* codes (spec Sec 7.2 Table 4). Values are the wire codes
/// themselves, not sequential — 0x07..0x09 have no spec meaning and are
/// deliberately absent so no enum value can round-trip an invalid kind.
enum ObjectKind : ubyte {
    PARTICIPANT = 1,
    TOPIC = 2,
    PUBLISHER = 3,
    SUBSCRIBER = 4,
    DATAWRITER = 5,
    DATAREADER = 6,
    TYPE = 10,
    QOSPROFILE = 11,
    APPLICATION = 12,
    AGENT = 13,
    CLIENT = 14,
    DOMAIN = 15,
}

/// The packed 16-bit word: `rawId` (12 bits) in the high bits, `kind`
/// (4 bits) in the low bits. Spec Sec 7.2.1.
struct ObjectId {
    ushort word;

    /// `word = (rawId << 4) | kind`.
    static ObjectId pack(ushort rawId, ObjectKind kind) {
        return ObjectId(cast(ushort) ((rawId << 4) | cast(ushort) kind));
    }

    /// Constructs directly from an already-packed 16-bit word (e.g. a value
    /// read off the wire, or a well-known singleton like OBJECTID_AGENT).
    static ObjectId fromRawWord(ushort word) {
        return ObjectId(word);
    }

    /// Wire encoding: big-endian — the one field in this spec that is *not*
    /// little-endian (everything else in a DDS-XRCE message is LE).
    ubyte[2] toBytes() const {
        return [cast(ubyte) ((word >> 8) & 0xFF), cast(ubyte) (word & 0xFF)];
    }
}

// --- MessageHeader (spec Sec 8.3.2) ------------------------------------------

/// Length of the ClientKey field, spec Sec 7.7.
enum size_t ClientKeyLen = 4;

alias ClientKeyBytes = ubyte[ClientKeyLen];

/// `session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80` does
/// not (4-byte header). Reserved values: SESSION_NONE_WITH_KEY = 0x00,
/// SESSION_NONE_WITHOUT_KEY = 0x80.
enum ubyte SessionCarriesKeyMax = 0x7F;

/// `session_id(1) stream_id(1) sequence_nr(2, LE) [client_key(4)]`. Every
/// operation in this port uses a session_id <= 0x7F, so the key is always
/// present — callers never construct a keyless header here.
ubyte[] encodeMessageHeader(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                             ClientKeyBytes clientKey) {
    ubyte[] result = [sessionId, streamId,
                       cast(ubyte) (sequenceNr & 0xFF), cast(ubyte) ((sequenceNr >> 8) & 0xFF)];
    if (sessionId <= SessionCarriesKeyMax) {
        result ~= clientKey[];
    }
    return result;
}

// --- SubmessageHeader (spec Sec 8.3.4) ---------------------------------------

/// FLAG_E_LITTLE_ENDIAN, bit 0. Every submessage this client sends sets it —
/// the whole port only ever writes little-endian bodies.
enum ubyte FlagLittleEndian = 0x01;

/// `submessage_id(1) flags(1) submessage_length(2, always LE)`. The length
/// field is always little-endian regardless of the body's own endianness
/// flag.
ubyte[] encodeSubmessageHeader(ubyte submessageId, ubyte flags, ushort bodyLen) {
    return [submessageId, flags, cast(ubyte) (bodyLen & 0xFF), cast(ubyte) ((bodyLen >> 8) & 0xFF)];
}

// Submessage ids used by this client (spec Sec 8.3.4 Table 8).
enum ubyte SmCreateClient = 0x00;
enum ubyte SmCreate = 0x01;
enum ubyte SmDelete = 0x03;
enum ubyte SmWriteData = 0x07;
enum ubyte SmReadData = 0x08;

// --- BaseObjectRequest (spec Sec 7.7.8) --------------------------------------

/// The wire RequestId is two raw, caller-supplied octets — not a
/// little-endian encoding of an integer. `crates/xrce`'s own
/// `BaseObjectRequest::encode` copies them through unchanged.
alias RequestIdBytes = ubyte[2];

/// `request_id(2 raw bytes) ++ object_id.toBytes()(2, big-endian)`.
ubyte[] encodeBaseObjectRequest(RequestIdBytes requestId, ObjectId objectId) {
    ubyte[] result = [requestId[0], requestId[1]];
    result ~= objectId.toBytes()[];
    return result;
}

// --- ObjectVariant (spec Sec 7.7.3) -------------------------------------------

/// ObjectVariant::ByReference's *inner* payload only (spec Sec 7.7.2):
/// `discriminator(1)=0x01 ++ 3 padding bytes ++ length(4, LE)=strlen+1 ++
/// name bytes ++ NUL`. The length includes the NUL terminator XCDR2 strings
/// carry. Correct only when this structure itself starts 4-aligned — not
/// the wire form a real agent accepts on its own; see
/// `encodeObjectVariantForKind`, whose 1-byte outer ObjectKind shifts this
/// padding to two bytes.
ubyte[] encodeByReference(string name) {
    uint n = cast(uint) (name.length + 1);
    ubyte[] result = [cast(ubyte) 0x01, cast(ubyte) 0x00, cast(ubyte) 0x00, cast(ubyte) 0x00,
                       cast(ubyte) (n & 0xFF), cast(ubyte) ((n >> 8) & 0xFF),
                       cast(ubyte) ((n >> 16) & 0xFF), cast(ubyte) ((n >> 24) & 0xFF)];
    foreach (c; name) {
        result ~= cast(ubyte) c;
    }
    result ~= cast(ubyte) 0x00;
    return result;
}

/// The real wire ObjectVariant (spec Sec 7.7.3): a **two-level**
/// discriminated union — the outer ObjectKind byte (`objectId`'s own low 4
/// bits — the outer discriminator and the object being created always
/// agree, by construction), then the inner REPRESENTATION_BY_REFERENCE
/// payload (disc 0x01, two padding bytes, then the same XCDR2 string as
/// `encodeByReference`), then that kind's own trailing field —
/// `parentObjectId` (an ObjectId, unaligned) for a
/// Topic/Publisher/Subscriber/Datawriter/Datareader, `null` for every
/// other kind — byte-oracled in `crates/xrce/src/object_repr.rs` against
/// real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
/// participant_id: commit 31c62df2a63). This — not `encodeByReference`
/// alone — is the only form a real agent accepts.
ubyte[] encodeObjectVariantForKind(ObjectId objectId, string name, ObjectId* parentObjectId = null) {
    uint n = cast(uint) (name.length + 1);
    ubyte[] result = [cast(ubyte) (objectId.word & 0x0F),
                       cast(ubyte) 0x01, cast(ubyte) 0x00, cast(ubyte) 0x00,
                       cast(ubyte) (n & 0xFF), cast(ubyte) ((n >> 8) & 0xFF),
                       cast(ubyte) ((n >> 16) & 0xFF), cast(ubyte) ((n >> 24) & 0xFF)];
    foreach (c; name) {
        result ~= cast(ubyte) c;
    }
    result ~= cast(ubyte) 0x00;
    if (parentObjectId !is null) {
        result ~= parentObjectId.toBytes()[];
    }
    return result;
}

// --- Full message encoders ----------------------------------------------------
//
// A message is `header ++ submessage_header ++ body`, concatenated directly:
// this client only ever sends one submessage per message, so the
// between-submessages alignment padding rule never applies here.

private ubyte[] encodeMessage(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                               ClientKeyBytes clientKey,
                               ubyte submessageId, ubyte flags, ubyte[] body) {
    ubyte[] result = encodeMessageHeader(sessionId, streamId, sequenceNr, clientKey);
    result ~= encodeSubmessageHeader(submessageId, flags, cast(ushort) body.length);
    result ~= body;
    return result;
}

/// eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
/// module doc has the byte-oracle measurement backing it), used as this
/// port's own CREATE_CLIENT requested_mtu.
enum ushort DefaultMtu = 508;

/// Setting "representation timestamps" (DDS-XRCE 1.0 Sec 7.8.2.1): the prose
/// lists `Time_t client_timestamp` in `CLIENT_Representation` after
/// `xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not.
/// `absent` (default) is the Annex A form (16 octets), `present` the prose
/// form. The agent has to be set the same way (`representation_timestamps:`
/// in its configuration).
enum RepresentationTimestamps {
    absent,
    present,
}

/// Setting "unannotated types" for the `Time_t` of the client timestamp (no
/// `@extensibility`, so APPENDABLE by DDS-XTypes 1.3 7.3.1.2.1.8): `appendable`
/// (default) writes a DHEADER = 8 in front of it, `final_` does not (`final`
/// is a D keyword). Read only with `RepresentationTimestamps.present`. This
/// client has no other unannotated type; the enum is the same two values as
/// `reliable.UnannotatedTypes`, kept here so `xrce_client.d` stands alone.
enum XrceUnannotatedTypes {
    appendable,
    final_,
}

/// `CLIENT_Representation` (Annex A, spec Sec 7.8.2.1): xrce_cookie(4) ++
/// xrce_version(2) ++ xrce_vendor_id(2) ++ [Time_t client_timestamp, only
/// with `RepresentationTimestamps.present`: DHEADER u32 = 8 when
/// `appendable`, seconds i32, nanoseconds u32, little endian] ++
/// client_key(4) ++ session_id(1) ++ a 0 properties-absent octet ++
/// requested_mtu(2, LE) — eProsima's own non-normative MTU extension, always
/// sent by this client. No PropertySeq support here — this client sends none.
/// `seconds` / `nanoseconds` are explicit so a test is deterministic; they
/// are only read with `present`. The defaults give the Annex A form.
ubyte[] encodeClientRepresentation(ubyte[2] version_, ubyte[2] vendorId,
                                    ClientKeyBytes clientKey, ubyte sessionId,
                                    ushort requestedMtu = DefaultMtu,
                                    RepresentationTimestamps timestamps = RepresentationTimestamps.absent,
                                    XrceUnannotatedTypes unannotatedTypes = XrceUnannotatedTypes.appendable,
                                    int seconds = 0, uint nanoseconds = 0) {
    ubyte[] result = [cast(ubyte) 'X', cast(ubyte) 'R', cast(ubyte) 'C', cast(ubyte) 'E'];
    result ~= version_[];
    result ~= vendorId[];
    if (timestamps == RepresentationTimestamps.present) {
        // Offset 8 is 4-aligned: no padding.
        if (unannotatedTypes == XrceUnannotatedTypes.appendable) {
            result ~= [cast(ubyte) 0x08, cast(ubyte) 0x00, cast(ubyte) 0x00, cast(ubyte) 0x00];
        }
        immutable uint sec = cast(uint) seconds;
        foreach (shift; [0, 8, 16, 24]) result ~= cast(ubyte) ((sec >> shift) & 0xFF);
        foreach (shift; [0, 8, 16, 24]) result ~= cast(ubyte) ((nanoseconds >> shift) & 0xFF);
    }
    result ~= clientKey[];
    result ~= sessionId;
    result ~= cast(ubyte) 0x00; // properties absent
    result ~= cast(ubyte) (requestedMtu & 0xFF);
    result ~= cast(ubyte) ((requestedMtu >> 8) & 0xFF);
    return result;
}

/// Spec Sec 7.8.2. The handshake precedes any session, so the header
/// carries SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) plus the ClientKey when
/// the requested `sessionId` is 0..127, and
/// SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) without ClientKey when it is
/// 128..255 (DDS-XRCE 1.0 Sec 8.3.2.1); stream=NONE(0x00), sequence_nr=0.
/// The requested `sessionId` itself is carried in the
/// `CLIENT_Representation` payload. `representation` is the XCDR2-encoded
/// `CLIENT_Representation` — build it with `encodeClientRepresentation` —
/// passed through byte for byte here.
ubyte[] encodeCreateClientMessage(ubyte sessionId, ClientKeyBytes clientKey, ubyte[] representation) {
    immutable ubyte headerSession = sessionId <= SessionCarriesKeyMax ? 0x00 : 0x80;
    return encodeMessage(headerSession, 0x00, 0, clientKey, SmCreateClient, FlagLittleEndian, representation);
}

/// Spec Sec 7.8.3. Flags = E | (reuse ? 0x02 : 0) | (replace ? 0x04 : 0).
/// Body = BaseObjectRequest ++ the real, two-level ObjectVariant
/// (`encodeObjectVariantForKind`) — the only representation this port
/// encodes (ByXmlString/InBinary are out of scope, same as
/// `crates/xrce-client`).
ubyte[] encodeCreateMessage(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                             ClientKeyBytes clientKey, RequestIdBytes requestId,
                             ObjectId objectId, string name,
                             ObjectId* parentObjectId = null,
                             bool reuse = false, bool replace = false) {
    ubyte flags = FlagLittleEndian;
    if (reuse) flags |= 0x02;
    if (replace) flags |= 0x04;
    ubyte[] body = encodeBaseObjectRequest(requestId, objectId);
    body ~= encodeObjectVariantForKind(objectId, name, parentObjectId);
    return encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmCreate, flags, body);
}

/// Spec Sec 7.8.3. Body = BaseObjectRequest only — no ObjectVariant.
ubyte[] encodeDeleteMessage(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                             ClientKeyBytes clientKey, RequestIdBytes requestId,
                             ObjectId objectId) {
    ubyte[] body = encodeBaseObjectRequest(requestId, objectId);
    return encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmDelete, FlagLittleEndian, body);
}

/// Spec Sec 7.8.4. Body = BaseObjectRequest ++ serialized_data, verbatim and
/// with no length prefix of its own — `submessage_length` is what bounds the
/// sample.
ubyte[] encodeWriteDataMessage(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                                ClientKeyBytes clientKey, RequestIdBytes requestId,
                                ObjectId objectId, ubyte[] serializedData) {
    ubyte[] body = encodeBaseObjectRequest(requestId, objectId);
    body ~= serializedData;
    return encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmWriteData, FlagLittleEndian, body);
}

/// Spec Sec 7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This port
/// never sets a content filter or delivery control, so ReadSpecification is
/// always `preferred_stream_id, data_format=0x00, content_filter_present=0x00,
/// delivery_control_present=0x00`.
ubyte[] encodeReadDataMessage(ubyte sessionId, ubyte streamId, ushort sequenceNr,
                               ClientKeyBytes clientKey, RequestIdBytes requestId,
                               ObjectId objectId, ubyte preferredStreamId) {
    ubyte[] body = encodeBaseObjectRequest(requestId, objectId);
    body ~= [preferredStreamId, cast(ubyte) 0x00, cast(ubyte) 0x00, cast(ubyte) 0x00];
    return encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmReadData, FlagLittleEndian, body);
}

// --- Layer 2: the state machine (mirrors crates/xrce-client) ----------------

/// Spec Sec 7.2 lifecycle.
enum ClientState {
    DISCONNECTED, /// Initial: the client knows its ClientKey but is not
                  /// connected.
    CONNECTING,   /// CREATE_CLIENT sent, STATUS_AGENT not yet received.
    CONNECTED,    /// Connected — operations are possible.
}

/// Mirrors `crates/xrce-client::ClientError`. `TRANSPORT` and `QUEUE_FULL`
/// are carried over for parity with the Rust enum even though neither is
/// reachable from this D port: `Transport.deliver` has no failure channel at
/// all, and `QUEUE_FULL` is reserved/unused even in the Rust reference (its
/// `out_queue` has no capacity cap).
enum ClientErrorKind {
    INVALID_STATE, /// Operation in the wrong lifecycle status.
    TRANSPORT,     /// Unreachable from this D port — see above.
    QUEUE_FULL,    /// Unreachable — reserved for parity with Rust.
}

/// Thrown in place of Rust's `Result<_, ClientError>` — this SDK signals
/// refusals via exceptions throughout (see `WireException` in `zerodds.d`),
/// not a Result type, so the state machine follows the same idiom here.
class XrceClientError : Exception {
    ClientErrorKind kind;
    this(string msg, ClientErrorKind kind) {
        super(msg);
        this.kind = kind;
    }
}

private void refuseState(string msg) {
    throw new XrceClientError(msg, ClientErrorKind.INVALID_STATE);
}

class XrceClient {
    private ClientKeyBytes clientKeyVal;
    private ClientState stateVal;
    private Transport transport;
    private ushort nextRequestId;
    private ubyte sessionIdVal;
    private ubyte streamIdVal;
    private ushort nextSeq;
    private RepresentationTimestamps representationTimestampsVal;
    private XrceUnannotatedTypes unannotatedTypesVal;

    /// `state = Disconnected`; `nextRequestId` starts at 1; default
    /// `sessionId = 0x01`, `streamId = 0x80` (builtin-reliable — every
    /// post-handshake operation is reliable) — these match the fixtures;
    /// `nextSeq` starts at 0 (the CREATE_CLIENT handshake always sends
    /// `sequence_nr = 0` on `streamId = NONE` and does not consume this
    /// counter; the first reliable-stream operation gets sequence_nr 0).
    ///
    /// `representationTimestamps` (default `absent`) and `unannotatedTypes`
    /// (default `appendable`) are used by `connect(version_, vendorId)`.
    this(ClientKeyBytes clientKey, Transport transport,
         RepresentationTimestamps representationTimestamps = RepresentationTimestamps.absent,
         XrceUnannotatedTypes unannotatedTypes = XrceUnannotatedTypes.appendable) {
        this.representationTimestampsVal = representationTimestamps;
        this.unannotatedTypesVal = unannotatedTypes;
        this.clientKeyVal = clientKey;
        this.stateVal = ClientState.DISCONNECTED;
        this.transport = transport;
        this.nextRequestId = 1;
        this.sessionIdVal = 0x01;
        this.streamIdVal = 0x80;
        this.nextSeq = 0;
    }

    ClientState state() const { return stateVal; }
    ClientKeyBytes clientKey() const { return clientKeyVal; }
    RepresentationTimestamps representationTimestamps() const { return representationTimestampsVal; }
    XrceUnannotatedTypes unannotatedTypes() const { return unannotatedTypesVal; }

    /// The id for this call, then advance — wraps at 16 bits and skips 0
    /// (mirrors Rust's `next_request_id.wrapping_add(1).max(1)`).
    private ushort allocRequestId() {
        ushort result = nextRequestId;
        ushort next = cast(ushort) (nextRequestId + 1);
        if (next == 0) next = 1;
        nextRequestId = next;
        return result;
    }

    /// Big-endian encoding of the auto-allocated request id — matching
    /// `ObjectId.toBytes()` (also big-endian) so both halves of a
    /// `BaseObjectRequest` use the same byte order. `crates/xrce`'s own
    /// `BaseObjectRequest` treats RequestId as two opaque bytes and leaves
    /// the int-to-bytes choice to the caller — this is that choice (id 1 ->
    /// `[0x00, 0x01]`, matching the same choice the Lua and Nim ports made).
    private static RequestIdBytes requestIdBytes(ushort id) {
        return [cast(ubyte) ((id >> 8) & 0xFF), cast(ubyte) (id & 0xFF)];
    }

    private void requireConnected() {
        if (stateVal != ClientState.CONNECTED) {
            refuseState("operation requires state == Connected");
        }
    }

    /// Initiates the handshake: encodes and delivers CREATE_CLIENT, then
    /// transitions to Connecting.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless `state ==
    /// Disconnected`.
    void connect(ubyte[] representation) {
        if (stateVal != ClientState.DISCONNECTED) {
            refuseState("connect: already connecting or connected");
        }
        auto msg = encodeCreateClientMessage(sessionIdVal, clientKeyVal, representation);
        transport.deliver(msg);
        stateVal = ClientState.CONNECTING;
    }

    /// As `connect(representation)`, building the `CLIENT_Representation` with
    /// the settings of this client; with `RepresentationTimestamps.present`
    /// the client timestamp is the wall clock at the time of the call.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless `state ==
    /// Disconnected`.
    void connect(ubyte[2] version_, ubyte[2] vendorId) {
        import std.datetime.systime : Clock;
        int seconds = 0;
        uint nanoseconds = 0;
        if (representationTimestampsVal == RepresentationTimestamps.present) {
            auto now = Clock.currTime();
            seconds = cast(int) now.toUnixTime();
            nanoseconds = cast(uint) now.fracSecs.total!"nsecs";
        }
        connect(encodeClientRepresentation(version_, vendorId, clientKeyVal, sessionIdVal,
                                           DefaultMtu, representationTimestampsVal,
                                           unannotatedTypesVal, seconds, nanoseconds));
    }

    /// Marks the handshake complete (the caller has received STATUS_AGENT
    /// externally). Pure bookkeeping — no wire I/O, matching the Rust
    /// reference exactly.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless `state ==
    /// Connecting`.
    void markConnected() {
        if (stateVal != ClientState.CONNECTING) {
            refuseState("markConnected: not connecting");
        }
        stateVal = ClientState.CONNECTED;
    }

    /// Submits a CREATE operation for an `ObjectVariant::ByReference(name)`.
    /// Returns the allocated `request_id`.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless `state ==
    /// Connected`.
    ushort createObject(ObjectId objectId, string name, ObjectId* parentObjectId = null,
                        bool reuse = false, bool replace = false) {
        requireConnected();
        auto req = allocRequestId();
        auto msg = encodeCreateMessage(sessionIdVal, streamIdVal, nextSeq, clientKeyVal,
                                        requestIdBytes(req), objectId, name, parentObjectId, reuse, replace);
        transport.deliver(msg);
        nextSeq = cast(ushort) (nextSeq + 1);
        return req;
    }

    /// Submits a DELETE operation. Returns the allocated `request_id`.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless connected.
    ushort deleteObject(ObjectId objectId) {
        requireConnected();
        auto req = allocRequestId();
        auto msg = encodeDeleteMessage(sessionIdVal, streamIdVal, nextSeq, clientKeyVal,
                                        requestIdBytes(req), objectId);
        transport.deliver(msg);
        nextSeq = cast(ushort) (nextSeq + 1);
        return req;
    }

    /// Submits a WRITE_DATA operation. Returns the allocated `request_id`.
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless connected.
    ushort requestWrite(ObjectId writerObjectId, ubyte[] payload) {
        requireConnected();
        auto req = allocRequestId();
        auto msg = encodeWriteDataMessage(sessionIdVal, streamIdVal, nextSeq, clientKeyVal,
                                           requestIdBytes(req), writerObjectId, payload);
        transport.deliver(msg);
        nextSeq = cast(ushort) (nextSeq + 1);
        return req;
    }

    /// Submits a READ_DATA operation (pull-based). Returns the allocated
    /// `request_id`. `preferred_stream_id` is the client's own `stream_id`,
    /// `data_format` is always Data (0x00).
    ///
    /// Throws `XrceClientError` (`INVALID_STATE`) unless connected.
    ushort requestRead(ObjectId readerObjectId) {
        requireConnected();
        auto req = allocRequestId();
        auto msg = encodeReadDataMessage(sessionIdVal, streamIdVal, nextSeq, clientKeyVal,
                                          requestIdBytes(req), readerObjectId, streamIdVal);
        transport.deliver(msg);
        nextSeq = cast(ushort) (nextSeq + 1);
        return req;
    }

    /// Closes the session — goes back to Disconnected. That is all: does NOT
    /// reset `nextRequestId`, `sessionId`, `streamId`, or `nextSeq` —
    /// `crates/xrce-client::disconnect` only resets `state`. Matching this
    /// exactly matters, per the reference's own comment: a "helpful" reset of
    /// more than `state` is a divergence, not an improvement.
    void disconnect() {
        stateVal = ClientState.DISCONNECTED;
    }
}
