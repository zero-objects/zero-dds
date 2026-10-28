// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.ResourceLimits;

/**
 * ZeroDDS ResourceLimits (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The upper bounds on the resources one entity may use (DDS 1.2 §7.1.3.7). Default: LENGTH_UNLIMITED for all three.
 */
public final class ResourceLimitsImpl implements ResourceLimits, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int maxSamples;
    private final int maxInstances;
    private final int maxSamplesPerInstance;

    public ResourceLimitsImpl(ServiceEnvironment environment,
            int maxSamples,
            int maxInstances,
            int maxSamplesPerInstance) {
        this.environment = environment;
        this.maxSamples = maxSamples;
        this.maxInstances = maxInstances;
        this.maxSamplesPerInstance = maxSamplesPerInstance;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
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
    public ResourceLimits withMaxSamples(int maxSamples) {
        return new ResourceLimitsImpl(environment, maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public ResourceLimits withMaxInstances(int maxInstances) {
        return new ResourceLimitsImpl(environment, maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public ResourceLimits withMaxSamplesPerInstance(int maxSamplesPerInstance) {
        return new ResourceLimitsImpl(environment, maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof ResourceLimits)) return false;
        ResourceLimits other = (ResourceLimits) o;
        return maxSamples == other.getMaxSamples() && maxInstances == other.getMaxInstances()
                && maxSamplesPerInstance == other.getMaxSamplesPerInstance();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(maxSamples, maxInstances, maxSamplesPerInstance);
    }

    @Override
    public String toString() {
        return "ResourceLimits[maxSamples=" + maxSamples + ", maxInstances=" + maxInstances
                + ", maxSamplesPerInstance=" + maxSamplesPerInstance + "]";
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
    public ResourceLimitsImpl clone() {
        try {
            return (ResourceLimitsImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
