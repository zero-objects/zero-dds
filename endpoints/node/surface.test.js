// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The wire surface, against octets this project did not write.
//
// Every expected string below is a row of
// `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`, which holds what
// Cyclone DDS, RTI Connext, Fast DDS and OpenDDS each produced from their own
// serialiser for the cases under `tools/byte-gate/cases/`. The vendor and the
// row are named at every check, so a change here that agreed with itself and
// not with a foreign implementation fails.
//
// Where the vendors disagree the divergence is written down rather than
// averaged away: a case with three answers is three findings, and this SDK
// follows a named one.
//
// Each operation is exercised at the ends of its range and not only in the
// middle, because that is where the last four defects in these bindings were.

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const z = require('./zerodds.js');

const hex = (b) => Buffer.from(b).toString('hex');
const unhex = (s) => Buffer.from(s, 'hex');

// --- the encoding decides MAXALIGN ---------------------------------------

test('ext_final: the encoding decides where the double lands', () => {
  // byte-gate `ext_final`: `@final struct Ext { long a; double b; short c; }`
  // with a = 7, b = 1.5, c = -3. Cyclone, RTI and Fast DDS agree on all four
  // rows. The `double` aligns to eight under XCDR1 and to four under XCDR2, so
  // the same sample is 18 octets in one encoding and 14 in the other — which is
  // the whole of what the encoding argument buys.
  for (const [endian, encoding, want] of [
    [z.LITTLE, z.XCDR1, '0700000000000000000000000000f83ffdff'],
    [z.LITTLE, z.XCDR2, '07000000000000000000f83ffdff'],
    [z.BIG, z.XCDR1, '00000007000000003ff8000000000000fffd'],
    [z.BIG, z.XCDR2, '000000073ff8000000000000fffd'],
  ]) {
    const w = new z.Writer(endian, encoding);
    w.putU32(7);
    w.putF64(1.5);
    w.putU16(-3);
    assert.strictEqual(hex(w.bytes()), want, 'cyclone+rti+fastdds');
  }
});

test('the encoding has no default', () => {
  // A stream that did not say which encoding it is in would be XCDR2 by
  // accident. JavaScript has no type to enforce the argument, so the
  // constructor does.
  assert.throws(() => new z.Writer(z.LITTLE), TypeError);
  assert.throws(() => new z.Reader(Buffer.alloc(0), z.LITTLE), TypeError);
  assert.strictEqual(new z.Writer(z.LITTLE, z.XCDR1).maxAlign, 8);
  assert.strictEqual(new z.Writer(z.LITTLE, z.XCDR2).maxAlign, 4);
});

// --- DHEADER --------------------------------------------------------------

test('ext_appendable: the DHEADER counts the body and not itself', () => {
  // byte-gate `ext_appendable`: the same members under `@appendable`, which
  // XTypes 1.3 §7.3.1.2.1.8 makes the default extensibility. Under XCDR2 a
  // DHEADER precedes the body and counts it — 0x0e = 14 over an 18-octet
  // sample. Cyclone, RTI and Fast DDS agree on both rows.
  for (const [endian, want] of [
    [z.LITTLE, '0e00000007000000000000000000f83ffdff'],
    [z.BIG, '0000000e000000073ff8000000000000fffd'],
  ]) {
    const w = new z.Writer(endian, z.XCDR2);
    const d = w.dheaderBegin();
    w.putU32(7);
    w.putF64(1.5);
    w.putU16(-3);
    w.dheaderEnd(d);
    assert.strictEqual(hex(w.bytes()), want, 'cyclone+rti+fastdds');
  }
});

// --- the member header ----------------------------------------------------

test('ext_mutable: the EMHEADER in the length codes the vendors chose', () => {
  // byte-gate `ext_mutable` under XCDR2: all three vendors write the same 30
  // octets. The member headers are LC2, LC3 and LC1 — the length is in the code
  // and no NEXTINT is written for any of them — inside a DHEADER of 26.
  for (const [endian, want] of [
    [z.LITTLE, '1a000000000000200700000001000030000000000000f83f02000010fdff'],
    [z.BIG, '0000001a2000000000000007300000013ff800000000000010000002fffd'],
  ]) {
    const w = new z.Writer(endian, z.XCDR2);
    const d = w.dheaderBegin();
    let m = w.emheaderBegin(0, false, z.LC2);
    w.putU32(7);
    w.emheaderEnd(m, z.LC2);
    m = w.emheaderBegin(1, false, z.LC3);
    w.putF64(1.5);
    w.emheaderEnd(m, z.LC3);
    m = w.emheaderBegin(2, false, z.LC1);
    w.putU16(-3);
    w.emheaderEnd(m, z.LC1);
    w.dheaderEnd(d);
    assert.strictEqual(hex(w.bytes()), want, 'cyclone+rti+fastdds');
  }
});

