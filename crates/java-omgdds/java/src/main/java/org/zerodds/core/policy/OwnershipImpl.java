// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Ownership;

/**
 * ZeroDDS Ownership (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Whether one or all writers may update an instance (DDS 1.2 §7.1.3.5). Requested/offered: the kinds must be equal (DDS 1.4 §2.2.3.9). Default: SHARED.
 */
public final class OwnershipImpl implements Ownership, RequestedOffered<Ownership>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Ownership.Kind kind;

    public OwnershipImpl(ServiceEnvironment environment,
            Ownership.Kind kind) {
        this.environment = environment;
        this.kind = kind;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Ownership.Kind getKind() {
        return kind;
    }

    @Override
    public Ownership withKind(Ownership.Kind kind) {
        return new OwnershipImpl(environment, kind);
    }

    @Override
    public Ownership withShared() {
        return withKind(Kind.SHARED);
    }

    @Override
    public Ownership withExclusive() {
        return withKind(Kind.EXCLUSIVE);
    }

    @Override
    public int compareTo(Ownership other) {
        return kind.compareTo(other.getKind());
    }

    /**
     * DDS 1.4 §2.2.3.9: the offered kind must equal the requested one, in
     * either direction. The returned object compares as 0 for an equal kind
     * and as 1 for a different one, so that "compares greater than 0" is the
     * incompatibility test of both sides.
     */
    @Override
    public Comparable<Ownership> requestedOfferedContract() {
        return other -> kind == other.getKind() ? 0 : 1;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Ownership)) return false;
        Ownership other = (Ownership) o;
        return kind == other.getKind();
    }

    @Override
    public int hashCode() {
        return kind.hashCode();
    }

    @Override
    public String toString() {
        return "Ownership[kind=" + kind + "]";
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
    public OwnershipImpl clone() {
        try {
            return (OwnershipImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
