// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.Qos;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.zerodds.core.InstanceHandleImpl;
import org.zerodds.pub.DataWriterImpl;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * The identity of a written sample (DDS-RPC 1.0 §7.5.1.1.1 SampleIdentity): the GUID of the
 * writer and a sequence number, 24 octets as PID_RELATED_SAMPLE_IDENTITY (§7.8.2). Domain id 1398.
 */
class WriterSourceIdentityTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static byte[] guid() {
        byte[] guid = new byte[16];
        for (int i = 0; i < 16; i++) {
            guid[i] = (byte) i;
        }
        return guid;
    }

    @Test
    void theWireFormIsTheGuidThenTheSequenceNumberAsHighAndLowWordLittleEndian() {
        // sequence number 0x0000000500000007: high word 5, low word 7
        SourceIdentity identity = new SourceIdentity(guid(), 0x0000000500000007L);
        byte[] wire = identity.toBytesLe();
        assertEquals(24, wire.length);
        assertArrayEquals(guid(), java.util.Arrays.copyOf(wire, 16));
        assertArrayEquals(new byte[] {5, 0, 0, 0, 7, 0, 0, 0},
                java.util.Arrays.copyOfRange(wire, 16, 24));
        assertEquals(identity, SourceIdentity.fromBytesLe(wire));
    }

    @Test
    void aNegativeHighWordAndAFullLowWordSurviveTheRoundTrip() {
        // high word -1 (0xFFFFFFFF), low word 0xFFFFFFFE
        SourceIdentity identity = new SourceIdentity(guid(), 0xFFFFFFFFFFFFFFFEL);
        byte[] wire = identity.toBytesLe();
        assertArrayEquals(new byte[] {-1, -1, -1, -1, -2, -1, -1, -1},
                java.util.Arrays.copyOfRange(wire, 16, 24));
        SourceIdentity back = SourceIdentity.fromBytesLe(wire);
        assertEquals(identity, back);
        assertEquals(identity.sequence(), back.sequence());
        assertEquals(identity.hashCode(), back.hashCode());
    }

    @Test
    void aShortParameterIsNoIdentityAndAGuidHasSixteenOctets() {
        assertNull(SourceIdentity.fromBytesLe(new byte[23]));
        assertNull(SourceIdentity.fromBytesLe(null));
        assertThrows(IllegalArgumentException.class, () -> new SourceIdentity(new byte[15], 1));
        assertNotEquals(new SourceIdentity(guid(), 1), new SourceIdentity(guid(), 2));
    }

    @Test
    void aWriterGivesEverySampleItsOwnIdentityAndTheReaderSeesTheSameOne() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1398);
        org.omg.dds.topic.Topic<byte[]> topic = participant.createTopic("Identity", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        DataWriterImpl<byte[]> impl = (DataWriterImpl<byte[]>) writer;

        // without a wire writer the GUID is the instance handle of the writer
        byte[] expectedGuid = InstanceHandleImpl.bytesOf(writer.getInstanceHandle());
        SourceIdentity next = impl.nextIdentity();
        assertEquals(1, next.sequence());
        assertArrayEquals(expectedGuid, next.guid());

        SourceIdentity first = impl.writeIdentified(new byte[] {1}, null);
        assertEquals(next, first);
        SourceIdentity second = impl.writeIdentified(new byte[] {2}, first);
        assertEquals(2, second.sequence());
        assertEquals(3, impl.nextIdentity().sequence());

        List<Sample<byte[]>> samples = Qos.take(reader);
        assertEquals(2, samples.size());
        assertEquals(1, samples.get(0).getData()[0]);
        assertEquals(2, samples.get(1).getData()[0]);
    }
}
