// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;
import org.omg.dds.core.policy.History;

/**
 * ZeroDDS DurabilityService (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The service-side history of a non-VOLATILE durability (DDS 1.2 §7.1.3.5). Default: no cleanup delay, KEEP_LAST depth 1, LENGTH_UNLIMITED for the three resource limits.
 */
public final class DurabilityServiceImpl implements DurabilityService, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration serviceCleanupDelay;
    private final History.Kind historyKind;
    private final int historyDepth;
    private final int maxSamples;
    private final int maxInstances;
    private final int maxSamplesPerInstance;

    public DurabilityServiceImpl(ServiceEnvironment environment,
            org.omg.dds.core.Duration serviceCleanupDelay,
            History.Kind historyKind,
            int historyDepth,
            int maxSamples,
            int maxInstances,
            int maxSamplesPerInstance) {
        this.environment = environment;
        this.serviceCleanupDelay = serviceCleanupDelay;
        this.historyKind = historyKind;
        this.historyDepth = historyDepth;
        this.maxSamples = maxSamples;
        this.maxInstances = maxInstances;
        this.maxSamplesPerInstance = maxSamplesPerInstance;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public org.omg.dds.core.Duration getServiceCleanupDelay() {
        return serviceCleanupDelay;
    }

    @Override
    public History.Kind getHistoryKind() {
        return historyKind;
    }

    @Override
    public int getHistoryDepth() {
        return historyDepth;
    }

    @Override
    public int getMaxSamples() {
        return maxSamples;
    }

    @Override
    public int getMaxInstances() {
        return maxInstances;
    }

    @Override
    public int getMaxSamplesPerInstance() {
        return maxSamplesPerInstance;
    }

    @Override
    public DurabilityService withServiceCleanupDelay(Duration serviceCleanupDelay) {
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public DurabilityService withServiceCleanupDelay(long serviceCleanupDelay, TimeUnit unit) {
        return withServiceCleanupDelay(org.zerodds.core.DurationImpl.of(environment, serviceCleanupDelay, unit));
    }

    @Override
    public DurabilityService withHistoryKind(History.Kind historyKind) {
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public DurabilityService withHistoryDepth(int historyDepth) {
        if (historyDepth < 0) {
            throw new IllegalArgumentException("HISTORY depth must not be negative: " + historyDepth);
        }
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public DurabilityService withMaxSamples(int maxSamples) {
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public DurabilityService withMaxInstances(int maxInstances) {
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public DurabilityService withMaxSamplesPerInstance(int maxSamplesPerInstance) {
        return new DurabilityServiceImpl(environment, serviceCleanupDelay, historyKind, historyDepth,
                maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof DurabilityService)) return false;
        DurabilityService other = (DurabilityService) o;
        return java.util.Objects.equals(serviceCleanupDelay, other.getServiceCleanupDelay())
                && historyKind == other.getHistoryKind()
                && historyDepth == other.getHistoryDepth()
                && maxSamples == other.getMaxSamples()
                && maxInstances == other.getMaxInstances()
                && maxSamplesPerInstance == other.getMaxSamplesPerInstance();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(serviceCleanupDelay, historyKind, historyDepth, maxSamples,
                maxInstances, maxSamplesPerInstance);
    }

    @Override
    public String toString() {
        return "DurabilityService[cleanupDelay=" + serviceCleanupDelay + ", history="
                + historyKind + " depth " + historyDepth + ", maxSamples=" + maxSamples
                + ", maxInstances=" + maxInstances + ", maxSamplesPerInstance="
                + maxSamplesPerInstance + "]";
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
    public DurabilityServiceImpl clone() {
        try {
            return (DurabilityServiceImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
