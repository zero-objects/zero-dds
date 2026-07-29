// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Kotlin endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
// the JVM, byte-identical to the Rust core and the other SDKs. Sync (blocking)
// and async (a background receive thread + a blocking-queue channel).

package zerodds

import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.LinkedBlockingQueue

enum class Endian { LITTLE, BIG }

class Writer(private val endian: Endian) {
    private val buf = ArrayList<Byte>()

    private fun alignTo(a: Int) {
        val cap = if (a > 4) 4 else a
        while (buf.size % cap != 0) buf.add(0)
    }

    private fun putLE(a: Int, le: ByteArray) {
        alignTo(a)
        if (endian == Endian.BIG) for (i in le.indices.reversed()) buf.add(le[i])
        else for (b in le) buf.add(b)
    }

    fun putU8(v: Int) { buf.add(v.toByte()) }
    fun putU16(v: Int) = putLE(2, byteArrayOf(v.toByte(), (v ushr 8).toByte()))
    fun putU32(v: Long) =
        putLE(4, byteArrayOf(v.toByte(), (v ushr 8).toByte(), (v ushr 16).toByte(), (v ushr 24).toByte()))
    fun putU64(v: Long) = putLE(4, ByteArray(8) { i -> (v ushr (8 * i)).toByte() })
    fun putF32(v: Float) = putU32(java.lang.Float.floatToRawIntBits(v).toLong() and 0xFFFFFFFFL)
    fun putBytes(b: ByteArray) { for (x in b) buf.add(x) }
    fun putString(s: String) {
        putU32((s.length + 1).toLong())
        putBytes(s.toByteArray(Charsets.US_ASCII))
        putU8(0)
    }
    fun putSeqU8(b: ByteArray) { putU32(b.size.toLong()); putBytes(b) }
    fun bytes(): ByteArray = buf.toByteArray()
}

class Reader(private val buf: ByteArray, private val endian: Endian) {
    private var pos = 0

    private fun getLE(a: Int, n: Int): ByteArray {
        val cap = if (a > 4) 4 else a
        while (pos % cap != 0) pos++
        val out = buf.copyOfRange(pos, pos + n)
        pos += n
        if (endian == Endian.BIG) out.reverse()
        return out
    }

    fun getU32(): Long {
        val le = getLE(4, 4)
        return (le[0].toLong() and 0xFF) or ((le[1].toLong() and 0xFF) shl 8) or
            ((le[2].toLong() and 0xFF) shl 16) or ((le[3].toLong() and 0xFF) shl 24)
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    fun getU8(): Int = (buf[pos].toInt() and 0xFF).also { pos++ }
    fun getU16(): Int {
        val le = getLE(2, 2)
        return (le[0].toInt() and 0xFF) or ((le[1].toInt() and 0xFF) shl 8)
    }
    fun getU64(): Long {
        val le = getLE(4, 8)
        var v = 0L
        for (i in 0 until 8) v = v or ((le[i].toLong() and 0xFF) shl (8 * i))
        return v
    }
    fun getF32(): Float = java.lang.Float.intBitsToFloat(getU32().toInt())
    fun getString(): String {
        val n = getU32().toInt()
        val s = String(buf, pos, n - 1, Charsets.US_ASCII)
        pos += n
        return s
    }
    fun getSeqU8(): ByteArray {
        val n = getU32().toInt()
        val b = buf.copyOfRange(pos, pos + n)
        pos += n
        return b
    }
}

// --- XRCE framing ---

const val XRCE_SESSION_NOKEY = 0x80
const val XRCE_STREAM_BEST_EFFORT = 0x01

fun xrceWriteFrame(session: Int, stream: Int, seq: Int, sample: ByteArray): ByteArray {
    val out = ByteArray(8 + sample.size)
    out[0] = session.toByte(); out[1] = stream.toByte()
    out[2] = seq.toByte(); out[3] = (seq ushr 8).toByte()
    out[4] = 0x07; out[5] = 0x03
    out[6] = sample.size.toByte(); out[7] = (sample.size ushr 8).toByte()
    System.arraycopy(sample, 0, out, 8, sample.size)
    return out
}

fun xrceReadFrame(frame: ByteArray): ByteArray? {
    if (frame.size < 8 || (frame[4].toInt() and 0xFF) != 0x07) return null
    return frame.copyOfRange(8, frame.size)
}

// --- transport ---

interface Transport {
    fun deliver(frame: ByteArray): Boolean
    // Returns one frame, or null when nothing is ready.
    fun receive(): ByteArray?
}

// --- sync client ---

class Client(
    private val transport: Transport,
    private val session: Int = XRCE_SESSION_NOKEY,
    private val stream: Int = XRCE_STREAM_BEST_EFFORT,
) {
    private var seq = 1
    fun write(sample: ByteArray): Boolean {
        val frame = xrceWriteFrame(session, stream, seq, sample)
        seq++
        return transport.deliver(frame)
    }
    fun poll(): ByteArray? = transport.receive()?.let { xrceReadFrame(it) }
}

// --- async reader: a background thread pushes decoded bodies onto a channel ---

class AsyncReader(private val transport: Transport) {
    val samples = LinkedBlockingQueue<ByteArray>()
    @Volatile private var running = true

    private val thread = Thread {
        while (running) {
            val frame = transport.receive()
            if (frame == null) { Thread.sleep(1); continue }
            xrceReadFrame(frame)?.let { samples.put(it) }
        }
    }.apply { isDaemon = true; start() }

    fun close() { running = false }
}

// A thread-safe in-memory transport used by the tests + example.
class MemTransport : Transport {
    private val q = ConcurrentLinkedQueue<ByteArray>()
    override fun deliver(frame: ByteArray): Boolean { q.add(frame.copyOf()); return true }
    override fun receive(): ByteArray? = q.poll()
}
