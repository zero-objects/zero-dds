// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS Reliability (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Whether the service retransmits samples (DDS 1.2 §7.1.3.6). Requested/offered: BEST_EFFORT < RELIABLE. Default: RELIABLE with a 100 ms max blocking time for writers, BEST_EFFORT for readers and topics.
 */
public final class ReliabilityImpl implements Reliability, RequestedOffered<Reliability>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Reliability.Kind kind;
    private final org.omg.dds.core.Duration maxBlockingTime;

    public ReliabilityImpl(ServiceEnvironment environment,
            Reliability.Kind kind,
            org.omg.dds.core.Duration maxBlockingTime) {
        this.environment = environment;
        this.kind = kind;
        this.maxBlockingTime = maxBlockingTime;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Reliability.Kind getKind() {
        return kind;
    }

    @Override
    public org.omg.dds.core.Duration getMaxBlockingTime() {
        return maxBlockingTime;
    }

    @Override
    public Reliability withKind(Reliability.Kind kind) {
        return new ReliabilityImpl(environment, kind, maxBlockingTime);
    }

    @Override
    public Reliability withMaxBlockingTime(Duration maxBlockingTime) {
        return new ReliabilityImpl(environment, kind, maxBlockingTime);
    }

    @Override
    public Reliability withMaxBlockingTime(long maxBlockingTime, TimeUnit unit) {
        return withMaxBlockingTime(org.zerodds.core.DurationImpl.of(environment, maxBlockingTime, unit));
    }

    @Override
    public Reliability withBestEffort() {
        return withKind(Kind.BEST_EFFORT);
    }

    @Override
    public Reliability withReliable() {
        return withKind(Kind.RELIABLE);
    }

    @Override
    public int compareTo(Reliability other) {
        int byKind = kind.compareTo(other.getKind());
        if (byKind != 0) {
            return byKind;
        }
        return maxBlockingTime.compareTo(other.getMaxBlockingTime());
    }

    /**
     * DDS 1.4 §2.2.3.14: the offered kind must be at least the requested one
     * (BEST_EFFORT &lt; RELIABLE). {@code max_blocking_time} is not part of the
     * Request/Offer contract.
     */
    @Override
    public Comparable<Reliability> requestedOfferedContract() {
        return other -> kind.compareTo(other.getKind());
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Reliability)) return false;
        Reliability other = (Reliability) o;
        return kind == other.getKind() && java.util.Objects.equals(maxBlockingTime, other.getMaxBlockingTime());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(kind, maxBlockingTime);
    }

    @Override
    public String toString() {
        return "Reliability[kind=" + kind + ", maxBlockingTime=" + maxBlockingTime + "]";
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
    public ReliabilityImpl clone() {
        try {
            return (ReliabilityImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
