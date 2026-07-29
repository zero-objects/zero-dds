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
import java.io.File
import java.util.concurrent.TimeUnit

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

    println("ALL OK")
}
