// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.5.5 through §7.11.2.3: "A ClientEndpoint provides
 * functions to obtain the underlying DDS entities at the client side.
 * ClientEndpoint inherits from ServiceProxy. A ClientEndpoint shall not
 * be instantiated directly; it can be obtained from a Client object."
 *
 * <p>{@link #requestWriter()} and {@link #replyReader()} are the seam of this
 * binding ({@link DomainParticipant}): they carry the octets of §7.5.1.1.1's
 * framing. {@link #getRequestDataWriter(Class)} and
 * {@link #getReplyDataReader(Class)} are the same entities as the DDS Java PSM
 * has them, typed by the topic type.
 */
public interface ClientEndpoint extends ServiceProxy {

    /**
     * The writer the requests of this client are written with.
     *
     * @throws IllegalStateException when the client was closed
     */
    DataWriter requestWriter();

    /**
     * The reader the replies to this client are read with.
     *
     * @throws IllegalStateException when the client was closed
     */
    DataReader replyReader();

    /**
     * §7.11.2: the request DataWriter of the DDS Java PSM whose topic type is
     * {@code requestType}: the client's own, or that of a base interface whose operations
     * travel on the base's topics (§7.5.1.1.8). {@code null} when the participant is not one
     * of the PSM.
     *
     * @throws IllegalArgumentException when no request topic of the client has that type
     */
    <TReq> org.omg.dds.pub.DataWriter<TReq> getRequestDataWriter(Class<?> requestType);

    /** §7.11.2: the reply DataReader whose topic type is {@code replyType}; as above. */
    <TRep> org.omg.dds.sub.DataReader<TRep> getReplyDataReader(Class<?> replyType);

    /** The params the client was made with. */
    default ClientParams getClientParams() {
        throw new UnsupportedOperationException(
                getClass().getName() + " was not made from ClientParams");
    }
}
