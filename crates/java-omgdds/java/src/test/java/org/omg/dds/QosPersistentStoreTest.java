// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * DURABILITY kind PERSISTENT (DDS 1.4 §2.2.3.4): with the system property
 * {@code zerodds.durability.dir} the samples are kept in a journal and outlive the process;
 * {@code DurabilityService.reset()} stands for the end of a process, as it empties the memory
 * and leaves the directory. Domain ids 1400 to 1499 (this class uses 1490).
 */
class QosPersistentStoreTest {

    private static final int DOMAIN = 1490;
    private static final String PROPERTY = "zerodds.durability.dir";

    @TempDir
    Path dir;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
        System.setProperty(PROPERTY, dir.toString());
    }

    @AfterEach
    void tearDown() {
        System.clearProperty(PROPERTY);
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    private static DataWriterQos writerQos(Durability.Kind kind, boolean autodispose, long cleanupSeconds,
            int depth) {
        return Qos.writer()
                .withPolicy(Qos.pf().Durability().withKind(kind))
                .withPolicy(Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(autodispose))
                .withPolicy(Qos.pf().DurabilityService()
                        .withServiceCleanupDelay(cleanupSeconds, TimeUnit.SECONDS)
                        .withHistoryDepth(depth));
    }

    private static DataReaderQos readerQos(Durability.Kind kind) {
        return Qos.readerKeepAll().withPolicy(Qos.pf().Durability().withKind(kind));
    }

    /** A process: a participant on the domain, its topic, a writer made by {@code write}. */
    private static void aProcessWrites(DataWriterQos qos, java.util.function.Consumer<DataWriter<QosFixtures.Item>> write)
            throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(DOMAIN);
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            DataWriter<QosFixtures.Item> writer = Qos.writer(participant.createPublisher(), topic,
                    QosFixtures.SUPPORT, qos);
            write.accept(writer);
            writer.close();
        } finally {
            participant.close();
        }
        DurabilityService.instance().reset(); // the process ends: the memory is gone, the directory stays
    }

    /** Another process: what a PERSISTENT or TRANSIENT reader is given when it starts. */
    private static List<Sample<QosFixtures.Item>> aProcessReads(Durability.Kind kind) throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(DOMAIN);
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            DataReader<QosFixtures.Item> reader = Qos.reader(participant.createSubscriber(), topic,
                    QosFixtures.SUPPORT, readerQos(kind));
            return Qos.take(reader);
        } finally {
            participant.close();
        }
    }

    private static void write(DataWriter<QosFixtures.Item> writer, int id, int seq) {
        try {
            writer.write(new QosFixtures.Item(id, seq));
        } catch (Exception e) {
            throw new IllegalStateException(e);
        }
    }

    private static List<Integer> sortedSeqs(List<Sample<QosFixtures.Item>> samples) {
        List<Integer> seqs = new ArrayList<>();
        for (Sample<QosFixtures.Item> sample : samples) {
            if (sample.getData() != null) {
                seqs.add(sample.getData().seq());
            }
        }
        Collections.sort(seqs);
        return seqs;
    }

    private List<Path> journals() throws IOException {
        try (Stream<Path> files = Files.list(dir)) {
            return files.collect(Collectors.toList());
        }
    }

    @Test
    void persistentSamplesOutliveTheProcess() throws Exception {
        aProcessWrites(writerQos(Durability.Kind.PERSISTENT, false, 0, 2), writer -> {
            write(writer, 1, 0);
            write(writer, 1, 1);
            write(writer, 1, 2);
            write(writer, 2, 5);
        });
        assertEquals(1, journals().size(), "one journal for the topic");
        // the history depth 2 of the DURABILITY_SERVICE: samples 1 and 2 of the first instance, 5 of the second
        assertEquals(Arrays.asList(1, 2, 5), sortedSeqs(aProcessReads(Durability.Kind.PERSISTENT)));
        // and still, for a process after that
        DurabilityService.instance().reset();
        assertEquals(Arrays.asList(1, 2, 5), sortedSeqs(aProcessReads(Durability.Kind.PERSISTENT)));
    }

    @Test
    void transientSamplesAreNotWrittenAndDoNotOutliveTheProcess() throws Exception {
        aProcessWrites(writerQos(Durability.Kind.TRANSIENT, false, 0, 2), writer -> write(writer, 1, 0));
        assertEquals(0, journals().size());
        assertEquals(0, aProcessReads(Durability.Kind.TRANSIENT).size());
    }

    @Test
    void withoutADirectoryPersistentLivesAsLongAsTheProcess() throws Exception {
        System.clearProperty(PROPERTY);
        aProcessWrites(writerQos(Durability.Kind.PERSISTENT, false, 0, 2), writer -> write(writer, 1, 0));
        assertEquals(0, journals().size());
        assertEquals(0, aProcessReads(Durability.Kind.PERSISTENT).size());
    }

    @Test
    void aDisposedInstanceIsToldDisposedAfterTheRestartUntilItsDelayHasPassed() throws Exception {
        // the writer disposes when it closes; the store keeps the instance for an hour
        aProcessWrites(writerQos(Durability.Kind.PERSISTENT, true, 3600, 1), writer -> write(writer, 1, 7));
        List<Sample<QosFixtures.Item>> got = aProcessReads(Durability.Kind.PERSISTENT);
        assertEquals(2, got.size(), "the sample and the dispose");
        assertEquals(7, got.get(0).getData().seq());
        for (Sample<QosFixtures.Item> sample : got) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, sample.getInstanceState());
        }
    }

    @Test
    void aDisposedInstanceWithoutDelayIsForgottenAfterTheRestartAndStaysForgotten() throws Exception {
        aProcessWrites(writerQos(Durability.Kind.PERSISTENT, true, 0, 1), writer -> write(writer, 1, 7));
        assertEquals(0, aProcessReads(Durability.Kind.PERSISTENT).size());
        DurabilityService.instance().reset();
        assertEquals(0, aProcessReads(Durability.Kind.PERSISTENT).size(), "the purge was written, too");
    }

    @Test
    void aRecordThatWasCutOffAtTheEndOfTheJournalIsIgnored() throws Exception {
        aProcessWrites(writerQos(Durability.Kind.PERSISTENT, false, 0, 2), writer -> write(writer, 1, 3));
        Path journal = journals().get(0);
        // the tag of a sample record and nothing after it: a write the process did not finish
        Files.write(journal, new byte[] {1}, StandardOpenOption.APPEND);
        assertEquals(Arrays.asList(3), sortedSeqs(aProcessReads(Durability.Kind.PERSISTENT)));
    }
}
