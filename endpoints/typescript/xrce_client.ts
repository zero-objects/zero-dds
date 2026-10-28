// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 client state machine + wire codec, in TypeScript, compiled
// with `tsc` and run on Node. A port of `crates/xrce-client/src/lib.rs`'s
// `XrceClient<T>` state machine (340 lines) combined with the concrete
// submessage codecs of `crates/xrce`
// (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). `crates/xrce-client`
// itself never touches the wire -- its module doc says so directly: "the
// caller delegates submessage encoding; we only track state". This module
// is that combination, matching `endpoints/c`'s `zdwc_*` framing (the
// port every fixture in endpoints/testdata/xrce-client/ was verified
// against) rather than reinventing the byte layout -- in particular the
// `request_id` byte order: big-endian, per
// `crates/xrce-client::request_id_octets`, not `endpoints/node`'s own
// (older, little-endian) choice for the same field.
//
// This is a standalone port, not a derivative of endpoints/node's
// zerodds.js -- there is no pre-existing TypeScript endpoint SDK in this
// repository, so it plugs into the same `{ deliver, receive }` transport
// shape Node/Kotlin/F#/etc. use without depending on Node's own module.
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
//   new XrceClient  -> §7.8.2 CREATE_CLIENT
//   connect         -> §8.4.5 (handshake)
//   createObject    -> §7.8.3 CREATE
//   deleteObject    -> §7.8.3 DELETE
//   requestWrite    -> §7.8.4 WRITE_DATA
//   requestRead     -> §7.8.5 READ_DATA
//   disconnect      -> §8.4.5

// --- ObjectKind / ObjectId (spec §7.2, §7.2.1) ------------------------------

// ObjectKind codes actually exercised by this client (DDS-XRCE 1.0 Table 4).
export enum XrceObjectKind {
  Participant = 0x01,
  Topic = 0x02,
  Publisher = 0x03,
  Subscriber = 0x04,
  DataWriter = 0x05,
  DataReader = 0x06,
}

// ObjectId (spec §7.2.1): a 12-bit raw id packed with a 4-bit ObjectKind
// into a 16-bit wire value, big-endian on the wire (crates/xrce
// object_id.rs::to_bytes).
export function xrceObjectId(rawId: number, kind: XrceObjectKind): number {
  return ((rawId & 0x0fff) << 4) | (kind & 0x0f);
}

function objectIdBytes(id: number): Buffer {
  return Buffer.from([(id >> 8) & 0xff, id & 0xff]);
}

// --- lifecycle state (mirrors ClientState) ----------------------------------

export enum XrceClientState {
  Disconnected = 'Disconnected',
  Connecting = 'Connecting',
  Connected = 'Connected',
}

// Mirrors `ClientError::InvalidState`: an operation was attempted in the
// wrong lifecycle state (e.g. createObject before connect).
export class XrceClientError extends Error {}

// Synchronous XRCE client state machine -- a port of crates/xrce-client's
// `XrceClient<T>`. It does not hold a transport and does not send
// anything itself, exactly like the Rust crate: pair its request ids with
// the frame* builders below and hand the result to your own transport's
// `deliver()`.
// DDS-XRCE 1.0 contradicts itself on CLIENT_Representation: the prose of
// §7.8.2.1 lists `Time_t client_timestamp` after xrce_vendor_id, the IDL of
// Annex A and the bytes of Annex B.1 do not. 'absent' (default) writes the
// Annex A form, 'present' the prose form. The agent has to be set the same
// way (`representation_timestamps:` in its configuration).
export type RepresentationTimestamps = 'absent' | 'present';

// Form of the unannotated Time_t (DDS-XTypes 1.3 §7.3.1.2.1.8): 'appendable'
// (default) writes a DHEADER = 8 in front of it, 'final' none.
export type UnannotatedTypes = 'appendable' | 'final';

export interface ClientRepresentationOptions {
  representationTimestamps?: RepresentationTimestamps;
  // Time_t seconds (i32) and nanoseconds (u32); used with 'present' only.
  seconds?: number;
  nanoseconds?: number;
  unannotatedTypes?: UnannotatedTypes;
}

export interface XrceClientOptions {
  representationTimestamps?: RepresentationTimestamps;
  unannotatedTypes?: UnannotatedTypes;
}

export class XrceClient {
  readonly clientKey: Buffer;
  readonly representationTimestamps: RepresentationTimestamps;
  readonly unannotatedTypes: UnannotatedTypes;
  private _state: XrceClientState = XrceClientState.Disconnected;
  private _sessionId = 0;
  private _nextRequestId = 1;

