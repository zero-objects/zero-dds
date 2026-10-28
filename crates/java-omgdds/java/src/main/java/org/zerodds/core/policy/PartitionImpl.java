// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.Partition;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * ZeroDDS Partition (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The partition a publisher or subscriber belongs to (DDS 1.2 §7.1.3.9). Default: the single empty name, which matches every other partition.
 */
public final class PartitionImpl implements Partition, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final LinkedHashSet<String> name;

    public PartitionImpl(ServiceEnvironment environment,
            Set<String> name) {
        this.environment = environment;
        this.name = new LinkedHashSet<>(name);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Set<String> getName() {
        return java.util.Collections.unmodifiableSet(name);
    }

    @Override
    public Partition withName(Collection<String> name) {
        return new PartitionImpl(environment, new LinkedHashSet<>(name));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Partition)) return false;
        Partition other = (Partition) o;
        return name.equals(other.getName());
    }

    @Override
    public int hashCode() {
        return name.hashCode();
    }

    @Override
    public String toString() {
        return "Partition[name=" + name + "]";
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
    public PartitionImpl clone() {
        try {
            return (PartitionImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
