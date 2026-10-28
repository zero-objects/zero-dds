// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the native Kotlin endpoint: byte-identity vs the Rust goldens, plus
// sync + async loopback. Runs as a main (no gradle) — exits non-zero on failure.

import zerodds.AsyncReader
import zerodds.Client
import zerodds.Encoding
import zerodds.LC1
import zerodds.LC2
import zerodds.LC3
import zerodds.Endian
import zerodds.MemTransport
import zerodds.Reader
import zerodds.Writer
import zerodds.xrceReadFrame
import java.io.File
import java.util.concurrent.TimeUnit

/** The DataWriter these tests address (§7.2.1: raw 0x001 + OBJK_DATAWRITER). */
private val WRITER_ID = zerodds.xrceObjectId(0x001, zerodds.OBJK_DATAWRITER)

// Builds an XRCE frame with an arbitrary submessage id + declared body length,
// for exercising the reader's direction + validation logic.
// The body is the 4-octet BaseObjectRequest (§7.7.8) followed by `body` as the
// sample; `declaredLen` is the submessage_length written into the header, so a
// test can declare a wrong one on purpose.
private fun frame(
    id: Int,
    body: ByteArray,
    declaredLen: Int = zerodds.XRCE_BASE_SIZE + body.size,
): ByteArray {
    val out = ByteArray(8 + zerodds.XRCE_BASE_SIZE + body.size)
    out[0] = 0x80.toByte(); out[1] = 0x01
    out[2] = 1; out[3] = 0
    out[4] = id.toByte(); out[5] = zerodds.XRCE_WRITE_FLAGS.toByte()
    out[6] = (declaredLen and 0xFF).toByte(); out[7] = ((declaredLen ushr 8) and 0xFF).toByte()
    out[8] = 0x00; out[9] = 0x01
    out[10] = 0x00; out[11] = 0x15
    System.arraycopy(body, 0, out, 8 + zerodds.XRCE_BASE_SIZE, body.size)
    return out
}

private fun fixture(w: Writer) {
    w.putU32(0xA1B2C3D4L)
    w.putU16(0x1234)
    w.putU8(0x5A)
    w.putF32(3.5f)
    w.putU64(0x0102030405060708L)
    w.putString("bay-12")
    w.putSeqU8(byteArrayOf(0xDE.toByte(), 0xAD.toByte(), 0xBE.toByte(), 0xEF.toByte()))
}

private fun sampleBody(id: Long): ByteArray {
    val w = Writer(Endian.LITTLE, Encoding.XCDR2)
    w.putU32(id)
    w.putU16(0)
    w.putU8(0)
    return w.bytes()
}

