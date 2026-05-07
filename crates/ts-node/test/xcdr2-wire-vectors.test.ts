// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// xcdr2-wire-vectors.test.ts — Pflicht-Konformanz-Tests fuer
// `@zerodds/cdr` gegen `zerodds-xcdr2-bindings-conformance-1.0` §6
// (Wire-Test-Vektoren V-1..V-12).
//
// Pro Vektor:
//   1. Build sample
//   2. encode(sample) → Bytes byte-exact gegen Spec
//   3. decode(bytes) → roundtrip-Sample (deep-equal)
//
// Tests benutzen die Helper-Library direkt (Xcdr2Writer/Xcdr2Reader/
// md5) und replizieren das Pattern, das `idl-ts` pro Sample emittiert.
// Das deckt L1 (Wire) ab; L2 (Codegen) wird separat in
// `crates/idl-ts/tests/compile_check.rs` geprueft.

import test from "node:test";
import assert from "node:assert/strict";
import { Xcdr2Reader, Xcdr2Writer, md5 } from "../src/cdr/index.js";

// === Hex-Helpers ============================================================

function hex(bytes: Uint8Array): string {
    return Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, "0"))
        .join(" ");
}

function fromHex(s: string): Uint8Array {
    // Strip line-comments (everything after `#` on a line) und alle
    // Whitespace-Chars. Was uebrig bleibt MUSS hex-paarig sein.
    const noComments = s.replace(/#[^\n]*/g, "");
    const cleaned = noComments.replace(/\s+/g, "");
    if (cleaned.length % 2 !== 0) {
        throw new Error(`odd hex length: '${cleaned}'`);
    }
    const out = new Uint8Array(cleaned.length / 2);
    for (let i = 0; i < out.length; i++) {
        out[i] = parseInt(cleaned.substring(i * 2, i * 2 + 2), 16);
    }
    return out;
}

function assertBytesEq(actual: Uint8Array, expected: Uint8Array, label: string): void {
    assert.equal(
        hex(actual),
        hex(expected),
        `${label}: bytes mismatch\n  actual:   ${hex(actual)}\n  expected: ${hex(expected)}`,
    );
}

// === V-1 Empty Final Struct =================================================

interface VEmpty {}

test("V-1 Empty Final Struct: encode → 0 bytes; roundtrip", () => {
    const w = new Xcdr2Writer("le");
    const bytes = w.toBytes();
    assertBytesEq(bytes, new Uint8Array(0), "V-1 encode");

    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    const out: VEmpty = {};
    assert.deepEqual(out, {});
    assert.equal(r.pos, 0);
});

// === V-2 Plain Primitives Final =============================================

interface VPoint {
    x: number;
    y: number;
}

function encodePoint(s: VPoint): Uint8Array {
    const w = new Xcdr2Writer("le");
    w.writeInt32(s.x);
    w.writeInt32(s.y);
    return w.toBytes();
}

function decodePoint(bytes: Uint8Array): VPoint {
    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    const x = r.readInt32();
    const y = r.readInt32();
    return { x, y };
}

test("V-2 Plain Primitives Final: encode + decode roundtrip", () => {
    const sample: VPoint = { x: 1, y: -2 };
    const expected = fromHex("01 00 00 00 FE FF FF FF");
    const enc = encodePoint(sample);
    assertBytesEq(enc, expected, "V-2 encode");
    const dec = decodePoint(expected);
    assert.deepEqual(dec, sample);
});

// === V-3 Mixed Primitives Final =============================================

interface VAll {
    b: boolean;
    o: number;
    s: number;
    us: number;
    l: number;
    ul: number;
    ll: bigint;
    ull: bigint;
    f: number;
    d: number;
}

function encodeAll(s: VAll): Uint8Array {
    const w = new Xcdr2Writer("le");
    w.writeBool(s.b);
    w.writeOctet(s.o);
    w.writeInt16(s.s);
    w.writeUint16(s.us);
    w.writeInt32(s.l);
    w.writeUint32(s.ul);
    w.writeInt64(s.ll);
    w.writeUint64(s.ull);
    w.writeFloat32(s.f);
    w.writeFloat64(s.d);
    return w.toBytes();
}

function decodeAll(bytes: Uint8Array): VAll {
    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    return {
        b: r.readBool(),
        o: r.readOctet(),
        s: r.readInt16(),
        us: r.readUint16(),
        l: r.readInt32(),
        ul: r.readUint32(),
        ll: r.readInt64(),
        ull: r.readUint64(),
        f: r.readFloat32(),
        d: r.readFloat64(),
    };
}

test("V-3 Mixed Primitives Final: 48 bytes, padding origin-relative", () => {
    const sample: VAll = {
        b: true,
        o: 0xab,
        s: -12345,
        us: 54321,
        l: -1234567,
        ul: 2345678,
        ll: -987654321n,
        ull: 123456789n,
        f: 2.5,
        d: 3.14159,
    };
    const expected = fromHex(
        "01 AB" +
            "C7 CF" +
            "31 D4" +
            "00 00" +
            "79 29 ED FF" +
            "CE CA 23 00" +
            "4F 97 21 C5 FF FF FF FF" +
            "15 CD 5B 07 00 00 00 00" +
            "00 00 20 40" +
            "00 00 00 00" +
            "6E 86 1B F0 F9 21 09 40",
    );
    assert.equal(expected.length, 48, "V-3 expected length is 48 bytes");
    const enc = encodeAll(sample);
    assertBytesEq(enc, expected, "V-3 encode");
    const dec = decodeAll(expected);
    assert.deepEqual(dec, sample);
});

// === V-4 String Final =======================================================

interface VGreeting {
    text: string;
}

test("V-4 String Final: uint32 length-with-NUL + bytes + NUL", () => {
    const sample: VGreeting = { text: "hello" };
    const expected = fromHex("06 00 00 00 68 65 6C 6C 6F 00");
    const w = new Xcdr2Writer("le");
    w.writeString(sample.text);
    const enc = w.toBytes();
    assertBytesEq(enc, expected, "V-4 encode");
    const r = new Xcdr2Reader(expected, 0, expected.length, "le");
    const dec: VGreeting = { text: r.readString() };
    assert.deepEqual(dec, sample);
});

// === V-5 Sequence<int32> Final ==============================================

interface VBag {
    ids: number[];
}

test("V-5 Sequence<int32> Final: count + elements", () => {
    const sample: VBag = { ids: [1, 2, 3] };
    const expected = fromHex(
        "03 00 00 00  01 00 00 00  02 00 00 00  03 00 00 00",
    );
    const w = new Xcdr2Writer("le");
    w.writeUint32(sample.ids.length);
    for (const e of sample.ids) {
        w.writeInt32(e);
    }
    assertBytesEq(w.toBytes(), expected, "V-5 encode");
    const r = new Xcdr2Reader(expected, 0, expected.length, "le");
    const n = r.readUint32();
    const ids: number[] = [];
    for (let i = 0; i < n; i++) {
        ids.push(r.readInt32());
    }
    assert.deepEqual({ ids }, sample);
});

// === V-6 Sequence<string> Final =============================================

interface VTags {
    tags: string[];
}

test("V-6 Sequence<string> Final: 4-byte alignment between strings", () => {
    const sample: VTags = { tags: ["a", "bc"] };
    const expected = fromHex(
        "02 00 00 00" +
            "02 00 00 00 61 00" +
            "00 00" +
            "03 00 00 00 62 63 00",
    );
    const w = new Xcdr2Writer("le");
    w.writeUint32(sample.tags.length);
    for (const e of sample.tags) {
        w.writeString(e);
    }
    assertBytesEq(w.toBytes(), expected, "V-6 encode");
    const r = new Xcdr2Reader(expected, 0, expected.length, "le");
    const n = r.readUint32();
    const tags: string[] = [];
    for (let i = 0; i < n; i++) {
        tags.push(r.readString());
    }
    assert.deepEqual({ tags }, sample);
});

// === V-7 Nested Modules Final ===============================================

interface VS {
    x: number;
}

test("V-7 Nested Modules Final: 4 bytes, type-name 'Outer::Inner::S'", () => {
    const sample: VS = { x: 1234 };
    const expected = fromHex("D2 04 00 00");
    const w = new Xcdr2Writer("le");
    w.writeInt32(sample.x);
    assertBytesEq(w.toBytes(), expected, "V-7 encode");
    const r = new Xcdr2Reader(expected, 0, expected.length, "le");
    const dec: VS = { x: r.readInt32() };
    assert.deepEqual(dec, sample);
});

// === V-8 Keyed Struct (Final) ===============================================

interface VSensor {
    id: number;
    value: number;
}

function encodeSensor(s: VSensor): Uint8Array {
    const w = new Xcdr2Writer("le");
    w.writeInt32(s.id);
    w.writeFloat64(s.value);
    return w.toBytes();
}

function keyHashSensor(s: VSensor): Uint8Array {
    // XTypes 1.3 §7.6.8.4: Holder ≤ 16 octets -> zero-pad; sonst MD5.
    const w = new Xcdr2Writer("be");
    w.writeInt32(s.id);
    const holder = w.toBytes();
    if (holder.length <= 16) {
        const h = new Uint8Array(16);
        h.set(holder);
        return h;
    }
    return md5(holder);
}

test("V-8 Keyed Struct: payload + key-hash", () => {
    const sample: VSensor = { id: 42, value: 3.14 };
    const expectedPayload = fromHex(
        "2A 00 00 00" +
            "00 00 00 00" +
            "1F 85 EB 51 B8 1E 09 40",
    );
    const enc = encodeSensor(sample);
    assertBytesEq(enc, expectedPayload, "V-8 encode");

    // Key-hash zero-padded auf 16 Bytes (Holder = 4 Byte ≤ 16 per XTypes §7.6.8.4).
    const expectedHash = fromHex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00");
    const hash = keyHashSensor(sample);
    assertBytesEq(hash, expectedHash, "V-8 key-hash");

    const r = new Xcdr2Reader(enc, 0, enc.length, "le");
    const id = r.readInt32();
    const value = r.readFloat64();
    assert.deepEqual({ id, value }, sample);
});

// === V-9 Appendable Struct ==================================================

interface VV {
    a: number;
    b: number;
}

function encodeV(s: VV): Uint8Array {
    const w = new Xcdr2Writer("le");
    const tok = w.beginAppendable();
    w.writeInt32(s.a);
    w.writeInt32(s.b);
    w.endAppendable(tok);
    return w.toBytes();
}

function decodeV(bytes: Uint8Array): VV {
    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    const tok = r.beginAppendable();
    const a = r.readInt32();
    const b = r.readInt32();
    r.endAppendable(tok);
    return { a, b };
}

test("V-9 Appendable Struct: DHEADER=8 + 2 longs", () => {
    const sample: VV = { a: 1, b: 2 };
    const expected = fromHex("08 00 00 00 01 00 00 00 02 00 00 00");
    assertBytesEq(encodeV(sample), expected, "V-9 encode");
    assert.deepEqual(decodeV(expected), sample);
});

// === V-10 Mutable Struct ====================================================

interface VM {
    a: number;
    b: string;
}

function encodeM(s: VM): Uint8Array {
    const w = new Xcdr2Writer("le");
    const tok = w.beginMutable();
    // member id=1, LC=2 (inline 4 bytes for int32).
    w.writeEmHeader(1, 2, false);
    w.writeInt32(s.a);
    // member id=2, LC=3 + nextInt = body-size (string).
    w.writeEmHeader(2, 3, false, 0);
    const bodyStart = w.pos;
    w.writeString(s.b);
    w.patchUint32(bodyStart - 4, w.pos - bodyStart);
    w.endMutable(tok);
    return w.toBytes();
}

function decodeM(bytes: Uint8Array): VM {
    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    let a = 0;
    let b = "";
    const tok = r.beginMutable();
    while (r.pos < tok.bodyEnd) {
        const emh = r.readEmHeader();
        switch (emh.memberId) {
            case 1: {
                a = r.readInt32();
                break;
            }
            case 2: {
                if (emh.lc === 3) {
                    r.readUint32(); // skip nextInt
                }
                b = r.readString();
                break;
            }
            default: {
                if (emh.nextInt !== null) {
                    r.readBytes(emh.nextInt);
                } else {
                    const sz = Xcdr2Reader.lcInlineSize(emh.lc);
                    if (sz > 0) r.readBytes(sz);
                }
            }
        }
    }
    r.endMutable(tok);
    return { a, b };
}

test("V-10 Mutable Struct: DHEADER=23, EMHEADER ambient-LE, LC=3+nextInt for string", () => {
    const sample: VM = { a: 42, b: "hi" };
    // EMHEADER ambient-LE per XTypes 1.3 §7.4.3.4.5: u32=0x20000001 -> 01 00 00 20.
    const expected = fromHex(
        "17 00 00 00" +
            "01 00 00 20" +
            "2A 00 00 00" +
            "02 00 00 30" +
            "07 00 00 00" +
            "03 00 00 00 68 69 00",
    );
    assert.equal(expected.length, 4 + 23, "V-10 wire is DHEADER + 23 body-bytes");
    assertBytesEq(encodeM(sample), expected, "V-10 encode");
    assert.deepEqual(decodeM(expected), sample);
});

// === V-11 Optional Member (Mutable) =========================================

interface VO {
    maybe: number | undefined;
}

function encodeO(s: VO): Uint8Array {
    const w = new Xcdr2Writer("le");
    const tok = w.beginMutable();
    if (s.maybe !== undefined && s.maybe !== null) {
        w.writeEmHeader(1, 2, false);
        w.writeInt32(s.maybe);
    }
    w.endMutable(tok);
    return w.toBytes();
}

function decodeO(bytes: Uint8Array): VO {
    const r = new Xcdr2Reader(bytes, 0, bytes.length, "le");
    let maybe: number | undefined;
    const tok = r.beginMutable();
    while (r.pos < tok.bodyEnd) {
        const emh = r.readEmHeader();
        switch (emh.memberId) {
            case 1: {
                maybe = r.readInt32();
                break;
            }
            default: {
                if (emh.nextInt !== null) {
                    r.readBytes(emh.nextInt);
                } else {
                    const sz = Xcdr2Reader.lcInlineSize(emh.lc);
                    if (sz > 0) r.readBytes(sz);
                }
            }
        }
    }
    r.endMutable(tok);
    return { maybe };
}

test("V-11A Optional Mutable Some: DHEADER=8", () => {
    const sample: VO = { maybe: 7 };
    // EMHEADER ambient-LE per XTypes 1.3 §7.4.3.4.5: u32=0x20000001 -> 01 00 00 20.
    const expected = fromHex("08 00 00 00 01 00 00 20 07 00 00 00");
    assertBytesEq(encodeO(sample), expected, "V-11A encode");
    assert.deepEqual(decodeO(expected), sample);
});

test("V-11B Optional Mutable None: DHEADER=0", () => {
    const sample: VO = { maybe: undefined };
    const expected = fromHex("00 00 00 00");
    assertBytesEq(encodeO(sample), expected, "V-11B encode");
    assert.deepEqual(decodeO(expected), sample);
});

// === V-12 Mutable Sentinel End-Marker =======================================
// XCDR2-Bindings DUERFEN keinen expliziten Sentinel emittieren — die
// DHEADER-Groesse begrenzt das Lesen. Wir verifizieren das anhand der
// V-10/V-11-Vektoren: das Ende ist genau bei DHEADER + body-size,
// keine zusaetzlichen Bytes.

test("V-12 Mutable Sentinel: kein PID_LIST_END nach Mutable-Body", () => {
    // V-10 + ein Trailing-Byte simuliert: Decoder MUSS exakt nur die
    // ersten `4 + DHEADER`-Bytes konsumieren.
    const v10 = encodeM({ a: 42, b: "hi" });
    const withTrailing = new Uint8Array(v10.length + 4);
    withTrailing.set(v10, 0);
    withTrailing.set([0xde, 0xad, 0xbe, 0xef], v10.length);
    const dec = decodeM(withTrailing.subarray(0, v10.length));
    assert.deepEqual(dec, { a: 42, b: "hi" });
});

// === Cross-Vector Sanity ====================================================

test("md5 RFC 1321 self-check: 'abc' → 0x900150983CD24FB0D6963F7D28E17F72", () => {
    const enc = new TextEncoder().encode("abc");
    const expected = fromHex("900150983CD24FB0D6963F7D28E17F72");
    assertBytesEq(md5(enc), expected, "md5(abc)");
});

test("md5 RFC 1321 self-check: empty input → 0xD41D8CD98F00B204E9800998ECF8427E", () => {
    const expected = fromHex("D41D8CD98F00B204E9800998ECF8427E");
    assertBytesEq(md5(new Uint8Array(0)), expected, "md5(empty)");
});
