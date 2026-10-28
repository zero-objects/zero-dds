// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for xrce_client.js: the state-machine transitions
// crates/xrce-client/src/lib.rs's own unit tests check, plus the wire
// octets against endpoints/testdata/xrce-client/*.bin — fixtures produced
// by endpoints/golden-gen actually running zerodds_xrce_client::XrceClient
// (crates/xrce-client) over zerodds_xrce (crates/xrce), the same two crates
// this module ports. All 20 language ports (plus
// endpoints/xrce-agent-demo) check the same five files; a hardcoded hex
// string here would be a second copy of the truth that can drift from it
// unnoticed. ClientKey = 01:02:03:04 throughout, session_id = 0x01 (<=127,
// so every header after CREATE_CLIENT carries the ClientKey; the
// CREATE_CLIENT header itself has session 0x00, see below).
//
//   createClient: CREATE_CLIENT, a real CLIENT_Representation
//     (version={1,0}, vendorId={0,0}, this clientKey, sessionId=0x01, no
//     properties, requestedMtu=508); header = (session 0x00, stream
//     STREAM_ID_NONE, seq 0, key). The header session is
//     SESSION_ID_NONE_WITH_CLIENT_KEY (DDS-XRCE 1.0 §8.3.2.1), not the
//     sessionId 0x01 the payload asks for.
//   create: CREATE, request_id={0x00,0x01}, object_id=pack(raw=0x001,
//     kind=DATAWRITER), byReference("SensorWriter") under
//     parent=pack(raw=0x001, kind=PUBLISHER) — the real two-level
//     ObjectVariant (§7.7.3), reuse=false, replace=false; header =
//     (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 0, key).
//   delete: DELETE, request_id={0x00,0x02}, object_id=pack(raw=0x001,
//     kind=DATAWRITER) — deletes the writer CREATEd above; header =
//     (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 1, key).
//   readData: READ_DATA, request_id={0x00,0x03}, object_id=pack(raw=0x002,
//     kind=DATAREADER), preferredStreamId=0x01, dataFormat=Data, no
//     filter, no delivery control; header = (session 0x01, stream
//     STREAM_ID_BUILTIN_RELIABLE, seq 2, key).
//   writeData: WRITE_DATA, request_id={0x00,0x04}, object_id=pack(raw=
//     0x001, kind=DATAWRITER) — the same writer — sample=[0x19,0x00];
//     header = (session 0x01, stream STREAM_ID_BUILTIN_RELIABLE, seq 3,
//     key).
//
// These four post-handshake fixtures are read independently, each as one
// wire example of its own submessage kind — not as one
// application-consistent object lifecycle: writeData targets the writer
// delete already removed, and nothing about either message's bytes
// depends on the other one having run first.

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const xc = require('./xrce_client.js');

const KEY = Buffer.from([0x01, 0x02, 0x03, 0x04]);

const hex = (b) => Buffer.from(b).toString('hex');

const TESTDATA_DIR = path.join(__dirname, '..', 'testdata', 'xrce-client');
const fixture = (name) => fs.readFileSync(path.join(TESTDATA_DIR, `${name}.bin`));

// --- state-machine tests, mirroring crates/xrce-client's own unit tests ---

test('new client starts Disconnected', () => {
  const c = new xc.XrceClient(KEY);
  assert.strictEqual(c.state, xc.DISCONNECTED);
  assert.deepStrictEqual(c.clientKey, KEY);
});

test('connect transitions to Connecting', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  assert.strictEqual(c.state, xc.CONNECTING);
});

test('markConnected transitions to Connected', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  c.markConnected();
  assert.strictEqual(c.state, xc.CONNECTED);
});

test('double connect rejected', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  assert.throws(() => c.connect(), xc.ClientError);
});

test('createObject without connect rejected', () => {
  const c = new xc.XrceClient(KEY);
  const oid = xc.newObjectId(0x010, xc.OBJK_TOPIC);
  assert.throws(() => c.createObject(oid, xc.byReference('topic-ref')), xc.ClientError);
});

test('requestWrite without connect rejected', () => {
  const c = new xc.XrceClient(KEY);
  const oid = xc.newObjectId(0x010, xc.OBJK_TOPIC);
  assert.throws(() => c.requestWrite(oid, Buffer.from('x')), xc.ClientError);
});

test('requestRead without connect rejected', () => {
  const c = new xc.XrceClient(KEY);
  const oid = xc.newObjectId(0x010, xc.OBJK_TOPIC);
  assert.throws(() => c.requestRead(oid), xc.ClientError);
});

test('full lifecycle creates unique, monotonic request ids', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  c.markConnected();
  const oid = xc.newObjectId(0x010, xc.OBJK_TOPIC);
  const r1 = c.createObject(oid, xc.byReference('a'));
  const r2 = c.deleteObject(oid);
  const r3 = c.requestWrite(oid, Buffer.from('x'));
  const r4 = c.requestRead(oid);
  assert.deepStrictEqual([r1, r2, r3, r4], [1, 2, 3, 4]);
});

