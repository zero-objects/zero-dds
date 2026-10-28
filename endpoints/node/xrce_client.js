// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 client state machine + wire codec. A port of
// `crates/xrce-client/src/lib.rs`'s `XrceClient<T>` state machine (340
// lines) combined with the concrete submessage codecs of `crates/xrce`
// (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). `crates/xrce-client`
// itself never touches the wire — its module doc says so directly: "the
// caller delegates submessage encoding; we only track state". This module
// is that combination.
//
// Distinct from `zerodds.js`'s WRITE_DATA framing used by `Client`/
// `AsyncReader`/`reliable.js` (the pre-existing simplified data-path
// framing: sample body only, no BaseObjectRequest) — `XrceClient` here
// speaks the spec-conformant object-lifecycle frames a real XRCE agent
// expects for CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the conformant
// WRITE_DATA that carries the target ObjectId (DDS-XRCE 1.0 Annex B.3.1).
// It plugs into the same `{ deliver, receive }` transport contract — it is
// the caller's job to hand the frames this module builds to that
// transport's `deliver()`, no new transport is introduced here.
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
//   new XrceClient  -> §7.8.2 CREATE_CLIENT
//   connect         -> §8.4.5 (handshake)
//   createObject    -> §7.8.3 CREATE
//   deleteObject    -> §7.8.3 DELETE
//   requestWrite    -> §7.8.4 WRITE_DATA
//   requestRead     -> §7.8.5 READ_DATA
//   disconnect      -> §8.4.5

'use strict';

// --- ObjectKind / ObjectId (spec §7.2, §7.2.1) -----------------------------

const OBJK_INVALID = 0x00;
const OBJK_PARTICIPANT = 0x01;
const OBJK_TOPIC = 0x02;
const OBJK_PUBLISHER = 0x03;
const OBJK_SUBSCRIBER = 0x04;
const OBJK_DATAWRITER = 0x05;
const OBJK_DATAREADER = 0x06;
const OBJK_TYPE = 0x0a;
const OBJK_QOSPROFILE = 0x0b;
const OBJK_APPLICATION = 0x0c;
const OBJK_AGENT = 0x0d;
const OBJK_CLIENT = 0x0e;
const OBJK_OTHER = 0x0f;

const OBJECTID_INVALID = 0x0000; // DDS-XRCE 1.0 §7.6: {0x00, 0x00}

// Packs a 12-bit rawId and a 4-bit kind into a 16-bit ObjectId.
function newObjectId(rawId, kind) {
  if (rawId > 0x0fff) {
    throw new RangeError(`newObjectId: raw id ${rawId} exceeds 12 bits`);
  }
  return ((rawId << 4) | (kind & 0x0f)) >>> 0;
}

function objectIdKind(objectId) {
  return objectId & 0x0f;
}

function objectIdRaw12(objectId) {
  return (objectId >> 4) & 0x0fff;
}

// Wire encoding: 2 octets, big-endian (crates/xrce `ObjectId::to_bytes`).
function objectIdBytes(objectId) {
  return Buffer.from([(objectId >> 8) & 0xff, objectId & 0xff]);
}

function objectIdFromBytes(b) {
  if (b.length < 2) {
    throw new RangeError('objectIdFromBytes: need 2 bytes');
  }
  return ((b[0] << 8) | b[1]) >>> 0;
}

// --- ObjectVariant (spec §7.7) ---------------------------------------------

const VARIANT_BY_REFERENCE = 0x01;
const VARIANT_BY_XML_STRING = 0x02;
const VARIANT_IN_BINARY = 0x03;

// Object-specific CREATE payload. Build via byReference/byXmlString/
// inBinary — do not construct directly.
class ObjectVariant {
  constructor(kind, strPayload, binPayload) {
    this.kind = kind;
    this.strPayload = strPayload;
    this.binPayload = binPayload;
  }

