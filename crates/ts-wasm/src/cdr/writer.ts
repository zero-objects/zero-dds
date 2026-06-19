// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// writer.ts — Xcdr2Writer.
// Conformance: OMG XTypes 1.3 §7.4 + zerodds-xcdr2-bindings-conformance-1.0 §3.
//
// Alignment policy: relative to the buffer start (§7.4.1.5). Begins
// a sub-block (DHEADER / EMHEADER-NEXTINT) gets its own origin
// point, the caller can call `pushAlignmentOrigin()` and after
// the block end call `popAlignmentOrigin()`.

import { XcdrError } from './errors.js';
import type { EndianMode } from './types.js';

/// Buffer growth step (doubles automatically).
const INITIAL_CAPACITY = 64;

/// XCDR2 writer with a dynamically growing Uint8Array backing.
/// Stateful: has a write cursor `pos` and an "origin"
/// position for alignment calculations.
export class Xcdr2Writer {
    private buf: Uint8Array;
    private view: DataView;
    private _pos: number;
    private readonly littleEndian: boolean;
    /// Stack der Alignment-Origin-Positionen (default: [0]).
    private originStack: number[];

    constructor(endian: EndianMode = 'le') {
        this.buf = new Uint8Array(INITIAL_CAPACITY);
        this.view = new DataView(this.buf.buffer);
        this._pos = 0;
        this.littleEndian = endian === 'le';
        this.originStack = [0];
    }

    /// Aktuelle Schreib-Position (Bytes geschrieben).
    get pos(): number {
        return this._pos;
    }

    /// Returns the written content as a new Uint8Array
    /// (not an alias to the internal buffer).
    toBytes(): Uint8Array {
        return this.buf.slice(0, this._pos);
    }

    /// Endian mode for multi-byte values.
    get endian(): EndianMode {
        return this.littleEndian ? 'le' : 'be';
    }

    private ensureCapacity(addBytes: number): void {
        const need = this._pos + addBytes;
        if (need <= this.buf.length) {
            return;
        }
        let cap = this.buf.length;
        while (cap < need) {
            cap *= 2;
        }
        const next = new Uint8Array(cap);
        next.set(this.buf.subarray(0, this._pos));
        this.buf = next;
        this.view = new DataView(this.buf.buffer);
    }

    private currentOrigin(): number {
        return this.originStack[this.originStack.length - 1] ?? 0;
    }

