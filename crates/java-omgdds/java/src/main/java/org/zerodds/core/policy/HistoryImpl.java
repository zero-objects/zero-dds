// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.History;

/**
 * ZeroDDS History (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>How many samples of one instance a writer keeps (DDS 1.2 §7.1.3.7). Default: KEEP_LAST with depth 1.
 */
public final class HistoryImpl implements History, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final History.Kind kind;
    private final int depth;

    public HistoryImpl(ServiceEnvironment environment,
            History.Kind kind,
            int depth) {
        this.environment = environment;
        this.kind = kind;
        this.depth = depth;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public History.Kind getKind() {
        return kind;
    }

    @Override
    public int getDepth() {
        return depth;
    }

    @Override
    public History withKind(History.Kind kind) {
        return new HistoryImpl(environment, kind, depth);
    }

    @Override
    public History withDepth(int depth) {
        if (depth < 0) {
            throw new IllegalArgumentException("HISTORY depth must not be negative: " + depth);
        }
        return new HistoryImpl(environment, kind, depth);
    }

    @Override
    public History withKeepAll() {
        return withKind(Kind.KEEP_ALL);
    }

    @Override
    public History withKeepLast(int depth) {
        return new HistoryImpl(environment, Kind.KEEP_LAST, depth);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof History)) return false;
        History other = (History) o;
        return kind == other.getKind() && depth == other.getDepth();
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(kind, depth);
    }

    @Override
    public String toString() {
        return "History[kind=" + kind + ", depth=" + depth + "]";
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
    public HistoryImpl clone() {
        try {
            return (HistoryImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
