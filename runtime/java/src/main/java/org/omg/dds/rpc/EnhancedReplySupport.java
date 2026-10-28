// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The codec of an Enhanced Service Mapping reply type — DDS-RPC 1.0
 * §7.5.1.2.5.
 *
 * <p>The Enhanced reply has no header. Its {@code remoteEx} member carries
 * the remote exception code (rule 5), which only the generated structure
 * can read and write, so a type support used with
 * {@link ServiceMapping#ENHANCED} offers the two operations the runtime
 * needs from it.
 *
 * @param <T> the reply topic type
 */
public interface EnhancedReplySupport<T> extends TypeSupport<T> {

    /**
     * The code in the {@code remoteEx} member of {@code reply}, or
     * {@link RemoteExceptionCode#REMOTE_EX_OK} when the reply carries a
     * result.
     */
    RemoteExceptionCode remoteExceptionOf(T reply);

    /** A reply that carries {@code code} in {@code remoteEx} and no result. */
    T remoteExceptionSample(RemoteExceptionCode code);
}
