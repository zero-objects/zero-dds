// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.ServiceEnvironment;

/**
 * ZeroDDS implementation of {@link org.omg.dds.core.DDSException} itself — RETCODE_ERROR
 * (DDS-Java-PSM 1.0 §7.3.2, Table 7.1): the operation failed for a reason that
 * none of the other return codes names. {@code DDSException} is abstract in the
 * OMG API, so this class is what the service throws for that code.
 *
 * <p>Like every {@code DDSException} it is a {@link org.omg.dds.core.DDSObject}:
 * {@link #getEnvironment()} returns the environment given at construction, or
 * the default environment of this implementation when none was given.
 */
public final class ErrorException extends org.omg.dds.core.DDSException {

    private static final long serialVersionUID = 8L;

    private final transient ServiceEnvironment environment;

    public ErrorException() {
        super();
        this.environment = null;
    }

    public ErrorException(String message) {
        super(message);
        this.environment = null;
    }

    public ErrorException(String message, Throwable cause) {
        super(message, cause);
        this.environment = null;
    }

    public ErrorException(ServiceEnvironment environment, String message) {
        super(message);
        this.environment = environment;
    }

    public ErrorException(ServiceEnvironment environment, String message, Throwable cause) {
        super(message, cause);
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment != null
                ? environment : org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}