test('disconnect clears state', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  c.markConnected();
  c.disconnect();
  assert.strictEqual(c.state, xc.DISCONNECTED);
});

// --- wire-octet tests -------------------------------------------------------

test('CREATE_CLIENT matches fixture', () => {
  const rep = xc.encodeClientRepresentation(xc.XRCE_VERSION, Buffer.from([0x00, 0x00]), KEY, 0x01, xc.DEFAULT_MTU);
  // §8.3.2.1: the CREATE_CLIENT of a session_id below 128 goes in a header with
  // SESSION_ID_NONE_WITH_CLIENT_KEY (0x00); the payload asks for session 1.
  const got = xc.buildCreateClientMessage(0x00, xc.STREAM_ID_NONE, 0, KEY, rep);
  assert.strictEqual(hex(got), hex(fixture('xrce_client_create_client')));
});

// DDS-XRCE 1.0 §7.8.2.1 prose vs Annex A / B.1: representationTimestamps.
test('representationTimestamps: absent (default) 16 octets, present appendable 28, present final 24', () => {
  const vendor = Buffer.from([0x00, 0x00]);
  const tail = [0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0xfc, 0x01];
  const head = [0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x00, 0x00];
  const timeT = [0x8c, 0xaa, 0x88, 0x5a, 0x00, 0x65, 0xcd, 0x1d];
  const absent = Buffer.from([...head, ...tail]);
  const appendable = Buffer.from([...head, 0x08, 0x00, 0x00, 0x00, ...timeT, ...tail]);
  const final = Buffer.from([...head, ...timeT, ...tail]);
  const t = { seconds: 1518905996, nanoseconds: 500000000 };
  assert.strictEqual(hex(xc.encodeClientRepresentation(xc.XRCE_VERSION, vendor, KEY, 0x01, 508)), hex(absent));
  assert.strictEqual(hex(xc.encodeClientRepresentation(xc.XRCE_VERSION, vendor, KEY, 0x01, 508, { representationTimestamps: 'absent', ...t })), hex(absent));
  assert.strictEqual(hex(xc.encodeClientRepresentation(xc.XRCE_VERSION, vendor, KEY, 0x01, 508, { representationTimestamps: 'present', ...t })), hex(appendable));
  assert.strictEqual(hex(xc.encodeClientRepresentation(xc.XRCE_VERSION, vendor, KEY, 0x01, 508, { representationTimestamps: 'present', unannotatedTypes: 'final', ...t })), hex(final));
  assert.strictEqual(new xc.XrceClient(KEY, { representationTimestamps: 'present' }).clientRepresentation().length, 28);
  assert.strictEqual(new xc.XrceClient(KEY).clientRepresentation().length, 16);
});

test('CREATE_CLIENT for a session_id >= 128 has header session 0x80 and no key', () => {
  // §8.3.2.1: SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) carries no client key.
  const rep = xc.encodeClientRepresentation(xc.XRCE_VERSION, Buffer.from([0x00, 0x00]), KEY, 0x81, xc.DEFAULT_MTU);
  const got = xc.buildCreateClientMessage(xc.SESSION_ID_NONE_WITHOUT_CLIENT_KEY, xc.STREAM_ID_NONE, 0, null, rep);
  assert.strictEqual(got[0], 0x80);
  assert.strictEqual(got.length, 4 + 4 + rep.length);
  assert.strictEqual(got[4 + 4 + 12], 0x81);
});

