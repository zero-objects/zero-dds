// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The WASM subscriber of tests/integration/tests/bindings_ffi.rs: the DCPS
// client of zerodds-ts-wasm (DDS-TS Annex C, crates/ts-wasm/src/dcps) through
// the daemon zerodds-ws-bridged (zerodds-ws-bridge-1.0), and the crate's
// WebAssembly codec (its `pkg-node/` glue, which tests/integration/ts_wasm.sh
// generates) decoding every body. Run from crates/ts-wasm, whose packages
// hold `ws` and tsx; Node needs no browser for either:
//
//   node --import tsx sub_wasm.mts <bridge url> <domain> <topic> <type name> <samples>
//
// The protocol of sub.c, with the decoded members beside each body:
// `EVENT matched` once the topic's connection is open, `EVENT sample <hex>`
// and `EVENT decoded <seq> <text>` per body, `EVENT received n=<count>`, one
// more take after standard input closed, `EVENT done`.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

import { DomainParticipantFactory, type DataReader } from "../../../../crates/ts-wasm/src/dcps/facade.js";
import type { WebSocketLike } from "../../../../crates/ts-wasm/src/dcps/transport.js";

// `ws` and the CommonJS glue resolve from the crate's own packages.
const require = createRequire(new URL("../../../../crates/ts-wasm/package.json", import.meta.url));
const { WebSocket } = require("ws") as {
  WebSocket: new (url: string, protocols?: string[]) => WebSocketLike;
};
const { CdrDecoder } = require("./pkg-node/zerodds_ts_wasm.js") as {
  CdrDecoder: new (bytes: Uint8Array, endianness: number, encoding?: number) => {
    readU32(): number;
    readString(): string;
    remaining(): number;
  };
};

/// Prints one body and its members as the WebAssembly codec reads them:
/// XCDR2 (`MAXALIGN` 4), little endian, `unsigned long seq; string text;`.
function show(body: Uint8Array): void {
  console.log("EVENT sample " + Buffer.from(body).toString("hex"));
  const decoder = new CdrDecoder(body, 0, 2);
  const seq = decoder.readU32();
  const text = decoder.readString();
  console.log(`EVENT decoded ${seq} ${text} rest=${decoder.remaining()}`);
}

function takeAll(reader: DataReader): number {
  const bodies = reader.take();
  for (const body of bodies) show(body);
  return bodies.length;
}

async function main(argv: string[]): Promise<number> {
  if (argv.length !== 5) {
    console.error("usage: sub_wasm.mts <bridge url> <domain> <topic> <type name> <samples>");
    return 2;
  }
  const [url, domain, topicName, typeName, samples] = argv;
  const wanted = Number(samples);
  const participant = await DomainParticipantFactory.instance().createParticipantWebSocket(
    url,
    Number(domain),
    (u, protocols) => {
      const ws = new WebSocket(u, protocols);
      // What ends a topic's connection, on standard error for the transcript.
      ws.addEventListener("close", (ev) => {
        const e = ev as unknown as { code?: number; reason?: string };
        console.error(`websocket ${u} closed: ${e.code ?? ""} ${e.reason ?? ""}`);
      });
      ws.addEventListener("error", (ev) => {
        const e = ev as unknown as { message?: string };
        console.error(`websocket ${u} error: ${e.message ?? String(ev)}`);
      });
      return ws;
    },
  );
  try {
    const topic = participant.createTopic(topicName, typeName);
    const reader = participant.createSubscriber().createBytesReader(topic);
    try {
      await reader.waitForMatchedPublication(1, 10000);
    } catch (e) {
      console.log("EVENT unmatched " + String(e));
      return 1;
    }
    console.log("EVENT matched");
    let taken = 0;
    const deadline = Date.now() + 10000;
    while (taken < wanted && Date.now() < deadline) {
      if (!(await reader.waitForData(Math.max(0, deadline - Date.now())))) break;
      taken += takeAll(reader);
    }
    console.log(`EVENT received n=${taken}`);
    readFileSync(0);
    // A sample taken now was delivered twice.
    takeAll(reader);
    console.log("EVENT done");
    return 0;
  } finally {
    participant.destroy();
  }
}

process.exitCode = await main(process.argv.slice(2));
