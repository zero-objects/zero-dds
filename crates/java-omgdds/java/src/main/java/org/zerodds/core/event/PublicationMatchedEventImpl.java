// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.event;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.pub.DataWriter;

/**
 * ZeroDDS {@link PublicationMatched} (DDS-Java-PSM 1.0 §7.3.7.2): the event object a
 * listener callback receives. It names the entity the change happened on and
 * carries the status of DDS 1.2 §7.1.4.1.
 *
 * <p>Immutable: the status is fixed at construction.
 */
public final class PublicationMatchedEventImpl<TYPE> extends PublicationMatchedEvent<TYPE> {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final transient org.omg.dds.pub.DataWriter<TYPE> source;
    private final PublicationMatchedStatus status;

    public PublicationMatchedEventImpl(ServiceEnvironment environment, org.omg.dds.pub.DataWriter<TYPE> source, PublicationMatchedStatus status) {
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
    public org.omg.dds.pub.DataWriter<TYPE> getSource() {
        return source;
    }

    @Override
    public PublicationMatchedStatus getStatus() {
        return status;
    }

    @Override
    public PublicationMatchedEvent<TYPE> clone() {
        return new PublicationMatchedEventImpl<>(environment, source, status);
    }

    @Override
    public String toString() {
        return "PublicationMatched" + status;
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
