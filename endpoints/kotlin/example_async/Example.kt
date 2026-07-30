// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native Kotlin/JVM endpoint: the same telemetry
// publisher, but the subscriber consumes via the AsyncReader's blocking-queue
// channel and decodes every field. Compile with src/Zerodds.kt (see QUICKSTART).

import zerodds.AsyncReader
import zerodds.Client
import zerodds.Endian
import zerodds.MemTransport
import zerodds.Reader
import zerodds.Writer

data class Reading(val id: Long, val value: Float, val label: String) {
    fun marshal(endian: Endian): ByteArray {
        val w = Writer(endian)
        w.putU32(id)
        w.putF32(value)
        w.putString(label)
        return w.bytes()
    }
}

fun decodeReading(body: ByteArray): Reading {
    val r = Reader(body, Endian.LITTLE)
    return Reading(r.getU32(), r.getF32(), r.getString())
}

fun main() {
    val total = 5
    val t = MemTransport()
    val w = Client(t)

    // Publisher.
    for (i in 0 until total) {
        w.write(Reading(0x2000L + i, 100.0f - i, "sensor-%02d".format(i)).marshal(Endian.LITTLE))
    }

    // Subscriber: take from the AsyncReader channel; decode; stop at total.
    val reader = AsyncReader(t)
    var got = 0
    while (got < total) {
        val body = reader.samples.take()
        val r = decodeReading(body)
        println("async reading $got: id=0x${r.id.toString(16)} value=${"%.1f".format(r.value)} label=\"${r.label}\"")
        got++
    }
    reader.close()
    println("ALL OK")
}
