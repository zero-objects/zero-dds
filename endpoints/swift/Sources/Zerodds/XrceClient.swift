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
// This reuses the SDK's existing `Transport` protocol (see `Zerodds.swift`)
// as-is. It is additive: `Client`/`AsyncReader` in `Zerodds.swift` speak an
// older, in-house, non-conformant frame on purpose; this file is the
// conformant DDS-XRCE 1.0 sibling that can talk to a real agent.
//
// One difference from the Rust reference is worth flagging up front: in
// `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
// `request_read` take unused parameters and never touch the wire — they are
// pure request-id bookkeeping, with encoding left to a caller that does not
// exist yet. This port closes that gap: every op here actually encodes its
// submessage with the Layer 1 encoders below and calls `transport.deliver`,
// because "reach a real XRCE agent over the network" is the entire point of
// the port. The state-machine *rules* (refusals, transitions, id allocation,
// what `disconnect` resets) are unchanged.
//
// Idiom: Swift's own concurrency/error model uses `throws`, not Rust's
// `Result<_, ClientError>` — every fallible operation below is a `throws`
// function throwing `XrceClientError`, matching `WireError`'s own use of
// `throws` elsewhere in this SDK.

import Foundation

// --- ObjectId (spec §7.2.1) --------------------------------------------------

/// 4-bit OBJK_* codes (spec §7.2 Table 4). Values are the wire codes
/// themselves, not sequential — 0x07..0x09 have no spec meaning and are
/// deliberately absent so no case can round-trip an invalid kind.
public enum ObjectKind: UInt16 {
    case participant = 1
    case topic = 2
    case publisher = 3
    case subscriber = 4
    case datawriter = 5
    case datareader = 6
    case type = 10
    case qosProfile = 11
    case application = 12
    case agent = 13
    case client = 14
    case domain = 15
}

/// The packed 16-bit word: `rawId` (12 bits) in the high bits, `kind`
/// (4 bits) in the low bits. Spec §7.2.1.
public struct ObjectId: Equatable {
    public let word: UInt16

    /// `word = (rawId << 4) | kind`.
    public init(rawId: UInt16, kind: ObjectKind) {
        word = (rawId << 4) | kind.rawValue
    }

    /// Constructs directly from an already-packed 16-bit word (e.g. a value
    /// read off the wire, or a well-known singleton like OBJECTID_AGENT).
    public init(fromRawWord word: UInt16) {
        self.word = word
    }

    /// Wire encoding: big-endian — the one field in this spec that is *not*
    /// little-endian (everything else in a DDS-XRCE message is LE).
    public func toBytes() -> [UInt8] {
        [UInt8((word >> 8) & 0xFF), UInt8(word & 0xFF)]
    }
}

// --- MessageHeader (spec §8.3.2) ---------------------------------------------

/// Length of the ClientKey field, spec §7.7.
public let clientKeyLen = 4
public typealias ClientKeyBytes = [UInt8]

/// `session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80` does
/// not (4-byte header). Reserved values: SESSION_NONE_WITH_KEY = 0x00,
/// SESSION_NONE_WITHOUT_KEY = 0x80.
public let sessionCarriesKeyMax: UInt8 = 0x7F

/// `session_id(1) stream_id(1) sequence_nr(2, LE) [client_key(4)]`. Every
/// operation in this port uses a session_id <= 0x7F, so the key is always
/// present — callers never construct a keyless header here.
func encodeMessageHeader(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes
) -> [UInt8] {
    var result: [UInt8] = [sessionId, streamId, UInt8(sequenceNr & 0xFF), UInt8((sequenceNr >> 8) & 0xFF)]
    if sessionId <= sessionCarriesKeyMax {
        result.append(contentsOf: clientKey)
    }
    return result
}

// --- SubmessageHeader (spec §8.3.4) ------------------------------------------

