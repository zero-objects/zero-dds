// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Node endpoint SDK (ADR 0013): a from-scratch XCDR wire-core in
// plain JavaScript, byte-identical to the Rust core and the other SDKs. Sync
// (poll) and async (an async iterator — the idiomatic Node model).

'use strict';

const LITTLE = 0;
const BIG = 1;

// The XCDR version the stream is written in.
//
// It is a constructor argument and has no default. XTypes 1.3 §7.4.1 puts
// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
// know which one it was in would be writing XCDR2 and calling it XCDR — which
// is what this SDK did until the alignment cap became a field: `_align` read
// `a > 4 ? 4 : a`, and every `long long` in an XCDR1 sample landed four octets
// early with nothing in the interface to say so.
//
// JavaScript has no types to enforce the argument, so the constructor refuses
// a stream whose encoding was left out rather than picking one of the two.
const XCDR1 = 0; // PLAIN_CDR/PL_CDR — MAXALIGN 8
const XCDR2 = 1; // PLAIN_CDR2/DELIMITED_CDR/PL_CDR2 — MAXALIGN 4

const MAX_ALIGN = { [XCDR1]: 8, [XCDR2]: 4 };

// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 read it from a
// NEXTINT that follows the header.
const LC0 = 0, LC1 = 1, LC2 = 2, LC3 = 3, LC4 = 4, LC5 = 5, LC6 = 6, LC7 = 7;

// PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
const PID_LIST_END_ID = 0x3f02;
// The PID that introduces the twelve-octet parameter header. §7.4.1.2.
const PID_EXTENDED_ID = 0x3f01;
// The highest member id a short parameter header carries, inclusive.
const PID_SHORT_MAX_ID = 0x3f00;
// Bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
const PID_FLAG_MUST_UNDERSTAND = 0x4000;
// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
// which is the implementation-extension flag.
const EM_FLAG_MUST_UNDERSTAND = 0x40000000;

// The octets an argument of `bits` width may carry.
//
// A JavaScript number is a double and carries every wire width below 53 bits
// exactly, so a value outside the width is a caller's defect and not a
// representation limit. Truncating it is the defect this refuses:
// `putU16(70000)` wrote `7011`, the low sixteen bits of 0x11170, and the frame
// was the right length either way so nothing downstream could tell.
function pattern(v, bits, call) {
  if (!Number.isInteger(v)) throw new RangeError(`${call}: ${v} is not an integer`);
  const unsignedMax = 2 ** bits - 1;
  const signedMin = -(2 ** (bits - 1));
  if (v > unsignedMax || v < signedMin) {
    throw new RangeError(`${call}: ${v} is not a ${bits}-bit value (${signedMin}..${unsignedMax})`);
  }
  return v < 0 ? v + 2 ** bits : v;
}

