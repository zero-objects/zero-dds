// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.core.InstanceHandle;

/**
 * ZeroDDS SubscriptionMatchedStatus.
 *
 * <p>SUBSCRIPTION_MATCHED: the reader's subscription became associated with one more writer (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: §7.3.7.1 lets an implementation pool and reuse a status
 * object, so an application that keeps one has to copy it. ZeroDDS never
 * changes a status it handed out, so the {@code _change} counts are fixed
 * at construction.
 */
public final class SubscriptionMatchedStatusImpl extends SubscriptionMatchedStatus implements Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int totalCount;
    private final int totalCountChange;
    private final int currentCount;
    private final int currentCountChange;
    private final org.omg.dds.core.InstanceHandle lastPublicationHandle;

    public SubscriptionMatchedStatusImpl(ServiceEnvironment environment,
            int totalCount,
            int totalCountChange,
            int currentCount,
            int currentCountChange,
            org.omg.dds.core.InstanceHandle lastPublicationHandle) {
        this.environment = environment;
        this.totalCount = totalCount;
        this.totalCountChange = totalCountChange;
        this.currentCount = currentCount;
        this.currentCountChange = currentCountChange;
        this.lastPublicationHandle = lastPublicationHandle;
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
    public int getCurrentCount() {
        return currentCount;
    }

    @Override
    public int getCurrentCountChange() {
        return currentCountChange;
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
        if (!(o instanceof SubscriptionMatchedStatusImpl)) {
            return false;
        }
        SubscriptionMatchedStatusImpl that = (SubscriptionMatchedStatusImpl) o;
        return totalCount == that.totalCount && totalCountChange == that.totalCountChange && currentCount == that.currentCount && currentCountChange == that.currentCountChange && java.util.Objects.equals(lastPublicationHandle, that.lastPublicationHandle);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(totalCount, totalCountChange, currentCount, currentCountChange, lastPublicationHandle);
    }

    @Override
    public String toString() {
        return "SubscriptionMatchedStatus[" + "totalCount=" + totalCount + "," + "totalCountChange=" + totalCountChange + "," + "currentCount=" + currentCount + "," + "currentCountChange=" + currentCountChange + "," + "lastPublicationHandle=" + lastPublicationHandle + "]";
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
    public SubscriptionMatchedStatusImpl clone() {
        try {
            return (SubscriptionMatchedStatusImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
