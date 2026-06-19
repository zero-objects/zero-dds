// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.bridge;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.ArrayList;
import java.util.List;

/**
 * Pure-Java multi-process bridge client for {@code zerodds-grpc-bridged}
 * (DDS-Java-PSM 1.0 §8.7, listener-callbacks spec §7.3 "gRPC-Bridge-Pfad").
 *
 * <p>A Java process drives a DDS runtime that lives in a separate process via
 * gRPC: {@code Publish(Sample{payload})} writes opaque bytes to a DDS
 * DataWriter, {@code Subscribe} drains a DataReader. The bytes are the DDS
 * user data 1:1 — the Java side stays the spec's
 * {@code org.omg.dds.*} type model and serializes with the project's
 * XCDR2 codec before publishing.
 *
 * <p>The daemon speaks <em>prior-knowledge</em> cleartext HTTP/2 (h2c), so
 * this client is a minimal raw HTTP/2 framer over a plain {@link Socket} —
 * no grpc-java, no HTTP/2 upgrade, no TLS — matching "pure-Java per Vendor-
 * Extension". Request headers use HPACK literal-without-indexing fields;
 * response HEADERS frames are skipped (only DATA frames carry the protobuf
 * {@code Sample}/{@code PublishAck} messages).
 *
 * <p>Java-8 source compatible (the release build targets bytecode 8).
 */
public final class GrpcBridgeClient {

    private static final byte[] PREFACE =
            "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".getBytes(java.nio.charset.StandardCharsets.US_ASCII);

    // HTTP/2 frame types.
    private static final int FT_DATA = 0x0;
    private static final int FT_HEADERS = 0x1;
    private static final int FT_SETTINGS = 0x4;
    // HTTP/2 flags.
    private static final int FL_END_STREAM = 0x1;
    private static final int FL_END_HEADERS = 0x4;

    private final String host;
    private final int port;
    private final int timeoutMs;

    public GrpcBridgeClient(String host, int port) {
        this(host, port, 5000);
    }

    public GrpcBridgeClient(String host, int port, int timeoutMs) {
        this.host = host;
        this.port = port;
        this.timeoutMs = timeoutMs;
    }

    /**
     * gRPC {@code Publish(Sample{payload})} on the topic service. Returns the
     * {@code PublishAck.accepted} count (1 when the DDS write succeeded).
     *
     * @param servicePath fully-qualified service, e.g.
     *     {@code /zerodds.bridge.v1.DemoStream}
     * @param payload opaque DDS user-data bytes
     */
    public long publish(String servicePath, byte[] payload) throws IOException {
        try (Socket sock = openAndHandshake()) {
            OutputStream out = sock.getOutputStream();
            InputStream in = sock.getInputStream();

            // HEADERS (no END_STREAM — DATA carries the Sample).
            byte[] hdrs = encodeHeaders(servicePath + "/Publish");
            writeFrame(out, FT_HEADERS, FL_END_HEADERS, 1, hdrs);
            // DATA: LPM(Sample) + END_STREAM.
            byte[] lpm = lpm(protoSample(payload));
            writeFrame(out, FT_DATA, FL_END_STREAM, 1, lpm);
            out.flush();

            byte[] body = readResponseData(in);
            return body == null ? 0L : decodePublishAck(body);
        }
    }

    /**
     * gRPC {@code Subscribe} on the topic service. Returns the next
     * {@code Sample.payload} the bridge has, or {@code null} when the DDS
     * reader currently has no data (pull-based stream cardinality, §4.2).
     */
    public byte[] subscribe(String servicePath) throws IOException {
        try (Socket sock = openAndHandshake()) {
            OutputStream out = sock.getOutputStream();
            InputStream in = sock.getInputStream();

            byte[] hdrs = encodeHeaders(servicePath + "/Subscribe");
            writeFrame(out, FT_HEADERS, FL_END_HEADERS | FL_END_STREAM, 1, hdrs);
            out.flush();

            byte[] body = readResponseData(in);
            return body == null ? null : decodeSamplePayload(body);
        }
    }

    /**
     * Convenience: polls {@link #subscribe} until a sample arrives or the
     * deadline passes. Returns {@code null} on timeout.
     */
    public byte[] subscribeBlocking(String servicePath, long deadlineMillis) throws IOException {
        while (System.currentTimeMillis() < deadlineMillis) {
            byte[] s = subscribe(servicePath);
            if (s != null) {
                return s;
            }
            try {
                Thread.sleep(100);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return null;
            }
        }
        return null;
    }

