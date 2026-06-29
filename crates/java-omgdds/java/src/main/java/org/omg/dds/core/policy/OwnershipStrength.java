// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

/**
 * OMG DDS OwnershipStrengthQosPolicy — DDS-DCPS 1.4 §2.2.3.12.2.
 *
 * <p>Applies to a DataWriter only and is meaningful only when the
 * {@link Ownership} kind is {@code EXCLUSIVE}. The writer with the highest
 * strength owns each instance and is the sole source the reader accepts.
 * Default value is {@code 0}.
 */
public final class OwnershipStrength {
    public static final OwnershipStrength DEFAULT = new OwnershipStrength(0);

    private final int value;

    public OwnershipStrength(int value) {
        this.value = value;
    }

    public static OwnershipStrength of(int value) {
        return new OwnershipStrength(value);
    }

    public int value() {
        return value;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof OwnershipStrength)) return false;
        return value == ((OwnershipStrength) o).value;
    }

    @Override
    public int hashCode() {
        return Integer.hashCode(value);
    }

    @Override
    public String toString() {
        return "OwnershipStrength(" + value + ")";
    }
}
