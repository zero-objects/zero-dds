// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.Arrays;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.topic.BuiltinTopicKey;

/**
 * ZeroDDS {@link BuiltinTopicKey} (DDS 1.2 §2.2.1.3.1, DDSI-RTPS 2.5
 * §9.3.1.2): the four 32-bit words that identify a discovered entity.
 *
 * <p>§2.2.1.3.1 makes {@code getValue} a copy, so a caller that mutates the
 * array it got back cannot corrupt the key of the discovered entity.
 */
public final class BuiltinTopicKeyImpl implements BuiltinTopicKey {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int[] value;

    public BuiltinTopicKeyImpl(ServiceEnvironment environment, int[] value) {
        this.environment = environment;
        this.value = value.clone();
    }

    /**
     * The key of a handle (DDS 1.2 §2.2.1.3.1): the four words of the
     * handle, big-endian, as the built-in topics carry them.
     */
    public static BuiltinTopicKeyImpl of(ServiceEnvironment environment,
            org.omg.dds.core.InstanceHandle handle) {
        byte[] bytes = org.zerodds.core.InstanceHandleImpl.bytesOf(handle);
        int[] words = new int[4];
        for (int word = 0; word < 4; word++) {
            int value = 0;
            for (int b = 0; b < 4; b++) {
                value = (value << 8) | (bytes[word * 4 + b] & 0xFF);
            }
            words[word] = value;
        }
        return new BuiltinTopicKeyImpl(environment, words);
    }

    @Override
    public int[] getValue() {
        return value.clone();
    }

    @Override
    public void copyFrom(BuiltinTopicKey src) {
        int[] other = src.getValue();
        System.arraycopy(other, 0, value, 0, Math.min(other.length, value.length));
    }

    @Override
    public BuiltinTopicKey clone() {
        return new BuiltinTopicKeyImpl(environment, value);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof BuiltinTopicKeyImpl
                && Arrays.equals(value, ((BuiltinTopicKeyImpl) o).value);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(value);
    }

    @Override
    public String toString() {
        return "BuiltinTopicKey" + Arrays.toString(value);
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
}
