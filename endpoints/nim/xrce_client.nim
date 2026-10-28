# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# DDS-XRCE 1.0 client, ported from `crates/xrce-client`. Two layers:
#
#   1. Pure wire encoders (no state, no I/O) for MessageHeader, Submessage-
#      Header, ObjectId, BaseObjectRequest, ObjectVariant::ByReference, and
#      the five submessage bodies this client speaks
#      (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA).
#   2. `XrceClient`, a synchronous state machine mirroring
#      `crates/xrce-client::XrceClient` 1:1 in its lifecycle rules
#      (Disconnected -> Connecting -> Connected, monotonic request ids,
#      `disconnect` resetting only `state`).
#
# This reuses the SDK's existing `Transport` type (see `zerodds.nim`) as-is.
# It is additive: `Client`/`AsyncReader` in `zerodds.nim` speak an older,
# in-house, non-conformant frame on purpose; this file is the conformant
# DDS-XRCE 1.0 sibling that can talk to a real agent.
#
# One difference from the Rust reference is worth flagging up front: in
# `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
# `request_read` take underscore-prefixed parameters and never touch the
# wire — they are pure request-id bookkeeping, with encoding left to a
# caller that does not exist yet. This port closes that gap: every op here
# actually encodes its submessage with the Layer 1 encoders below and calls
# `transport.deliver`, because "reach a real XRCE agent over the network" is
# the entire point of the port. The state-machine *rules* (refusals,
# transitions, id allocation, what `disconnect` resets) are unchanged.

import std/[options, times]
import ./zerodds
from ./reliable import UnannotatedTypes
export UnannotatedTypes

# --- ObjectId (spec Sec 7.2.1) ------------------------------------------------

type ObjectKind* = enum
  ## 4-bit OBJK_* codes (spec Sec 7.2 Table 4). Values are the wire codes
  ## themselves, not sequential — 0x07..0x09 have no spec meaning and are
  ## deliberately absent so no enum value can round-trip an invalid kind.
  okParticipant = 1
  okTopic = 2
  okPublisher = 3
  okSubscriber = 4
  okDatawriter = 5
  okDatareader = 6
  okType = 10
  okQosProfile = 11
  okApplication = 12
  okAgent = 13
  okClient = 14
  okDomain = 15

type ObjectId* = distinct uint16
  ## The packed 16-bit word: `raw_id` (12 bits) in the high bits, `kind`
  ## (4 bits) in the low bits. Spec Sec 7.2.1.

proc packObjectId*(rawId: uint16, kind: ObjectKind): ObjectId =
  ## `word = (raw_id << 4) | kind`.
  ObjectId((rawId shl 4) or uint16(ord(kind)))

proc fromRawWord*(word: uint16): ObjectId =
  ## Constructs directly from an already-packed 16-bit word (e.g. a value
  ## read off the wire, or a well-known singleton like OBJECTID_AGENT).
  ObjectId(word)

proc rawWord*(id: ObjectId): uint16 =
  ## The packed word back out, for callers that need to compare/store it.
  uint16(id)

proc toBytes*(id: ObjectId): array[2, byte] =
  ## Wire encoding: big-endian — the one field in this spec that is *not*
  ## little-endian (everything else in a DDS-XRCE message is LE).
  let w = uint16(id)
  [byte((w shr 8) and 0xFF'u16), byte(w and 0xFF'u16)]

# --- MessageHeader (spec Sec 8.3.2) ------------------------------------------

const ClientKeyLen* = 4
  ## Length of the ClientKey field, spec Sec 7.7.

type ClientKeyBytes* = array[ClientKeyLen, byte]

const SessionCarriesKeyMax* = 0x7F'u8
  ## `session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80`
  ## does not (4-byte header). Reserved values: SESSION_NONE_WITH_KEY =
  ## 0x00, SESSION_NONE_WITHOUT_KEY = 0x80.

proc encodeMessageHeader*(sessionId, streamId: uint8, sequenceNr: uint16,
                           clientKey: ClientKeyBytes): seq[byte] =
  ## `session_id(1) stream_id(1) sequence_nr(2, LE) [client_key(4)]`. Every
  ## operation in this port uses a session_id <= 0x7F, so the key is always
  ## present — callers never construct a keyless header here.
  result = @[sessionId, streamId,
             byte(sequenceNr and 0xFF'u16), byte((sequenceNr shr 8) and 0xFF'u16)]
  if sessionId <= SessionCarriesKeyMax:
    for b in clientKey:
      result.add(b)

