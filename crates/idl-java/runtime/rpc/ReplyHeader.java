// DDS-RPC 1.0 §7.5.1.1.1 — ReplyHeader (related_request_id + remote_ex).
package org.zerodds.rpc;

import java.util.Objects;

/**
 * Header prepended to every sample on the reply topic. Mirrors
 * {@code dds_rpc::common_types::ReplyHeader}.
 */
public final class ReplyHeader {
    private final SampleIdentity relatedRequestId;
    private final RemoteExceptionCode remoteEx;

    public ReplyHeader(SampleIdentity relatedRequestId, RemoteExceptionCode remoteEx) {
        this.relatedRequestId = Objects.requireNonNull(relatedRequestId, "relatedRequestId");
        this.remoteEx = remoteEx == null ? RemoteExceptionCode.OK : remoteEx;
    }

    public SampleIdentity relatedRequestId() {
        return relatedRequestId;
    }

    public RemoteExceptionCode remoteEx() {
        return remoteEx;
    }
}
