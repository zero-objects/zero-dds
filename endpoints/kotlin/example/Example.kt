// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native Kotlin endpoint SDK: sync (pull) and async
// (background thread + channel) over an in-memory transport.

import zerodds.AsyncReader
import zerodds.Client
import zerodds.Endian
import zerodds.MemTransport
import zerodds.Reader
import zerodds.Writer
import java.util.concurrent.TimeUnit

private fun sample(id: Long, label: String): ByteArray {
    val w = Writer(Endian.LITTLE)
    w.putU32(id)
    w.putString(label)
    return w.bytes()
}

fun main() {
    // --- sync ---
    val t = MemTransport()
    val c = Client(t)
    c.write(sample(0x42, "sync-hello"))
    c.poll()?.let { println("sync: received id=0x${Reader(it, Endian.LITTLE).getU32().toString(16)}") }

    // --- async (background thread + channel) ---
    val t2 = MemTransport()
    val w = Client(t2)
    for (i in 0 until 3) w.write(sample(0x100L + i, "async"))
    val r = AsyncReader(t2)
    for (i in 0 until 3) {
        val body = r.samples.poll(2, TimeUnit.SECONDS) ?: break
        println("async: received id=0x${Reader(body, Endian.LITTLE).getU32().toString(16)}")
    }
    r.close()

    println("ALL OK")
}
