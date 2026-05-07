// DDS-RPC 1.0 §7.11.2.1 — Marker base class for IDL-declared exceptions.
//
// A UserException carries the OK status code by default — it is a
// well-known result, not a transport failure. The replier serializes
// the exception via the Reply union and the requester re-throws it on
// the caller side.
package org.zerodds.rpc;

/** Base class for IDL-declared exceptions exported by a service. */
public class UserException extends RemoteException {
    private static final long serialVersionUID = 1L;

    public UserException(String message) {
        super(message, RemoteExceptionCode.OK);
    }

    public UserException(String message, Throwable cause) {
        super(message, cause, RemoteExceptionCode.OK);
    }
}
