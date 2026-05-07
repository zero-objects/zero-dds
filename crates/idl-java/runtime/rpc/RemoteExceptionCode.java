// DDS-RPC 1.0 §7.5.1.1.1 Tab.7.55 — Server-side error code carried in the
// ReplyHeader. Mirrors `dds_rpc::common_types::RemoteExceptionCode`.
package org.zerodds.rpc;

/** Wire-encoded RPC error code. */
public enum RemoteExceptionCode {
    /** Operation completed successfully. */
    OK(0),
    /** Operation is not supported by this service. */
    UNSUPPORTED(1),
    /** Argument value violates the operation contract. */
    INVALID_ARGUMENT(2),
    /** Server has insufficient resources to complete the request. */
    OUT_OF_RESOURCES(3),
    /** The requested operation does not exist on this service. */
    UNKNOWN_OPERATION(4),
    /** A user-defined exception was raised that the stub does not understand. */
    UNKNOWN_EXCEPTION(5),
    /** The service interface is not known to the server. */
    UNKNOWN_INTERFACE(6);

    private final int code;

    RemoteExceptionCode(int code) {
        this.code = code;
    }

    /** Returns the wire-level discriminator. */
    public int code() {
        return code;
    }

    /** Looks up an enum value from its wire-level discriminator. */
    public static RemoteExceptionCode fromCode(int code) {
        for (RemoteExceptionCode rec : values()) {
            if (rec.code == code) {
                return rec;
            }
        }
        throw new IllegalArgumentException("unknown RemoteExceptionCode: " + code);
    }
}
