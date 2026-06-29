// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// reader.ts — Xcdr2Reader (Inverse zu Xcdr2Writer).
// Konformanz: OMG XTypes 1.3 §7.4.

import { XcdrError } from './errors.js';
import type { EndianMode } from './types.js';

/// XCDR2 maximum alignment (XTypes 1.3 §7.4.1.1.1 / §7.4.3.2.3 MAXALIGN(2)=4).
/// 64-bit primitives align to 4, not 8 — mirrors the Xcdr2Writer clamp.
const XCDR2_MAX_ALIGN = 4;
/// XCDR1 / classic CDR alignment cap (8). Pass to the reader ctor to decode
/// the legacy wire (no DHEADER on aggregates, PL_CDR1 for @mutable).
export const XCDR1_MAX_ALIGN = 8;

/// XCDR2 reader. Reads from a Uint8Array slice.
export class Xcdr2Reader {
    private readonly view: DataView;
    private readonly bytes: Uint8Array;
    private readonly start: number;
    private readonly end: number;
    private _pos: number;
    private readonly littleEndian: boolean;
    private originStack: number[];
    private readonly maxAlign: number;

    constructor(
        bytes: Uint8Array,
        offset = 0,
        length: number = bytes.length - offset,
        endian: EndianMode = 'le',
        maxAlign: number = XCDR2_MAX_ALIGN,
    ) {
        if (offset < 0 || length < 0 || offset + length > bytes.length) {
            throw new XcdrError(`reader bounds: offset=${offset} length=${length} buf=${bytes.length}`);
        }
        this.bytes = bytes;
        this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        this.start = offset;
        this.end = offset + length;
        this._pos = offset;
        this.littleEndian = endian === 'le';
        this.originStack = [offset];
        // XCDR1 / classic CDR uses an 8-byte alignment cap (no DHEADER,
        // PL_CDR1 for @mutable); XCDR2 caps at 4.
        this.maxAlign = maxAlign === XCDR1_MAX_ALIGN ? XCDR1_MAX_ALIGN : XCDR2_MAX_ALIGN;
    }

    /// `true` when reading the XCDR1 / classic CDR wire.
    get isXcdr1(): boolean {
        return this.maxAlign === XCDR1_MAX_ALIGN;
    }

    /// Aktueller Lese-Cursor (absolute Position im Backing-Buffer).
    get pos(): number {
        return this._pos;
    }

    /// Verbleibende Bytes ab `pos`.
    get remaining(): number {
        return this.end - this._pos;
    }

    get endian(): EndianMode {
        return this.littleEndian ? 'le' : 'be';
    }

    private currentOrigin(): number {
        return this.originStack[this.originStack.length - 1] ?? this.start;
    }

    /// Advances `pos` so that `(pos - origin) % alignment == 0`.
    /// XCDR2 caps the maximum alignment at 4 (XTypes 1.3 §7.4.1.1.1).
    align(alignment: number): void {
        if (alignment > this.maxAlign) {
            alignment = this.maxAlign;
        }
        if (alignment <= 1) {
            return;
        }
        const origin = this.currentOrigin();
        const rel = this._pos - origin;
        const pad = (alignment - (rel % alignment)) % alignment;
        if (pad === 0) {
            return;
        }
        this.requireBytes(pad);
        this._pos += pad;
    }

    pushAlignmentOrigin(): void {
        this.originStack.push(this._pos);
    }

    popAlignmentOrigin(): void {
        if (this.originStack.length <= 1) {
            throw new XcdrError('popAlignmentOrigin: stack underflow');
        }
        this.originStack.pop();
    }

    private requireBytes(n: number): void {
        if (this._pos + n > this.end) {
            throw new XcdrError(
                `read beyond end: need ${n} bytes at pos ${this._pos}, end ${this.end}`,
            );
        }
    }

    // === Primitives ===

    readBool(): boolean {
        this.requireBytes(1);
        const b = this.bytes[this._pos++];
        return b !== 0;
    }

    readOctet(): number {
        this.requireBytes(1);
        return this.bytes[this._pos++]!;
    }

    readChar(): string {
        return String.fromCharCode(this.readOctet());
    }

    readWChar(): string {
        return String.fromCharCode(this.readUint16());
    }

    readInt8(): number {
        this.requireBytes(1);
        const v = this.view.getInt8(this._pos);
        this._pos += 1;
        return v;
    }

    readUint8(): number {
        return this.readOctet();
    }

