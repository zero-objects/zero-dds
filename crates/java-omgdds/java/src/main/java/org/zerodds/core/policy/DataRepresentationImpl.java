// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.DataRepresentation;
import java.util.ArrayList;
import java.util.List;

/**
 * ZeroDDS DataRepresentation (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The XCDR versions a topic may use on the wire (DDS-XTypes 1.0). Default: XCDR version 1 only.
 */
public final class DataRepresentationImpl implements DataRepresentation, RequestedOffered<DataRepresentation>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final ArrayList<Short> value;

    public DataRepresentationImpl(ServiceEnvironment environment,
            List<Short> value) {
        this.environment = environment;
        this.value = new ArrayList<>(value);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public List<Short> getValue() {
        return value;
    }

    @Override
    public DataRepresentation withValue(List<Short> value) {
        return new DataRepresentationImpl(environment, new ArrayList<>(value));
    }

    @Override
    public DataRepresentation withValue(short... value) {
        List<Short> ids = new ArrayList<>(value.length);
        for (short id : value) {
            ids.add(id);
        }
        return new DataRepresentationImpl(environment, ids);
    }

    @Override
    public int compareTo(DataRepresentation other) {
        return compareValues(value, other.getValue());
    }

    /** Lexicographic comparison of the XCDR version list, shortest list first. */
    private static int compareValues(List<Short> a, List<Short> b) {
        int common = Math.min(a.size(), b.size());
        for (int i = 0; i < common; i++) {
            int byElement = Short.compare(a.get(i), b.get(i));
            if (byElement != 0) {
                return byElement;
            }
        }
        return Integer.compare(a.size(), b.size());
    }

    @Override
    public Comparable<DataRepresentation> requestedOfferedContract() {
        return this;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof DataRepresentation)) return false;
        DataRepresentation other = (DataRepresentation) o;
        return value.equals(other.getValue());
    }

    @Override
    public int hashCode() {
        return value.hashCode();
    }

    @Override
    public String toString() {
        return "DataRepresentation[value=" + value + "]";
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
    public DataRepresentationImpl clone() {
        try {
            return (DataRepresentationImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
