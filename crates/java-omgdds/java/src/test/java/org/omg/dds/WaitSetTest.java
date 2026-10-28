// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.Condition;
import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.WaitSet;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.omg.dds.topic.Topic;
import org.zerodds.DataStates;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.7.4 and DDS 1.2 §7.1.2.1.6 to §7.1.2.1.9: conditions and the wait set.
 */
class WaitSetTest {

    private static final org.omg.dds.core.ServiceEnvironment ENV = Qos.env();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aGuardConditionIsFalseAtFirstAndTheWaitSetKnowsWhatIsAttached() {
        GuardCondition guard = GuardCondition.newGuardCondition(ENV);
        assertFalse(guard.getTriggerValue());
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        assertTrue(waitSet.getConditions().isEmpty());
        waitSet.attachCondition(guard);
        waitSet.attachCondition(guard); // attaching twice has no effect
        assertEquals(1, waitSet.getConditions().size());
        waitSet.detachCondition(guard);
        assertTrue(waitSet.getConditions().isEmpty());
        assertThrows(PreconditionNotMetException.class, () -> waitSet.detachCondition(guard));
    }

    @Test
    void aWaitWithATimeoutEndsInATimeoutException() {
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        waitSet.attachCondition(GuardCondition.newGuardCondition(ENV));
        assertThrows(TimeoutException.class, () -> waitSet.waitForConditions(30, TimeUnit.MILLISECONDS));
    }

    @Test
    void aGuardConditionSetFromAnotherThreadEndsABlockedWaitAndIsReported() throws Exception {
        GuardCondition guard = GuardCondition.newGuardCondition(ENV);
        GuardCondition other = GuardCondition.newGuardCondition(ENV);
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        waitSet.attachCondition(guard);
        waitSet.attachCondition(other);
        Thread setter = new Thread(() -> {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                return;
            }
            guard.setTriggerValue(true);
        });
        setter.start();
        List<Condition> active = new ArrayList<>();
        long start = System.nanoTime();
        waitSet.waitForConditions(active); // no timeout: only the signal ends it
        setter.join();
        assertTrue(TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start) < 5000);
        assertEquals(1, active.size());
        assertSame(guard, active.get(0));
    }

    @Test
    void aSecondThreadWaitingOnTheSameWaitSetIsRefused() throws Exception {
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        GuardCondition guard = GuardCondition.newGuardCondition(ENV);
        waitSet.attachCondition(guard);
        CountDownLatch blocked = new CountDownLatch(1);
        Thread first = new Thread(() -> {
            blocked.countDown();
            waitSet.waitForConditions();
        });
        first.start();
        blocked.await();
        Thread.sleep(100);
        AtomicReference<Throwable> second = new AtomicReference<>();
        Thread other = new Thread(() -> {
            try {
                waitSet.waitForConditions(1, TimeUnit.SECONDS);
            } catch (Throwable t) {
                second.set(t);
            }
        });
        other.start();
        other.join();
        guard.setTriggerValue(true);
        first.join();
        assertTrue(second.get() instanceof PreconditionNotMetException, String.valueOf(second.get()));
    }

    @Test
    void aStatusConditionTriggersOnTheStatusItIsEnabledForAndHasItsEntityAsParent() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(940);
        Topic<byte[]> topic = participant.createTopic("WaitStatus", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        StatusCondition<DataReader<byte[]>> condition = reader.getStatusCondition();
        assertSame(reader, condition.getParent());
        // the default list is all statuses
        assertTrue(condition.getEnabledStatuses().contains(SubscriptionMatchedStatus.class));
        assertFalse(condition.getTriggerValue());
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        waitSet.attachCondition(condition);
        Thread writerThread = new Thread(() -> {
            try {
                Thread.sleep(50);
            } catch (InterruptedException e) {
                return;
            }
            participant.createPublisher().createDataWriter(topic);
        });
        writerThread.start();
        waitSet.waitForConditions(5, TimeUnit.SECONDS); // woken by the SUBSCRIPTION_MATCHED change
        writerThread.join();
        assertTrue(condition.getTriggerValue());
        // enabling only another status makes it false
        java.util.Set<Class<? extends org.omg.dds.core.status.Status>> only = new java.util.HashSet<>();
        only.add(org.omg.dds.core.status.LivelinessChangedStatus.class);
        condition.setEnabledStatuses(only);
        assertFalse(condition.getTriggerValue());
    }

    @Test
    void aReadConditionFollowsTheStatesOfTheSamplesAndWakesTheWaitSet() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(941);
        Topic<byte[]> topic = participant.createTopic("WaitRead", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        ReadCondition<byte[]> notRead = reader.createReadCondition(DataStates.notRead(ENV));
        ReadCondition<byte[]> read = reader.createReadCondition(
                new org.zerodds.sub.DataStateImpl(ENV).with(SampleState.READ).withAnyViewState()
                        .withAnyInstanceState()); // with() adds, so start from the empty set
        assertSame(reader, notRead.getParent());
        assertEquals(1, notRead.getSampleStates().size());
        assertTrue(notRead.getSampleStates().contains(SampleState.NOT_READ));
        assertEquals(2, notRead.getViewStates().size());
        assertTrue(notRead.getInstanceStates().contains(InstanceState.ALIVE));
        assertFalse(notRead.getTriggerValue());
        WaitSet waitSet = WaitSet.newWaitSet(ENV);
        waitSet.attachCondition(notRead);
        Thread writerThread = new Thread(() -> {
            try {
                Thread.sleep(50);
                writer.write(new byte[] {1});
            } catch (InterruptedException | TimeoutException e) {
                // the test fails on the wait
            }
        });
        writerThread.start();
        Collection<Condition> active = new ArrayList<>();
        waitSet.waitForConditions(active, 5, TimeUnit.SECONDS);
        writerThread.join();
        assertEquals(1, active.size());
        assertTrue(notRead.getTriggerValue());
        assertFalse(read.getTriggerValue());
        reader.read(new ArrayList<>());
        assertFalse(notRead.getTriggerValue());
        assertTrue(read.getTriggerValue());
        assertEquals(ViewState.NOT_NEW, reader.read(new ArrayList<>()).get(0).getViewState());
    }
}