fun main() {
    val goldenDir = System.getenv("GOLDEN_DIR") ?: "build"

    // byte-identity
    for ((endian, file) in listOf(Endian.LITTLE to "golden_le.bin", Endian.BIG to "golden_be.bin")) {
        val w = Writer(endian, Encoding.XCDR2)
        fixture(w)
        val golden = File("$goldenDir/$file").readBytes()
        check(w.bytes().contentEquals(golden)) {
            "$file: not byte-identical (got ${w.bytes().size}, want ${golden.size})"
        }
        println("$file: ${golden.size} bytes byte-identical to Rust golden")
    }

    // sync loopback (pull)
    run {
        val t = MemTransport()
        val c = Client(t, WRITER_ID)
        for (i in 0 until 5) check(c.write(sampleBody(0x3000L + i)))
        for (i in 0 until 5) {
            val body = c.poll() ?: error("sync: no sample $i")
            check(Reader(body, Endian.LITTLE, Encoding.XCDR2).getU32() == 0x3000L + i) { "sync: out of order" }
        }
        println("sync loopback: 5 samples in order")
    }

    // async loopback (background thread + channel)
    run {
        val t = MemTransport()
        val w = Client(t, WRITER_ID)
        for (i in 0 until 5) check(w.write(sampleBody(0x1000L + i)))
        val r = AsyncReader(t)
        for (i in 0 until 5) {
            val body = r.samples.poll(5, TimeUnit.SECONDS) ?: error("async: timeout at $i")
            check(Reader(body, Endian.LITTLE, Encoding.XCDR2).getU32() == 0x1000L + i) { "async: out of order" }
        }
        r.close()
        println("async loopback: 5 samples in order")
    }

    // string is UTF-8 with a byte-count length prefix (not the char count).
    run {
        // "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
        val s = "grüße"
        val utf8 = byteArrayOf(0x67, 0x72, 0xC3.toByte(), 0xBC.toByte(), 0xC3.toByte(), 0x9F.toByte(), 0x65)
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putString(s)
        val wire = w.bytes()
        // 4-byte LE length prefix = byte count incl. NUL = 8, NOT the char count (5).
        val prefix = (wire[0].toInt() and 0xFF) or ((wire[1].toInt() and 0xFF) shl 8) or
            ((wire[2].toInt() and 0xFF) shl 16) or ((wire[3].toInt() and 0xFF) shl 24)
        check(prefix == utf8.size + 1) { "length prefix must be UTF-8 byte count + 1, got $prefix" }
        check(wire.copyOfRange(4, 4 + utf8.size).contentEquals(utf8)) { "UTF-8 body mismatch" }
        check(wire[4 + utf8.size].toInt() == 0) { "missing NUL terminator" }
        check(wire.size == 4 + utf8.size + 1) { "total wire length ${wire.size}" }
        check(Reader(wire, Endian.LITTLE, Encoding.XCDR2).getString() == s) { "roundtrip mismatch" }
        println("string UTF-8: byte-count length prefix + roundtrip ok")
    }

    // XRCE framing: direction + negative frame vectors.
    run {
        // Hub -> endpoint direction is DATA (0x09); the reader must accept it.
        val data = byteArrayOf(0xAA.toByte(), 0xBB.toByte(), 0xCC.toByte())
        check(xrceReadFrame(frame(0x09, data))?.contentEquals(data) == true) { "DATA/0x09 body" }
        // Loopback WRITE_DATA (0x07) still accepted.
        val wd = byteArrayOf(0x01, 0x02)
        check(xrceReadFrame(frame(0x07, wd))?.contentEquals(wd) == true) { "WRITE_DATA/0x07 body" }
        // Unknown submessage id -> reject.
        check(xrceReadFrame(frame(0x0A, wd)) == null) { "unknown id must reject" }
        // Truncated header (fewer than 8 bytes) -> reject.
        for (len in 0 until 8 + zerodds.XRCE_BASE_SIZE) {
            check(xrceReadFrame(ByteArray(len)) == null) { "len $len must reject" }
        }
        // Declared length past the datagram -> reject.
        check(xrceReadFrame(frame(0x09, wd, 100)) == null) { "over-long declared length must reject" }
        // Trailing bytes past the declared length must not leak into the body.
        val bounded = xrceReadFrame(frame(0x09, byteArrayOf(1, 2, 3, 4), zerodds.XRCE_BASE_SIZE + 2))
        check(bounded?.contentEquals(byteArrayOf(1, 2)) == true) { "body bounded by declared length" }
        // Writer refuses a sample larger than the 16-bit length field.
        var refused = false
        try {
            zerodds.xrceWriteFrame(0x80, 0x01, 1, WRITER_ID, ByteArray(0x10000))
        } catch (e: IllegalArgumentException) {
            refused = true
        }
        check(refused) { "sample > 0xFFFF must be refused" }
        val maxSample = ByteArray(0xFFFF - zerodds.XRCE_BASE_SIZE)
        check(zerodds.xrceWriteFrame(0x80, 0x01, 1, WRITER_ID, maxSample).size == 8 + 0xFFFF) {
            "a maximal body must still encode"
        }
        println("xrce framing: direction (DATA/0x09) + negative vectors ok")
    }

    surface()

    println("ALL OK")
}

// --- the wire surface, against octets this project did not write ---------
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

private fun hex(b: ByteArray): String {
    val out = StringBuilder(b.size * 2)
    for (x in b) out.append("%02x".format(x.toInt() and 0xFF))
    return out.toString()
}

