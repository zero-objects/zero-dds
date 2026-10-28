// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-AMQP 1.0 §7 / §8.1 — the TypeScript runtime the generated helpers
// call.
//
// Standard-library-only: a generated file imports this and needs nothing
// else. The octets it writes are OASIS AMQP 1.0 Part 1 §1.6 primitives;
// the framing decisions belong to the generated helper.
//
// A runtime and not generated code, for the reason the C++ header gives.

export type Bytes = Uint8Array;

/** §7.1.3 — the two halves of a DDS `Time_t`. */
export interface TimeT {
    sec: bigint;
    nanosec: number;
}

function fixed(code: number, value: bigint, width: number): Bytes {
    const out = new Uint8Array(1 + width);
    out[0] = code;
    let rest = BigInt.asUintN(width * 8, value);
    for (let i = width - 1; i >= 0; i--) {
        out[1 + i] = Number(rest & 0xffn);
        rest >>= 8n;
    }
    return out;
}

/** §1.6.1 */
export function encodeNull(): Bytes {
    return Uint8Array.of(0x40);
}

/** §1.6.2 — the compact form. */
export function encodeBoolean(value: boolean): Bytes {
    return Uint8Array.of(value ? 0x41 : 0x42);
}

export function encodeUbyte(value: number): Bytes {
    return fixed(0x50, BigInt(value), 1);
}

export function encodeByte(value: number): Bytes {
    return fixed(0x51, BigInt(value), 1);
}

export function encodeUshort(value: number): Bytes {
    return fixed(0x60, BigInt(value), 2);
}

export function encodeShort(value: number): Bytes {
    return fixed(0x61, BigInt(value), 2);
}

/*
 * §7.1's table gives these four rows two or three codes each — "compact
 * form for -128 <= n <= 127", "compact / zero" — and the choice is part
 * of the encoding a peer compares. The narrow form whenever it fits, in
 * every column.
 */
export function encodeUint(value: number): Bytes {
    if (value === 0) return Uint8Array.of(0x43);
    if (value <= 0xff) return fixed(0x52, BigInt(value), 1);
    return fixed(0x70, BigInt(value), 4);
}

export function encodeInt(value: number): Bytes {
    if (value >= -128 && value <= 127) return fixed(0x54, BigInt(value), 1);
    return fixed(0x71, BigInt(value), 4);
}

export function encodeUlong(value: bigint): Bytes {
    if (value === 0n) return Uint8Array.of(0x44);
    if (value <= 0xffn) return fixed(0x53, value, 1);
    return fixed(0x80, value, 8);
}

export function encodeLong(value: bigint): Bytes {
    if (value >= -128n && value <= 127n) return fixed(0x55, value, 1);
    return fixed(0x81, value, 8);
}

/** §1.6.11 — IEEE 754 binary32, big-endian. */
export function encodeFloat(value: number): Bytes {
    const out = new Uint8Array(5);
    out[0] = 0x72;
    new DataView(out.buffer).setFloat32(1, value, false);
    return out;
}

/** §1.6.12 — IEEE 754 binary64, big-endian. */
export function encodeDouble(value: number): Bytes {
    const out = new Uint8Array(9);
    out[0] = 0x82;
    new DataView(out.buffer).setFloat64(1, value, false);
    return out;
}

/** §1.6.16 — a UTF-32 code point. */
export function encodeChar(codepoint: number): Bytes {
    return fixed(0x73, BigInt(codepoint), 4);
}

/** §1.6.17 — milliseconds since the Unix epoch. */
export function encodeTimestamp(millis: bigint): Bytes {
    return fixed(0x83, millis, 8);
}

/** The narrow form for up to 255 octets and the wide one beyond. */
function lengthPrefixed(narrow: number, wide: number, data: Uint8Array): Bytes {
    if (data.length <= 0xff) {
        const out = new Uint8Array(2 + data.length);
        out[0] = narrow;
        out[1] = data.length;
        out.set(data, 2);
        return out;
    }
    const out = new Uint8Array(5 + data.length);
    out[0] = wide;
    new DataView(out.buffer).setUint32(1, data.length, false);
    out.set(data, 5);
    return out;
}

/** §1.6.20 — str8 / str32-utf8. */
export function encodeStr(value: string): Bytes {
    return lengthPrefixed(0xa1, 0xb1, new TextEncoder().encode(value));
}

/** §1.6.21 — sym8 / sym32. */
export function encodeSymbol(value: string): Bytes {
    return lengthPrefixed(0xa3, 0xb3, new TextEncoder().encode(value));
}

/** §1.6.19 — vbin8 / vbin32. */
export function encodeBinary(value: Uint8Array): Bytes {
    return lengthPrefixed(0xa0, 0xb0, value);
}

export function concat(parts: Bytes[]): Bytes {
    let size = 0;
    for (const part of parts) {
        size += part.length;
    }
    const out = new Uint8Array(size);
    let at = 0;
    for (const part of parts) {
        out.set(part, at);
        at += part.length;
    }
    return out;
}

