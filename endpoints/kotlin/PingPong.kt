// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Live ping-pong driver for the native Kotlin endpoint SDK: argv = <mode> <port>
// (mode = "sync" | "async"). Marshals a typed Ping (@final { long seq; string
// msg; }) with the Kotlin Writer, frames it as XRCE WRITE_DATA, and sends it to
// the Rust peer over a real UDP DatagramSocket (UdpTransport); receives the DATA
// reply, decodes the Pong, and prints the line the harness asserts:
//   PONG seq=<n> reply=pong:<msg>
//
// sync  = the caller owns the run-loop, Client.poll() drains the transport.
// async = AsyncReader runs a daemon thread that pushes decoded bodies onto a
//         LinkedBlockingQueue the consumer blocks on.

import zerodds.AsyncReader
import zerodds.Client
import zerodds.Endian
import zerodds.Reader
import zerodds.UdpTransport
import zerodds.Writer
import java.net.InetAddress
import java.util.concurrent.TimeUnit

fun main(args: Array<String>) {
    val mode = args[0]
    val port = args[1].toInt()
    val host = InetAddress.getByName("127.0.0.1")
    val transport = UdpTransport(host, port)
    val client = Client(transport)

    // Marshal the Ping (XCDR2 little-endian, @final => no DHEADER): u32 seq=1,
    // then the UTF-8 byte-count string "hello from app".
    val w = Writer(Endian.LITTLE)
    w.putU32(1)
    w.putString("hello from app")
    client.write(w.bytes())

    val deadline = System.currentTimeMillis() + 30_000
    val body: ByteArray = if (mode == "async") {
        val reader = AsyncReader(transport)
        val b = reader.samples.poll(30, TimeUnit.SECONDS) ?: error("async: no pong")
        reader.close()
        b
    } else {
        var b: ByteArray? = null
        while (b == null && System.currentTimeMillis() < deadline) {
            b = client.poll()
        }
        b ?: error("sync: no pong")
    }

    val r = Reader(body, Endian.LITTLE)
    val seq = r.getU32()
    val reply = r.getString()
    println("PONG seq=$seq reply=$reply")
    transport.close()
}
