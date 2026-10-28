// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import org.zerodds.security.Types.DataHolder;
import org.zerodds.security.Types.EndpointConfig;
import org.zerodds.security.Types.EndpointRequest;
import org.zerodds.security.Types.ParticipantConfig;
import org.zerodds.security.Types.ParticipantRequest;
import org.zerodds.security.Types.RelayRole;
import org.zerodds.security.Types.RemoteReaderDecision;
import org.zerodds.security.Types.TopicConfig;
import org.zerodds.security.Types.TopicRequest;

/**
 * AccessControl plugin (DDS-Security 1.2 9.4.2.9, Table 39). Handles are
 * 64-bit numbers, 0 is HandleNIL. An operation that fails throws
 * {@link SecurityException}; a refusal is {@code false}, not an exception.
 * The runtime calls the operations of one plugin one at a time.
 */
public interface AccessControlPlugin {
    String pluginClassId();

    long validateLocalPermissions(long identity, ParticipantRequest participant);

    long validateRemotePermissions(
            long localIdentity,
            long remoteIdentity,
            DataHolder remotePermissionsToken,
            DataHolder remoteCredentialToken);

    boolean checkCreateParticipant(long permissions, ParticipantRequest participant);

    boolean checkCreateDatawriter(long permissions, EndpointRequest request);

    boolean checkCreateDatareader(long permissions, EndpointRequest request);

    boolean checkCreateTopic(long permissions, TopicRequest request);

    boolean checkLocalDatawriterRegisterInstance(long permissions, byte[] writer, byte[] key);

    boolean checkLocalDatawriterDisposeInstance(long permissions, byte[] writer, byte[] key);

    boolean checkRemoteParticipant(long permissions, ParticipantRequest participant);

    boolean checkRemoteDatawriter(long permissions, EndpointRequest request);

    RemoteReaderDecision checkRemoteDatareader(long permissions, EndpointRequest request);

    boolean checkRemoteTopic(long permissions, TopicRequest request);

    boolean checkLocalDatawriterMatch(
            long writerPermissions,
            long readerPermissions,
            EndpointRequest publication,
            EndpointRequest subscription);

    boolean checkLocalDatareaderMatch(
            long writerPermissions,
            long readerPermissions,
            EndpointRequest subscription,
            EndpointRequest publication);

    boolean checkRemoteDatawriterRegisterInstance(
            long permissions, byte[] reader, byte[] publication, byte[] key);

    boolean checkRemoteDatawriterDisposeInstance(
            long permissions, byte[] reader, byte[] publication, byte[] key);

    boolean checkRelay(long permissions, RelayRole role, EndpointRequest request);

    DataHolder getPermissionsToken(long handle);

    DataHolder getPermissionsCredentialToken(long handle);

    /** The listener belongs to the runtime and lives as long as the plugin. */
    void setListener(AccessControlListener listener);

    void returnPermissionsToken(DataHolder token);

    void returnPermissionsCredentialToken(DataHolder token);

    ParticipantConfig getParticipantSecurityConfig(long permissions);

    TopicConfig getTopicSecurityConfig(long permissions, String topicName);

    EndpointConfig getDatawriterSecurityConfig(long permissions, EndpointRequest request);

    EndpointConfig getDatareaderSecurityConfig(long permissions, EndpointRequest request);

    void returnParticipantSecurityConfig(ParticipantConfig config);

    void returnTopicSecurityConfig(TopicConfig config);

    void returnDatawriterSecurityConfig(EndpointConfig config);

    void returnDatareaderSecurityConfig(EndpointConfig config);

    /** Called once when the runtime lets go of the plugin. */
    void destroy();
}
