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

import zerodds.AsyncReliableWriter
import java.net.InetAddress
import kotlin.system.exitProcess

fun main(args: Array<String>) {
    val port = args[0].toInt()
    val n = if (args.size > 1) args[1].toInt() else 12

    val host = InetAddress.getByName("127.0.0.1")
    val writer = AsyncReliableWriter(host, port)
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
