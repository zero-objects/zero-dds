// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.Condition;
import org.omg.dds.core.GuardCondition;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.WaitSet;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.QueryCondition;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.zerodds.internal.InProcessBus;
import org.zerodds.sub.DataStateImpl;

import java.util.ArrayList;
import java.util.Collection;
import java.util.EnumSet;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS 1.4 §2.2.2.5.3.5 to §2.2.2.5.3.7 and §2.2.2.5.3.31 and DDS-Java-PSM 1.0 §7.3.7.3: the
 * ReadCondition and QueryCondition of a reader (state masks, expression and parameters, trigger
 * value on a WaitSet, close), the StatusCondition of an entity.
 */
class ReaderConditionTest {

    private static int dom = 1060;

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

    private static DataStateImpl sampleStates(SampleState state) {
        return new DataStateImpl(Qos.env()).with(state).withAnyViewState().withAnyInstanceState();
    }

    @Test
    void aReadConditionIsTrueWhileTheReaderHoldsASampleInItsStates() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        ReadCondition<Keyed> notRead = reader.createReadCondition(sampleStates(SampleState.NOT_READ));
        ReadCondition<Keyed> read = reader.createReadCondition(sampleStates(SampleState.READ));
        // the masks are the ones the condition was created with, and the reader is its parent
        assertSame(reader, notRead.getParent());
        assertEquals(EnumSet.of(SampleState.NOT_READ), notRead.getSampleStates());
        assertEquals(EnumSet.allOf(ViewState.class), notRead.getViewStates());
        assertEquals(EnumSet.allOf(InstanceState.class), notRead.getInstanceStates());
        assertEquals(EnumSet.of(SampleState.READ), read.getSampleStates());

        WaitSet waitSet = WaitSet.newWaitSet(Qos.env());
        waitSet.attachCondition(notRead);
        assertFalse(notRead.getTriggerValue());
        assertThrows(TimeoutException.class, () -> waitSet.waitForConditions(30, TimeUnit.MILLISECONDS));

        writer.write(sample(1, 0));
        assertTrue(notRead.getTriggerValue());
        assertFalse(read.getTriggerValue());
        Collection<Condition> active = new ArrayList<>();
        waitSet.waitForConditions(active, 5, TimeUnit.SECONDS);
        assertEquals(1, active.size());
        assertSame(notRead, active.iterator().next());