test('mut_mustunderstand: the flag is bit 31 of the EMHEADER', () => {
  // `mut_mustunderstand` under XCDR2 reads `010000a0` for id 1 at LC2 with the
  // flag — Cyclone and RTI both write it. Fast DDS writes `01000020` for the
  // same member, i.e. it drops the flag; two of three carry it and this SDK
  // carries it.
  const w = new z.Writer(z.LITTLE, z.XCDR2);
  const d = w.dheaderBegin();
  let m = w.emheaderBegin(1, true, z.LC2);
  w.putU32(7);
  w.emheaderEnd(m, z.LC2);
  m = w.emheaderBegin(2, false, z.LC2);
  w.putU32(-2);
  w.emheaderEnd(m, z.LC2);
  w.dheaderEnd(d);
  assert.strictEqual(hex(w.bytes()), '10000000010000a00700000002000020feffffff',
    'cyclone+rti; fastdds drops bit 31');
});

test('the member header refuses what it cannot frame', () => {
  // The top of the range: 0x0FFFFFFF fits and 0x10000000 does not. A truncating
  // header would put a different member's id on the wire.
  new z.Writer(z.LITTLE, z.XCDR2).emheaderBegin(0x0fffffff, false, z.LC2);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).emheaderBegin(0x10000000, false, z.LC2));
  assert.throws(() => {
    const w = new z.Writer(z.LITTLE, z.XCDR2);
    const m = w.emheaderBegin(0, false, z.LC2);
    w.putU16(1);
    w.emheaderEnd(m, z.LC2);
  }, /LC2 frames 4 octets and the member wrote 2/);
});

// --- the PL_CDR1 parameter list -------------------------------------------

test('ext_mutable under XCDR1: the parameter list, in RTI\'s form', () => {
  // byte-gate `ext_mutable` under XCDR1, RTI's row. Three things it pins: the
  // `double`'s parameter declares 8 and not 12, because its value aligns
  // against its own block; the `short`'s declares 4 and not 2, because the
  // padding that brings the next header back to a four-octet boundary is inside
  // the length; and the sentinel carries the must-understand flag, so it is
  // 0x7F02.
  //
  // The vendors do not agree here and the disagreement is the measurement:
  // Cyclone writes every parameter in the twelve-octet extended form (56 octets
  // for this sample), Fast DDS declares 2 for the `short` and closes with the
  // bare 0x3F02 (32 octets, differing from RTI in two places).
  for (const [endian, want] of [
    [z.LITTLE, '000004000700000001000800000000000000f83f02000400fdff0000027f0000'],
    [z.BIG, '0000000400000007000100083ff800000000000000020004fffd00007f020000'],
  ]) {
    const w = new z.Writer(endian, z.XCDR1);
    let p = w.pidBegin(0, false);
    w.putU32(7);
    w.pidEnd(p);
    p = w.pidBegin(1, false);
    w.putF64(1.5);
    w.pidEnd(p);
    p = w.pidBegin(2, false);
    w.putU16(-3);
    w.pidEnd(p);
    w.pidListEnd();
    assert.strictEqual(hex(w.bytes()), want, 'rti; cyclone extended, fastdds len 2 + 3f02');
  }
});

test('mut_mustunderstand under XCDR1: the flag is bit 14 of the id', () => {
  // RTI's row reads `0140` for id 1 with the flag, and Cyclone sets the same
  // bit inside its extended header. Fast DDS writes `0100` — no flag.
  const w = new z.Writer(z.LITTLE, z.XCDR1);
  let p = w.pidBegin(1, true);
  w.putU32(7);
  w.pidEnd(p);
  p = w.pidBegin(2, false);
  w.putU32(-2);
  w.pidEnd(p);
  w.pidListEnd();
  assert.strictEqual(hex(w.bytes()), '014004000700000002000400feffffff027f0000',
    'rti; fastdds drops bit 14');
});

