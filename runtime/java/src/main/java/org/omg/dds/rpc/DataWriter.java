// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * A writer of serialised samples on one topic.
 *
 * <p>The Java half of the seam §7.11.1.4.23 names for C++: the
 * request-reply binding is defined "irrespective of the DDS C++ language
 * bindings", and the same has to hold here or the binding would be
 * written twice, once per DDS Java implementation.
 *
 * <p>The operations with a default body are the ones a binding may not be
 * able to offer; the default throws {@link RemoteUnsupportedError} naming
 * the operation, which is the result the caller needs to see rather than a
 * behaviour that quietly differs.
 */
public interface DataWriter {

    /** Write one sample and return the identity the middleware gave it. */
    SampleIdentity write(byte[] sample);

    /**
     * Write one sample with the identity of the request it answers in the
     * DATA's inline QoS — DDS-RPC 1.0 §7.8.2: "The PID_RELATED_SAMPLE_IDENTITY
     * shall be used for data samples sent as reply to a request", and §7.8.2.3:
     * "the related sample identity is the same as the identity of the request
     * sample".
     *
     * @param sample the sample body
     * @param related the identity of the request, as the reader took it
     * @return the identity of the sample written
     * @throws RemoteUnsupportedError when the binding cannot put a parameter
     *     into the inline QoS
     */
    default SampleIdentity write(byte[] sample, SampleIdentity related) {
        throw new RemoteUnsupportedError(
                "this DataWriter binding cannot send PID_RELATED_SAMPLE_IDENTITY (§7.8.2)");
    }

    /** Whether {@link #write(byte[], SampleIdentity)} works on this binding. */
    default boolean carriesRelatedIdentity() {
        return false;
    }

    /**
     * The identity {@link #write} will stamp on the next sample, without
     * consuming it.
     *
     * <p>A request header carries the identity of the sample it is part
     * of (DDS-RPC 1.0 §7.5.1.1.1 {@code requestId}, §7.8.1), and the
     * header must be filled before the sample is written, so the
     * requester asks the writer for the identity it is about to stamp.
     * A binding that cannot predict its writer's identity cannot satisfy
     * the Basic mapping; this is abstract rather than defaulting to
     * {@link SampleIdentity#UNKNOWN}, which would correlate every reply
     * to the invalid identity.
     */
    SampleIdentity nextIdentity();

    /** How many readers this writer is matched with. */
    int matchedCount();

    /**
     * How many matched readers announce {@code serviceInstanceName} as
     * their {@code service_instance_name} (DDS-RPC 1.0 §7.6.2.1.2).
     */
    default int matchedCount(String serviceInstanceName) {
        throw new RemoteUnsupportedError(
                "this DataWriter binding does not see service_instance_name (§7.6.2.1)");
    }

    /**
     * Write one sample to the matched readers that announce
     * {@code serviceInstanceName} only — §7.11.1.4.3: "Requests sent
     * through a bound requester reference shall be sent to the bound
     * service instance only."
     */
    default SampleIdentity writeTo(byte[] sample, String serviceInstanceName) {
        throw new RemoteUnsupportedError(
                "this DataWriter binding cannot address one service instance (§7.11.1.4.3)");
    }

    /**
     * Change the mutable policies — §7.10.2: "The users may modify the
     * mutable QoS policies at run-time."
     *
     * @throws IllegalArgumentException when a policy that is not mutable
     *     would change
     */
    default void setQos(RpcQos qos) {
        throw new RemoteUnsupportedError("this DataWriter binding cannot change its QoS (§7.10.2)");
    }

    /**
     * The GUID of this writer, the first 16 octets of the identity of every sample it writes
     * (§7.5.1.1.1).
     */
    default byte[] guid() {
        return nextIdentity().writerGuid();
    }

    /**
     * The service instances the matched readers announced (§7.6.2.1.2), without the empty name.
     * A binding that sees no {@code service_instance_name} has none to list.
     */
    default java.util.Collection<String> matchedInstances() {
        throw new RemoteUnsupportedError(
                "this DataWriter binding does not see service_instance_name (§7.6.2.1)");
    }

    /**
     * Announces the {@code topic_aliases} (§7.4.3, PID 0x0082) and the related endpoint
     * ({@code related_datareader_key}, PID 0x0081, §7.6.2.1.2) of this writer in discovery,
     * next to the service instance name it was created with.
     *
     * @param topicAliases the alternative names of the topic, possibly empty
     * @param relatedEntityGuid the GUID of the reader on the other topic of the pair, or
     *     {@code null}
     * @return {@code false} when the binding has no discovery to announce them in (a loop
     *     inside one process)
     */
    default boolean announce(java.util.List<String> topicAliases, byte[] relatedEntityGuid) {
        return false;
    }

    /**
     * The matched readers with what they announced in discovery (DDS-RPC 1.0 §7.6.2.1.2), or
     * {@code null} when the binding sees no discovery data (a loop inside one process): the
     * runtime then matches by count.
     */
    default java.util.List<RpcPeerInfo> matchedPeers() {
        return null;
    }

    /**
     * The DataWriter of the DDS Java PSM this writer is, typed by the topic type (§7.11.2);
     * {@code null} for a binding that is not the PSM or a writer of octets only.
     */
    default org.omg.dds.pub.DataWriter<?> psm() {
        return null;
    }
}