# --- SubmessageHeader (spec Sec 8.3.4) ---------------------------------------

const FlagLittleEndian* = 0x01'u8
  ## FLAG_E_LITTLE_ENDIAN, bit 0. Every submessage this client sends sets it
  ## — the whole port only ever writes little-endian bodies.

proc encodeSubmessageHeader*(submessageId, flags: uint8, bodyLen: uint16): seq[byte] =
  ## `submessage_id(1) flags(1) submessage_length(2, always LE)`. The length
  ## field is always little-endian regardless of the body's own endianness
  ## flag.
  @[submessageId, flags, byte(bodyLen and 0xFF'u16), byte((bodyLen shr 8) and 0xFF'u16)]

# Submessage ids used by this client (spec Sec 8.3.4 Table 8).
const SmCreateClient* = 0x00'u8
const SmCreate* = 0x01'u8
const SmDelete* = 0x03'u8
const SmWriteData* = 0x07'u8
const SmReadData* = 0x08'u8

# --- BaseObjectRequest (spec Sec 7.7.8) --------------------------------------

type RequestIdBytes* = array[2, byte]
  ## The wire RequestId is two raw, caller-supplied octets — not a
  ## little-endian encoding of an integer. `crates/xrce`'s own
  ## `BaseObjectRequest::encode` copies them through unchanged.

proc encodeBaseObjectRequest*(requestId: RequestIdBytes, objectId: ObjectId): seq[byte] =
  ## `request_id(2 raw bytes) ++ object_id.to_bytes()(2, big-endian)`.
  result = @[requestId[0], requestId[1]]
  for b in objectId.toBytes():
    result.add(b)

# --- ObjectVariant (spec Sec 7.7.3) ------------------------------------------

