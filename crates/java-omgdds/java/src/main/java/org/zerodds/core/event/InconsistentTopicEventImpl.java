// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.event;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.event.InconsistentTopicEvent;
import org.omg.dds.core.status.InconsistentTopicStatus;
import org.omg.dds.topic.Topic;

/**
 * ZeroDDS {@link InconsistentTopicEvent} (DDS-Java-PSM 1.0 §7.3.7.2): the
 * event object a listener callback receives. It names the topic whose
 * offered QoS was incompatible and carries INCONSISTENT_TOPIC
 * (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: the status is fixed at construction.
 */
public final class InconsistentTopicEventImpl<TYPE>
extends InconsistentTopicEvent<TYPE> {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final transient Topic<TYPE> source;
    private final InconsistentTopicStatus status;

    public InconsistentTopicEventImpl(ServiceEnvironment environment,
            Topic<TYPE> source, InconsistentTopicStatus status) {
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
    public Topic<TYPE> getSource() {
        return source;
    }

    @Override
    public InconsistentTopicStatus getStatus() {
        return status;
    }

    @Override
    public InconsistentTopicEvent<TYPE> clone() {
        return new InconsistentTopicEventImpl<>(environment, source, status);
    }

    @Override
    public String toString() {
        return "InconsistentTopicEvent" + status;
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