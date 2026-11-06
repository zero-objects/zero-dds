// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;
import java.util.Collections;
import org.junit.jupiter.api.Test;

/**
 * The discovery extensions of DDS-RPC 1.0 §7.6.2.1.2 on the SEDP announcement: the
 * {@code service_instance_name} (PID 0x0080), the {@code related_*_key} (PID 0x0081) and the
 * {@code topic_aliases} (PID 0x0082), and the matching by topic alias (§7.4.3).
 */
class RpcDiscoveryTest {

    private static byte[] guid(int tag) {
        byte[] out = new byte[16];
        for (int at = 0; at < 16; ++at) {
            out[at] = (byte) (tag + at);
        }
        return out;
    }

    private static byte[] announce(boolean writer, RpcDiscovery rpc) {
        return DiscoveryData.buildEndpoint(writer, new byte[12], guid(1), "Calc_Request", "T",
                Rtps.RELIABILITY_RELIABLE, Rtps.DURABILITY_VOLATILE, new int[] {0}, null,
                new byte[0], new byte[0], null, rpc);
    }

    /**
     * The value of parameter {@code pid} in a PL_CDR_LE payload (the encapsulation header, then
     * the parameter list of DDSI-RTPS 2.5 §9.4.2.11), read here and not by the decoder under test.
     */
    private static byte[] parameter(byte[] payload, int pid) {
        java.nio.ByteBuffer b = java.nio.ByteBuffer.wrap(payload).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        b.position(4);
        while (b.remaining() >= 4) {
            int id = b.getShort() & 0xFFFF;
            int length = b.getShort() & 0xFFFF;
            if (id == 0x0001) {
                break;
            }
            byte[] value = new byte[length];
            b.get(value);
            if (id == pid) {
                return value;
            }
        }
        return null;
    }

    private static byte[] cdrString(String text) {
        byte[] octets = text.getBytes(java.nio.charset.StandardCharsets.US_ASCII);
        java.nio.ByteBuffer b = java.nio.ByteBuffer.allocate(4 + octets.length + 1)
                .order(java.nio.ByteOrder.LITTLE_ENDIAN);
        b.putInt(octets.length + 1).put(octets).put((byte) 0);
        return b.array();
    }

    @Test
    void rpc_7_6_2_1_2_the_three_discovery_parameters_are_read_from_their_parameter_ids() {
        RpcDiscovery sent = new RpcDiscovery("calc-7", guid(0x40),
                Arrays.asList("Base_Service_Request", "Other_Service_Request"));
        for (boolean writer : new boolean[] {true, false}) {
            byte[] payload = announce(writer, sent);
            // service_instance_name: a string<256> on PID 0x0080
            assertArrayEquals(cdrString("calc-7"),
                    Arrays.copyOf(parameter(payload, 0x0080), cdrString("calc-7").length));
            // related_entity_guid: the sixteen octets of the GUID on PID 0x0081
            assertArrayEquals(guid(0x40), parameter(payload, 0x0081));
            // topic_aliases: a sequence of strings on PID 0x0082, the count first
            byte[] aliases = parameter(payload, 0x0082);
            java.nio.ByteBuffer expected = java.nio.ByteBuffer.allocate(4 + 28 + 28)
                    .order(java.nio.ByteOrder.LITTLE_ENDIAN);
            expected.putInt(2).put(cdrString("Base_Service_Request")).put(new byte[3])
                    .put(cdrString("Other_Service_Request"));
            byte[] head = Arrays.copyOf(expected.array(), expected.position());
            assertArrayEquals(head, Arrays.copyOf(aliases, head.length));

            DiscoveryData.Endpoint got = DiscoveryData.parseEndpoint(writer, payload);
            assertNotNull(got);
            assertEquals("calc-7", got.rpc.serviceInstanceName());
            assertArrayEquals(guid(0x40), got.rpc.relatedEntityGuid());
            assertEquals(Arrays.asList("Base_Service_Request", "Other_Service_Request"),
                    got.rpc.topicAliases(), "the order of the aliases is kept");
        }
    }

    @Test
    void rpc_7_6_2_1_2_a_name_beyond_string_256_is_refused() {
        char[] long257 = new char[257];
        Arrays.fill(long257, 'x');
        assertThrows(IllegalArgumentException.class,
                () -> new RpcDiscovery(new String(long257), null, null));
        assertThrows(IllegalArgumentException.class,
                () -> new RpcDiscovery("", null, Collections.singletonList(new String(long257))));
        char[] long256 = new char[256];
        Arrays.fill(long256, 'y');
        assertEquals(256, new RpcDiscovery(new String(long256), null, null)
                .serviceInstanceName().length(), "string<256> holds 256 octets");
        // An announcement that carries one is dropped, as the Rust decoder drops it.
        ParameterList pl = new ParameterList();
        pl.add(Rtps.PID_SERVICE_INSTANCE_NAME, ParameterList.cdrString(new String(long257)));
        assertNull(RpcDiscovery.readFrom(pl));
    }

    @Test
    void rpc_7_4_3_a_topic_corresponds_by_name_or_by_alias() {
        RpcDiscovery withAlias = new RpcDiscovery("", null, Collections.singletonList("Base_Request"));
        assertTrue(RpcDiscovery.topicsCorrespond("A", RpcDiscovery.NONE, "A", RpcDiscovery.NONE));
        assertFalse(RpcDiscovery.topicsCorrespond("A", RpcDiscovery.NONE, "B", RpcDiscovery.NONE));
        // The derived service names its parent's topic as an alias: the base client matches.
        assertTrue(RpcDiscovery.topicsCorrespond("Base_Request", RpcDiscovery.NONE,
                "Derived_Request", withAlias));
        assertTrue(RpcDiscovery.topicsCorrespond("Derived_Request", withAlias,
                "Base_Request", RpcDiscovery.NONE));
        assertFalse(RpcDiscovery.topicsCorrespond("Other_Request", RpcDiscovery.NONE,
                "Derived_Request", withAlias));
    }

}
