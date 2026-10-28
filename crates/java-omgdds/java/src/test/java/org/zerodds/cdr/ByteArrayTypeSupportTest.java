// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * DDS-Java-PSM 1.0 §7.8.2 / XTypes 1.3 Annex E — the built-in {@code byte[]}
 * type is {@code DDS::Bytes}, an {@code @extensibility(APPENDABLE)} struct
 * wrapping a {@code ByteSeq}, so its wire form is a 4-byte DHEADER (body
 * size), a 4-byte sequence length, then the octets.
 */
class ByteArrayTypeSupportTest {

    @Test
    void typeNameIsDdsBytes() {
        assertEquals("DDS::Bytes", ByteArrayTypeSupport.INSTANCE.getTypeName());
        assertEquals(ExtensibilityKind.APPENDABLE,
                ByteArrayTypeSupport.INSTANCE.getExtensibility());
    }

    @Test
    void encodeFramesTheBytes() {
        byte[] out = ByteArrayTypeSupport.INSTANCE.encode(
                new byte[]{1, 2, 3}, EndianMode.LITTLE_ENDIAN);
        // DHEADER = 3 + 4 = 7 (LE), length = 3 (LE), then the octets.
        assertArrayEquals(new byte[]{
                7, 0, 0, 0,
                3, 0, 0, 0,
                1, 2, 3,
        }, out);
    }

    @Test
    void decodeUnframesTheBytes() {
        byte[] out = ByteArrayTypeSupport.INSTANCE.decode(new byte[]{
                7, 0, 0, 0,
                3, 0, 0, 0,
                1, 2, 3,
        });
        assertArrayEquals(new byte[]{1, 2, 3}, out);
    }

    @Test
    void roundTrip() {
        byte[] sample = new byte[]{10, 20, 30, 40, 50};
        byte[] framed = ByteArrayTypeSupport.INSTANCE.encode(sample);
        assertArrayEquals(sample, ByteArrayTypeSupport.INSTANCE.decode(framed));
    }

    @Test
    void bigEndianFramingFollowsTheChosenByteOrder() {
        byte[] out = ByteArrayTypeSupport.INSTANCE.encode(
                new byte[]{1, 2, 3}, EndianMode.BIG_ENDIAN);
        assertArrayEquals(new byte[]{
                0, 0, 0, 7,
                0, 0, 0, 3,
                1, 2, 3,
        }, out);
    }

    @Test
    void emptySampleCarriesItsZeroLength() {
        assertArrayEquals(new byte[]{4, 0, 0, 0, 0, 0, 0, 0},
                ByteArrayTypeSupport.INSTANCE.encode(new byte[0]));
        assertArrayEquals(new byte[0],
                ByteArrayTypeSupport.INSTANCE.decode(new byte[]{4, 0, 0, 0, 0, 0, 0, 0}));
    }

    @Test
    void truncatedOrInconsistentFramingIsRefused() {
        // Too short for a DHEADER plus a sequence length.
        assertThrows(XcdrException.class,
                () -> ByteArrayTypeSupport.INSTANCE.decode(new byte[]{1, 2, 3}));
        // DHEADER does not cover the sequence it precedes.
        assertThrows(XcdrException.class, () -> ByteArrayTypeSupport.INSTANCE.decode(
                new byte[]{4, 0, 0, 0, 3, 0, 0, 0, 1, 2, 3}));
        // Sequence length exceeds the payload.
        assertThrows(XcdrException.class, () -> ByteArrayTypeSupport.INSTANCE.decode(
                new byte[]{8, 0, 0, 0, 3, 0, 0, 0, 1, 2}));
    }
}
