// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.io.Closeable;

import org.omg.dds.core.DDSObject;
import org.omg.dds.core.ServiceEnvironment;

/**
 * DDS-RPC 1.0 §7.11.2.2.2: "RPCEntity is the base interface extended by
 * all the active entities. It inherits from the DDSObject interface
 * defined in [DDS-Java-PSM] and java.io.Closeable. Its purpose is to
 * provide quick access to the RPCRuntime object."
 *
 * <p>Both supertypes are named here. {@link org.omg.dds.core.DDSObject}
 * is [DDS-Java-PSM] §7.3.1's, and its one operation —
 * {@code getEnvironment} — is what "quick access to the RPCRuntime
 * object" comes out as in Java: §7.11.2.2.3 makes {@link RPCRuntime} a
 * {@link ServiceEnvironment}, so an entity a runtime made returns that
 * runtime from it.
 */
public interface RPCEntity extends DDSObject, Closeable {

    /**
     * The machine-readable {@code RPCEntity.java}'s "quick access to the
     * RPCRuntime object": the environment of the entity, which
     * {@link RPCRuntime} makes a {@link ServiceEnvironment}.
     *
     * @throws IllegalStateException when the entity was derived from none.
     */
    default RPCRuntime getRPCRuntime() {
        return (RPCRuntime) getEnvironment();
    }

    /** Release the underlying DDS entities. */
    @Override
    void close();

    /** Whether this reference holds nothing. */
    boolean isNull();

    /**
     * [DDS-Java-PSM] §7.3.1: "the ServiceEnvironment from which they are
     * ultimately derived."
     *
     * <p>{@link RPCRuntime#createRequester} and
     * {@link RPCRuntime#createReplier} put the runtime into the params
     * they build from, and {@link Requester} and {@link Replier} return
     * it. An entity that was built without one was derived from no
     * environment, and this says so rather than returning a {@code null}
     * the caller would dereference one frame later.
     *
     * @return the environment this entity was derived from.
     * @throws IllegalStateException when it was derived from none.
     */
    @Override
    default ServiceEnvironment getEnvironment() {
        throw new IllegalStateException(
                getClass().getName() + " was not derived from an RPCRuntime "
                        + "and has no ServiceEnvironment");
    }
}
