// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import java.util.ArrayList;
import java.util.List;

/** The data types of the plugin interfaces (DDS-Security 1.2 clause 8 and 9). */
public final class Types {
    private Types() {}

    /** Property_t (Table 5). */
    public static final class Property {
        public String name = "";
        public String value = "";
        public boolean propagate;

        public Property() {}

        public Property(String name, String value) {
            this.name = name;
            this.value = value;
        }
    }

    /** BinaryProperty_t (Table 5). */
    public static final class BinaryProperty {
        public String name = "";
        public byte[] value = new byte[0];
        public boolean propagate;
    }

    /** DataHolder (Table 6): the form of every token. */
    public static final class DataHolder {
        public String classId = "";
        public List<Property> properties = new ArrayList<>();
        public List<BinaryProperty> binaryProperties = new ArrayList<>();
    }

    /** A name and a value: a QoS entry, a data tag. */
    public static final class Pair {
        public final String name;
        public final String value;

        public Pair(String name, String value) {
            this.name = name;
            this.value = value;
        }
    }

    /** (supported, required) algorithm masks (7.3.10). */
    public static final class Masks {
        public int supportedMask;
        public int requiredMask;
    }

    /** The algorithms the Authentication plugin announces. */
    public static final class AlgorithmMasks {
        public Masks trustChain = new Masks();
        public Masks messageAuth = new Masks();
        public Masks keyEstablishment = new Masks();
    }

    /** ParticipantSecurityAlgorithmInfo (7.3.14). */
    public static final class ParticipantAlgorithmInfo {
        public Masks trustChain = new Masks();
        public Masks messageAuth = new Masks();
        public Masks sharedSecret = new Masks();
        public int cipherSupportedMask;
        public int cipherBuiltinEndpointsRequiredMask;
        public int cipherBuiltinKxEndpointsRequiredMask;
        public int cipherUserEndpointsDefaultRequiredMask;
    }

    /** What the runtime asks the plugin about a participant. */
    public static final class ParticipantRequest {
        public int domainId;
        public String domainTag = "";
        public byte[] participantGuid = new byte[16];
        public List<Property> properties = new ArrayList<>();
        public boolean hasIdentityAlgorithms;
        public AlgorithmMasks identityAlgorithms = new AlgorithmMasks();
    }

    /** ... about a topic. */
    public static final class TopicRequest {
        public int domainId;
        public String topicName = "";
        public List<Pair> qos = new ArrayList<>();
    }

    /** ... about a DataWriter or DataReader. */
    public static final class EndpointRequest {
        public int domainId;
        public String topicName = "";
        public List<String> partitions = new ArrayList<>();
        public List<Pair> dataTags = new ArrayList<>();
        public List<Pair> qos = new ArrayList<>();
    }

    /** ParticipantSecurityConfig (9.4.2.4). */
    public static final class ParticipantConfig {
        public boolean allowUnauthenticatedParticipants;
        public boolean isAccessProtected;
        public boolean isRtpsAxkProtected;
        public boolean isRtpsPskProtected;
        public boolean isDiscoveryProtected;
        public boolean isLivelinessProtected;
        public boolean isKeyRevisionEnabled;
        public int pluginParticipantAttributes;
        public List<Property> acParticipantProperties = new ArrayList<>();
        public ParticipantAlgorithmInfo algorithmInfo = new ParticipantAlgorithmInfo();
    }

    /** TopicSecurityConfig (9.4.2.6). */
    public static final class TopicConfig {
        public boolean isReadProtected;
        public boolean isWriteProtected;
        public boolean isDiscoveryProtected;
        public boolean isLivelinessProtected;
    }

    /** EndpointSecurityConfig (9.4.2.7). */
    public static final class EndpointConfig {
        public TopicConfig topic = new TopicConfig();
        public boolean isSubmessageProtected;
        public boolean isPayloadProtected;
        public boolean isKeyProtected;
        public int pluginEndpointAttributes;
        public List<Property> acEndpointProperties = new ArrayList<>();
        public int cipherRequiredMask;
        public int cipherSupportedMask;
    }

    /** The answer of check_remote_datareader (9.4.2.9.11). */
    public enum RemoteReaderDecision {
        DENY,
        PERMIT,
        RELAY_ONLY
    }

    /** The role of check_relay. */
    public enum RelayRole {
        READER,
        WRITER
    }

    /** LogOptions (Table 45). */
    public static final class LogOptions {
        public int logLevel;
        /** {@code null} for none. */
        public String logFile;
        public boolean distribute;
    }

    /** A DataTag (9.7). */
    public static final class DataTag {
        public final String name;
        public final String value;

        public DataTag(String name, String value) {
            this.name = name;
            this.value = value;
        }
    }

    /** The levels of LogOptions (9.6.2.1.1). */
    public static final class LogLevel {
        private LogLevel() {}

        public static final int FATAL = 0;
        public static final int SEVERE = 1;
        public static final int ERROR = 2;
        public static final int WARNING = 3;
        public static final int NOTICE = 4;
        public static final int INFO = 5;
        public static final int DEBUG = 6;
        public static final int TRACE = 7;
    }

    /** ValidationResult (9.3.2.11). */
    public static final class Validation {
        private Validation() {}

        public static final int OK = 0;
        public static final int FAILED = 1;
        public static final int PENDING_RETRY = 2;
        public static final int PENDING_HANDSHAKE_REQUEST = 3;
        public static final int PENDING_HANDSHAKE_MESSAGE = 4;
        public static final int OK_FINAL_MESSAGE = 5;
    }

    /** The result of validate_local_identity. */
    public static final class LocalIdentityValidation {
        public int result = Validation.OK;
        public long identity;
        public byte[] adjustedGuid = new byte[16];
    }

    /** The result of validate_remote_identity. */
    public static final class RemoteIdentityValidation {
        public int result = Validation.OK;
        public long identity;
        /** {@code null} for none. */
        public byte[] localAuthRequestToken;
    }

    /** What a step of the handshake gives. */
    public static final class HandshakeStep {
        public static final int SEND_MESSAGE = 0;
        public static final int SEND_FINAL_MESSAGE = 1;
        public static final int COMPLETE = 2;
        public static final int WAITING_FOR_PEER = 3;
        public static final int PENDING_RETRY = 4;

        public int kind = WAITING_FOR_PEER;
        public byte[] token = new byte[0];
        public long secret;
        /** The handle of the handshake (begin_handshake_request and _reply). */
        public long handshake;
    }

    /** EndpointSecurityAlgorithmInfo (7.3.16). */
    public static final class EndpointAlgorithmInfo {
        public int requiredMask;
        public int supportedMask;
    }

    /** An encoded serialized payload. */
    public static final class EncodedPayload {
        public byte[] encoded = new byte[0];
        public byte[] extraInlineQos = new byte[0];
    }

    /** The result of preprocess_secure_submsg. */
    public static final class PreprocessedSubmessage {
        public static final int INFO = 0;
        public static final int DATAWRITER = 1;
        public static final int DATAREADER = 2;

        public long datawriter;
        public long datareader;
        public int category = INFO;
    }
}