/// FLAG_E_LITTLE_ENDIAN, bit 0. Every submessage this client sends sets it —
/// the whole port only ever writes little-endian bodies.
let flagLittleEndian: UInt8 = 0x01

/// `submessage_id(1) flags(1) submessage_length(2, always LE)`. The length
/// field is always little-endian regardless of the body's own endianness
/// flag.
func encodeSubmessageHeader(submessageId: UInt8, flags: UInt8, bodyLen: UInt16) -> [UInt8] {
    [submessageId, flags, UInt8(bodyLen & 0xFF), UInt8((bodyLen >> 8) & 0xFF)]
}

// Submessage ids used by this client (spec §8.3.4 Table 8).
private let smCreateClient: UInt8 = 0x00
private let smCreate: UInt8 = 0x01
private let smDelete: UInt8 = 0x03
private let smWriteData: UInt8 = 0x07
private let smReadData: UInt8 = 0x08

// --- BaseObjectRequest (spec §7.7.8) -----------------------------------------

/// The wire RequestId is two raw, caller-supplied octets — not a
/// little-endian encoding of an integer. `crates/xrce`'s own
/// `BaseObjectRequest::encode` copies them through unchanged.
public typealias RequestIdBytes = (UInt8, UInt8)

/// `request_id(2 raw bytes) ++ object_id.toBytes()(2, big-endian)`.
func encodeBaseObjectRequest(requestId: RequestIdBytes, objectId: ObjectId) -> [UInt8] {
    [requestId.0, requestId.1] + objectId.toBytes()
}

// --- ObjectVariant (spec §7.7.3) ---------------------------------------------

/// The trailing field several `ObjectKind`s' own `..._Representation` struct
/// appends after the inner representation (spec §7.7.3). At most one of
/// `domainId`/`parentObjectId` is ever set for a given, valid `ObjectKind`.
public struct ObjectVariantExtra {
    /// `OBJK_PARTICIPANT_Representation.domain_id`.
    public var domainId: Int16?
    /// `OBJK_TOPIC_Representation.participant_id` /
    /// `OBJK_PUBLISHER_Representation.participant_id` /
    /// `OBJK_SUBSCRIBER_Representation.participant_id` /
    /// `DATAWRITER_Representation.publisher_id` /
    /// `DATAREADER_Representation.subscriber_id` — "the id of the container
    /// this one is created under", whatever Annex A calls the field for this
    /// particular kind.
    public var parentObjectId: ObjectId?

    public init(domainId: Int16? = nil, parentObjectId: ObjectId? = nil) {
        self.domainId = domainId
        self.parentObjectId = parentObjectId
    }
}

/// The real wire `ObjectVariant` (spec §7.7.3): a **two-level** discriminated
/// union — the outer `ObjectKind` byte (this object's own kind nibble,
/// `objectId.word & 0x0F` — the outer discriminator and the object being
/// created always agree, by construction), then the inner
/// REPRESENTATION_BY_REFERENCE payload (`disc(1)=0x01 ++ 2 padding bytes ++
/// length(4, LE)=strlen+1 ++ name bytes ++ NUL`), then that kind's own
/// trailing field: `domainId` (`short`, 2-byte aligned) for OBJK_PARTICIPANT,
/// `parentObjectId` (`ObjectId`, unaligned) for OBJK_TOPIC/OBJK_PUBLISHER/
/// OBJK_SUBSCRIBER/OBJK_DATAWRITER/OBJK_DATAREADER, or nothing for every
/// other kind — byte-oracled in `crates/xrce/src/object_repr.rs` against
/// real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (OBJK_TOPIC's
/// own participant_id: commit 31c62df2a63). This — not the single-level
/// form a previous version of this port wrote — is the only form a real
/// agent accepts.
func encodeObjectVariantForKind(objectId: ObjectId, name: String, extra: ObjectVariantExtra = ObjectVariantExtra()) -> [UInt8] {
    let nameBytes = Array(name.utf8)
    let n = UInt32(nameBytes.count + 1)
    var result: [UInt8] = [
        UInt8(objectId.word & 0x0F), // outer ObjectKind byte
        0x01, 0x00, 0x00, // inner disc=BY_REFERENCE + 2 padding bytes
        UInt8(n & 0xFF), UInt8((n >> 8) & 0xFF), UInt8((n >> 16) & 0xFF), UInt8((n >> 24) & 0xFF),
    ]
    result.append(contentsOf: nameBytes)
    result.append(0x00)
    if let domainId = extra.domainId {
        if result.count % 2 != 0 { result.append(0) } // `short` aligns to 2 bytes; ObjectId (below) does not
        let d = UInt16(bitPattern: domainId)
        result.append(UInt8(d & 0xFF))
        result.append(UInt8((d >> 8) & 0xFF))
    }
    if let parent = extra.parentObjectId {
        result.append(contentsOf: parent.toBytes())
    }
    return result
}

