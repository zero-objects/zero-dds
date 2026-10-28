// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import java.util.List;
import org.zerodds.security.Types.DataHolder;
import org.zerodds.security.Types.EncodedPayload;
import org.zerodds.security.Types.EndpointAlgorithmInfo;
import org.zerodds.security.Types.EndpointConfig;
import org.zerodds.security.Types.ParticipantAlgorithmInfo;
import org.zerodds.security.Types.ParticipantConfig;
import org.zerodds.security.Types.PreprocessedSubmessage;
import org.zerodds.security.Types.Property;

/**
 * Cryptographic plugin (DDS-Security 1.2 9.5.1, Tables 42 to 44):
 * CryptoKeyFactory, CryptoKeyExchange and CryptoTransform in one interface. A
 * {@code null} result of a create or encode or decode operation is the answer
 * "none" / "does not apply" (FALSE without an exception); a failure throws
 * {@link SecurityException}. {@code index} arrays hold the
 * {@code receiving_*_crypto_list_index} in {@code [0]}, in and out.
 */
public interface CryptographicPlugin {
    long registerLocalParticipant(
            long identity,
            long permissions,
            List<Property> properties,
            ParticipantConfig config,
            ParticipantAlgorithmInfo info);

    long registerMatchedRemoteParticipant(
            long localParticipant, long remoteIdentity, long remotePermissions, long sharedSecret);

    long registerLocalDatawriter(
            long participant,
            List<Property> properties,
            EndpointConfig config,
            byte[] endpointGuid,
            EndpointAlgorithmInfo info);

    long registerMatchedRemoteDatareader(
            long localDatawriter, long remoteParticipant, long sharedSecret, boolean relayOnly);

    long registerLocalDatareader(
            long participant,
            List<Property> properties,
            EndpointConfig config,
            byte[] endpointGuid,
            EndpointAlgorithmInfo info);

    long registerMatchedRemoteDatawriter(long localDatareader, long remoteParticipant, long sharedSecret);

    int reviseLocalEntityKeys(long participant);

    boolean activateKeyRevision(long participant, int revision);

    boolean unregisterParticipant(long handle);

    boolean unregisterDatawriter(long handle);

    boolean unregisterDatareader(long handle);

    List<DataHolder> createLocalParticipantCryptoTokens(long localParticipant, long remoteParticipant, int keyRevision);

    boolean setRemoteParticipantCryptoTokens(long localParticipant, long remoteParticipant, List<DataHolder> tokens);

    List<DataHolder> createLocalDatawriterCryptoTokens(long localDatawriter, long remoteDatareader, int keyRevision);

    boolean setRemoteDatawriterCryptoTokens(long localDatareader, long remoteDatawriter, List<DataHolder> tokens);

    List<DataHolder> createLocalDatareaderCryptoTokens(long localDatareader, long remoteDatawriter, int keyRevision);

    boolean setRemoteDatareaderCryptoTokens(long localDatawriter, long remoteDatareader, List<DataHolder> tokens);

    boolean returnCryptoTokens(List<DataHolder> tokens);

    EncodedPayload encodeSerializedPayload(byte[] plain, long sendingDatawriter);

    byte[] encodeDatawriterSubmessage(byte[] plain, long sendingDatawriter, long[] receivingDatareaders, int[] index);

    byte[] encodeDatareaderSubmessage(byte[] plain, long sendingDatareader, long[] receivingDatawriters);

    byte[] encodeRtpsMessage(
            byte[] plain, long sendingParticipant, long[] receivingParticipants, int[] index, boolean transformWithPsk);

    byte[] decodeRtpsMessage(byte[] encoded, long receivingParticipant, long sendingParticipant);

    PreprocessedSubmessage preprocessSecureSubmsg(
            byte[] encodedSubmessage, long receivingParticipant, long sendingParticipant);

    byte[] decodeDatawriterSubmessage(byte[] encodedSubmessage, long receivingDatareader, long sendingDatawriter);

    byte[] decodeDatareaderSubmessage(byte[] encodedSubmessage, long receivingDatawriter, long sendingDatareader);

    byte[] decodeSerializedPayload(
            byte[] encoded, byte[] inlineQos, long receivingDatareader, long sendingDatawriter);

    /** Called once when the runtime lets go of the plugin. */
    void destroy();
}
