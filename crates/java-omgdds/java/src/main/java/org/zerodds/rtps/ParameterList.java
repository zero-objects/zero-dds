// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * RTPS ParameterList (§9.4.2.11) — the on-wire form of the SPDP/SEDP builtin
 * topic data and DATA inline-QoS. Ported byte-for-byte from
 * {@code crates/rtps/src/parameter_list.rs}.
 *
 * <p>Wire layout of each parameter: {@code id:u16, length:u16, value:length}.
 * The length field is the value length <em>padded up to a 4-byte multiple</em>
 * (it includes the padding). The list ends with PID_SENTINEL (0x0001, len 0).
 * All discovery payloads are little-endian; the Rust decoder rejects a
 * non-4-aligned length, so every value here is padded to 4.
 */
public final class ParameterList {

    public static final class Parameter {
        public final int id;
        public final byte[] value;

        public Parameter(int id, byte[] value) {
            this.id = id;
            this.value = value;
        }
    }

    private final List<Parameter> params = new ArrayList<>();

    public ParameterList add(int id, byte[] value) {
        params.add(new Parameter(id, value));
        return this;
    }

    public List<Parameter> params() {
        return params;
    }

    /** First parameter with {@code id} (must-understand/vendor bits masked). */
    public byte[] find(int id) {
        for (Parameter p : params) {
            if ((p.id & 0x3FFF) == (id & 0x3FFF)) {
                return p.value;
            }
        }
        return null;
    }

    /** Little-endian encode incl. trailing sentinel. */
    public byte[] toBytesLe() {
        Wire w = new Wire();
        for (Parameter p : params) {
            int raw = p.value.length;
            int padded = (raw + 3) & ~3;
            w.u16le(p.id);
            w.u16le(padded);
            w.bytes(p.value);
            for (int i = raw; i < padded; i++) {
                w.u8(0);
            }
        }
        w.u16le(Rtps.PID_SENTINEL);
        w.u16le(0);
        return w.toBytes();
    }

    /** Little-endian decode until the sentinel; ignores trailing bytes. */
    public static ParameterList fromBytesLe(byte[] b, int off, int end) {
        ParameterList pl = new ParameterList();
        int pos = off;
        while (pos + 4 <= end) {
            int id = Wire.u16le(b, pos);
            int len = Wire.u16le(b, pos + 2);
            pos += 4;
            if (id == Rtps.PID_SENTINEL) {
                break;
            }
            if (pos + len > end) {
                break;
            }
            pl.params.add(new Parameter(id, Wire.slice(b, pos, len)));
            pos += len;
        }
        return pl;
    }

    // ---- value encoders (LE) — parameter_list.rs / publication_data.rs ----

    /** CDR string: u32 len(=bytes+1) + UTF-8 + null; value itself padded to 4 by encoder. */
    public static byte[] cdrString(String s) {
        byte[] utf = s.getBytes(StandardCharsets.UTF_8);
        Wire w = new Wire(utf.length + 8);
        w.u32le(utf.length + 1L);
        w.bytes(utf);
        w.u8(0);
        return w.toBytes();
    }

    /** Decode a CDR string value (len u32 incl null + bytes + null). */
    public static String readCdrString(byte[] value) {
        if (value.length < 4) {
            return "";
        }
        int len = (int) Wire.u32le(value, 0);
        if (len <= 0 || 4 + len > value.length) {
            return "";
        }
        // strip trailing null terminator
        int n = len - 1;
        return new String(value, 4, n, StandardCharsets.UTF_8);
    }

    /** Duration value (8 bytes: i32 seconds + u32 fraction/nanos, LE). */
    public static byte[] duration(int seconds, long fraction) {
        Wire w = new Wire(8);
        w.u32le(seconds & 0xFFFFFFFFL);
        w.u32le(fraction);
        return w.toBytes();
    }

    /** u32 value LE. */
    public static byte[] u32(long v) {
        Wire w = new Wire(4);
        w.u32le(v);
        return w.toBytes();
    }

    /** 2-byte value + 2 padding (PROTOCOL_VERSION / VENDOR_ID). */
    public static byte[] twoBytesPadded(byte a, byte b) {
        return new byte[] {a, b, 0, 0};
    }

    /** RELIABILITY / LIVELINESS value: kind u32 + 8-byte duration. */
    public static byte[] kindDuration(int kind, int seconds, long fraction) {
        Wire w = new Wire(12);
        w.u32le(kind & 0xFFFFFFFFL);
        w.u32le(seconds & 0xFFFFFFFFL);
        w.u32le(fraction);
        return w.toBytes();
    }

    /** DATA_REPRESENTATION value: u32 count + count×int16 LE. */
    public static byte[] dataRepresentation(int... ids) {
        Wire w = new Wire(4 + ids.length * 2);
        w.u32le(ids.length);
        for (int id : ids) {
            w.u16le(id & 0xFFFF);
        }
        return w.toBytes();
    }

    /** sequence<string> value: u32 count + each CDR string (each 4-aligned). */
    public static byte[] stringSeq(List<String> items) {
        Wire w = new Wire();
        w.u32le(items.size());
        for (String s : items) {
            byte[] cs = cdrString(s);
            w.bytes(cs);
            w.padTo4();
        }
        return w.toBytes();
    }
}