        // read: the sample is READ now
        Qos.read(reader);
        assertFalse(notRead.getTriggerValue());
        assertTrue(read.getTriggerValue());
        assertThrows(TimeoutException.class, () -> waitSet.waitForConditions(30, TimeUnit.MILLISECONDS));
        // take: no sample is left
        Qos.take(reader);
        assertFalse(read.getTriggerValue());
    }

    @Test
    void aQueryConditionNeedsOneSampleThatIsInTheStatesAndSatisfiesTheExpression() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        QueryCondition<Keyed> condition = reader.createQueryCondition(
                sampleStates(SampleState.READ), "seq > 5", List.of());
        writer.write(sample(1, 0));
        Qos.read(reader); // 1/0 is READ and does not satisfy the expression
        writer.write(sample(2, 10)); // satisfies the expression, but is NOT_READ
        assertFalse(condition.getTriggerValue(),
                "the states and the expression hold for different samples");
        Qos.read(reader); // 2/10 is READ now
        assertTrue(condition.getTriggerValue());
    }

    @Test
    void theQueryParametersAreTheCurrentOnesWhenTheTriggerValueIsEvaluated() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        QueryCondition<Keyed> condition = reader.createQueryCondition("seq > %0", List.of("5"));
        assertEquals("seq > %0", condition.getQueryExpression());
        assertEquals(List.of("5"), condition.getQueryParameters());
        writer.write(sample(1, 3));
        assertFalse(condition.getTriggerValue());
        condition.setQueryParameters(List.of("2"));
        assertEquals(List.of("2"), condition.getQueryParameters());
        assertTrue(condition.getTriggerValue());
        condition.setQueryParameters(List.of("3"));
        assertFalse(condition.getTriggerValue());
    }

    @Test
    void aClosedConditionIsGoneAndAClosedConditionOnAWaitSetNeverTriggers() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        ReadCondition<Keyed> condition = reader.createReadCondition(sampleStates(SampleState.NOT_READ));
        WaitSet waitSet = WaitSet.newWaitSet(Qos.env());
        waitSet.attachCondition(condition);
        condition.close();
        condition.close(); // closing again has no effect
        assertThrows(AlreadyClosedException.class, condition::getTriggerValue);
        writer.write(sample(1, 0));
        assertThrows(TimeoutException.class, () -> waitSet.waitForConditions(30, TimeUnit.MILLISECONDS));
        // the reader is not affected
        assertEquals(1, Qos.read(reader).size());
    }

    @Test
    void closeContainedEntitiesClosesTheConditionsOfTheReaderAndNotTheReader() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        ReadCondition<Keyed> readCondition = reader.createReadCondition(sampleStates(SampleState.NOT_READ));
        QueryCondition<Keyed> queryCondition = reader.createQueryCondition("seq > 0", List.of());
        reader.closeContainedEntities();
        assertThrows(AlreadyClosedException.class, readCondition::getTriggerValue);
        assertThrows(AlreadyClosedException.class, queryCondition::getTriggerValue);
        assertThrows(AlreadyClosedException.class, () -> queryCondition.setQueryParameters(List.of()));
        // the reader works, and new conditions work
        writer.write(sample(1, 1));
        assertEquals(1, Qos.read(reader).size());
        ReadCondition<Keyed> again = reader.createReadCondition(sampleStates(SampleState.READ));
        assertTrue(again.getTriggerValue());
    }

    @Test
    void closingTheReaderClosesItsConditions() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        ReadCondition<Keyed> condition = reader.createReadCondition(sampleStates(SampleState.NOT_READ));
        reader.close();
        assertThrows(AlreadyClosedException.class, condition::getTriggerValue);
        assertThrows(AlreadyClosedException.class,
                () -> reader.createReadCondition(sampleStates(SampleState.NOT_READ)));
        assertThrows(AlreadyClosedException.class,
                () -> reader.createQueryCondition("seq > 0", List.of()));
    }

    @Test
    void theWaitSetListsWhatIsAttachedAndADetachedConditionDoesNotEndAWait() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        ReadCondition<Keyed> readCondition = reader.createReadCondition(sampleStates(SampleState.NOT_READ));
        GuardCondition guard = GuardCondition.newGuardCondition(Qos.env());
        WaitSet waitSet = WaitSet.newWaitSet(Qos.env());
        waitSet.attachCondition(readCondition);
        waitSet.attachCondition(guard);
        assertEquals(2, waitSet.getConditions().size());
        assertTrue(waitSet.getConditions().contains(readCondition));
        assertTrue(waitSet.getConditions().contains(guard));
        waitSet.detachCondition(guard);
        assertEquals(1, waitSet.getConditions().size());
        assertFalse(waitSet.getConditions().contains(guard));
        // a detached condition that becomes true does not end the wait
        guard.setTriggerValue(true);
        assertThrows(TimeoutException.class, () -> waitSet.waitForConditions(30, TimeUnit.MILLISECONDS));
        // attached again, it does, and it is the one reported
        waitSet.attachCondition(guard);
        Collection<Condition> active = new ArrayList<>();
        waitSet.waitForConditions(active, 5, TimeUnit.SECONDS);
        assertEquals(1, active.size());
        assertSame(guard, active.iterator().next());
        // a condition that is not attached cannot be detached
        waitSet.detachCondition(guard);
        assertThrows(PreconditionNotMetException.class, () -> waitSet.detachCondition(guard));
    }

    @Test
    void aStatusConditionTriggersOnlyOnTheStatusesItIsEnabledFor() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        org.omg.dds.core.StatusCondition<DataReader<Keyed>> condition = reader.getStatusCondition();
        assertSame(reader, condition.getParent());
        // the default is every status
        assertEquals(Status.allStatuses(Qos.env()), condition.getEnabledStatuses());
        assertFalse(condition.getTriggerValue());

        DataWriter<Keyed> writer = rig.writer(); // SUBSCRIPTION_MATCHED changes
        assertTrue(condition.getTriggerValue());
        Set<Class<? extends Status>> only = new HashSet<>();
        only.add(DataAvailableStatus.class);
        condition.setEnabledStatuses(only);
        assertEquals(only, condition.getEnabledStatuses());
        assertFalse(condition.getTriggerValue(), "only DATA_AVAILABLE counts, and there is no data");
        writer.write(sample(1, 0));
        assertTrue(condition.getTriggerValue());
        Qos.take(reader); // take resets DATA_AVAILABLE
        assertFalse(condition.getTriggerValue());
    }
}
