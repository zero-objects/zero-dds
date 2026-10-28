// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.4 2.2.2.4.1.10: a coherent set reaches a reader as a whole. A reader on another
 * thread takes all samples of a set or none of them, never part of it.
 */
class CoherentSetAtomicityTest {

    private static final int SET_SIZE = 25;
    private static final int SETS = 300;

    @Test
    void aReaderOnAnotherThreadNeverSeesPartOfASet() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1340);
        PublisherQos coherent = Qos.publisher().withPolicy(
                Qos.policies().Presentation().withTopic().withCoherentAccess(true));
        Publisher publisher = participant.createPublisher(coherent);
        Topic<byte[]> topic = participant.createTopic("CoherentAtomic", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        final DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());

        final AtomicBoolean done = new AtomicBoolean();
        final AtomicInteger total = new AtomicInteger();
        final List<Integer> partial = new ArrayList<Integer>();
        Thread taker = new Thread(new Runnable() {
            @Override
            public void run() {
                while (!done.get() || total.get() < SETS * SET_SIZE) {
                    // read the flag before the take: all sets are delivered when it is set, so an empty
                    // take after that is the end (a flag read after the take can follow sets that the
                    // take did not see)
                    final boolean finished = done.get();
                    List<Sample<byte[]>> taken = Qos.take(reader);
                    if (taken.size() % SET_SIZE != 0) {
                        synchronized (partial) {
                            partial.add(taken.size());
                        }
                    }
                    total.addAndGet(taken.size());
                    if (finished && taken.isEmpty()) {
                        break;
                    }
                }
            }
        });
        taker.start();
        for (int s = 0; s < SETS; s++) {
            publisher.beginCoherentChanges();
            for (int i = 0; i < SET_SIZE; i++) {
                writer.write(new byte[] {(byte) i});
            }
            publisher.endCoherentChanges();
        }
        done.set(true);
        taker.join(30000);
        participant.close();

        synchronized (partial) {
            assertTrue(partial.isEmpty(), "a take returned part of a set: " + partial);
        }
        assertEquals(SETS * SET_SIZE, total.get());
    }
}
