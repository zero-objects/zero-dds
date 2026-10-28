// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;

/**
 * ZeroDDS {@link ModifiableInstanceHandle} (DDS-Java-PSM 1.0 §7.3.4).
 */
public final class ModifiableInstanceHandleImpl extends ModifiableInstanceHandle implements Cloneable {

    private static final long serialVersionUID = 4589938527533296265L;

    private transient ServiceEnvironment environment;
    final byte[] bytes;

    public ModifiableInstanceHandleImpl(ServiceEnvironment environment, byte[] bytes) {
        if (bytes.length != InstanceHandleImpl.SIZE) {
            throw new IllegalArgumentException(
                    "an InstanceHandle is " + InstanceHandleImpl.SIZE + " bytes, got " + bytes.length);
        }
        this.environment = environment;
        this.bytes = bytes.clone();
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean isNil() {
        for (byte b : bytes) {
            if (b != 0) {
                return false;
            }
        }
        return true;
    }

    @Override
    public void copyFrom(InstanceHandle src) {
        System.arraycopy(InstanceHandleImpl.bytesOf(src), 0, bytes, 0, bytes.length);
    }

    @Override
    public InstanceHandle immutableCopy() {
        return new InstanceHandleImpl(environment, bytes);
    }

    @Override
    public ModifiableInstanceHandle modifiableCopy() {
        return new ModifiableInstanceHandleImpl(environment, bytes);
    }

    @Override
    public int compareTo(InstanceHandle other) {
        return InstanceHandleImpl.compareUnsigned(bytes, InstanceHandleImpl.bytesOf(other));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof InstanceHandle)) return false;
        InstanceHandle other = (InstanceHandle) o;
        return java.util.Arrays.equals(bytes, InstanceHandleImpl.bytesOf(other));
    }

    @Override
    public int hashCode() {
        return java.util.Arrays.hashCode(bytes);
    }

    @Override
    public String toString() {
        return "ModifiableInstanceHandle[" + InstanceHandleImpl.hex(bytes) + "]";
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
    public ModifiableInstanceHandleImpl clone() {
        return new ModifiableInstanceHandleImpl(environment, bytes);
    }
}
