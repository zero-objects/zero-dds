// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * {@code REMOTE_EX_INVALID_ARGUMENT} — DDS-RPC 1.0 §7.11.1.3 through §7.11.2.1.
 *
 * <p>Unchecked, because Figure 7.6 puts every remote exception under
 * {@code std::runtime_error} or {@code std::logic_error} in C++, both of
 * which are thrown without being declared. That is also why
 * §7.11.1.1.2 rule 6.c gives a generated method no exception
 * specification and this binding's generated Java no {@code throws}
 * clause.
 */
public class RemoteInvalidArgumentError extends RuntimeException {

    private static final long serialVersionUID = 1L;

    public RemoteInvalidArgumentError(String message) {
        super(message);
    }
}
