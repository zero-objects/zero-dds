// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The reader, against the sample each case declares.
 *
 * The counterpart of `against-recorded-octets.ts` and deliberately not
 * its mirror. The write side is held against
 * `tests/interop-results/<case>/zerodds-rust/<encoding>.<endian>.bin`;
 * this one reads those same octets and holds what comes back against
 * `tests/interop-corpus/<case>/values.json` — **the case's own sample**,
 * which is what the octets were produced from.
 *
 * Not against what the Rust probe reads back from its own bytes: two
 * decoders that are wrong in the same way agree with each other, and a
 * round trip through one column proves only that the column is
 * self-consistent. `values.json` is the one statement of the value that
 * no decoder produced.
 *
 * The samples are decoded by hand here, because the generator for this
 * column does not exist yet: this is what proves the read half before
 * anything is built on top of it.
 *
 * The exit code carries the class, `docs/specs/zerodds-verdicts-1.0.md`:
 * 0 where every case came back, 6 where this harness has not
 * implemented something, and a bare non-zero for a disagreement, which
 * is `refused` with the first line as its evidence.
 */

import { readFileSync, existsSync } from "node:fs";
import { Endianness } from "../src/endianness.js";
import {
  FixedDigitCount,
  ParameterRole,
  Status,
  WcharFraming,
  WcharWidth,
  WstringLengthPolicy,
  WstringUnitOrder,
} from "../src/policy.js";
import { Reader } from "../src/reader.js";

// The runtime itself pulls in nothing from Node — it has to run in a
// browser too — so the two globals this *test* needs are declared here
// rather than through a dependency on `@types/node`.
declare const process: { exit(code: number): never };

/** The repository root, found by walking up until the corpus is there. */
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

function octets(caseName: string, encoding: string, order: string): Uint8Array {
  const path = `${ROOT}tests/interop-results/${caseName}/zerodds-rust/${encoding}.${order}.bin`;
  return new Uint8Array(readFileSync(path));
}

function sample(caseName: string): Record<string, unknown> {
  const path = `${ROOT}tests/interop-corpus/${caseName}/values.json`;
  return JSON.parse(readFileSync(path, "utf-8")) as Record<string, unknown>;
}

/**
 * What a decoded value is compared as.
 *
 * `JSON.stringify` will not carry a `bigint`, and the corpus states the
 * two 64-bit members as decimal *strings* for exactly that reason — so
 * a `bigint` is compared as its decimal and not as a number that has
 * already lost its low bits.
 */
function same(got: unknown, want: unknown): boolean {
  if (Array.isArray(got) && Array.isArray(want)) {
    return (
      got.length === want.length &&
      got.every((element, index) => same(element, want[index]))
    );
  }
  if (typeof got === "bigint") {
    return got.toString() === String(want);
  }
  if (typeof got === "number" && typeof want === "number") {
    // Both sides come from the same binary64, so this is equality and
    // not a tolerance: a `float` member is rounded once by the writer
    // and read back as that rounding.
    return Object.is(got, want) || got === want;
  }
  return got === want;
}

interface Case {
  readonly name: string;
  readonly encodings: readonly (readonly [string, number])[];
  readonly read: (r: Reader, encoding: string) => Record<string, unknown>;
}

/** `primitive-mixed`: fourteen scalars and the alignment between them. */
function primitiveMixed(r: Reader): Record<string, unknown> {
  return {
    flag: r.readBool(),
    raw_octet: r.readU8(),
    unsigned_byte: r.readU8(),
    signed_byte: r.readI8(),
    letter: r.readChar(),
    wide_letter: r.readWchar(
      WcharWidth.Octets2,
      WcharFraming.Fixed,
      WstringUnitOrder.Stream,
    ),
    short_signed: r.readI16(),
    short_unsigned: r.readU16(),
    long_signed: r.readI32(),
    long_unsigned: r.readU32(),
    llong_signed: r.readI64(),
    llong_unsigned: r.readU64(),
    single_float: r.readF32(),
    double_float: r.readF64(),
  };
}

/**
 * `struct-appendable`: the delimiter a non-final aggregate carries.
 *
 * §7.4.3.5.3 gives the DHEADER to `@appendable` in encoding version 2
 * only, so under XCDR1 there is none and a reader that consumed four
 * octets for one would read every member from the wrong offset.
 */
