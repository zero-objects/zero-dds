// SPDX-License-Identifier: Apache-2.0
package org.zerodds;

import org.omg.dds.core.ServiceEnvironment;

/**
 * ZeroDDS concrete {@link ServiceEnvironment} (DDS-Java-PSM 1.0 §7.3.1).
 *
 * <p>This is the class an application names when bootstrapping the ZeroDDS
 * pure-Java service:
 *
 * <pre>{@code
 * var env = ServiceEnvironment.createInstance("org.zerodds.ServiceEnvironmentImpl");
 * var factory = DomainParticipantFactory.getInstance(env);
 * }</pre>
 *
 * <p>ZeroDDS is pure Java — there is no native library to load, so the
 * environment is a lightweight root marker. It must be {@code public} with a
 * {@code public} no-argument constructor so that
 * {@link ServiceEnvironment#createInstance(String)} can instantiate it
 * reflectively.
 */
public final class ServiceEnvironmentImpl extends ServiceEnvironment {

    /** Reflectively invoked by {@link ServiceEnvironment#createInstance(String)}. */
    public ServiceEnvironmentImpl() {
        super();
    }
}
