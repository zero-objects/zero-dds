// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.InstanceHandle;

/**
 * ZeroDDS LivelinessChangedStatus.
 *
 * <p>LIVELINESS_CHANGED: how many matched writers assert liveliness and how many do not (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: §7.3.7.1 lets an implementation pool and reuse a status
 * object, so an application that keeps one has to copy it. ZeroDDS never
 * changes a status it handed out, so the {@code _change} counts are fixed
 * at construction.
 */
public final class LivelinessChangedStatusImpl extends LivelinessChangedStatus implements Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int aliveCount;
    private final int notAliveCount;
    private final int aliveCountChange;
    private final int notAliveCountChange;
    private final org.omg.dds.core.InstanceHandle lastPublicationHandle;

    public LivelinessChangedStatusImpl(ServiceEnvironment environment,
            int aliveCount,
            int notAliveCount,
            int aliveCountChange,
            int notAliveCountChange,
            org.omg.dds.core.InstanceHandle lastPublicationHandle) {
        this.environment = environment;
        this.aliveCount = aliveCount;
        this.notAliveCount = notAliveCount;
        this.aliveCountChange = aliveCountChange;
        this.notAliveCountChange = notAliveCountChange;
        this.lastPublicationHandle = lastPublicationHandle;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public int getAliveCount() {
        return aliveCount;
    }

    @Override
    public int getNotAliveCount() {
        return notAliveCount;
    }

    @Override
    public int getAliveCountChange() {
        return aliveCountChange;
    }

    @Override
    public int getNotAliveCountChange() {
        return notAliveCountChange;
    }

    @Override
    public org.omg.dds.core.InstanceHandle getLastPublicationHandle() {
        return lastPublicationHandle;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof LivelinessChangedStatusImpl)) {
            return false;
        }
        LivelinessChangedStatusImpl that = (LivelinessChangedStatusImpl) o;
        return aliveCount == that.aliveCount && notAliveCount == that.notAliveCount && aliveCountChange == that.aliveCountChange && notAliveCountChange == that.notAliveCountChange && java.util.Objects.equals(lastPublicationHandle, that.lastPublicationHandle);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(aliveCount, notAliveCount, aliveCountChange, notAliveCountChange, lastPublicationHandle);
    }

    @Override
    public String toString() {
        return "LivelinessChangedStatus[" + "aliveCount=" + aliveCount + "," + "notAliveCount=" + notAliveCount + "," + "aliveCountChange=" + aliveCountChange + "," + "notAliveCountChange=" + notAliveCountChange + "," + "lastPublicationHandle=" + lastPublicationHandle + "]";
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
    public LivelinessChangedStatusImpl clone() {
        try {
            return (LivelinessChangedStatusImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