test('pid_3f00: 0x3F00 is the last id the short header carries', () => {
  // byte-gate `pid_3f00`: `@id(16127)` and `@id(16128)`, i.e. 0x3EFF and
  // 0x3F00. RTI and Fast DDS both still use the short header for 0x3F00, so the
  // boundary is inclusive — and one past it is refused rather than truncated.
  const w = new z.Writer(z.LITTLE, z.XCDR1);
  let p = w.pidBegin(0x3eff, false);
  w.putU32(7);
  w.pidEnd(p);
  p = w.pidBegin(0x3f00, false);
  w.putU32(9);
  w.pidEnd(p);
  w.pidListEnd();
  assert.strictEqual(hex(w.bytes()), 'ff3e040007000000003f040009000000027f0000',
    'rti+fastdds; cyclone extended');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR1).pidBegin(0x3f01, false));
});

// --- the tail is not padded ------------------------------------------------

test('tail_pad: a sample is not rounded up at the end', () => {
  // byte-gate `tail_pad`: `@final struct Odd { long a; short b; }` with 1 and 2
  // is six octets in all four rows and all three vendors. A writer that rounded
  // the sample up to a four-octet boundary would add two.
  for (const [endian, want] of [[z.LITTLE, '010000000200'], [z.BIG, '000000010002']]) {
    for (const encoding of [z.XCDR1, z.XCDR2]) {
      const w = new z.Writer(endian, encoding);
      w.putU32(1);
      w.putU16(2);
      assert.strictEqual(hex(w.bytes()), want, 'cyclone+rti+fastdds');
    }
  }
});

// --- the wide character and string ----------------------------------------

test('wchar_member: two octets in the stream\'s order', () => {
  // byte-gate `wchar_member`: `@final struct WCh { wchar c; octet tail; }` with
  // U+0061 and 0xAB. Cyclone and Fast DDS write a two-octet code unit in both
  // encodings; RTI writes four octets under XCDR1 and two under XCDR2. This SDK
  // writes two octets always — the majority form, and RTI's own XCDR2 form.
  for (const [endian, want] of [[z.LITTLE, '6100ab'], [z.BIG, '0061ab']]) {
    for (const encoding of [z.XCDR1, z.XCDR2]) {
      const w = new z.Writer(endian, encoding);
      w.putWchar(0x0061);
      w.putU8(0xab);
      assert.strictEqual(hex(w.bytes()), want, 'cyclone+fastdds; rti 4 octets under XCDR1');
    }
  }
  // U+FFFF is the last one that fits and U+10000 the first that does not.
  new z.Writer(z.LITTLE, z.XCDR2).putWchar(0xffff);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putWchar(0x10000));
});

test('wstring_member: the length word counts octets', () => {
  // byte-gate `wstring_member`: `wstring w; octet tail;` with the two units
  // U+0061 U+0062. Three vendors, three answers, and the length word is the
  // axis they split on:
  //
  //   cyclone  04000000 6100 6200 ab  — the length counts OCTETS
  //   fastdds  02000000 6100 6200 ab  — the length counts CODE UNITS
  //   rti      03000000 ...           — units incl. a terminator, four octets
  //            wide, under XCDR1; under XCDR2 RTI writes the octet count.
  //
  // This SDK writes the octet count with two-octet units and no terminator.
  //
  // A fourth axis separates them in a big-endian stream, and only there:
  // Cyclone's row is `00000004 6100 6200 ab` — the length word is swapped and
  // the code units are not, so its wide units go out little-endian whatever the
  // stream says. RTI's row is `00000004 0061 0062 ab`, every word in the
  // stream's order. Cyclone swaps a standalone `wchar` (its `wchar_member` row
  // is `0061ab`) and does not swap the elements of a `wstring`, which is a
  // disagreement inside one vendor. This SDK writes every unit in the stream's
  // order — RTI's form, and Cyclone's own form for a `wchar`.
  for (const [endian, want] of [
    [z.LITTLE, '0400000061006200ab'],  // cyclone + rti agree
    [z.BIG, '0000000400610062ab'],     // rti; cyclone leaves the units LE
  ]) {
    const w = new z.Writer(endian, z.XCDR2);
    w.putWstring([0x0061, 0x0062]);
    w.putU8(0xab);
    assert.strictEqual(hex(w.bytes()), want, 'cyclone+rti; fastdds counts units');
  }
  // The empty one is a length of zero and nothing after it.
  const e = new z.Writer(z.LITTLE, z.XCDR2);
  e.putWstring([]);
  assert.strictEqual(hex(e.bytes()), '00000000');
});