    readInt16(): number {
        this.align(2);
        this.requireBytes(2);
        const v = this.view.getInt16(this._pos, this.littleEndian);
        this._pos += 2;
        return v;
    }

    readUint16(): number {
        this.align(2);
        this.requireBytes(2);
        const v = this.view.getUint16(this._pos, this.littleEndian);
        this._pos += 2;
        return v;
    }

    readInt32(): number {
        this.align(4);
        this.requireBytes(4);
        const v = this.view.getInt32(this._pos, this.littleEndian);
        this._pos += 4;
        return v;
    }

    readUint32(): number {
        this.align(4);
        this.requireBytes(4);
        const v = this.view.getUint32(this._pos, this.littleEndian);
        this._pos += 4;
        return v;
    }

    readInt64(): bigint {
        this.align(8);
        this.requireBytes(8);
        const v = this.view.getBigInt64(this._pos, this.littleEndian);
        this._pos += 8;
        return v;
    }

    readUint64(): bigint {
        this.align(8);
        this.requireBytes(8);
        const v = this.view.getBigUint64(this._pos, this.littleEndian);
        this._pos += 8;
        return v;
    }

    readFloat32(): number {
        this.align(4);
        this.requireBytes(4);
        const v = this.view.getFloat32(this._pos, this.littleEndian);
        this._pos += 4;
        return v;
    }

    readFloat64(): number {
        this.align(8);
        this.requireBytes(8);
        const v = this.view.getFloat64(this._pos, this.littleEndian);
        this._pos += 8;
        return v;
    }

    /// XTypes §7.4.4.6 — UTF-8 string with a length-incl-NUL prefix.
    readString(): string {
        const len = this.readUint32();
        if (len === 0) {
            // Per spec the length should always be >= 1 (NUL alone),
            // but we accept 0 as empty-defensive behavior.
            return '';
        }
        this.requireBytes(len);
        // length incl. NUL, so -1 for the content.
        const contentLen = len - 1;
        const slice = this.bytes.subarray(this._pos, this._pos + contentLen);
        const s = new TextDecoder('utf-8').decode(slice);
        this._pos += len; // incl. NUL.
        return s;
    }

    /// XTypes §7.4.4.6 — wstring as UTF-16-LE.
    readWString(): string {
        const byteLen = this.readUint32();
        this.align(2);
        this.requireBytes(byteLen);
        const codeUnits = byteLen / 2;
        let s = '';
        for (let i = 0; i < codeUnits; i++) {
            // UTF-16 units in the message byte order, not a hardcoded LE — a
            // big-endian stream carries big-endian units (mirrors writeWString).
            s += String.fromCharCode(this.view.getUint16(this._pos, this.littleEndian));
            this._pos += 2;
        }
        return s;
    }

    readBytes(n: number): Uint8Array {
        this.requireBytes(n);
        const slice = this.bytes.slice(this._pos, this._pos + n);
        this._pos += n;
        return slice;
    }

    /// Reads an IDL `fixed<P,S>` value: (P+2)/2 packed-BCD octets back into a
    /// decimal string (e.g. "123.45"). Inverse of `Xcdr2Writer.writeFixedBcd`;
    /// ported from crates/cdr/src/fixed.rs::to_string_repr.
    readFixedBcd(p: number, s: number): string {
        const n = (p + 2) >> 1;
        const bytes = this.readBytes(n);
        const chars: string[] = [];
        let sign = "+";
        for (let i = 0; i < n; i++) {
            const hi = (bytes[i] >> 4) & 0x0f;
            const lo = bytes[i] & 0x0f;
            chars.push(String.fromCharCode(48 + (hi % 10)));
            if (i === n - 1) sign = lo === 0x0d ? "-" : "+";
            else chars.push(String.fromCharCode(48 + (lo % 10)));
        }
        while (chars.length > s + 1 && chars[0] === "0") chars.shift();
        let out = sign === "-" ? "-" : "";
        if (s > 0) {
            const dotPos = Math.max(chars.length - s, 0);
            for (let i = 0; i < chars.length; i++) {
                if (i === dotPos) out += ".";
                out += chars[i];
            }
        } else {
            out += chars.join("");
        }
        return out;
    }

