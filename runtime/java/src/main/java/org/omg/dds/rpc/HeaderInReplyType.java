// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * A reply type that carries its own header — DDS-RPC 1.0 §7.3.1.4.
 *
 * <p>"When using the Basic Service Mapping, the reply type shall have a
 * data member named {@code header} of type {@code dds::rpc::ReplyHeader}"
 * (§7.3.1.4). See {@link HeaderInRequestType}; this is the same for the
 * {@link Replier}, which sets the header, and the {@link Requester}, which
 * reads it.
 *
 * @param <T> the reply type
 */
public interface HeaderInReplyType<T> {

    /** Sets the {@code header} member of {@code value}. */
    void setReplyHeader(T value, ReplyHeader header);

    /** The {@code header} member of {@code value}. */
    ReplyHeader replyHeader(T value);
}
