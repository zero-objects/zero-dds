// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dcps.test.ts — browser DCPS-over-WebSocket (DDS-TS Annex C) through the
// binary frames of zerodds-ws-bridge-1.0.
//
// The frame layout and the topic paths are held to the specification's text
// (§4.3, the example of §5.1). The facade and the flat C.2 operations run
// against an in-process WebSocket server that routes by path as the daemon
// does (§4.2) and hands every binary frame a client sends to the
// connections on the same path, so the client stack runs without a browser;
// the daemon itself is the peer in tests/integration/tests/bindings_ffi.rs.
// The browser uses the same code with the global `WebSocket`; here the Node
// `ws` package is injected through a WebSocketFactory.

import test from "node:test";
import assert from "node:assert/strict";
import { WebSocket, WebSocketServer } from "ws";

import {
  DomainParticipantFactory,
  SUBPROTOCOL,
  decodeFrame,
  encodeFrame,
  topicPath,
  type WebSocketLike,
} from "../src/index.js";
import {
  registerParticipant,
  createTopic,
  createPublisher,
  createSubscriber,
  createDataWriter,
  createDataReader,
  writeSample,
  takeSamples,
  deleteParticipant,
} from "../src/index.js";

/// An in-process server routing by path (§4.2): a binary frame a client
/// sends goes to every connection on its path, the sender's included, as the
/// daemon's DDS writer and reader of one topic would carry it. It records the
/// paths and the subprotocols the clients asked for.
function startBridge(): Promise<{
  url: string;
  paths: string[];
  protocols: string[];
  close: () => Promise<void>;
}> {
  const paths: string[] = [];
  const protocols: string[] = [];
  return new Promise((resolve) => {
    const wss = new WebSocketServer(
      {
        port: 0,
        handleProtocols: (offered) => {
          protocols.push(...offered);
          return offered.has(SUBPROTOCOL) ? SUBPROTOCOL : false;
        },
      },
      () => {
        const addr = wss.address();
        const port = typeof addr === "object" && addr ? addr.port : 0;
        resolve({
          url: `ws://127.0.0.1:${port}`,
          paths,
          protocols,
          close: () => new Promise<void>((r) => wss.close(() => r())),
        });
      },
    );
    const pathOf = new Map<unknown, string>();
    wss.on("connection", (ws, req) => {
      const path = req.url ?? "/";
      paths.push(path);
      pathOf.set(ws, path);
      ws.on("message", (raw, isBinary) => {
        if (!isBinary) return;
        const frame = new Uint8Array(raw as Buffer);
        assert.ok(decodeFrame(frame), "the client sends binary frames of §4.3");
        for (const client of wss.clients) {
          if (pathOf.get(client) === path) client.send(frame);
        }
      });
    });
  });
}

const nodeWsFactory = (url: string, protocols?: string[]): WebSocketLike =>
  new WebSocket(url, protocols) as unknown as WebSocketLike;

test("wsbridge 5.1: the topic path is the slug of the topic name", () => {
  // The example of zerodds-ws-bridge-1.0 §5.1.
  assert.equal(topicPath("Chat::Message"), "/topics/chat/message");
  // Step 3: every other character outside [a-z0-9/_-] is an underscore.
  assert.equal(topicPath("Sensor Reading!"), "/topics/sensor_reading_");
});

test("wsbridge 4.3: a sample frame is ZDB1, flags, the encapsulation header, the payload", () => {
  const frame = encodeFrame(new Uint8Array([7, 0, 0, 0]));
  // `ZDB1`, flags 0, `00 07 00 00` (XCDR2 little endian), the payload.
  assert.deepEqual(
    Array.from(frame),
    [0x5a, 0x44, 0x42, 0x31, 0x00, 0x00, 0x07, 0x00, 0x00, 7, 0, 0, 0],
  );
  const back = decodeFrame(frame)!;
  assert.equal(back.flags, 0);
  assert.equal(back.identifier, 0x0007);
  assert.deepEqual(Array.from(back.payload), [7, 0, 0, 0]);
  // Shorter than magic, flags and header; another magic; no CDR representation.
  assert.equal(decodeFrame(new Uint8Array([0x5a, 0x44, 0x42, 0x31, 0, 0, 7, 0])), null);
  assert.equal(decodeFrame(new Uint8Array([0x5a, 0x44, 0x42, 0x32, 0, 0, 7, 0, 0])), null);
  assert.equal(decodeFrame(new Uint8Array([0x5a, 0x44, 0x42, 0x31, 0, 0, 0x04, 0, 0])), null);
});

