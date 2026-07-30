// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable-stream wire codec + sender/receiver state machines + an async,
// window-bounded reliable writer for the native pure-Kotlin endpoint SDK
// (ADR 0013). Byte-identical to crates/xrce, endpoints/c and endpoints/java:
// WRITE_DATA carries the RFC-1982 sample sequence in the 2-byte header field;
// HEARTBEAT/ACKNACK use the golden control-frame layout (session 0x80,
// stream NONE, msg-seq 1) that endpoints/golden-gen emits.
//
// Frame = 8-byte header + body, little-endian:
//   [0]=session [1]=stream [2..4)=seq LE [4]=submsg id [5]=flags
//   [6..8)=body-len LE [8..)=body
//
// Sequence numbers live in the RFC-1982 16-bit space [0,65535]. The in-flight
// map (a TreeMap) stores numeric key order, which is NOT the RFC-1982 order
// across a 16-bit wrap. The HEARTBEAT window bounds are therefore derived from
// an RFC-1982 (seqLt/seqGt) scan of the keys — never from firstKey()/lastKey():
// a window straddling the wrap (0xFFFE,0xFFFF,0x0000,0x0001) has RFC-1982 base
// 0xFFFE / end 0x0001, while the numeric min/max would wrongly report
// 0x0000 / 0xFFFF. Mirrors window_base / serial_max_in_flight in
// crates/xrce/src/reliable.rs.

package zerodds

import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.SocketTimeoutException
import java.util.TreeMap
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

// --- constants ---

const val REL_SESSION_NOKEY = 0x80
const val REL_STREAM_RELIABLE = 0x80 // reliable stream id (bit 7 set)
const val REL_STREAM_NONE = 0x00 // control-frame stream in the golden layout

const val REL_SM_WRITE_DATA = 0x07
const val REL_SM_ACKNACK = 0x0A
const val REL_SM_HEARTBEAT = 0x0B
const val REL_FLAGS_WRITE = 0x03
const val REL_FLAGS_CTRL = 0x01

/** Sender window cap: 16 in-flight samples (matches the 16-bit ACKNACK bitmap). */
const val REL_WINDOW = 16

/** Receiver out-of-order buffer cap (DoS bound). */
const val REL_RECV_BUF = 64

/** Per-sample payload cap: 64 KiB (u16 submessage length limit). */
const val REL_MAX_PAYLOAD = 65535

/** Heartbeat period (spec recommends 100ms; 500ms here, no Tx pacing layer underneath). */
const val REL_HEARTBEAT_PERIOD_MS = 500L

// --- RFC-1982 16-bit circular sequence-number comparisons ---

/** `a < b` per RFC-1982 (wrapping), for a,b in [0,65535]. */
fun seqLt(a: Int, b: Int): Boolean = ((a - b) and 0xFFFF) >= 0x8000

/** `a > b` per RFC-1982 (wrapping). */
fun seqGt(a: Int, b: Int): Boolean {
    val d = (a - b) and 0xFFFF
    return d != 0 && d < 0x8000
}

// --- control-message payloads ---

class Heartbeat(first: Int, last: Int, stream: Int) {
    val first: Int = first and 0xFFFF
    val last: Int = last and 0xFFFF
    val stream: Int = stream and 0xFF
}

class AckNack(firstUnacked: Int, nackLo: Int, nackHi: Int, stream: Int) {
    val firstUnacked: Int = firstUnacked and 0xFFFF
    val nackLo: Int = nackLo and 0xFF
    val nackHi: Int = nackHi and 0xFF
    val stream: Int = stream and 0xFF

    fun bitmap(): Int = (nackLo and 0xFF) or ((nackHi and 0xFF) shl 8)
}

class WriteData(val seq: Int, val sample: ByteArray)

// --- frame builders ---

/** Reliable WRITE_DATA frame: the header seq carries the RFC-1982 sample seq. */
fun reliableWriteFrame(seq: Int, sample: ByteArray): ByteArray {
    val out = ByteArray(8 + sample.size)
    out[0] = REL_SESSION_NOKEY.toByte()
    out[1] = REL_STREAM_RELIABLE.toByte()
    out[2] = (seq and 0xFF).toByte()
    out[3] = ((seq shr 8) and 0xFF).toByte()
    out[4] = REL_SM_WRITE_DATA.toByte()
    out[5] = REL_FLAGS_WRITE.toByte()
    out[6] = (sample.size and 0xFF).toByte()
    out[7] = ((sample.size shr 8) and 0xFF).toByte()
    System.arraycopy(sample, 0, out, 8, sample.size)
    return out
}