  constructor(clientKey: Uint8Array, options: XrceClientOptions = {}) {
    this.clientKey = Buffer.from(clientKey);
    this.representationTimestamps = options.representationTimestamps ?? 'absent';
    this.unannotatedTypes = options.unannotatedTypes ?? 'appendable';
    // validates both values
    encodeClientRepresentation([1, 0], [0, 0], this.clientKey, 1, DEFAULT_MTU, {
      representationTimestamps: this.representationTimestamps,
      unannotatedTypes: this.unannotatedTypes,
    });
  }

  // The CLIENT_Representation of this client (session_id = the one given to
  // connect()) for frameCreateClient, with its representationTimestamps
  // option. The Time_t is the wall clock at the time of the call.
  clientRepresentation(
    version: [number, number] = [0x01, 0x00],
    vendorId: [number, number] = [0x00, 0x00],
    requestedMtu = DEFAULT_MTU,
  ): Buffer {
    const ms = Date.now();
    return encodeClientRepresentation(version, vendorId, this.clientKey, this._sessionId, requestedMtu, {
      representationTimestamps: this.representationTimestamps,
      seconds: Math.floor(ms / 1000),
      nanoseconds: (ms % 1000) * 1_000_000,
      unannotatedTypes: this.unannotatedTypes,
    });
  }

  get state(): XrceClientState {
    return this._state;
  }

  get sessionId(): number {
    return this._sessionId;
  }

  // §8.4.5 handshake: proposes `requestedSessionId` (1..127, §8.3.2.1)
  // and moves Disconnected -> Connecting. The caller still has to build
  // and deliver CREATE_CLIENT itself, e.g. via frameCreateClient.
  connect(requestedSessionId: number): void {
    if (this._state !== XrceClientState.Disconnected) {
      throw new XrceClientError(`invalid state: ${this._state}`);
    }
    this._sessionId = requestedSessionId;
    this._state = XrceClientState.Connecting;
  }

  // Completes the handshake once the caller has received STATUS_AGENT
  // (Connecting -> Connected).
  markConnected(): void {
    if (this._state !== XrceClientState.Connecting) {
      throw new XrceClientError(`invalid state: ${this._state}`);
    }
    this._state = XrceClientState.Connected;
  }

  // Closes the session -- back to Disconnected. Like the Rust client, this
  // sends nothing.
  disconnect(): void {
    this._state = XrceClientState.Disconnected;
  }

  private requireConnected(): void {
    if (this._state !== XrceClientState.Connected) {
      throw new XrceClientError(`invalid state: ${this._state}`);
    }
  }

  // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
  // never a valid request id) -- identical to
  // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
  private allocRequestId(): number {
    const req = this._nextRequestId;
    let next = (req + 1) & 0xffff;
    if (next === 0) next = 1;
    this._nextRequestId = next;
    return req;
  }

  // Submits a CREATE (§7.8.3). Returns a request id.
  createObject(): number {
    this.requireConnected();
    return this.allocRequestId();
  }

  // Submits a DELETE (§7.8.3).
  deleteObject(): number {
    this.requireConnected();
    return this.allocRequestId();
  }

  // Submits a WRITE_DATA (§7.8.4).
  requestWrite(): number {
    this.requireConnected();
    return this.allocRequestId();
  }

  // Submits a READ_DATA (§7.8.5, pull-based).
  requestRead(): number {
    this.requireConnected();
    return this.allocRequestId();
  }
}

// --- wire framing ------------------------------------------------------------
//
// MessageHeader (spec §8.3.2) + one SubmessageHeader (spec §8.3.4) + body.
// Every frame this client builds after CREATE_CLIENT carries the ClientKey
// (session_id <= 127, §8.3.2.1); the CREATE_CLIENT header has session_id
// SESSION_ID_NONE_WITH_CLIENT_KEY or SESSION_ID_NONE_WITHOUT_CLIENT_KEY -- the fixtures in endpoints/testdata/xrce-client/ use it
// uniformly, including for WRITE_DATA/READ_DATA, matching endpoints/c's
// `zdwc_frame_*`.

export const SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00;
export const SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80;
export const STREAM_ID_NONE = 0x00;
export const STREAM_ID_BEST_EFFORT = 0x01;
export const STREAM_ID_BUILTIN_RELIABLE = 0x80;

// eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
// module doc has the byte-oracle measurement backing it), used as this
// port's own CREATE_CLIENT requested_mtu.
export const DEFAULT_MTU = 508;

const SM_CREATE_CLIENT = 0x00;
const SM_CREATE = 0x01;
const SM_DELETE = 0x03;
const SM_WRITE_DATA = 0x07;
const SM_READ_DATA = 0x08;

const FLAG_E = 0x01; // E-flag: submessage body is little-endian (spec §8.3.4).
const FLAG_REUSE = 0x02;
const FLAG_REPLACE = 0x04;

