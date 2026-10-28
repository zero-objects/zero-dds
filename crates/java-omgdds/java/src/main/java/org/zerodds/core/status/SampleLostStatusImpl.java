// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.status.SampleLostStatus;

/**
 * ZeroDDS SampleLostStatus.
 *
 * <p>SAMPLE_LOST: a sample could not be received (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: §7.3.7.1 lets an implementation pool and reuse a status
 * object, so an application that keeps one has to copy it. ZeroDDS never
 * changes a status it handed out, so the {@code _change} counts are fixed
 * at construction.
 */
public final class SampleLostStatusImpl extends SampleLostStatus implements Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int totalCount;
    private final int totalCountChange;

    public SampleLostStatusImpl(ServiceEnvironment environment,
            int totalCount,
            int totalCountChange) {
        this.environment = environment;
        this.totalCount = totalCount;
        this.totalCountChange = totalCountChange;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public int getTotalCount() {
        return totalCount;
    }

    @Override
    public int getTotalCountChange() {
        return totalCountChange;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof SampleLostStatusImpl)) {
            return false;
        }
        SampleLostStatusImpl that = (SampleLostStatusImpl) o;
        return totalCount == that.totalCount && totalCountChange == that.totalCountChange;
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(totalCount, totalCountChange);
    }

    @Override
    public String toString() {
        return "SampleLostStatus[" + "totalCount=" + totalCount + "," + "totalCountChange=" + totalCountChange + "]";
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
    public SampleLostStatusImpl clone() {
        try {
            return (SampleLostStatusImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