    // ------------------------------------------------------------------
    // HTTP/2 + gRPC plumbing
    // ------------------------------------------------------------------

    private Socket openAndHandshake() throws IOException {
        Socket sock = new Socket();
        sock.connect(new InetSocketAddress(host, port), timeoutMs);
        sock.setSoTimeout(timeoutMs);
        sock.setTcpNoDelay(true);
        OutputStream out = sock.getOutputStream();
        out.write(PREFACE);
        writeFrame(out, FT_SETTINGS, 0, 0, new byte[0]);
        out.flush();
        // Drain the server's SETTINGS frame.
        readOneFrame(sock.getInputStream());
        return sock;
    }

    private static void writeFrame(OutputStream out, int type, int flags, int streamId, byte[] payload)
            throws IOException {
        int len = payload.length;
        byte[] hdr = new byte[9];
        hdr[0] = (byte) ((len >>> 16) & 0xFF);
        hdr[1] = (byte) ((len >>> 8) & 0xFF);
        hdr[2] = (byte) (len & 0xFF);
        hdr[3] = (byte) type;
        hdr[4] = (byte) flags;
        hdr[5] = (byte) ((streamId >>> 24) & 0xFF);
        hdr[6] = (byte) ((streamId >>> 16) & 0xFF);
        hdr[7] = (byte) ((streamId >>> 8) & 0xFF);
        hdr[8] = (byte) (streamId & 0xFF);
        out.write(hdr);
        if (len > 0) {
            out.write(payload);
        }
    }

    /** One parsed HTTP/2 frame. */
    private static final class Frame {
        final int type;
        final int flags;
        final byte[] payload;

        Frame(int type, int flags, byte[] payload) {
            this.type = type;
            this.flags = flags;
            this.payload = payload;
        }
    }

    private static Frame readOneFrame(InputStream in) throws IOException {
        byte[] hdr = readExact(in, 9);
        if (hdr == null) {
            return null;
        }
        int len = ((hdr[0] & 0xFF) << 16) | ((hdr[1] & 0xFF) << 8) | (hdr[2] & 0xFF);
        int type = hdr[3] & 0xFF;
        int flags = hdr[4] & 0xFF;
        byte[] payload = len > 0 ? readExact(in, len) : new byte[0];
        if (len > 0 && payload == null) {
            return null;
        }
        return new Frame(type, flags, payload);
    }

    /**
     * Reads response frames, returning the first DATA frame's LPM-decoded
     * protobuf message (the {@code PublishAck}/{@code Sample}), or
     * {@code null} when the response stream ends with no DATA.
     */
    private static byte[] readResponseData(InputStream in) throws IOException {
        byte[] data = null;
        for (int i = 0; i < 16; i++) {
            Frame f;
            try {
                f = readOneFrame(in);
            } catch (IOException e) {
                break;
            }
            if (f == null) {
                break;
            }
            if (f.type == FT_DATA && f.payload.length > 0 && data == null) {
                data = stripLpm(f.payload);
            }
            // HEADERS/SETTINGS frames are skipped; END_STREAM on any frame ends it.
            if ((f.flags & FL_END_STREAM) != 0 && f.type == FT_HEADERS) {
                break;
            }
        }
        return data;
    }

    private static byte[] readExact(InputStream in, int n) throws IOException {
        byte[] buf = new byte[n];
        int off = 0;
        while (off < n) {
            int r = in.read(buf, off, n - off);
            if (r < 0) {
                return null;
            }
            off += r;
        }
        return buf;
    }

    /** gRPC length-prefixed message: 1-byte compressed flag + 4-byte BE length + bytes. */
    private static byte[] lpm(byte[] msg) {
        byte[] out = new byte[5 + msg.length];
        out[0] = 0; // uncompressed
        out[1] = (byte) ((msg.length >>> 24) & 0xFF);
        out[2] = (byte) ((msg.length >>> 16) & 0xFF);
        out[3] = (byte) ((msg.length >>> 8) & 0xFF);
        out[4] = (byte) (msg.length & 0xFF);
        System.arraycopy(msg, 0, out, 5, msg.length);
        return out;
    }

