// DDS-RPC 1.0 §7.11.2.1 — RemoteException for transport-/dispatch-level
// failures. RuntimeException-Subclass so client code does not need to
// declare it on every call.
package org.zerodds.rpc;

/**
 * Thrown by Requesters and Repliers when a request cannot be completed
 * for transport, dispatch or unknown-operation reasons.
 *
 * <p>User-defined IDL exceptions extend {@link UserException} (which is
 * itself a RemoteException with code {@link RemoteExceptionCode#OK}) so
 * a single catch-all can branch on {@link #errorCode()}.
 */
public class RemoteException extends RuntimeException {
    private static final long serialVersionUID = 1L;
    private final RemoteExceptionCode errorCode;

    public RemoteException(String message, RemoteExceptionCode code) {
        super(message);
        this.errorCode = code != null ? code : RemoteExceptionCode.UNKNOWN_EXCEPTION;
    }

    public RemoteException(String message, Throwable cause, RemoteExceptionCode code) {
        super(message, cause);
        this.errorCode = code != null ? code : RemoteExceptionCode.UNKNOWN_EXCEPTION;
    }

    public RemoteException(Throwable cause) {
        super(cause);
        this.errorCode = RemoteExceptionCode.UNKNOWN_EXCEPTION;
    }

    /** Returns the wire-level error code carried in the ReplyHeader. */
    public RemoteExceptionCode errorCode() {
        return errorCode;
    }
}