// CREATE/DELETE/READ_DATA/WRITE_DATA — one client, exercised through its
// real request-id counter (not just the encoder). All four are reliable,
// so they share STREAM_ID_BUILTIN_RELIABLE and a real, strictly
// increasing sequence number (0..3) — not the best-effort stream with
// hand-picked numbers a previous fixture set used.
test('CREATE/DELETE/READ_DATA/WRITE_DATA match the session fixtures', () => {
  const c = new xc.XrceClient(KEY);
  c.connect();
  c.markConnected();

  const writer = xc.newObjectId(0x001, xc.OBJK_DATAWRITER);
  const publisher = xc.newObjectId(0x001, xc.OBJK_PUBLISHER);
  const reader = xc.newObjectId(0x002, xc.OBJK_DATAREADER);
  const variant = xc.byReference('SensorWriter');

  let req = c.createObject(writer, variant);
  assert.strictEqual(req, 1);
  let got = xc.buildCreateMessage(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 0, KEY, req, writer, xc.OBJK_DATAWRITER, variant, {
    parentObjectId: publisher,
  });
  assert.strictEqual(hex(got), hex(fixture('xrce_client_create')));

  req = c.deleteObject(writer);
  assert.strictEqual(req, 2);
  got = xc.buildDeleteMessage(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 1, KEY, req, writer);
  assert.strictEqual(hex(got), hex(fixture('xrce_client_delete')));

  const spec = new xc.ReadSpecification(0x01, xc.DATA_FORMAT_DATA);
  req = c.requestRead(reader);
  assert.strictEqual(req, 3);
  got = xc.buildReadDataMessage(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 2, KEY, req, reader, spec);
  assert.strictEqual(hex(got), hex(fixture('xrce_client_read_data')));

  req = c.requestWrite(writer, Buffer.from([0x19, 0x00]));
  assert.strictEqual(req, 4);
  got = xc.buildWriteDataMessage(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 3, KEY, req, writer, Buffer.from([0x19, 0x00]));
  assert.strictEqual(hex(got), hex(fixture('xrce_client_write_data')));

  // A second CREATE, appended to the same session: OBJK_TOPIC's own
  // representation carries its participant's ObjectId (commit
  // 31c62df2a63, `xrce: OBJK_TOPIC_Representation carries its
  // participant's ObjectId`), the same trailing field OBJK_PUBLISHER's
  // representation carries above — xrce_client_create_topic.bin is a
  // sixth fixture generated the same way (endpoints/golden-gen), not a
  // hand-picked byte string.
  const topicOid = xc.newObjectId(0x010, xc.OBJK_TOPIC);
  const participant = xc.newObjectId(0x001, xc.OBJK_PARTICIPANT);
  const topicVariant = xc.byReference('HelloWorldTopic');
  req = c.createObject(topicOid, topicVariant);
  assert.strictEqual(req, 5);
  got = xc.buildCreateMessage(0x01, xc.STREAM_ID_BUILTIN_RELIABLE, 4, KEY, req, topicOid, xc.OBJK_TOPIC, topicVariant, {
    parentObjectId: participant,
  });
  assert.strictEqual(hex(got), hex(fixture('xrce_client_create_topic')));
});

// crates/xrce/src/object_repr.rs's own
// `topic_representation_matches_the_captured_create_bytes` test decodes
// these exact 96 bytes (the real eProsima Micro-XRCE-DDS 3.0.1 capture,
// tools/wire-capture/xrce-sessions/client-to-agent.hex) as
// OBJK_TOPIC/ByXmlString/participant_id=ObjectId(1, PARTICIPANT). This
// port's encodeForKind — unlike xrce_client_create_topic.bin's
// ByReference case above — can also express ByXmlString, so it is
// checked against that same oracle directly.
test('OBJK_TOPIC ByXmlString matches the Rust byte oracle', () => {
  const xml =
    '<dds><topic><name>HelloWorldTopic</name><dataType>HelloWorld</dataType></topic></dds>';
  const expected = Buffer.concat([
    Buffer.from([0x02, 0x02, 0x00, 0x00, 0x56, 0x00, 0x00, 0x00]),
    Buffer.from(xml, 'utf8'),
    Buffer.from([0x00, 0x00, 0x11]),
  ]);
  const participant = xc.newObjectId(0x001, xc.OBJK_PARTICIPANT);
  const got = xc.byXmlString(xml).encodeForKind(xc.OBJK_TOPIC, { parentObjectId: participant });
  assert.strictEqual(hex(got), hex(expected));
});

// OBJK_TOPIC now carries a parent id (hasParent), so a CREATE without one
// is refused the same way OBJK_PUBLISHER's already is — encodeForKind's
// own hasParent/parentObjectId-null guard, not a topic-specific check.
test('OBJK_TOPIC CREATE without a parent rejected', () => {
  assert.throws(() => xc.byReference('HelloWorldTopic').encodeForKind(xc.OBJK_TOPIC), RangeError);
});

// Cross-check independent of the fixtures above (crates/xrce
// `new_packs_kind_into_lower_4_bits`).
test('ObjectId packs kind into the low 4 bits and round-trips', () => {
  const id = xc.newObjectId(0x123, xc.OBJK_DATAWRITER);
  assert.strictEqual(id, (0x123 << 4) | 0x05);
  assert.strictEqual(xc.objectIdKind(id), xc.OBJK_DATAWRITER);
  assert.strictEqual(xc.objectIdRaw12(id), 0x123);
  const back = xc.objectIdFromBytes(xc.objectIdBytes(id));
  assert.strictEqual(back, id);
});

// Cross-checks the constructor guard crates/xrce's
// `MessageHeader::with_client_key`/`without_client_key` enforce.
test('MessageHeader rejects a session_id/client_key mismatch', () => {
  assert.throws(() => xc.encodeMessageHeader(10, xc.STREAM_ID_NONE, 0, null), RangeError);
  assert.throws(() => xc.encodeMessageHeader(200, xc.STREAM_ID_NONE, 0, KEY), RangeError);
});
