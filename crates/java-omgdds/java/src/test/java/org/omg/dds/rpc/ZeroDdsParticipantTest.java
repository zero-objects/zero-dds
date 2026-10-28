// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.Qos;
import org.zerodds.ddsrpc.ZeroDdsParticipant;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.SourceIdentity;
import org.zerodds.rtps.ParameterList;
import org.zerodds.rtps.Rtps;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-RPC 1.0 §7.8.2 over the DDS Java PSM: a write returns the identity the
 * sample was sent under and takes the identity it relates to; the sample taken
 * carries both.
 */
class ZeroDdsParticipantTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static byte[] octets(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    @Test
    void aWriteReturnsItsIdentityAndTheReaderSeesItWithTheRelatedOne() {
        ZeroDdsParticipant rpc = new ZeroDdsParticipant(Qos.factory().createParticipant(910));
        RpcQos qos = RpcQos.defaults();
        DataReader reader = rpc.createReader("Calc_Request", "Calc_Request", qos, null);
        DataWriter writer = rpc.createWriter("Calc_Request", "Calc_Request", qos, null);
        assertTrue(writer.carriesRelatedIdentity());
        assertEquals(1, writer.matchedCount());
        assertEquals(1, reader.matchedCount());

        SampleIdentity expected = writer.nextIdentity();
        SampleIdentity first = writer.write(octets(1, 2, 3));
        assertEquals(expected, first);
        assertEquals(first.sequenceNumber() + 1, writer.nextIdentity().sequenceNumber());
        SampleIdentity second = writer.write(octets(4), first);
        assertNotEquals(first, second);
        assertArrayEquals(first.writerGuid(), second.writerGuid());

        List<TakenSample> taken = reader.take();
        assertEquals(2, taken.size());
        assertArrayEquals(octets(1, 2, 3), taken.get(0).octets());
        assertEquals(first, taken.get(0).identity());
        assertTrue(taken.get(0).relatedIdentity().isUnknown());
        // the octets are the sample: no framing is added in front of the RPC header
        assertArrayEquals(octets(4), taken.get(1).octets());
        assertEquals(second, taken.get(1).identity());
        assertEquals(first, taken.get(1).relatedIdentity());
    }

    @Test
    void theIdentityIsTwentyFourOctetsOnTheWire() {
        byte[] guid = new byte[16];
        guid[0] = 7;
        guid[15] = 9;
        // high word -1 and low word 5, the form of RELATED_SAMPLE_IDENTITY_INVALID with a low word
        SourceIdentity identity = new SourceIdentity(guid, 0xFFFFFFFF00000005L);
        byte[] wire = identity.toBytesLe();
        assertEquals(24, wire.length);
        assertArrayEquals(octets(0xff, 0xff, 0xff, 0xff, 5, 0, 0, 0),
                java.util.Arrays.copyOfRange(wire, 16, 24));
        assertEquals(identity, SourceIdentity.fromBytesLe(wire));
        assertEquals(0x0083, Rtps.PID_RELATED_SAMPLE_IDENTITY);
        // as the inline QoS carries it
        ParameterList inline = new ParameterList().add(Rtps.PID_RELATED_SAMPLE_IDENTITY, wire);
        byte[] bytes = inline.toBytesLe();
        ParameterList back = ParameterList.fromBytesLe(bytes, 0, bytes.length);
        assertNotNull(back.find(Rtps.PID_RELATED_SAMPLE_IDENTITY));
        assertEquals(identity, SourceIdentity.fromBytesLe(back.find(Rtps.PID_RELATED_SAMPLE_IDENTITY)));
        assertFalse(identity.equals(new SourceIdentity(guid, 5)));
    }
}
