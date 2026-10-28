// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.Collections;
import java.util.List;

/**
 * A matched endpoint with what it announced in discovery (DDS-RPC 1.0 §7.6.2.1.2): its own
 * GUID, its {@code service_instance_name}, the GUID of its related endpoint
 * ({@code related_datawriter_key} / {@code related_datareader_key}) and its
 * {@code topic_aliases}.
 */
public final class RpcPeerInfo {

    private final byte[] guid;
    private final String instanceName;
    private final byte[] relatedGuid;
    private final List<String> aliases;

    /**
     * @param guid the 16-octet GUID of the endpoint, or {@code null}
     * @param instanceName {@code service_instance_name}, {@code ""} when absent
     * @param relatedGuid the GUID of the related endpoint, or {@code null} when absent
     * @param aliases the topic aliases
     */
    public RpcPeerInfo(byte[] guid, String instanceName, byte[] relatedGuid, List<String> aliases) {
        this.guid = guid == null ? null : guid.clone();
        this.instanceName = instanceName == null ? "" : instanceName;
        this.relatedGuid = relatedGuid == null ? null : relatedGuid.clone();
        this.aliases = aliases == null ? Collections.<String>emptyList() : aliases;
    }

    /** The GUID of the endpoint, or {@code null}. */
    public byte[] guid() {
        return guid == null ? null : guid.clone();
    }

    /** The service instance name, {@code ""} when the endpoint announced none. */
    public String instanceName() {
        return instanceName;
    }

    /** The GUID of the related endpoint, or {@code null} when none was announced. */
    public byte[] relatedGuid() {
        return relatedGuid == null ? null : relatedGuid.clone();
    }

    /** The topic aliases the endpoint announced. */
    public List<String> aliases() {
        return aliases;
    }

    /** Whether the endpoint announced any of the three extensions. */
    public boolean announced() {
        return !instanceName.isEmpty() || relatedGuid != null || !aliases.isEmpty();
    }
}
