// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.event;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.event.DataOnReadersEvent;
import org.omg.dds.core.status.DataOnReadersStatus;
import org.omg.dds.sub.Subscriber;

/**
 * ZeroDDS {@link DataOnReadersEvent} (DDS-Java-PSM 1.0 §7.3.7.2): the event
 * object a subscriber listener receives when one of its readers has data.
 * {@link #getStatus()} names that reader (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: the status is fixed at construction.
 */
public final class DataOnReadersEventImpl extends DataOnReadersEvent {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final transient Subscriber source;
    private final DataOnReadersStatus status;

    public DataOnReadersEventImpl(ServiceEnvironment environment,
            Subscriber source, DataOnReadersStatus status) {
        super(source);
        this.environment = environment;
        this.source = source;
        this.status = status;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Subscriber getSource() {
        return source;
    }

    @Override
    public DataOnReadersStatus getStatus() {
        return status;
    }

    @Override
    public DataOnReadersEvent clone() {
        return new DataOnReadersEventImpl(environment, source, status);
    }

    @Override
    public String toString() {
        return "DataOnReadersEvent" + status;
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }
}