// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;

/**
 * ZeroDDS TypeConsistencyEnforcement (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>How far a reader tolerates a type that differs from the one it was created with (DDS-XTypes 1.0). Default: EXACT_TYPE_TYPE_CONSISTENCY.
 */
public final class TypeConsistencyEnforcementImpl implements TypeConsistencyEnforcement, RequestedOffered<TypeConsistencyEnforcement>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final TypeConsistencyEnforcement.Kind kind;

    public TypeConsistencyEnforcementImpl(ServiceEnvironment environment,
            TypeConsistencyEnforcement.Kind kind) {
        this.environment = environment;
        this.kind = kind;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public TypeConsistencyEnforcement.Kind getKind() {
        return kind;
    }

    @Override
    public TypeConsistencyEnforcement withKind(TypeConsistencyEnforcement.Kind kind) {
        return new TypeConsistencyEnforcementImpl(environment, kind);
    }

    @Override
    public TypeConsistencyEnforcement withExactTypeTypeConsistency() {
        return withKind(Kind.EXACT_TYPE_TYPE_CONSISTENCY);
    }

    @Override
    public TypeConsistencyEnforcement withExactNameTypeConsistency() {
        return withKind(Kind.EXACT_NAME_TYPE_CONSISTENCY);
    }

    @Override
    public TypeConsistencyEnforcement withDeclaredTypeConsistency() {
        return withKind(Kind.DECLARED_TYPE_CONSISTENCY);
    }

    @Override
    public TypeConsistencyEnforcement withAssignableTypeConsistency() {
        return withKind(Kind.ASSIGNABLE_TYPE_CONSISTENCY);
    }

    @Override
    public int compareTo(TypeConsistencyEnforcement other) {
        return kind.compareTo(other.getKind());
    }

    @Override
    public Comparable<TypeConsistencyEnforcement> requestedOfferedContract() {
        return this;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TypeConsistencyEnforcement)) return false;
        TypeConsistencyEnforcement other = (TypeConsistencyEnforcement) o;
        return kind == other.getKind();
    }

    @Override
    public int hashCode() {
        return kind.hashCode();
    }

    @Override
    public String toString() {
        return "TypeConsistencyEnforcement[kind=" + kind + "]";
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
    public TypeConsistencyEnforcementImpl clone() {
        try {
            return (TypeConsistencyEnforcementImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