    /// Pads the cursor so that `(pos - origin) % alignment == 0`.
    /// Writes null bytes as padding.
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
        this.ensureCapacity(pad);
        // Uint8Array is 0-initialized; but we must make sure
        // in case the buffer were reused — OK here.
        for (let i = 0; i < pad; i++) {
            this.buf[this._pos + i] = 0;
        }
        this._pos += pad;
    }

    /// Sets a new alignment origin at the current position.
    /// XTypes §7.4.4.4 (DHEADER) resets internal member offsets — the caller
    /// can model this with it if the language spec requires it.
    pushAlignmentOrigin(): void {
        this.originStack.push(this._pos);
    }

    /// Restores the previous alignment origin.
    popAlignmentOrigin(): void {
        if (this.originStack.length <= 1) {
            throw new XcdrError('popAlignmentOrigin: stack underflow');
        }
        this.originStack.pop();
    }

    // === Primitive Writes ===

    writeBool(v: boolean): void {
        this.ensureCapacity(1);
        this.buf[this._pos++] = v ? 1 : 0;
    }

    writeOctet(v: number): void {
        if (v < 0 || v > 0xff || !Number.isInteger(v)) {
            throw new XcdrError(`octet out of range: ${v}`);
        }
        this.ensureCapacity(1);
        this.buf[this._pos++] = v & 0xff;
    }

    writeChar(c: string): void {
        if (c.length !== 1) {
            throw new XcdrError(`char must be single character, got length ${c.length}`);
        }
        const code = c.charCodeAt(0);
        if (code > 0xff) {
            throw new XcdrError(`char out of 1-byte range: U+${code.toString(16)}`);
        }
        this.writeOctet(code);
    }

    writeWChar(c: string): void {
        if (c.length !== 1) {
            throw new XcdrError(`wchar must be single character, got length ${c.length}`);
        }
        this.writeUint16(c.charCodeAt(0));
    }

    writeInt8(v: number): void {
        if (v < -128 || v > 127 || !Number.isInteger(v)) {
            throw new XcdrError(`int8 out of range: ${v}`);
        }
        this.ensureCapacity(1);
        this.view.setInt8(this._pos, v);
        this._pos += 1;
    }

    writeUint8(v: number): void {
        this.writeOctet(v);
    }

    writeInt16(v: number): void {
        if (v < -32768 || v > 32767 || !Number.isInteger(v)) {
            throw new XcdrError(`int16 out of range: ${v}`);
        }
        this.align(2);
        this.ensureCapacity(2);
        this.view.setInt16(this._pos, v, this.littleEndian);
        this._pos += 2;
    }

    writeUint16(v: number): void {
        if (v < 0 || v > 0xffff || !Number.isInteger(v)) {
            throw new XcdrError(`uint16 out of range: ${v}`);
        }
        this.align(2);
        this.ensureCapacity(2);
        this.view.setUint16(this._pos, v, this.littleEndian);
        this._pos += 2;
    }

    writeInt32(v: number): void {
        if (v < -2147483648 || v > 2147483647 || !Number.isInteger(v)) {
            throw new XcdrError(`int32 out of range: ${v}`);
        }
        this.align(4);
        this.ensureCapacity(4);
        this.view.setInt32(this._pos, v, this.littleEndian);
        this._pos += 4;
    }

    writeUint32(v: number): void {
        if (v < 0 || v > 0xffffffff || !Number.isInteger(v)) {
            throw new XcdrError(`uint32 out of range: ${v}`);
        }
        this.align(4);
        this.ensureCapacity(4);
        this.view.setUint32(this._pos, v, this.littleEndian);
        this._pos += 4;
    }

    writeInt64(v: bigint): void {
        const min = -(1n << 63n);
        const max = (1n << 63n) - 1n;
        if (v < min || v > max) {
            throw new XcdrError(`int64 out of range: ${v}`);
        }
        this.align(8);
        this.ensureCapacity(8);
        this.view.setBigInt64(this._pos, v, this.littleEndian);
        this._pos += 8;
    }

    writeUint64(v: bigint): void {
        const max = (1n << 64n) - 1n;
        if (v < 0n || v > max) {
            throw new XcdrError(`uint64 out of range: ${v}`);
        }
        this.align(8);
        this.ensureCapacity(8);
        this.view.setBigUint64(this._pos, v, this.littleEndian);
        this._pos += 8;
    }

    writeFloat32(v: number): void {
        this.align(4);
        this.ensureCapacity(4);
        this.view.setFloat32(this._pos, v, this.littleEndian);
        this._pos += 4;
    }

    writeFloat64(v: number): void {
        this.align(8);
        this.ensureCapacity(8);
        this.view.setFloat64(this._pos, v, this.littleEndian);
        this._pos += 8;
    }

    /// XTypes §7.4.4.6 — bounded/unbounded UTF-8 string.
    /// Wire-Format: uint32 length-with-NUL + UTF-8-bytes + NUL.
    writeString(s: string): void {
        const enc = new TextEncoder().encode(s);
        // length = bytes + 1 (for NUL).
        const len = enc.length + 1;
        this.writeUint32(len);
        this.ensureCapacity(len);
        this.buf.set(enc, this._pos);
        this._pos += enc.length;
        this.buf[this._pos++] = 0;
    }

    /// XTypes §7.4.4.6 — wstring as UTF-16-LE on the wire.
    /// Wire: uint32 length-in-bytes + UTF-16-LE code units (no NUL).
    writeWString(s: string): void {
        const codeUnits = s.length;
        const byteLen = codeUnits * 2;
        this.writeUint32(byteLen);
        this.align(2);
        this.ensureCapacity(byteLen);
        for (let i = 0; i < codeUnits; i++) {
            this.view.setUint16(this._pos, s.charCodeAt(i), true);
            this._pos += 2;
        }
    }

    /// Writes raw bytes without alignment.
    writeBytes(bytes: Uint8Array): void {
        this.ensureCapacity(bytes.length);
        this.buf.set(bytes, this._pos);
        this._pos += bytes.length;
    }

    /// Patches a 32-bit value at an already-written
    /// position. Used by the codegen for EMHEADER NEXTINT backpatching
    /// required.
    patchUint32(pos: number, value: number): void {
        if (pos < 0 || pos + 4 > this._pos) {
            throw new XcdrError(`patchUint32 out of bounds: pos=${pos}, written=${this._pos}`);
        }
        this.view.setUint32(pos, value >>> 0, this.littleEndian);
    }

    // === Extensibility-Helper ===

    /// Begins an appendable block. Reserves 4 bytes for the
    /// DHEADER (uint32 byte-size der folgenden Member), gibt Token
    /// back for the later `endAppendable(token)` call.
    /// The origin is set to the start position of the member body
    /// (XTypes §7.4.4.4 — alignment within the DHEADER is
    /// relative to the position right after the DHEADER).
    beginAppendable(): number {
        this.align(4);
        const dheaderPos = this._pos;
        this.ensureCapacity(4);
        // Placeholder — written in endAppendable().
        this.view.setUint32(dheaderPos, 0, this.littleEndian);
        this._pos += 4;
        this.pushAlignmentOrigin();
        return dheaderPos;
    }

    /// Closes an appendable block: computes the body size
    /// and writes it back to the DHEADER position.
    endAppendable(token: number): void {
        const bodyStart = token + 4;
        const size = this._pos - bodyStart;
        this.view.setUint32(token, size, this.littleEndian);
        this.popAlignmentOrigin();
    }

    /// Begins a mutable block. Identical to `beginAppendable`
    /// in Bezug auf DHEADER + Origin-Reset.
    beginMutable(): number {
        return this.beginAppendable();
    }

    /// Closes a mutable block (no sentinel; XCDR2 bounds
    /// die Read-Range via DHEADER, vgl. §6 V-12).
    endMutable(token: number): void {
        this.endAppendable(token);
    }

    /// XTypes §7.4.3.4.2 EMHEADER1 Encoding (PL_CDR2).
    ///
    /// Wire layout (4 bytes, **always big-endian** independent of the
    /// stream endian — see zerodds-xcdr2-bindings-conformance-1.0
    /// §6 V-10/V-11A):
    ///   byte0 = (MU << 7) | (LC << 4) | (id_high_nibble & 0x0F)
    ///   byte1..3 = remaining id-bits in BE
    ///
    /// LC values (zerodds-xcdr2-bindings-conformance-1.0 §6):
    ///   - LC=0/1/2 → inline-size 1/2/4 Bytes (Primitives)
    ///   - LC=3   → for primitives: inline 8 bytes;
    ///              for variable-size members: convention "nextInt
    ///              folgt = body-size in Bytes", siehe V-10
    ///   - LC=4..7 → NEXTINT-Form (32-bit Body-Size)
    ///
    /// The optional `nextInt` parameter is written in the stream endian
    /// (for an LE stream as LE uint32), not in BE.
    writeEmHeader(memberId: number, lc: number, mustUnderstand = false, nextInt?: number): void {
        if (memberId < 0 || memberId > 0x0fffffff) {
            throw new XcdrError(`member-id out of 28-bit range: ${memberId}`);
        }
        if (lc < 0 || lc > 7) {
            throw new XcdrError(`LC out of 0..7 range: ${lc}`);
        }
        const mu = mustUnderstand ? 1 : 0;
        const word = (mu << 31) | (lc << 28) | (memberId & 0x0fffffff);
        // EMHEADER ist immer Big-Endian (`false` = BE in DataView).
        this.ensureCapacity(4);
        this.view.setUint32(this._pos, word >>> 0, false);
        this._pos += 4;
        if (typeof nextInt === 'number') {
            // NEXTINT folgt in Stream-Endian.
            this.writeUint32(nextInt);
        }
    }
}
