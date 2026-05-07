// DDS-RPC 1.0 §7.11.2 — ServiceContext (per-invocation context object).
//
// Exposes the inbound RequestHeader and any participant traits the
// runtime wants to surface to the handler. Skeleton-only: the runtime
// fills in the fields when dispatching; user code reads them.
package org.zerodds.rpc;

/**
 * Per-invocation context carried alongside a service handler call.
 * Mirrors the Spec's `ServiceContext` (Java PSM).
 */
public final class ServiceContext {
    private final RequestHeader requestHeader;
    private final String serviceName;

    public ServiceContext(RequestHeader requestHeader, String serviceName) {
        this.requestHeader = requestHeader;
        this.serviceName = serviceName == null ? "" : serviceName;
    }

    public RequestHeader requestHeader() {
        return requestHeader;
    }

    public String serviceName() {
        return serviceName;
    }
}