    /// Begins an appendable block: reads the DHEADER, pushes the origin
    /// to the body-start position, and returns the `bodyEnd` offset
    /// (absolute position in the buffer).
    beginAppendable(): { bodyEnd: number; noFrame?: boolean } {
        // XCDR1 / classic CDR: no DHEADER — the aggregate continues in the same
        // stream with the parent's alignment origin. Frame-less no-op.
        if (this.isXcdr1) {
            return { bodyEnd: this.end, noFrame: true };
        }
        const size = this.readUint32();
        const bodyStart = this._pos;
        this.pushAlignmentOrigin();
        return { bodyEnd: bodyStart + size };
    }

    endAppendable(token: { bodyEnd: number; noFrame?: boolean }): void {
        if (token.noFrame) {
            return; // XCDR1: no frame to close.
        }
        // Skip over unknown trailing bytes.
        if (this._pos < token.bodyEnd) {
            this._pos = token.bodyEnd;
        }
        this.popAlignmentOrigin();
    }

    beginMutable(): { bodyEnd: number } {
        return this.beginAppendable();
    }

    endMutable(token: { bodyEnd: number; noFrame?: boolean }): void {
        this.endAppendable(token);
    }

    /// Begins one PL_CDR1 (@mutable XCDR1) member: a 4-byte aligned
    /// [u16 PID][u16 length] header, then pushes a member-relative origin so the
    /// member's body decodes inline. Returns `null` at the PID_LIST_END sentinel.
    /// PID_EXTENDED (length 8) carries a 32-bit member id + 32-bit body length.
    /// Mirrors cdr-core `xcdr1::read_pl_cdr1_member`.
    beginPlCdr1Member(): { memberId: number; bodyEnd: number } | null {
        const PID_LIST_END = 0x3f02;
        const PID_EXTENDED = 0x3f01;
        this.align(4);
        if (this._pos + 4 > this.end) {
            return null;
        }
        const pid = this.readUint16();
        const lenU16 = this.readUint16();
        if (pid === PID_LIST_END) {
            return null;
        }
        let memberId: number;
        let bodyLen: number;
        if (pid === PID_EXTENDED) {
            memberId = this.readUint32();
            bodyLen = this.readUint32();
        } else {
            memberId = pid;
            bodyLen = lenU16;
        }
        const bodyStart = this._pos;
        this.requireBytes(bodyLen);
        this.pushAlignmentOrigin();
        return { memberId, bodyEnd: bodyStart + bodyLen };
    }

    /// Closes a `beginPlCdr1Member`: pops the origin, positions at the body end,
    /// and skips the trailing 4-byte pad.
    endPlCdr1Member(token: { memberId: number; bodyEnd: number }): void {
        this.popAlignmentOrigin();
        this._pos = token.bodyEnd;
        const pad = (4 - (this._pos & 3)) & 3;
        for (let i = 0; i < pad && this._pos < this.end; i++) {
            this._pos++;
        }
    }

    /// Reads EMHEADER1 (4 bytes BE; see the writer docs) plus
    /// an optional NEXTINT (stream-endian).
    ///
    /// Note: for LC=3 with variable-size members the
    /// standard check yields no NEXTINT (LC=3 = inline 8 bytes per
    /// XTypes 1.3 §7.4.3.4.2). The zerodds conformance convention
    /// (V-10) overrides this in the codegen decode path — it reads
    /// the NEXTINT explicitly after the EMHEADER for non-primitive
    /// members.
    readEmHeader(): { memberId: number; lc: number; mustUnderstand: boolean; nextInt: number | null } {
        // EMHEADER1 is ambient-stream-endian per XTypes 1.3 §7.4.3.4.5.
        const word = this.readUint32();
        const mustUnderstand = (word >>> 31) === 1;
        const lc = (word >>> 28) & 0x7;
        const memberId = word & 0x0fffffff;
        // Only LC4 carries a SEPARATE NEXTINT (uint32 body length) after the
        // EMHEADER. LC5/6/7 reuse the member body's OWN leading 4-byte length
        // word as the framing length (XTypes 1.3 §7.4.3.4.2 /
        // zerodds_cdr::struct_enc::LengthCode::reuses_leading_len), so reading a
        // separate NEXTINT here would wrongly swallow the body's length prefix.
        let nextInt: number | null = null;
        if (lc === 4) {
            nextInt = this.readUint32();
        }
        return { memberId, lc, mustUnderstand, nextInt };
    }

    /// Inline length-code mapping for LC=0..3.
    static lcInlineSize(lc: number): number {
        switch (lc) {
            case 0:
                return 1;
            case 1:
                return 2;
            case 2:
                return 4;
            case 3:
                return 8;
            default:
                return -1;
        }
    }
}