proc encodeByReference*(name: string): seq[byte] =
  ## ObjectVariant::ByReference's *inner* payload only (spec Sec 7.7.2):
  ## `discriminator(1)=0x01 ++ 3 padding bytes ++ length(4, LE)=strlen+1 ++
  ## name bytes ++ NUL`. The length includes the NUL terminator XCDR2
  ## strings carry. Correct only when this structure itself starts
  ## 4-aligned — not the wire form a real agent accepts on its own; see
  ## `encodeObjectVariantForKind`, whose 1-byte outer ObjectKind shifts
  ## this padding to two bytes.
  let n = uint32(name.len + 1)
  result = @[0x01'u8, 0x00'u8, 0x00'u8, 0x00'u8,
             byte(n and 0xFF'u32), byte((n shr 8) and 0xFF'u32),
             byte((n shr 16) and 0xFF'u32), byte((n shr 24) and 0xFF'u32)]
  for c in name:
    result.add(byte(c))
  result.add(0x00'u8)

proc encodeObjectVariantForKind*(objectId: ObjectId, name: string,
                                  parentObjectId: Option[ObjectId] = none(ObjectId)): seq[byte] =
  ## The real wire ObjectVariant (spec Sec 7.7.3): a **two-level**
  ## discriminated union — the outer ObjectKind byte (`objectId`'s own low
  ## 4 bits — the outer discriminator and the object being created always
  ## agree, by construction), then the inner REPRESENTATION_BY_REFERENCE
  ## payload (disc 0x01, two padding bytes, then the same XCDR2 string as
  ## `encodeByReference`), then that kind's own trailing field —
  ## `parentObjectId` (an ObjectId, unaligned) for a
  ## Topic/Publisher/Subscriber/Datawriter/Datareader, `none(ObjectId)`
  ## for every other kind — byte-oracled in
  ## `crates/xrce/src/object_repr.rs` against real eProsima Micro-XRCE-DDS
  ## Client 3.0.1 CREATE traffic (Topic's own participant_id: commit
  ## 31c62df2a63). This — not `encodeByReference` alone — is the only
  ## form a real agent accepts.
  let n = uint32(name.len + 1)
  result = @[byte(rawWord(objectId) and 0x0F'u16),
             0x01'u8, 0x00'u8, 0x00'u8,
             byte(n and 0xFF'u32), byte((n shr 8) and 0xFF'u32),
             byte((n shr 16) and 0xFF'u32), byte((n shr 24) and 0xFF'u32)]
  for c in name:
    result.add(byte(c))
  result.add(0x00'u8)
  if parentObjectId.isSome:
    for b in parentObjectId.get().toBytes():
      result.add(b)

# --- Full message encoders ----------------------------------------------------
#
# A message is `header ++ submessage_header ++ body`, concatenated directly:
# this client only ever sends one submessage per message, so the
# between-submessages alignment padding rule never applies here.

proc encodeMessage(sessionId, streamId: uint8, sequenceNr: uint16,
                    clientKey: ClientKeyBytes,
                    submessageId, flags: uint8, body: seq[byte]): seq[byte] =
  result = encodeMessageHeader(sessionId, streamId, sequenceNr, clientKey)
  result.add(encodeSubmessageHeader(submessageId, flags, uint16(body.len)))
  result.add(body)

const DefaultMtu*: uint16 = 508
  ## eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
  ## module doc has the byte-oracle measurement backing it), used as this
  ## port's own CREATE_CLIENT requested_mtu.

type RepresentationTimestamps* {.pure.} = enum
  ## Form of the `CLIENT_Representation` (DDS-XRCE 1.0 Sec 7.8.2.1). The
  ## prose lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL
  ## of Annex A and the bytes of Annex B.1 do not have it. The bytes do not
  ## tell the forms apart, so it is a setting; the agent has to be set the
  ## same way (`representation_timestamps:` in its configuration).
  ## `Absent` (default): the Annex A form. `Present`: the prose form.
  Absent, Present

proc encodeClientRepresentation*(version, vendorId: array[2, byte],
                                  clientKey: ClientKeyBytes, sessionId: uint8,
                                  requestedMtu = DefaultMtu,
                                  timestamps = RepresentationTimestamps.Absent,
                                  seconds: int32 = 0, nanoseconds: uint32 = 0,
                                  unannotated = UnannotatedTypes.Appendable): seq[byte] =
  ## `CLIENT_Representation` (Annex A, spec Sec 7.8.2.1): xrce_cookie(4) ++
  ## xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++
  ## session_id(1) ++ a 0 properties-absent octet ++ requested_mtu(2, LE)
  ## — eProsima's own non-normative MTU extension, always sent by this
  ## client. No PropertySeq support here — this client sends none.
  ##
  ## With `timestamps = RepresentationTimestamps.Present` the `Time_t` of
  ## the client (`seconds` i32, `nanoseconds` u32, little-endian) follows
  ## `xrce_vendor_id`. `Time_t` has no `@extensibility`, so it is APPENDABLE
  ## (DDS-XTypes 1.3 7.3.1.2.1.8): a DHEADER u32 = 8 in front (28 octets in
  ## all); `unannotated = UnannotatedTypes.Final` leaves it out (24 octets).
  result = @[byte('X'), byte('R'), byte('C'), byte('E')]
  result.add(version[0]); result.add(version[1])
  result.add(vendorId[0]); result.add(vendorId[1])
  if timestamps == RepresentationTimestamps.Present:
    if unannotated == UnannotatedTypes.Appendable:
      result.add([8'u8, 0'u8, 0'u8, 0'u8])
    let sec = cast[uint32](seconds)
    for i in 0 ..< 4:
      result.add(byte((sec shr (8 * i)) and 0xFF'u32))
    for i in 0 ..< 4:
      result.add(byte((nanoseconds shr (8 * i)) and 0xFF'u32))
  for b in clientKey:
    result.add(b)
  result.add(sessionId)
  result.add(0x00'u8) # properties absent
  result.add(byte(requestedMtu and 0xFF'u16))
  result.add(byte((requestedMtu shr 8) and 0xFF'u16))

proc encodeCreateClientMessage*(sessionId: uint8, clientKey: ClientKeyBytes,
                                 representation: seq[byte]): seq[byte] =
  ## Spec Sec 7.8.2. `sessionId` is the id this client asks for (it belongs
  ## in the `CLIENT_Representation`). The header carries
  ## SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with the key) when it is
  ## 0..127 and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80, no key) when it
  ## is 128..255 (DDS-XRCE 1.0 Sec 8.3.2.1), stream=NONE(0x00),
  ## sequence_nr=0 — the handshake precedes any session. `representation` is the
  ## XCDR2-encoded `CLIENT_Representation` — build it with
  ## `encodeClientRepresentation` — passed through byte for byte here.
  let headerSessionId = if sessionId < 128: 0x00'u8 else: 0x80'u8
  encodeMessage(headerSessionId, 0x00'u8, 0'u16, clientKey, SmCreateClient, FlagLittleEndian, representation)

proc encodeCreateMessage*(sessionId, streamId: uint8, sequenceNr: uint16,
                           clientKey: ClientKeyBytes, requestId: RequestIdBytes,
                           objectId: ObjectId, name: string,
                           parentObjectId = none(ObjectId),
                           reuse = false, replace = false): seq[byte] =
  ## Spec Sec 7.8.3. Flags = E | (reuse ? 0x02 : 0) | (replace ? 0x04 : 0).
  ## Body = BaseObjectRequest ++ the real, two-level ObjectVariant
  ## (`encodeObjectVariantForKind`) — the only representation this port
  ## encodes (ByXmlString/InBinary are out of scope, same as
  ## `crates/xrce-client`).
  var flags = FlagLittleEndian
  if reuse: flags = flags or 0x02'u8
  if replace: flags = flags or 0x04'u8
  var body = encodeBaseObjectRequest(requestId, objectId)
  body.add(encodeObjectVariantForKind(objectId, name, parentObjectId))
  encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmCreate, flags, body)

proc encodeDeleteMessage*(sessionId, streamId: uint8, sequenceNr: uint16,
                           clientKey: ClientKeyBytes, requestId: RequestIdBytes,
                           objectId: ObjectId): seq[byte] =
  ## Spec Sec 7.8.3. Body = BaseObjectRequest only — no ObjectVariant.
  let body = encodeBaseObjectRequest(requestId, objectId)
  encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmDelete, FlagLittleEndian, body)

proc encodeWriteDataMessage*(sessionId, streamId: uint8, sequenceNr: uint16,
                              clientKey: ClientKeyBytes, requestId: RequestIdBytes,
                              objectId: ObjectId, serializedData: seq[byte]): seq[byte] =
  ## Spec Sec 7.8.4. Body = BaseObjectRequest ++ serialized_data, verbatim
  ## and with no length prefix of its own — `submessage_length` is what
  ## bounds the sample.
  var body = encodeBaseObjectRequest(requestId, objectId)
  body.add(serializedData)
  encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmWriteData, FlagLittleEndian, body)

proc encodeReadDataMessage*(sessionId, streamId: uint8, sequenceNr: uint16,
                             clientKey: ClientKeyBytes, requestId: RequestIdBytes,
                             objectId: ObjectId, preferredStreamId: uint8): seq[byte] =
  ## Spec Sec 7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This
  ## port never sets a content filter or delivery control, so
  ## ReadSpecification is always `preferred_stream_id, data_format=0x00,
  ## content_filter_present=0x00, delivery_control_present=0x00`.
  var body = encodeBaseObjectRequest(requestId, objectId)
  body.add(@[preferredStreamId, 0x00'u8, 0x00'u8, 0x00'u8])
  encodeMessage(sessionId, streamId, sequenceNr, clientKey, SmReadData, FlagLittleEndian, body)

# --- Layer 2: the state machine (mirrors crates/xrce-client) ----------------

type ClientState* = enum
  ## Spec Sec 7.2 lifecycle.
  csDisconnected ## Initial: the client knows its ClientKey but is not
                 ## connected.
  csConnecting   ## CREATE_CLIENT sent, STATUS_AGENT not yet received.
  csConnected    ## Connected — operations are possible.

type ClientErrorKind* = enum
  ## Mirrors `crates/xrce-client::ClientError`. `Transport` and `QueueFull`
  ## are carried over for parity with the Rust enum even though neither is
  ## reachable from this Nim port: `Transport.deliver` has no failure
  ## channel at all (unlike e.g. Zig's, which reports failure via a `bool`
  ## return), and `QueueFull` is reserved/unused even in the Rust reference
  ## (its `out_queue` has no capacity cap).
  cekInvalidState ## Operation in the wrong lifecycle status.
  cekTransport    ## Unreachable from this Nim port — see above.
  cekQueueFull    ## Unreachable — reserved for parity with Rust.

type XrceClientError* = object of CatchableError
  ## Raised in place of Rust's `Result<_, ClientError>` — this SDK signals
  ## refusals via exceptions throughout (see `WireError` in `zerodds.nim`),
  ## not a Result type, so the state machine follows the same idiom here.
  kind*: ClientErrorKind

proc refuseState(msg: string) {.noreturn.} =
  var e = newException(XrceClientError, msg)
  e.kind = cekInvalidState
  raise e

type XrceClient* = ref object
  clientKeyVal: ClientKeyBytes
  stateVal: ClientState
  transport: Transport
  nextRequestId: uint16
  sessionIdVal: uint8
  streamIdVal: uint8
  nextSeq: uint16
  timestampsVal: RepresentationTimestamps

proc newXrceClient*(clientKey: ClientKeyBytes, transport: Transport,
                    representationTimestamps = RepresentationTimestamps.Absent): XrceClient =
  ## `state = Disconnected`; `next_request_id` starts at 1; default
  ## `session_id = 0x01`, `stream_id = 0x80` (builtin-reliable — every
  ## post-handshake operation is reliable) — these match the fixtures;
  ## `next_seq` starts at 0 (the CREATE_CLIENT handshake always sends
  ## `sequence_nr = 0` on `stream_id = NONE` and does not consume this
  ## counter; the first reliable-stream operation gets sequence_nr 0).
  ## `representationTimestamps` (default `Absent`) is the form of the
  ## `CLIENT_Representation` that `clientRepresentation` / `connect` build.
  XrceClient(clientKeyVal: clientKey, stateVal: csDisconnected, transport: transport,
             nextRequestId: 1'u16, sessionIdVal: 0x01'u8, streamIdVal: 0x80'u8, nextSeq: 0'u16,
             timestampsVal: representationTimestamps)

proc state*(c: XrceClient): ClientState = c.stateVal

proc clientKey*(c: XrceClient): ClientKeyBytes = c.clientKeyVal

proc representationTimestamps*(c: XrceClient): RepresentationTimestamps = c.timestampsVal

proc setRepresentationTimestamps*(c: XrceClient, form: RepresentationTimestamps) =
  ## Selects the form of the `CLIENT_Representation` (DDS-XRCE 1.0 7.8.2.1
  ## prose vs Annex A / B.1). The agent has to be set the same way.
  c.timestampsVal = form

proc clientRepresentation*(c: XrceClient, version, vendorId: array[2, byte],
                           requestedMtu = DefaultMtu): seq[byte] =
  ## The `CLIENT_Representation` of this client in its
  ## `representationTimestamps` form; with `Present` the timestamp is the
  ## wall clock at the time of the call.
  let t = getTime()
  encodeClientRepresentation(version, vendorId, c.clientKeyVal, c.sessionIdVal,
                             requestedMtu, c.timestampsVal,
                             int32(t.toUnix()), uint32(t.nanosecond))

proc allocRequestId(c: XrceClient): uint16 =
  ## The id for this call, then advance — wraps at 16 bits and skips 0
  ## (mirrors Rust's `next_request_id.wrapping_add(1).max(1)`). `uint16`
  ## arithmetic in Nim wraps modulo 2^16 already, matching `wrapping_add`.
  result = c.nextRequestId
  var next = c.nextRequestId + 1'u16
  if next == 0'u16:
    next = 1'u16
  c.nextRequestId = next

proc requestIdBytes(id: uint16): RequestIdBytes =
  ## Big-endian encoding of the auto-allocated request id — matching
  ## `ObjectId.toBytes()` (also big-endian) so both halves of a
  ## `BaseObjectRequest` use the same byte order. `crates/xrce`'s
  ## `BaseObjectRequest` itself treats RequestId as two opaque bytes (see
  ## `RequestIdBytes` above) and leaves the int-to-bytes choice to the
  ## caller — this is that choice (id 1 -> `[0x00, 0x01]`, matching the
  ## same choice the Lua and Zig ports made).
  [byte((id shr 8) and 0xFF'u16), byte(id and 0xFF'u16)]

proc connect*(c: XrceClient, representation: seq[byte]) =
  ## Initiates the handshake: encodes and delivers CREATE_CLIENT, then
  ## transitions to Connecting.
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless `state ==
  ## Disconnected`.
  if c.stateVal != csDisconnected:
    refuseState("connect: already connecting or connected")
  let msg = encodeCreateClientMessage(c.sessionIdVal, c.clientKeyVal, representation)
  c.transport.deliver(msg)
  c.stateVal = csConnecting

proc connect*(c: XrceClient, version, vendorId: array[2, byte],
              requestedMtu = DefaultMtu) =
  ## `connect` with the representation built by `clientRepresentation`
  ## (the client's `representationTimestamps` form, wall clock).
  c.connect(c.clientRepresentation(version, vendorId, requestedMtu))

proc markConnected*(c: XrceClient) =
  ## Marks the handshake complete (the caller has received STATUS_AGENT
  ## externally). Pure bookkeeping — no wire I/O, matching the Rust
  ## reference exactly.
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless `state ==
  ## Connecting`.
  if c.stateVal != csConnecting:
    refuseState("markConnected: not connecting")
  c.stateVal = csConnected

proc requireConnected(c: XrceClient) =
  if c.stateVal != csConnected:
    refuseState("operation requires state == Connected")

proc createObject*(c: XrceClient, objectId: ObjectId, name: string,
                    parentObjectId = none(ObjectId),
                    reuse = false, replace = false): uint16 =
  ## Submits a CREATE operation for an `ObjectVariant::ByReference(name)`,
  ## under an optional parent object (that kind's own trailing field,
  ## spec Sec 7.7.3). Returns the allocated `request_id`.
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless `state ==
  ## Connected`.
  c.requireConnected()
  let req = c.allocRequestId()
  let msg = encodeCreateMessage(c.sessionIdVal, c.streamIdVal, c.nextSeq, c.clientKeyVal,
                                 requestIdBytes(req), objectId, name, parentObjectId, reuse, replace)
  c.transport.deliver(msg)
  c.nextSeq = c.nextSeq + 1'u16
  req

proc deleteObject*(c: XrceClient, objectId: ObjectId): uint16 =
  ## Submits a DELETE operation. Returns the allocated `request_id`.
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless connected.
  c.requireConnected()
  let req = c.allocRequestId()
  let msg = encodeDeleteMessage(c.sessionIdVal, c.streamIdVal, c.nextSeq, c.clientKeyVal,
                                 requestIdBytes(req), objectId)
  c.transport.deliver(msg)
  c.nextSeq = c.nextSeq + 1'u16
  req

proc requestWrite*(c: XrceClient, writerObjectId: ObjectId, payload: seq[byte]): uint16 =
  ## Submits a WRITE_DATA operation. Returns the allocated `request_id`.
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless connected.
  c.requireConnected()
  let req = c.allocRequestId()
  let msg = encodeWriteDataMessage(c.sessionIdVal, c.streamIdVal, c.nextSeq, c.clientKeyVal,
                                    requestIdBytes(req), writerObjectId, payload)
  c.transport.deliver(msg)
  c.nextSeq = c.nextSeq + 1'u16
  req

proc requestRead*(c: XrceClient, readerObjectId: ObjectId): uint16 =
  ## Submits a READ_DATA operation (pull-based). Returns the allocated
  ## `request_id`. `preferred_stream_id` is the client's own `stream_id`,
  ## `data_format` is always Data (0x00).
  ##
  ## Raises `XrceClientError` (`cekInvalidState`) unless connected.
  c.requireConnected()
  let req = c.allocRequestId()
  let msg = encodeReadDataMessage(c.sessionIdVal, c.streamIdVal, c.nextSeq, c.clientKeyVal,
                                   requestIdBytes(req), readerObjectId, c.streamIdVal)
  c.transport.deliver(msg)
  c.nextSeq = c.nextSeq + 1'u16
  req

proc disconnect*(c: XrceClient) =
  ## Closes the session — goes back to Disconnected. That is all: does NOT
  ## reset `next_request_id`, `session_id`, `stream_id`, or `next_seq` —
  ## `crates/xrce-client::disconnect` only resets `state` (and clears an
  ## internal out-queue this port has no equivalent of). Matching this
  ## exactly matters, per the reference's own comment: a "helpful" reset of
  ## more than `state` is a divergence, not an improvement.
  c.stateVal = csDisconnected
