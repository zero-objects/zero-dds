// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Node wire runtime's self-test for the three axes `Dialect::Hdds`
// selects.
//
//   node runtime/node/selftest.js
//
// Silent and exit 0 is the pass, as in `runtime/c/tests/selftest.c` and
// `runtime/java/src/test/java/org/zerodds/wire/WireSelfTest.java`. Every
// expectation here is a byte string, not a round trip: a writer and a reader
// that share a misreading of §7.4.1 agree with each other perfectly.
//
// Expected octets from the recorded HDDS columns:
// tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin,
// tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin,
// tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
// tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin.

"use strict";

const {
  Endianness,
  ParameterRole,
  Reader,
  Status,
  Writer,
  WstringLengthPolicy,
  bitPatternInt,
} = require("./zdwire.js");

let checks = 0;
let failures = 0;

function hex(octets) {
  return Array.from(octets)
    .map((octet) => octet.toString(16).padStart(2, "0"))
    .join("");
}

function expect(what, got, want) {
  checks += 1;
  if (got !== want) {
    failures += 1;
    console.log(`FAIL ${what}: ${got} is not ${want}`);
  }
}

function expectThrows(what, body) {
  checks += 1;
  try {
    body();
  } catch {
    return;
  }
  failures += 1;
  console.log(`FAIL ${what}: nothing was raised`);
}

function unhex(text) {
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
  expect(
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
  expect(
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
  expect(
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
  expect(
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
  expect(
    "a UTF-8 wstring is its octets and a NUL",
    hex(writer.finish()),
    "0400000061626300",
  );
}
{
  const writer = new Writer(Endianness.Little, 4);
  writer.writeWstringUtf8Octets("äöü ሴ Z", WstringLengthPolicy.OctetsTerminated2);
  expect(
    "a multi-byte UTF-8 wstring counts its octets",
    hex(writer.finish()),
    "0d000000c3a4c3b6c3bc20e188b4205a00",
  );
}
{
  const writer = new Writer(Endianness.Little, 4);
  writer.writeWstringUtf8OctetsBounded("ሴäZ", 3, WstringLengthPolicy.OctetsTerminated2);
  expect(
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
  expect(
    "a UTF-8 wstring reads back as its octets",
    reader.readWstringUtf8Octets(WstringLengthPolicy.OctetsTerminated2),
    "äöü ሴ Z",
  );
}

// Malformed UTF-8 and a count the stream cannot hold are refused, never
// replaced or truncated.
expectThrows("a lone surrogate is not written as UTF-8 octets", () => {
  new Writer(Endianness.Little, 4).writeWstringUtf8Octets(
    "\ud834",
    WstringLengthPolicy.OctetsTerminated2,
  );
});
expectThrows("a wstring length past the buffer", () => {
  new Reader(unhex("ff0000006100"), Endianness.Little, 4).readWstringUtf8Octets(
    WstringLengthPolicy.OctetsTerminated2,
  );
});

// -- the operations no corpus case reaches ----------------------------------
//
// The five the generated code reaches only in a dialect or with a member no
// corpus case carries: `writePidExtended`, `writeUnionOverlay`,
// `readerClearDiscard`, `readerSave` and `readerRestore`
// (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
// vectors, each against an oracle outside this code.
{
  // The first parameter of `annotation-autoid-sequential` as Cyclone DDS
  // recorded it, XCDR1 little endian (tests/interop-corpus/expected-bytes.tsv).
  const writer = new Writer(Endianness.Little, 8);
  const header = writer.writePidExtended(0, ParameterRole.Member, false);
  writer.writeI32(-1234567);
  writer.finishPid(header);
  expect(
    "writePidExtended: Cyclone's recorded parameter",
    hex(writer.finish()),
    "017f0800" + "00000000" + "04000000" + "7929edff",
  );
}
{
  // A `short` read through a four-octet and a one-octet `default:` branch:
  // Cyclone DDS 11.0.1 writes the union's storage, the member's octets with
  // zeros above (`combined-everything`, crates/wire-codec/src/encode.rs
  // `write_selected_branch`).
  const writer = new Writer(Endianness.Little, 4);
  const pattern = bitPatternInt(-2n, 2);
  writer.writeUnionOverlay(pattern, 4);
  writer.writeUnionOverlay(pattern, 1);
  expect("writeUnionOverlay: the storage at the branch's width", hex(writer.finish()), "feff0000" + "fe");
}
{
  // XTypes 1.3 §7.2.2.4.4.4.9: a member's explicit @default comes before
  // Table 9's value, so the call site asks for DISCARD and takes it back.
  // Any other refusal is not a narrowing and stays.
  const discarded = new Reader(unhex("01020304"), Endianness.Little, 4);
  discarded.readerRefuse(Status.Discarded);
  expect("readerClearDiscard takes back a Discarded", discarded.readerClearDiscard(), true);
  expect("the status after readerClearDiscard", discarded.readerStatus(), Status.Ok);
  const refused = new Reader(unhex("01020304"), Endianness.Little, 4);
  refused.readerRefuse(Status.OutOfRange);
  expect("readerClearDiscard leaves an OutOfRange", refused.readerClearDiscard(), false);
  expect("an OutOfRange stays", refused.readerStatus(), Status.OutOfRange);
}
{
  // XTypes 1.3 §7.4.3.5.3 rule (15): a delimited map<long, long> as its
  // DHEADER and the pairs, without a count. The reader counts the pairs, goes
  // back and reads them again; a refusal made while counting goes back with
  // the cursor.
  const reader = new Reader(
    unhex("10000000" + "01000000" + "0a000000" + "02000000" + "14000000"),
    Endianness.Little,
    4,
  );
  const extent = reader.readDheader();
  const end = reader.readerPosition() + extent;
  const mark = reader.readerSave();
  let pairs = 0;
  while (reader.readerPosition() < end && reader.readerStatus() === Status.Ok) {
    reader.readI32();
    reader.readI32();
    pairs += 1;
  }
  reader.readerRefuse(Status.OutOfRange);
  reader.readerRestore(mark);
  expect("the counting pass finds two pairs", pairs, 2);
  expect(
    "the pairs read again",
    [reader.readI32(), reader.readI32(), reader.readI32(), reader.readI32()].join(","),
    "1,10,2,20",
  );
  expect("the end of the map", reader.readerPosition(), 20);
  expect("readerRestore takes the refusal back", reader.readerStatus(), Status.Ok);
}

if (failures !== 0) {
  console.log(`${failures} of ${checks} checks failed`);
  process.exit(1);
}