// request_id (spec §7.7.8) as this port's own choice of byte order for a
// host-side monotonic counter: big-endian, matching ObjectId's own wire
// order and `crates/xrce-client::request_id_octets` (the answer this
// repository settled on).
function requestIdBytes(requestId: number): Buffer {
  return Buffer.from([(requestId >> 8) & 0xff, requestId & 0xff]);
}

// MessageHeader WITH ClientKey (8 bytes): session_id(1) + stream_id(1) +
// sequence_nr(2, LE) + ClientKey(4).
function header(sessionId: number, streamId: number, seq: number, clientKey: Buffer): Buffer {
  const out = Buffer.alloc(8);
  out[0] = sessionId;
  out[1] = streamId;
  out.writeUInt16LE(seq, 2);
  clientKey.copy(out, 4);
  return out;
}

// SubmessageHeader (4 bytes): id + flags + body length (LE, always -- spec
// §8.3.4).
function smHeader(smId: number, flags: number, bodyLen: number): Buffer {
  if (bodyLen > 0xffff) {
    throw new RangeError(`smHeader: body length ${bodyLen} exceeds the 16-bit length field`);
  }
  return Buffer.from([smId, flags, bodyLen & 0xff, (bodyLen >> 8) & 0xff]);
}

// CLIENT_Representation (Annex A, §7.8.2.1): xrce_cookie(4) + version(2) +
// vendor_id(2) + client_key(4) + session_id(1) + a `0` properties-absent
// octet + requestedMtu(2, LE) -- eProsima's own non-normative MTU
// extension (crates/xrce/src/handshake.rs's module doc has the
// byte-oracle measurement backing DEFAULT_MTU), always sent by this
// client. No PropertySeq support here -- this client sends none.
//
// `options.representationTimestamps` 'absent' (default) is the Annex A / B.1
// form. 'present' is the prose form of §7.8.2.1: after xrce_vendor_id the
// `Time_t client_timestamp` (`seconds` i32, `nanoseconds` u32, little
// endian). Time_t is APPENDABLE (DDS-XTypes 1.3 §7.3.1.2.1.8) with
// `unannotatedTypes: 'appendable'` (default): DHEADER = 8 in front, 12 octets;
// 'final': no DHEADER, 8 octets.
export function encodeClientRepresentation(
  version: [number, number],
  vendorId: [number, number],
  clientKey: Buffer,
  sessionId: number,
  requestedMtu = DEFAULT_MTU,
  options: ClientRepresentationOptions = {},
): Buffer {
  const ts = options.representationTimestamps ?? 'absent';
  const unann = options.unannotatedTypes ?? 'appendable';
  if (ts !== 'absent' && ts !== 'present') {
    throw new RangeError(`representationTimestamps must be 'absent' or 'present', got ${String(ts)}`);
  }
  if (unann !== 'appendable' && unann !== 'final') {
    throw new RangeError(`unannotatedTypes must be 'appendable' or 'final', got ${String(unann)}`);
  }
  const mtu = Buffer.alloc(2);
  mtu.writeUInt16LE(requestedMtu, 0);
  const parts: Buffer[] = [Buffer.from('XRCE', 'ascii'), Buffer.from(version), Buffer.from(vendorId)];
  if (ts === 'present') {
    const timeT = Buffer.alloc(unann === 'appendable' ? 12 : 8);
    let off = 0;
    if (unann === 'appendable') {
      timeT.writeUInt32LE(8, 0); // DHEADER of Time_t
      off = 4;
    }
    timeT.writeInt32LE((options.seconds ?? 0) | 0, off);
    timeT.writeUInt32LE((options.nanoseconds ?? 0) >>> 0, off + 4);
    parts.push(timeT);
  }
  parts.push(
    clientKey,
    Buffer.from([sessionId & 0xff, 0]), // session_id, then properties-absent
    mtu,
  );
  return Buffer.concat(parts);
}

// CREATE_CLIENT (§7.8.2/§8.3.5.1): no session exists yet, so the header's
// session_id is SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, MessageHeader WITH the
// ClientKey) when `sessionId` -- the session the representation asks for --
// is 0..127, and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80, 4-octet header,
// no ClientKey) when it is 128..255 (DDS-XRCE 1.0 §8.3.2.1);
// stream_id=0x00 STREAM_ID_NONE; sequence_nr=0; plus one
// CREATE_CLIENT submessage (flags=0x01, E-flag only) whose body is a real
// CLIENT_Representation (build it with encodeClientRepresentation) --
// byte-identical to xrce_client_create_client.bin.
export function frameCreateClient(sessionId: number, clientKey: Buffer, representation: Buffer): Buffer {
  const h =
    sessionId <= 127
      ? header(SESSION_ID_NONE_WITH_CLIENT_KEY, STREAM_ID_NONE, 0, clientKey)
      : Buffer.from([SESSION_ID_NONE_WITHOUT_CLIENT_KEY, STREAM_ID_NONE, 0, 0]);
  return Buffer.concat([h, smHeader(SM_CREATE_CLIENT, FLAG_E, representation.length), representation]);
}

// CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
// WITH the ClientKey + one CREATE submessage (§7.7.8 BaseObjectRequest +
// the real, two-level §7.7.3 ObjectVariant: the outer `objectKind` byte,
// then the inner REPRESENTATION_BY_REFERENCE payload, then that kind's
// own trailing field -- `parentObjectId` for a
// Topic/Publisher/Subscriber/DataWriter/DataReader, `undefined` for
// every other kind -- byte-oracled in crates/xrce/src/object_repr.rs
// against real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic
// (Topic's own participant_id: commit 31c62df2a63). `reference` is a
// NUL-terminated object_reference string (§7.7.3.1.1).
export function frameCreate(
  sessionId: number,
  clientKey: Buffer,
  seq: number,
  requestId: number,
  objectId: number,
  objectKind: number,
  reference: string,
  parentObjectId?: number,
  reuse = false,
  replace = false,
): Buffer {
  const refBytes = Buffer.from(reference, 'utf8');
  const lenInclNul = refBytes.length + 1;
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32LE(lenInclNul, 0);
  const variant = Buffer.concat([
    Buffer.from([objectKind & 0xff]), // outer ObjectKind byte (§7.7.3)
    Buffer.from([0x01, 0x00, 0x00]), // inner disc=BY_REFERENCE + 2 padding
    lenBuf,
    refBytes,
    Buffer.from([0x00]), // NUL terminator
    ...(parentObjectId !== undefined ? [objectIdBytes(parentObjectId)] : []),
  ]);
  const body = Buffer.concat([requestIdBytes(requestId), objectIdBytes(objectId), variant]);
  let flags = FLAG_E;
  if (reuse) flags |= FLAG_REUSE;
  if (replace) flags |= FLAG_REPLACE;
  return Buffer.concat([header(sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey), smHeader(SM_CREATE, flags, body.length), body]);
}

// DELETE (§7.8.3/§8.3.5.4): MessageHeader WITH the ClientKey + one DELETE
// submessage (§7.7.8 BaseObjectRequest only).
export function frameDelete(sessionId: number, clientKey: Buffer, seq: number, requestId: number, objectId: number): Buffer {
  const body = Buffer.concat([requestIdBytes(requestId), objectIdBytes(objectId)]);
  return Buffer.concat([header(sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey), smHeader(SM_DELETE, FLAG_E, body.length), body]);
}

// WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA): MessageHeader WITH the
// ClientKey + one WRITE_DATA submessage (§7.7.8 BaseObjectRequest +
// `sample` verbatim, no length of its own -- DDS-XRCE 1.0 Annex B.3.1).
// The 16-bit submessage_length bounds the body (BaseObjectRequest +
// sample); `smHeader` throws RangeError when it does not fit, rather than
// truncate the length while copying the full payload.
export function frameWriteData(
  sessionId: number,
  clientKey: Buffer,
  seq: number,
  requestId: number,
  writerObjectId: number,
  sample: Buffer,
): Buffer {
  const body = Buffer.concat([requestIdBytes(requestId), objectIdBytes(writerObjectId), sample]);
  return Buffer.concat([header(sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey), smHeader(SM_WRITE_DATA, FLAG_E, body.length), body]);
}

// READ_DATA (§7.8.5/§8.3.5.9): MessageHeader WITH the ClientKey + one
// READ_DATA submessage (§7.7.8 BaseObjectRequest + a minimal
// ReadSpecification: no content filter, no delivery control -- both
// @optional members absent). `preferredStreamId` selects the StreamId the
// agent should use for the DATA reply.
export function frameReadData(
  sessionId: number,
  clientKey: Buffer,
  seq: number,
  requestId: number,
  readerObjectId: number,
  preferredStreamId: number,
): Buffer {
  const body = Buffer.concat([
    requestIdBytes(requestId),
    objectIdBytes(readerObjectId),
    Buffer.from([
      preferredStreamId & 0xff,
      0x00, // data_format = FORMAT_DATA
      0x00, // content_filter_expression? = absent
      0x00, // delivery_control? = absent
    ]),
  ]);
  return Buffer.concat([header(sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey), smHeader(SM_READ_DATA, FLAG_E, body.length), body]);
}
