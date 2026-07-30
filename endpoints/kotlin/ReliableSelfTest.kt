// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit suite mirroring crates/xrce/src/reliable.rs + byte-golden assertion for
// the pure-Kotlin reliable-stream classes (zerodds.*).
// Usage: ReliableSelfTestKt [golden_heartbeat_le.bin golden_acknack_le.bin]
// Prints "ALL OK" and exits 0 on success.

import zerodds.AckNack
import zerodds.Heartbeat
import zerodds.RecvStatus
import zerodds.ReliableReceiver
import zerodds.ReliableSender
import zerodds.SubmitStatus
import zerodds.REL_MAX_PAYLOAD
import zerodds.REL_RECV_BUF
import zerodds.REL_WINDOW
import zerodds.acknackFrame
import zerodds.heartbeatFrame
import java.io.File

private fun check(cond: Boolean, msg: String) {
    if (!cond) {
        println("FAIL: $msg")
        throw AssertionError(msg)
    }
}

fun main(args: Array<String>) {
    // --- Sender ---
    run {
        val s = ReliableSender()
        val a = s.submit(byteArrayOf(1, 2))
        val b = s.submit(byteArrayOf(3, 4))
        check(a.status == SubmitStatus.OK && a.seq == 0, "monotonic seq 0")
        check(b.status == SubmitStatus.OK && b.seq == 1, "monotonic seq 1")
        check(s.inFlightCount() == 2, "in-flight count")
    }
    run {
        val s = ReliableSender()
        val huge = ByteArray(REL_MAX_PAYLOAD + 1)
        check(s.submit(huge).status == SubmitStatus.PAYLOAD_TOO_LARGE, "payload too large")
    }
    run {
        val s = ReliableSender()
        for (i in 0 until REL_WINDOW) check(s.submit(byteArrayOf(0)).status == SubmitStatus.OK, "fill window")
        check(s.submit(byteArrayOf(0)).status == SubmitStatus.WINDOW_FULL, "window full")
    }
    run {
        val s = ReliableSender()
        s.submit(byteArrayOf(1))
        val base = 1_000_000L // arbitrary epoch; only deltas matter
        val hb = s.pendingHeartbeat(base)
        check(hb != null, "heartbeat fires first")
        check(hb!!.first == 0 && hb.last == 0 && hb.stream == 0x80, "heartbeat body")
        check(s.pendingHeartbeat(base + 100) == null, "heartbeat silenced <500ms")
        check(s.pendingHeartbeat(base + 600) != null, "heartbeat after 500ms")
    }
    run {
        val s = ReliableSender()
        check(s.pendingHeartbeat(0L) == null, "no heartbeat when empty")
    }
    run {
        val s = ReliableSender()
        s.submit(byteArrayOf(0xA0.toByte())) // seq 0
        s.submit(byteArrayOf(0xA1.toByte())) // seq 1
        s.submit(byteArrayOf(0xA2.toByte())) // seq 2
        // base=2, bitmap=0b1 -> seq2 still missing, 0+1 acknowledged
        s.recvAcknack(AckNack(2, 0x01, 0x00, 0x80))
        check(s.inFlightCount() == 1, "acknack clears acked")
        check(s.getInFlight(2) != null, "seq2 retransmittable")
    }
    run {
        val s = ReliableSender()
        for (i in 0 until 5) s.submit(byteArrayOf(0))
        s.recvAcknack(AckNack(5, 0, 0, 0x80)) // full clear
        check(s.inFlightCount() == 0, "acknack full clear")
    }

    // --- Receiver ---
    run {
        val r = ReliableReceiver()
        r.recvData(0, byteArrayOf(10))
        r.recvData(1, byteArrayOf(11))
        val d = r.drainInOrder()
        check(d.size == 2 && d[0][0].toInt() == 10 && d[1][0].toInt() == 11, "in-order drain")
        check(r.expected() == 2, "expected advanced")
    }
    run {
        val r = ReliableReceiver()
        r.recvData(2, byteArrayOf(22))
        r.recvData(0, byteArrayOf(20))
        val d1 = r.drainInOrder()
        check(d1.size == 1 && d1[0][0].toInt() == 20, "reorder: only seq0")
        r.recvData(1, byteArrayOf(21))
        val d2 = r.drainInOrder()
        check(d2.size == 2 && d2[0][0].toInt() == 21 && d2[1][0].toInt() == 22, "reorder: 1+2")
    }
    run {
        val r = ReliableReceiver()
        r.recvData(0, byteArrayOf(1))
        r.drainInOrder()
        r.recvData(0, byteArrayOf(99)) // duplicate
        check(r.outOfOrderCount() == 0, "duplicate dropped")
    }
    run {
        val r = ReliableReceiver()
        for (i in 1..REL_RECV_BUF) check(r.recvData(i, byteArrayOf(1)) == RecvStatus.OK, "fill recv buffer")
        check(r.recvData(REL_RECV_BUF + 1, byteArrayOf(1)) == RecvStatus.BUFFER_FULL, "recv buffer full")
    }
    run {
        val r = ReliableReceiver()
        r.recvData(1, byteArrayOf(1))
        r.recvData(3, byteArrayOf(3))
        val bm = r.pendingAcknack(3).bitmap()
        check((bm and 1) != 0, "slot 0 missing")
        check((bm and (1 shl 2)) != 0, "slot 2 missing")
        check((bm and (1 shl 1)) == 0, "slot 1 present")
        check((bm and (1 shl 3)) == 0, "slot 3 present")
    }
    run {
        val r = ReliableReceiver()
        r.recvData(0, byteArrayOf(3))
        r.reset()
        check(r.expected() == 0 && r.outOfOrderCount() == 0, "reset clears receiver")
    }

    // --- end-to-end loss recovery (in-process) ---
    run {
        val s = ReliableSender()
        val r = ReliableReceiver()
        val seqs = IntArray(3)
        for (i in 0 until 3) seqs[i] = s.submit(byteArrayOf(i.toByte())).seq
        r.recvData(seqs[0], byteArrayOf(0)) // seq1 lost
        r.recvData(seqs[2], byteArrayOf(2))
        val d = r.drainInOrder()
        check(d.size == 1, "only seq0 before recovery")
        val ack = r.pendingAcknack(seqs[2])
        s.recvAcknack(ack)
        check(s.getInFlight(seqs[1]) != null, "seq1 retransmittable")
        r.recvData(seqs[1], s.getInFlight(seqs[1])!!)
        val d2 = r.drainInOrder()
        check(d2.size == 2, "seq1+2 after recovery")
    }

    // --- RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit
    //     wrap (mirrors crates/xrce's wrap regression tests). Seeds the
    //     sender/receiver up to the wrap using only the public API, then
    //     straddles 0x0000.
    run {
        val s = ReliableSender()
        var seq: Int // walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
        do {
            seq = s.submit(byteArrayOf(0)).seq
            s.recvAcknack(AckNack((seq + 1) and 0xFFFF, 0, 0, 0x80))
        } while (seq != 0xFFFD)
        check(s.inFlightCount() == 0, "wrap seed: sender window drained")

        val q0 = s.submit(byteArrayOf(10)).seq // 0xFFFE
        val q1 = s.submit(byteArrayOf(11)).seq // 0xFFFF (lost below)
        val q2 = s.submit(byteArrayOf(12)).seq // 0x0000
        val q3 = s.submit(byteArrayOf(13)).seq // 0x0001
        check(q0 == 0xFFFE && q1 == 0xFFFF && q2 == 0x0000 && q3 == 0x0001, "wrap seqs")

        val hbw = s.pendingHeartbeat(0L)
        check(
            hbw != null && hbw.first == 0xFFFE && hbw.last == 0x0001,
            "heartbeat window across wrap = [0xFFFE,0x0001] (not numeric 0x0000,0xFFFF)",
        )

        val r = ReliableReceiver() // seed expected to 0xFFFE
        for (k in 0..0xFFFD) {
            r.recvData(k, byteArrayOf(0))
            r.drainInOrder()
        }
        check(r.expected() == 0xFFFE, "wrap seed: receiver expects 0xFFFE")

        r.recvData(q0, byteArrayOf(10)) // 0xFFFF lost
        r.recvData(q2, byteArrayOf(12))
        r.recvData(q3, byteArrayOf(13))
        val dw = r.drainInOrder()
        check(dw.size == 1 && dw[0][0].toInt() == 10, "only 0xFFFE before recovery")
        check(r.expected() == 0xFFFF, "receiver blocked at 0xFFFF")

        val ackw = r.pendingAcknack(q3)
        check(ackw.firstUnacked == 0xFFFF, "acknack base = 0xFFFF across wrap")
        val bmw = ackw.bitmap()
        check((bmw and 0b1) != 0, "0xFFFF NACKed")
        check((bmw and 0b110) == 0, "0x0000/0x0001 present")

        s.recvAcknack(ackw)
        check(s.getInFlight(q1) != null, "0xFFFF retransmittable")
        check(
            s.getInFlight(q0) == null && s.getInFlight(q2) == null && s.getInFlight(q3) == null,
            "0xFFFE/0x0000/0x0001 acked",
        )
        check(s.inFlightCount() == 1, "only 0xFFFF left in-flight")

        r.recvData(q1, s.getInFlight(q1)!!)
        val dw2 = r.drainInOrder()
        check(
            dw2.size == 3 && dw2[0][0].toInt() == 11 && dw2[1][0].toInt() == 12 && dw2[2][0].toInt() == 13,
            "0xFFFF,0x0000,0x0001 deliver in RFC-1982 order",
        )
    }

    // --- byte-golden (hardcoded) ---
    val hb = heartbeatFrame(Heartbeat(1, 3, 0x80))
    val hbExpect = byteArrayOf(
        0x80.toByte(), 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80.toByte(),
    )
    check(hb.contentEquals(hbExpect), "heartbeat byte-golden (hardcoded)")

    val ak = acknackFrame(AckNack(1, 0, 0, 0x80))
    val akExpect = byteArrayOf(
        0x80.toByte(), 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80.toByte(),
    )
    check(ak.contentEquals(akExpect), "acknack byte-golden (hardcoded)")

    if (args.size >= 2) {
        val gHb = File(args[0]).readBytes()
        val gAk = File(args[1]).readBytes()
        check(hb.contentEquals(gHb), "heartbeat byte-identical to golden file")
        check(ak.contentEquals(gAk), "acknack byte-identical to golden file")
        println("golden files matched")
    }

    println("ALL OK")
}
