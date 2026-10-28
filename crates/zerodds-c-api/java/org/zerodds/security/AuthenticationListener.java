// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

/** AuthenticationListener (DDS-Security 1.2 9.3.2.12): the runtime gives it to the plugin. */
public interface AuthenticationListener {
    /** The plugin revoked the identity behind {@code handle}; returns 0 for success. */
    int onRevokeIdentity(long handle);

    /** The status of the identity behind {@code handle} changed. */
    void onStatusChanged(long handle, int statusKind);
}
