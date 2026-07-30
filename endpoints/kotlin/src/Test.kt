// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the native Kotlin endpoint: byte-identity vs the Rust goldens, plus
// sync + async loopback. Runs as a main (no gradle) — exits non-zero on failure.

import zerodds.AsyncReader
import zerodds.Client
import zerodds.Endian
import zerodds.MemTransport
import zerodds.Reader
import zerodds.Writer
import zerodds.xrceReadFrame
import java.io.File
import java.util.concurrent.TimeUnit

// Builds an XRCE frame with an arbitrary submessage id + declared body length,
// for exercising the reader's direction + validation logic.
private fun frame(id: Int, body: ByteArray, declaredLen: Int = body.size): ByteArray {
    val out = ByteArray(8 + body.size)
    out[0] = 0x80.toByte(); out[1] = 0x01
    out[2] = 1; out[3] = 0
    out[4] = id.toByte(); out[5] = 0x03
    out[6] = (declaredLen and 0xFF).toByte(); out[7] = ((declaredLen ushr 8) and 0xFF).toByte()
    System.arraycopy(body, 0, out, 8, body.size)
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
    val w = Writer(Endian.LITTLE)
    w.putU32(id)
    w.putU16(0)
    w.putU8(0)
    return w.bytes()
}

fun main() {
    val goldenDir = System.getenv("GOLDEN_DIR") ?: "build"

    // byte-identity
    for ((endian, file) in listOf(Endian.LITTLE to "golden_le.bin", Endian.BIG to "golden_be.bin")) {
        val w = Writer(endian)
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
        val c = Client(t)
        for (i in 0 until 5) check(c.write(sampleBody(0x3000L + i)))
        for (i in 0 until 5) {
            val body = c.poll() ?: error("sync: no sample $i")
            check(Reader(body, Endian.LITTLE).getU32() == 0x3000L + i) { "sync: out of order" }
        }
        println("sync loopback: 5 samples in order")
    }

    // async loopback (background thread + channel)
    run {
        val t = MemTransport()
        val w = Client(t)
        for (i in 0 until 5) check(w.write(sampleBody(0x1000L + i)))
        val r = AsyncReader(t)
        for (i in 0 until 5) {
            val body = r.samples.poll(5, TimeUnit.SECONDS) ?: error("async: timeout at $i")
            check(Reader(body, Endian.LITTLE).getU32() == 0x1000L + i) { "async: out of order" }
        }
        r.close()
        println("async loopback: 5 samples in order")
    }

    // string is UTF-8 with a byte-count length prefix (not the char count).
    run {
        // "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
        val s = "grüße"
        val utf8 = byteArrayOf(0x67, 0x72, 0xC3.toByte(), 0xBC.toByte(), 0xC3.toByte(), 0x9F.toByte(), 0x65)
        val w = Writer(Endian.LITTLE)
        w.putString(s)
        val wire = w.bytes()
        // 4-byte LE length prefix = byte count incl. NUL = 8, NOT the char count (5).
        val prefix = (wire[0].toInt() and 0xFF) or ((wire[1].toInt() and 0xFF) shl 8) or
            ((wire[2].toInt() and 0xFF) shl 16) or ((wire[3].toInt() and 0xFF) shl 24)
        check(prefix == utf8.size + 1) { "length prefix must be UTF-8 byte count + 1, got $prefix" }
        check(wire.copyOfRange(4, 4 + utf8.size).contentEquals(utf8)) { "UTF-8 body mismatch" }
        check(wire[4 + utf8.size].toInt() == 0) { "missing NUL terminator" }
        check(wire.size == 4 + utf8.size + 1) { "total wire length ${wire.size}" }
        check(Reader(wire, Endian.LITTLE).getString() == s) { "roundtrip mismatch" }
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
        for (len in 0 until 8) check(xrceReadFrame(ByteArray(len)) == null) { "len $len must reject" }
        // Declared length past the datagram -> reject.
        check(xrceReadFrame(frame(0x09, wd, 100)) == null) { "over-long declared length must reject" }
        // Trailing bytes past the declared length must not leak into the body.
        val bounded = xrceReadFrame(frame(0x09, byteArrayOf(1, 2, 3, 4), 2))
        check(bounded?.contentEquals(byteArrayOf(1, 2)) == true) { "body bounded by declared length" }
        // Writer refuses a sample larger than the 16-bit length field.
        var refused = false
        try {
            zerodds.xrceWriteFrame(0x80, 0x01, 1, ByteArray(0x10000))
        } catch (e: IllegalArgumentException) {
            refused = true
        }
        check(refused) { "sample > 0xFFFF must be refused" }
        check(zerodds.xrceWriteFrame(0x80, 0x01, 1, ByteArray(0xFFFF)).size == 8 + 0xFFFF) { "0xFFFF must still encode" }
        println("xrce framing: direction (DATA/0x09) + negative vectors ok")
    }

    println("ALL OK")
}
