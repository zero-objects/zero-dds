// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.5.2 through §7.11.2.3: "A ServiceEndpoint provides
 * type-independent functions to manage a service (e.g., pause, resume,
 * close, etc.) A ServiceEndpoint shall not be instantiated directly; it
 * can be obtained from a Service object."
 */
public interface ServiceEndpoint extends RPCEntity {

    void pause();

    void resume();

    boolean isPaused();

    /** The state of the service: closed, paused or running. */
    default ServiceStatus getServiceStatus() {
        if (isNull()) {
            return ServiceStatus.CLOSED;
        }
        return isPaused() ? ServiceStatus.PAUSED : ServiceStatus.RUNNING;
    }

    /**
     * §7.11.2: the request DataReader of the DDS Java PSM whose topic type is
     * {@code requestType}: the service's own, or that of a base interface whose operations
     * arrive on the base's topics (§7.5.1.1.8). {@code null} when the participant is not one
     * of the PSM.
     *
     * @throws IllegalArgumentException when no request topic of the service has that type
     */
    <TReq> org.omg.dds.sub.DataReader<TReq> getRequestDataReader(Class<?> requestType);

    /** §7.11.2: the reply DataWriter whose topic type is {@code replyType}; as above. */
    <TRep> org.omg.dds.pub.DataWriter<TRep> getReplyDataWriter(Class<?> replyType);

    /** The params the service was made with. */
    default ServiceParams getServiceParams() {
        throw new UnsupportedOperationException(
                getClass().getName() + " was not made from ServiceParams");
    }
}