// --- Full message encoders ---------------------------------------------------
//
// A message is `header ++ submessage_header ++ body`, concatenated directly:
// this client only ever sends one submessage per message, so the
// between-submessages alignment padding rule never applies here.

private func encodeMessage(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes,
    submessageId: UInt8, flags: UInt8, body: [UInt8]
) -> [UInt8] {
    var result = encodeMessageHeader(
        sessionId: sessionId, streamId: streamId, sequenceNr: sequenceNr, clientKey: clientKey)
    result.append(contentsOf: encodeSubmessageHeader(
        submessageId: submessageId, flags: flags, bodyLen: UInt16(body.count)))
    result.append(contentsOf: body)
    return result
}

/// eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
/// module doc has the byte-oracle measurement backing it), used as this
/// port's own CREATE_CLIENT `requested_mtu`.
public let defaultMtu: UInt16 = 508

/// `CLIENT_Representation` (Annex A, spec §7.8.2.1): `xrce_cookie(4) ++
/// xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1) ++
/// a 0 properties-absent octet ++ requestedMtu(2, LE)` — eProsima's own
/// non-normative MTU extension, always sent by this port. No `PropertySeq`
/// support here — this client sends none.
///
/// `representationTimestamps` (default `.absent`) selects the form of
/// DDS-XRCE 1.0 §7.8.2.1: the prose lists `Time_t client_timestamp` after
/// `xrce_vendor_id`, the IDL of Annex A and the bytes of Annex B.1 do not
/// have it. The bytes do not tell the forms apart; the agent has to be set the
/// same way (`representation_timestamps:` in its configuration). With
/// `.present` the `Time_t` (`timestamp`: seconds i32, nanoseconds u32,
/// little-endian; default: the wall clock at the time of the call) follows
/// `xrce_vendor_id`. `Time_t` has no `@extensibility`, so it is APPENDABLE
/// (DDS-XTypes 1.3 7.3.1.2.1.8): a DHEADER u32 = 8 in front (28 octets in
/// all); `unannotatedTypes: .final` leaves it out (24 octets).
public func encodeClientRepresentation(
    version: (UInt8, UInt8), vendorId: (UInt8, UInt8), clientKey: ClientKeyBytes,
    sessionId: UInt8, requestedMtu: UInt16 = defaultMtu,
    representationTimestamps: RepresentationTimestamps = .absent,
    timestamp: (seconds: Int32, nanoseconds: UInt32)? = nil,
    unannotatedTypes: XrceUnannotatedTypes = .appendable
) -> [UInt8] {
    func u32le(_ v: UInt32) -> [UInt8] {
        [UInt8(v & 0xFF), UInt8((v >> 8) & 0xFF), UInt8((v >> 16) & 0xFF), UInt8((v >> 24) & 0xFF)]
    }
    var result: [UInt8] = Array("XRCE".utf8)
    result.append(contentsOf: [version.0, version.1, vendorId.0, vendorId.1])
    if representationTimestamps == .present {
        let t = timestamp ?? wallClockTime()
        if unannotatedTypes == .appendable { result.append(contentsOf: u32le(8)) }
        result.append(contentsOf: u32le(UInt32(bitPattern: t.seconds)))
        result.append(contentsOf: u32le(t.nanoseconds))
    }
    result.append(contentsOf: clientKey)
    result.append(sessionId)
    result.append(0x00) // properties absent
    result.append(contentsOf: [UInt8(requestedMtu & 0xFF), UInt8((requestedMtu >> 8) & 0xFF)])
    return result
}

