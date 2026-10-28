// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.event;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.event.SampleLostEvent;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.sub.DataReader;

/**
 * ZeroDDS {@link SampleLost} (DDS-Java-PSM 1.0 §7.3.7.2): the event object a
 * listener callback receives. It names the entity the change happened on and
 * carries the status of DDS 1.2 §7.1.4.1.
 *
 * <p>Immutable: the status is fixed at construction.
 */
public final class SampleLostEventImpl<TYPE> extends SampleLostEvent<TYPE> {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final transient org.omg.dds.sub.DataReader<TYPE> source;
    private final SampleLostStatus status;

    public SampleLostEventImpl(ServiceEnvironment environment, org.omg.dds.sub.DataReader<TYPE> source, SampleLostStatus status) {
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
    public org.omg.dds.sub.DataReader<TYPE> getSource() {
        return source;
    }

    @Override
    public SampleLostStatus getStatus() {
        return status;
    }

    @Override
    public SampleLostEvent<TYPE> clone() {
        return new SampleLostEventImpl<>(environment, source, status);
    }

    @Override
    public String toString() {
        return "SampleLost" + status;
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
