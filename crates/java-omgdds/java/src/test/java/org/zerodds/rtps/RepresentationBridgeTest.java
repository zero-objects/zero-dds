// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.junit.jupiter.api.Test;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.XcdrException;
import static org.junit.jupiter.api.Assertions.*;

class RepresentationBridgeTest {
    @Test void xtypes_7_6_3_1_2_the_encapsulation_identifier_names_representation_extensibility_and_byte_order() {
        for (int rep : new int[] {0, 2}) {
            for (ExtensibilityKind kind : ExtensibilityKind.values()) {
                for (EndianMode endian : EndianMode.values()) {
                    byte[] header = SerializedSample.encapsulation(rep, kind, endian);
                    SerializedSample sample = SerializedSample.parse(header);
                    assertEquals(rep, sample.representation);
                    assertEquals(endian, sample.endian);
                    assertDoesNotThrow(() -> sample.requireFraming(kind));
                }
            }
        }
        assertArrayEquals(new byte[] {0, 9, 0, 0}, SerializedSample.encapsulation(
                2, ExtensibilityKind.APPENDABLE, EndianMode.LITTLE_ENDIAN));
        assertArrayEquals(new byte[] {0, 11, 0, 0}, SerializedSample.encapsulation(
                2, ExtensibilityKind.MUTABLE, EndianMode.LITTLE_ENDIAN));
    }

    @Test void xtypes_7_6_3_1_2_the_options_carry_the_padding_count_and_an_unknown_identifier_is_refused() {
        for (byte[] payload : new byte[][] {new byte[0], {0, 7, 0}, {0, 5, 0, 0},
                {1, 7, 0, 0}, {0, 7, 1, 0}, {0, 7, 0, 4}, {0, 7, 0, 2, 0}}) {
            assertThrows(XcdrException.class, () -> SerializedSample.parse(payload));
        }
        assertArrayEquals(new byte[] {42}, SerializedSample.parse(new byte[] {0, 7, 0, 2, 42, 0, 0}).body());
        assertThrows(XcdrException.class, () -> SerializedSample.parse(new byte[] {0, 9, 0, 0})
                .requireFraming(ExtensibilityKind.FINAL));
    }

    @Test void xtypes_7_6_3_1_1_a_writer_and_a_reader_match_when_the_reader_lists_the_writers_representation() {
        byte[] payload = DiscoveryData.buildEndpoint(false, new byte[12], new byte[16], "t", "T",
                Rtps.RELIABILITY_BEST_EFFORT, Rtps.DURABILITY_VOLATILE,
                new int[] {2}, null, new byte[0]);
        assertArrayEquals(new int[] {2}, DiscoveryData.parseEndpoint(false, payload).dataRepresentations);
        assertFalse(RtpsParticipant.representationsOverlap(new int[] {2}, new int[] {0}));
        assertTrue(RtpsParticipant.representationsOverlap(new int[] {2}, new int[] {0, 2}));
        assertThrows(IllegalArgumentException.class, () -> DdsWireBridge.validateRepresentations(new int[] {3}));
    }
}