/// Form of the `CLIENT_Representation` (DDS-XRCE 1.0 §7.8.2.1 prose vs Annex A
/// / B.1). `.absent` (default): the Annex A form, 16 octets. `.present`: the
/// prose form with the `Time_t` of the client.
public enum RepresentationTimestamps: Equatable {
    case absent
    case present
}

/// Encoding of `Time_t` (no `@extensibility`, hence APPENDABLE by DDS-XTypes
/// 1.3 7.3.1.2.1.8) in the `CLIENT_Representation`. Same two values as
/// `UnannotatedTypes` of the reliable stream: `.appendable` (default) with a
/// DHEADER, `.final` without.
public enum XrceUnannotatedTypes: Equatable {
    case appendable
    case final
}

private func wallClockTime() -> (seconds: Int32, nanoseconds: UInt32) {
    var ts = timespec()
    clock_gettime(CLOCK_REALTIME, &ts)
    return (Int32(truncatingIfNeeded: ts.tv_sec), UInt32(ts.tv_nsec))
}

/// Spec §7.8.2. `sessionId` is the id this client asks for (it belongs in the
/// `CLIENT_Representation`). The header carries SESSION_ID_NONE_WITH_CLIENT_KEY
/// (0x00, with the key) when it is 0..127 and SESSION_ID_NONE_WITHOUT_CLIENT_KEY
/// (0x80, no key) when it is 128..255 (DDS-XRCE 1.0 §8.3.2.1), `stream=NONE(0x00)`, `sequence_nr=0` —
/// the handshake precedes any session. `representation` is the XCDR2-encoded
/// `CLIENT_Representation` — build it with `encodeClientRepresentation` —
/// passed through byte for byte here.
public func encodeCreateClientMessage(sessionId: UInt8, clientKey: ClientKeyBytes, representation: [UInt8]) -> [UInt8] {
    encodeMessage(
        sessionId: sessionId < 128 ? 0x00 : 0x80, streamId: 0x00, sequenceNr: 0, clientKey: clientKey,
        submessageId: smCreateClient, flags: flagLittleEndian, body: representation)
}

/// Spec §7.8.3. Flags = E | (reuse ? 0x02 : 0) | (replace ? 0x04 : 0). Body =
/// BaseObjectRequest ++ the real, two-level ObjectVariant
/// (`encodeObjectVariantForKind`) — the only representation this port
/// encodes (ByXmlString/InBinary are out of scope, same as
/// `crates/xrce-client`).
public func encodeCreateMessage(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes,
    requestId: RequestIdBytes, objectId: ObjectId, name: String,
    extra: ObjectVariantExtra = ObjectVariantExtra(), reuse: Bool = false, replace: Bool = false
) -> [UInt8] {
    var flags = flagLittleEndian
    if reuse { flags |= 0x02 }
    if replace { flags |= 0x04 }
    var body = encodeBaseObjectRequest(requestId: requestId, objectId: objectId)
    body.append(contentsOf: encodeObjectVariantForKind(objectId: objectId, name: name, extra: extra))
    return encodeMessage(
        sessionId: sessionId, streamId: streamId, sequenceNr: sequenceNr, clientKey: clientKey,
        submessageId: smCreate, flags: flags, body: body)
}

