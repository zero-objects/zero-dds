// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.omg.dds.topic.Topic;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;

import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * The SampleInfo of DDS 1.4 §2.2.2.5.1 as the PSM's {@code Sample} carries it: generation counts
 * (§2.2.2.5.1.5), ranks (§2.2.2.5.1.6), view_state (§2.2.2.5.1.8), source_timestamp,
 * instance_handle and publication_handle.
 */
class ReaderSampleInfoTest {

    private static int dom = 1000;

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

    @Test
    void anInstanceDisposedAndWrittenAgainCountsADisposedGeneration() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        Qos.dispose(writer, sample(1, 0));
        writer.write(sample(1, 1));

        // the reader holds: the first sample, the notification of the dispose (no data), the
        // sample that made the instance ALIVE again
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(3, got.size());
        assertEquals(0, got.get(0).getData().seq());
        assertNull(got.get(1).getData());
        assertEquals(1, got.get(2).getData().seq());
        // disposed_generation_count: 0 before the instance was reborn, 1 after
        assertEquals(0, got.get(0).getDisposedGenerationCount());
        assertEquals(0, got.get(1).getDisposedGenerationCount());
        assertEquals(1, got.get(2).getDisposedGenerationCount());
        for (Sample<Keyed> s : got) {
            assertEquals(0, s.getNoWritersGenerationCount());
        }
        // sample_rank: the samples of the instance after this one in the collection
        assertEquals(2, got.get(0).getSampleRank());
        assertEquals(1, got.get(1).getSampleRank());
        assertEquals(0, got.get(2).getSampleRank());
        // generation_rank = (generations of the last sample in the collection) - (those of S)
        assertEquals(1, got.get(0).getGenerationRank());
        assertEquals(1, got.get(1).getGenerationRank());
        assertEquals(0, got.get(2).getGenerationRank());
        // absolute_generation_rank = (generations of the instance now) - (those of S)
        assertEquals(1, got.get(0).getAbsoluteGenerationRank());
        assertEquals(1, got.get(1).getAbsoluteGenerationRank());
        assertEquals(0, got.get(2).getAbsoluteGenerationRank());
        // instance_state is the state of the instance when the collection was obtained
        for (Sample<Keyed> s : got) {
            assertEquals(InstanceState.ALIVE, s.getInstanceState());
        }
    }

    @Test
    void anInstanceWithoutWritersWrittenAgainCountsANoWritersGeneration() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(2, 0));
        Qos.unregister(writer, sample(2, 0));
        writer.write(sample(2, 1));

        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(3, got.size());
        assertEquals(0, got.get(0).getNoWritersGenerationCount());
        assertEquals(0, got.get(1).getNoWritersGenerationCount());
        assertEquals(1, got.get(2).getNoWritersGenerationCount());
        for (Sample<Keyed> s : got) {
            assertEquals(0, s.getDisposedGenerationCount());
        }
        assertEquals(1, got.get(0).getGenerationRank());
        assertEquals(1, got.get(0).getAbsoluteGenerationRank());
        assertEquals(0, got.get(2).getGenerationRank());
    }

    @Test
    void theRanksAreMeasuredWithinTheSamplesOfOneInstanceInTheCollection() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(2, 0));
        writer.write(sample(1, 1));
        writer.write(sample(2, 1));

        // the reader returns the samples instance by instance: 1/0, 1/1, 2/0, 2/1
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(4, got.size());
        assertEquals(1, got.get(0).getSampleRank());
        assertEquals(0, got.get(1).getSampleRank());
        assertEquals(1, got.get(2).getSampleRank());
        assertEquals(0, got.get(3).getSampleRank());
        for (Sample<Keyed> s : got) {
            assertEquals(0, s.getGenerationRank());
            assertEquals(0, s.getAbsoluteGenerationRank());
        }
        // a collection cut after the first sample of an instance: that sample is the last of
        // the instance in it, so its rank is 0
        List<Sample<Keyed>> one = reader.select().maxSamples(1).read(new java.util.ArrayList<>());
        assertEquals(1, one.size());
        assertEquals(0, one.get(0).getSampleRank());
    }

    @Test
    void allSamplesOfAnInstanceInOneCollectionHaveTheSameViewStateAndTheSampleStateOfBeforeTheRead() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(1, 1));

        List<Sample<Keyed>> first = Qos.read(reader);
        assertEquals(2, first.size());
        for (Sample<Keyed> s : first) {
            assertEquals(ViewState.NEW, s.getViewState());
            assertEquals(SampleState.NOT_READ, s.getSampleState());
        }
        List<Sample<Keyed>> second = Qos.read(reader);
        for (Sample<Keyed> s : second) {
            assertEquals(ViewState.NOT_NEW, s.getViewState());
            assertEquals(SampleState.READ, s.getSampleState());
        }
        // a reborn instance is NEW again (§2.2.2.5.1.8)
        Qos.dispose(writer, sample(1, 1));
        writer.write(sample(1, 2));
        List<Sample<Keyed>> all = Qos.read(reader);
        Sample<Keyed> reborn = all.get(all.size() - 1);
        assertEquals(ViewState.NEW, reborn.getViewState());
        assertEquals(1, reborn.getDisposedGenerationCount());
    }

    @Test
    void theSourceTimestampIsTheOneTheWriterGave() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0), Values.ofMillis(42_000L));
        writer.write(sample(2, 0), 77_000L, TimeUnit.MILLISECONDS);
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(2, got.size());
        assertEquals(42_000L, got.get(0).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
        assertEquals(77_000L, got.get(1).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void thePublicationHandleNamesTheWriterThatIsMatchedWithTheReader() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        Sample<Keyed> got = Qos.read(reader).get(0);
        assertEquals(1, reader.getMatchedPublications().size());
        InstanceHandle matched = reader.getMatchedPublications().iterator().next();
        assertEquals(matched, got.getPublicationHandle());
        assertFalse(got.getPublicationHandle().isNil());
    }

    @Test
    void theInstanceHandleOfASampleIsTheOneTheReaderAndTheWriterLookUp() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(5, 0));
        Sample<Keyed> got = Qos.read(reader).get(0);
        assertFalse(got.getInstanceHandle().isNil());
        assertEquals(got.getInstanceHandle(), reader.lookupInstance(sample(5, 99)));
        assertEquals(got.getInstanceHandle(), writer.lookupInstance(sample(5, 98)));
        // another key, another handle
        writer.write(sample(6, 0));
        assertNotEquals(reader.lookupInstance(sample(5, 0)), reader.lookupInstance(sample(6, 0)));
    }

    @Test
    void aKeyLongerThanSixteenOctetsKeepsItsOwnInstanceHandle() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<ReaderRig.Named> topic = participant.createTopic("ReaderNamedTopic",
                ReaderRig.Named.class);
        DataReader<ReaderRig.Named> reader = Qos.reader(participant.createSubscriber(), topic,
                ReaderRig.NAMED, Qos.readerKeepAll());
        DataWriter<ReaderRig.Named> writer =
                Qos.writer(participant.createPublisher(), topic, ReaderRig.NAMED, Qos.writer());
        // 17 octets each; byte 0 and byte 16 are exchanged, so the XOR of the octets
        // that share a position of a 16-octet fold is the same for both keys
        String first = "x" + "yyyyyyyyyyyyyyy" + "z";
        String second = "z" + "yyyyyyyyyyyyyyy" + "x";
        writer.write(new ReaderRig.Named(first, 1));
        writer.write(new ReaderRig.Named(second, 2));

        List<Sample<ReaderRig.Named>> got = Qos.read(reader);
        assertEquals(2, got.size());
        assertNotEquals(got.get(0).getInstanceHandle(), got.get(1).getInstanceHandle());
        assertEquals(got.get(0).getInstanceHandle(),
                reader.lookupInstance(new ReaderRig.Named(first, 0)));
        assertEquals(got.get(1).getInstanceHandle(),
                reader.lookupInstance(new ReaderRig.Named(second, 0)));
        assertEquals(got.get(1).getInstanceHandle(),
                writer.lookupInstance(new ReaderRig.Named(second, 0)));
    }

    @Test
    void anInstanceWithoutLiveWritersAndWithoutSamplesIsForgottenByTheReader() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        Qos.unregister(writer, sample(1, 0));
        assertFalse(reader.lookupInstance(sample(1, 0)).isNil());
        // the notification of the missing writer and the sample are taken: DDS 1.4
        // §2.2.2.5.1.8 lets the reader reclaim the instance
        List<Sample<Keyed>> taken = Qos.take(reader);
        assertEquals(2, taken.size());
        assertEquals(InstanceState.NOT_ALIVE_NO_WRITERS, taken.get(0).getInstanceState());
        assertTrue(reader.lookupInstance(sample(1, 0)).isNil());
        // a later sample is an instance never seen: NEW, generation counts 0
        writer.write(sample(1, 1));
        Sample<Keyed> again = Qos.read(reader).get(0);
        assertEquals(ViewState.NEW, again.getViewState());
        assertEquals(0, again.getNoWritersGenerationCount());
        assertEquals(0, again.getAbsoluteGenerationRank());
    }

    @Test
    void aDisposedInstanceStaysDisposedWhenItsLastWriterUnregisters() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        Qos.dispose(writer, sample(1, 0));
        Qos.unregister(writer, sample(1, 0));
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(2, got.size()); // the sample and the notification of the dispose
        for (Sample<Keyed> s : got) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, s.getInstanceState());
        }
        // once its samples are taken the reader forgets it
        Qos.take(reader);
        assertTrue(reader.lookupInstance(sample(1, 0)).isNil());
    }
}