  // disc(1) + `padLen` zero padding bytes + u32 length (little endian) +
  // payload, with a trailing NUL for the two string variants (XCDR2
  // `string<>`, crates/xrce/src/object_repr.rs). `padLen` is 3 for the
  // never-used-on-the-wire single-level form (this structure alone starts
  // 4-aligned) and 2 for `encodeForKind` (a 1-byte ObjectKind precedes it
  // there).
  _encodeInner(padLen) {
    let payload;
    let extraNul = false;
    if (this.kind === VARIANT_BY_REFERENCE || this.kind === VARIANT_BY_XML_STRING) {
      payload = Buffer.from(this.strPayload, 'utf8');
      extraNul = true;
    } else {
      payload = Buffer.from(this.binPayload);
    }
    const n = payload.length + (extraNul ? 1 : 0);
    const out = Buffer.alloc(1 + padLen + 4 + payload.length + (extraNul ? 1 : 0));
    out[0] = this.kind;
    out.writeUInt32LE(n, 1 + padLen);
    payload.copy(out, 1 + padLen + 4);
    // out[1..1+padLen] already zero (padding); the trailing NUL byte, when
    // extraNul, is already zero too.
    return out;
  }

  // The real wire `ObjectVariant` (spec §7.7.3): a **two-level**
  // discriminated union — the outer `ObjectKind` byte (`objectKind`, e.g.
  // OBJK_DATAWRITER) selects which `..._Representation` struct follows,
  // then this inner format-discriminated payload (`_encodeInner`), then
  // that struct's own trailing field: `domainId` (`short`, 2-byte
  // aligned) for OBJK_PARTICIPANT, `parentObjectId` (`ObjectId`,
  // unaligned) for OBJK_TOPIC/OBJK_PUBLISHER/OBJK_SUBSCRIBER/
  // OBJK_DATAWRITER/OBJK_DATAREADER, or nothing for every other kind —
  // byte-oracled in crates/xrce/src/object_repr.rs against real eProsima
  // Micro-XRCE-DDS Client 3.0.1 CREATE traffic (OBJK_TOPIC's own
  // participant_id: commit 31c62df2a63). This — not the bare
  // `_encodeInner` — is the only form a real agent accepts.
  encodeForKind(objectKind, { domainId, parentObjectId } = {}) {
    const hasDomainId = objectKind === OBJK_PARTICIPANT;
    const hasParent = [OBJK_TOPIC, OBJK_PUBLISHER, OBJK_SUBSCRIBER, OBJK_DATAWRITER, OBJK_DATAREADER].includes(
      objectKind,
    );
    if (hasDomainId && domainId == null) {
      throw new RangeError('encodeForKind: OBJK_PARTICIPANT representation requires domainId');
    }
    if (!hasDomainId && domainId != null) {
      throw new RangeError('encodeForKind: only OBJK_PARTICIPANT representation carries domainId');
    }
    if (hasParent && parentObjectId == null) {
      throw new RangeError("encodeForKind: this ObjectKind's representation requires a parent id");
    }
    if (!hasParent && parentObjectId != null) {
      throw new RangeError("encodeForKind: this ObjectKind's representation carries no parent id");
    }

    const parts = [Buffer.from([objectKind]), this._encodeInner(2)];
    if (domainId != null) {
      let len = parts.reduce((n, p) => n + p.length, 0);
      if (len % 2 !== 0) {
        parts.push(Buffer.from([0])); // `short` aligns to 2 bytes; ObjectId (below) does not
        len += 1;
      }
      const d = Buffer.alloc(2);
      d.writeUInt16LE(domainId & 0xffff, 0);
      parts.push(d);
    }
    if (parentObjectId != null) {
      parts.push(objectIdBytes(parentObjectId));
    }
    return Buffer.concat(parts);
  }
}

// REPRESENTATION_BY_REFERENCE: a name pointing to a template registered in
// the agent.
function byReference(name) {
  return new ObjectVariant(VARIANT_BY_REFERENCE, name);
}

// REPRESENTATION_AS_XML_STRING: inline XML describing the object.
function byXmlString(xml) {
  return new ObjectVariant(VARIANT_BY_XML_STRING, xml);
}

// REPRESENTATION_IN_BINARY: inline XCDR2-encoded bytes.
function inBinary(data) {
  return new ObjectVariant(VARIANT_IN_BINARY, undefined, data);
}

// --- BaseObjectRequest (spec §7.7.8) ---------------------------------------

// Packs the state machine's u16 request id into the wire RequestId(2
// octets) field, most significant octet first.
//
// DDS-XRCE 1.0 gives RequestId as octet[2] and never as an integer, so the
// wire has no byte order — but this client hands out a uint16 counter, and
// the mapping from one to the other went unstated until three ports had
// already chosen one way and three the other. crates/xrce-client's
// `request_id_octets` settles it as big-endian, which is what the fixtures
// under endpoints/testdata/xrce-client hold: [0x00, 0x01] for the first
// request.
function requestIdBytes(requestId) {
  return Buffer.from([(requestId >> 8) & 0xff, requestId & 0xff]);
}

