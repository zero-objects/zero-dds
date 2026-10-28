// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The error codes of DDS-RPC 1.0 §7.5.1.1.1's
 * {@code RemoteExceptionCode_t}, in the order the IDL declares them.
 */
public enum RemoteExceptionCode {
    REMOTE_EX_OK(0),
    REMOTE_EX_UNSUPPORTED(1),
    REMOTE_EX_INVALID_ARGUMENT(2),
    REMOTE_EX_OUT_OF_RESOURCES(3),
    REMOTE_EX_UNKNOWN_OPERATION(4),
    REMOTE_EX_UNKNOWN_EXCEPTION(5);

    private final int wireValue;

    RemoteExceptionCode(int wireValue) {
        this.wireValue = wireValue;
    }

    /** The discriminator the reply header carries. */
    public int wireValue() {
        return wireValue;
    }

    /**
     * The code a wire discriminator names.
     *
     * @throws RemoteUnknownExceptionError for a value outside the six the
     *     enum declares — a reply this binding did not understand, which
     *     is reported rather than read as success.
     */
    public static RemoteExceptionCode fromWire(int value) {
        for (RemoteExceptionCode code : values()) {
            if (code.wireValue == value) {
                return code;
            }
        }
        throw new RemoteUnknownExceptionError("remote exception code " + value + " is not one of the six");
    }

    /**
     * Raise the exception §7.11.1.3's table gives this code, or return
     * for {@code REMOTE_EX_OK}. §7.11.2.1 has no body in the published
     * document and §7.11.2's preamble makes the C++ table stand.
     */
    public void raise(String what) {
        switch (this) {
            case REMOTE_EX_OK:
                return;
            case REMOTE_EX_UNSUPPORTED:
                throw new RemoteUnsupportedError(what);
            case REMOTE_EX_INVALID_ARGUMENT:
                throw new RemoteInvalidArgumentError(what);
            case REMOTE_EX_OUT_OF_RESOURCES:
                throw new RemoteOutOfResourcesError(what);
            case REMOTE_EX_UNKNOWN_OPERATION:
                throw new RemoteUnknownOperationError(what);
            case REMOTE_EX_UNKNOWN_EXCEPTION:
            default:
                throw new RemoteUnknownExceptionError(what);
        }
    }
}
