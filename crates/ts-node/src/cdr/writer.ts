// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// writer.ts — Xcdr2Writer.
// Conformance: OMG XTypes 1.3 §7.4 + zerodds-xcdr2-bindings-conformance-1.0 §3.
//
// Alignment policy: relative to the buffer start (§7.4.1.5). If a
// sub-block (DHEADER / EMHEADER NEXTINT) begins its own origin
// point, the caller can set `pushAlignmentOrigin()` and call
// `popAlignmentOrigin()` after the block ends.

import { XcdrError } from './errors.js';
import type { EndianMode } from './types.js';

/// Buffer growth step (doubles automatically).
const INITIAL_CAPACITY = 64;

/// XCDR2 maximum alignment (XTypes 1.3 §7.4.1.1.1 / §7.4.3.2.3 MAXALIGN(2)=4).
/// All alignments are clamped to this; 8-byte primitives align to 4, not 8.
const XCDR2_MAX_ALIGN = 4;
/// XCDR1 / classic CDR alignment cap (8). Pass to the writer ctor for the
/// legacy wire (no DHEADER on aggregates, PL_CDR1 for @mutable).
export const XCDR1_MAX_ALIGN = 8;

/// XCDR2 writer with a dynamically growing Uint8Array backing.
/// Stateful: has a write cursor `pos` and an "origin"
/// position for alignment computations.
export class Xcdr2Writer {
    private buf: Uint8Array;
    private view: DataView;
    private _pos: number;
    private readonly littleEndian: boolean;
    /// Stack of the alignment-origin positions (default: [0]).
    private originStack: number[];
    private readonly maxAlign: number;

    constructor(endian: EndianMode = 'le', maxAlign: number = XCDR2_MAX_ALIGN) {
        this.buf = new Uint8Array(INITIAL_CAPACITY);
        this.view = new DataView(this.buf.buffer);
        this._pos = 0;
        this.littleEndian = endian === 'le';
        this.originStack = [0];
        this.maxAlign = maxAlign === XCDR1_MAX_ALIGN ? XCDR1_MAX_ALIGN : XCDR2_MAX_ALIGN;
    }

    /// `true` when writing the XCDR1 / classic CDR wire.
    get isXcdr1(): boolean {
        return this.maxAlign === XCDR1_MAX_ALIGN;
    }

    /// Current write position (bytes written).
    get pos(): number {
        return this._pos;
    }

