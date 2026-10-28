// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

/**
 * A matched remote or local endpoint with what it announced for DDS-RPC 1.0 §7.6.2.1: its own
 * GUID and its {@link RpcDiscovery}.
 */
public final class RpcPeer {

    private final byte[] guid;
    private final RpcDiscovery rpc;

    /**
     * @param guid the 16-octet GUID of the endpoint, or {@code null} when the binding gives its
     *     endpoints none
     * @param rpc what it announced
     */
    public RpcPeer(byte[] guid, RpcDiscovery rpc) {
        this.guid = guid == null ? null : guid.clone();
        this.rpc = rpc == null ? RpcDiscovery.NONE : rpc;
    }

    /** The GUID of the endpoint, or {@code null}. */
    public byte[] guid() {
        return guid == null ? null : guid.clone();
    }

    /** What the endpoint announced. */
    public RpcDiscovery rpc() {
        return rpc;
    }

    @Override
    public String toString() {
        return "RpcPeer{" + rpc + "}";
    }
}
