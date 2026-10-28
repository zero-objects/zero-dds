// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS Liveliness (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>How a writer asserts that an instance is alive (DDS 1.2 §7.1.3.7). Requested/offered: the offered kind must be at least the requested one and, for equal kinds, the offered lease must not exceed the requested one. Default: AUTOMATIC with an infinite lease.
 */
public final class LivelinessImpl implements Liveliness, RequestedOffered<Liveliness>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Liveliness.Kind kind;
    private final org.omg.dds.core.Duration leaseDuration;

    public LivelinessImpl(ServiceEnvironment environment,
            Liveliness.Kind kind,
            org.omg.dds.core.Duration leaseDuration) {
        this.environment = environment;
        this.kind = kind;
        this.leaseDuration = leaseDuration;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Liveliness.Kind getKind() {
        return kind;
    }

    @Override
    public org.omg.dds.core.Duration getLeaseDuration() {
        return leaseDuration;
    }

    @Override
    public Liveliness withKind(Liveliness.Kind kind) {
        return new LivelinessImpl(environment, kind, leaseDuration);
    }

    @Override
    public Liveliness withLeaseDuration(Duration leaseDuration) {
        return new LivelinessImpl(environment, kind, leaseDuration);
    }

    @Override
    public Liveliness withLeaseDuration(long leaseDuration, TimeUnit unit) {
        return withLeaseDuration(org.zerodds.core.DurationImpl.of(environment, leaseDuration, unit));
    }

    @Override
    public Liveliness withAutomatic() {
        return withKind(Kind.AUTOMATIC);
    }

    @Override
    public Liveliness withManualByParticipant() {
        return withKind(Kind.MANUAL_BY_PARTICIPANT);
    }

    @Override
    public Liveliness withManualByTopic() {
        return withKind(Kind.MANUAL_BY_TOPIC);
    }

    @Override
    public int compareTo(Liveliness other) {
        int byKind = kind.compareTo(other.getKind());
        if (byKind != 0) {
            return byKind;
        }
        return leaseDuration.compareTo(other.getLeaseDuration());
    }

    @Override
    public Comparable<Liveliness> requestedOfferedContract() {
        // DDS 1.4 §2.2.3.11: the offered kind must be at least the requested one
        // (AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC) and the offered lease
        // duration no longer than the requested one.
        return other -> {
            int byKind = kind.compareTo(other.getKind());
            return byKind != 0 ? byKind : other.getLeaseDuration().compareTo(leaseDuration);
        };
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Liveliness)) return false;
        Liveliness other = (Liveliness) o;
        return kind == other.getKind() && java.util.Objects.equals(leaseDuration, other.getLeaseDuration());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(kind, leaseDuration);
    }

    @Override
    public String toString() {
        return "Liveliness[kind=" + kind + ", leaseDuration=" + leaseDuration + "]";
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
    public LivelinessImpl clone() {
        try {
            return (LivelinessImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
