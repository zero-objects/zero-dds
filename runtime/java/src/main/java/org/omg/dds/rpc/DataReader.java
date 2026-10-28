// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.List;

/**
 * A reader of serialised samples on one topic. See {@link DataWriter},
 * which also explains the operations with a default body.
 */
public interface DataReader {

    /** Take every sample that has arrived, removing it from the reader. */
    List<TakenSample> take();

    /** How many writers this reader is matched with. */
    int matchedCount();

    /**
     * Register the function the reader calls when a sample has arrived —
     * the DDS {@code on_data_available} of DDS 1.4 §2.2.4.4, which
     * DDS-RPC 1.0 §7.9.2.2 and §7.11.1.4.7 to §7.11.1.4.10 need to call a
     * listener "when a request is available" and "when a reply is
     * available". {@code null} removes it.
     *
     * @param listener the function
     * @throws RemoteUnsupportedError when the binding does not notify
     */
    default void setDataAvailableListener(Runnable listener) {
        throw new RemoteUnsupportedError(
                "this DataReader binding cannot notify when a sample arrives (§7.9.2.2)");
    }

    /** The reader counterpart of {@link DataWriter#setQos}. */
    default void setQos(RpcQos qos) {
        throw new RemoteUnsupportedError("this DataReader binding cannot change its QoS (§7.10.2)");
    }

    /** The GUID of this reader, or {@code null} for a binding that gives its endpoints none. */
    default byte[] guid() {
        return null;
    }

    /**
     * Announces the {@code topic_aliases} (§7.4.3, PID 0x0082) and the related endpoint
     * ({@code related_datawriter_key}, PID 0x0081, §7.6.2.1.2) of this reader in discovery,
     * next to the service instance name it was created with.
     *
     * @param topicAliases the alternative names of the topic, possibly empty
     * @param relatedEntityGuid the GUID of the writer on the other topic of the pair, or
     *     {@code null}
     * @return {@code false} when the binding has no discovery to announce them in
     */
    default boolean announce(java.util.List<String> topicAliases, byte[] relatedEntityGuid) {
        return false;
    }

    /**
     * The matched writers with what they announced in discovery (DDS-RPC 1.0 §7.6.2.1.2), or
     * {@code null} when the binding sees no discovery data (a loop inside one process): the
     * runtime then matches by count.
     */
    default java.util.List<RpcPeerInfo> matchedPeers() {
        return null;
    }

    /**
     * The DataReader of the DDS Java PSM this reader is, typed by the topic type (§7.11.2);
     * {@code null} for a binding that is not the PSM or a reader of octets only.
     */
    default org.omg.dds.sub.DataReader<?> psm() {
        return null;
    }
}
