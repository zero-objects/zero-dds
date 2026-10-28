// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The writer, against the octets the Rust column recorded.
 *
 * Not against a constant written here: a hard-coded expectation would
 * be a third opinion about the wire.
 * `tests/interop-results/<case>/zerodds-rust/<encoding>.<endian>.bin`
 * is what the Rust column produced and what every other column in the
 * ledger is held against, so a difference is a disagreement between
 * two implementations of §7.4 and that is the finding.
 *
 * The samples are written out by hand here rather than generated,
 * because the generator does not exist yet: this is what proves the
 * runtime before that is built on top of it.
 */

import { readFileSync, existsSync } from "node:fs";
import { Endianness } from "../src/endianness.js";
import { WstringLengthPolicy } from "../src/policy.js";
import { Reader } from "../src/reader.js";
import { Writer } from "../src/writer.js";

// The runtime itself pulls in nothing from Node — it has to run in a
// browser too — so the two globals this *test* needs are declared here
// rather than through a dependency on `@types/node`.
declare const process: { exit(code: number): never };

/**
 * The repository root, found by walking up until the corpus is there.
 *
 * Not a fixed number of `..`: this file is compiled into a directory
 * whose depth is the compiler's choice, and a hard-coded climb read
 * the wrong path the first time it ran.
 */
function repositoryRoot(): string {
  let at = new URL(".", import.meta.url).pathname;
  for (let depth = 0; depth < 8; depth += 1) {
    if (existsSync(`${at}tests/interop-corpus`)) {
      return at;
    }
    at = `${at}../`;
  }
  throw new Error("the corpus is not above this file");
}

const ROOT = repositoryRoot();

function recorded(caseName: string, encoding: string, order: string): string {
  const path = `${ROOT}tests/interop-results/${caseName}/zerodds-rust/${encoding}.${order}.bin`;
  return hex(readFileSync(path));
}

function hex(octets: Uint8Array): string {
  return Array.from(octets)
    .map((octet) => octet.toString(16).padStart(2, "0"))
    .join("");
}

/** `primitive-mixed`: fourteen scalars and the alignment between them. */
function primitiveMixed(order: Endianness, maxAlign: number): Uint8Array {
  const writer = new Writer(order, maxAlign);
  writer.writeBool(true);
  writer.writeU8(171);
  writer.writeU8(254);
  writer.writeI8(-13);
  writer.writeChar("Z");
  writer.writeWchar("ä");
  writer.writeI16(-12345);
  writer.writeU16(48879);
  writer.writeI32(-1234567);
  writer.writeU32(3735928559);
  writer.writeI64(-987654321987n);
  writer.writeU64(16045690984833335998n);
  writer.writeF32(-3.75);
  writer.writeF64(3.141592653589793);
  return writer.finish();
}

/**
 * `struct-appendable`: the delimiter a non-final aggregate carries.
 *
 * Under XCDR1 there is none — §7.4.3.5.3 gives the DHEADER to
 * `@appendable` and `@mutable` in encoding version 2 only — so the
 * same three members are eight octets shorter there, and a writer that
 * wrote one under both matches neither column.
 */
function structAppendable(order: Endianness, encoding: string): Uint8Array {
  const version2 = encoding !== "plain-cdr1";
  const writer = new Writer(order, version2 ? 4 : 8);
  const delimiter = version2 ? writer.writeDheader() : undefined;
  writer.writeI32(-1234567);
  writer.writeF64(3.141592653589793);
  writer.writeI16(-2);
  if (delimiter !== undefined) {
    writer.finishDheader(delimiter);
  }
  return writer.finish();
}

/** `string-bounded-utf8`: a bound counted in characters, not octets. */
function stringBoundedUtf8(order: Endianness, maxAlign: number): Uint8Array {
  const writer = new Writer(order, maxAlign);
  writer.writeStringBounded("grüße → 世界", 32);
  writer.writeI32(-1);
  return writer.finish();
}

/** `enum-sparse-values`: values that are not ordinal positions. */
function enumSparseValues(order: Endianness, maxAlign: number): Uint8Array {
  const writer = new Writer(order, maxAlign);
  // FAIL, OK, WARN — the declared values and not the positions. The
  // bit bound comes first, which is the order the shared walk calls in;
  // both arguments are a `number`, so the swapped pair this test used
  // to write was a legal call that produced a plausible octet.
  writer.writeEnum(32, 65535n);
  writer.writeEnum(32, 1n);
  writer.writeEnum(32, 100n);
  return writer.finish();
}

interface Check {
  readonly name: string;
  readonly build: (order: Endianness, encoding: string, maxAlign: number) => Uint8Array;
}

const CHECKS: Check[] = [
  {
    name: "primitive-mixed",
    build: (order, _encoding, maxAlign) => primitiveMixed(order, maxAlign),
  },
  {
    name: "struct-appendable",
    build: (order, encoding) => structAppendable(order, encoding),
  },
  {
    name: "string-bounded-utf8",
    build: (order, _encoding, maxAlign) => stringBoundedUtf8(order, maxAlign),
  },
  {
    name: "enum-sparse-values",
    build: (order, _encoding, maxAlign) => enumSparseValues(order, maxAlign),
  },
];

let failures = 0;
let checked = 0;

for (const check of CHECKS) {
  for (const [encoding, maxAlign] of [
    ["plain-cdr1", 8],
    ["plain-cdr2", 4],
  ] as const) {
    for (const order of [Endianness.Little, Endianness.Big] as const) {
      const want = recorded(check.name, encoding, order);
      const got = hex(check.build(order, encoding, maxAlign));
      checked += 1;
      if (got !== want) {
        failures += 1;
        console.log(`${check.name} ${encoding}/${order}`);
        console.log(`  typescript ${got}`);
        console.log(`  rust       ${want}`);
      }
    }
  }
}