function compound(narrow: number, wide: number, parts: Bytes[], count: number): Bytes {
    const body = concat(parts);
    if (body.length <= 254 && count <= 255) {
        const out = new Uint8Array(3 + body.length);
        out[0] = narrow;
        out[1] = body.length + 1;
        out[2] = count;
        out.set(body, 3);
        return out;
    }
    const out = new Uint8Array(9 + body.length);
    out[0] = wide;
    const view = new DataView(out.buffer);
    view.setUint32(1, body.length + 4, false);
    view.setUint32(5, count, false);
    out.set(body, 9);
    return out;
}

/**
 * §1.6.22 — list0 / list8 / list32.
 *
 * §7.1.7: "An empty sequence or array SHALL be encoded as the list0
 * type-code 0x45, not as an empty 0xC0."
 */
export function encodeList(items: Bytes[]): Bytes {
    if (items.length === 0) return Uint8Array.of(0x45);
    return compound(0xc0, 0xd0, items, items.length);
}

/** §1.6.23 — map8 / map32. The count is entries times two. */
export function encodeMap(flattened: Bytes[]): Bytes {
    return compound(0xc1, 0xd1, flattened, flattened.length);
}

/** §7.2.1.1 — `0x00`, a `ulong` descriptor, the body. */
export function describedUlong(descriptor: bigint, body: Bytes): Bytes {
    return concat([Uint8Array.of(0x00), encodeUlong(descriptor), body]);
}

/** §7.2.1.2 — a `symbol` and the body, as AMQP §1.3.4's two-element list. */
export function describedSymbol(symbol: string, body: Bytes): Bytes {
    return encodeList([encodeSymbol(symbol), body]);
}

/** §8.1.2 — the symbolic name of an enumerator. */
export function enumName(ordinal: number, literals: string[]): string {
    if (ordinal < 0 || ordinal >= literals.length) {
        throw new RangeError("no enumerator has that ordinal");
    }
    return literals[ordinal];
}

// -- §8.1.2 JSON -----------------------------------------------------

/** RFC 8259 §7. */
export function jsonStr(value: string): string {
    let out = '"';
    for (const character of value) {
        const code = character.codePointAt(0) ?? 0;
        switch (character) {
            case '"':
                out += '\\"';
                break;
            case "\\":
                out += "\\\\";
                break;
            case "\n":
                out += "\\n";
                break;
            case "\r":
                out += "\\r";
                break;
            case "\t":
                out += "\\t";
                break;
            case "\b":
                out += "\\b";
                break;
            case "\f":
                out += "\\f";
                break;
            default:
                if (code < 0x20) {
                    out += "\\u" + code.toString(16).padStart(4, "0");
                } else {
                    out += character;
                }
        }
    }
    return out + '"';
}

export function jsonBool(value: boolean): string {
    return value ? "true" : "false";
}

export function jsonI64(value: bigint | number): string {
    return value.toString();
}

export function jsonU64(value: bigint | number): string {
    return value.toString();
}

/**
 * §8.1.2 — "Floating-point NaN and infinities are not representable in
 * JSON and SHALL cause the encoder to raise an error."
 */
export function jsonDouble(value: number): string {
    if (!Number.isFinite(value)) {
        throw new RangeError("DDS-AMQP 1.0 8.1.2: NaN and infinities are not JSON");
    }
    const written = String(value);
    if (!written.includes(".") && !written.includes("e") && !written.includes("E")) {
        return written + ".0";
    }
    return written;
}

export function jsonChar(codepoint: number): string {
    return jsonStr(String.fromCodePoint(codepoint));
}

/** §8.1.2 — "octet sequences: Base64 (RFC 4648) string." */
export function jsonBase64(bytes: Uint8Array): string {
    const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let out = "";
    for (let i = 0; i < bytes.length; i += 3) {
        const remaining = bytes.length - i;
        let triple = bytes[i] << 16;
        if (remaining > 1) triple |= bytes[i + 1] << 8;
        if (remaining > 2) triple |= bytes[i + 2];
        const chunk = remaining < 3 ? remaining : 3;
        for (let position = 0; position < 4; position++) {
            out += position > chunk ? "=" : alphabet[(triple >> (18 - 6 * position)) & 0x3f];
        }
    }
    return jsonStr(out);
}

/** §8.1.2 — "Time_t: JSON object {"sec": n, "nanosec": m}." */
export function jsonTime(sec: bigint | number, nanosec: number): string {
    return '{"sec":' + sec.toString() + ',"nanosec":' + nanosec.toString() + "}";
}

/** §7.1.3 — the standard Time_t mapping, in milliseconds. */
export function timeMillis(sec: bigint | number, nanosec: number): bigint {
    return BigInt(sec) * 1000n + BigInt(Math.floor(nanosec / 1000000));
}