// golden layout: session, stream=NONE, msg-seq=1, submsg, flags, len=5
private fun ctrlHeader(submsg: Int): ByteArray = byteArrayOf(
    REL_SESSION_NOKEY.toByte(), REL_STREAM_NONE.toByte(), 0x01, 0x00,
    submsg.toByte(), REL_FLAGS_CTRL.toByte(), 0x05, 0x00,
)

fun heartbeatFrame(h: Heartbeat): ByteArray {
    val out = ByteArray(13)
    System.arraycopy(ctrlHeader(REL_SM_HEARTBEAT), 0, out, 0, 8)
    out[8] = (h.first and 0xFF).toByte()
    out[9] = ((h.first shr 8) and 0xFF).toByte()
    out[10] = (h.last and 0xFF).toByte()
    out[11] = ((h.last shr 8) and 0xFF).toByte()
    out[12] = (h.stream and 0xFF).toByte()
    return out
}

fun acknackFrame(a: AckNack): ByteArray {
    val out = ByteArray(13)
    System.arraycopy(ctrlHeader(REL_SM_ACKNACK), 0, out, 0, 8)
    out[8] = (a.firstUnacked and 0xFF).toByte()
    out[9] = ((a.firstUnacked shr 8) and 0xFF).toByte()
    out[10] = (a.nackLo and 0xFF).toByte()
    out[11] = (a.nackHi and 0xFF).toByte()
    out[12] = (a.stream and 0xFF).toByte()
    return out
}

// --- frame parsers ---

private fun i16(b: ByteArray, off: Int): Int =
    (b[off].toInt() and 0xFF) or ((b[off + 1].toInt() and 0xFF) shl 8)

fun parseWrite(f: ByteArray): WriteData? {
    if (f.size < 8 || (f[4].toInt() and 0xFF) != REL_SM_WRITE_DATA) return null
    // Body is bounded by the declared submessage length, never frame[8..] — a
    // trailing-byte or wrong-length datagram must not leak into the sample.
    val smLen = i16(f, 6)
    if (8 + smLen > f.size) return null
    val seq = i16(f, 2)
    return WriteData(seq, f.copyOfRange(8, 8 + smLen))
}

/** Parses a control frame by byte[4]=submsg-id, body at offset 8 (peer contract). */
fun parseHeartbeat(f: ByteArray): Heartbeat? {
    if (f.size < 13 || (f[4].toInt() and 0xFF) != REL_SM_HEARTBEAT) return null
    return Heartbeat(i16(f, 8), i16(f, 10), f[12].toInt() and 0xFF)
}

fun parseAckNack(f: ByteArray): AckNack? {
    if (f.size < 13 || (f[4].toInt() and 0xFF) != REL_SM_ACKNACK) return null
    return AckNack(i16(f, 8), f[10].toInt() and 0xFF, f[11].toInt() and 0xFF, f[12].toInt() and 0xFF)
}

// --- sender ---

enum class SubmitStatus { OK, PAYLOAD_TOO_LARGE, WINDOW_FULL }

class SubmitResult(val status: SubmitStatus, val seq: Int)

class ReliableSender(stream: Int = REL_STREAM_RELIABLE) {
    private val stream: Int = stream and 0xFF
    private var nextSeq = 0
    private val inFlight = TreeMap<Int, ByteArray>()
    private var haveHeartbeat = false
    private var lastHeartbeatMs = 0L

    /** Submits a new sample. Assigns the next monotonic seqnr on success. */
    fun submit(payload: ByteArray): SubmitResult {
        if (payload.size > REL_MAX_PAYLOAD) return SubmitResult(SubmitStatus.PAYLOAD_TOO_LARGE, -1)
        if (inFlight.size >= REL_WINDOW) return SubmitResult(SubmitStatus.WINDOW_FULL, -1)
        val seq = nextSeq
        inFlight[seq] = payload
        nextSeq = (nextSeq + 1) and 0xFFFF
        return SubmitResult(SubmitStatus.OK, seq)
    }

    fun inFlightCount(): Int = inFlight.size

    /** In-flight payload lookup (e.g. for retransmit). Null if not (or no longer) in-flight. */
    fun getInFlight(seq: Int): ByteArray? = inFlight[seq]

    /** Ascending in-flight seqnrs (deterministic retransmit order). */
    fun inFlightSeqs(): List<Int> = ArrayList(inFlight.keys)