// `request_id (2 octets) + object_id (2 octets)` — the header every
// CREATE/DELETE/WRITE_DATA/READ_DATA body starts with.
function encodeBaseObjectRequest(reqIdBytes, objectId) {
  return Buffer.concat([reqIdBytes, objectIdBytes(objectId)]);
}

// --- DataFormat / ReadSpecification / DataDeliveryControl (spec §8.3.5.9) --

const DATA_FORMAT_DATA = 0x00; // FORMAT_DATA (0b000) — a bare sample, no SampleInfo.

// Paces a READ_DATA reply: four unsigned shorts, 8 octets little-endian.
class DataDeliveryControl {
  constructor(maxSamples, maxElapsedTime, maxBytesPerSecond, minPacePeriod) {
    this.maxSamples = maxSamples;
    this.maxElapsedTime = maxElapsedTime;
    this.maxBytesPerSecond = maxBytesPerSecond;
    this.minPacePeriod = minPacePeriod;
  }

  encode() {
    const out = Buffer.alloc(8);
    out.writeUInt16LE(this.maxSamples, 0);
    out.writeUInt16LE(this.maxElapsedTime, 2);
    out.writeUInt16LE(this.maxBytesPerSecond, 4);
    out.writeUInt16LE(this.minPacePeriod, 6);
    return out;
  }
}

// How the client wants READ_DATA's samples delivered. The two `@optional`
// members use the conservative presence-flag encoding
// crates/xrce/src/submessages/read_data.rs chose (1 = present / 0 = absent
// ahead of the payload), not the full XCDR2 EMHEADER form — Phase 2 there
// too.
class ReadSpecification {
  constructor(preferredStreamId, dataFormat, contentFilterExpression, deliveryControl) {
    this.preferredStreamId = preferredStreamId;
    this.dataFormat = dataFormat === undefined ? DATA_FORMAT_DATA : dataFormat;
    this.contentFilterExpression = contentFilterExpression || null;
    this.deliveryControl = deliveryControl || null;
  }

  encode() {
    const parts = [Buffer.from([this.preferredStreamId & 0xff, this.dataFormat & 0xff])];
    if (this.contentFilterExpression !== null) {
      const s = Buffer.from(this.contentFilterExpression, 'utf8');
      const lenBuf = Buffer.alloc(4);
      lenBuf.writeUInt32LE(s.length, 0);
      parts.push(Buffer.from([1]), lenBuf, s);
    } else {
      parts.push(Buffer.from([0]));
    }
    if (this.deliveryControl !== null) {
      parts.push(Buffer.from([1]), this.deliveryControl.encode());
    } else {
      parts.push(Buffer.from([0]));
    }
    return Buffer.concat(parts);
  }
}

// --- MessageHeader (spec §8.3.2) -------------------------------------------

const SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00;
const SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80;

const STREAM_ID_NONE = 0x00;
const STREAM_ID_BEST_EFFORT = 0x01;
const STREAM_ID_BUILTIN_RELIABLE = 0x80;

const CLIENT_KEY_LEN = 4;
const CLIENTKEY_INVALID = Buffer.alloc(CLIENT_KEY_LEN); // spec §7.8.2.2

// session_id(1) + stream_id(1) + sequence_nr(2, LE) + an optional 4-octet
// ClientKey — present exactly when session<=127 (spec §8.3.2.1). Throws if
// `clientKey`'s presence disagrees, mirroring
// `MessageHeader::with_client_key`/`without_client_key` refusing the same
// mismatch.
function encodeMessageHeader(session, stream, seq, clientKey) {
  const carriesKey = session <= 127;
  if (carriesKey !== (clientKey != null)) {
    throw new RangeError(
      `encodeMessageHeader: session_id 0x${session.toString(16)} requires clientKey=${carriesKey}`,
    );
  }
  const out = Buffer.alloc(carriesKey ? 4 + CLIENT_KEY_LEN : 4);
  out[0] = session;
  out[1] = stream;
  out.writeUInt16LE(seq, 2);
  if (clientKey != null) {
    Buffer.from(clientKey).copy(out, 4);
  }
  return out;
}

