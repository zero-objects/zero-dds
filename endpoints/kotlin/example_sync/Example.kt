// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native Kotlin/JVM endpoint: a sensor-telemetry
// publisher writes typed Reading samples; a subscriber polls and decodes every
// field. Compile with src/Zerodds.kt (see QUICKSTART).

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
    val c = Client(t)

    // Publisher: frame + deliver 5 typed readings with varying values.
    for (i in 0 until total) {
        c.write(Reading(0x1000L + i, 20.0f + i * 0.5f, "bay-%02d".format(i)).marshal(Endian.LITTLE))
    }

    // Subscriber: poll; decode every field; stop at total.
    var got = 0
    while (got < total) {
        val body = c.poll() ?: break
        val r = decodeReading(body)
        println("sync reading $got: id=0x${r.id.toString(16)} value=${"%.1f".format(r.value)} label=\"${r.label}\"")
        got++
    }
    if (got != total) { println("incomplete"); return }
    println("ALL OK")
}