console.log(`${checked - failures} of ${checked} agree with the Rust column`);

// -- the three axes `Dialect::Hdds` selects ---------------------------------
//
// Expected octets copied from the recorded HDDS columns, named per check:
// tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin,
// tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin,
// tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
// tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin.

let hddsChecks = 0;
let hddsFailures = 0;

function expectHdds(what: string, got: string, want: string): void {
  hddsChecks += 1;
  if (got !== want) {
    hddsFailures += 1;
    console.log(what);
    console.log(`  typescript ${got}`);
    console.log(`  hdds       ${want}`);
  }
}

function expectHddsThrows(what: string, body: () => void): void {
  hddsChecks += 1;
  try {
    body();
  } catch {
    return;
  }
  hddsFailures += 1;
  console.log(`${what}: nothing was raised`);
}

function unhex(text: string): Uint8Array {
  const octets = new Uint8Array(text.length / 2);
  for (let index = 0; index < octets.length; index += 1) {
    octets[index] = Number.parseInt(text.slice(index * 2, index * 2 + 2), 16);
  }
  return octets;
}

// ElementPadding::Packed: each collection element counts its own alignment
// from its own first octet, so the second Cell's long lands at offset six.
{
  const writer = new Writer(Endianness.Little, 4);
  let saved = writer.beginPackedElement();
  writer.writeI32(1);
  writer.writeI16(2);
  writer.finishPackedElement(saved);
  saved = writer.beginPackedElement();
  writer.writeI32(3);
  writer.writeI16(4);
  writer.finishPackedElement(saved);
  writer.writeU8(119);
  expectHdds(
    "a packed array element starts where the last ended",
    hex(writer.finish()),
    "01000000020003000000040077",
  );

  const reader = new Reader(writer.finish(), Endianness.Little, 4);
  saved = reader.readerBeginPackedElement();
  const first = `${reader.readI32()},${reader.readI16()}`;
  reader.readerFinishPackedElement(saved);
  saved = reader.readerBeginPackedElement();
  const second = `${reader.readI32()},${reader.readI16()}`;
  reader.readerFinishPackedElement(saved);
  expectHdds(
    "a packed element reads back at its own origin",
    `${first},${second},${reader.readU8()}`,
    "1,2,3,4,119",
  );
}

// The same inside two @final members.
{
  const writer = new Writer(Endianness.Little, 4);
  let saved = writer.beginPackedElement();
  writer.writeI32(-1234567);
  writer.writeI16(-2);
  writer.finishPackedElement(saved);
  saved = writer.beginPackedElement();
  writer.writeI32(1);
  writer.writeI16(32767);
  writer.finishPackedElement(saved);
  expectHdds(
    "a packed aggregate counts alignment from its own start",
    hex(writer.finish()),
    "7929edfffeff01000000ff7f",
  );
}

// A sequence element's own length in DHEADER form: the 06000000 in front of
// 7929edfffeff is the element's six octets, not the encapsulation's.
{
  const writer = new Writer(Endianness.Little, 4);
  const header = writer.writeDheader();
  writer.writeI32(-1234567);
  writer.writeI16(-2);
  writer.finishDheader(header);
  expectHdds(
    "a sequence element carries its own length",
    hex(writer.finish()),
    "060000007929edfffeff",
  );
}

// WstringCodeUnits::Utf8: the characters' own octets, the octet count with the
// terminator, and C2 still reached.
{
  const writer = new Writer(Endianness.Little, 4);
  writer.writeWstringUtf8Octets("abc", WstringLengthPolicy.OctetsTerminated2);
  expectHdds(
    "a UTF-8 wstring is its octets and a NUL",
    hex(writer.finish()),
    "0400000061626300",
  );
}
{
  const writer = new Writer(Endianness.Little, 4);
  writer.writeWstringUtf8Octets(
    "äöü ሴ Z",
    WstringLengthPolicy.OctetsTerminated2,
  );
  expectHdds(
    "a multi-byte UTF-8 wstring counts its octets",
    hex(writer.finish()),
    "0d000000c3a4c3b6c3bc20e188b4205a00",
  );
}
{
  const writer = new Writer(Endianness.Little, 4);
  writer.writeWstringUtf8OctetsBounded(
    "ሴäZ",
    3,
    WstringLengthPolicy.OctetsTerminated2,
  );
  expectHdds(
    "a bounded UTF-8 wstring holds three units",
    hex(writer.finish()),
    "07000000e188b4c3a45a00",
  );
}
{
  const reader = new Reader(
    unhex("0d000000c3a4c3b6c3bc20e188b4205a00"),
    Endianness.Little,
    4,
  );
  expectHdds(
    "a UTF-8 wstring reads back as its octets",
    reader.readWstringUtf8Octets(WstringLengthPolicy.OctetsTerminated2),
    "äöü ሴ Z",
  );
}

// Malformed UTF-8 and a count the stream cannot hold are refused, never
// replaced or truncated.
expectHddsThrows("a lone surrogate is not written as UTF-8 octets", () => {
  new Writer(Endianness.Little, 4).writeWstringUtf8Octets(
    "\ud834",
    WstringLengthPolicy.OctetsTerminated2,
  );
});
expectHddsThrows("a wstring length past the buffer", () => {
  new Reader(unhex("ff0000006100"), Endianness.Little, 4).readWstringUtf8Octets(
    WstringLengthPolicy.OctetsTerminated2,
  );
});

console.log(`${hddsChecks - hddsFailures} of ${hddsChecks} HDDS-axis checks hold`);
process.exit(failures + hddsFailures === 0 ? 0 : 1);