// --- submessage ids (spec §8.3.5, values 0..15) ----------------------------

const SM_CREATE_CLIENT = 0x00;
const SM_CREATE = 0x01;
const SM_DELETE = 0x03;
const SM_WRITE_DATA = 0x07;
const SM_READ_DATA = 0x08;

const FLAG_E_LE = 0x01; // E-flag: the submessage body is little-endian (spec §8.3.4).

// One full XRCE Message: MessageHeader + one SubmessageHeader (id, flags,
// little-endian u16 length) + body. No padding is needed — a single
// submessage is always the last one in its message (spec §8.3.3 pads only
// *between* submessages).
function buildMessage(session, stream, seq, clientKey, smId, flags, body) {
  const header = encodeMessageHeader(session, stream, seq, clientKey);
  if (body.length > 0xffff) {
    throw new RangeError('buildMessage: submessage body exceeds the 16-bit length field');
  }
  const smHeader = Buffer.from([smId, flags, body.length & 0xff, (body.length >> 8) & 0xff]);
  return Buffer.concat([header, smHeader, body]);
}

const XRCE_COOKIE = Buffer.from('XRCE', 'ascii');
const XRCE_VERSION = Buffer.from([0x01, 0x00]);
const DEFAULT_MTU = 508; // eProsima's own hard datagram ceiling; see the doc below.

// `CLIENT_Representation` (Annex A, §7.8.2.1): xrce_cookie(4) +
// xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1) + a
// `0` properties-absent octet + `requestedMtu` (2, little endian) —
// eProsima's own non-normative MTU extension
// (crates/xrce/src/handshake.rs's module doc has the byte-oracle
// measurement backing `DEFAULT_MTU = 508`), always sent by this client. No
// `PropertySeq` support here — this client sends none.
//
// Option `representationTimestamps` ('absent' default, or 'present'):
// 'absent' is the Annex A / B.1 form. 'present' is the prose form of
// §7.8.2.1: after xrce_vendor_id the `Time_t client_timestamp` (`seconds`
// i32, `nanoseconds` u32, little endian). Time_t is APPENDABLE (DDS-XTypes
// 1.3 §7.3.1.2.1.8) with `unannotatedTypes: 'appendable'` (default): DHEADER
// = 8 in front, 12 octets; with 'final' no DHEADER, 8 octets. The agent has
// to be set the same way (`representation_timestamps:` in its configuration).
function encodeClientRepresentation(version, vendorId, clientKey, sessionId, requestedMtu = DEFAULT_MTU, options = {}) {
  const { representationTimestamps = 'absent', seconds = 0, nanoseconds = 0, unannotatedTypes = 'appendable' } = options;
  if (representationTimestamps !== 'absent' && representationTimestamps !== 'present') {
    throw new RangeError(`representationTimestamps must be 'absent' or 'present', got ${String(representationTimestamps)}`);
  }
  if (unannotatedTypes !== 'appendable' && unannotatedTypes !== 'final') {
    throw new RangeError(`unannotatedTypes must be 'appendable' or 'final', got ${String(unannotatedTypes)}`);
  }
  const mtu = Buffer.alloc(2);
  mtu.writeUInt16LE(requestedMtu, 0);
  const parts = [XRCE_COOKIE, Buffer.from(version), Buffer.from(vendorId)];
  if (representationTimestamps === 'present') {
    const timeT = Buffer.alloc(unannotatedTypes === 'appendable' ? 12 : 8);
    let off = 0;
    if (unannotatedTypes === 'appendable') {
      timeT.writeUInt32LE(8, 0); // DHEADER of Time_t
      off = 4;
    }
    timeT.writeInt32LE(seconds | 0, off);
    timeT.writeUInt32LE(nanoseconds >>> 0, off + 4);
    parts.push(timeT);
  }
  parts.push(
    Buffer.from(clientKey),
    Buffer.from([sessionId & 0xff, 0]), // session_id, then properties-absent
    mtu,
  );
  return Buffer.concat(parts);
}

