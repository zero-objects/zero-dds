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
    // Wire rule (docs/specs/zerodds-xcdr2-kotlin-1.0.md §"string"): uint32
    // (UTF-8 byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the
    // BYTE count of the UTF-8 encoding, not the Kotlin char count — multibyte
    // characters occupy more than one byte. Byte-identical to the C/Rust core.
    fun putString(s: String) {
        val raw = s.toByteArray(Charsets.UTF_8)
        putU32((raw.size + 1).toLong())
        putBytes(raw)
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
    // Inverse of putString: read n bytes (n incl. the NUL), decode the first
    // n-1 as UTF-8.
    fun getString(): String {
        val n = getU32().toInt()
        val s = String(buf, pos, n - 1, Charsets.UTF_8)
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

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF; refuse rather than truncate the length while copying the full payload
// (the C SDK rejects the same way).
fun xrceWriteFrame(session: Int, stream: Int, seq: Int, sample: ByteArray): ByteArray {
    require(sample.size <= 0xFFFF) { "sample exceeds 16-bit submessage_length" }
    val out = ByteArray(8 + sample.size)
    out[0] = session.toByte(); out[1] = stream.toByte()
    out[2] = seq.toByte(); out[3] = (seq ushr 8).toByte()
    out[4] = 0x07; out[5] = 0x03
    out[6] = sample.size.toByte(); out[7] = (sample.size ushr 8).toByte()
    System.arraycopy(sample, 0, out, 8, sample.size)
    return out
}

// Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent — what
// this endpoint SENDS (xrceWriteFrame); DATA (0x09) is agent->client — what it
// RECEIVES from the hub. The reader accepts both (parity with the C/Python
// SDKs): 0x09 for real hub traffic, 0x07 for loopback. It returns null — never
// throws, never a bogus sample — on a short header, an unknown submessage id, or
// a declared length that runs past the datagram (truncated / wrong length); the
// body is bounded by the declared length so trailing bytes never leak in.
fun xrceReadFrame(frame: ByteArray): ByteArray? {
    if (frame.size < 8) return null
    val id = frame[4].toInt() and 0xFF
    if (id != 0x09 && id != 0x07) return null
    val smLen = (frame[6].toInt() and 0xFF) or ((frame[7].toInt() and 0xFF) shl 8)
    if (8 + smLen > frame.size) return null
    return frame.copyOfRange(8, 8 + smLen)
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

// A real UDP transport: one DatagramSocket carries both directions, so the peer
// replies to the app's source port. `receive()` polls with a short SO_TIMEOUT
// and returns null on expiry (never blocks the AsyncReader drain forever). This
// is the drop-in the QUICKSTART mentions — the sync Client and the AsyncReader
// run unchanged over a live datagram link instead of MemTransport.
class UdpTransport(
    private val host: java.net.InetAddress,
    private val port: Int,
    pollMs: Int = 100,
) : Transport {
    private val sock = java.net.DatagramSocket().apply { soTimeout = pollMs }

    override fun deliver(frame: ByteArray): Boolean {
        sock.send(java.net.DatagramPacket(frame, frame.size, host, port))
        return true
    }

    override fun receive(): ByteArray? {
        val buf = ByteArray(4096)
        val pkt = java.net.DatagramPacket(buf, buf.size)
        return try {
            sock.receive(pkt)
            pkt.data.copyOf(pkt.length)
        } catch (e: java.net.SocketTimeoutException) {
            null
        }
    }

    fun close() = sock.close()
}
