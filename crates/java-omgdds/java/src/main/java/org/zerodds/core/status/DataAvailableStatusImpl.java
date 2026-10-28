// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.status.DataAvailableStatus;

/**
 * ZeroDDS DataAvailableStatus.
 *
 * <p>DATA_AVAILABLE: the entity has data that read/take can return (DDS 1.2 §7.1.4.1). It carries no counter.
 *
 * <p>Immutable: §7.3.7.1 lets an implementation pool and reuse a status
 * object, so an application that keeps one has to copy it. ZeroDDS never
 * changes a status it handed out, so the {@code _change} counts are fixed
 * at construction.
 */
public final class DataAvailableStatusImpl extends DataAvailableStatus implements Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;

    public DataAvailableStatusImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof DataAvailableStatusImpl)) {
            return false;
        }
        DataAvailableStatusImpl that = (DataAvailableStatusImpl) o;
        return true;
    }

    @Override
    public int hashCode() {
        return getClass().hashCode();
    }

    @Override
    public String toString() {
        return "DataAvailableStatus";
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
    public DataAvailableStatusImpl clone() {
        try {
            return (DataAvailableStatusImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