test('wstring_nonbmp: a surrogate pair is two units, not a truncation', () => {
  // byte-gate `wstring_nonbmp`: U+1D11E, which is one code point and two UTF-16
  // code units. Cyclone writes the surrogate pair and a length of four octets.
  // Fast DDS writes `010000001ed1ab` — one unit 0xD11E, the low sixteen bits of
  // the code point: a silent truncation that produces a frame of the right
  // shape holding a different character.
  const w = new z.Writer(z.LITTLE, z.XCDR2);
  w.putWstring([0xd834, 0xdd1e]);
  w.putU8(0xab);
  assert.strictEqual(hex(w.bytes()), '0400000034d81eddab',
    'cyclone+rti; fastdds truncates to 0xD11E');
  // The same sample big-endian, against RTI's row: every unit in the stream's
  // order. Cyclone's row keeps the units little-endian here too.
  const be = new z.Writer(z.BIG, z.XCDR2);
  be.putWstring([0xd834, 0xdd1e]);
  be.putU8(0xab);
  assert.strictEqual(hex(be.bytes()), '00000004d834dd1eab',
    'rti; cyclone leaves the units LE');
});

test('char is one octet and refuses what does not fit', () => {
  // U+0161's low octet is 0x61, which is `a`: a truncating writer produces a
  // frame of the right length holding a plausible, wrong character.
  new z.Writer(z.LITTLE, z.XCDR2).putChar(0xff);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putChar(0x0161));
});

// --- the 64-bit float ------------------------------------------------------

test('putF64 writes the bit pattern', () => {
  // 1.5 is `ext_final`'s `b` and is checked above against three vendors; the
  // rest are the ends of the exponent range, where a narrowing to `float` would
  // show.
  for (const [value, want] of [
    [1.5, '000000000000f83f'],
    [0, '0000000000000000'],
    [3.141592653589793, '182d4454fb210940'],
    [Number.MAX_VALUE, 'ffffffffffffef7f'],
    [Number.MIN_VALUE, '0100000000000000'], // the smallest subnormal
  ]) {
    const w = new z.Writer(z.LITTLE, z.XCDR2);
    w.putF64(value);
    assert.strictEqual(hex(w.bytes()), want, `putF64(${value})`);
  }
});

// --- the ends of every integer range ---------------------------------------

test('every width refuses a value it would have to truncate', () => {
  // A width that truncates is the defect this SDK carried: `putU16(70000)`
  // wrote `7011`, the low sixteen bits of 0x11170, with no error anywhere. Each
  // width is checked at 0, at -1, at the signed minimum, at the unsigned
  // maximum and one past it.
  const w8 = new z.Writer(z.LITTLE, z.XCDR2);
  w8.putU8(0); w8.putU8(255); w8.putU8(-128); w8.putU8(-1);
  assert.strictEqual(hex(w8.bytes()), '00ff80ff');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU8(256), RangeError);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU8(-129), RangeError);

  const w16 = new z.Writer(z.LITTLE, z.XCDR2);
  w16.putU16(0); w16.putU16(65535); w16.putU16(-32768); w16.putU16(-1);
  assert.strictEqual(hex(w16.bytes()), '0000ffff0080ffff');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU16(65536), RangeError);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU16(70000), RangeError);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU16(-32769), RangeError);

  const w32 = new z.Writer(z.LITTLE, z.XCDR2);
  w32.putU32(0); w32.putU32(4294967295); w32.putU32(-2147483648); w32.putU32(-1);
  assert.strictEqual(hex(w32.bytes()), '00000000ffffffff00000080ffffffff');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU32(4294967296), RangeError);
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU32(-2147483649), RangeError);

  const w64 = new z.Writer(z.LITTLE, z.XCDR2);
  w64.putU64(0n); w64.putU64(0xffffffffffffffffn); w64.putU64(-(2n ** 63n));
  assert.strictEqual(hex(w64.bytes()),
    '0000000000000000' + 'ffffffffffffffff' + '0000000000000080');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU64(2n ** 64n), RangeError);
  // A number where a BigInt is required would round above 2^53 rather than
  // refuse, so the type is checked.
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putU64(1), TypeError);
});