    /**
     * Tick: returns a HEARTBEAT if the heartbeat period elapsed and in-flight
     * samples exist; null otherwise.
     */
    fun pendingHeartbeat(nowMs: Long): Heartbeat? {
        if (inFlight.isEmpty()) return null
        val due = !haveHeartbeat || (nowMs - lastHeartbeatMs) >= REL_HEARTBEAT_PERIOD_MS
        if (!due) return null
        haveHeartbeat = true
        lastHeartbeatMs = nowMs
        // RFC-1982 window base (oldest unacked) and end (newest unacked), NOT the
        // numeric firstKey()/lastKey() — those are wrong across a 16-bit wrap.
        var first = 0
        var last = 0
        var seen = false
        for (k in inFlight.keys) {
            if (!seen) {
                first = k
                last = k
                seen = true
            } else {
                if (seqLt(k, first)) first = k
                if (seqGt(k, last)) last = k
            }
        }
        return Heartbeat(first, last, stream)
    }

    /**
     * Processes an incoming ACKNACK: everything strictly before firstUnacked is
     * acknowledged, and within the 16-slot bitmap a clear bit means acknowledged
     * (prune) while a set bit means still missing (retain for retransmit).
     */
    fun recvAcknack(ack: AckNack) {
        val base = ack.firstUnacked
        val bitmap = ack.bitmap()

        val before = inFlight.keys.filter { seqLt(it, base) }
        for (k in before) inFlight.remove(k)

        for (i in 0 until 16) {
            val seq = (base + i) and 0xFFFF
            if (((bitmap shr i) and 1) == 0) inFlight.remove(seq)
        }
    }

    /** Resets the stream state (e.g. after a RESET submessage). */
    fun reset() {
        nextSeq = 0
        inFlight.clear()
        haveHeartbeat = false
    }
}

// --- receiver ---

enum class RecvStatus { OK, BUFFER_FULL }

class ReliableReceiver(stream: Int = REL_STREAM_RELIABLE) {
    private val stream: Int = stream and 0xFF
    private var expectedSeq = 0
    private val received = TreeMap<Int, ByteArray>()

    /** A sample with seq + payload arrived: buffer it (dup < expected dropped). */
    fun recvData(seq: Int, payload: ByteArray): RecvStatus {
        if (seqLt(seq, expectedSeq)) return RecvStatus.OK // duplicate, already delivered -> drop
        if (received.containsKey(seq)) return RecvStatus.OK // already buffered
        if (received.size >= REL_RECV_BUF) return RecvStatus.BUFFER_FULL
        received[seq] = payload
        return RecvStatus.OK
    }

    /** Returns all contiguous samples available from expected, advancing it. */
    fun drainInOrder(): List<ByteArray> {
        val out = ArrayList<ByteArray>()
        while (true) {
            val payload = received.remove(expectedSeq) ?: break
            out.add(payload)
            expectedSeq = (expectedSeq + 1) and 0xFFFF
        }
        return out
    }

    fun expected(): Int = expectedSeq

    fun outOfOrderCount(): Int = received.size

    /**
     * Computes the ACKNACK for the missing slots in [expected, expected+16).
     * hint (nullable) is the last seqnr the sender is known to have sent (from a
     * HEARTBEAT); slots beyond it are not marked missing.
     */
    fun pendingAcknack(hint: Int?): AckNack {
        var bitmap = 0
        for (i in 0 until 16) {
            val seq = (expectedSeq + i) and 0xFFFF
            if (hint != null && seqGt(seq, hint)) continue
            if (!received.containsKey(seq)) bitmap = bitmap or (1 shl i)
        }
        return AckNack(expectedSeq, bitmap and 0xFF, (bitmap shr 8) and 0xFF, stream)
    }

    fun reset() {
        expectedSeq = 0
        received.clear()
    }
}

// --- async reliable writer ---

