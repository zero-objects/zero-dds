// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.ReaderDataLifecycle;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS ReaderDataLifecycle (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>How long a reader keeps samples of an instance that has no writers or is disposed (DDS 1.2 §7.1.3.9). Default: infinite for both delays.
 */
public final class ReaderDataLifecycleImpl implements ReaderDataLifecycle, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration autoPurgeNoWriterSamplesDelay;
    private final org.omg.dds.core.Duration autoPurgeDisposedSamplesDelay;

    public ReaderDataLifecycleImpl(ServiceEnvironment environment,
            org.omg.dds.core.Duration autoPurgeNoWriterSamplesDelay,
            org.omg.dds.core.Duration autoPurgeDisposedSamplesDelay) {
        this.environment = environment;
        this.autoPurgeNoWriterSamplesDelay = autoPurgeNoWriterSamplesDelay;
        this.autoPurgeDisposedSamplesDelay = autoPurgeDisposedSamplesDelay;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public org.omg.dds.core.Duration getAutoPurgeNoWriterSamplesDelay() {
        return autoPurgeNoWriterSamplesDelay;
    }

    @Override
    public org.omg.dds.core.Duration getAutoPurgeDisposedSamplesDelay() {
        return autoPurgeDisposedSamplesDelay;
    }

    @Override
    public ReaderDataLifecycle withAutoPurgeNoWriterSamplesDelay(Duration autoPurgeNoWriterSamplesDelay) {
        return new ReaderDataLifecycleImpl(environment, autoPurgeNoWriterSamplesDelay, autoPurgeDisposedSamplesDelay);
    }

    @Override
    public ReaderDataLifecycle withAutoPurgeNoWriterSamplesDelay(long autoPurgeNoWriterSamplesDelay, TimeUnit unit) {
        return withAutoPurgeNoWriterSamplesDelay(
                org.zerodds.core.DurationImpl.of(environment, autoPurgeNoWriterSamplesDelay, unit));
    }

    @Override
    public ReaderDataLifecycle withAutoPurgeDisposedSamplesDelay(Duration autoPurgeDisposedSamplesDelay) {
        return new ReaderDataLifecycleImpl(environment, autoPurgeNoWriterSamplesDelay, autoPurgeDisposedSamplesDelay);
    }

    @Override
    public ReaderDataLifecycle withAutoPurgeDisposedSamplesDelay(long autoPurgeDisposedSamplesDelay, TimeUnit unit) {
        return withAutoPurgeDisposedSamplesDelay(
                org.zerodds.core.DurationImpl.of(environment, autoPurgeDisposedSamplesDelay, unit));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof ReaderDataLifecycle)) return false;
        ReaderDataLifecycle other = (ReaderDataLifecycle) o;
        return java.util.Objects.equals(autoPurgeNoWriterSamplesDelay, other.getAutoPurgeNoWriterSamplesDelay())
                && java.util.Objects.equals(autoPurgeDisposedSamplesDelay, other.getAutoPurgeDisposedSamplesDelay());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(autoPurgeNoWriterSamplesDelay, autoPurgeDisposedSamplesDelay);
    }

    @Override
    public String toString() {
        return "ReaderDataLifecycle[noWriters=" + autoPurgeNoWriterSamplesDelay
                + ", disposed=" + autoPurgeDisposedSamplesDelay + "]";
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
    public ReaderDataLifecycleImpl clone() {
        try {
            return (ReaderDataLifecycleImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
