// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Durability;

/**
 * ZeroDDS Durability (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Whether data written before a reader joins survives that reader (DDS 1.2 §7.1.3.4). Requested/offered: the offered kind must be at least the requested kind. Default: VOLATILE.
 */
public final class DurabilityImpl implements Durability, RequestedOffered<Durability>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Durability.Kind kind;

    public DurabilityImpl(ServiceEnvironment environment,
            Durability.Kind kind) {
        this.environment = environment;
        this.kind = kind;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Durability.Kind getKind() {
        return kind;
    }

    @Override
    public Durability withKind(Durability.Kind kind) {
        return new DurabilityImpl(environment, kind);
    }

    @Override
    public Durability withVolatile() {
        return withKind(Kind.VOLATILE);
    }

    @Override
    public Durability withTransientLocal() {
        return withKind(Kind.TRANSIENT_LOCAL);
    }

    @Override
    public Durability withTransient() {
        return withKind(Kind.TRANSIENT);
    }

    @Override
    public Durability withPersitent() {
        return withKind(Kind.PERSISTENT);
    }

    @Override
    public int compareTo(Durability other) {
        return kind.compareTo(other.getKind());
    }

    @Override
    public Comparable<Durability> requestedOfferedContract() {
        return this;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Durability)) return false;
        Durability other = (Durability) o;
        return kind == other.getKind();
    }

    @Override
    public int hashCode() {
        return kind.hashCode();
    }

    @Override
    public String toString() {
        return "Durability[kind=" + kind + "]";
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
    public DurabilityImpl clone() {
        try {
            return (DurabilityImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