    private static byte[] stripLpm(byte[] frame) {
        if (frame.length < 5) {
            return new byte[0];
        }
        int len = ((frame[1] & 0xFF) << 24)
                | ((frame[2] & 0xFF) << 16)
                | ((frame[3] & 0xFF) << 8)
                | (frame[4] & 0xFF);
        int end = Math.min(5 + len, frame.length);
        byte[] out = new byte[end - 5];
        System.arraycopy(frame, 5, out, 0, out.length);
        return out;
    }

    // ------------------------------------------------------------------
    // HPACK (literal header field without indexing, no Huffman)
    // ------------------------------------------------------------------

    private static byte[] encodeHeaders(String path) {
        List<byte[]> parts = new ArrayList<byte[]>();
        parts.add(literal(":method", "POST"));
        parts.add(literal(":scheme", "http"));
        parts.add(literal(":path", path));
        parts.add(literal("content-type", "application/grpc+proto"));
        int total = 0;
        for (byte[] p : parts) {
            total += p.length;
        }
        byte[] out = new byte[total];
        int off = 0;
        for (byte[] p : parts) {
            System.arraycopy(p, 0, out, off, p.length);
            off += p.length;
        }
        return out;
    }

    /** HPACK 6.2.2 literal header field without indexing, new name. */
    private static byte[] literal(String name, String value) {
        byte[] n = name.getBytes(java.nio.charset.StandardCharsets.US_ASCII);
        byte[] v = value.getBytes(java.nio.charset.StandardCharsets.US_ASCII);
        // 0x00 prefix (no-index, index 0 ⇒ literal name) + len(name) + name + len(value) + value.
        // Header names/values here are < 128 bytes ⇒ single-byte length, H=0.
        byte[] out = new byte[1 + 1 + n.length + 1 + v.length];
        int o = 0;
        out[o++] = 0x00;
        out[o++] = (byte) (n.length & 0x7F);
        System.arraycopy(n, 0, out, o, n.length);
        o += n.length;
        out[o++] = (byte) (v.length & 0x7F);
        System.arraycopy(v, 0, out, o, v.length);
        return out;
    }

    // ------------------------------------------------------------------
    // protobuf (hand-rolled — Sample{bytes payload=1}, PublishAck{uint64 accepted=1})
    // ------------------------------------------------------------------

    static byte[] protoSample(byte[] payload) {
        byte[] len = varint(payload.length);
        byte[] out = new byte[1 + len.length + payload.length];
        out[0] = 0x0A; // field 1, wire-type 2
        System.arraycopy(len, 0, out, 1, len.length);
        System.arraycopy(payload, 0, out, 1 + len.length, payload.length);
        return out;
    }

    static byte[] decodeSamplePayload(byte[] msg) {
        if (msg.length == 0 || (msg[0] & 0xFF) != 0x0A) {
            return null;
        }
        long[] cur = new long[] {1};
        long len = readVarint(msg, cur);
        int start = (int) cur[0];
        int end = (int) Math.min(start + len, msg.length);
        byte[] out = new byte[end - start];
        System.arraycopy(msg, start, out, 0, out.length);
        return out;
    }

    static long decodePublishAck(byte[] msg) {
        if (msg.length == 0 || (msg[0] & 0xFF) != 0x08) {
            return 0L;
        }
        long[] cur = new long[] {1};
        return readVarint(msg, cur);
    }

    private static byte[] varint(long v) {
        byte[] tmp = new byte[10];
        int i = 0;
        while (true) {
            int b = (int) (v & 0x7F);
            v >>>= 7;
            if (v != 0) {
                b |= 0x80;
            }
            tmp[i++] = (byte) b;
            if (v == 0) {
                break;
            }
        }
        byte[] out = new byte[i];
        System.arraycopy(tmp, 0, out, 0, i);
        return out;
    }

    private static long readVarint(byte[] b, long[] cursor) {
        long val = 0;
        int shift = 0;
        int i = (int) cursor[0];
        while (i < b.length && shift < 64) {
            int x = b[i++] & 0xFF;
            val |= ((long) (x & 0x7F)) << shift;
            if ((x & 0x80) == 0) {
                break;
            }
            shift += 7;
        }
        cursor[0] = i;
        return val;
    }
}
