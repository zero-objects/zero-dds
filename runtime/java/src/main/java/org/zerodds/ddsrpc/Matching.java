// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.ServiceMapping;
import org.omg.dds.rpc.ServiceMatching;

/** Resolves {@link ServiceMatching#PER_MAPPING}. */
final class Matching {

    private Matching() {}

    /** Whether requests and replies wait for the other side to be matched. */
    static boolean gates(ServiceMatching matching, ServiceMapping mapping) {
        switch (matching) {
            case UNTIL_MATCHED:
                return true;
            case IMMEDIATE:
                return false;
            case PER_MAPPING:
            default:
                return mapping == ServiceMapping.ENHANCED;
        }
    }
}
