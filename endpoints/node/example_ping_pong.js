// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Live ping-pong driver: argv = <mode> <peer-port>, mode in {sync, async}.
// Marshals a `Ping { long seq; string msg; }` (@final -> inline XCDR2) with the
// native Node wire-core (zerodds.js Writer, byte-identical to the Rust core),
// frames it XRCE WRITE_DATA, and sends it over a real UDP socket to the Rust
// peer (zerodds-endpoint-e2e's bind_peer). The peer replies a `Pong` in a DATA
// frame; this app decodes it and prints `PONG seq=<n> reply=<reply>`.
//   sync  = the caller owns the poll loop (Client.poll()).
//   async = the async iterator drains the socket (AsyncReader.stream()).

'use strict';

const z = require('./zerodds');
const { UdpTransport } = require('./udp');

const PING_SEQ = 1;
const PING_MSG = 'hello from app';

function marshalPing() {
  const w = new z.Writer(z.LITTLE);
  w.putU32(PING_SEQ);
  w.putString(PING_MSG);
  return w.bytes();
}

function decodePong(body) {
  const rd = new z.Reader(body, z.LITTLE);
  const seq = rd.getU32();
  const reply = rd.getString();
  return { seq, reply };
}

async function main() {
  const mode = process.argv[2];
  const port = parseInt(process.argv[3], 10);
  const transport = new UdpTransport('127.0.0.1', port);
  const client = new z.Client(transport);

  // Marshal + frame + send the Ping (Client.write does the XRCE WRITE_DATA framing).
  client.write(marshalPing());

  let body = null;
  if (mode === 'async') {
    // Async: iterate the async stream until the first Pong arrives.
    const reader = new z.AsyncReader(transport);
    for await (const b of reader.stream()) {
      body = b;
      reader.close();
      break;
    }
  } else {
    // Sync: the caller owns the run-loop; poll until a frame is available.
    const deadline = Date.now() + 20000;
    for (;;) {
      body = client.poll();
      if (body) break;
      if (Date.now() >= deadline) break;
      await new Promise((resolve) => setTimeout(resolve, 2));
    }
  }

  if (!body) {
    console.error('no Pong received');
    transport.close();
    process.exit(1);
  }
  const pong = decodePong(body);
  console.log(`PONG seq=${pong.seq} reply=${pong.reply}`);
  transport.close();
  process.exit(0);
}

main();