// --- the packed decimal ----------------------------------------------------

test('fixed-basic: packed BCD, independent of the byte order', () => {
  // `tests/interop-corpus/fixed-basic` is `@final struct Sample { Money amount;
  // Money debt; octet tail; }` over `typedef fixed<9,3> Money`, with
  // "123456.789", "-000000.001" and 0xAB. P + 1 = 10 is even, so there is no
  // leading zero nibble and each value is five octets.
  //
  // `fixed<P,S>` is CORBA's and not DDS's, so the foreign octets come from an
  // ORB and not from the four DDS vendors: Cyclone, RTI, Fast DDS and OpenDDS
  // each refuse the type in their IDL front end, which is why `fixed_basic`
  // carries no `wrote` row in vendor_bytes.rows.tsv at all.
  //
  // omniORB wrote `123456789c000000001dab` in both byte orders
  // (tests/interop-results/fixed-basic/omniorb/plain-cdr1.{le,be}.bin), octet
  // for octet what this SDK writes — and the two orders being equal is the
  // measurement that no multi-octet integer is written here.
  //
  // JacORB wrote `123456789c1dab` for the same sample, big-endian only: the
  // first value agrees and the second is one octet rather than five, so it
  // packs the significant digits instead of the declared width. TAO produced no
  // bytes at all — `tao_idl` refused the IDL.
  const le = new z.Writer(z.LITTLE, z.XCDR1);
  le.putFixed('123456.789', 9, 3);
  le.putFixed('-000000.001', 9, 3);
  le.putU8(0xab);
  assert.strictEqual(hex(le.bytes()), '123456789c000000001dab',
    'omniorb; jacorb packs the second value to one octet');
  const be = new z.Writer(z.BIG, z.XCDR1);
  be.putFixed('123456.789', 9, 3);
  be.putFixed('-000000.001', 9, 3);
  be.putU8(0xab);
  assert.strictEqual(hex(be.bytes()), '123456789c000000001dab', 'omniorb');
});

test('fixed at the ends of its range', () => {
  for (const [value, digits, scale, want] of [
    ['1', 1, 0, '1c'],
    ['-9', 1, 0, '9d'],
    ['12', 2, 0, '012c'],         // P + 1 odd -> a leading zero nibble
    ['0.123', 3, 3, '123c'],      // scale == digits: all fraction
    ['-0', 1, 0, '0c'],           // a negative zero is written positive
    ['-0.000', 3, 3, '000c'],
    ['7.5', 9, 3, '000007500c'],  // padding on both sides at once
    ['1234567890123456789012345678901', 31, 0, '1234567890123456789012345678901c'],
    ['-1234567890123456789012345678901', 31, 0, '1234567890123456789012345678901d'],
  ]) {
    const w = new z.Writer(z.LITTLE, z.XCDR2);
    w.putFixed(value, digits, scale);
    assert.strictEqual(hex(w.bytes()), want, `putFixed(${value}, ${digits}, ${scale})`);
    assert.strictEqual(w.bytes().length, Math.floor((digits + 2) / 2));
  }
  for (const [value, digits, scale] of [
    ['1234567', 6, 0], // seven integer digits into six
    ['1.2345', 6, 3],  // four fraction digits into three
    ['1.0', 2, 3],     // a scale wider than the digit count
    ['twelve', 4, 0],  // not a decimal
    ['', 4, 0],        // no digits at all
  ]) {
    assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putFixed(value, digits, scale),
      RangeError, `putFixed(${value}, ${digits}, ${scale}) was accepted`);
  }
});

// --- the bitset anchor -----------------------------------------------------

test('an eight-octet anchor takes the encoding\'s alignment', () => {
  const one = new z.Writer(z.LITTLE, z.XCDR1);
  one.putU8(0xab);
  one.putBitset(1n, 8);
  assert.strictEqual(hex(one.bytes()), 'ab00000000000000' + '0100000000000000');
  const two = new z.Writer(z.LITTLE, z.XCDR2);
  two.putU8(0xab);
  two.putBitset(1n, 8);
  assert.strictEqual(hex(two.bytes()), 'ab000000' + '0100000000000000');
  assert.throws(() => new z.Writer(z.LITTLE, z.XCDR2).putBitset(0n, 3), RangeError);
});

