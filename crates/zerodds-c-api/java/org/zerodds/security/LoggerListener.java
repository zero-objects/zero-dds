// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

/** LoggerListener (DDS-Security 1.2 9.6.2.3): the plugin calls it for every message it logs. */
public interface LoggerListener {
    /** A logged message. */
    int logMessage(int level, byte[] participant, String category, String message);
}
