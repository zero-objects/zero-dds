// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// writer.ts — Xcdr2Writer.
// Konformanz: OMG XTypes 1.3 §7.4 + zerodds-xcdr2-bindings-conformance-1.0 §3.
//
// Alignment-Politik: relativ zum Buffer-Start (§7.4.1.5). Beginnt
// ein Sub-Block (DHEADER / EMHEADER-NEXTINT) seine eigene Origin-
// Stelle, kann der Caller `pushAlignmentOrigin()` setzen und nach
// Block-Ende `popAlignmentOrigin()` aufrufen.

import { XcdrError } from './errors.js';
import type { EndianMode } from './types.js';

/// Buffer-Wachstums-Schrittweite (verdoppelt sich automatisch).
const INITIAL_CAPACITY = 64;

/// XCDR2-Writer mit dynamisch wachsendem Uint8Array-Backing.
/// Stateful: hat einen Schreib-Cursor `pos` und eine "Origin"-
/// Position fuer Alignment-Berechnungen.
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

    /// Liefert den geschriebenen Inhalt als neue Uint8Array
    /// (kein Alias auf den internen Buffer).
    toBytes(): Uint8Array {
        return this.buf.slice(0, this._pos);
    }

    /// Endian-Mode fuer Multi-Byte-Werte.
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

    /// Padded den Cursor so dass `(pos - origin) % alignment == 0`.
    /// Schreibt Null-Bytes als Padding.
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
        // Uint8Array ist 0-initialisiert; wir muessen aber sicher
        // gehen, falls Buffer wiederverwendet wuerde — hier OK.
        for (let i = 0; i < pad; i++) {
            this.buf[this._pos + i] = 0;
        }
        this._pos += pad;
    }

    /// Setzt eine neue Alignment-Origin auf die aktuelle Position.
    /// XTypes §7.4.4.4 (DHEADER) reset interne Member-Offsets — Caller
    /// kann das hiermit modellieren wenn die Sprach-Spec das verlangt.
    pushAlignmentOrigin(): void {
        this.originStack.push(this._pos);
    }

    /// Restauriert die vorherige Alignment-Origin.
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
        // length = bytes + 1 (fuer NUL).
        const len = enc.length + 1;
        this.writeUint32(len);
        this.ensureCapacity(len);
        this.buf.set(enc, this._pos);
        this._pos += enc.length;
        this.buf[this._pos++] = 0;
    }

    /// XTypes §7.4.4.6 — wstring als UTF-16-LE auf der Wire.
    /// Wire: uint32 length-in-bytes + UTF-16-LE-Code-Units (kein NUL).
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

    /// Schreibt rohe Bytes ohne Alignment.
    writeBytes(bytes: Uint8Array): void {
        this.ensureCapacity(bytes.length);
        this.buf.set(bytes, this._pos);
        this._pos += bytes.length;
    }

    /// Patcht einen 32-bit-Wert an einer bereits geschriebenen
    /// Position. Wird vom Codegen fuer EMHEADER-NEXTINT-Backpatching
    /// benoetigt.
    patchUint32(pos: number, value: number): void {
        if (pos < 0 || pos + 4 > this._pos) {
            throw new XcdrError(`patchUint32 out of bounds: pos=${pos}, written=${this._pos}`);
        }
        this.view.setUint32(pos, value >>> 0, this.littleEndian);
    }

    // === Extensibility-Helper ===

    /// Beginnt einen Appendable-Block. Reserviert 4 Bytes fuer den
    /// DHEADER (uint32 byte-size der folgenden Member), gibt Token
    /// zurueck zum spaeteren `endAppendable(token)`-Aufruf.
    /// Origin wird auf die Startposition des Member-Body gesetzt
    /// (XTypes §7.4.4.4 — Alignment innerhalb des DHEADER ist
    /// relativ zur Position direkt nach dem DHEADER).
    beginAppendable(): number {
        this.align(4);
        const dheaderPos = this._pos;
        this.ensureCapacity(4);
        // Placeholder — wird in endAppendable() geschrieben.
        this.view.setUint32(dheaderPos, 0, this.littleEndian);
        this._pos += 4;
        this.pushAlignmentOrigin();
        return dheaderPos;
    }

    /// Schliesst einen Appendable-Block: berechnet die Body-Groesse
    /// und schreibt sie an die DHEADER-Position zurueck.
    endAppendable(token: number): void {
        const bodyStart = token + 4;
        const size = this._pos - bodyStart;
        this.view.setUint32(token, size, this.littleEndian);
        this.popAlignmentOrigin();
    }

    /// Beginnt einen Mutable-Block. Identisch zu `beginAppendable`
    /// in Bezug auf DHEADER + Origin-Reset.
    beginMutable(): number {
        return this.beginAppendable();
    }

    /// Schliesst einen Mutable-Block (kein Sentinel; XCDR2 begrenzt
    /// die Read-Range via DHEADER, vgl. §6 V-12).
    endMutable(token: number): void {
        this.endAppendable(token);
    }

    /// XTypes §7.4.3.4.2 EMHEADER1 Encoding (PL_CDR2).
    ///
    /// Wire-Layout (4 Bytes, **always Big-Endian** unabhaengig vom
    /// Stream-Endian — siehe zerodds-xcdr2-bindings-conformance-1.0
    /// §6 V-10/V-11A):
    ///   byte0 = (MU << 7) | (LC << 4) | (id_high_nibble & 0x0F)
    ///   byte1..3 = remaining id-bits in BE
    ///
    /// LC-Werte (zerodds-xcdr2-bindings-conformance-1.0 §6):
    ///   - LC=0/1/2 → inline-size 1/2/4 Bytes (Primitives)
    ///   - LC=3   → fuer Primitives: inline 8 Bytes;
    ///              fuer variable-size Member: Konvention "nextInt
    ///              folgt = body-size in Bytes", siehe V-10
    ///   - LC=4..7 → NEXTINT-Form (32-bit Body-Size)
    ///
    /// EMHEADER1 + optionaler NEXTINT folgen beide dem ambient Stream-
    /// Endian gemaess XTypes 1.3 §7.4.3.4.5. In LE-Stream → LE bytes.
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
