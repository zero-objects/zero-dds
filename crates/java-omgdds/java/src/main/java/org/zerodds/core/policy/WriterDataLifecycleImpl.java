// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.WriterDataLifecycle;

/**
 * ZeroDDS WriterDataLifecycle (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Whether unregistering an instance also disposes it (DDS 1.2 §7.1.3.7). Default: true.
 */
public final class WriterDataLifecycleImpl implements WriterDataLifecycle, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final boolean autDisposeUnregisteredInstances;

    public WriterDataLifecycleImpl(ServiceEnvironment environment,
            boolean autDisposeUnregisteredInstances) {
        this.environment = environment;
        this.autDisposeUnregisteredInstances = autDisposeUnregisteredInstances;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean isAutDisposeUnregisteredInstances() {
        return autDisposeUnregisteredInstances;
    }

    @Override
    public WriterDataLifecycle withAutDisposeUnregisteredInstances(boolean autDisposeUnregisteredInstances) {
        return new WriterDataLifecycleImpl(environment, autDisposeUnregisteredInstances);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof WriterDataLifecycle)) return false;
        WriterDataLifecycle other = (WriterDataLifecycle) o;
        return autDisposeUnregisteredInstances == other.isAutDisposeUnregisteredInstances();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(autDisposeUnregisteredInstances);
    }

    @Override
    public String toString() {
        return "WriterDataLifecycle[autodisposeUnregisteredInstances=" + autDisposeUnregisteredInstances + "]";
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
    public WriterDataLifecycleImpl clone() {
        try {
            return (WriterDataLifecycleImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
