// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/** The header every request sample carries — DDS-RPC 1.0 §7.5.1.1.1. */
public final class RequestHeader {

    private final SampleIdentity requestId;
    private final String instanceName;

    /**
     * The header before a requester stamps it: {@link SampleIdentity#UNKNOWN} and the default
     * instance. The constructor of the machine-readable {@code RequestHeader.java}.
     */
    public RequestHeader() {
        this(null, null);
    }

    public RequestHeader(SampleIdentity requestId, String instanceName) {
        this.requestId = requestId == null ? SampleIdentity.UNKNOWN : requestId;
        this.instanceName = instanceName == null ? "" : instanceName;
    }

    /** The identity the client correlates the reply by — §7.8.1. */
    public SampleIdentity requestId() {
        return requestId;
    }

    /** The service instance this request is bound to, empty for any. */
    public String instanceName() {
        return instanceName;
    }
}
