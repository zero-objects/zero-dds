// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

/**
 * OMG DDS Java-PSM ReturnCode — Spec §7.2.1.1.
 *
 * <p>Numeric values match the canonical DDS specification (§2.2.1.1 in DCPS).
 */
public enum ReturnCode {
    OK(0),
    ERROR(1),
    UNSUPPORTED(2),
    BAD_PARAMETER(3),
    PRECONDITION_NOT_MET(4),
    OUT_OF_RESOURCES(5),
    NOT_ENABLED(6),
    IMMUTABLE_POLICY(7),
    INCONSISTENT_POLICY(8),
    ALREADY_DELETED(9),
    TIMEOUT(10),
    NO_DATA(11),
    ILLEGAL_OPERATION(12),
    NOT_ALLOWED_BY_SECURITY(13);

    private final int code;

    ReturnCode(int code) {
        this.code = code;
    }

    public int code() {
        return code;
    }

    public boolean isOk() {
        return this == OK;
    }

    public static ReturnCode fromCode(int code) {
        for (ReturnCode rc : values()) {
            if (rc.code == code) {
                return rc;
            }
        }
        throw new IllegalArgumentException("unknown ReturnCode " + code);
    }
}
