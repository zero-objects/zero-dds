// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// reader.ts — Xcdr2Reader (Inverse zu Xcdr2Writer).
// Konformanz: OMG XTypes 1.3 §7.4.

import { XcdrError } from './errors.js';
import type { EndianMode } from './types.js';

/// XCDR2 reader. Reads from a Uint8Array slice.
export class Xcdr2Reader {
    private readonly view: DataView;
    private readonly bytes: Uint8Array;
    private readonly start: number;
    private readonly end: number;
    private _pos: number;
    private readonly littleEndian: boolean;
    private originStack: number[];

    constructor(
        bytes: Uint8Array,
        offset = 0,
        length: number = bytes.length - offset,
        endian: EndianMode = 'le',
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
    align(alignment: number): void {
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
            // Per spec, length should always be >= 1 (NUL alone),
            // but we accept 0 as an empty-defensive behaviour.
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
            s += String.fromCharCode(this.view.getUint16(this._pos, true));
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

    /// Begins an appendable block: reads DHEADER, pushes origin
    /// to the body start position, and returns the `bodyEnd` offset
    /// back (absolute position in the buffer).
    beginAppendable(): { bodyEnd: number } {
        const size = this.readUint32();
        const bodyStart = this._pos;
        this.pushAlignmentOrigin();
        return { bodyEnd: bodyStart + size };
    }

    endAppendable(token: { bodyEnd: number }): void {
        // Skip over unknown trailing bytes.
        if (this._pos < token.bodyEnd) {
            this._pos = token.bodyEnd;
        }
        this.popAlignmentOrigin();
    }

    beginMutable(): { bodyEnd: number } {
        return this.beginAppendable();
    }

    endMutable(token: { bodyEnd: number }): void {
        this.endAppendable(token);
    }

    /// Reads EMHEADER1 (4 bytes BE; see writer docs) plus
    /// optional NEXTINT (stream endian).
    ///
    /// Note: for LC=3 with variable-size members, the
    /// standard check yields no NEXTINT (LC=3 = inline 8 bytes per
    /// XTypes 1.3 §7.4.3.4.2). The zerodds conformance convention
    /// (V-10) overrides this in the codegen decode path — it reads
    /// the NEXTINT explicitly after the EMHEADER for non-primitive
    /// Member.
    readEmHeader(): { memberId: number; lc: number; mustUnderstand: boolean; nextInt: number | null } {
        // EMHEADER ist immer Big-Endian.
        this.requireBytes(4);
        const word = this.view.getUint32(this._pos, false);
        this._pos += 4;
        const mustUnderstand = (word >>> 31) === 1;
        const lc = (word >>> 28) & 0x7;
        const memberId = word & 0x0fffffff;
        let nextInt: number | null = null;
        if (lc >= 4) {
            // NEXTINT folgt in Stream-Endian.
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
