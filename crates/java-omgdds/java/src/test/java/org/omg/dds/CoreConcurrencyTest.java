// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.Condition;
import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.WaitSet;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Set;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.4: the operations of the entities are reentrant. The
 * tests start several threads on one object and wait for them with an upper
 * limit; they check results that do not depend on timing: counts of samples
 * written and taken, distinct objects created, the QoS that is left.
 */
class CoreConcurrencyTest {

    private static final int DOMAIN = 1508;
    private static final int THREADS = 8;
    private static final long LIMIT_SECONDS = 120;

    private DomainParticipant participant;
    private Topic<byte[]> topic;
    private Publisher publisher;
    private Subscriber subscriber;
    private ExecutorService pool;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("CoreConcurrency", byte[].class);
        publisher = participant.createPublisher();
        subscriber = participant.createSubscriber();
        pool = Executors.newFixedThreadPool(THREADS + 2);
    }

    @AfterEach
    void tearDown() {
        pool.shutdownNow();
        participant.close();
        InProcessBus.instance().reset();
    }

    /** Runs every task on its own thread, all released at once, and waits for all. */
    private <T> List<T> runTogether(List<Callable<T>> tasks) throws Exception {
        final CountDownLatch start = new CountDownLatch(1);
        List<Future<T>> futures = new ArrayList<>();
        for (final Callable<T> task : tasks) {
            futures.add(pool.submit(new Callable<T>() {
                @Override
                public T call() throws Exception {
                    start.await();
                    return task.call();
                }
            }));
        }
        start.countDown();
        List<T> results = new ArrayList<>();
        for (Future<T> future : futures) {
            results.add(future.get(LIMIT_SECONDS, TimeUnit.SECONDS));
        }
        return results;
    }

    @Test
    void severalThreadsCreateEntitiesAndSetTheQosOfOnePublisherAndSubscriber() throws Exception {
        final Set<Object> created = Collections.newSetFromMap(new IdentityHashMap<Object, Boolean>());
        List<Callable<List<Object>>> tasks = new ArrayList<>();
        final Set<String> partitions = new HashSet<>();
        for (int t = 0; t < THREADS; t++) {
            final int thread = t;
            partitions.add("p" + thread);
            tasks.add(new Callable<List<Object>>() {
                @Override
                public List<Object> call() {
                    List<Object> mine = new ArrayList<>();
                    for (int i = 0; i < 10; i++) {
                        mine.add(publisher.createDataWriter(topic));
                        mine.add(subscriber.createDataReader(topic));
                        mine.add(participant.createTopic("CoreConcurrency" + thread + "_" + i, byte[].class));
                        publisher.setQos(publisher.getQos().withPolicy(
                                Qos.pf().Partition().withName(Collections.singleton("p" + thread))));
                        subscriber.setQos(subscriber.getQos().withPolicy(
                                Qos.pf().Partition().withName(Collections.singleton("p" + thread))));
                    }
                    return mine;
                }
            });
        }
        for (List<Object> mine : runTogether(tasks)) {
            assertEquals(30, mine.size());
            created.addAll(mine);
        }
        // 8 threads x 10 rounds x (writer, reader, topic): every object is a different one
        assertEquals(THREADS * 10 * 3, created.size());
        // the QoS that is left is the one of the last setQos call of one of the threads
        assertTrue(partitions.contains(publisher.getQos().getPartition().getName().iterator().next()));
        assertTrue(partitions.contains(subscriber.getQos().getPartition().getName().iterator().next()));
    }

    @Test
    void severalThreadsWriteTakeAndRegisterOnOneWriterAndReader() throws Exception {
        final DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        final DataReader<byte[]> reader = subscriber.createDataReader(topic, Qos.readerKeepAll());
        final AtomicInteger taken = new AtomicInteger();
        final AtomicBoolean writing = new AtomicBoolean(true);
        final int writers = 4;
        final int perWriter = 50;

        Future<?> taker = pool.submit(new Callable<Object>() {
            @Override
            public Object call() {
                while (writing.get()) {
                    taken.addAndGet(reader.take(new ArrayList<org.omg.dds.sub.Sample<byte[]>>()).size());
                    Thread.yield();
                }
                return null;
            }
        });
        List<Callable<Object>> tasks = new ArrayList<>();
        for (int t = 0; t < writers; t++) {
            final int thread = t;
            tasks.add(new Callable<Object>() {
                @Override
                public Object call() throws Exception {
                    for (int i = 0; i < perWriter; i++) {
                        writer.write(new byte[] {(byte) thread, (byte) i});
                    }
                    return null;
                }
            });
        }
        for (int t = 0; t < 2; t++) {
            tasks.add(new Callable<Object>() {
                @Override
                public Object call() throws Exception {
                    for (int i = 0; i < 20; i++) {
                        assertNotNull(writer.registerInstance(new byte[] {1}));
                    }
                    return null;
                }
            });
        }
        runTogether(tasks);
        writing.set(false);
        taker.get(LIMIT_SECONDS, TimeUnit.SECONDS);
        taken.addAndGet(reader.take(new ArrayList<org.omg.dds.sub.Sample<byte[]>>()).size());
        // 4 writers x 50 samples, each delivered once to the one reader
        assertEquals(writers * perWriter, taken.get());
    }

    @Test
    void closingAWriterWhileOthersWriteLeavesOnlyAlreadyClosed() throws Exception {
        final DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        final DataReader<byte[]> reader = subscriber.createDataReader(topic);
        List<Callable<Object>> tasks = new ArrayList<>();
        for (int t = 0; t < 4; t++) {
            tasks.add(new Callable<Object>() {
                @Override
                public Object call() throws Exception {
                    try {
                        for (int i = 0; i < 200; i++) {
                            writer.write(new byte[] {1});
                        }
                    } catch (AlreadyClosedException expected) {
                        // the close won the race
                    }
                    return null;
                }
            });
        }
        tasks.add(new Callable<Object>() {
            @Override
            public Object call() {
                writer.close();
                return null;
            }
        });
        tasks.add(new Callable<Object>() {
            @Override
            public Object call() {
                try {
                    for (int i = 0; i < 200; i++) {
                        reader.take(new ArrayList<org.omg.dds.sub.Sample<byte[]>>());
                    }
                } catch (AlreadyClosedException expected) {
                    // closed below
                }
                return null;
            }
        });
        runTogether(tasks);
        assertTrue(Qos.isClosed(writer));
        reader.close();
        assertTrue(Qos.isClosed(reader));
    }

    @Test
    void conditionsAreAttachedAndDetachedWhileAThreadWaits() throws Exception {
        final WaitSet waitSet = WaitSet.newWaitSet(Qos.env());
        final GuardCondition guard = GuardCondition.newGuardCondition(Qos.env());
        waitSet.attachCondition(guard);
        final DataWriter<byte[]> writer = publisher.createDataWriter(topic);

        Future<List<Condition>> waiter = pool.submit(new Callable<List<Condition>>() {
            @Override
            public List<Condition> call() throws Exception {
                List<Condition> active = new ArrayList<>();
                waitSet.waitForConditions(active, LIMIT_SECONDS, TimeUnit.SECONDS);
                return active;
            }
        });
        for (int i = 0; i < 100; i++) {
            GuardCondition extra = GuardCondition.newGuardCondition(Qos.env());
            waitSet.attachCondition(extra);
            assertTrue(waitSet.getConditions().contains(extra));
            assertTrue(waitSet.getConditions().contains(guard));
            waitSet.detachCondition(extra);
            Set<Class<? extends Status>> none = Collections.emptySet();
            writer.getStatusCondition().setEnabledStatuses(none);
            writer.getStatusCondition().setEnabledStatuses(
                    Arrays.<Class<? extends Status>>asList(org.omg.dds.core.status.PublicationMatchedStatus.class));
        }
        assertEquals(Collections.singletonList(guard), new ArrayList<Condition>(waitSet.getConditions()));
        guard.setTriggerValue(true);
        List<Condition> active = waiter.get(LIMIT_SECONDS, TimeUnit.SECONDS);
        assertTrue(active.contains(guard));
    }
}
