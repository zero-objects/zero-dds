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
    void javapsm_7_8_2_a_byte_array_topic_has_the_type_dds_bytes_and_is_appendable() {
        assertEquals("DDS::Bytes", ByteArrayTypeSupport.INSTANCE.getTypeName());
        assertEquals(ExtensibilityKind.APPENDABLE,
                ByteArrayTypeSupport.INSTANCE.getExtensibility());
    }

    @Test
    void javapsm_7_8_2_a_byte_array_is_written_as_dheader_length_and_octets() {
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
    void javapsm_7_8_2_a_byte_array_is_read_from_dheader_length_and_octets() {
        byte[] out = ByteArrayTypeSupport.INSTANCE.decode(new byte[]{
                7, 0, 0, 0,
                3, 0, 0, 0,
                1, 2, 3,
        });
        assertArrayEquals(new byte[]{1, 2, 3}, out);
    }

    @Test
    void javapsm_7_8_2_a_big_endian_byte_array_has_big_endian_dheader_and_length() {
        byte[] out = ByteArrayTypeSupport.INSTANCE.encode(
                new byte[]{1, 2, 3}, EndianMode.BIG_ENDIAN);
        assertArrayEquals(new byte[]{
                0, 0, 0, 7,
                0, 0, 0, 3,
                1, 2, 3,
        }, out);
    }

}
