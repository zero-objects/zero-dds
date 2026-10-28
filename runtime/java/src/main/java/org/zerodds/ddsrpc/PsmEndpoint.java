// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import java.util.concurrent.TimeUnit;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.rpc.RemoteUnsupportedError;
import org.omg.dds.rpc.RpcQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;

/**
 * What the params of DDS-RPC 1.0 §7.11.2 say about one DDS entity of an RPC endpoint in the
 * terms of the DDS Java PSM: the topic type it is typed by ({@link TopicSample}), the
 * Publisher or Subscriber it is created in, and its QoS ({@code withDataWriterQos},
 * {@code withDataReaderQos}, {@code withPublisher}, {@code withSubscriber}). Every part may be
 * absent; a binding over the PSM ({@link ZeroDdsParticipant}) then creates the entity as it
 * would without them.
 *
 * @param <T> the topic type
 */
public final class PsmEndpoint<T> {

    private final TopicSample<T> sample;
    private final Publisher publisher;
    private final Subscriber subscriber;
    private final DataWriterQos writerQos;
    private final DataReaderQos readerQos;

    private PsmEndpoint(TopicSample<T> sample, Publisher publisher, Subscriber subscriber,
            DataWriterQos writerQos, DataReaderQos readerQos) {
        this.sample = sample;
        this.publisher = publisher;
        this.subscriber = subscriber;
        this.writerQos = writerQos;
        this.readerQos = readerQos;
    }

    /** A DataWriter: typed by {@code sample}, in {@code publisher}, with {@code qos}. */
    public static <T> PsmEndpoint<T> writer(
            TopicSample<T> sample, Publisher publisher, DataWriterQos qos) {
        return new PsmEndpoint<>(sample, publisher, null, qos, null);
    }

    /** A DataReader: typed by {@code sample}, in {@code subscriber}, with {@code qos}. */
    public static <T> PsmEndpoint<T> reader(
            TopicSample<T> sample, Subscriber subscriber, DataReaderQos qos) {
        return new PsmEndpoint<>(sample, null, subscriber, null, qos);
    }

    /** The topic type and its framing, or {@code null}: the entity carries octets only. */
    public TopicSample<T> sample() {
        return sample;
    }

    public Publisher publisher() {
        return publisher;
    }

    public Subscriber subscriber() {
        return subscriber;
    }

    public DataWriterQos writerQos() {
        return writerQos;
    }

    public DataReaderQos readerQos() {
        return readerQos;
    }

    /**
     * Refuses an endpoint that names PSM entities or PSM QoS for a binding that is not the
     * PSM: it could only ignore them, and the application would not learn that it did.
     *
     * @throws RemoteUnsupportedError when {@code endpoint} names a Publisher, a Subscriber or
     *     a QoS
     */
    public static void refuseWithoutPsm(PsmEndpoint<?> endpoint) {
        if (endpoint != null
                && (endpoint.publisher != null || endpoint.subscriber != null
                        || endpoint.writerQos != null || endpoint.readerQos != null)) {
            throw new RemoteUnsupportedError(
                    "this DomainParticipant binding is not the DDS Java PSM: it creates no entity"
                            + " in a Publisher or Subscriber and takes no DataWriterQos or"
                            + " DataReaderQos (§7.11.2)");
        }
    }

    /**
     * The {@link RpcQos} of a writer created with {@code qos}: its reliability, durability,
     * history, deadline and lifespan, and the data representation of {@code base}.
     */
    public static RpcQos rpcQosOf(DataWriterQos qos, RpcQos base) {
        RpcQos out = common(base, qos.getReliability(), qos.getDurability(), qos.getHistory(),
                qos.getDeadline());
        org.omg.dds.core.Duration lifespan = qos.getLifespan().getDuration();
        return out.withLifespanNanos(nanosOrZero(lifespan));
    }

    /** As {@link #rpcQosOf(DataWriterQos, RpcQos)}, for a reader, which has no lifespan. */
    public static RpcQos rpcQosOf(DataReaderQos qos, RpcQos base) {
        // The DataReaderQos of DDS-Java-PSM 1.0 Annex B declares no getReliability(); the
        // policy is in the QoS all the same (EntityQos.get).
        return common(base, qos.get(org.omg.dds.core.policy.Reliability.class), qos.getDurability(),
                qos.getHistory(), qos.getDeadline()).withLifespanNanos(base.lifespanNanos());
    }

    private static RpcQos common(RpcQos base, org.omg.dds.core.policy.Reliability reliability,
            org.omg.dds.core.policy.Durability durability,
            org.omg.dds.core.policy.History history,
            org.omg.dds.core.policy.Deadline deadline) {
        return base
                .withReliability(reliability.getKind()
                                == org.omg.dds.core.policy.Reliability.Kind.RELIABLE
                        ? RpcQos.Reliability.RELIABLE : RpcQos.Reliability.BEST_EFFORT)
                .withDurability(RpcQos.Durability.valueOf(durability.getKind().name()))
                .withHistory(history.getKind() == org.omg.dds.core.policy.History.Kind.KEEP_ALL
                                ? RpcQos.HistoryKind.KEEP_ALL : RpcQos.HistoryKind.KEEP_LAST,
                        Math.max(1, history.getDepth()))
                .withDeadlineNanos(nanosOrZero(deadline.getPeriod()));
    }

    /** The nanoseconds of a period; {@code 0}, which RpcQos reads as none, for an infinite one. */
    private static long nanosOrZero(org.omg.dds.core.Duration duration) {
        return duration == null || duration.isInfinite() ? 0L
                : duration.getDuration(TimeUnit.NANOSECONDS);
    }
}
