// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// example_reliable app: argv = <peer-port> [N]. Submits N samples through the
// AsyncReliableWriter (zerodds.*) -- the producer path is a pure queue enqueue;
// a drain Thread owns the reliable sender state, sends WRITE_DATA over a real
// DatagramSocket, fires HEARTBEAT on a timer, and retransmits on ACKNACK until
// the send window drains. The peer (zerodds-endpoint-e2e's
// bind_reliable_peer/reliable_receive) injects loss and replies ACKNACK; loss
// recovery is proven when every sample lands gap-free and in order on the peer.
//
// `<peer-port> 0 bench`: producer latency, the decoupled enqueue (`offer`, no
// socket call) against the inline send of the same frame on the producer
// (reliable-endpoint 1.0 §5 point 4). Prints one `BENCH` line.

import zerodds.AsyncReliableWriter
import zerodds.reliableWriteFrame
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import kotlin.system.exitProcess

/** The DataWriter of every frame's BaseObjectRequest (DDS-XRCE 1.0 §7.7.8). */
const val WRITER_ID = 0x0015

/** Below the writer's queue capacity of 4096: no enqueue meets backpressure. */
const val BENCH_ITERS = 4000

/** Nanoseconds per send of [iters] frames, the producer framing and sending each itself. */
fun inlineSendNs(host: InetAddress, port: Int, iters: Int): Double {
    val sample = ByteArray(4)
    DatagramSocket().use { socket ->
        val t0 = System.nanoTime()
        for (i in 0 until iters) {
            val frame = reliableWriteFrame(i and 0xFFFF, WRITER_ID, sample)
            socket.send(DatagramPacket(frame, frame.size, host, port))
        }
        return (System.nanoTime() - t0).toDouble() / iters
    }
}

/** Nanoseconds per enqueue of [iters] samples into a writer whose drain is not started. */
fun enqueueNs(host: InetAddress, port: Int, iters: Int): Double {
    val sample = ByteArray(4)
    AsyncReliableWriter(host, port, WRITER_ID).use { writer ->
        val t0 = System.nanoTime()
        for (i in 0 until iters) {
            check(writer.offer(sample.copyOf())) { "enqueue $i met a full queue" }
        }
        return (System.nanoTime() - t0).toDouble() / iters
    }
}

fun bench(host: InetAddress, port: Int) {
    // One round each for the JIT, then the measured round.
    inlineSendNs(host, port, BENCH_ITERS)
    enqueueNs(host, port, BENCH_ITERS)
    val inline = inlineSendNs(host, port, BENCH_ITERS)
    val enqueue = enqueueNs(host, port, BENCH_ITERS)
    println(String.format(java.util.Locale.ROOT, "BENCH enqueue_ns=%.1f inline_send_ns=%.1f", enqueue, inline))
}

fun main(args: Array<String>) {
    val port = args[0].toInt()
    val n = if (args.size > 1) args[1].toInt() else 12

    val host = InetAddress.getByName("127.0.0.1")
    if (args.size > 2 && args[2] == "bench") {
        bench(host, port)
        return
    }
    val writer = AsyncReliableWriter(host, port, WRITER_ID)
    writer.start()

    for (i in 0 until n) {
        // Sample i = its index as 4-byte little-endian (the peer/test decodes it).
        val sample = byteArrayOf(
            (i and 0xFF).toByte(), ((i shr 8) and 0xFF).toByte(),
            ((i shr 16) and 0xFF).toByte(), ((i shr 24) and 0xFF).toByte(),
        )
        writer.submit(sample)
    }

    val ok = writer.finish(25_000)
    writer.close()
    if (ok) {
        System.err.println("RELIABLE OK: all $n acknowledged (delivered=${writer.delivered()})")
        exitProcess(0)
    } else {
        val err = writer.drainError()
        System.err.println("RELIABLE INCOMPLETE" + (if (err != null) ": $err" else ""))
        exitProcess(1)
    }
}