function structAppendable(r: Reader, encoding: string): Record<string, unknown> {
  if (encoding !== "plain-cdr1") {
    // Read and not kept: the member count is the type's, so the
    // delimiter moves the cursor and says nothing this decoder needs.
    r.readDheader();
  }
  return { a: r.readI32(), b: r.readF64(), c: r.readI16() };
}

/** `string-bounded-utf8`: a bound counted in characters, not octets. */
function stringBoundedUtf8(r: Reader): Record<string, unknown> {
  return { text: r.readString(), tail: r.readI32() };
}

/**
 * `enum-sparse-values`: values that are not ordinal positions.
 *
 * The decoder compares the holder's pattern against each declared
 * enumerator, which is what a generated one does — §7.4.3.5.3 rule (5)
 * makes the holder signed and `readEnum` sign-extends, so the arms are
 * the declared values taken the same way.
 */
function enumSparseValues(r: Reader): Record<string, unknown> {
  const names = new Map<bigint, string>([
    [1n, "OK"],
    [100n, "WARN"],
    [65535n, "FAIL"],
  ]);
  const one = () => names.get(r.readEnum(32)) ?? "<no enumerator>";
  return { a: one(), b: one(), c: one() };
}

/**
 * `array-1d-primitives`: every scalar kind, three of each.
 *
 * §7.4.3.5 puts an array's elements on the wire as one flat run with no
 * count in front of them — the length is in the type — so what this
 * measures beside the scalar reads is that nothing consumed a count.
 */
function array1dPrimitives(r: Reader): Record<string, unknown> {
  const three = <T>(read: () => T): T[] => [read(), read(), read()];
  return {
    flag: three(() => r.readBool()),
    raw_octet: three(() => r.readU8()),
    unsigned_byte: three(() => r.readU8()),
    signed_byte: three(() => r.readI8()),
    letter: three(() => r.readChar()),
    wide_letter: three(() =>
      r.readWchar(WcharWidth.Octets2, WcharFraming.Fixed, WstringUnitOrder.Stream),
    ),
    short_signed: three(() => r.readI16()),
    short_unsigned: three(() => r.readU16()),
    long_signed: three(() => r.readI32()),
    long_unsigned: three(() => r.readU32()),
    llong_signed: three(() => r.readI64()),
    llong_unsigned: three(() => r.readU64()),
    single_float: three(() => r.readF32()),
    double_float: three(() => r.readF64()),
  };
}

/** `sequence-bounded-full`: a count, then that many elements. */
function sequenceBoundedFull(r: Reader): Record<string, unknown> {
  const count = r.readSequenceLength();
  const ids: number[] = [];
  for (let index = 0; index < count; index += 1) {
    ids.push(r.readI32());
  }
  return { ids, tail: r.readU8() };
}

/**
 * `wstring-bounded-utf8`: §7.4.1.1.2's octet count over UTF-16 units,
 * with no terminator and no byte-order mark.
 */
function wstringBoundedUtf8(r: Reader): Record<string, unknown> {
  return {
    text: r.readWstring(
      WstringLengthPolicy.Specification,
      WstringUnitOrder.Stream,
    ),
    tail: r.readI32(),
  };
}

/**
 * `struct-mutable` under PL_CDR1: a parameter list walked by its ids.
 *
 * The loop is the one §7.4.1.2 describes and not a fixed sequence of
 * three reads: a parameter list states its members' ids, so the order
 * on the wire is the writer's and a reader that assumed one would break
 * on the first peer that reordered.
 */
function structMutablePlCdr1(r: Reader): Record<string, unknown> {
  const out: Record<string, unknown> = { a: 0, b: 0, c: 0 };
  for (let guard = 0; guard < 64; guard += 1) {
    const header = r.readPidAs(ParameterRole.Member);
    if (header.end) {
      break;
    }
    if (header.pid === 0) {
      out["a"] = r.readI32();
    } else if (header.pid === 1) {
      out["b"] = r.readF64();
    } else if (header.pid === 2) {
      out["c"] = r.readI16();
    } else {
      // A parameter this type has no member for. PL_CDR1 carries a
      // length on every one, so stepping over it is always possible.
      r.readerSkipTo(r.readerPosition() + header.length);
    }
    r.finishPidRead(header);
  }
  return out;
}

/**
 * `struct-mutable` under PL_CDR2: a DHEADER, then one EMHEADER a
 * member. §7.4.3.4.2.
 */