    /// Returns the written content as a new Uint8Array
    /// (no alias on the internal buffer).
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
    ///
    /// XCDR2 (= PLAIN_CDR2 / PL_CDR2) caps the maximum alignment at 4
    /// (XTypes 1.3 §7.4.1.1.1; §7.4.3.2.3 INIT MAXALIGN(2)=4): 64-bit
    /// primitives (int64/uint64/float64) align to min(sizeof, 4) = 4, never 8.
    /// This matches the cdr-core `Xcdr2Writer` reference, byte-identical with
    /// Cyclone/FastDDS on the RTPS path.
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
        this.ensureCapacity(pad);
        // Uint8Array is 0-initialized; but we must be sure
        // in case the buffer were reused — OK here.
        for (let i = 0; i < pad; i++) {
            this.buf[this._pos + i] = 0;
        }
        this._pos += pad;
    }

    /// Sets a new alignment origin at the current position.
    /// XTypes §7.4.4.4 (DHEADER) resets internal member offsets — the caller
    /// can model this with it when the language spec requires it.
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
    /// Wire format: uint32 length-with-NUL + UTF-8 bytes + NUL.
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
            // UTF-16 units in the message byte order (mirrors readWString) — a
            // big-endian stream must carry big-endian units, not a hardcoded LE.
            this.view.setUint16(this._pos, s.charCodeAt(i), this.littleEndian);
            this._pos += 2;
        }
    }

    /// Writes raw bytes without alignment.
    writeBytes(bytes: Uint8Array): void {
        this.ensureCapacity(bytes.length);
        this.buf.set(bytes, this._pos);
        this._pos += bytes.length;
    }

    /// Writes an IDL `fixed<P,S>` value (decimal string, e.g. "123.45") as
    /// CORBA/GIOP §9.3.2.7 packed BCD: P digit nibbles MSB-first + a sign
    /// nibble (0xC pos, 0xD neg), a leading 0x0 pad when P+1 is odd, packed
    /// 2 nibbles/byte high-first. (P+2)/2 octets, no length prefix, no
    /// alignment. Ported from crates/cdr/src/fixed.rs (oracle-validated).
    writeFixedBcd(decimal: string, p: number, s: number): void {
        let str = decimal;
        let positive = true;
        if (str.startsWith("-")) { positive = false; str = str.slice(1); }
        else if (str.startsWith("+")) { str = str.slice(1); }
        const dot = str.indexOf(".");
        let intPart = dot < 0 ? str : str.slice(0, dot);
        let fracPart = dot < 0 ? "" : str.slice(dot + 1);
        const intNeeded = p - s;
        if (intPart.length > intNeeded) {
            throw new XcdrError(`fixed: integer part '${intPart}' exceeds P-S=${intNeeded}`);
        }
        if (fracPart.length > s) {
            throw new XcdrError(`fixed: fractional part '${fracPart}' exceeds S=${s}`);
        }
        const digits = intPart.padStart(intNeeded, "0") + fracPart.padEnd(s, "0");
        const nibbles: number[] = [];
        if ((p + 1) % 2 === 1) nibbles.push(0);
        for (const c of digits) {
            const d = c.charCodeAt(0) - 48;
            if (d < 0 || d > 9) throw new XcdrError(`fixed: non-digit '${c}'`);
            nibbles.push(d);
        }
        nibbles.push(positive ? 0x0c : 0x0d);
        const out = new Uint8Array(nibbles.length / 2);
        for (let b = 0; b < out.length; b++) {
            out[b] = (nibbles[2 * b] << 4) | nibbles[2 * b + 1];
        }
        this.writeBytes(out);
    }

    /// Patches a 32-bit value at an already-written
    /// position. Needed by the codegen for EMHEADER NEXTINT backpatching.
    patchUint32(pos: number, value: number): void {
        if (pos < 0 || pos + 4 > this._pos) {
            throw new XcdrError(`patchUint32 out of bounds: pos=${pos}, written=${this._pos}`);
        }
        this.view.setUint32(pos, value >>> 0, this.littleEndian);
    }

    // === Extensibility helpers ===

    /// Begins an appendable block. Reserves 4 bytes for the
    /// DHEADER (uint32 byte-size of the following members), returns a token
    /// for the later `endAppendable(token)` call.
    /// The origin is set to the start position of the member body
    /// (XTypes §7.4.4.4 — alignment within the DHEADER is
    /// relative to the position directly after the DHEADER).
    beginAppendable(): number {
        // XCDR1 / classic CDR: no DHEADER — write the body inline; -1 marks the
        // frame-less token so endAppendable patches nothing.
        if (this.isXcdr1) {
            return -1;
        }
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
        if (token < 0) {
            return; // XCDR1: no frame to close.
        }
        const bodyStart = token + 4;
        const size = this._pos - bodyStart;
        this.view.setUint32(token, size, this.littleEndian);
        this.popAlignmentOrigin();
    }

    /// Begins a mutable block. Identical to `beginAppendable`
    /// regarding DHEADER + origin reset.
    beginMutable(): number {
        return this.beginAppendable();
    }

    /// Closes a mutable block (no sentinel; XCDR2 bounds
    /// the read range via the DHEADER, cf. §6 V-12).
    endMutable(token: number): void {
        this.endAppendable(token);
    }

    /// Writes one PL_CDR1 (@mutable XCDR1) member: a 4-byte aligned
    /// [u16 PID][u16 length] header (PID_EXTENDED long form for ids >= 0x3F00 /
    /// bodies > 0xFFFF), the member `body` bytes (built member-relative), then
    /// zero-pad to the next 4-byte boundary. Mirrors cdr-core
    /// `xcdr1::encode_pl_cdr1_member`.
    writePlCdr1Member(memberId: number, body: Uint8Array): void {
        this.align(4);
        const bodyLen = body.length;
        if (memberId >= 0x3f00 || bodyLen > 0xffff) {
            this.writeUint16(0x3f01);
            this.writeUint16(8);
            this.writeUint32(memberId);
            this.writeUint32(bodyLen);
        } else {
            this.writeUint16(memberId);
            this.writeUint16(bodyLen);
        }
        this.writeBytes(body);
        const pad = (4 - (bodyLen % 4)) % 4;
        for (let i = 0; i < pad; i++) {
            this.writeOctet(0);
        }
    }

    /// Writes the PID_LIST_END (0x3F02) terminator of a PL_CDR1 list.
    writePlCdr1Sentinel(): void {
        this.align(4);
        this.writeUint16(0x3f02);
        this.writeUint16(0);
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
    ///   - LC=0/1/2 → inline size 1/2/4 bytes (primitives)
    ///   - LC=3   → for primitives: inline 8 bytes;
    ///              for variable-size members: convention "nextInt
    ///              follows = body-size in bytes", see V-10
    ///   - LC=4..7 → NEXTINT form (32-bit body size)
    ///
    /// EMHEADER1 + the optional NEXTINT both follow the ambient stream
    /// endian per XTypes 1.3 §7.4.3.4.5. In an LE stream → LE bytes.
    writeEmHeader(memberId: number, lc: number, mustUnderstand = false, nextInt?: number): void {
        if (memberId < 0 || memberId > 0x0fffffff) {
            throw new XcdrError(`member-id out of 28-bit range: ${memberId}`);
        }
        if (lc < 0 || lc > 7) {
            throw new XcdrError(`LC out of 0..7 range: ${lc}`);
        }
        const mu = mustUnderstand ? 1 : 0;
        const word = (mu << 31) | (lc << 28) | (memberId & 0x0fffffff);
        // Stream-Endian via writeUint32 (delegiert an this.endian).
        this.writeUint32(word >>> 0);
        if (typeof nextInt === 'number') {
            this.writeUint32(nextInt);
        }
    }
}
