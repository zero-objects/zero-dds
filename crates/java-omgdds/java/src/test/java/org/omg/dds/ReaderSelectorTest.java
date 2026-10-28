// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.QueryCondition;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.internal.InProcessBus;
import org.zerodds.sub.DataStateImpl;
import org.zerodds.sub.SampleImpl;
import org.zerodds.sub.SelectorImpl;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS-Java-PSM 1.0 §7.7.3: the {@code Selector} of read and take (instance, nextInstance,
 * dataState, content, maxSamples) and the defaults it starts with, read_next_sample and
 * take_next_sample, and the read and take with a ReadCondition or QueryCondition
 * (DDS 1.4 §2.2.2.5.3.8 to §2.2.2.5.3.19).
 */
class ReaderSelectorTest {

    private static int dom = 1020;

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

    /** Three instances (ids 3, 1, 2 written in this order), two samples each. */
    private static DataReader<Keyed> threeInstances(ReaderRig rig) throws Exception {
        DataReader<Keyed> reader = rig.keepAllReader();
        fill(rig.writer());
        return reader;
    }

    private static InstanceHandle handleOf(DataReader<Keyed> reader, int id) {
        return reader.lookupInstance(sample(id, 0));
    }

    private static DataStateImpl states(SampleState sample, ViewState view, InstanceState instance) {
        DataStateImpl state = new DataStateImpl(Qos.env());
        if (sample == null) {
            state.withAnySampleState();
        } else {
            state.with(sample);
        }
        if (view == null) {
            state.withAnyViewState();
        } else {
            state.with(view);
        }
        if (instance == null) {
            state.withAnyInstanceState();
        } else {
            state.with(instance);
        }
        return state;
    }

    private static List<Integer> ids(List<Sample<Keyed>> samples) {
        List<Integer> out = new ArrayList<>();
        for (Sample<Keyed> s : samples) {
            out.add(s.getData().id());
        }
        return out;
    }

