// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dcps.test.ts — browser DCPS-over-WebSocket roundtrip (DDS-TS Annex C).
//
// Drives the facade + flat C.2 operations against an in-process WebSocket
// server that speaks the ZeroDDS websocket-bridge JSON protocol
// (subscribe / publish / notify), so the whole client stack is exercised
// without a browser. The browser uses the same code with the global
// `WebSocket`; here we inject the Node `ws` package via a WebSocketFactory.

import test from "node:test";
import assert from "node:assert/strict";
import { WebSocket, WebSocketServer } from "ws";

import {
  DomainParticipantFactory,
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

// A minimal in-process bridge: tracks per-connection topic subscriptions and
// echoes every published sample back to subscribers of that topic as a
// `notify`. Matches crates/websocket-bridge/src/dds_bridge.rs wire format.
function startBridge(): Promise<{ url: string; close: () => Promise<void> }> {
  return new Promise((resolve) => {
    const wss = new WebSocketServer({ port: 0 }, () => {
      const addr = wss.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      resolve({
        url: `ws://127.0.0.1:${port}`,
        close: () =>
          new Promise<void>((r) => wss.close(() => r())),
      });
    });
    const subs = new Map<unknown, Set<string>>();
    wss.on("connection", (ws) => {
      subs.set(ws, new Set());
      ws.on("message", (raw) => {
        const msg = JSON.parse(raw.toString());
        if (msg.op === "subscribe") {
          subs.get(ws)!.add(msg.topic);
        } else if (msg.op === "publish") {
          // Fan out to every connection subscribed to this topic.
          for (const client of wss.clients) {
            if (subs.get(client)?.has(msg.topic)) {
              client.send(
                JSON.stringify({
                  op: "notify",
                  topic: msg.topic,
                  data: msg.data,
                  subscription_id: "sub-x",
                }),
              );
            }
          }
        }
      });
    });
  });
}

const nodeWsFactory = (url: string): WebSocketLike =>
  new WebSocket(url) as unknown as WebSocketLike;

test("wasm DCPS facade: browser pub/sub roundtrip over the bridge", async () => {
  const bridge = await startBridge();
  try {
    const factory = DomainParticipantFactory.instance();
    const participant = await factory.createParticipantWebSocket(
      bridge.url,
      0,
      nodeWsFactory,
    );

    const topic = participant.createBytesTopic("Chatter");
    const writer = participant.createPublisher().createBytesWriter(topic);
    const reader = participant.createSubscriber().createBytesReader(topic);

    await writer.waitForMatchedSubscription(1, 5000);
    await reader.waitForMatchedPublication(1, 5000);

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
    writeSample(w, xcdr2);

    // Poll takeSamples within 1s (Annex C.5.1).
    let samples = takeSamples(r, 10);
    for (let i = 0; i < 100 && samples.length === 0; i++) {
      await new Promise((res) => setTimeout(res, 10));
      samples = takeSamples(r, 10);
    }
    assert.equal(samples.length, 1, "expected exactly one sample");
    assert.equal(samples[0].info.validData, true);
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

// A big-endian notify (`"be":true`) must surface `info.bigEndian === true` so a
// browser consumer dispatches the big-endian decoder; the LE default omits the
// field and stays `false`. Drives a fake socket directly to inject the frame.
test("wasm bridge: notify be:true surfaces info.bigEndian", async () => {
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
  const transport = await BridgeTransport.connect("ws://fake", () => {
    // Open on the next tick so connect() resolves.
    queueMicrotask(() => listeners["open"]?.forEach((cb) => cb({})));
    return fake as never;
  });

  const emit = (obj: unknown) =>
    (listeners["message"] as MsgCb[] | undefined)?.forEach((cb) =>
      cb({ data: JSON.stringify(obj) }),
    );

  // base64("AQI=") == bytes [1,2]; LE then BE on the same topic.
  emit({ op: "notify", topic: "T", data: "AQI=" });
  emit({ op: "notify", topic: "T", data: "AQI=", be: true });

  const drained = transport.drain("T", 0);
  assert.equal(drained.length, 2);
  assert.equal(drained[0].bigEndian, false);
  assert.equal(drained[1].bigEndian, true);

  const samples = drained.map(sampleFromBytes);
  assert.equal(samples[0].info.bigEndian, false);
  assert.equal(samples[1].info.bigEndian, true);
});