// Async-decoupled reliable writer (ADR 0013), mirroring endpoints/java's
// AsyncReliableWriter and endpoints/c's zerodds_reliable_async.[ch]: the
// producer enqueues a raw sample into a bounded queue — no socket syscall, no
// lock contention on the hot path — and returns. A single drain thread owns the
// ReliableSender state end-to-end: it pulls queued samples into the send
// window, frames + sends WRITE_DATA, fires HEARTBEAT on a period gate, and on
// every inbound ACKNACK prunes acknowledged samples and retransmits the ones the
// peer still marks missing. The window (REL_WINDOW=16) bounds in-flight samples;
// a full window backpressures the producer via the bounded queue rather than
// dropping data or spinning — no data loss, no hang.
class AsyncReliableWriter(
    private val peerHost: InetAddress,
    private val peerPort: Int,
    private val drainDeadlineMs: Long = 20_000L,
) : AutoCloseable {
    private val queue = ArrayBlockingQueue<ByteArray>(4096)
    private val sender = ReliableSender()
    private val socket = DatagramSocket().apply { soTimeout = SOCKET_POLL_MS }
    private val producerDone = AtomicBoolean(false)
    private val stopped = AtomicBoolean(false)
    private val deliveredCount = AtomicLong(0)

    @Volatile private var drained = false
    @Volatile private var drainErr: Exception? = null

    private val drainThread = Thread({ drainLoop() }, "zdw-kotlin-reliable-drain").apply {
        isDaemon = true
    }

    fun start() = drainThread.start()

    /** Producer hot path: enqueue only (no syscall). Blocks only if the queue is momentarily full. */
    fun submit(payload: ByteArray) {
        try {
            queue.put(payload)
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
        }
    }

    /** Non-blocking enqueue. */
    fun offer(payload: ByteArray): Boolean = queue.offer(payload)

    /**
     * Signals that no more samples will be submitted, then blocks (up to
     * timeoutMs) until the drain thread has emptied both the queue and the send
     * window (every sample acknowledged). Returns whether it fully drained.
     */
    fun finish(timeoutMs: Long): Boolean {
        producerDone.set(true)
        try {
            drainThread.join(timeoutMs)
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
        }
        return drained
    }

    fun delivered(): Long = deliveredCount.get()

    fun drainError(): Exception? = drainErr

    private fun drainLoop() {
        val buf = ByteArray(8192)
        // Set only once the producer signals done (finish/close); a still-live
        // writer must never be bounded by a global lifetime deadline.
        var drainDeadline = -1L
        try {
            while (!stopped.get()) {
                // 1) Move queued samples into the sender's window.
                while (sender.inFlightCount() < REL_WINDOW) {
                    val payload = queue.poll() ?: break
                    val r = sender.submit(payload)
                    if (r.status != SubmitStatus.OK) break // only payload-too-large lands here
                    send(reliableWriteFrame(r.seq, payload))
                    deliveredCount.incrementAndGet()
                }

                // 2) HEARTBEAT timer (first immediately, then every 500ms while unacked).
                sender.pendingHeartbeat(System.currentTimeMillis())?.let { send(heartbeatFrame(it)) }

                // 3) Drain one inbound ACKNACK (short socket timeout so this never
                //    blocks the producer-queue drain for long).
                val n = try {
                    val pkt = DatagramPacket(buf, buf.size)
                    socket.receive(pkt)
                    pkt.length
                } catch (e: SocketTimeoutException) {
                    -1
                }
                if (n >= 0) {
                    val frame = buf.copyOf(n)
                    parseAckNack(frame)?.let { ack ->
                        sender.recvAcknack(ack) // prune: drops every acknowledged sample
                        retransmitMissing(ack)
                    }
                }

                // 4) Only after the producer signals done: drain queue + window,
                //    but no longer than drainDeadlineMs so an unacked window (dead
                //    peer) cannot hang the thread forever.
                if (producerDone.get()) {
                    if (queue.isEmpty() && sender.inFlightCount() == 0) {
                        drained = true
                        break
                    }
                    if (drainDeadline < 0) drainDeadline = System.currentTimeMillis() + drainDeadlineMs
                    if (System.currentTimeMillis() >= drainDeadline) break
                }
            }
        } catch (e: Exception) {
            drainErr = e
        } finally {
            stopped.set(true)
        }
    }

    private fun retransmitMissing(ack: AckNack) {
        val base = ack.firstUnacked
        val bitmap = ack.bitmap()
        for (i in 0 until 16) {
            if (((bitmap shr i) and 1) == 0) continue // acknowledged / outside in-flight set
            val seq = (base + i) and 0xFFFF
            sender.getInFlight(seq)?.let { send(reliableWriteFrame(seq, it)) }
        }
    }

    private fun send(frame: ByteArray) {
        socket.send(DatagramPacket(frame, frame.size, peerHost, peerPort))
    }

    override fun close() {
        stopped.set(true)
        socket.close()
    }

    companion object {
        private const val SOCKET_POLL_MS = 20
    }
}
