// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;

import java.util.Arrays;

/**
 * ZeroDDS {@link InstanceHandle} (DDS-Java-PSM 1.0 §7.3.4).
 *
 * <p>The handle is the opaque 16-byte key of an instance. All-zero bytes are
 * the nil handle of §7.3.4.
 */
public class InstanceHandleImpl extends InstanceHandle implements Cloneable {

    private static final long serialVersionUID = 4987266126475078295L;

    static final int SIZE = 16;

    transient ServiceEnvironment environment;
    final byte[] bytes;

    public InstanceHandleImpl(ServiceEnvironment environment, byte[] bytes) {
        if (bytes.length != SIZE) {
            throw new IllegalArgumentException(
                    "an InstanceHandle is " + SIZE + " bytes, got " + bytes.length);
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
    public ModifiableInstanceHandle modifiableCopy() {
        return new ModifiableInstanceHandleImpl(environment, bytes);
    }

    @Override
    public int compareTo(InstanceHandle other) {
        return compareUnsigned(bytes, bytesOf(other));
    }

    /**
     * Lexicographic comparison of two octet sequences as unsigned values
     * ({@code Arrays.compareUnsigned} needs Java 9).
     */
    public static int compareUnsigned(byte[] a, byte[] b) {
        int common = Math.min(a.length, b.length);
        for (int i = 0; i < common; i++) {
            int diff = (a[i] & 0xff) - (b[i] & 0xff);
            if (diff != 0) {
                return diff;
            }
        }
        return a.length - b.length;
    }

    public static byte[] bytesOf(InstanceHandle handle) {
        if (handle instanceof InstanceHandleImpl) {
            InstanceHandleImpl impl = (InstanceHandleImpl) handle;
            return impl.bytes;
        }
        if (handle instanceof ModifiableInstanceHandleImpl) {
            ModifiableInstanceHandleImpl impl = (ModifiableInstanceHandleImpl) handle;
            return impl.bytes;
        }
        if (handle == null) {
            throw new IllegalArgumentException("an InstanceHandle must not be null");
        }
        return foreignBytes(handle);
    }

    /**
     * The 16 octets standing for an {@link InstanceHandle} of another
     * implementation (§7.2.2, value types of another implementation are
     * accepted). The handle is nil exactly when {@code isNil()} says so;
     * otherwise the octets are the first 16 of the SHA-256 of its class name
     * and of its Java serialization (the handle is {@code Serializable}), or
     * of its {@code toString()} when it cannot be serialized. Equal foreign
     * handles give equal octets; such octets name no instance of this service.
     */
    private static byte[] foreignBytes(InstanceHandle handle) {
        byte[] out = new byte[SIZE];
        if (handle.isNil()) {
            return out;
        }
        try {
            java.security.MessageDigest digest = java.security.MessageDigest.getInstance("SHA-256");
            digest.update(handle.getClass().getName().getBytes(java.nio.charset.StandardCharsets.UTF_8));
            byte[] form;
            try {
                java.io.ByteArrayOutputStream buffer = new java.io.ByteArrayOutputStream();
                java.io.ObjectOutputStream stream = new java.io.ObjectOutputStream(buffer);
                stream.writeObject(handle);
                stream.flush();
                form = buffer.toByteArray();
            } catch (java.io.IOException notSerializable) {
                form = String.valueOf(handle).getBytes(java.nio.charset.StandardCharsets.UTF_8);
            }
            System.arraycopy(digest.digest(form), 0, out, 0, SIZE);
            if (isAllZero(out)) {
                out[SIZE - 1] = 1; // a non-nil handle never maps to the nil octets
            }
            return out;
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 is part of every Java platform", e);
        }
    }

    private static boolean isAllZero(byte[] a) {
        for (byte b : a) {
            if (b != 0) {
                return false;
            }
        }
        return true;
    }

    /** {@code bytes} as uppercase hex, the form of an InstanceHandle's text. */
    static String hex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02X", b));
        }
        return sb.toString();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof InstanceHandle)) return false;
        InstanceHandle other = (InstanceHandle) o;
        // a handle equals every handle with the same octets, a modifiable copy included
        return Arrays.equals(bytes, bytesOf(other));
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(bytes);
    }

    @Override
    public String toString() {
        return "InstanceHandle[" + hex(bytes) + "]";
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
    public InstanceHandleImpl clone() {
        try {
            return (InstanceHandleImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}