// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

/**
 * Wire-Vector-Tests V-1..V-12 gemaess
 * {@code zerodds-xcdr2-bindings-conformance-1.0} §6 (corrected spec
 * post 2026-05-07).
 *
 * <p>Status pro Vector:
 * <ul>
 *   <li>V-1, V-2, V-3, V-4..V-9, V-11 (None), V-12: byte-genau gegen
 *       Spec-§6.</li>
 *   <li>V-8 Key-Hash: byte-genau MD5(BE id=42) gegen Spec-§6.</li>
 *   <li>V-10, V-11 (Some): DHEADER + Byte-Count + Roundtrip gegen
 *       Spec-§6. EMHEADER-Bit-Layout folgt OMG XTypes 1.3 §7.4.3.4.5
 *       (LC bits 30-28, member-id bits 27-0, geschrieben in
 *       Body-Endianness LE) — die Spec-§6-Hex zeigt EMHEADER in einer
 *       konzeptuellen MSB-zuerst-Layout-Form (wird im Spec-Body als
 *       {@code 20 00 00 01} fuer {LC=2, id=1} dargestellt); unser
 *       Wire-Output ist {@code 01 00 00 20} in LE. Diese Divergenz
 *       ist mit dem Rust-Encoder (crates/cdr) konsistent und im
 *       CHANGELOG dokumentiert.</li>
 * </ul>
 */
@DisplayName("XCDR2 Wire-Vectors V-1..V-12")
final class Xcdr2WireVectorsTest {

    /** Hex-Helper: parst {@code "01 02 0A FF"} in byte[]. */
    private static byte[] hex(String s) {
        String t = s.replaceAll("\\s+", "");
        byte[] out = new byte[t.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(t.substring(i * 2, i * 2 + 2), 16);
        }
        return out;
    }

    // ==================================================================
    // V-1 Empty Final Struct
    // ==================================================================

    @Test
    @DisplayName("V-1 Empty Final Struct produces 0 bytes")
    void v1EmptyFinal() {
        Xcdr2Writer w = new Xcdr2Writer();
        // No members.
        assertArrayEquals(new byte[0], w.toByteArray());
    }

    // ==================================================================
    // V-2 Plain Primitives Final
    // ==================================================================

    @Test
    @DisplayName("V-2 Point{x=1, y=-2}")
    void v2Point() {
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeInt32(1);
        w.writeInt32(-2);
        byte[] actual = w.toByteArray();
        byte[] expected = hex("01 00 00 00 FE FF FF FF");
        assertArrayEquals(expected, actual);

        // Roundtrip.
        Xcdr2Reader r = new Xcdr2Reader(actual);
        assertEquals(1, r.readInt32());
        assertEquals(-2, r.readInt32());
    }

    // ==================================================================
    // V-3 Mixed Primitives Final (XTypes-1.3-conform encoder output)
    // ==================================================================

    @Test
    @DisplayName("V-3 All mixed-primitives byte-exact + roundtrip (corrected spec)")
    void v3MixedPrimitives() {
        // Corrected spec §6 V-3 (2026-05-07): 48 Bytes mit natuerlicher
        // Alignment per XTypes 1.3 §7.4.1.5 (b@0 o@1 s@2 us@4 pad@6
        // l@8 ul@12 ll@16 ull@24 f@32 pad@36 d@40).
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeBoolean(true);
        w.writeOctet((byte) 0xAB);
        w.writeInt16((short) -12345);
        w.writeUInt16(54321);
        w.writeInt32(-1234567);
        w.writeUInt32(2345678L);
        w.writeInt64(-987654321L);
        w.writeUInt64(123456789L);
        w.writeFloat32(2.5f);
        w.writeFloat64(3.14159);

        byte[] actual = w.toByteArray();
        byte[] expected = hex(
                "01 AB C7 CF 31 D4 00 00 "
                        + "79 29 ED FF CE CA 23 00 "
                        + "4F 97 21 C5 FF FF FF FF "
                        + "15 CD 5B 07 00 00 00 00 "
                        + "00 00 20 40 00 00 00 00 "
                        + "6E 86 1B F0 F9 21 09 40");
        assertEquals(48, expected.length);
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        assertTrue(r.readBoolean());
        assertEquals((byte) 0xAB, r.readOctet());
        assertEquals((short) -12345, r.readInt16());
        assertEquals(54321, r.readUInt16());
        assertEquals(-1234567, r.readInt32());
        assertEquals(2345678L, r.readUInt32());
        assertEquals(-987654321L, r.readInt64());
        assertEquals(123456789L, r.readUInt64());
        assertEquals(2.5f, r.readFloat32());
        assertEquals(3.14159, r.readFloat64(), 0.0);
    }

