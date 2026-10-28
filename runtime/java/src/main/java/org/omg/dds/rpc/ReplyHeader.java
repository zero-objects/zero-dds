// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/** The header every reply sample carries — DDS-RPC 1.0 §7.5.1.1.1. */
public final class ReplyHeader {

    private final SampleIdentity relatedRequestId;
    private final RemoteExceptionCode remoteEx;

    /**
     * The header before a replier stamps it: {@link SampleIdentity#UNKNOWN} and
     * {@code REMOTE_EX_OK}. The constructor of the machine-readable {@code ReplyHeader.java}.
     */
    public ReplyHeader() {
        this(SampleIdentity.UNKNOWN, RemoteExceptionCode.REMOTE_EX_OK);
    }

    public ReplyHeader(SampleIdentity relatedRequestId, RemoteExceptionCode remoteEx) {
        this.relatedRequestId = relatedRequestId == null ? SampleIdentity.UNKNOWN : relatedRequestId;
        this.remoteEx = remoteEx == null ? RemoteExceptionCode.REMOTE_EX_OK : remoteEx;
    }

    /** The request this reply answers — §7.8.1. */
    public SampleIdentity relatedRequestId() {
        return relatedRequestId;
    }

    /** The outcome — §7.5.2. */
    public RemoteExceptionCode remoteEx() {
        return remoteEx;
    }
}
