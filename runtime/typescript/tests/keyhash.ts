// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The KeyHash: `md5()` against RFC 1321 §A.5's own test suite, and
 * `Writer.zeroPadded()`/`Writer.md5()` against DDSI-RTPS 2.5 §9.6.3.8's
 * sixteen-octet threshold.
 *
 * The counterpart of `runtime/java`'s `WireSelfTest.md5AgreesWithRfc1321`
 * and `runtime/csharp`'s `SelfTest.cs`. `md5()` is hand-rolled here for
 * the reason `writer.ts`'s own doc comment on it gives: this runtime has
 * to run in a browser as well as Node, and the Web Crypto API a browser
 * exposes has no MD5 at all.
 */

import { Endianness } from "../src/endianness.js";
import { Writer, md5 } from "../src/writer.js";

declare const process: { exit(code: number): never };

function hex(octets: Uint8Array): string {
  return Array.from(octets)
    .map((octet) => octet.toString(16).padStart(2, "0"))
    .join("");
}

let failures = 0;

function check(name: string, got: string, want: string): void {
  if (got !== want) {
    failures += 1;
    console.log(`FAIL ${name}`);
    console.log(`  got  ${got}`);
    console.log(`  want ${want}`);
  }
}

// RFC 1321 §A.5.
const VECTORS: readonly [string, string][] = [
  ["", "d41d8cd98f00b204e9800998ecf8427e"],
  ["a", "0cc175b9c0f1b6a831c399e269772661".slice(0, 32)],
  ["abc", "900150983cd24fb0d6963f7d28e17f72"],
  ["message digest", "f96b697d7cb7938d525a2f31aaf161d0"],
  ["abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"],
  [
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
    "d174ab98d277d9f5a5611c2c9f419d9f",
  ],
  [
    "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
    "57edf4a22be3c955ac49da2e2107b67a",
  ],
];

for (const [input, want] of VECTORS) {
  const bytes = new TextEncoder().encode(input);
  check(`md5(${JSON.stringify(input)})`, hex(md5(bytes)), want);
}

// A holder of sixteen octets or fewer is the KeyHash, zero-padded; a
// longer one is its MD5. DDSI-RTPS 2.5 §9.6.3.8.
{
  const short = new Writer(Endianness.Big);
  short.writeU32(1);
  const expected = new Uint8Array(16);
  expected.set(short.finish());
  check(
    "a short key holder is zero-padded, not hashed",
    hex(short.zeroPadded()),
    hex(expected),
  );
}
{
  const wide = new Writer(Endianness.Big);
  for (let index = 0; index < 5; index += 1) {
    wide.writeU32(index);
  }
  check("a long key holder is hashed", hex(wide.md5()), hex(md5(wide.finish())));
  // The KeyHash the wire actually carries is over the exact holder
  // bytes — the finding this test would have caught: an implementation
  // that hashed a truncated or padded copy would still produce sixteen
  // octets and never fail a length check.
  check(
    "the holder feeding md5() is unpadded and untruncated",
    String(wide.finish().length),
    "20",
  );
}

console.log(`${VECTORS.length + 2 - failures} of ${VECTORS.length + 2} agree`);
process.exit(failures === 0 ? 0 : 1);
