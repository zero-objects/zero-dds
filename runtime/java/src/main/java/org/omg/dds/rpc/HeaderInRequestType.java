// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * A request type that carries its own header — DDS-RPC 1.0 §7.3.1.4.
 *
 * <p>"When using the Basic Service Mapping, the request type shall have a
 * data member named {@code header} of type {@code dds::rpc::RequestHeader}"
 * (§7.3.1.4). The {@link TypeSupport} of such a type implements this
 * interface; the {@link Requester} then puts the header into the value
 * before it serialises it, and the {@link Replier} reads the header from
 * the value it deserialised, instead of writing and reading the header in
 * front of the payload as it does for the topic types of an interface.
 * The octets of the sample are the octets of the structure, and the header
 * is its first member.
 *
 * @param <T> the request type
 */
public interface HeaderInRequestType<T> {

    /**
     * Sets the {@code header} member of {@code value}. The {@link Requester}
     * calls this on the request it is given, so the request holds the
     * identity it was sent with afterwards.
     */
    void setRequestHeader(T value, RequestHeader header);

    /** The {@code header} member of {@code value}. */
    RequestHeader requestHeader(T value);
}
