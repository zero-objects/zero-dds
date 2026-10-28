// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.ServiceEnvironment;

/**
 * ZeroDDS implementation of {@link AlreadyClosedException} — RETCODE_ALREADY_DELETED
 * (DDS-Java-PSM 1.0 §7.3.2, Table 7.1).
 *
 * <p>Like every {@code DDSException} it is a {@link org.omg.dds.core.DDSObject}:
 * {@link #getEnvironment()} returns the environment given at construction, or
 * the default environment of this implementation when none was given.
 */
public final class AlreadyClosedException extends org.omg.dds.core.AlreadyClosedException {

    private static final long serialVersionUID = 1L;

    private final transient ServiceEnvironment environment;

    public AlreadyClosedException() {
        super();
        this.environment = null;
    }

    public AlreadyClosedException(String message) {
        super(message);
        this.environment = null;
    }

    public AlreadyClosedException(String message, Throwable cause) {
        super(message, cause);
        this.environment = null;
    }

    public AlreadyClosedException(ServiceEnvironment environment, String message) {
        super(message);
        this.environment = environment;
    }

    public AlreadyClosedException(ServiceEnvironment environment, String message, Throwable cause) {
        super(message, cause);
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment != null
                ? environment : org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}