// CREATE_CLIENT (§7.8.2/§8.3.5.1): the client -> agent handshake
// `XrceClient.connect()` starts. `representation` is the XCDR2-encoded
// CLIENT_Representation (build it with `encodeClientRepresentation`) — this
// function only frames it, without interpreting it. `session` is the header's
// session_id: SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with `clientKey`) when the
// representation asks for a sessionId of 0..127, SESSION_ID_NONE_WITHOUT_CLIENT_KEY
// (0x80, `clientKey` null) when it asks for 128..255 (DDS-XRCE 1.0 §8.3.2.1).
function buildCreateClientMessage(session, stream, seq, clientKey, representation) {
  return buildMessage(session, stream, seq, clientKey, SM_CREATE_CLIENT, FLAG_E_LE, Buffer.from(representation));
}

// CREATE (§7.8.3/§8.3.5.2) for the request id `XrceClient.createObject`
// returned. `objectKind` is the outer `ObjectKind` byte the wire
// `ObjectVariant` (§7.7.3) always carries alongside the inner `variant`;
// `domainId`/`parentObjectId` are that kind's own trailing field — see
// `ObjectVariant.encodeForKind`. `reuse`/`replace` set CreationMode bits
// 1/2 of the submessage flags.
function buildCreateMessage(
  session,
  stream,
  seq,
  clientKey,
  requestId,
  objectId,
  objectKind,
  variant,
  { reuse = false, replace = false, domainId, parentObjectId } = {},
) {
  const base = encodeBaseObjectRequest(requestIdBytes(requestId), objectId);
  const body = Buffer.concat([base, variant.encodeForKind(objectKind, { domainId, parentObjectId })]);
  let flags = FLAG_E_LE;
  if (reuse) flags |= 0x02;
  if (replace) flags |= 0x04;
  return buildMessage(session, stream, seq, clientKey, SM_CREATE, flags, body);
}

// DELETE (§7.8.3/§8.3.5.4) for the request id `XrceClient.deleteObject`
// returned.
function buildDeleteMessage(session, stream, seq, clientKey, requestId, objectId) {
  const body = encodeBaseObjectRequest(requestIdBytes(requestId), objectId);
  return buildMessage(session, stream, seq, clientKey, SM_DELETE, FLAG_E_LE, body);
}

// WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA) for the request id
// `XrceClient.requestWrite` returned: BaseObjectRequest (4 octets) then
// `sample` verbatim — no length of its own (DDS-XRCE 1.0 Annex B.3.1;
// crates/xrce/src/submessages/write_data.rs).
function buildWriteDataMessage(session, stream, seq, clientKey, requestId, writer, sample) {
  const base = encodeBaseObjectRequest(requestIdBytes(requestId), writer);
  const body = Buffer.concat([base, Buffer.from(sample)]);
  return buildMessage(session, stream, seq, clientKey, SM_WRITE_DATA, FLAG_E_LE, body);
}

// READ_DATA (§7.8.5/§8.3.5.9) for the request id `XrceClient.requestRead`
// returned.
function buildReadDataMessage(session, stream, seq, clientKey, requestId, reader, spec) {
  const base = encodeBaseObjectRequest(requestIdBytes(requestId), reader);
  const body = Buffer.concat([base, spec.encode()]);
  return buildMessage(session, stream, seq, clientKey, SM_READ_DATA, FLAG_E_LE, body);
}

// --- XrceClient state machine (port of crates/xrce-client's XrceClient<T>) -

const DISCONNECTED = 'Disconnected';
const CONNECTING = 'Connecting';
const CONNECTED = 'Connected';

// Mirrors `ClientError::InvalidState`: an operation was attempted in the
// wrong lifecycle state (e.g. createObject before connect).
class ClientError extends Error {}

// Synchronous XRCE client state machine — a port of crates/xrce-client's
// `XrceClient<T>`. It does not hold a transport and does not send anything
// itself, exactly like the Rust crate: pair its request ids with the
// build*Message functions above and hand the result to your own
// transport's `deliver()`. That split is deliberate: it is what lets this
// module be tested against the same fixtures a Rust caller combining
// crates/xrce-client with crates/xrce would produce.
class XrceClient {
  // options: { representationTimestamps: 'absent' | 'present', unannotatedTypes:
  // 'appendable' | 'final' } for clientRepresentation() (defaults 'absent',
  // 'appendable').
  constructor(clientKey, options = {}) {
    this.clientKey = Buffer.from(clientKey);
    this.state = DISCONNECTED;
    this._nextRequestId = 1;
    this.representationTimestamps = options.representationTimestamps || 'absent';
    this.unannotatedTypes = options.unannotatedTypes || 'appendable';
    // validates both values
    encodeClientRepresentation(XRCE_VERSION, [0, 0], this.clientKey, 1, DEFAULT_MTU, {
      representationTimestamps: this.representationTimestamps,
      unannotatedTypes: this.unannotatedTypes,
    });
  }

