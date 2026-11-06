// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import org.junit.jupiter.api.Test;

/** Pure-Java RTPS wire stack: byte-layout unit tests + a real UDP roundtrip. */
final class RtpsWireTest {

    @Test
    void rtps_9_6_2_2_1_a_vendor_parameter_id_is_its_own_namespace_and_only_the_must_understand_bit_is_masked() {
        byte[] standard = new byte[] {1, 2, 3, 4};
        byte[] vendor = new byte[] {5, 6, 7, 8};
        for (int id : new int[] {0x0002, 0x0075}) {
            for (int flag : new int[] {0, 0x4000}) {
                for (boolean vendorFirst : new boolean[] {true, false}) {
                    ParameterList list = new ParameterList();
                    if (vendorFirst) list.add(id | 0x8000 | flag, vendor).add(id | flag, standard);
                    else list.add(id | flag, standard).add(id | 0x8000 | flag, vendor);
                    byte[] encoded = list.toBytesLe();
                    ParameterList decoded = ParameterList.fromBytesLe(encoded, 0, encoded.length);
                    for (int queryFlag : new int[] {0, 0x4000}) {
                        assertArrayEquals(standard, decoded.find(id | queryFlag));
                        assertArrayEquals(vendor, decoded.find(id | 0x8000 | queryFlag));
                        List<byte[]> standardMatches = DiscoveryData.findAll(decoded, id | queryFlag);
                        List<byte[]> vendorMatches = DiscoveryData.findAll(decoded, id | 0x8000 | queryFlag);
                        assertEquals(1, standardMatches.size());
                        assertEquals(1, vendorMatches.size());
                        assertArrayEquals(standard, standardMatches.get(0));
                        assertArrayEquals(vendor, vendorMatches.get(0));
                    }
                }
            }
        }
        ParameterList vendorOnly = new ParameterList().add(0xC075, vendor);
        org.junit.jupiter.api.Assertions.assertNull(vendorOnly.find(0x0075));
        assertTrue(DiscoveryData.findAll(vendorOnly, 0x4075).isEmpty());
        assertArrayEquals(vendor, vendorOnly.find(0x8075));
    }

    /** Independent XTypes aggregate framing, not produced by DiscoveryData. */
    private static byte[] standardTypeInformationVector() {
        java.nio.ByteBuffer b = java.nio.ByteBuffer.allocate(100).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        b.putInt(96);
        for (int i = 0; i < 2; i++) {
            b.putInt(0x40001001 + i).putInt(40).putInt(36).putInt(20);
            b.put((byte)(0xf1 + i));
            for (int n = 0; n < 14; n++) b.put((byte)(i * 16 + n + 1));
            b.put((byte)0).putInt(77 + i).putInt(0).putInt(4).putInt(0);
        }
        assertEquals(100, b.position());
        return b.array();
    }

    @Test
    void rtps_9_4_2_11_every_parameter_length_is_a_multiple_of_four() {
        ParameterList pl = new ParameterList()
                .add(Rtps.PID_TOPIC_NAME, ParameterList.cdrString("Chatter"))
                .add(Rtps.PID_TYPE_NAME, ParameterList.cdrString("zerodds::RawBytes"));
        byte[] wire = pl.toBytesLe();
        // every parameter length field must be 4-aligned (Rust decoder rejects otherwise)
        int pos = 0;
        while (pos + 4 <= wire.length) {
            int id = Wire.u16le(wire, pos);
            int len = Wire.u16le(wire, pos + 2);
            pos += 4;
            if (id == Rtps.PID_SENTINEL) {
                break;
            }
            assertEquals(0, len % 4, "PID 0x" + Integer.toHexString(id) + " length not 4-aligned");
            pos += len;
        }
        ParameterList back = ParameterList.fromBytesLe(wire, 0, wire.length);
        assertEquals("Chatter", ParameterList.readCdrString(back.find(Rtps.PID_TOPIC_NAME)));
        assertEquals("zerodds::RawBytes", ParameterList.readCdrString(back.find(Rtps.PID_TYPE_NAME)));
    }

    @Test
    void rtps_9_4_2_11_a_string_parameter_counts_its_terminating_nul() {
        byte[] v = ParameterList.cdrString("ab");
        // u32 length = bytes+1 = 3, then 'a','b','\0'
        assertEquals(3, (int) Wire.u32le(v, 0));
        assertEquals('a', v[4]);
        assertEquals('b', v[5]);
        assertEquals(0, v[6]);
    }

}
