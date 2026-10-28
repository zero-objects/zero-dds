// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.FixedDigitCount;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

/**
 * Wire-vector tests V-1..V-12 per
 * {@code zerodds-xcdr2-bindings-conformance-1.0} §6 (corrected spec
 * post 2026-05-07).
 *
 * <p>These call {@code org.zerodds.wire.Writer}/{@code Reader} directly —
 * the one XCDR2 codec this binding has, the same one
 * {@code zerodds-idlc --lang java}'s generated {@code ZdTypeSupports}
 * companion drives (see {@code tools/idlc/src/java_support.rs}). There is
 * no second, hand-maintained low-level codec left to verify against this
 * file separately.
 *
 * <p>Status pro Vector:
 * <ul>
 *   <li>V-1, V-2, V-3, V-4..V-9, V-11 (None), V-12: byte-exact against
 *       spec §6.</li>
 *   <li>V-8 key hash: byte-exact MD5(BE id=42) against spec §6.</li>
 *   <li>V-10, V-11 (Some): DHEADER + byte count + roundtrip against
 *       spec §6. The EMHEADER bit layout follows OMG XTypes 1.3 §7.4.3.4.5
 *       (LC bits 30-28, member-id bits 27-0, written in
 *       body endianness LE) — the spec §6 hex shows EMHEADER in a
 *       conceptual MSB-first layout form (represented in the spec body as
 *       {@code 20 00 00 01} for {LC=2, id=1}); our
 *       Wire output is {@code 01 00 00 20} in LE. This divergence
 *       is consistent with the Rust encoder and is
 *       documented in the CHANGELOG.</li>
 *   <li>V-10's {@code string} member: length code 5 (XTypes 1.3
 *       §7.4.3.4.2 — the NEXTINT of codes 5-7 is "reused also as part of
 *       the serialized member", so a self-delimiting member's own leading
 *       length word stands in for it and none is written separately).
 *       The removed hand-written {@code Xcdr2Writer} used length code 4
 *       with a separately-written NEXTINT ahead of the string's own
 *       length prefix — DHEADER 23 / total 27 bytes. The spec permits
 *       both; the compact, generator-matching form
 *       ({@code docs/specs/zerodds-ast-1.0.md} §7.1) is DHEADER 19 /
 *       total 23. Confirmed against {@code zerodds-idlc --lang java}'s
 *       own generated output for this exact case.</li>
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

    private static Writer newWriter() {
        return new Writer(Endianness.LITTLE, 4, PlCdr1Framing.defaults(), false);
    }

    private static Writer newWriter(Endianness endianness) {
        return new Writer(endianness, 4, PlCdr1Framing.defaults(), false);
    }

    private static Reader newReader(byte[] bytes) {
        return new Reader(bytes, Endianness.LITTLE, 4, PlCdr1Framing.defaults());
    }

    // ==================================================================
    // V-1 Empty Final Struct
    // ==================================================================

    @Test
    @DisplayName("V-1 Empty Final Struct produces 0 bytes")
    void v1EmptyFinal() {
        Writer w = newWriter();
        // No members.
        assertArrayEquals(new byte[0], w.finish());
    }

    // ==================================================================
    // V-2 Plain Primitives Final
    // ==================================================================

    @Test
    @DisplayName("V-2 Point{x=1, y=-2}")
    void v2Point() {
        Writer w = newWriter();
        w.writeI32(1);
        w.writeI32(-2);
        byte[] actual = w.finish();
        byte[] expected = hex("01 00 00 00 FE FF FF FF");
        assertArrayEquals(expected, actual);

        // Roundtrip.
        Reader r = newReader(actual);
        assertEquals(1, r.readI32());
        assertEquals(-2, r.readI32());
    }

    // ==================================================================
    // V-3 Mixed Primitives Final (XTypes-1.3-conform encoder output)
    // ==================================================================

    @Test
    @DisplayName("V-3 All mixed-primitives byte-exact + roundtrip (corrected spec)")
    void v3MixedPrimitives() {
        // XCDR2 caps MAXALIGN at 4 (XTypes 1.3 §7.4.1.1.1): 8-byte primitives
        // align to min(sizeof, 4) = 4, NOT 8. Layout: b@0 o@1 s@2 us@4 pad@6
        // l@8 ul@12 ll@16 ull@24 f@32 d@36 -> 44 bytes.
        Writer w = newWriter();
        w.writeBool(true);
        w.writeU8(0xAB);
        w.writeI16(-12345);
        w.writeU16(54321);
        w.writeI32(-1234567);
        w.writeU32(2345678L);
        w.writeI64(-987654321L);
        w.writeU64(123456789L);
        w.writeF32(2.5f);
        w.writeF64(3.14159);

        byte[] actual = w.finish();
        byte[] expected = hex(
                "01 AB C7 CF 31 D4 00 00 "
                        + "79 29 ED FF CE CA 23 00 "
                        + "4F 97 21 C5 FF FF FF FF "
                        + "15 CD 5B 07 00 00 00 00 "
                        + "00 00 20 40 6E 86 1B F0 "
                        + "F9 21 09 40");
        assertEquals(44, expected.length);
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        assertTrue(r.readBool());
        assertEquals(0xAB, r.readU8());
        assertEquals(-12345, r.readI16());
        assertEquals(54321, r.readU16());
        assertEquals(-1234567, r.readI32());
        assertEquals(2345678L, r.readU32());
        assertEquals(-987654321L, r.readI64());
        assertEquals(123456789L, r.readU64());
        assertEquals(2.5f, r.readF32());
        assertEquals(3.14159, r.readF64(), 0.0);
    }

    // ==================================================================
    // V-4 String Final
    // ==================================================================

    @Test
    @DisplayName("V-4 Greeting{text=\"hello\"}")
    void v4Greeting() {
        Writer w = newWriter();
        w.writeString("hello");
        byte[] actual = w.finish();
        byte[] expected = hex("06 00 00 00 68 65 6C 6C 6F 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        assertEquals("hello", r.readString());
    }

    // ==================================================================
    // V-5 Sequence<int32> Final
    // ==================================================================

    @Test
    @DisplayName("V-5 Bag{ids=[1,2,3]}")
    void v5Bag() {
        Writer w = newWriter();
        int[] ids = {1, 2, 3};
        w.writeU32(ids.length);
        for (int v : ids) {
            w.writeI32(v);
        }
        byte[] actual = w.finish();
        byte[] expected = hex("03 00 00 00 01 00 00 00 02 00 00 00 03 00 00 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        int n = r.readSequenceLength();
        assertEquals(3, n);
        for (int i = 0; i < n; i++) {
            assertEquals(ids[i], r.readI32());
        }
    }

    // ==================================================================
    // V-6 Sequence<string> Final
    // ==================================================================

    @Test
    @DisplayName("V-6 Tags{tags=[\"a\",\"bc\"]}")
    void v6Tags() {
        // XCDR2 7.4.3.5: seq<string> has non-primitive elements -> DHEADER
        // (uint32 = byte length of [count + elements]) prepended.
        Writer w = newWriter();
        String[] tags = {"a", "bc"};
        Writer.DHeader dh = w.writeDheader();
        w.writeU32(tags.length);
        for (String s : tags) {
            w.writeString(s);
        }
        w.finishDheader(dh);
        byte[] actual = w.finish();
        byte[] expected =
                hex("13 00 00 00 02 00 00 00 02 00 00 00 61 00 00 00 03 00 00 00 62 63 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        r.readDheader();
        int n = r.readSequenceLength();
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
        Writer w = newWriter();
        w.writeI32(1234);
        byte[] actual = w.finish();
        byte[] expected = hex("D2 04 00 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        assertEquals(1234, r.readI32());
    }

    // ==================================================================
    // V-8 Keyed Struct (Final)
    // ==================================================================

    @Test
    @DisplayName("V-8 Sensor{id=42, value=3.14}")
    void v8Sensor() {
        Writer w = newWriter();
        w.writeI32(42);
        w.writeF64(3.14);
        byte[] actual = w.finish();
        // XCDR2 caps MAXALIGN at 4 (XTypes 1.3 §7.4.1.1.1): the double aligns to
        // 4 (off 4), NOT 8 — no 4-byte pad after the int32.
        byte[] expected = hex(
                "2A 00 00 00 1F 85 EB 51 B8 1E 09 40");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        assertEquals(42, r.readI32());
        assertEquals(3.14, r.readF64(), 0.0);
    }

    @Test
    @DisplayName("V-8 Key-Hash zero-pad for 4-byte holder per XTypes 7.6.8.4")
    void v8KeyHash() {
        // PlainCdr2BeKeyHolder: BE encoding of the @key fields.
        Writer w = newWriter(Endianness.BIG);
        w.writeI32(42);
        byte[] beKey = w.finish();
        assertArrayEquals(hex("00 00 00 2A"), beKey);

        // XTypes 1.3 §7.6.8.4: holder ≤ 16 octets -> zero-pad to 16 bytes.
        // MD5 only for holders > 16 octets. Here: holder = 4 bytes -> zero-pad.
        byte[] hash = new byte[16];
        if (beKey.length <= 16) {
            System.arraycopy(beKey, 0, hash, 0, beKey.length);
        } else {
            hash = Md5.hash(beKey);
        }
        byte[] expected = hex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00");
        assertArrayEquals(expected, hash);

        // Self-check of the Md5 implementation (separate from §7.6.8.4):
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
        Writer w = newWriter();
        Writer.DHeader dh = w.writeDheader();
        w.writeI32(1);
        w.writeI32(2);
        w.finishDheader(dh);
        byte[] actual = w.finish();
        byte[] expected = hex("08 00 00 00 01 00 00 00 02 00 00 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        int size = r.readDheader();
        assertEquals(8, size);
        assertEquals(1, r.readI32());
        assertEquals(2, r.readI32());
    }

    // ==================================================================
    // V-10 Mutable Struct
    // ==================================================================

    @Test
    @DisplayName("V-10 M{a=42, b=\"hi\"} Mutable (DHEADER=19, string under LC=5)")
    void v10Mutable() {
        // Body = 4(EM1) + 4(a) + 4(EM2) + 7(string, own length reused as the
        // EMHEADER's NEXTINT) = 19, total 23 bytes. XTypes 1.3 §7.4.3.4.2:
        // length codes 5-7 reuse the member's own leading length word as the
        // NEXTINT rather than writing a separate one — a `string` is exactly
        // that case. (The removed Xcdr2Writer wrote a redundant, separate
        // NEXTINT under length code 4 here instead — DHEADER 23 / total 27 —
        // which is not what this runtime, or the generated companion built
        // on it, produce.)
        Writer w = newWriter();
        Writer.DHeader dh = w.writeDheader();
        Writer.EmHeader h1 = w.writeEmheader(1, 2, false);
        w.writeI32(42);
        w.finishEmheader(h1);
        Writer.EmHeader h2 = w.writeEmheader(2, 5, false);
        w.writeString("hi");
        w.finishEmheader(h2);
        w.finishDheader(dh);

        byte[] actual = w.finish();

        assertEquals(23, actual.length);
        // DHEADER: 13 00 00 00 (=19 LE).
        assertArrayEquals(hex("13 00 00 00"), java.util.Arrays.copyOfRange(actual, 0, 4));

        Reader r = newReader(actual);
        int bodySize = r.readDheader();
        assertEquals(19, bodySize);
        Reader.EmHeaderRead e1 = r.readEmheader();
        assertEquals(1, e1.memberId());
        assertEquals(2, e1.lengthCode());
        assertFalse(e1.mustUnderstand());
        assertEquals(42, r.readI32());
        Reader.EmHeaderRead e2 = r.readEmheader();
        assertEquals(2, e2.memberId());
        assertEquals(5, e2.lengthCode());
        assertEquals("hi", r.readString());
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // V-11 Optional Member (Mutable)
    // ==================================================================

    @Test
    @DisplayName("V-11A O{maybe=Some(7)} Mutable Optional present (DHEADER=8)")
    void v11OptionalSome() {
        Writer w = newWriter();
        Writer.DHeader dh = w.writeDheader();
        Writer.EmHeader h = w.writeEmheader(1, 2, false);
        w.writeI32(7);
        w.finishEmheader(h);
        w.finishDheader(dh);
        byte[] actual = w.finish();

        assertEquals(12, actual.length);
        // DHEADER: 08 00 00 00 (=8 LE).
        assertArrayEquals(hex("08 00 00 00"), java.util.Arrays.copyOfRange(actual, 0, 4));

        Reader r = newReader(actual);
        int bodySize = r.readDheader();
        assertEquals(8, bodySize);
        Reader.EmHeaderRead e = r.readEmheader();
        assertEquals(1, e.memberId());
        assertEquals(2, e.lengthCode());
        assertEquals(7, r.readI32());
        assertEquals(0, r.remaining());
    }

    @Test
    @DisplayName("V-11 O{maybe=None} Mutable Optional absent")
    void v11OptionalNone() {
        Writer w = newWriter();
        Writer.DHeader dh = w.writeDheader();
        // No EMHEADER emitted for absent optional.
        w.finishDheader(dh);
        byte[] actual = w.finish();
        byte[] expected = hex("00 00 00 00");
        assertArrayEquals(expected, actual);

        Reader r = newReader(actual);
        int bodySize = r.readDheader();
        assertEquals(0, bodySize);
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // V-12 Mutable Sentinel End-Marker
    // ==================================================================

    @Test
    @DisplayName("V-12 XCDR2 emits NO explicit PID_LIST_END sentinel")
    void v12NoSentinel() {
        // Mutable with two EMHEADERs — we test that no
        // explicit sentinel PID appears after the last EMHEADER.
        Writer w = newWriter();
        Writer.DHeader dh = w.writeDheader();
        Writer.EmHeader h1 = w.writeEmheader(1, 2, false);
        w.writeI32(42);
        w.finishEmheader(h1);
        Writer.EmHeader h2 = w.writeEmheader(2, 2, false);
        w.writeI32(43);
        w.finishEmheader(h2);
        w.finishDheader(dh);
        byte[] actual = w.finish();

        // Body = 16 bytes (2x 4-byte emheader + 2x 4-byte value).
        assertEquals(20, actual.length);

        // DHEADER LE = 16
        assertEquals((byte) 16, actual[0]);

        // Reading respects DHEADER bound — no sentinel pseudo-PID.
        Reader r = newReader(actual);
        int bodySize = r.readDheader();
        assertEquals(16, bodySize);
        Reader.EmHeaderRead e1 = r.readEmheader();
        assertEquals(1, e1.memberId());
        assertEquals(42, r.readI32());
        Reader.EmHeaderRead e2 = r.readEmheader();
        assertEquals(2, e2.memberId());
        assertEquals(43, r.readI32());
        // No more bytes after.
        assertEquals(0, r.remaining());
    }

    // ==================================================================
    // Sanity checks for helpers
    // ==================================================================

    @Test
    @DisplayName("Reader rejects underflow")
    void readerUnderflow() {
        Reader r = newReader(new byte[]{0x01, 0x02});
        try {
            r.readI32();
        } catch (org.zerodds.wire.WireException expected) {
            return;
        }
        assertFalse(true, "expected a WireException");
    }

    @Test
    @DisplayName("Endian flip: BE encoder writes big-endian")
    void endianFlipBe() {
        Writer w = newWriter(Endianness.BIG);
        w.writeI32(0x01020304);
        byte[] actual = w.finish();
        assertArrayEquals(hex("01 02 03 04"), actual);
    }

    @Test
    @DisplayName("fixed<P,S> packed BCD — oracle vectors (JacORB/omniORB) + roundtrip")
    void fixedBcdOracleVectors() {
        String[][] cases = {
            {"123.45", "5", "2", "12 34 5c"},
            {"1234", "4", "0", "01 23 4c"},
            {"-1.50", "6", "2", "00 00 15 0d"},
        };
        for (String[] c : cases) {
            int p = Integer.parseInt(c[1]);
            int s = Integer.parseInt(c[2]);
            Writer w = newWriter();
            w.writeFixed(c[0], p, s, FixedDigitCount.DECLARED);
            assertArrayEquals(hex(c[3]), w.finish(), c[0]);
            Reader r = newReader(w.finish());
            assertEquals(0, new java.math.BigDecimal(r.readFixed(p, s, FixedDigitCount.DECLARED))
                    .compareTo(new java.math.BigDecimal(c[0])));
        }
    }
}
