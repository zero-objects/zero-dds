// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.cdr.ReflectionTypeSupport.MemberRef;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.core.qos.DataWriterQosImpl;
import org.zerodds.core.qos.QosCompatibility;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.sub.DataReaderImpl;
import org.zerodds.sub.SubscriberImpl;

import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Predicate;

/**
 * The join of a MultiTopic for one DataReader (DDS-DCPS 1.4 §2.2.2.3.4).
 *
 * <p>A DataReader on a MultiTopic "accesses instances that are constructed at
 * the DataReader side from the instances written by multiple DataWriter
 * entities". For each such reader this engine holds
 * <ul>
 *   <li>one internal DataReader per topic of the {@code FROM} clause, on the
 *       reader's Subscriber (so the Subscriber's partitions and the
 *       reader's QoS apply to what is received),</li>
 *   <li>the current sample of every instance of each of those topics,</li>
 *   <li>one internal DataWriter of the result type, on a topic that only
 *       exists for this reader and never leaves the process.</li>
 * </ul>
 * When a sample arrives, the engine forms every tuple of the current samples
 * that contains it and satisfies the natural join, projects the joined row
 * onto the output fields, filters it with the {@code WHERE} clause and
 * writes the result sample. The reader on the result topic is an ordinary
 * {@link DataReaderImpl}: history, sample states, listeners, conditions and
 * the WaitSet behave as they do for any reader.
 *
 * <h2>Instances</h2>
 * A result instance is identified by the key members of the result type. It
 * exists once all its constituting instances exist; its states follow the
 * constituting ones as §2.2.2.3.4 describes: it is ALIVE while all of them
 * are, NOT_ALIVE_DISPOSED as soon as one is disposed (the engine disposes
 * the result instance), and NOT_ALIVE_NO_WRITERS when one has no writers and
 * none is disposed (the engine unregisters it). The view state is the one of
 * the result reader: NEW from the moment the instance exists until the
 * application first accesses it.
 */
public final class MultiTopicEngine<TYPE> {

    private static final AtomicLong SEQUENCE = new AtomicLong();

    /** Prefix of the topic names that exist only inside a process for a join. */
    static final String INTERNAL_PREFIX = "$zerodds.multitopic$";

    /** Whether {@code topicName} names the internal topic of a join. */
    static boolean isInternal(String topicName) {
        return topicName != null && topicName.startsWith(INTERNAL_PREFIX);
    }

    private enum Liveness { ALIVE, NO_WRITERS }

    private enum ResultState { ALIVE, NO_WRITERS, DISPOSED }

    /** The current sample of one instance of a source topic. */
    private static final class Entry {
        final Map<String, Object> row;
        final Object sample;
        final InstanceHandle handle;
        final long timestampMillis;
        Liveness liveness = Liveness.ALIVE;

        Entry(Map<String, Object> row, Object sample, InstanceHandle handle,
                long timestampMillis) {
            this.row = row;
            this.sample = sample;
            this.handle = handle;
            this.timestampMillis = timestampMillis;
        }
    }

    private static final class Source {
        final int index;
        final Topic<?> topic;
        final List<MemberRef> members;
        DataReaderImpl<?> reader;
        final Map<InstanceHandle, Entry> entries = new LinkedHashMap<>();

        Source(int index, Topic<?> topic, List<MemberRef> members) {
            this.index = index;
            this.topic = topic;
            this.members = members;
        }
    }

    /** A constituting instance: the instance {@code handle} of source {@code source}. */
    private static final class Ref {
        final int source;
        final InstanceHandle handle;