    @Test
    void theDefaultSelectorSelectsEverything() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        DataReader.Selector<Keyed> selector = reader.select();
        // §7.7.3: instanceHandle=null, nextInstance=false, dataState=any, queryExpression=null,
        // maxSamples=unlimited
        assertNull(selector.getInstance());
        assertFalse(selector.retrieveNextInstance());
        assertNull(selector.getQueryExpression());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, selector.getMaxSamples());
        assertNotNull(selector.getDataState());
        assertEquals(DataStateImpl.any(Qos.env()), selector.getDataState());
        assertNull(selector.getCondition());
        // read() with it returns the samples of all instances, in the order the instances arrived
        List<Sample<Keyed>> got = selector.read(new ArrayList<>());
        assertEquals(List.of(3, 3, 1, 1, 2, 2), ids(got));
    }

    @Test
    void maxSamplesLimitsReadAndTakeAndUnlimitedIsAcceptedAsAValue() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        assertEquals(4, reader.select().maxSamples(4).read(new ArrayList<>()).size());
        assertEquals(3, count(reader.read(3)));
        assertEquals(0, reader.select().maxSamples(0).read(new ArrayList<>()).size());
        assertEquals(6, reader.select().maxSamples(ResourceLimits.LENGTH_UNLIMITED)
                .read(new ArrayList<>()).size());
        assertThrows(IllegalArgumentException.class, () -> reader.select().maxSamples(-2));
        // take(2) removes the first two samples of the reader
        assertEquals(2, count(reader.take(2)));
        List<Sample<Keyed>> rest = Qos.read(reader);
        assertEquals(List.of(1, 1, 2, 2), ids(rest));
    }

    private static int count(java.util.Iterator<?> iterator) {
        int n = 0;
        while (iterator.hasNext()) {
            iterator.next();
            n++;
        }
        return n;
    }

    @Test
    void anInstanceHandleLimitsReadAndTakeToThatInstance() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        InstanceHandle two = handleOf(reader, 2);
        List<Sample<Keyed>> got = reader.select().instance(two).read(new ArrayList<>());
        assertEquals(List.of(2, 2), ids(got));
        for (Sample<Keyed> s : got) {
            assertEquals(two, s.getInstanceHandle());
        }
        // take removes only these
        List<Sample<Keyed>> taken = reader.select().instance(two).take(new ArrayList<>());
        assertEquals(List.of(2, 2), ids(taken));
        assertEquals(List.of(3, 3, 1, 1), ids(Qos.read(reader)));
        // a handle of no instance of the reader selects nothing
        assertEquals(0, reader.select().instance(two).read(new ArrayList<>()).size());
    }

    @Test
    void nextInstanceWalksTheInstancesInTheOrderOfTheirHandles() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        // the handles of the keys 1, 2, 3 are ordered like the keys (big-endian key octets)
        assertTrue(handleOf(reader, 1).compareTo(handleOf(reader, 2)) < 0);
        assertTrue(handleOf(reader, 2).compareTo(handleOf(reader, 3)) < 0);

        // from no handle on: the instance with the smallest handle, and only that
        List<Sample<Keyed>> first = reader.select().nextInstance(true).read(new ArrayList<>());
        assertEquals(List.of(1, 1), ids(first));
        InstanceHandle previous = first.get(0).getInstanceHandle();
        List<Sample<Keyed>> second =
                reader.select().instance(previous).nextInstance(true).read(new ArrayList<>());
        assertEquals(List.of(2, 2), ids(second));
        previous = second.get(0).getInstanceHandle();
        List<Sample<Keyed>> third =
                reader.select().instance(previous).nextInstance(true).read(new ArrayList<>());
        assertEquals(List.of(3, 3), ids(third));
        previous = third.get(0).getInstanceHandle();
        assertEquals(0, reader.select().instance(previous).nextInstance(true)
                .read(new ArrayList<>()).size());

        // take_next_instance takes the samples of that one instance
        List<Sample<Keyed>> taken = reader.select().nextInstance(true).take(new ArrayList<>());
        assertEquals(List.of(1, 1), ids(taken));
        assertEquals(List.of(3, 3, 2, 2), ids(Qos.read(reader)));
    }

    @Test
    void nextInstanceSkipsInstancesWithoutSamplesInTheSelectedStates() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        // the samples of instance 2 are read
        reader.select().instance(handleOf(reader, 2)).read(new ArrayList<>());
        DataStateImpl notRead = states(SampleState.NOT_READ, null, null);
        List<Sample<Keyed>> got = reader.select().dataState(notRead)
                .instance(handleOf(reader, 1)).nextInstance(true).read(new ArrayList<>());
        assertEquals(List.of(3, 3), ids(got)); // 2 has no sample that is NOT_READ
    }

    @Test
    void theDataStateSelectsOnSampleViewAndInstanceStates() throws Exception {
        // every query reads, and a read changes the sample and view states, so each query has a
        // reader of its own that was brought into the same state
        ReaderRig rig = new ReaderRig(nextDom());
        List<DataReader<Keyed>> readers = new ArrayList<>();
        for (int i = 0; i < 8; i++) {
            readers.add(rig.keepAllReader());
        }
        org.omg.dds.pub.DataWriter<Keyed> writer = rig.writer();
        fill(writer);
        // instance 3 is disposed: each reader gets a notification sample without data
        Qos.dispose(writer, sample(3, 0));
        // the samples of instance 1 are read: READ, and the instance NOT_NEW
        for (DataReader<Keyed> reader : readers) {
            reader.select().instance(handleOf(reader, 1)).read(new ArrayList<>());
        }
        // instance 3: 3/0 3/1 and the notification, NEW, NOT_ALIVE_DISPOSED, NOT_READ
        // instance 1: 1/0 1/1, NOT_NEW, ALIVE, READ
        // instance 2: 2/0 2/1, NEW, ALIVE, NOT_READ

        assertEquals(List.of(1, 1), ids(readers.get(0).select()
                .dataState(states(SampleState.READ, null, null)).read(new ArrayList<>())));
        assertEquals(5, readers.get(1).select()
                .dataState(states(SampleState.NOT_READ, null, null))
                .read(new ArrayList<>()).size());
        assertEquals(List.of(1, 1), ids(readers.get(2).select()
                .dataState(states(null, ViewState.NOT_NEW, null)).read(new ArrayList<>())));
        assertEquals(5, readers.get(3).select()
                .dataState(states(null, ViewState.NEW, null)).read(new ArrayList<>()).size());
        List<Sample<Keyed>> disposed = readers.get(4).select()
                .dataState(states(null, null, InstanceState.NOT_ALIVE_DISPOSED))
                .read(new ArrayList<>());
        assertEquals(3, disposed.size());
        for (Sample<Keyed> s : disposed) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, s.getInstanceState());
        }
        assertEquals(4, readers.get(5).select()
                .dataState(states(null, null, InstanceState.ALIVE)).read(new ArrayList<>()).size());
        assertEquals(0, readers.get(6).select()
                .dataState(states(null, null, InstanceState.NOT_ALIVE_NO_WRITERS))
                .read(new ArrayList<>()).size());
        // take with a mask removes only what it selected
        DataReader<Keyed> taker = readers.get(7);
        assertEquals(2, taker.select().dataState(states(SampleState.READ, null, null))
                .take(new ArrayList<>()).size());
        assertEquals(5, Qos.read(taker).size());
    }

    private static void fill(org.omg.dds.pub.DataWriter<Keyed> writer) throws Exception {
        for (int id : new int[] {3, 1, 2}) {
            writer.write(sample(id, 0));
            writer.write(sample(id, 1));
        }
    }

    @Test
    void readNextSampleReturnsTheNextSampleNotYetReadAndTakeNextSampleRemovesIt() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        org.omg.dds.pub.DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(1, 1));
        SampleImpl<Keyed> holder = blank();
        assertTrue(reader.readNextSample(holder));
        assertEquals(0, holder.getData().seq());
        assertEquals(SampleState.NOT_READ, holder.getSampleState());
        // the second call returns the next sample, not the same again
        assertTrue(reader.readNextSample(holder));
        assertEquals(1, holder.getData().seq());
        // no sample that is not read is left, and the holder keeps what it had
        assertFalse(reader.readNextSample(holder));
        assertEquals(1, holder.getData().seq());
        assertEquals(2, Qos.read(reader).size()); // read does not remove

        writer.write(sample(2, 5));
        assertTrue(reader.takeNextSample(holder));
        assertEquals(5, holder.getData().seq());
        assertFalse(reader.takeNextSample(holder));
        assertEquals(2, Qos.read(reader).size()); // the taken sample is gone
    }

    private static SampleImpl<Keyed> blank() {
        return SampleImpl.of(Qos.env(), null, null, null, null, SampleState.READ, ViewState.NOT_NEW,
                InstanceState.ALIVE);
    }

    @Test
    void aReadConditionOfTheReaderSelectsItsStatesWithTheSelector() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        ReadCondition<Keyed> notRead =
                reader.createReadCondition(states(SampleState.NOT_READ, null, null));
        List<Sample<Keyed>> first = withCondition(reader, notRead).read(new ArrayList<>());
        assertEquals(6, first.size());
        // they are READ now: the condition selects none, and the trigger value is false
        assertEquals(0, withCondition(reader, notRead).read(new ArrayList<>()).size());
        assertFalse(notRead.getTriggerValue());
        assertEquals(6, reader.select().read(new ArrayList<>()).size());

        // a condition of another reader is PRECONDITION_NOT_MET
        DataReader<Keyed> other = rig.keepAllReader();
        ReadCondition<Keyed> foreign = other.createReadCondition(states(null, null, null));
        assertThrows(PreconditionNotMetException.class,
                () -> withCondition(reader, foreign).read(new ArrayList<>()));
        // a closed condition is ALREADY_DELETED
        notRead.close();
        assertThrows(org.zerodds.core.AlreadyClosedException.class,
                () -> withCondition(reader, notRead).read(new ArrayList<>()));
    }

    @Test
    void aQueryConditionOfTheReaderSelectsByItsExpressionAndTheNextInstanceKeepsIt() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = threeInstances(rig);
        QueryCondition<Keyed> later = reader.createQueryCondition("seq > %0", List.of("0"));
        // seq 1 of the instances 3, 1, 2
        List<Sample<Keyed>> got = withCondition(reader, later).read(new ArrayList<>());
        assertEquals(List.of(3, 1, 2), ids(got));
        // read_next_instance_w_condition: from instance 1 on, the next with such a sample
        List<Sample<Keyed>> next = withCondition(reader, later)
                .instance(handleOf(reader, 1)).nextInstance(true).read(new ArrayList<>());
        assertEquals(List.of(2), ids(next));
        assertEquals(1, next.get(0).getData().seq());
        // the parameters of the condition are the current ones
        later.setQueryParameters(List.of("5"));
        assertEquals(0, withCondition(reader, later).read(new ArrayList<>()).size());
        // take_w_condition removes only what the condition selected
        later.setQueryParameters(List.of("0"));
        assertEquals(3, withCondition(reader, later).take(new ArrayList<>()).size());
        assertEquals(3, Qos.read(reader).size());
    }

    private static SelectorImpl<Keyed> withCondition(DataReader<Keyed> reader,
            ReadCondition<Keyed> condition) {
        return ((SelectorImpl<Keyed>) reader.select()).condition(condition);
    }

    @Test
    void theCopyFlavourReplacesWhatTheListHeldAndAllocatesAListForNull() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        org.omg.dds.pub.DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(1, 1));
        writer.write(sample(1, 2));
        // a list with more samples than are returned is trimmed
        List<Sample<Keyed>> list = new ArrayList<>();
        for (int i = 0; i < 5; i++) {
            list.add(blank());
        }
        assertEquals(2, reader.select().maxSamples(2).read(list).size());
        assertEquals(2, list.size());
        assertEquals(0, list.get(0).getData().seq());
        assertEquals(1, list.get(1).getData().seq());
        // a list with fewer is extended
        List<Sample<Keyed>> one = new ArrayList<>();
        one.add(blank());
        assertEquals(3, reader.read(one).size());
        assertEquals(3, one.size());
        // null: a new list
        List<Sample<Keyed>> fresh = reader.read((List<Sample<Keyed>>) null);
        assertNotNull(fresh);
        assertEquals(3, fresh.size());
        List<Sample<Keyed>> viaSelector = reader.select().take((List<Sample<Keyed>>) null);
        assertEquals(3, viaSelector.size());
        List<Sample<Keyed>> none = reader.take((List<Sample<Keyed>>) null);
        assertNotNull(none);
        assertTrue(none.isEmpty());
    }
}