  // The CLIENT_Representation of this client for buildCreateClientMessage,
  // with its representationTimestamps option. The Time_t is the wall clock at
  // the time of the call.
  clientRepresentation(version = XRCE_VERSION, vendorId = [0x00, 0x00], sessionId = 0x01, requestedMtu = DEFAULT_MTU) {
    const ms = Date.now();
    return encodeClientRepresentation(version, vendorId, this.clientKey, sessionId, requestedMtu, {
      representationTimestamps: this.representationTimestamps,
      seconds: Math.floor(ms / 1000),
      nanoseconds: (ms % 1000) * 1000000,
      unannotatedTypes: this.unannotatedTypes,
    });
  }

  // Initiates the handshake (Disconnected -> Connecting). The caller still
  // has to build and deliver CREATE_CLIENT itself, e.g. via
  // buildCreateClientMessage.
  connect() {
    if (this.state !== DISCONNECTED) throw new ClientError(`invalid state: ${this.state}`);
    this.state = CONNECTING;
  }

  // Completes the handshake once the caller has received STATUS_AGENT
  // (Connecting -> Connected).
  markConnected() {
    if (this.state !== CONNECTING) throw new ClientError(`invalid state: ${this.state}`);
    this.state = CONNECTED;
  }

  // Closes the session — back to Disconnected. Like the Rust client, this
  // sends nothing.
  disconnect() {
    this.state = DISCONNECTED;
  }

  _requireConnected() {
    if (this.state !== CONNECTED) throw new ClientError(`invalid state: ${this.state}`);
  }

  // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
  // never a valid request id) — identical to
  // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
  _allocRequestId() {
    const req = this._nextRequestId;
    let next = (req + 1) & 0xffff;
    if (next === 0) next = 1;
    this._nextRequestId = next;
    return req;
  }

  // Submits a CREATE (§7.8.3). Returns a request id.
  createObject(_objectId, _variant) {
    this._requireConnected();
    return this._allocRequestId();
  }

  // Submits a DELETE (§7.8.3).
  deleteObject(_objectId) {
    this._requireConnected();
    return this._allocRequestId();
  }

  // Submits a WRITE_DATA (§7.8.4).
  requestWrite(_writer, _payload) {
    this._requireConnected();
    return this._allocRequestId();
  }

  // Submits a READ_DATA (§7.8.5, pull-based).
  requestRead(_reader) {
    this._requireConnected();
    return this._allocRequestId();
  }
}

module.exports = {
  OBJK_INVALID,
  OBJK_PARTICIPANT,
  OBJK_TOPIC,
  OBJK_PUBLISHER,
  OBJK_SUBSCRIBER,
  OBJK_DATAWRITER,
  OBJK_DATAREADER,
  OBJK_TYPE,
  OBJK_QOSPROFILE,
  OBJK_APPLICATION,
  OBJK_AGENT,
  OBJK_CLIENT,
  OBJK_OTHER,
  OBJECTID_INVALID,
  newObjectId,
  objectIdKind,
  objectIdRaw12,
  objectIdBytes,
  objectIdFromBytes,
  ObjectVariant,
  byReference,
  byXmlString,
  inBinary,
  encodeBaseObjectRequest,
  DATA_FORMAT_DATA,
  DataDeliveryControl,
  ReadSpecification,
  SESSION_ID_NONE_WITH_CLIENT_KEY,
  SESSION_ID_NONE_WITHOUT_CLIENT_KEY,
  STREAM_ID_NONE,
  STREAM_ID_BEST_EFFORT,
  STREAM_ID_BUILTIN_RELIABLE,
  CLIENT_KEY_LEN,
  CLIENTKEY_INVALID,
  encodeMessageHeader,
  XRCE_COOKIE,
  XRCE_VERSION,
  DEFAULT_MTU,
  encodeClientRepresentation,
  buildCreateClientMessage,
  buildCreateMessage,
  buildDeleteMessage,
  buildWriteDataMessage,
  buildReadDataMessage,
  DISCONNECTED,
  CONNECTING,
  CONNECTED,
  ClientError,
  XrceClient,
};