        Ref(int source, InstanceHandle handle) {
            this.source = source;
            this.handle = handle;
        }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof Ref)) {
                return false;
            }
            Ref other = (Ref) o;
            return source == other.source && handle.equals(other.handle);
        }

        @Override
        public int hashCode() {
            return 31 * source + handle.hashCode();
        }
    }

    private static final class Result<T> {
        T value;
        ResultState state = ResultState.ALIVE;
        Set<Ref> constituents = new LinkedHashSet<>();
    }

    private final MultiTopicImpl<TYPE> multiTopic;
    private final MultiTopicPlan plan;
    private final SubscriberImpl subscriber;
    private final TopicTypeSupport<TYPE> codec;
    private final List<Source> sources = new ArrayList<>();
    private final Map<Object, Result<TYPE>> results = new LinkedHashMap<>();
    private final Topic<TYPE> resultTopic;
    private final String resultTopicName;
    private final int domainId;
    private DataWriterImpl<TYPE> writer;
    private boolean started;
    private boolean closed;

    private final Deque<Runnable> pending = new ArrayDeque<>();
    private boolean draining;

    MultiTopicEngine(MultiTopicImpl<TYPE> multiTopic, MultiTopicPlan plan,
            SubscriberImpl subscriber, int domainId, TopicTypeSupport<TYPE> codec,
            DomainParticipant participant, ServiceEnvironment environment) {
        this.multiTopic = multiTopic;
        this.plan = plan;
        this.subscriber = subscriber;
        this.domainId = domainId;
        this.codec = codec;
        this.resultTopicName = INTERNAL_PREFIX + multiTopic.getName() + "#"
                + SEQUENCE.incrementAndGet();
        this.resultTopic = new TopicImpl<>(environment, participant,
                resultTopicName, multiTopic.getTypeSupport(),
                participant.getDefaultTopicQos());
    }

    /** The topic the result reader reads. */
    public Topic<TYPE> resultTopic() {
        return resultTopic;
    }

    /**
     * Opens the result writer and the readers of the source topics. Called
     * after the result reader exists, so nothing the join writes is missed.
     */
    public void start(DataReaderQos readerQos, Set<String> partitions) {
        // The engine disposes a result itself when a constituent is disposed and unregisters it
        // when none is alive; an unregister that disposed first would show NOT_ALIVE_DISPOSED
        // for a result whose constituents merely lost their writers (DDS 1.4 2.2.3.21).
        DataWriterQos writerQos = offeredQosFor(readerQos).withPolicy(
                multiTopic.getEnvironment().getSPI().getPolicyFactory().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false));
        writer = new DataWriterImpl<>(domainId, null, resultTopic, codec,
                writerQos, partitions);
        writer.enable();
        List<Topic<?>> topics = multiTopic.getSources();
        for (int i = 0; i < topics.size(); i++) {
            Source source = new Source(i, topics.get(i), plan.sourceMembers.get(i));
            sources.add(source);
            source.reader = openSource(source, readerQos);
        }
        // A sample that arrived while the later sources were still being
        // opened is in its reader; it is joined now that every source is known.
        synchronized (this) {
            started = true;
        }
        for (Source source : new ArrayList<>(sources)) {
            received(source);
        }
    }

    /**
     * The QoS the internal writer offers: what the result reader requests
     * wherever the policy applies to a writer, so the request-offered
     * check of §2.2.3 never refuses the join's own samples.
     */
    private DataWriterQos offeredQosFor(DataReaderQos readerQos) {
        ServiceEnvironment env = multiTopic.getEnvironment();
        DataWriterQos merged = DataWriterQosImpl.defaults(env);
        for (Map.Entry<Class<? extends QosPolicy>, ? extends QosPolicy> entry
                : QosCompatibility.policiesOf(readerQos).entrySet()) {
            if (entry.getValue() instanceof QosPolicy.ForDataWriter) {
                merged = merged.withPolicy((QosPolicy.ForDataWriter) entry.getValue());
            }
        }
        return merged;
    }

    @SuppressWarnings("unchecked")
    private <S> DataReaderImpl<S> openSource(Source source, DataReaderQos readerQos) {
        Topic<S> topic = (Topic<S>) source.topic;
        DataReaderImpl<S> reader = subscriber.createInternalReader(topic, readerQos);
        Set<Class<? extends Status>> statuses = new LinkedHashSet<>();
        statuses.add(DataAvailableStatus.class);
        reader.setListener(new DataReaderAdapter<S>() {
            @Override
            public void onDataAvailable(DataAvailableEvent<S> status) {
                received(source);
            }
        }, statuses);
        return reader;
    }

    /** Stops the join and closes what it opened. */
    public void close() {
        List<DataReaderImpl<?>> readers = new ArrayList<>();
        DataWriterImpl<TYPE> toClose;
        synchronized (this) {
            if (closed) {
                return;
            }
            closed = true;
            for (Source source : sources) {
                if (source.reader != null) {
                    readers.add(source.reader);
                }
            }
            toClose = writer;
        }
        for (DataReaderImpl<?> reader : readers) {
            subscriber.closeInternalReader(reader);
        }
        if (toClose != null) {
            toClose.close();
        }
    }

    // --- Receiving ----------------------------------------------------------

    private void received(Source source) {
        synchronized (this) {
            if (closed || !started) {
                return;
            }
            // Taking the samples and updating the join state happen under one
            // monitor, so the samples of a source are applied in the order
            // they were taken. The writes are queued and run below with no
            // monitor held (DDS-Java-PSM 1.0 §7.2.4).
            for (Sample<?> sample : takeAll(source.reader)) {
                apply(source, sample);
            }
        }
        drain();
    }

    private static <S> List<Sample<S>> takeAll(DataReaderImpl<S> reader) {
        return reader.take(new ArrayList<Sample<S>>());
    }

    private void apply(Source source, Sample<?> sample) {
        InstanceHandle handle = sample.getInstanceHandle();
        Object data = sample.getData();
        Ref ref = new Ref(source.index, handle);
        if (data != null) {
            Entry entry = new Entry(rowOf(source.members, data), data, handle,
                    timestampOf(sample));
            source.entries.put(handle, entry);
            join(source.index, entry);
            return;
        }
        InstanceState state = sample.getInstanceState();
        if (state == InstanceState.NOT_ALIVE_DISPOSED) {
            source.entries.remove(handle);
            for (Result<TYPE> result : results.values()) {
                if (result.constituents.contains(ref)
                        && result.state != ResultState.DISPOSED) {
                    result.state = ResultState.DISPOSED;
                    TYPE value = result.value;
                    long at = timestampOf(sample);
                    enqueue(() -> writer.dispose(value, at));
                }
            }
        } else if (state == InstanceState.NOT_ALIVE_NO_WRITERS) {
            Entry entry = source.entries.get(handle);
            if (entry != null) {
                entry.liveness = Liveness.NO_WRITERS;
            }
            for (Result<TYPE> result : results.values()) {
                if (result.constituents.contains(ref)
                        && result.state == ResultState.ALIVE) {
                    result.state = ResultState.NO_WRITERS;
                    TYPE value = result.value;
                    long at = timestampOf(sample);
                    enqueue(() -> writer.unregisterInstance(value, at));
                }
            }
        }
    }

    private static long timestampOf(Sample<?> sample) {
        return sample.getSourceTimestamp() == null
                ? System.currentTimeMillis()
                : sample.getSourceTimestamp().getTime(TimeUnit.MILLISECONDS);
    }

    // --- The join -----------------------------------------------------------

    private static Map<String, Object> rowOf(List<MemberRef> members, Object sample) {
        Map<String, Object> row = new LinkedHashMap<>();
        for (MemberRef member : members) {
            row.put(member.getName(), member.get(sample));
        }
        return row;
    }

    /** Every tuple of current samples that contains {@code fixed} and joins. */
    private void join(int fixedIndex, Entry fixed) {
        Entry[] chosen = new Entry[sources.size()];
        joinFrom(0, fixedIndex, fixed, new LinkedHashMap<>(), chosen);
    }

    private void joinFrom(int position, int fixedIndex, Entry fixed,
            Map<String, Object> merged, Entry[] chosen) {
        if (position == sources.size()) {
            produce(merged, chosen);
            return;
        }
        Collection<Entry> candidates = position == fixedIndex
                ? Collections.singletonList(fixed)
                : new ArrayList<>(sources.get(position).entries.values());
        for (Entry candidate : candidates) {
            Map<String, Object> next = merge(merged, candidate.row);
            if (next == null) {
                continue;
            }
            chosen[position] = candidate;
            joinFrom(position + 1, fixedIndex, fixed, next, chosen);
        }
        chosen[position] = null;
    }

    /** {@code merged} extended by {@code row}, or {@code null} if the join columns differ. */
    private Map<String, Object> merge(Map<String, Object> merged, Map<String, Object> row) {
        Map<String, Object> out = new LinkedHashMap<>(merged);
        for (Map.Entry<String, Object> e : row.entrySet()) {
            if (out.containsKey(e.getKey())) {
                if (plan.joinColumns.contains(e.getKey())
                        && !Objects.deepEquals(out.get(e.getKey()), e.getValue())) {
                    return null;
                }
            } else {
                out.put(e.getKey(), e.getValue());
            }
        }
        return out;
    }

    private void produce(Map<String, Object> joined, Entry[] chosen) {
        Map<String, Object> outputs = new LinkedHashMap<>();
        Map<String, Object> whereRow = new LinkedHashMap<>(joined);
        for (MultiTopicPlan.Output o : plan.outputs) {
            Object value = navigate(joined.get(o.path.get(0)), o.path);
            outputs.put(o.name,
                    MultiTopicPlan.convert(value, plan.resultMembers.get(o.name)));
            if (o.aliased || !whereRow.containsKey(o.name)) {
                whereRow.put(o.name, value);
            }
        }
        Predicate<Object> where = multiTopic.rowFilter();
        if (!where.test(whereRow)) {
            return;
        }
        @SuppressWarnings("unchecked")
        TYPE value = (TYPE) ReflectionTypeSupport.assemble(plan.resultClass, outputs);
        Object key = codec.isKeyed()
                ? ByteBuffer.wrap(codec.keyHash(value)) : Boolean.TRUE;
        Result<TYPE> result = results.get(key);
        if (result == null) {
            result = new Result<>();
            results.put(key, result);
        }
        result.value = value;
        result.state = ResultState.ALIVE;
        result.constituents = new LinkedHashSet<>();
        long timestamp = 0;
        boolean lacksWriters = false;
        for (int i = 0; i < chosen.length; i++) {
            Entry entry = chosen[i];
            result.constituents.add(new Ref(i, entry.handle));
            timestamp = Math.max(timestamp, entry.timestampMillis);
            lacksWriters |= entry.liveness == Liveness.NO_WRITERS;
        }
        long at = timestamp;
        enqueue(() -> {
            try {
                writer.write(value, at);
            } catch (java.util.concurrent.TimeoutException e) {
                // The result writer has the default RESOURCE_LIMITS, which never block.
                throw new org.zerodds.core.OutOfResourcesException(e.getMessage());
            }
        });
        if (lacksWriters) {
            // §2.2.2.3.4: ALIVE only if all constituting instances are.
            result.state = ResultState.NO_WRITERS;
            enqueue(() -> writer.unregisterInstance(value, at));
        }
    }

    /** The value at {@code path} below the already-read first element. */
    private static Object navigate(Object head, List<String> path) {
        Object current = head;
        for (int i = 1; i < path.size() && current != null; i++) {
            Object found = null;
            for (MemberRef member : ReflectionTypeSupport.membersOf(current.getClass())) {
                if (member.getName().equals(path.get(i))) {
                    found = member.get(current);
                    break;
                }
            }
            current = found;
        }
        return current;
    }

    // --- Writing without a monitor held ------------------------------------

    private synchronized void enqueue(Runnable action) {
        pending.addLast(action);
    }

    /**
     * Runs the queued writes in order, on whichever thread gets here first;
     * a thread that finds the queue being drained leaves its actions to it.
     * No monitor is held while a write runs, because a write delivers to the
     * result readers and so to application listeners.
     */
    private void drain() {
        synchronized (this) {
            if (draining) {
                return;
            }
            draining = true;
        }
        while (true) {
            Runnable action;
            synchronized (this) {
                action = pending.pollFirst();
                if (action == null) {
                    draining = false;
                    return;
                }
                if (closed) {
                    pending.clear();
                    draining = false;
                    return;
                }
            }
            action.run();
        }
    }

    @Override
    public String toString() {
        return "MultiTopicEngine[" + resultTopicName + "]";
    }
}
