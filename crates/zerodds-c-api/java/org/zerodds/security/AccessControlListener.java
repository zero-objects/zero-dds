// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

/**
 * AccessControlListener (DDS-Security 1.2 9.4.2.10): the runtime gives it to
 * the plugin with {@code setListener}; the plugin calls it when it revokes
 * permissions.
 */
public interface AccessControlListener {
    /** The plugin revoked the permissions behind {@code handle}. */
    boolean onRevokePermissions(String plugin, long handle);
}
