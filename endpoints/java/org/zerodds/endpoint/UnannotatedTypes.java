// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

package org.zerodds.endpoint;

/**
 * Extensibility assumed for the DDS-XRCE 1.0 Annex A types that carry no
 * {@code @extensibility} annotation (here: ACKNACK_Payload).
 *
 * <ul>
 *   <li>{@link #APPENDABLE} (default): DDS-XTypes 1.3 7.3.1.2.1.8 -- the
 *       struct is preceded by a DHEADER. ACKNACK body: 9 octets, frame 17.</li>
 *   <li>{@link #FINAL}: the form of the reference implementation (eProsima) --
 *       no DHEADER. ACKNACK body: 5 octets, frame 13.</li>
 * </ul>
 *
 * Readers accept both forms in either setting.
 */
public enum UnannotatedTypes {
    APPENDABLE,
    FINAL
}
