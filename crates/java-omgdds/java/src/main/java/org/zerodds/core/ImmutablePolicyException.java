// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.ServiceEnvironment;

/**
 * ZeroDDS implementation of {@link ImmutablePolicyException} — RETCODE_IMMUTABLE_POLICY
 * (DDS-Java-PSM 1.0 §7.3.2, Table 7.1).
 *
 * <p>Like every {@code DDSException} it is a {@link org.omg.dds.core.DDSObject}:
 * {@link #getEnvironment()} returns the environment given at construction, or
 * the default environment of this implementation when none was given.
 */
public final class ImmutablePolicyException extends org.omg.dds.core.ImmutablePolicyException {

    private static final long serialVersionUID = 3L;

    private final transient ServiceEnvironment environment;

    public ImmutablePolicyException() {
        super();
        this.environment = null;
    }

    public ImmutablePolicyException(String message) {
        super(message);
        this.environment = null;
    }

    public ImmutablePolicyException(String message, Throwable cause) {
        super(message, cause);
        this.environment = null;
    }

    public ImmutablePolicyException(ServiceEnvironment environment, String message) {
        super(message);
        this.environment = environment;
    }

    public ImmutablePolicyException(ServiceEnvironment environment, String message, Throwable cause) {
        super(message, cause);
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment != null
                ? environment : org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}