function structMutablePlCdr2(r: Reader): Record<string, unknown> {
  const out: Record<string, unknown> = { a: 0, b: 0, c: 0 };
  const length = r.readDheader();
  const end = r.readerPosition() + length;
  while (r.readerPosition() < end && r.readerStatus() === Status.Ok) {
    const header = r.readEmheader();
    const at = r.readerPosition();
    if (header.memberId === 0) {
      out["a"] = r.readI32();
    } else if (header.memberId === 1) {
      out["b"] = r.readF64();
    } else if (header.memberId === 2) {
      out["c"] = r.readI16();
    } else {
      r.skipMember(header);
      continue;
    }
    // The header stated the extent; the member's own read is held
    // against it rather than trusted to have agreed. A member that read
    // fewer octets than the header framed is a length code this decoder
    // has misunderstood, and carrying on would take the next EMHEADER
    // out of the middle of a value.
    r.readerSkipTo(at + header.extent);
  }
  return out;
}

const CASES: Case[] = [
  {
    name: "primitive-mixed",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => primitiveMixed(r),
  },
  {
    name: "struct-appendable",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r, encoding) => structAppendable(r, encoding),
  },
  {
    name: "string-bounded-utf8",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => stringBoundedUtf8(r),
  },
  {
    name: "enum-sparse-values",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => enumSparseValues(r),
  },
  {
    name: "array-1d-primitives",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => array1dPrimitives(r),
  },
  {
    name: "sequence-bounded-full",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => sequenceBoundedFull(r),
  },
  {
    name: "wstring-bounded-utf8",
    encodings: [
      ["plain-cdr1", 8],
      ["plain-cdr2", 4],
    ],
    read: (r) => wstringBoundedUtf8(r),
  },
  {
    name: "struct-mutable",
    encodings: [["pl-cdr1", 8]],
    read: (r) => structMutablePlCdr1(r),
  },
  {
    name: "struct-mutable",
    encodings: [["pl-cdr2", 4]],
    read: (r) => structMutablePlCdr2(r),
  },
];

let checked = 0;
let failures = 0;
let gaps = 0;

for (const check of CASES) {
  const want = sample(check.name);
  for (const [encoding, maxAlign] of check.encodings) {
    for (const order of [Endianness.Little, Endianness.Big] as const) {
      checked += 1;
      let got: Record<string, unknown>;
      const reader = new Reader(
        octets(check.name, encoding, order),
        order,
        maxAlign,
      );
      try {
        got = check.read(reader, encoding);
      } catch (error) {
        failures += 1;
        console.log(`${check.name} ${encoding}/${order}: ${String(error)}`);
        continue;
      }
      if (reader.readerStatus() !== Status.Ok) {
        failures += 1;
        console.log(
          `${check.name} ${encoding}/${order}: the reader refused with ${reader.readerStatus()}`,
        );
        continue;
      }
      const wrong = Object.keys(want).filter(
        (field) => !same(got[field], want[field]),
      );
      // A field the sample states and the decoder did not produce is a
      // gap in this harness and not a disagreement about the wire; the
      // two carry different exit codes.
      const missing = Object.keys(want).filter((field) => !(field in got));
      if (missing.length > 0) {
        gaps += 1;
        console.log(
          `${check.name} ${encoding}/${order}: this harness reads no ${missing.join(", ")}`,
        );
        continue;
      }
      if (wrong.length > 0) {
        failures += 1;
        console.log(`${check.name} ${encoding}/${order}`);
        for (const field of wrong) {
          console.log(
            `  ${field}: read ${String(got[field])}, the sample says ${String(want[field])}`,
          );
        }
      }
      // The whole sample and nothing after it. A decoder that stopped
      // early reads every field correctly and leaves the tail, which is
      // the one defect a field-by-field comparison cannot see.
      if (
        wrong.length === 0 &&
        missing.length === 0 &&
        reader.readerRemaining() !== 0
      ) {
        failures += 1;
        console.log(
          `${check.name} ${encoding}/${order}: ${reader.readerRemaining()} octets left after the sample`,
        );
      }
    }
  }
}

// One reference to each policy this column carries but no case above
// reaches, so that an enumeration losing a member is a compile error
// here rather than a case that quietly stops being covered.
void [FixedDigitCount.Declared, WstringUnitOrder.ByteOrderMark];

console.log(`${checked - failures - gaps} of ${checked} read back as the sample`);
if (gaps > 0) {
  console.log(`${gaps} of ${checked} are gaps in this harness, not in the runtime`);
  process.exit(6);
}
process.exit(failures === 0 ? 0 : 1);
