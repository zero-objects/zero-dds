// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.DestinationOrder;

/**
 * ZeroDDS DestinationOrder (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The order in which samples of one instance are handed to the application (DDS 1.2 §7.1.3.8). Default: BY_RECEPTION_TIMESTAMP.
 */
public final class DestinationOrderImpl implements DestinationOrder, RequestedOffered<DestinationOrder>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final DestinationOrder.Kind kind;

    public DestinationOrderImpl(ServiceEnvironment environment,
            DestinationOrder.Kind kind) {
        this.environment = environment;
        this.kind = kind;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public DestinationOrder.Kind getKind() {
        return kind;
    }

    @Override
    public DestinationOrder withKind(DestinationOrder.Kind kind) {
        return new DestinationOrderImpl(environment, kind);
    }

    @Override
    public DestinationOrder withReceptionTimestamp() {
        return withKind(Kind.BY_RECEPTION_TIMESTAMP);
    }

    @Override
    public DestinationOrder withSourceTimestamp() {
        return withKind(Kind.BY_SOURCE_TIMESTAMP);
    }

    @Override
    public int compareTo(DestinationOrder other) {
        return kind.compareTo(other.getKind());
    }

    @Override
    public Comparable<DestinationOrder> requestedOfferedContract() {
        return this;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof DestinationOrder)) return false;
        DestinationOrder other = (DestinationOrder) o;
        return kind == other.getKind();
    }

    @Override
    public int hashCode() {
        return kind.hashCode();
    }

    @Override
    public String toString() {
        return "DestinationOrder[kind=" + kind + "]";
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

    /** A copy by value (DDS-Java-PSM 1.0 7.3.3). */
    @Override
    public DestinationOrderImpl clone() {
        try {
            return (DestinationOrderImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
