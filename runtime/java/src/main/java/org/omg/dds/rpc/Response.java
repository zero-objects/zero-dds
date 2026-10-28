// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * What a service implementation answers one request with: the reply
 * topic sample and the remote exception code its header carries
 * (§7.5.2).
 *
 * @param <T> the reply topic type
 */
public final class Response<T> {

    private final T reply;
    private final RemoteExceptionCode code;

    private Response(T reply, RemoteExceptionCode code) {
        this.reply = reply;
        this.code = code;
    }

    /** A reply with a result. */
    public static <T> Response<T> ok(T reply) {
        return new Response<>(reply, RemoteExceptionCode.REMOTE_EX_OK);
    }

    /**
     * A reply that reports {@code code}; {@code reply} is the sample the
     * header travels with, since the topic type needs a body.
     */
    public static <T> Response<T> failed(T reply, RemoteExceptionCode code) {
        return new Response<>(reply, code);
    }

    public T reply() {
        return reply;
    }

    public RemoteExceptionCode code() {
        return code;
    }
}
