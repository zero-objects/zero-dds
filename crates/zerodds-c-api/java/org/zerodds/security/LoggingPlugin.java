// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import org.zerodds.security.Types.LogOptions;

/** Logging plugin (DDS-Security 1.2 9.6.2.2, Table 46). */
public interface LoggingPlugin {
    String pluginClassId();

    void setLogOptions(LogOptions options);

    void enableLogging();

    /** {@code listener} is {@code null} for none. */
    void setListener(LoggerListener listener);

    /** Logs a message; logging does not fail the operation that logs. */
    void log(int level, byte[] participant, String category, String message);

    /** Called once when the runtime lets go of the plugin. */
    void destroy();
}