/// Spec §7.8.3. Body = BaseObjectRequest only — no ObjectVariant.
public func encodeDeleteMessage(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes,
    requestId: RequestIdBytes, objectId: ObjectId
) -> [UInt8] {
    let body = encodeBaseObjectRequest(requestId: requestId, objectId: objectId)
    return encodeMessage(
        sessionId: sessionId, streamId: streamId, sequenceNr: sequenceNr, clientKey: clientKey,
        submessageId: smDelete, flags: flagLittleEndian, body: body)
}

/// Spec §7.8.4. Body = BaseObjectRequest ++ serialized_data, verbatim and
/// with no length prefix of its own — `submessage_length` is what bounds the
/// sample.
public func encodeWriteDataMessage(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes,
    requestId: RequestIdBytes, objectId: ObjectId, serializedData: [UInt8]
) -> [UInt8] {
    var body = encodeBaseObjectRequest(requestId: requestId, objectId: objectId)
    body.append(contentsOf: serializedData)
    return encodeMessage(
        sessionId: sessionId, streamId: streamId, sequenceNr: sequenceNr, clientKey: clientKey,
        submessageId: smWriteData, flags: flagLittleEndian, body: body)
}

/// Spec §7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This port
/// never sets a content filter or delivery control, so ReadSpecification is
/// always `preferred_stream_id, data_format=0x00, content_filter_present=0x00,
/// delivery_control_present=0x00`.
public func encodeReadDataMessage(
    sessionId: UInt8, streamId: UInt8, sequenceNr: UInt16, clientKey: ClientKeyBytes,
    requestId: RequestIdBytes, objectId: ObjectId, preferredStreamId: UInt8
) -> [UInt8] {
    var body = encodeBaseObjectRequest(requestId: requestId, objectId: objectId)
    body.append(contentsOf: [preferredStreamId, 0x00, 0x00, 0x00])
    return encodeMessage(
        sessionId: sessionId, streamId: streamId, sequenceNr: sequenceNr, clientKey: clientKey,
        submessageId: smReadData, flags: flagLittleEndian, body: body)
}

// --- Layer 2: the state machine (mirrors crates/xrce-client) ----------------

/// Spec §7.2 lifecycle.
public enum XrceClientState: Equatable {
    /// Initial: the client knows its ClientKey but is not connected.
    case disconnected
    /// CREATE_CLIENT sent, STATUS_AGENT not yet received.
    case connecting
    /// Connected — operations are possible.
    case connected
}

/// Mirrors `crates/xrce-client::ClientError`. `.transport` and `.queueFull`
/// are carried over for parity with the Rust enum even though neither is
/// reachable from this Swift port: `Transport.deliver` has no failure
/// channel at all, and `.queueFull` is reserved/unused even in the Rust
/// reference (its `out_queue` has no capacity cap).
public enum XrceClientError: Error, Equatable, CustomStringConvertible {
    /// Operation in the wrong lifecycle status.
    case invalidState
    /// Unreachable from this Swift port — see above.
    case transport
    /// Unreachable — reserved for parity with Rust.
    case queueFull

    public var description: String {
        switch self {
        case .invalidState: return "XrceClientError.invalidState: not in the required lifecycle state"
        case .transport: return "XrceClientError.transport"
        case .queueFull: return "XrceClientError.queueFull"
        }
    }
}

/// DDS-XRCE 1.0 client: a from-scratch port of `crates/xrce-client` (Rust)
/// plus the wire codecs it delegates to in `crates/xrce`.
///
/// This is additive to `Client`/`AsyncReader` (in `Zerodds.swift`), which
/// speak an older, in-house, non-conformant frame on purpose — this type is
/// the conformant sibling that actually reaches a real DDS-XRCE agent.
public final class XrceClient {
    private let clientKeyVal: ClientKeyBytes
    private var stateVal: XrceClientState
    private let transport: Transport
    private var nextRequestId: UInt16
    private var sessionIdVal: UInt8
    private var streamIdVal: UInt8
    private var nextSeq: UInt16
    /// Form of the `CLIENT_Representation` (DDS-XRCE 1.0 §7.8.2.1 prose vs Annex A / B.1).
    public var representationTimestamps: RepresentationTimestamps