class Writer {
  constructor(endian, encoding) {
    if (encoding !== XCDR1 && encoding !== XCDR2) {
      throw new TypeError('Writer(endian, encoding): the encoding is XCDR1 or XCDR2 and has no default');
    }
    this.buf = [];
    this.endian = endian;
    this.encoding = encoding;
    // The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
    this.maxAlign = MAX_ALIGN[encoding];
    // The position alignment is measured from. Zero for the stream, and moved
    // to the start of a PL_CDR1 parameter's value for the length of that
    // parameter — §7.4.1.2 aligns a parameter's value against its own block,
    // not against the stream. Without it a `double` in a parameter takes four
    // octets of padding and the header declares twelve where Cyclone, RTI and
    // Fast DDS all declare eight.
    this.origin = 0;
  }
  _align(a) {
    const cap = a > this.maxAlign ? this.maxAlign : a;
    while ((this.buf.length - this.origin) % cap !== 0) this.buf.push(0);
  }
  _putLE(a, le) {
    this._align(a);
    if (this.endian === BIG) {
      for (let i = le.length - 1; i >= 0; i--) this.buf.push(le[i]);
    } else {
      for (const b of le) this.buf.push(b);
    }
  }
  putU8(v) { this.buf.push(pattern(v, 8, 'putU8')); }
  // A `boolean` is the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it: 0 or
  // 1, never any other pattern.
  putBool(v) { this.buf.push(v ? 1 : 0); }
  putU16(v) {
    const p = pattern(v, 16, 'putU16');
    this._putLE(2, [p & 0xff, (p >>> 8) & 0xff]);
  }
  putU32(v) {
    const p = pattern(v, 32, 'putU32');
    this._putLE(4, [p & 0xff, (p >>> 8) & 0xff, (p >>> 16) & 0xff, (p >>> 24) & 0xff]);
  }
  // A `long long` at its natural eight-octet alignment, which maxAlign then
  // caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
  //
  // The argument is a BigInt: a JavaScript number loses precision above 2^53
  // and would round a `long long` silently.
  putU64(v) {
    if (typeof v !== 'bigint') throw new TypeError(`putU64: ${v} is not a BigInt`);
    if (v > 0xffffffffffffffffn || v < -(2n ** 63n)) {
      throw new RangeError(`putU64: ${v} is not a 64-bit value`);
    }
    const p = v < 0n ? v + 2n ** 64n : v;
    const le = [];
    for (let i = 0n; i < 8n; i++) le.push(Number((p >> (8n * i)) & 0xffn));
    this._putLE(8, le);
  }
  putF32(v) {
    const b = Buffer.alloc(4);
    b.writeFloatLE(v, 0); // logical little-endian bit pattern
    this._putLE(4, [b[0], b[1], b[2], b[3]]);
  }
  // `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and the
  // floating type most DDS topics use; it goes out as its IEEE-754 bit pattern
  // at the same alignment as putU64.
  putF64(v) {
    const b = Buffer.alloc(8);
    b.writeDoubleLE(v, 0);
    this._putLE(8, [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
  }
  putBytes(b) { for (const x of b) this.buf.push(x & 0xff); }
  // Wire rule (docs/specs/zerodds-xcdr2-node-1.0.md §"string"): uint32 (UTF-8
  // byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the BYTE
  // count of the UTF-8 encoding, not the JS string length (UTF-16 code units) —
  // multibyte characters occupy more than one byte. Byte-identical to the C/Rust
  // core.
  putString(s) {
    const raw = Buffer.from(s, 'utf8');
    this.putU32(raw.length + 1);
    this.putBytes(raw);
    this.putU8(0);
  }
  putSeqU8(b) { this.putU32(b.length); this.putBytes(b); }

  // An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
  //
  // A code point above U+00FF is refused and not truncated. `š` is U+0161 and
  // its low octet is 0x61, which is `a`: a truncating writer produces a frame
  // of the right length holding a different, plausible character, and nothing
  // downstream can tell. The Rust core refuses `primitive-char-above-latin1` in
  // the same words.
  putChar(codePoint) {
    if (!Number.isInteger(codePoint) || codePoint < 0 || codePoint > 0xff) {
      throw new RangeError(`expected a character below U+0100, found U+${codePoint.toString(16).toUpperCase()}`);
    }
    this.buf.push(codePoint);
  }

  // An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
  // order.
  //
  // The four policy axes a wide character carries are answered here and not by
  // the caller: the unit is two octets wide, it goes out in the stream's byte
  // order, there is no GIOP length octet before it, and no byte-order mark.
  //
  // A code point above U+FFFF is refused. The refusal is width-independent — a
  // four-octet form would zero-extend a code *unit*, not a code point — which
  // is how the Rust core refuses `primitive-wchar-above-bmp`.
  putWchar(codePoint) {
    if (!Number.isInteger(codePoint) || codePoint < 0 || codePoint > 0xffff) {
      throw new RangeError(`expected a BMP character, found U+${codePoint.toString(16).toUpperCase()}`);
    }
    this.putU16(codePoint);
  }

  // An IDL `wstring` from its UTF-16 code units.
  //
  // The length word counts **octets** and not code units, there is no
  // terminator, there is no byte-order mark, and each unit goes out two octets
  // wide in the stream's own byte order. Those are the four axes, and they are
  // the binding's answer — the caller hands over units and nothing else.
  putWstring(units) {
    this.putU32(units.length * 2);
    for (const unit of units) {
      if (!Number.isInteger(unit) || unit < 0 || unit > 0xffff) {
        throw new RangeError(`a wstring code unit is two octets, found ${unit}`);
      }
      this.putU16(unit);
    }
  }

  // A `bitset` as its anchor word: the holder the declared bits pack into,
  // written at the anchor's own width and therefore at the anchor's own
  // alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
  // anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
  //
  // `bits` is the packed holder (a BigInt, because an eight-octet anchor does
  // not fit a JavaScript number) and `anchor` its width in octets, both
  // properties of the declared type. What this call decides is the pair the
  // type does not: which write the holder goes out through, and what that write
  // aligns to.
  putBitset(bits, anchor) {
    const v = BigInt(bits);
    switch (anchor) {
      case 1: return this.putU8(Number(v & 0xffn));
      case 2: return this.putU16(Number(v & 0xffffn));
      case 4: return this.putU32(Number(v & 0xffffffffn));
      case 8: return this.putU64(BigInt.asUintN(64, v));
      default:
        throw new RangeError(`a bitset anchor is one, two, four or eight octets, not ${anchor}`);
    }
  }

  // An IDL `fixed<digits,scale>` as packed BCD.
  // CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
  putFixed(value, digits, scale) { this.putBytes(fixedOctets(value, digits, scale)); }

  // --- back-patched framing ---------------------------------------------
  //
  // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same shape:
  // a slot reserved before the body and filled in once the body's length is
  // known. These are the only writes into an already-written position, and they
  // honour the wire byte order like every other write.

  _patchU32(at, v) {
    const le = [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff];
    for (let i = 0; i < 4; i++) this.buf[at + i] = this.endian === BIG ? le[3 - i] : le[i];
  }
  _patchU16(at, v) {
    const le = [v & 0xff, (v >>> 8) & 0xff];
    for (let i = 0; i < 2; i++) this.buf[at + i] = this.endian === BIG ? le[1 - i] : le[i];
  }

  // Reserves a 4-aligned DHEADER slot and returns the position the body starts
  // at, to hand back to dheaderEnd. XTypes 1.3 §7.4.3.5.
  dheaderBegin() {
    this.putU32(0);
    return this.buf.length;
  }
  // Back-patches the DHEADER with the byte length written since bodyStart. The
  // DHEADER itself is not counted — §7.4.3.5 measures the content that follows
  // it.
  dheaderEnd(bodyStart) { this._patchU32(bodyStart - 4, this.buf.length - bodyStart); }

  // Writes the EMHEADER for one member of a @mutable aggregate and returns the
  // body-start position. XTypes 1.3 §7.4.3.4.2:
  //
  //     EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
  //
  // with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
  // emheaderEnd fills it in; for LC0..LC3 the code alone gives the length and
  // nothing follows; for LC5..LC7 the body's own leading four octets are the
  // length, so no separate NEXTINT is written and emheaderEnd has nothing to
  // patch.
  emheaderBegin(id, mustUnderstand, lengthCode) {
    if (!Number.isInteger(id) || id < 0 || id > 0x0fffffff) {
      throw new RangeError(`member id ${id} does not fit the 28 bits of an EMHEADER`);
    }
    if (!Number.isInteger(lengthCode) || lengthCode < LC0 || lengthCode > LC7) {
      throw new RangeError(`length code ${lengthCode} is not one of LC0..LC7`);
    }
    let header = lengthCode * 2 ** 28 + id;
    if (mustUnderstand) header += 2 ** 31;
    this.putU32(header);
    if (lengthCode === LC4) this.putU32(0);
    return this.buf.length;
  }

  // Closes the member emheaderBegin opened.
  //
  // It takes the length code back rather than remembering it, so a caller that
  // paired an LC4 begin with a differently-coded end gets a refusal instead of
  // a NEXTINT patched over the first four octets of a member body.
  emheaderEnd(bodyStart, lengthCode) {
    const written = this.buf.length - bodyStart;
    if (lengthCode === LC4) {
      this._patchU32(bodyStart - 4, written);
      return;
    }
    if (lengthCode <= LC3) {
      // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
      // means the plan and the member disagree. Silence here would put a length
      // on the wire that no octet count backs.
      const want = 1 << lengthCode;
      if (written !== want) {
        throw new RangeError(`LC${lengthCode} frames ${want} octets and the member wrote ${written}`);
      }
      return;
    }
    if (written < 4) {
      throw new RangeError(
        `LC${lengthCode} takes its length from the body's first four octets and the member wrote ${written}`);
    }
  }

  // Writes a PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a
  // four-octet boundary an `unsigned short` parameter id, then an `unsigned
  // short` byte length of the value that follows. Both go out in the stream's
  // byte order.
  //
  // The must-understand flag is bit 14 of the id field. Bit 15 is the
  // implementation-extension flag and is not set here.
  //
  // The parameter's value aligns against the block and not against the stream,
  // so the alignment origin moves for the length of the parameter and pidEnd
  // restores it.
  pidBegin(pid, mustUnderstand) {
    if (!Number.isInteger(pid) || pid < 0 || pid > PID_SHORT_MAX_ID) {
      throw new RangeError(`member id ${pid} needs the extended parameter header`);
    }
    this._align(4);
    const id = mustUnderstand ? pid | PID_FLAG_MUST_UNDERSTAND : pid;
    this._putLE(2, [id & 0xff, (id >>> 8) & 0xff]);
    const at = this.buf.length;
    this.buf.push(0, 0);
    const saved = this.origin;
    this.origin = this.buf.length;
    return { at, extended: false, savedOrigin: saved };
  }

  // Writes the twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for a
  // member id above 0x3F00 or a value longer than 65535 octets:
  //
  //     [PID_EXTENDED | mu : u16][8 : u16][eMemberHeader : u32][length : u32]
  //
  // The must-understand flag on the marker is not the member's — it is on the
  // marker under this framing whatever the member says, and the member's own
  // flag is bit 30 of the eMemberHeader. Reading it off the marker instead
  // makes every parameter must-understand, which is what Cyclone's wire looks
  // like if you do.
  pidExtendedBegin(memberId, mustUnderstand) {
    if (!Number.isInteger(memberId) || memberId < 0 || memberId > 0x0fffffff) {
      throw new RangeError(`member id ${memberId} does not fit the 28 bits of an eMemberHeader`);
    }
    this._align(4);
    const marker = PID_EXTENDED_ID | PID_FLAG_MUST_UNDERSTAND;
    this._putLE(2, [marker & 0xff, (marker >>> 8) & 0xff]);
    this._putLE(2, [8, 0]);
    const header = mustUnderstand ? memberId + EM_FLAG_MUST_UNDERSTAND : memberId;
    this._putLE(4, [header & 0xff, (header >>> 8) & 0xff, (header >>> 16) & 0xff,
      Math.floor(header / 2 ** 24) & 0xff]);
    const at = this.buf.length;
    this.buf.push(0, 0, 0, 0);
    const saved = this.origin;
    this.origin = this.buf.length;
    return { at, extended: true, savedOrigin: saved };
  }

  // Closes the parameter pidBegin or pidExtendedBegin opened.
  //
  // The declared length counts the value's octets **and** the padding that
  // brings the next header back to a four-octet boundary: a reader skips
  // `length` from the end of the header and has to land on the next parameter.
  // The padding itself is written after the length is patched, so the two
  // cannot disagree.
  pidEnd(header) {
    const width = header.extended ? 4 : 2;
    let length = this.buf.length - header.at - width;
    // The parameter's own origin is still in force here, so the round-up is
    // measured from the block that the length describes.
    const past = (this.buf.length - header.savedOrigin) % 4;
    if (past !== 0) length += 4 - past;
    if (header.extended) {
      this._patchU32(header.at, length);
    } else {
      if (length > 0xffff) {
        throw new RangeError(`a short parameter header describes at most 65535 octets, not ${length}`);
      }
      this._patchU16(header.at, length);
    }
    this.origin = header.savedOrigin;
    this._align(4);
  }

  // The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END with
  // a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
  //
  // The must-understand flag is set on the sentinel — 0x7F02 on the wire, which
  // is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the flag is
  // not the sentinel's meaning either way, and this is the majority form the
  // Rust column records.
  pidListEnd() {
    this._align(4);
    const id = PID_LIST_END_ID | PID_FLAG_MUST_UNDERSTAND;
    this._putLE(2, [id & 0xff, (id >>> 8) & 0xff]);
    this._putLE(2, [0, 0]);
  }

  bytes() { return Buffer.from(this.buf); }
}

// The packing putFixed writes, apart so it can be tested at the edges of its
// range without a Writer.
//
// `value` is the decimal as the sample wrote it — an optional sign, an integer
// part and an optional fraction. Everything after that is this function's:
//
//   - the integer part is left-padded and the fraction right-padded to exactly
//     `digits` decimal digits, with the point implied `scale` from the right;
//     the scale itself is not on the wire,
//   - a leading zero nibble is prepended when `digits + 1` is odd, so digits and
//     sign pair up,
//   - two nibbles to the octet, most significant digit first, high nibble first,
//   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
//     goes out positive, because there is one zero to write,
//   - alignment is one, and no multi-octet integer is written, so the byte order
//     never enters.
//
// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
function fixedOctets(value, digits, scale) {
  if (!Number.isInteger(digits) || digits < 1 || digits > 31) {
    throw new RangeError(`fixed<${digits},${scale}>: a digit count is 1..31`);
  }
  if (!Number.isInteger(scale) || scale < 0 || scale > digits) {
    throw new RangeError(`fixed<${digits},${scale}>: a scale is 0..digits`);
  }
  let text = String(value);
  let negative = false;
  if (text.startsWith('+') || text.startsWith('-')) {
    negative = text.startsWith('-');
    text = text.slice(1);
  }
  const dot = text.indexOf('.');
  let integral = dot >= 0 ? text.slice(0, dot) : text;
  let fraction = dot >= 0 ? text.slice(dot + 1) : '';
  if (integral === '' && fraction === '') {
    throw new RangeError(`fixed<${digits},${scale}>: \`${value}\` has no digits`);
  }
  if (!/^[0-9]*$/.test(integral) || !/^[0-9]*$/.test(fraction)) {
    throw new RangeError(`fixed<${digits},${scale}>: \`${value}\` is not a decimal`);
  }
  // Leading zeros carry no value, so they are dropped before the width is
  // checked: `000000.001` is one significant integer digit, not six.
  integral = integral.replace(/^0+/, '');
  if (integral.length > digits - scale) {
    throw new RangeError(
      `fixed<${digits},${scale}>: \`${value}\` needs ${integral.length} integer digits and the type has ${digits - scale}`);
  }
  // A trailing zero in the fraction is padding either way, so it is dropped
  // before the width is checked and re-added below.
  while (fraction.length > scale && fraction.endsWith('0')) fraction = fraction.slice(0, -1);
  if (fraction.length > scale) {
    throw new RangeError(
      `fixed<${digits},${scale}>: \`${value}\` needs ${fraction.length} fraction digits and the type has ${scale}`);
  }
  const nibbles = [];
  if ((digits + 1) % 2 === 1) nibbles.push(0);
  for (let i = 0; i < digits - scale - integral.length; i++) nibbles.push(0);
  for (const ch of integral) nibbles.push(ch.charCodeAt(0) - 48);
  for (const ch of fraction) nibbles.push(ch.charCodeAt(0) - 48);
  for (let i = 0; i < scale - fraction.length; i++) nibbles.push(0);
  nibbles.push(negative && nibbles.some((n) => n !== 0) ? 0x0d : 0x0c);
  const out = [];
  for (let i = 0; i + 1 < nibbles.length; i += 2) out.push((nibbles[i] << 4) | nibbles[i + 1]);
  return out;
}

class Reader {
  constructor(buf, endian, encoding) {
    if (encoding !== XCDR1 && encoding !== XCDR2) {
      throw new TypeError('Reader(buf, endian, encoding): the encoding is XCDR1 or XCDR2 and has no default');
    }
    this.buf = buf;
    this.pos = 0;
    this.endian = endian;
    this.encoding = encoding;
    // The same cap the writer used, or the fields land elsewhere.
    this.maxAlign = MAX_ALIGN[encoding];
    // Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    // block.
    this.origin = 0;
  }
  _getLE(a, n) {
    const cap = a > this.maxAlign ? this.maxAlign : a;
    while ((this.pos - this.origin) % cap !== 0) this.pos++;
    const out = Array.from(this.buf.subarray(this.pos, this.pos + n));
    this.pos += n;
    if (this.endian === BIG) out.reverse();
    return out;
  }
  getU32() {
    const le = this._getLE(4, 4);
    return (le[0] | (le[1] << 8) | (le[2] << 16) | (le[3] << 24)) >>> 0;
  }
  // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
  getU8() {
    const v = this.buf[this.pos];
    this.pos++;
    return v;
  }
  getU16() {
    const le = this._getLE(2, 2);
    return (le[0] | (le[1] << 8)) >>> 0;
  }
  // A `long long` at its natural eight-octet alignment, which maxAlign then
  // caps: eight under XCDR1 and four under XCDR2.
  //
  // It passed 4 here while the writer passed 8, which under XCDR1 read the four
  // octets of padding as the value's low half and returned a different number
  // with no error anywhere. Returns a BigInt — a JS number loses precision
  // above 2^53.
  getU64() {
    const le = this._getLE(8, 8);
    let v = 0n;
    for (let i = 0; i < 8; i++) v |= BigInt(le[i]) << (8n * BigInt(i));
    return v;
  }
  getF32() {
    const le = this._getLE(4, 4);
    return Buffer.from(le).readFloatLE(0);
  }
  getF64() {
    const le = this._getLE(8, 8);
    return Buffer.from(le).readDoubleLE(0);
  }
  getBool() { return this.getU8() !== 0; }
  // The one octet an IDL `char` occupies.
  getChar() { return this.getU8(); }
  // One two-octet `wchar` code unit.
  getWchar() { return this.getU16(); }
  // A `wstring` back into its UTF-16 code units. The length word counts octets,
  // so the unit count is half of it.
  getWstring() {
    const octets = this.getU32();
    const units = [];
    for (let i = 0; i < octets / 2; i++) units.push(this.getU16());
    return units;
  }
  // A `bitset` anchor of the given width back into its holder word, as a
  // BigInt so an eight-octet anchor survives.
  getBitset(anchor) {
    switch (anchor) {
      case 1: return BigInt(this.getU8());
      case 2: return BigInt(this.getU16());
      case 4: return BigInt(this.getU32());
      case 8: return this.getU64();
      default:
        throw new RangeError(`a bitset anchor is one, two, four or eight octets, not ${anchor}`);
    }
  }
  // A `fixed<digits,scale>` back into the decimal it was written from: a
  // leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
  // and a `.` with `scale` fraction digits where the scale is not zero.
  //
  // The scale is not on the wire, so it is an argument here for the same reason
  // it is one in putFixed.
  getFixed(digits, scale) {
    const octets = Math.floor((digits + 2) / 2);
    const nibbles = [];
    for (let i = 0; i < octets; i++) {
      const b = this.getU8();
      nibbles.push(b >> 4, b & 0x0f);
    }
    const sign = nibbles[nibbles.length - 1];
    const body = nibbles.slice(nibbles.length - 1 - digits, nibbles.length - 1);
    let out = sign === 0x0d ? '-' : '';
    for (let i = 0; i < body.length; i++) {
      if (scale !== 0 && i === digits - scale) out += '.';
      out += String(body[i]);
    }
    return out;
  }

  // Reads a DHEADER and returns the announced body byte length.
  dheaderRead() { return this.getU32(); }

  // A member header: the member id, the must-understand flag, the length code,
  // and the member body's extent in octets measured from the position this call
  // leaves the cursor at.
  //
  // LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
  // follows the header, and the extent is that word. LC5..LC7 do not consume
  // anything: the next four octets are the member's own leading word and belong
  // to the body, so they are peeked and the extent is 4 + the word for LC5,
  // 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two count elements,
  // not octets.
  emheaderRead() {
    const header = this.getU32();
    const id = header & 0x0fffffff;
    const mustUnderstand = (header & 0x80000000) !== 0;
    const lengthCode = (header >>> 28) & 0x7;
    let extent;
    if (lengthCode <= LC3) {
      extent = 1 << lengthCode;
    } else if (lengthCode === LC4) {
      extent = this.getU32();
    } else {
      const at = this.pos;
      const word = this.getU32();
      this.pos = at;
      extent = lengthCode === LC5 ? 4 + word : lengthCode === LC6 ? 4 + 4 * word : 4 + 8 * word;
    }
    return { id, mustUnderstand, lengthCode, extent };
  }

  // A PL_CDR1 parameter header in either form: the member id with its flags
  // stripped, the member's must-understand flag, the declared byte length, and
  // whether the list ends here.
  //
  // The extended form is recognised by the marker PID_EXTENDED, and the
  // must-understand answer then comes from bit 30 of the eMemberHeader and
  // never from the marker — the marker carries the flag under this framing
  // whatever the member says.
  //
  // The value's alignment origin is moved to the position this call leaves, and
  // pidSkip restores it.
  pidRead() {
    const raw = this.getU16();
    const short = raw & 0x3fff;
    if (short === PID_LIST_END_ID) {
      this.getU16();
      return { id: 0, mustUnderstand: false, length: 0, listEnd: true };
    }
    if (short === PID_EXTENDED_ID) {
      this.getU16(); // the slength, always 8
      const header = this.getU32();
      const length = this.getU32();
      this.origin = this.pos;
      return {
        id: header & 0x0fffffff,
        mustUnderstand: (header & EM_FLAG_MUST_UNDERSTAND) !== 0,
        length,
        listEnd: false,
      };
    }
    const length = this.getU16();
    this.origin = this.pos;
    return {
      id: short,
      mustUnderstand: (raw & PID_FLAG_MUST_UNDERSTAND) !== 0,
      length,
      listEnd: false,
    };
  }

  // Moves the cursor past a parameter value of `length` octets from `bodyStart`
  // and restores the stream's alignment origin.
  pidSkip(bodyStart, length) {
    this.pos = bodyStart + length;
    this.origin = 0;
    while (this.pos % 4 !== 0) this.pos++;
  }
  // Inverse of putString: read n bytes (n incl. the NUL), decode the first n-1
  // as UTF-8.
  getString() {
    const n = this.getU32();
    const s = this.buf.subarray(this.pos, this.pos + n - 1).toString('utf8');
    this.pos += n;
    return s;
  }
  getSeqU8() {
    const n = this.getU32();
    const b = Buffer.from(this.buf.subarray(this.pos, this.pos + n));
    this.pos += n;
    return b;
  }
}

// --- XRCE framing ---

const XRCE_SESSION_NOKEY = 0x80;
const XRCE_STREAM_BEST_EFFORT = 0x01;
// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 —
// DDS-XRCE 1.0 §8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
// require a SampleInfo in the body, which this profile does not carry.
const XRCE_WRITE_FLAGS = 0x01;
// Wire size of the BaseObjectRequest that precedes the sample (§7.7.8).
const XRCE_BASE_SIZE = 4;
// OBJK_DATAWRITER — the ObjectKind of a DataWriter (§7.2.2).
const OBJK_DATAWRITER = 0x05;

// An ObjectId (§7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
function xrceObjectId(rawId, kind) {
  return ((rawId & 0x0fff) << 4) | (kind & 0x0f);
}

// The submessage body is the BaseObjectRequest (§7.7.8: request_id then
// object_id, each two octets most significant first) followed by the sample.
// `objectId` names the target DataWriter — without it no XRCE agent can route
// the sample. The request_id is the message's own `seq`, which §7.7.8 only
// requires to be unique per stream.
//
// Returns null when the body is larger than the 16-bit submessage_length
// field can describe (0xFFFF): refuse rather than truncate the length while
// copying the full payload (the C SDK rejects the same way).
function xrceWriteFrame(session, stream, seq, objectId, sample) {
  const bodyLen = XRCE_BASE_SIZE + sample.length;
  if (bodyLen > 0xffff) return null;
  const out = Buffer.alloc(8 + bodyLen);
  out[0] = session; out[1] = stream;
  out[2] = seq & 0xff; out[3] = (seq >>> 8) & 0xff;
  out[4] = 0x07; out[5] = XRCE_WRITE_FLAGS;
  out[6] = bodyLen & 0xff; out[7] = (bodyLen >>> 8) & 0xff;
  out[8] = (seq >>> 8) & 0xff; out[9] = seq & 0xff;
  out[10] = (objectId >>> 8) & 0xff; out[11] = objectId & 0xff;
  Buffer.from(sample).copy(out, 8 + XRCE_BASE_SIZE);
  return out;
}

// Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent — what
// this endpoint SENDS (xrceWriteFrame); DATA (0x09) is agent->client — what it
// RECEIVES from the hub. The reader accepts both (parity with the C/Python
// SDKs): 0x09 for real hub traffic, 0x07 for loopback. Returns null — never
// throws, never a bogus sample — on a short header, an unknown submessage id, or
// a declared length that runs past the datagram (truncated / wrong length); the
// body is bounded by the declared length so trailing bytes never leak in.
function xrceReadFrame(frame) {
  if (frame.length < 8 + XRCE_BASE_SIZE) return null;
  if (frame[4] !== 0x09 && frame[4] !== 0x07) return null;
  const smLen = frame[6] | (frame[7] << 8);
  if (smLen < XRCE_BASE_SIZE || 8 + smLen > frame.length) return null;
  return frame.subarray(8 + XRCE_BASE_SIZE, 8 + smLen);
}

// `{ requestId, objectId }` of a data frame — the identity an agent routes on
// (§7.7.8) — or null under the same rules as `xrceReadFrame`.
function xrceFrameBase(frame) {
  if (xrceReadFrame(frame) === null) return null;
  return {
    requestId: (frame[8] << 8) | frame[9],
    objectId: (frame[10] << 8) | frame[11],
  };
}

// --- sync client ---

class Client {
  constructor(transport, objectId, session = XRCE_SESSION_NOKEY, stream = XRCE_STREAM_BEST_EFFORT) {
    this.transport = transport;
    // ObjectId of the target DataWriter (§7.7.8 BaseObjectRequest).
    this.objectId = objectId;
    this.session = session;
    this.stream = stream;
    this.seq = 1;
  }
  // `xrceWriteFrame` answers null for a sample the 16-bit
  // submessage_length cannot describe, and this used to hand that null
  // straight to `deliver`. The builder's refusal was representable and
  // nobody read it; now it stops here. The sequence is not advanced,
  // because nothing was written.
  write(sample) {
    const frame = xrceWriteFrame(this.session, this.stream, this.seq, this.objectId, sample);
    if (frame === null) {
      throw new RangeError(
        `${sample.length} octets: the 16-bit XRCE submessage_length describes at most 65535`);
    }
    this.seq = (this.seq + 1) & 0xffff;
    return this.transport.deliver(frame);
  }
  // One non-blocking receive: returns the sample body, or null.
  poll() {
    const frame = this.transport.receive();
    return frame === null ? null : xrceReadFrame(frame);
  }
}

// --- async reader: an async iterator (idiomatic Node) ---

class AsyncReader {
  constructor(transport) {
    this.transport = transport;
    this._stop = false;
  }
  // for await (const body of reader.stream()) { ... }
  async *stream() {
    while (!this._stop) {
      const frame = this.transport.receive();
      if (frame === null) {
        await new Promise((r) => setTimeout(r, 1));
        continue;
      }
      const body = xrceReadFrame(frame);
      if (body) yield body;
    }
  }
  close() { this._stop = true; }
}

// A simple in-memory transport for tests + example.
class MemTransport {
  constructor() { this.q = []; }
  deliver(frame) { this.q.push(Buffer.from(frame)); return true; }
  receive() { return this.q.length ? this.q.shift() : null; }
}

module.exports = {
  LITTLE, BIG, XCDR1, XCDR2, Writer, Reader,
  LC0, LC1, LC2, LC3, LC4, LC5, LC6, LC7,
  PID_LIST_END_ID, PID_EXTENDED_ID, PID_SHORT_MAX_ID,
  fixedOctets,
  XRCE_SESSION_NOKEY, XRCE_STREAM_BEST_EFFORT,
  xrceWriteFrame, xrceReadFrame, xrceFrameBase, xrceObjectId,
  XRCE_WRITE_FLAGS, XRCE_BASE_SIZE, OBJK_DATAWRITER,
  Client, AsyncReader, MemTransport,
};