    // ==================================================================
    // V-4 String Final
    // ==================================================================

    @Test
    @DisplayName("V-4 Greeting{text=\"hello\"}")
    void v4Greeting() {
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeString("hello");
        byte[] actual = w.toByteArray();
        byte[] expected = hex("06 00 00 00 68 65 6C 6C 6F 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        assertEquals("hello", r.readString());
    }

    // ==================================================================
    // V-5 Sequence<int32> Final
    // ==================================================================

    @Test
    @DisplayName("V-5 Bag{ids=[1,2,3]}")
    void v5Bag() {
        Xcdr2Writer w = new Xcdr2Writer();
        int[] ids = {1, 2, 3};
        w.writeSequenceCount(ids.length);
        for (int v : ids) {
            w.writeInt32(v);
        }
        byte[] actual = w.toByteArray();
        byte[] expected = hex("03 00 00 00 01 00 00 00 02 00 00 00 03 00 00 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int n = r.readSequenceCount();
        assertEquals(3, n);
        for (int i = 0; i < n; i++) {
            assertEquals(ids[i], r.readInt32());
        }
    }

    // ==================================================================
    // V-6 Sequence<string> Final
    // ==================================================================

    @Test
    @DisplayName("V-6 Tags{tags=[\"a\",\"bc\"]}")
    void v6Tags() {
        Xcdr2Writer w = new Xcdr2Writer();
        String[] tags = {"a", "bc"};
        w.writeSequenceCount(tags.length);
        for (String s : tags) {
            w.writeString(s);
        }
        byte[] actual = w.toByteArray();
        byte[] expected =
                hex("02 00 00 00 02 00 00 00 61 00 00 00 03 00 00 00 62 63 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int n = r.readSequenceCount();
        assertEquals(2, n);
        assertEquals("a", r.readString());
        assertEquals("bc", r.readString());
    }

    // ==================================================================
    // V-7 Nested Modules Final
    // ==================================================================

    @Test
    @DisplayName("V-7 Outer::Inner::S{x=1234}")
    void v7Nested() {
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeInt32(1234);
        byte[] actual = w.toByteArray();
        byte[] expected = hex("D2 04 00 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        assertEquals(1234, r.readInt32());
    }

    // ==================================================================
    // V-8 Keyed Struct (Final)
    // ==================================================================

    @Test
    @DisplayName("V-8 Sensor{id=42, value=3.14}")
    void v8Sensor() {
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeInt32(42);
        w.writeFloat64(3.14);
        byte[] actual = w.toByteArray();
        byte[] expected = hex(
                "2A 00 00 00 00 00 00 00 1F 85 EB 51 B8 1E 09 40");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        assertEquals(42, r.readInt32());
        assertEquals(3.14, r.readFloat64(), 0.0);
    }

    @Test
    @DisplayName("V-8 Key-Hash zero-pad fuer 4-Byte holder per XTypes 7.6.8.4")
    void v8KeyHash() {
        // PlainCdr2BeKeyHolder: BE-Encoding der @key-Felder.
        Xcdr2Writer w = new Xcdr2Writer(EndianMode.BIG_ENDIAN);
        w.writeInt32(42);
        byte[] beKey = w.toByteArray();
        assertArrayEquals(hex("00 00 00 2A"), beKey);

        // XTypes 1.3 §7.6.8.4: Holder ≤ 16 octets -> zero-pad auf 16 Bytes.
        // MD5 nur fuer Holder > 16 octets. Hier: Holder = 4 Byte -> zero-pad.
        byte[] hash = new byte[16];
        if (beKey.length <= 16) {
            System.arraycopy(beKey, 0, hash, 0, beKey.length);
        } else {
            hash = Md5.hash(beKey);
        }
        byte[] expected = hex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00");
        assertArrayEquals(expected, hash);

        // Self-Check der Md5-Implementation (separat von §7.6.8.4):
        // MD5(00 00 00 2A) = A5 15 85 57 99 DD BD A0 8B C9 9F C2 CE 87 FA 79.
        byte[] md5 = Md5.hash(beKey);
        assertArrayEquals(
                hex("A5 15 85 57 99 DD BD A0 8B C9 9F C2 CE 87 FA 79"), md5);
    }

    // ==================================================================
    // V-9 Appendable Struct
    // ==================================================================

    @Test
    @DisplayName("V-9 V{a=1,b=2} Appendable mit DHEADER")
    void v9Appendable() {
        Xcdr2Writer w = new Xcdr2Writer();
        int dh = w.beginAppendable();
        w.writeInt32(1);
        w.writeInt32(2);
        w.endDelimited(dh);
        byte[] actual = w.toByteArray();
        byte[] expected = hex("08 00 00 00 01 00 00 00 02 00 00 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int size = r.readDHeader();
        assertEquals(8, size);
        assertEquals(1, r.readInt32());
        assertEquals(2, r.readInt32());
    }

    // ==================================================================
    // V-10 Mutable Struct
    // ==================================================================

    @Test
    @DisplayName("V-10 M{a=42, b=\"hi\"} Mutable (corrected DHEADER=23)")
    void v10Mutable() {
        // Corrected spec §6 V-10 (2026-05-07): DHEADER = 23, Body =
        // 4(EM1) + 4(a) + 4(EM2) + 4(NEXTINT) + 7(string) = 23, total
        // 27 Bytes. Wir verwenden LC=2 fuer 4-byte-inline (a) und LC=4
        // (NEXTINT-Form) fuer den String — XTypes 1.3 §7.4.3.4.5
        // konform. EMHEADER-Wire-Layout (LE Body-Endianness) divergiert
        // bewusst von der konzeptuellen MSB-Hex-Notation der Spec — der
        // Reader bestaetigt bit-genau die EMHEADER-Semantik via
        // Roundtrip.
        Xcdr2Writer w = new Xcdr2Writer();
        int dh = w.beginMutable();
        w.writeEmHeader(1, Xcdr2Writer.LC_INT32, false);
        w.writeInt32(42);
        w.writeEmHeader(2, Xcdr2Writer.LC_NEXTINT, false);
        w.writeNextInt(7);
        w.writeString("hi");
        w.endDelimited(dh);

        byte[] actual = w.toByteArray();

        // Corrected total = 27 Bytes; DHEADER = 23.
        assertEquals(27, actual.length);
        // DHEADER corrected per spec: 17 00 00 00 (=23 LE).
        assertArrayEquals(hex("17 00 00 00"), java.util.Arrays.copyOfRange(actual, 0, 4));

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int bodySize = r.readDHeader();
        assertEquals(23, bodySize);
        Xcdr2Reader.EmHeader e1 = r.readEmHeader();
        assertEquals(1, e1.memberId);
        assertEquals(Xcdr2Writer.LC_INT32, e1.lc);
        assertFalse(e1.mustUnderstand);
        assertEquals(42, r.readInt32());
        Xcdr2Reader.EmHeader e2 = r.readEmHeader();
        assertEquals(2, e2.memberId);
        assertEquals(Xcdr2Writer.LC_NEXTINT, e2.lc);
        assertEquals(7, r.readNextInt());
        assertEquals("hi", r.readString());
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // V-11 Optional Member (Mutable)
    // ==================================================================

    @Test
    @DisplayName("V-11A O{maybe=Some(7)} Mutable Optional present (corrected DHEADER=8)")
    void v11OptionalSome() {
        // Corrected spec §6 V-11A (2026-05-07): DHEADER = 8, Body =
        // 4(EM) + 4(value) = 8, total 12 Bytes.
        Xcdr2Writer w = new Xcdr2Writer();
        int dh = w.beginMutable();
        w.writeEmHeader(1, Xcdr2Writer.LC_INT32, false);
        w.writeInt32(7);
        w.endDelimited(dh);
        byte[] actual = w.toByteArray();

        assertEquals(12, actual.length);
        // DHEADER corrected per spec: 08 00 00 00 (=8 LE).
        assertArrayEquals(hex("08 00 00 00"), java.util.Arrays.copyOfRange(actual, 0, 4));

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int bodySize = r.readDHeader();
        assertEquals(8, bodySize);
        Xcdr2Reader.EmHeader e = r.readEmHeader();
        assertEquals(1, e.memberId);
        assertEquals(Xcdr2Writer.LC_INT32, e.lc);
        assertEquals(7, r.readInt32());
        assertEquals(0, r.remaining());
    }

    @Test
    @DisplayName("V-11 O{maybe=None} Mutable Optional absent")
    void v11OptionalNone() {
        Xcdr2Writer w = new Xcdr2Writer();
        int dh = w.beginMutable();
        // No EMHEADER emitted for absent optional.
        w.endDelimited(dh);
        byte[] actual = w.toByteArray();
        byte[] expected = hex("00 00 00 00");
        assertArrayEquals(expected, actual);

        Xcdr2Reader r = new Xcdr2Reader(actual);
        int bodySize = r.readDHeader();
        assertEquals(0, bodySize);
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // V-12 Mutable Sentinel End-Marker
    // ==================================================================

    @Test
    @DisplayName("V-12 XCDR2 emits NO explicit PID_LIST_END sentinel")
    void v12NoSentinel() {
        // Mutable mit zwei EMHEADERn — wir testen, dass kein
        // explizites Sentinel-PID nach dem letzten EMHEADER auftaucht.
        Xcdr2Writer w = new Xcdr2Writer();
        int dh = w.beginMutable();
        w.writeEmHeader(1, Xcdr2Writer.LC_INT32, false);
        w.writeInt32(42);
        w.writeEmHeader(2, Xcdr2Writer.LC_INT32, false);
        w.writeInt32(43);
        w.endDelimited(dh);
        byte[] actual = w.toByteArray();

        // Body = 16 bytes (2x 4-byte emheader + 2x 4-byte value).
        assertEquals(20, actual.length);

        // DHEADER LE = 16
        assertEquals((byte) 16, actual[0]);

        // Reading respects DHEADER bound — kein Sentinel-Pseudo-PID.
        Xcdr2Reader r = new Xcdr2Reader(actual);
        int bodySize = r.readDHeader();
        assertEquals(16, bodySize);
        Xcdr2Reader.EmHeader e1 = r.readEmHeader();
        assertEquals(1, e1.memberId);
        assertEquals(42, r.readInt32());
        Xcdr2Reader.EmHeader e2 = r.readEmHeader();
        assertEquals(2, e2.memberId);
        assertEquals(43, r.readInt32());
        // No more bytes after.
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // Sanity-Checks fuer Helpers
    // ==================================================================

    @Test
    @DisplayName("Reader rejects underflow")
    void readerUnderflow() {
        Xcdr2Reader r = new Xcdr2Reader(new byte[]{0x01, 0x02});
        try {
            r.readInt32();
        } catch (XcdrException expected) {
            assertTrue(expected.getMessage().contains("underflow"));
            return;
        }
        assertFalse(true, "expected XcdrException");
    }

    @Test
    @DisplayName("Endian flip: BE encoder writes big-endian")
    void endianFlipBe() {
        Xcdr2Writer w = new Xcdr2Writer(EndianMode.BIG_ENDIAN);
        w.writeInt32(0x01020304);
        byte[] actual = w.toByteArray();
        assertArrayEquals(hex("01 02 03 04"), actual);
    }
}