    /// `state = .disconnected`; `nextRequestId` starts at 1; default
    /// `sessionId = 0x01`, `streamId = 0x80` (builtin-reliable) — these
    /// match the fixtures; `nextSeq` starts at 0 (the CREATE_CLIENT
    /// handshake always sends `sequence_nr = 0` on `streamId = NONE` and
    /// does not consume this counter — the first reliable-stream operation
    /// gets sequence_nr 0).
    public init(clientKey: ClientKeyBytes, transport: Transport,
                representationTimestamps: RepresentationTimestamps = .absent) {
        precondition(clientKey.count == clientKeyLen, "XrceClient(clientKey:) takes 4 bytes")
        self.clientKeyVal = clientKey
        self.stateVal = .disconnected
        self.transport = transport
        self.nextRequestId = 1
        self.sessionIdVal = 0x01
        self.streamIdVal = 0x80
        self.nextSeq = 0
        self.representationTimestamps = representationTimestamps
    }

    public var state: XrceClientState { stateVal }
    public var clientKey: ClientKeyBytes { clientKeyVal }

    /// The id for this call, then advance — wraps at 16 bits and skips 0
    /// (mirrors Rust's `next_request_id.wrapping_add(1).max(1)`).
    private func allocRequestId() -> UInt16 {
        let result = nextRequestId
        let next = nextRequestId &+ 1
        nextRequestId = next == 0 ? 1 : next
        return result
    }

    /// Big-endian encoding of the auto-allocated request id — matching
    /// `ObjectId.toBytes()` (also big-endian) so both halves of a
    /// `BaseObjectRequest` use the same byte order. `crates/xrce`'s own
    /// `BaseObjectRequest` treats RequestId as two opaque bytes and leaves
    /// the int-to-bytes choice to the caller — this is that choice (id 1 ->
    /// `[0x00, 0x01]`, matching the same choice the other ports made).
    private static func requestIdBytes(_ id: UInt16) -> RequestIdBytes {
        (UInt8((id >> 8) & 0xFF), UInt8(id & 0xFF))
    }

    private func requireConnected() throws {
        guard stateVal == .connected else { throw XrceClientError.invalidState }
    }

    /// This client's `CLIENT_Representation` in its `representationTimestamps`
    /// form; with `.present` the timestamp is the wall clock at the time of the call.
    public func clientRepresentation(
        version: (UInt8, UInt8), vendorId: (UInt8, UInt8), requestedMtu: UInt16 = defaultMtu
    ) -> [UInt8] {
        encodeClientRepresentation(
            version: version, vendorId: vendorId, clientKey: clientKeyVal, sessionId: sessionIdVal,
            requestedMtu: requestedMtu, representationTimestamps: representationTimestamps)
    }

    /// `connect(representation:)` with the representation built by `clientRepresentation`.
    public func connect(version: (UInt8, UInt8), vendorId: (UInt8, UInt8), requestedMtu: UInt16 = defaultMtu) throws {
        try connect(representation: clientRepresentation(version: version, vendorId: vendorId, requestedMtu: requestedMtu))
    }

    /// Initiates the handshake: encodes and delivers CREATE_CLIENT, then
    /// transitions to `.connecting`.
    ///
    /// - Throws: `XrceClientError.invalidState` unless `state == .disconnected`.
    public func connect(representation: [UInt8]) throws {
        guard stateVal == .disconnected else { throw XrceClientError.invalidState }
        let msg = encodeCreateClientMessage(sessionId: sessionIdVal, clientKey: clientKeyVal, representation: representation)
        transport.deliver(msg)
        stateVal = .connecting
    }

