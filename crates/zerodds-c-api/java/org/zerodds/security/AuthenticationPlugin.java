// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import java.util.List;
import org.zerodds.security.Types.AlgorithmMasks;
import org.zerodds.security.Types.BinaryProperty;
import org.zerodds.security.Types.HandshakeStep;
import org.zerodds.security.Types.LocalIdentityValidation;
import org.zerodds.security.Types.Property;
import org.zerodds.security.Types.RemoteIdentityValidation;

/**
 * Authentication plugin (DDS-Security 1.2 9.3.2.11, Table 29). A token is the
 * CDR of a DataHolder. A token an operation returns is given back with the
 * matching {@code return*} operation after the runtime copied it. An
 * operation that fails throws {@link SecurityException}.
 */
public interface AuthenticationPlugin {
    String pluginClassId();

    LocalIdentityValidation validateLocalIdentity(
            int domainId,
            List<Property> properties,
            List<BinaryProperty> binaryProperties,
            byte[] candidateParticipantGuid);

    byte[] getIdentityToken(long identity);

    byte[] getIdentityStatusToken(long identity);

    AlgorithmMasks setParticipantSecurityConfig(long identity, AlgorithmMasks config);

    void setPermissionsCredentialAndToken(long identity, byte[] credentialToken, byte[] permissionsToken);

    /** {@code remoteAuthRequestToken} is {@code null} for none. */
    RemoteIdentityValidation validateRemoteIdentity(
            long localIdentity,
            byte[] remoteParticipantGuid,
            byte[] remoteIdentityToken,
            byte[] remoteAuthRequestToken);

    /** The handle of the handshake is {@code HandshakeStep.handshake}. */
    HandshakeStep beginHandshakeRequest(long initiator, long replier, byte[] serializedLocalParticipantData);

    /** The handle of the handshake is {@code HandshakeStep.handshake}. */
    HandshakeStep beginHandshakeReply(
            long replier, long initiator, byte[] requestToken, byte[] serializedLocalParticipantData);

    HandshakeStep processHandshake(long handshake, byte[] token);

    long getSharedSecret(long handshake);

    byte[] getAuthenticatedPeerCredentialToken(long handshake);

    /** {@code listener} is {@code null} for none. */
    void setListener(AuthenticationListener listener);

    void returnIdentityToken(byte[] token);

    void returnIdentityStatusToken(byte[] token);

    void returnAuthenticatedPeerCredentialToken(byte[] token);

    void returnHandshakeHandle(long handshake);

    void returnIdentityHandle(long identity);

    void returnSharedsecretHandle(long secret);

    /** The PermissionsToken the participant announces; empty for none. */
    default byte[] getPermissionsToken() {
        return new byte[0];
    }

    /** The octets of a SharedSecret, where the builtin Cryptographic plugin is used; {@code null} for an unknown handle. */
    default byte[] getSharedSecretBytes(long secret) {
        return null;
    }

    /** Fills the two challenges of the handshake behind a SharedSecret; false for an unknown handle. */
    default boolean getSharedSecretChallenges(long secret, byte[] challenge1, byte[] challenge2) {
        return false;
    }

    /** Called once when the runtime lets go of the plugin. */
    void destroy();
}
