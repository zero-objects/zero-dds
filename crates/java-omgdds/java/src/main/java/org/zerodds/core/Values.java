// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;

import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * The value-type construction the runtime needs where the PSM's
 * {@code new<ClassName>} factories (DDS-Java-PSM 1.0 §7.2.3) are not the
 * right shape — an instant from a wall-clock reading, a handle from raw bytes.
 */
public final class Values {

    private Values() {}

    /** A {@link Time} of {@code millis} since the epoch. */
    public static Time ofMillis(ServiceEnvironment environment, long millis) {
        long[] parts = TimeImpl.split(millis, TimeUnit.MILLISECONDS);
        return new TimeImpl(environment, parts[0], parts[1]);
    }

    /** An {@link InstanceHandle} over {@code bytes}, which must be 16 bytes long. */
    public static InstanceHandle of(ServiceEnvironment environment, byte[] bytes) {
        return new InstanceHandleImpl(environment, bytes);
    }

    /** The nil {@link InstanceHandle} of {@code environment}. */
    public static InstanceHandle nil(ServiceEnvironment environment) {
        return new InstanceHandleImpl(environment, new byte[InstanceHandleImpl.SIZE]);
    }

    /** A fresh {@link InstanceHandle} for {@code uuid}. */
    public static InstanceHandle ofUuid(ServiceEnvironment environment) {
        return of(environment, uuidBytes());
    }

    /** {@code millis} as a {@link Duration}. */
    public static Duration durationOfMillis(ServiceEnvironment environment, long millis) {
        return DurationImpl.of(environment, millis, TimeUnit.MILLISECONDS);
    }

    /** The infinite {@link Duration} of {@code environment}. */
    public static Duration infiniteDuration(ServiceEnvironment environment) {
        return DurationImpl.infinite(environment);
    }

    /**
     * The overloads below build their value in the environment of this
     * binding's own {@code ServiceEnvironmentImpl} singleton, which is the one
     * an application gets from
     * {@code ServiceEnvironment.createInstance(…)} with
     * {@code org.omg.dds.serviceClassName} set to that class. Entities that
     * know their own environment use the explicit overloads above.
     */

    /** A {@link Time} of {@code millis} since the epoch, in the default environment. */
    public static Time ofMillis(long millis) {
        return ofMillis(getDefaultEnvironment(), millis);
    }

    /** An {@link InstanceHandle} over {@code bytes}, in the default environment. */
    public static InstanceHandle of(byte[] bytes) {
        return of(getDefaultEnvironment(), bytes);
    }

    /** The nil {@link InstanceHandle} of the default environment. */
    public static InstanceHandle nil() {
        return nil(getDefaultEnvironment());
    }

    /** {@code millis} as a {@link Duration} of the default environment. */
    public static Duration durationOfMillis(long millis) {
        return durationOfMillis(getDefaultEnvironment(), millis);
    }

    /** The infinite {@link Duration} of the default environment. */
    public static Duration infiniteDuration() {
        return infiniteDuration(getDefaultEnvironment());
    }

    /** The environment the value types without an explicit one live in. */
    public static ServiceEnvironment getDefaultEnvironment() {
        return org.zerodds.ServiceEnvironmentImpl.getDefaultEnvironment();
    }

    private static byte[] uuidBytes() {
        UUID id = UUID.randomUUID();
        byte[] out = new byte[InstanceHandleImpl.SIZE];
        long msb = id.getMostSignificantBits();
        long lsb = id.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) out[i] = (byte) (msb >>> (56 - 8 * i));
        for (int i = 0; i < 8; i++) out[8 + i] = (byte) (lsb >>> (56 - 8 * i));
        return out;
    }
}