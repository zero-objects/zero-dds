// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import java.util.List;
import org.zerodds.security.Types.DataTag;

/** DataTagging plugin (DDS-Security 1.2 9.7). */
public interface DataTaggingPlugin {
    String pluginClassId();

    void setTags(byte[] endpoint, List<DataTag> tags);

    List<DataTag> getTags(byte[] endpoint);

    /** Called once when the runtime lets go of the plugin. */
    void destroy();
}
