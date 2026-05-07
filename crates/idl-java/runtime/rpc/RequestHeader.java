// DDS-RPC 1.0 §7.5.1.1.1 — RequestHeader (request_id + instance_name).
package org.zerodds.rpc;

import java.util.Objects;

/**
 * Header prepended to every sample on the request topic. Mirrors
 * {@code dds_rpc::common_types::RequestHeader}.
 */
public final class RequestHeader {
    private final SampleIdentity requestId;
    private final String instanceName;

    public RequestHeader(SampleIdentity requestId, String instanceName) {
        this.requestId = Objects.requireNonNull(requestId, "requestId");
        this.instanceName = instanceName == null ? "" : instanceName;
    }

    public SampleIdentity requestId() {
        return requestId;
    }

    public String instanceName() {
        return instanceName;
    }
}
