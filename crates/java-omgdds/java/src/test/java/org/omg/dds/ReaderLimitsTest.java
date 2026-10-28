// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * RESOURCE_LIMITS of a reader and the SAMPLE_REJECTED status that names the reason and the
 * instance (DDS 1.4 §2.2.3.19, §2.2.4.1), and the historical data of a TRANSIENT_LOCAL writer
 * that a late reader gets (DDS 1.4 §2.2.3.4, §2.2.2.5.3.32).
 */
class ReaderLimitsTest {

    private static int dom = 1160;

    private static int nextDom() {
        return dom++;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    /** The handle of the key {@code id} of {@link Keyed}: its 4 key octets, big-endian, then zeros. */
    private static InstanceHandle handleOfId(int id) {
        byte[] bytes = new byte[16];
        bytes[0] = (byte) (id >>> 24);
        bytes[1] = (byte) (id >>> 16);
        bytes[2] = (byte) (id >>> 8);
        bytes[3] = (byte) id;
        return Values.of(bytes);
    }

    @Test
    void aSampleOfANewInstanceBeyondMaxInstancesIsRejectedWithThatReasonAndInstance()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().ResourceLimits().withMaxInstances(1)));
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        assertEquals(0, reader.getSampleRejectedStatus().getTotalCount());
        writer.write(sample(2, 0));
        writer.write(sample(1, 1)); // the instance 1 is known: no new instance
        SampleRejectedStatus status = reader.getSampleRejectedStatus();
        assertEquals(1, status.getTotalCount());
        assertEquals(1, status.getTotalCountChange());
        assertEquals(SampleRejectedStatus.Kind.REJECTED_BY_INSTANCES_LIMIT, status.getLastReason());
        assertEquals(handleOfId(2), status.getLastInstanceHandle());
        // the samples of instance 1 are in the reader, instance 2 is not
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(2, got.size());
        assertEquals(1, got.get(0).getData().id());
        assertEquals(1, got.get(1).getData().id());
        assertEquals(0, reader.getSampleRejectedStatus().getTotalCountChange());
    }

    @Test
    void aSampleBeyondMaxSamplesPerInstanceIsRejectedWithThatReasonAndInstance()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(2)));
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(1, 1));
        writer.write(sample(2, 0)); // another instance has room
        writer.write(sample(1, 2));
        SampleRejectedStatus status = reader.getSampleRejectedStatus();
        assertEquals(1, status.getTotalCount());
        assertEquals(SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT,
                status.getLastReason());
        assertEquals(handleOfId(1), status.getLastInstanceHandle());
        assertEquals(3, Qos.read(reader).size());
        // taking the samples makes room
        Qos.take(reader);
        writer.write(sample(1, 3));
        assertEquals(1, Qos.read(reader).size());
        assertEquals(1, reader.getSampleRejectedStatus().getTotalCount());
    }

    @Test
    void aLateReaderGetsTheHistoricalDataOfATransientLocalWriterBeforeWaitForHistoricalDataReturns()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataWriter<Keyed> writer = Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(Qos.pf().Durability().withTransientLocal()));
        writer.write(sample(1, 5));
        writer.write(sample(2, 6));
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().Durability().withTransientLocal()));
        reader.waitForHistoricalData(5, TimeUnit.SECONDS);
        List<Sample<Keyed>> got = reader.read(new ArrayList<>());
        assertEquals(2, got.size());
        int sum = 0;
        for (Sample<Keyed> s : got) {
            assertFalse(s.getInstanceHandle().isNil());
            sum += s.getData().id() * 10 + s.getData().seq();
        }
        assertEquals(1 * 10 + 5 + 2 * 10 + 6, sum);
    }
}
