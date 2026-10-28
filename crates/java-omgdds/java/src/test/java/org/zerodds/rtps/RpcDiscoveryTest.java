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

    @Test
    void theThreeParametersTravelWithTheEndpoint() {
        RpcDiscovery sent = new RpcDiscovery("calc-7", guid(0x40),
                Arrays.asList("Base_Service_Request", "Other_Service_Request"));
        for (boolean writer : new boolean[] {true, false}) {
            DiscoveryData.Endpoint got = DiscoveryData.parseEndpoint(writer, announce(writer, sent));
            assertNotNull(got);
            assertEquals(sent, got.rpc);
            assertEquals("calc-7", got.rpc.serviceInstanceName());
            assertArrayEquals(guid(0x40), got.rpc.relatedEntityGuid());
            assertEquals(Arrays.asList("Base_Service_Request", "Other_Service_Request"),
                    got.rpc.topicAliases(), "the order of the aliases is kept");
        }
    }

    @Test
    void anEndpointWithoutTheExtensionsAnnouncesNone() {
        DiscoveryData.Endpoint got =
                DiscoveryData.parseEndpoint(true, announce(true, RpcDiscovery.NONE));
        assertTrue(got.rpc.isEmpty());
        assertEquals("", got.rpc.serviceInstanceName());
        assertNull(got.rpc.relatedEntityGuid());
        assertTrue(got.rpc.topicAliases().isEmpty());
    }

    @Test
    void aStringBeyondTheBoundIsRefused() {
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
    void aTopicCorrespondsByNameOrByAlias() {
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

    @Test
    void theGuidMustBeSixteenOctets() {
        assertThrows(IllegalArgumentException.class,
                () -> new RpcDiscovery("", new byte[15], null));
    }
}
