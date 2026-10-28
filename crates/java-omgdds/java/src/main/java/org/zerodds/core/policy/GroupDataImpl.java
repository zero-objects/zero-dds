// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.GroupData;
import java.util.Arrays;

/**
 * ZeroDDS GroupData (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>Opaque application data that groups a publisher or a subscriber (DDS 1.2 §7.1.3.2). Default: a zero-length byte sequence.
 */
public final class GroupDataImpl implements GroupData, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final byte[] value;

    public GroupDataImpl(ServiceEnvironment environment,
            byte[] value) {
        this.environment = environment;
        this.value = value == null ? new byte[0] : value.clone();
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public byte[] getValue() {
        return value.clone();
    }

    @Override
    public GroupData withValue(byte value[], int offset, int length) {
        return new GroupDataImpl(environment, Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof GroupData)) return false;
        GroupData other = (GroupData) o;
        return Arrays.equals(value, other.getValue());
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(value);
    }

    @Override
    public String toString() {
        return "GroupData[length=" + value.length + "]";
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
    public GroupDataImpl clone() {
        try {
            return (GroupDataImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