test("wasm DCPS facade: a writer's sample reaches the reader of its topic over the topic's path", async () => {
  const bridge = await startBridge();
  try {
    const factory = DomainParticipantFactory.instance();
    const participant = await factory.createParticipantWebSocket(
      bridge.url,
      0,
      nodeWsFactory,
    );

    const topic = participant.createBytesTopic("Chat::Message");
    const writer = participant.createPublisher().createBytesWriter(topic);
    const reader = participant.createSubscriber().createBytesReader(topic);

    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);
    // One connection for the topic, on its path, asking for the binary
    // frames' subprotocol (§4.1).
    assert.deepEqual(bridge.paths, ["/topics/chat/message"]);
    assert.ok(bridge.protocols.includes(SUBPROTOCOL));

    writer.write(new TextEncoder().encode("hello from browser"));

    const ready = await reader.waitForData(3000);
    assert.equal(ready, true, "waitForData timed out");

    const decoded = reader.take().map((b) => new TextDecoder().decode(b));
    assert.deepEqual(decoded, ["hello from browser"]);

    participant.destroy();
  } finally {
    await bridge.close();
  }
});

test("wasm DCPS flat C.2 operations: handle table roundtrip", async () => {
  const bridge = await startBridge();
  try {
    const participant = await DomainParticipantFactory.instance().createParticipantWebSocket(
      bridge.url,
      0,
      nodeWsFactory,
    );
    const p = registerParticipant(participant);

    const t = createTopic(p, "Echo", "PrimitiveSample");
    const pub = createPublisher(p);
    const sub = createSubscriber(p);
    const w = createDataWriter(pub, t);
    const r = createDataReader(sub, t);

    const xcdr2 = new Uint8Array([7, 0, 0, 0]); // a 'long v = 7' LE payload
    // Written before the topic's connection is open: it goes out when it is.
    writeSample(w, xcdr2);

    // Poll takeSamples within 1s (Annex C.5.1).
    let samples = takeSamples(r, 10);
    for (let i = 0; i < 100 && samples.length === 0; i++) {
      await new Promise((res) => setTimeout(res, 10));
      samples = takeSamples(r, 10);
    }
    assert.equal(samples.length, 1, "expected exactly one sample");
    assert.equal(samples[0].info.validData, true);
    assert.equal(samples[0].info.bigEndian, false);
    assert.deepEqual(Array.from(samples[0].bytes), [7, 0, 0, 0]);

    deleteParticipant(p);
    assert.throws(() => takeSamples(r, 1), /invalid or deleted reader/);
  } finally {
    await bridge.close();
  }
});

test("wasm DCPS: invalid-handle sentinel rejects at runtime (Annex C.1.1)", () => {
  assert.throws(() => takeSamples(0 as never, 1), RangeError);
});

// The byte order comes from the frame's encapsulation header (DDSI-RTPS 2.5
// §10.5: the identifiers of big endian are even), the instance state from its
// flags (§4.3: bit 0 a dispose, bit 2 an unregister); an `error` control
// message (§4.4) is kept for its topic. Drives a fake socket directly.
test("wsbridge 4.3: the header names the byte order, the flags the instance state", async () => {
  const { BridgeTransport } = await import("../src/dcps/transport.js");
  const { sampleFromBytes } = await import("../src/dcps/operations.js");

  type MsgCb = (ev: { data: unknown }) => void;
  const listeners: Record<string, ((ev: unknown) => void)[]> = {};
  const fake = {
    send() {},
    close() {},
    addEventListener(type: string, cb: (ev: unknown) => void) {
      (listeners[type] ??= []).push(cb);
    },
  };
  const transport = await BridgeTransport.connect("ws://fake", () => fake as never);
  transport.subscribe("T");

  const emit = (data: unknown) =>
    (listeners["message"] as MsgCb[] | undefined)?.forEach((cb) => cb({ data }));
  const frame = (flags: number, identifier: number, payload: number[]) =>
    new Uint8Array([0x5a, 0x44, 0x42, 0x31, flags, 0x00, identifier, 0x00, 0x00, ...payload]).buffer;

  emit(frame(0x00, 0x07, [1, 2])); // XCDR2 little endian
  emit(frame(0x00, 0x06, [1, 2])); // XCDR2 big endian
  emit(frame(0x01, 0x06, [3])); // a dispose with its key holder
  emit(frame(0x04, 0x06, [3])); // an unregister
  emit(JSON.stringify({ op: "error", code: 404, message: "T is not bridged in" }));

  const drained = transport.drain("T", 0);
  assert.equal(drained.length, 4);
  const samples = drained.map(sampleFromBytes);
  assert.deepEqual(
    samples.map((s) => [s.info.bigEndian, s.info.validData, s.info.instanceState]),
    [
      [false, true, "alive"],
      [true, true, "alive"],
      [true, false, "not_alive_disposed"],
      [true, false, "not_alive_no_writers"],
    ],
  );
  assert.deepEqual(transport.errors.get("T"), ["404 T is not bridged in"]);
});