private fun vector(name: String, w: Writer, want: String) {
    val got = hex(w.bytes())
    check(got == want) { "$name: got $got want $want" }
}

private fun refuses(name: String, body: () -> Unit) {
    try {
        body()
    } catch (e: IllegalArgumentException) {
        return
    }
    error("$name: accepted what it must refuse")
}

private fun surface() {
    // -- the encoding decides MAXALIGN -----------------------------------
    //
    // byte-gate `ext_final`: `@final struct Ext { long a; double b; short c; }`
    // with a = 7, b = 1.5, c = -3. Cyclone, RTI and Fast DDS agree on all four
    // rows. The `double` aligns to eight under XCDR1 and to four under XCDR2,
    // so the same sample is 18 octets in one encoding and 14 in the other —
    // which is the whole of what the encoding argument buys.
    for ((endian, encoding, want) in listOf(
        Triple(Endian.LITTLE, Encoding.XCDR1, "0700000000000000000000000000f83ffdff"),
        Triple(Endian.LITTLE, Encoding.XCDR2, "07000000000000000000f83ffdff"),
        Triple(Endian.BIG, Encoding.XCDR1, "00000007000000003ff8000000000000fffd"),
        Triple(Endian.BIG, Encoding.XCDR2, "000000073ff8000000000000fffd"),
    )) {
        val w = Writer(endian, encoding)
        w.putU32(7)
        w.putF64(1.5)
        w.putU16(-3)
        vector("ext_final/$endian/$encoding (cyclone+rti+fastdds)", w, want)
    }

    // -- DHEADER ----------------------------------------------------------
    //
    // byte-gate `ext_appendable`: the same members under `@appendable`, which
    // XTypes 1.3 §7.3.1.2.1.8 makes the default extensibility. Under XCDR2 a
    // DHEADER precedes the body and counts it — 0x0e = 14 over an 18-octet
    // sample. Under XCDR1 there is no delimiter at all, so the octets are
    // `ext_final`'s. Cyclone, RTI and Fast DDS agree on all four rows.
    for ((endian, want) in listOf(
        Endian.LITTLE to "0e00000007000000000000000000f83ffdff",
        Endian.BIG to "0000000e000000073ff8000000000000fffd",
    )) {
        val w = Writer(endian, Encoding.XCDR2)
        val d = w.dheaderBegin()
        w.putU32(7)
        w.putF64(1.5)
        w.putU16(-3)
        w.dheaderEnd(d)
        vector("ext_appendable/$endian/XCDR2 (cyclone+rti+fastdds)", w, want)
    }

    // -- EMHEADER, in the length codes the vendors chose -------------------
    //
    // byte-gate `ext_mutable` under XCDR2: all three vendors write the same 30
    // octets. The member headers are LC2, LC3 and LC1 — the length is in the
    // code and no NEXTINT is written for any of them — inside a DHEADER of 26.
    for ((endian, want) in listOf(
        Endian.LITTLE to "1a000000000000200700000001000030000000000000f83f02000010fdff",
        Endian.BIG to "0000001a2000000000000007300000013ff800000000000010000002fffd",
    )) {
        val w = Writer(endian, Encoding.XCDR2)
        val d = w.dheaderBegin()
        var m = w.emheaderBegin(0, false, LC2)
        w.putU32(7)
        w.emheaderEnd(m, LC2)
        m = w.emheaderBegin(1, false, LC3)
        w.putF64(1.5)
        w.emheaderEnd(m, LC3)
        m = w.emheaderBegin(2, false, LC1)
        w.putU16(-3)
        w.emheaderEnd(m, LC1)
        w.dheaderEnd(d)
        vector("ext_mutable/$endian/XCDR2 (cyclone+rti+fastdds)", w, want)
    }

    // The must-understand flag is bit 31 of the EMHEADER: `mut_mustunderstand`
    // under XCDR2 reads `010000a0` for id 1 at LC2 with the flag — Cyclone and
    // RTI both write it. Fast DDS writes `01000020` for the same member, i.e.
    // it drops the flag; two of three carry it and this SDK carries it.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        val d = w.dheaderBegin()
        var m = w.emheaderBegin(1, true, LC2)
        w.putU32(7)
        w.emheaderEnd(m, LC2)
        m = w.emheaderBegin(2, false, LC2)
        w.putU32(-2)
        w.emheaderEnd(m, LC2)
        w.dheaderEnd(d)
        vector(
            "mut_mustunderstand/LITTLE/XCDR2 (cyclone+rti; fastdds drops bit 31)",
            w, "10000000010000a00700000002000020feffffff",
        )
    }

    // -- the PL_CDR1 parameter list ---------------------------------------
    //
    // byte-gate `ext_mutable` under XCDR1, RTI's row. Three things it pins: the
    // `double`'s parameter declares 8 and not 12, because its value aligns
    // against its own block; the `short`'s declares 4 and not 2, because the
    // padding that brings the next header back to a four-octet boundary is
    // inside the length; and the sentinel carries the must-understand flag, so
    // it is 0x7F02.
    //
    // The vendors do not agree here and the disagreement is the measurement:
    // Cyclone writes every parameter in the twelve-octet extended form (56
    // octets for this sample), Fast DDS declares 2 for the `short` and closes
    // with the bare 0x3F02 (32 octets, differing from RTI in two places). This
    // SDK writes RTI's form.
    for ((endian, want) in listOf(
        Endian.LITTLE to "000004000700000001000800000000000000f83f02000400fdff0000027f0000",
        Endian.BIG to "0000000400000007000100083ff800000000000000020004fffd00007f020000",
    )) {
        val w = Writer(endian, Encoding.XCDR1)
        var p = w.pidBegin(0, false)
        w.putU32(7)
        w.pidEnd(p)
        p = w.pidBegin(1, false)
        w.putF64(1.5)
        w.pidEnd(p)
        p = w.pidBegin(2, false)
        w.putU16(-3)
        w.pidEnd(p)
        w.pidListEnd()
        vector("ext_mutable/$endian/XCDR1 (rti; cyclone extended, fastdds len 2 + 3f02)", w, want)
    }

    // The parameter must-understand flag is bit 14 of the id field:
    // `mut_mustunderstand` under XCDR1 reads `0140` for id 1 with the flag in
    // RTI's row, and Cyclone sets the same bit inside its extended header.
    // Fast DDS writes `0100` — no flag.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR1)
        var p = w.pidBegin(1, true)
        w.putU32(7)
        w.pidEnd(p)
        p = w.pidBegin(2, false)
        w.putU32(-2)
        w.pidEnd(p)
        w.pidListEnd()
        vector(
            "mut_mustunderstand/LITTLE/XCDR1 (rti; fastdds drops bit 14)",
            w, "014004000700000002000400feffffff027f0000",
        )
    }

    // byte-gate `pid_3f00`: `@id(16127)` and `@id(16128)`, i.e. 0x3EFF and
    // 0x3F00. RTI and Fast DDS both still use the short header for 0x3F00, so
    // the boundary is inclusive — which is what PID_SHORT_MAX_ID says and what
    // pidBegin refuses one past.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR1)
        var p = w.pidBegin(0x3EFF, false)
        w.putU32(7)
        w.pidEnd(p)
        p = w.pidBegin(0x3F00, false)
        w.putU32(9)
        w.pidEnd(p)
        w.pidListEnd()
        vector("pid_3f00/LITTLE/XCDR1 (rti+fastdds; cyclone extended)", w, "ff3e040007000000003f040009000000027f0000")
    }
    refuses("pidBegin(0x3F01)") { Writer(Endian.LITTLE, Encoding.XCDR1).pidBegin(0x3F01, false) }

    // -- the tail is not padded -------------------------------------------
    //
    // byte-gate `tail_pad`: `@final struct Odd { long a; short b; }` with 1 and
    // 2 is six octets in all four rows and all three vendors. A writer that
    // rounded the sample up to a four-octet boundary would add two.
    for ((endian, want) in listOf(Endian.LITTLE to "010000000200", Endian.BIG to "000000010002")) {
        for (encoding in listOf(Encoding.XCDR1, Encoding.XCDR2)) {
            val w = Writer(endian, encoding)
            w.putU32(1)
            w.putU16(2)
            vector("tail_pad/$endian/$encoding (cyclone+rti+fastdds)", w, want)
        }
    }

    // -- the wide character ------------------------------------------------
    //
    // byte-gate `wchar_member`: `@final struct WCh { wchar c; octet tail; }`
    // with U+0061 and 0xAB. Cyclone and Fast DDS write a two-octet code unit in
    // both encodings; RTI writes four octets under XCDR1 and two under XCDR2.
    // This SDK writes two octets always — the majority form, and RTI's own
    // XCDR2 form.
    for ((endian, want) in listOf(Endian.LITTLE to "6100ab", Endian.BIG to "0061ab")) {
        for (encoding in listOf(Encoding.XCDR1, Encoding.XCDR2)) {
            val w = Writer(endian, encoding)
            w.putWchar(0x0061)
            w.putU8(0xAB)
            vector("wchar_member/$endian/$encoding (cyclone+fastdds; rti 4 octets under XCDR1)", w, want)
        }
    }
    refuses("putWchar(U+10000)") { Writer(Endian.LITTLE, Encoding.XCDR2).putWchar(0x10000) }
    Writer(Endian.LITTLE, Encoding.XCDR2).putWchar(0xFFFF) // the last one that fits

    // -- the wide string ---------------------------------------------------
    //
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
    // the code units are not, so its wide units go out little-endian whatever
    // the stream says. RTI's row is `00000004 0061 0062 ab`, every word in the
    // stream's order. Cyclone swaps a standalone `wchar` (its `wchar_member`
    // row is `0061ab`) and does not swap the elements of a `wstring`, which is
    // a disagreement inside one vendor. This SDK writes every unit in the
    // stream's order — RTI's form, and Cyclone's own form for a `wchar`.
    for ((endian, want) in listOf(
        Endian.LITTLE to "0400000061006200ab", // cyclone + rti agree
        Endian.BIG to "0000000400610062ab",    // rti; cyclone leaves the units LE
    )) {
        val w = Writer(endian, Encoding.XCDR2)
        w.putWstring(intArrayOf(0x0061, 0x0062))
        w.putU8(0xAB)
        vector("wstring_member/$endian/XCDR2 (rti; fastdds counts units)", w, want)
    }

    // byte-gate `wstring_nonbmp`: U+1D11E, which is one code point and two
    // UTF-16 code units. Cyclone writes the surrogate pair and a length of four
    // octets. Fast DDS writes `010000001ed1ab` — one unit 0xD11E, the low
    // sixteen bits of the code point: a silent truncation that produces a frame
    // of the right shape holding a different character.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putWstring(intArrayOf(0xD834, 0xDD1E))
        w.putU8(0xAB)
        vector("wstring_nonbmp/LITTLE/XCDR2 (cyclone+rti; fastdds truncates to 0xD11E)", w, "0400000034d81eddab")
        // The same sample big-endian, against RTI's row: every unit in the
        // stream's order. Cyclone keeps the units little-endian here too.
        val be = Writer(Endian.BIG, Encoding.XCDR2)
        be.putWstring(intArrayOf(0xD834, 0xDD1E))
        be.putU8(0xAB)
        vector("wstring_nonbmp/BIG/XCDR2 (rti; cyclone leaves the units LE)", be, "00000004d834dd1eab")
    }

    // -- `char` is one octet and refuses what does not fit ------------------
    //
    // U+0161's low octet is 0x61, which is `a`: a truncating writer produces a
    // frame of the right length holding a plausible, wrong character.
    Writer(Endian.LITTLE, Encoding.XCDR2).putChar(0xFF)
    refuses("putChar(U+0161)") { Writer(Endian.LITTLE, Encoding.XCDR2).putChar(0x0161) }

    // -- the 64-bit float --------------------------------------------------
    //
    // 1.5 is `ext_final`'s `b` and is checked above against three vendors; the
    // rest are the ends of the exponent range, where a narrowing to `float`
    // would show.
    for ((value, want) in listOf(
        1.5 to "000000000000f83f",
        0.0 to "0000000000000000",
        3.141592653589793 to "182d4454fb210940",
        Double.MAX_VALUE to "ffffffffffffef7f",
        Double.MIN_VALUE to "0100000000000000", // the smallest subnormal
    )) {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putF64(value)
        vector("putF64($value)", w, want)
    }

    // -- the ends of every integer range ------------------------------------
    //
    // A width that truncates is the defect this SDK carried: `putU16(70000)`
    // wrote `7011`, the low sixteen bits of 0x11170, with no error anywhere.
    // Each width is checked at 0, at -1, at the signed minimum, at the unsigned
    // maximum and one past it.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putU8(0); w.putU8(255); w.putU8(-128); w.putU8(-1)
        vector("putU8 at its ends", w, "00ff80ff")
    }
    refuses("putU8(256)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU8(256) }
    refuses("putU8(-129)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU8(-129) }
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putU16(0); w.putU16(65535); w.putU16(-32768); w.putU16(-1)
        vector("putU16 at its ends", w, "0000ffff0080ffff")
    }
    refuses("putU16(65536)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU16(65536) }
    refuses("putU16(70000)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU16(70000) }
    refuses("putU16(-32769)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU16(-32769) }
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putU32(0); w.putU32(4294967295L); w.putU32(-2147483648L); w.putU32(-1)
        vector("putU32 at its ends", w, "00000000ffffffff00000080ffffffff")
    }
    refuses("putU32(4294967296)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU32(4294967296L) }
    refuses("putU32(-2147483649)") { Writer(Endian.LITTLE, Encoding.XCDR2).putU32(-2147483649L) }
    run {
        // Every Long is a 64-bit pattern, so the ends are written and not
        // refused: Long.MIN_VALUE is 0x8000000000000000 and -1 is all ones.
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putU64(0); w.putU64(-1); w.putU64(Long.MIN_VALUE); w.putU64(Long.MAX_VALUE)
        vector("putU64 at its ends", w,
            "0000000000000000" + "ffffffffffffffff" + "0000000000000080" + "ffffffffffffff7f")
    }

    // -- the member header refuses what it cannot frame ---------------------
    Writer(Endian.LITTLE, Encoding.XCDR2).emheaderBegin(0x0FFFFFFFL, false, LC2) // the last id that fits
    refuses("emheaderBegin(0x10000000)") {
        Writer(Endian.LITTLE, Encoding.XCDR2).emheaderBegin(0x10000000L, false, LC2)
    }
    refuses("LC2 over a two-octet body") {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        val m = w.emheaderBegin(0, false, LC2)
        w.putU16(1)
        w.emheaderEnd(m, LC2)
    }

    // -- the packed decimal -------------------------------------------------
    //
    // `tests/interop-corpus/fixed-basic` is `@final struct Sample { Money
    // amount; Money debt; octet tail; }` over `typedef fixed<9,3> Money`, with
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
    // packs the significant digits instead of the declared width. TAO produced
    // no bytes at all — `tao_idl` refused the IDL.
    run {
        val w = Writer(Endian.LITTLE, Encoding.XCDR1)
        w.putFixed("123456.789", 9, 3)
        w.putFixed("-000000.001", 9, 3)
        w.putU8(0xAB)
        vector("fixed-basic/LITTLE (omniorb; jacorb packs the second value to one octet)",
            w, "123456789c000000001dab")
        val be = Writer(Endian.BIG, Encoding.XCDR1)
        be.putFixed("123456.789", 9, 3)
        be.putFixed("-000000.001", 9, 3)
        be.putU8(0xAB)
        vector("fixed-basic/BIG (omniorb)", be, "123456789c000000001dab")
        check(hex(be.bytes()) == hex(w.bytes())) { "fixed must not depend on the byte order" }
    }
    for ((value, digits, scale, want) in listOf(
        Quad("1", 1, 0, "1c"),
        Quad("-9", 1, 0, "9d"),
        Quad("12", 2, 0, "012c"),          // P + 1 odd -> a leading zero nibble
        Quad("0.123", 3, 3, "123c"),       // scale == digits: all fraction
        Quad("-0", 1, 0, "0c"),            // a negative zero is written positive
        Quad("-0.000", 3, 3, "000c"),
        Quad("7.5", 9, 3, "000007500c"),   // padding on both sides at once
        Quad("1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901c"),
        Quad("-1234567890123456789012345678901", 31, 0, "1234567890123456789012345678901d"),
    )) {
        val w = Writer(Endian.LITTLE, Encoding.XCDR2)
        w.putFixed(value, digits, scale)
        vector("putFixed($value, $digits, $scale)", w, want)
        check(w.bytes().size == (digits + 2) / 2) { "putFixed($value): wrong octet count" }
    }
    refuses("fixed<6,0> over seven integer digits") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putFixed("1234567", 6, 0)
    }
    refuses("fixed<6,3> over four fraction digits") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putFixed("1.2345", 6, 3)
    }
    refuses("fixed with a scale wider than its digits") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putFixed("1.0", 2, 3)
    }
    refuses("fixed over something that is not a decimal") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putFixed("twelve", 4, 0)
    }
    refuses("fixed over no digits at all") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putFixed("", 4, 0)
    }

    // -- the bitset anchor ---------------------------------------------------
    //
    // The anchor is written at its own width and therefore at its own
    // alignment, which the encoding caps: eight under XCDR1 and four under
    // XCDR2 for an eight-octet anchor.
    run {
        val one = Writer(Endian.LITTLE, Encoding.XCDR1)
        one.putU8(0xAB)
        one.putBitset(1, 8)
        vector("bitset anchor 8 under XCDR1", one, "ab00000000000000" + "0100000000000000")
        val two = Writer(Endian.LITTLE, Encoding.XCDR2)
        two.putU8(0xAB)
        two.putBitset(1, 8)
        vector("bitset anchor 8 under XCDR2", two, "ab000000" + "0100000000000000")
    }
    refuses("a three-octet bitset anchor") {
        Writer(Endian.LITTLE, Encoding.XCDR2).putBitset(0, 3)
    }

    // -- the reading direction ------------------------------------------------
    //
    // The vendors' own octets, read back into the sample they were written
    // from: `ext_appendable` under XCDR2 in both byte orders.
    for ((endian, wire) in listOf(
        Endian.LITTLE to "0e00000007000000000000000000f83ffdff",
        Endian.BIG to "0000000e000000073ff8000000000000fffd",
    )) {
        val bytes = ByteArray(wire.length / 2) { ((wire.substring(it * 2, it * 2 + 2)).toInt(16)).toByte() }
        val r = Reader(bytes, endian, Encoding.XCDR2)
        val body = r.dheaderRead()
        check(body == 14L) { "ext_appendable/$endian: DHEADER reads $body, want 14" }
        check(r.getU32() == 7L) { "ext_appendable/$endian: a" }
        check(r.getF64() == 1.5) { "ext_appendable/$endian: b" }
        check(r.getU16() == 0xFFFD) { "ext_appendable/$endian: c" }
        check(r.pos == bytes.size) { "ext_appendable/$endian: stopped at ${r.pos} of ${bytes.size}" }
    }

    // The vendors' `ext_mutable` under XCDR2, through the member header.
    run {
        val wire = "1a000000000000200700000001000030000000000000f83f02000010fdff"
        val bytes = ByteArray(wire.length / 2) { ((wire.substring(it * 2, it * 2 + 2)).toInt(16)).toByte() }
        val r = Reader(bytes, Endian.LITTLE, Encoding.XCDR2)
        check(r.dheaderRead() == 26L) { "ext_mutable: DHEADER" }
        for ((id, lc, extent) in listOf(Triple(0L, LC2, 4L), Triple(1L, LC3, 8L), Triple(2L, LC1, 2L))) {
            val m = r.emheaderRead()
            check(m.id == id && m.lengthCode == lc && m.extent == extent) {
                "ext_mutable: id=${m.id} lc=${m.lengthCode} extent=${m.extent}, want $id/$lc/$extent"
            }
            r.pos += m.extent.toInt()
        }
        check(r.pos == bytes.size) { "ext_mutable: stopped at ${r.pos} of ${bytes.size}" }
    }

    // RTI's `ext_mutable` under XCDR1, through the parameter list.
    run {
        val wire = "000004000700000001000800000000000000f83f02000400fdff0000027f0000"
        val bytes = ByteArray(wire.length / 2) { ((wire.substring(it * 2, it * 2 + 2)).toInt(16)).toByte() }
        val r = Reader(bytes, Endian.LITTLE, Encoding.XCDR1)
        for ((id, length) in listOf(0L to 4L, 1L to 8L, 2L to 4L)) {
            val p = r.pidRead()
            check(!p.listEnd && p.id == id && p.length == length) {
                "ext_mutable/XCDR1: id=${p.id} len=${p.length}, want $id/$length"
            }
            r.pidSkip(r.pos, p.length)
        }
        check(r.pidRead().listEnd) { "ext_mutable/XCDR1: the sentinel did not end the list" }
        check(r.pos == bytes.size) { "ext_mutable/XCDR1: stopped at ${r.pos} of ${bytes.size}" }
    }

    // Every write reads back, in both encodings and both byte orders.
    for (encoding in listOf(Encoding.XCDR1, Encoding.XCDR2)) {
        for (endian in listOf(Endian.LITTLE, Endian.BIG)) {
            val w = Writer(endian, encoding)
            w.putU8(0xFF); w.putBool(true); w.putChar(0xFF)
            w.putU16(0xFFFF); w.putU32(0xFFFFFFFFL); w.putU64(-1L)
            w.putF32(-3.5f); w.putF64(3.141592653589793)
            w.putWchar(0xFFFF); w.putString("grüße")
            w.putSeqU8(byteArrayOf(0xDE.toByte(), 0xAD.toByte()))
            w.putWstring(intArrayOf(0xD83D, 0xDE00))
            w.putBitset(-0x439EB1FFFFF00001L, 8)
            w.putFixed("-000000.001", 9, 3)

            val r = Reader(w.bytes(), endian, encoding)
            check(r.getU8() == 0xFF) { "roundtrip getU8" }
            check(r.getBool()) { "roundtrip getBool" }
            check(r.getChar() == 0xFF) { "roundtrip getChar" }
            check(r.getU16() == 0xFFFF) { "roundtrip getU16" }
            check(r.getU32() == 0xFFFFFFFFL) { "roundtrip getU32" }
            check(r.getU64() == -1L) { "roundtrip getU64" }
            check(r.getF32() == -3.5f) { "roundtrip getF32" }
            check(r.getF64() == 3.141592653589793) { "roundtrip getF64" }
            check(r.getWchar() == 0xFFFF) { "roundtrip getWchar" }
            check(r.getString() == "grüße") { "roundtrip getString" }
            check(r.getSeqU8().contentEquals(byteArrayOf(0xDE.toByte(), 0xAD.toByte()))) { "roundtrip getSeqU8" }
            check(r.getWstring().contentEquals(intArrayOf(0xD83D, 0xDE00))) { "roundtrip getWstring" }
            check(r.getBitset(8) == -0x439EB1FFFFF00001L) { "roundtrip getBitset" }
            check(r.getFixed(9, 3) == "-000000.001") { "roundtrip getFixed" }
            check(r.pos == w.bytes().size) { "roundtrip/$endian/$encoding stopped at ${r.pos}" }
        }
    }

    println("surface: foreign octets - cyclone, rti and fastdds over 9 byte-gate cases, omniORB over fixed-basic")
}

private data class Quad(val value: String, val digits: Int, val scale: Int, val want: String)