    /// Marks the handshake complete (the caller has received STATUS_AGENT
    /// externally). Pure bookkeeping — no wire I/O, matching the Rust
    /// reference exactly.
    ///
    /// - Throws: `XrceClientError.invalidState` unless `state == .connecting`.
    public func markConnected() throws {
        guard stateVal == .connecting else { throw XrceClientError.invalidState }
        stateVal = .connected
    }

    /// Submits a CREATE operation for an `ObjectVariant::ByReference(name)`.
    /// Returns the allocated `request_id`.
    ///
    /// - Throws: `XrceClientError.invalidState` unless `state == .connected`.
    @discardableResult
    public func createObject(
        objectId: ObjectId, name: String, extra: ObjectVariantExtra = ObjectVariantExtra(),
        reuse: Bool = false, replace: Bool = false
    ) throws -> UInt16 {
        try requireConnected()
        let req = allocRequestId()
        let msg = encodeCreateMessage(
            sessionId: sessionIdVal, streamId: streamIdVal, sequenceNr: nextSeq, clientKey: clientKeyVal,
            requestId: Self.requestIdBytes(req), objectId: objectId, name: name, extra: extra,
            reuse: reuse, replace: replace)
        transport.deliver(msg)
        nextSeq = nextSeq &+ 1
        return req
    }

    /// Submits a DELETE operation. Returns the allocated `request_id`.
    ///
    /// - Throws: `XrceClientError.invalidState` unless connected.
    @discardableResult
    public func deleteObject(objectId: ObjectId) throws -> UInt16 {
        try requireConnected()
        let req = allocRequestId()
        let msg = encodeDeleteMessage(
            sessionId: sessionIdVal, streamId: streamIdVal, sequenceNr: nextSeq, clientKey: clientKeyVal,
            requestId: Self.requestIdBytes(req), objectId: objectId)
        transport.deliver(msg)
        nextSeq = nextSeq &+ 1
        return req
    }

    /// Submits a WRITE_DATA operation. Returns the allocated `request_id`.
    ///
    /// - Throws: `XrceClientError.invalidState` unless connected.
    @discardableResult
    public func requestWrite(writerObjectId: ObjectId, payload: [UInt8]) throws -> UInt16 {
        try requireConnected()
        let req = allocRequestId()
        let msg = encodeWriteDataMessage(
            sessionId: sessionIdVal, streamId: streamIdVal, sequenceNr: nextSeq, clientKey: clientKeyVal,
            requestId: Self.requestIdBytes(req), objectId: writerObjectId, serializedData: payload)
        transport.deliver(msg)
        nextSeq = nextSeq &+ 1
        return req
    }

    /// Submits a READ_DATA operation (pull-based). Returns the allocated
    /// `request_id`. `preferred_stream_id` is the client's own `stream_id`,
    /// `data_format` is always Data (0x00).
    ///
    /// - Throws: `XrceClientError.invalidState` unless connected.
    @discardableResult
    public func requestRead(readerObjectId: ObjectId) throws -> UInt16 {
        try requireConnected()
        let req = allocRequestId()
        let msg = encodeReadDataMessage(
            sessionId: sessionIdVal, streamId: streamIdVal, sequenceNr: nextSeq, clientKey: clientKeyVal,
            requestId: Self.requestIdBytes(req), objectId: readerObjectId, preferredStreamId: streamIdVal)
        transport.deliver(msg)
        nextSeq = nextSeq &+ 1
        return req
    }

    /// Closes the session — goes back to `.disconnected`. That is all: does
    /// NOT reset `nextRequestId`, `sessionId`, `streamId`, or `nextSeq` —
    /// `crates/xrce-client::disconnect` only resets `state`. Matching this
    /// exactly matters, per the reference's own comment: a "helpful" reset of
    /// more than `state` is a divergence, not an improvement.
    public func disconnect() {
        stateVal = .disconnected
    }
}