// --- the reading direction -------------------------------------------------

test('the vendors\' own octets read back into the sample', () => {
  // `ext_appendable` under XCDR2 in both byte orders — the octets Cyclone, RTI
  // and Fast DDS all wrote, in through this SDK's reader.
  for (const [endian, wire] of [
    [z.LITTLE, '0e00000007000000000000000000f83ffdff'],
    [z.BIG, '0000000e000000073ff8000000000000fffd'],
  ]) {
    const bytes = unhex(wire);
    const r = new z.Reader(bytes, endian, z.XCDR2);
    assert.strictEqual(r.dheaderRead(), 14);
    assert.strictEqual(r.getU32(), 7);
    assert.strictEqual(r.getF64(), 1.5);
    assert.strictEqual(r.getU16(), 0xfffd);
    assert.strictEqual(r.pos, bytes.length);
  }
});

test('the member header reads back from the vendors\' octets', () => {
  const bytes = unhex('1a000000000000200700000001000030000000000000f83f02000010fdff');
  const r = new z.Reader(bytes, z.LITTLE, z.XCDR2);
  assert.strictEqual(r.dheaderRead(), 26);
  for (const [id, lc, extent] of [[0, z.LC2, 4], [1, z.LC3, 8], [2, z.LC1, 2]]) {
    const m = r.emheaderRead();
    assert.deepStrictEqual([m.id, m.lengthCode, m.extent], [id, lc, extent]);
    r.pos += m.extent;
  }
  assert.strictEqual(r.pos, bytes.length);
});

test('the parameter list reads back from RTI\'s octets', () => {
  const bytes = unhex('000004000700000001000800000000000000f83f02000400fdff0000027f0000');
  const r = new z.Reader(bytes, z.LITTLE, z.XCDR1);
  for (const [id, length] of [[0, 4], [1, 8], [2, 4]]) {
    const p = r.pidRead();
    assert.strictEqual(p.listEnd, false);
    assert.deepStrictEqual([p.id, p.length], [id, length]);
    r.pidSkip(r.pos, p.length);
  }
  assert.strictEqual(r.pidRead().listEnd, true);
  assert.strictEqual(r.pos, bytes.length);
});

test('every write reads back, in both encodings and both byte orders', () => {
  for (const encoding of [z.XCDR1, z.XCDR2]) {
    for (const endian of [z.LITTLE, z.BIG]) {
      const w = new z.Writer(endian, encoding);
      w.putU8(0xff); w.putBool(true); w.putChar(0xff);
      w.putU16(0xffff); w.putU32(0xffffffff); w.putU64(0xffffffffffffffffn);
      w.putF32(-3.5); w.putF64(3.141592653589793);
      w.putWchar(0xffff); w.putString('grüße');
      w.putSeqU8(Buffer.from([0xde, 0xad]));
      w.putWstring([0xd83d, 0xde00]);
      w.putBitset(0xbc614e00000fffffn, 8);
      w.putFixed('-000000.001', 9, 3);

      const r = new z.Reader(w.bytes(), endian, encoding);
      assert.strictEqual(r.getU8(), 0xff);
      assert.strictEqual(r.getBool(), true);
      assert.strictEqual(r.getChar(), 0xff);
      assert.strictEqual(r.getU16(), 0xffff);
      assert.strictEqual(r.getU32(), 0xffffffff);
      assert.strictEqual(r.getU64(), 0xffffffffffffffffn);
      assert.strictEqual(r.getF32(), -3.5);
      assert.strictEqual(r.getF64(), 3.141592653589793);
      assert.strictEqual(r.getWchar(), 0xffff);
      assert.strictEqual(r.getString(), 'grüße');
      assert.deepStrictEqual(Array.from(r.getSeqU8()), [0xde, 0xad]);
      assert.deepStrictEqual(r.getWstring(), [0xd83d, 0xde00]);
      assert.strictEqual(r.getBitset(8), 0xbc614e00000fffffn);
      assert.strictEqual(r.getFixed(9, 3), '-000000.001');
      assert.strictEqual(r.pos, w.bytes().length);
    }
  }
});
