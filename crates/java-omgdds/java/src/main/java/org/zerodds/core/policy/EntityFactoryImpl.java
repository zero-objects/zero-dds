// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.EntityFactory;

/**
 * ZeroDDS EntityFactory (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Whether entities are enabled when they are created (DDS 1.2 §7.1.3.1). Default: true.
 */
public final class EntityFactoryImpl implements EntityFactory, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final boolean autoEnableCreatedEntities;

    public EntityFactoryImpl(ServiceEnvironment environment,
            boolean autoEnableCreatedEntities) {
        this.environment = environment;
        this.autoEnableCreatedEntities = autoEnableCreatedEntities;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean isAutoEnableCreatedEntities() {
        return autoEnableCreatedEntities;
    }

    @Override
    public EntityFactory withAutoEnableCreatedEntities(boolean autoEnableCreatedEntities) {
        return new EntityFactoryImpl(environment, autoEnableCreatedEntities);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof EntityFactory)) return false;
        EntityFactory other = (EntityFactory) o;
        return autoEnableCreatedEntities == other.isAutoEnableCreatedEntities();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(autoEnableCreatedEntities);
    }

    @Override
    public String toString() {
        return "EntityFactory[autoEnableCreatedEntities=" + autoEnableCreatedEntities + "]";
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
    public EntityFactoryImpl clone() {
        try {
            return (EntityFactoryImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
