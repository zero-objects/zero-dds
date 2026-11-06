// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;

import static org.junit.jupiter.api.Assertions.*;

/**
 * The XCDR2 codec of the hand-written type supports (org.zerodds.internal.Xcdr2Codec): the
 * octets of DDS-XTypes 1.3 §7.4.1 written out by hand, little-endian, every primitive aligned
 * to its size and at most to 4 (§7.4.1.1.1, MAXALIGN of XCDR2).
 */
class Xcdr2CodecTest {

    private static byte[] hex(String text) {
        String digits = text.replace(" ", "");
        byte[] out = new byte[digits.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(digits.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    @Test
    void javaomgdds_1_3_xcdr2codec_writes_and_reads_the_primitives_with_the_octets_of_xtypes_7_4() {
        // boolean at 0, octet at 1, int16 at 2, int32 at 4, int64 at 8 (aligned to 4, not 8),
        // float32 at 16, float64 at 20 (aligned to 4)
        byte[] expected = hex("01 42 34 12 78 56 34 12 F0 DE BC 9A 78 56 34 12 00 00 60 40"
                + " 00 00 00 00 00 00 04 40");
        ByteBuffer enc = Xcdr2Codec.encoder(64);
        Xcdr2Codec.writeBoolean(enc, true);
        Xcdr2Codec.writeByte(enc, (byte) 0x42);
        Xcdr2Codec.writeShort(enc, (short) 0x1234);
        Xcdr2Codec.writeInt(enc, 0x12345678);
        Xcdr2Codec.writeLong(enc, 0x123456789ABCDEF0L);
        Xcdr2Codec.writeFloat(enc, 3.5f);
        Xcdr2Codec.writeDouble(enc, 2.5);
        assertArrayEquals(expected, Xcdr2Codec.copyToBytes(enc));

        ByteBuffer dec = Xcdr2Codec.decoder(expected);
        assertTrue(Xcdr2Codec.readBoolean(dec));
        assertEquals(0x42, Xcdr2Codec.readByte(dec));
        assertEquals((short) 0x1234, Xcdr2Codec.readShort(dec));
        assertEquals(0x12345678, Xcdr2Codec.readInt(dec));
        assertEquals(0x123456789ABCDEF0L, Xcdr2Codec.readLong(dec));
        assertEquals(3.5f, Xcdr2Codec.readFloat(dec), 0.0f);
        assertEquals(2.5, Xcdr2Codec.readDouble(dec), 0.0);
    }

    @Test
    void javaomgdds_1_3_xcdr2codec_writes_and_reads_a_utf8_string_with_its_length_and_nul() {
        // §7.4.1.1.2: the length counts the UTF-8 octets and the terminating NUL; 'ä' is C3 A4
        byte[] expected = hex("07 00 00 00 4B C3 A4 66 65 72 00");
        ByteBuffer enc = Xcdr2Codec.encoder(64);
        Xcdr2Codec.writeString(enc, "Käfer");
        assertArrayEquals(expected, Xcdr2Codec.copyToBytes(enc));
        assertEquals("Käfer", Xcdr2Codec.readString(Xcdr2Codec.decoder(expected)));
    }
}
