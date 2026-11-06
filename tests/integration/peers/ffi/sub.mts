// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The TypeScript subscriber of tests/integration/tests/bindings_ffi.rs: the
// koffi binding of crates/ts-node over libzerodds (zerodds-ffi-loader-1.0
// §3.5), with ZERODDS_LIB naming the library. Run from crates/ts-node, whose
// packages hold koffi and tsx:
//
//   node --import tsx sub.mts <domain> <topic> <type name> <samples>
//
// The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
// `EVENT received n=<count>`, one more take after standard input closed,
// `EVENT done`.

import { readFileSync } from "node:fs";
import { Reader, Runtime } from "../../../../crates/ts-node/src/index.js";

function takeOne(reader: Reader): boolean {
  const body = reader.take();
  if (body === null) return false;
  console.log("EVENT sample " + Buffer.from(body).toString("hex"));
  return true;
}

async function main(argv: string[]): Promise<number> {
  if (argv.length !== 4) {
    console.error("usage: sub.mts <domain> <topic> <type name> <samples>");
    return 2;
  }
  const [domain, topic, typeName, samples] = argv;
  const wanted = Number(samples);
  const runtime = new Runtime(Number(domain));
  const reader = new Reader(runtime, topic, typeName, true);
  try {
    if (!reader.waitForMatched(1, 10000)) {
      console.log("EVENT unmatched");
      return 1;
    }
    console.log("EVENT matched");
    let taken = 0;
    const deadline = Date.now() + 10000;
    while (taken < wanted && Date.now() < deadline) {
      if (takeOne(reader)) {
        taken += 1;
      } else {
        // The reader has no blocking take.
        await new Promise((resolve) => setTimeout(resolve, 5));
      }
    }
    console.log(`EVENT received n=${taken}`);
    readFileSync(0);
    while (takeOne(reader)) {
      // A sample taken now was delivered twice.
    }
    console.log("EVENT done");
    return 0;
  } finally {
    reader.destroy();
    runtime.destroy();
  }
}

process.exitCode = await main(process.argv.slice(2));
