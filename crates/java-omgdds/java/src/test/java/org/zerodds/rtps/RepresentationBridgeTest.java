// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.junit.jupiter.api.Test;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.XcdrException;
import static org.junit.jupiter.api.Assertions.*;

class RepresentationBridgeTest {
    @Test void allSupportedHeadersRetainRepresentationEndianAndFraming() {
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

    @Test void invalidUnknownOptionsAndTruncatedPaddingAreRefused() {
        for (byte[] payload : new byte[][] {new byte[0], {0, 7, 0}, {0, 5, 0, 0},
                {1, 7, 0, 0}, {0, 7, 1, 0}, {0, 7, 0, 4}, {0, 7, 0, 2, 0}}) {
            assertThrows(XcdrException.class, () -> SerializedSample.parse(payload));
        }
        assertArrayEquals(new byte[] {42}, SerializedSample.parse(new byte[] {0, 7, 0, 2, 42, 0, 0}).body());
        assertThrows(XcdrException.class, () -> SerializedSample.parse(new byte[] {0, 9, 0, 0})
                .requireFraming(ExtensibilityKind.FINAL));
    }

    @Test void legacyDescriptorDoesNotPretendToDecodeXcdr1OrBigEndian() {
        assertArrayEquals(new int[] {2}, DdsWireBridge.readRepresentations(ByteArrayTypeSupport.INSTANCE));
        assertThrows(XcdrException.class, () -> DdsWireBridge.decode(ByteArrayTypeSupport.INSTANCE,
                SerializedSample.parse(new byte[] {0, 1, 0, 0, 42})));
        // XCDR2 FINAL encapsulation for the APPENDABLE DDS::Bytes type.
        assertThrows(XcdrException.class, () -> DdsWireBridge.decode(ByteArrayTypeSupport.INSTANCE,
                SerializedSample.parse(new byte[] {0, 7, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 42})));
        assertArrayEquals(new byte[] {42}, DdsWireBridge.decode(ByteArrayTypeSupport.INSTANCE,
                SerializedSample.parse(new byte[] {0, 9, 0, 0, 5, 0, 0, 0, 1, 0, 0, 0, 42})));
    }

    @Test void discoveryAdvertisesAndParsesExactRepresentationSet() {
        byte[] payload = DiscoveryData.buildEndpoint(false, new byte[12], new byte[16], "t", "T",
                Rtps.RELIABILITY_BEST_EFFORT, Rtps.DURABILITY_VOLATILE,
                new int[] {2}, null, new byte[0]);
        assertArrayEquals(new int[] {2}, DiscoveryData.parseEndpoint(false, payload).dataRepresentations);
        assertFalse(RtpsParticipant.representationsOverlap(new int[] {2}, new int[] {0}));
        assertTrue(RtpsParticipant.representationsOverlap(new int[] {2}, new int[] {0, 2}));
        assertThrows(IllegalArgumentException.class, () -> DdsWireBridge.validateRepresentations(new int[] {3}));
    }
}
