// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.Test;
import org.omg.dds.rpc.HeaderCodec;
import org.omg.dds.rpc.ServiceMapping;
import org.omg.dds.rpc.TypeSupport;
import org.omg.dds.rpc.WireEncoding;
import org.zerodds.cdr.TopicTypeSupport;

/**
 * {@link RpcTopicTypeSupport} announces the topic as {@link RawTypeSupport} does: every default
 * method of {@link TopicTypeSupport} that the raw support answers, the typed one answers too.
 * Missing was {@code matchesByTypeInformation}: the typed DataWriter and DataReader views of
 * DDS-RPC 1.0 §7.11.2 then paired by the type name, and a Java client of a base interface found
 * no service of the derived one (measured: {@code java_peer.rs}, "no service matched").
 */
class RpcTopicTypeSupportTest {

    /** A support that names its type, as a generated one does. */
    static final class Octets implements TypeSupport<byte[]> {
        @Override
        public byte[] serialize(byte[] value) {
            return value.clone();
        }

        @Override
        public byte[] deserialize(byte[] octets, HeaderCodec.Cursor cursor) {
            return octets.clone();
        }
    }

    @Test
    void rpc_7_11_2_a_described_topic_pairs_by_type_information_in_the_typed_view_too() {
        TypeDescription description = new TypeDescription(new byte[] {1}, new byte[][] {new byte[8]},
                new int[][] {new int[0]});
        RawTypeSupport raw = new RawTypeSupport("robot::Calc_Request", description);
        assertTrue(raw.matchesByTypeInformation());
        TopicSample<byte[]> sample = TopicSample.request(new Octets(), ServiceMapping.ENHANCED, WireEncoding.XCDR2);
        assertNotNull(sample);
        assertTrue(new RpcTopicTypeSupport<byte[]>(raw, sample).matchesByTypeInformation());
    }
}
