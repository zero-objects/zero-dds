// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

import java.lang.foreign.AddressLayout;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import org.zerodds.security.AccessControlListener;
import org.zerodds.security.AccessControlPlugin;
import org.zerodds.security.AuthenticationListener;
import org.zerodds.security.AuthenticationPlugin;
import org.zerodds.security.CryptographicPlugin;
import org.zerodds.security.DataTaggingPlugin;
import org.zerodds.security.LoggerListener;
import org.zerodds.security.LoggingPlugin;
import org.zerodds.security.PluginBinder;
import org.zerodds.security.SecurityException;
import org.zerodds.security.Types.AlgorithmMasks;
import org.zerodds.security.Types.BinaryProperty;
import org.zerodds.security.Types.DataHolder;
import org.zerodds.security.Types.DataTag;
import org.zerodds.security.Types.EncodedPayload;
import org.zerodds.security.Types.EndpointAlgorithmInfo;
import org.zerodds.security.Types.EndpointConfig;
import org.zerodds.security.Types.EndpointRequest;
import org.zerodds.security.Types.HandshakeStep;
import org.zerodds.security.Types.LocalIdentityValidation;
import org.zerodds.security.Types.LogOptions;
import org.zerodds.security.Types.ParticipantAlgorithmInfo;
import org.zerodds.security.Types.ParticipantConfig;
import org.zerodds.security.Types.ParticipantRequest;
import org.zerodds.security.Types.PreprocessedSubmessage;
import org.zerodds.security.Types.Property;
import org.zerodds.security.Types.RelayRole;
import org.zerodds.security.Types.RemoteIdentityValidation;
import org.zerodds.security.Types.RemoteReaderDecision;
import org.zerodds.security.Types.Validation;
import org.zerodds.security.Types.TopicConfig;
import org.zerodds.security.Types.TopicRequest;

/**
 * A DDS-Security 1.2 Authentication, AccessControl, Cryptographic, Logging and
 * DataTagging plugin written in Java and placed into a participant.
 *
 * <pre>
 * java --enable-preview --enable-native-access=ALL-UNNAMED PluginTest
 *     libzerodds domain identity_ca identity_cert identity_key
 *     permissions_ca governance.p7s permissions.p7s
 * </pre>
 *
 * The AccessControl plugin refuses the DataWriter and the DataReader of the
 * topic "Forbidden" and permits every other operation. The Logging plugin
 * counts what the runtime logs; the DataTagging plugin gives every endpoint
 * the tag owner=java-plugin. The program checks that the runtime called each
 * plugin where the specification calls it and that it destroyed each plugin
 * when it let go of it; it prints "OK" and exits 0, or names the first check
 * that failed.
 */
public final class PluginTest {
    static final String FORBIDDEN = "Forbidden";

    static volatile int validateLocal, createParticipant, createWriter, createReader;
    static volatile int refusedWriter, refusedReader;
    static volatile int permissionsTokenOut, permissionsTokenBack;
    static volatile int credentialTokenOut, credentialTokenBack;
    static volatile int participantConfigOut, participantConfigBack;
    static volatile int writerConfigOut, writerConfigBack;
    static volatile int readerConfigOut, readerConfigBack;
    static volatile int listenerSet, acDestroyed;
    static volatile int options, enabled, messages, denied, lgDestroyed;
    static volatile int setTags, getTags, tgDestroyed;
    static volatile int auValidateLocal, auGovernance, auIdentityOut, auIdentityBack;
    static volatile int auStatusOut, auStatusBack, auCredential, auConfig, auCredentialBeforeConfig;
    static volatile int auDestroyed;
    static volatile int crRegisterParticipant, crParticipantAxk, crVolatileWriter, crVolatileReader;
    static volatile int crProtectedWriter, crDestroyed;

    static final class Access implements AccessControlPlugin {
        public String pluginClassId() {
            return "DDS:Access:Java";
        }

        public long validateLocalPermissions(long identity, ParticipantRequest p) {
            validateLocal++;
            return 77;
        }

        public long validateRemotePermissions(long li, long ri, DataHolder pt, DataHolder ct) {
            return 78;
        }

        public boolean checkCreateParticipant(long permissions, ParticipantRequest p) {
            if (permissions == 77 && Integer.compareUnsigned(p.domainId, 233) < 0) {
                createParticipant++;
            }
            return true;
        }

        public boolean checkCreateDatawriter(long permissions, EndpointRequest r) {
            createWriter++;
            boolean permit = !FORBIDDEN.equals(r.topicName);
            if (!permit) {
                refusedWriter++;
            }
            return permit;
        }

        public boolean checkCreateDatareader(long permissions, EndpointRequest r) {
            createReader++;
            boolean permit = !FORBIDDEN.equals(r.topicName);
            if (!permit) {
                refusedReader++;
            }
            return permit;
        }

        public boolean checkCreateTopic(long permissions, TopicRequest r) {
            return true;
        }

        public boolean checkLocalDatawriterRegisterInstance(long permissions, byte[] writer, byte[] key) {
            return true;
        }

        public boolean checkLocalDatawriterDisposeInstance(long permissions, byte[] writer, byte[] key) {
            return true;
        }

        public boolean checkRemoteParticipant(long permissions, ParticipantRequest p) {
            return true;
        }

        public boolean checkRemoteDatawriter(long permissions, EndpointRequest r) {
            return true;
        }

        public RemoteReaderDecision checkRemoteDatareader(long permissions, EndpointRequest r) {
            return RemoteReaderDecision.PERMIT;
        }

        public boolean checkRemoteTopic(long permissions, TopicRequest r) {
            return true;
        }

        public boolean checkLocalDatawriterMatch(long wp, long rp, EndpointRequest a, EndpointRequest b) {
            return true;
        }

        public boolean checkLocalDatareaderMatch(long wp, long rp, EndpointRequest a, EndpointRequest b) {
            return true;
        }

        public boolean checkRemoteDatawriterRegisterInstance(long permissions, byte[] reader, byte[] publication, byte[] key) {
            return true;
        }

        public boolean checkRemoteDatawriterDisposeInstance(long permissions, byte[] reader, byte[] publication, byte[] key) {
            return true;
        }

        public boolean checkRelay(long permissions, RelayRole role, EndpointRequest r) {
            return false;
        }

        private static DataHolder token(String classId) {
            DataHolder h = new DataHolder();
            h.classId = classId;
            h.properties.add(new Property("owner", "java-plugin"));
            return h;
        }

        public DataHolder getPermissionsToken(long handle) {
            permissionsTokenOut++;
            return token("DDS:Access:Permissions:Java");
        }

        public DataHolder getPermissionsCredentialToken(long handle) {
            credentialTokenOut++;
            return token("DDS:Access:Credential:Java");
        }

        public void setListener(AccessControlListener listener) {
            listenerSet++;
        }

        public void returnPermissionsToken(DataHolder t) {
            if (!"DDS:Access:Permissions:Java".equals(t.classId)) {
                throw new SecurityException(SecurityException.BAD_ARGUMENT, "not the token this plugin issued");
            }
            permissionsTokenBack++;
        }

        public void returnPermissionsCredentialToken(DataHolder t) {
            if (!"DDS:Access:Credential:Java".equals(t.classId)) {
                throw new SecurityException(SecurityException.BAD_ARGUMENT, "not the token this plugin issued");
            }
            credentialTokenBack++;
        }

        public ParticipantConfig getParticipantSecurityConfig(long permissions) {
            participantConfigOut++;
            // The plugin allows every algorithm and requires none.
            ParticipantConfig c = new ParticipantConfig();
            c.algorithmInfo.trustChain.supportedMask = -1;
            c.algorithmInfo.messageAuth.supportedMask = -1;
            c.algorithmInfo.sharedSecret.supportedMask = -1;
            c.algorithmInfo.cipherSupportedMask = -1;
            return c;
        }

        public TopicConfig getTopicSecurityConfig(long permissions, String topicName) {
            return new TopicConfig();
        }

        public EndpointConfig getDatawriterSecurityConfig(long permissions, EndpointRequest r) {
            writerConfigOut++;
            EndpointConfig c = new EndpointConfig();
            // The payload of the topic "Protected" is protected, and encrypted.
            if ("Protected".equals(r.topicName)) {
                c.isPayloadProtected = true;
                c.pluginEndpointAttributes = 2; // IS_PAYLOAD_ENCRYPTED
            }
            return c;
        }

        public EndpointConfig getDatareaderSecurityConfig(long permissions, EndpointRequest r) {
            readerConfigOut++;
            return new EndpointConfig();
        }

        public void returnParticipantSecurityConfig(ParticipantConfig c) {
            participantConfigBack++;
        }

        public void returnTopicSecurityConfig(TopicConfig c) {}

        public void returnDatawriterSecurityConfig(EndpointConfig c) {
            writerConfigBack++;
        }

        public void returnDatareaderSecurityConfig(EndpointConfig c) {
            readerConfigBack++;
        }

        public void destroy() {
            acDestroyed++;
        }
    }

    static final class Logging implements LoggingPlugin {
        public String pluginClassId() {
            return "DDS:Logging:Java";
        }

        public void setLogOptions(LogOptions o) {
            options++;
        }

        public void enableLogging() {
            enabled++;
        }

        public void setListener(LoggerListener listener) {}

        public void log(int level, byte[] participant, String category, String message) {
            messages++;
            if ("access.create_datawriter.denied".equals(category)) {
                denied++;
            }
        }

        public void destroy() {
            lgDestroyed++;
        }
    }

    static final class Tagging implements DataTaggingPlugin {
        public String pluginClassId() {
            return "DDS:Tagging:Java";
        }

        public void setTags(byte[] endpoint, List<DataTag> tags) {
            setTags++;
        }

        public List<DataTag> getTags(byte[] endpoint) {
            getTags++;
            List<DataTag> tags = new ArrayList<>();
            tags.add(new DataTag("owner", "java-plugin"));
            return tags;
        }

        public void destroy() {
            tgDestroyed++;
        }
    }

    static final byte[] IDENTITY_TOKEN = "J-IDENTITY-TOKEN".getBytes();
    static final byte[] STATUS_TOKEN = "J-IDENTITY-STATUS-TOKEN".getBytes();

    static final class Authentication implements AuthenticationPlugin {
        public String pluginClassId() {
            return "DDS:Auth:Java";
        }

        public LocalIdentityValidation validateLocalIdentity(
                int domainId, List<Property> properties, List<BinaryProperty> binary, byte[] guid) {
            auValidateLocal++;
            for (Property p : properties) {
                if ("dds.sec.access.governance".equals(p.name)) {
                    auGovernance = 1;
                }
            }
            LocalIdentityValidation v = new LocalIdentityValidation();
            v.result = Validation.OK;
            v.identity = 100;
            v.adjustedGuid = guid.clone();
            return v;
        }

        public byte[] getIdentityToken(long identity) {
            auIdentityOut++;
            return IDENTITY_TOKEN;
        }

        public byte[] getIdentityStatusToken(long identity) {
            auStatusOut++;
            return STATUS_TOKEN;
        }

        public AlgorithmMasks setParticipantSecurityConfig(long identity, AlgorithmMasks config) {
            auConfig++;
            if (auCredential > 0) {
                auCredentialBeforeConfig = 1;
            }
            return config;
        }

        public void setPermissionsCredentialAndToken(long identity, byte[] credential, byte[] token) {
            auCredential++;
        }

        public RemoteIdentityValidation validateRemoteIdentity(
                long local, byte[] guid, byte[] token, byte[] request) {
            RemoteIdentityValidation v = new RemoteIdentityValidation();
            v.result = Validation.FAILED;
            return v;
        }

        public HandshakeStep beginHandshakeRequest(long a, long b, byte[] data) {
            throw new SecurityException(SecurityException.NOT_IMPLEMENTED, "the test has no peer");
        }

        public HandshakeStep beginHandshakeReply(long a, long b, byte[] request, byte[] data) {
            throw new SecurityException(SecurityException.NOT_IMPLEMENTED, "the test has no peer");
        }

        public HandshakeStep processHandshake(long handshake, byte[] token) {
            throw new SecurityException(SecurityException.NOT_IMPLEMENTED, "the test has no peer");
        }

        public long getSharedSecret(long handshake) {
            throw new SecurityException(SecurityException.NOT_IMPLEMENTED, "the test has no peer");
        }

        public byte[] getAuthenticatedPeerCredentialToken(long handshake) {
            throw new SecurityException(SecurityException.NOT_IMPLEMENTED, "the test has no peer");
        }

        public void setListener(AuthenticationListener listener) {}

        public void returnIdentityToken(byte[] t) {
            if (!java.util.Arrays.equals(t, IDENTITY_TOKEN)) {
                throw new SecurityException(SecurityException.BAD_ARGUMENT, "not the token this plugin issued");
            }
            auIdentityBack++;
        }

        public void returnIdentityStatusToken(byte[] t) {
            if (!java.util.Arrays.equals(t, STATUS_TOKEN)) {
                throw new SecurityException(SecurityException.BAD_ARGUMENT, "not the token this plugin issued");
            }
            auStatusBack++;
        }

        public void returnAuthenticatedPeerCredentialToken(byte[] t) {}

        public void returnHandshakeHandle(long h) {}

        public void returnIdentityHandle(long h) {}

        public void returnSharedsecretHandle(long h) {}

        public byte[] getPermissionsToken() {
            return "J-PERMISSIONS-TOKEN".getBytes();
        }

        public void destroy() {
            auDestroyed++;
        }
    }

    /** The transform of this plugin puts one octet 0xC0 before what it protects. */
    static byte[] wrap(byte[] plain) {
        byte[] out = new byte[plain.length + 1];
        out[0] = (byte) 0xC0;
        System.arraycopy(plain, 0, out, 1, plain.length);
        return out;
    }

    static byte[] unwrap(byte[] encoded) {
        return encoded.length == 0 ? encoded : java.util.Arrays.copyOfRange(encoded, 1, encoded.length);
    }

    static void noteProperties(List<Property> properties, boolean writer) {
        for (Property p : properties) {
            if ("dds.sec.builtin_endpoint_name".equals(p.name)) {
                if (writer && "BuiltinParticipantVolatileMessageSecureWriter".equals(p.value)) {
                    crVolatileWriter = 1;
                }
                if (!writer && "BuiltinParticipantVolatileMessageSecureReader".equals(p.value)) {
                    crVolatileReader = 1;
                }
            }
        }
    }

    static final class Cryptographic implements CryptographicPlugin {
        int writers, readers;

        public long registerLocalParticipant(long identity, long permissions, List<Property> properties,
                ParticipantConfig config, ParticipantAlgorithmInfo info) {
            crRegisterParticipant++;
            crParticipantAxk = config.isRtpsAxkProtected ? 1 : 0;
            return 500;
        }

        public long registerMatchedRemoteParticipant(long local, long identity, long permissions, long secret) {
            return 501;
        }

        public long registerLocalDatawriter(long participant, List<Property> properties, EndpointConfig config,
                byte[] guid, EndpointAlgorithmInfo info) {
            writers++;
            noteProperties(properties, true);
            if (config.isPayloadProtected) {
                crProtectedWriter = 1;
            }
            return 600 + writers;
        }

        public long registerMatchedRemoteDatareader(long writer, long participant, long secret, boolean relayOnly) {
            return 800;
        }

        public long registerLocalDatareader(long participant, List<Property> properties, EndpointConfig config,
                byte[] guid, EndpointAlgorithmInfo info) {
            readers++;
            noteProperties(properties, false);
            return 700 + readers;
        }

        public long registerMatchedRemoteDatawriter(long reader, long participant, long secret) {
            return 801;
        }

        public int reviseLocalEntityKeys(long participant) {
            return 1;
        }

        public boolean activateKeyRevision(long participant, int revision) {
            return true;
        }

        public boolean unregisterParticipant(long h) {
            return true;
        }

        public boolean unregisterDatawriter(long h) {
            return true;
        }

        public boolean unregisterDatareader(long h) {
            return true;
        }

        public List<DataHolder> createLocalParticipantCryptoTokens(long a, long b, int revision) {
            return null;
        }

        public boolean setRemoteParticipantCryptoTokens(long a, long b, List<DataHolder> tokens) {
            return true;
        }

        public List<DataHolder> createLocalDatawriterCryptoTokens(long a, long b, int revision) {
            return null;
        }

        public boolean setRemoteDatawriterCryptoTokens(long a, long b, List<DataHolder> tokens) {
            return true;
        }

        public List<DataHolder> createLocalDatareaderCryptoTokens(long a, long b, int revision) {
            return null;
        }

        public boolean setRemoteDatareaderCryptoTokens(long a, long b, List<DataHolder> tokens) {
            return true;
        }

        public boolean returnCryptoTokens(List<DataHolder> tokens) {
            return true;
        }

        public EncodedPayload encodeSerializedPayload(byte[] plain, long writer) {
            EncodedPayload p = new EncodedPayload();
            p.encoded = wrap(plain);
            return p;
        }

        public byte[] encodeDatawriterSubmessage(byte[] plain, long writer, long[] readers, int[] index) {
            return wrap(plain);
        }

        public byte[] encodeDatareaderSubmessage(byte[] plain, long reader, long[] writers) {
            return wrap(plain);
        }

        public byte[] encodeRtpsMessage(byte[] plain, long participant, long[] receivers, int[] index, boolean psk) {
            return wrap(plain);
        }

        public byte[] decodeRtpsMessage(byte[] encoded, long receiving, long sending) {
            return unwrap(encoded);
        }

        public PreprocessedSubmessage preprocessSecureSubmsg(byte[] encoded, long receiving, long sending) {
            return new PreprocessedSubmessage();
        }

        public byte[] decodeDatawriterSubmessage(byte[] encoded, long reader, long writer) {
            return unwrap(encoded);
        }

        public byte[] decodeDatareaderSubmessage(byte[] encoded, long writer, long reader) {
            return unwrap(encoded);
        }

        public byte[] decodeSerializedPayload(byte[] encoded, byte[] inlineQos, long reader, long writer) {
            return unwrap(encoded);
        }

        public void destroy() {
            crDestroyed++;
        }
    }

    static void check(boolean condition, String what) {
        if (!condition) {
            System.err.println("check failed: " + what);
            System.exit(1);
        }
    }

    static MethodHandle handle(SymbolLookup lib, String name, FunctionDescriptor fd) {
        return Linker.nativeLinker().downcallHandle(lib.find(name).orElseThrow(), fd);
    }

    public static void main(String[] args) throws Throwable {
        if (args.length != 8) {
            System.err.println("usage: PluginTest libzerodds domain ca cert key permissions_ca governance permissions");
            System.exit(2);
        }
        AddressLayout ptr = ValueLayout.ADDRESS;
        ValueLayout.OfInt i32 = ValueLayout.JAVA_INT;
        Arena arena = Arena.global();
        SymbolLookup lib = SymbolLookup.libraryLookup(Path.of(args[0]), arena);

        MethodHandle configCreate = handle(lib, "zerodds_security_config_create", FunctionDescriptor.of(ptr));
        MethodHandle configDestroy = handle(lib, "zerodds_security_config_destroy", FunctionDescriptor.ofVoid(ptr));
        String[] setters = {
            "zerodds_security_set_identity_ca_path", "zerodds_security_set_identity_cert_path",
            "zerodds_security_set_private_key_path", "zerodds_security_set_permissions_ca_path",
            "zerodds_security_set_governance_path", "zerodds_security_set_permissions_path"
        };
        MethodHandle runtimeCreate = handle(lib, "zerodds_runtime_create_secure", FunctionDescriptor.of(ptr, i32, ptr));
        MethodHandle runtimeDestroy = handle(lib, "zerodds_runtime_destroy", FunctionDescriptor.ofVoid(ptr));
        MethodHandle writerCreate = handle(lib, "zerodds_writer_create", FunctionDescriptor.of(ptr, ptr, ptr, ptr, i32));
        MethodHandle writerDestroy = handle(lib, "zerodds_writer_destroy", FunctionDescriptor.ofVoid(ptr));
        MethodHandle readerCreate = handle(lib, "zerodds_reader_create", FunctionDescriptor.of(ptr, ptr, ptr, ptr, i32));
        MethodHandle readerDestroy = handle(lib, "zerodds_reader_destroy", FunctionDescriptor.ofVoid(ptr));

        MemorySegment cfg = (MemorySegment) configCreate.invokeExact();
        check(cfg.address() != 0, "config created");
        for (int i = 0; i < setters.length; i++) {
            MethodHandle set = handle(lib, setters[i], FunctionDescriptor.of(i32, ptr, ptr));
            int status = (int) set.invokeExact(cfg, arena.allocateUtf8String(args[2 + i]));
            check(status == 0, setters[i]);
        }

        PluginBinder binder = new PluginBinder(lib);
        check(binder.setAccessControlPlugin(cfg, new Access()) == 0, "AccessControl plugin accepted");
        check(binder.setLoggingPlugin(cfg, new Logging()) == 0, "Logging plugin accepted");
        check(binder.setDataTaggingPlugin(cfg, new Tagging()) == 0, "DataTagging plugin accepted");
        check(binder.setAuthenticationPlugin(cfg, new Authentication()) == 0, "Authentication plugin accepted");
        check(binder.setCryptographicPlugin(cfg, new Cryptographic()) == 0, "Cryptographic plugin accepted");

        MemorySegment rt = (MemorySegment) runtimeCreate.invokeExact(Integer.parseInt(args[1]), cfg);
        check(rt.address() != 0, "secured runtime created");

        // 9.3.2.11: the Authentication plugin validates the identity, with the
        // paths of the configuration as the properties of Table 50; its tokens
        // are given back after the copy.
        check(auValidateLocal == 1, "validate_local_identity called once");
        check(auGovernance == 1, "the governance path is a property of validate_local_identity");
        check(auIdentityOut >= 1, "get_identity_token");
        check(auIdentityBack == auIdentityOut, "return_identity_token");
        check(auStatusOut >= 1, "get_identity_status_token");
        check(auStatusBack == auStatusOut, "return_identity_status_token");
        // 9.8.1 steps 8 and 10: the credential goes to the plugin before the
        // ParticipantSecurityConfig.
        check(auCredential >= 1, "set_permissions_credential_and_token");
        check(auConfig >= 1, "set_participant_security_config");
        check(auCredentialBeforeConfig == 1, "the credential goes before the config");
        // 9.8.1 step 2 and step 3: validate_local_permissions, then
        // check_create_participant with the handle it gave.
        check(validateLocal == 1, "validate_local_permissions called once");
        check(createParticipant == 1, "check_create_participant with that handle");
        // 9.8.1 steps 6 and 7: the tokens, given back after the copy.
        check(permissionsTokenOut >= 1, "get_permissions_token");
        check(permissionsTokenBack == permissionsTokenOut, "return_permissions_token");
        check(credentialTokenOut >= 1, "get_permissions_credential_token");
        check(credentialTokenBack == credentialTokenOut, "return_permissions_credential_token");
        // 9.8.1 step 9: the participant config.
        check(participantConfigOut >= 1, "get_participant_security_config");
        check(listenerSet >= 1, "set_listener");

        MemorySegment topicAllowed = arena.allocateUtf8String("Allowed");
        MemorySegment topicForbidden = arena.allocateUtf8String(FORBIDDEN);
        MemorySegment type = arena.allocateUtf8String("RawBytes");

        // 9.8.6 steps 5 and 6: the DataWriter.
        MemorySegment allowed = (MemorySegment) writerCreate.invokeExact(rt, topicAllowed, type, 0);
        check(allowed.address() != 0, "writer on Allowed");
        check(createWriter >= 1, "check_create_datawriter");
        check(writerConfigOut >= 1, "get_datawriter_security_config");
        check(writerConfigBack == writerConfigOut, "return_datawriter_security_config");
        MemorySegment refused = (MemorySegment) writerCreate.invokeExact(rt, topicForbidden, type, 0);
        check(refused.address() == 0, "writer on Forbidden refused");
        check(refusedWriter >= 1, "the plugin refused the writer");

        // 9.8.6 step 6: a topic whose EndpointSecurityConfig protects the payload
        // gets a key from the Cryptographic plugin.
        MemorySegment protectedTopic = arena.allocateUtf8String("Protected");
        MemorySegment protectedWriter = (MemorySegment) writerCreate.invokeExact(rt, protectedTopic, type, 0);
        check(protectedWriter.address() != 0, "writer on Protected");
        check(crProtectedWriter == 1, "the protected writer is registered with the Cryptographic plugin");
        writerDestroy.invokeExact(protectedWriter);
        // 9.8.1 step 11, 9.8.9.1: the Cryptographic plugin registers the
        // participant, with the config of the AccessControl plugin, and the
        // volatile message endpoints under the names of the specification.
        check(crRegisterParticipant == 1, "register_local_participant once");
        check(crParticipantAxk == 0, "is_rtps_axk_protected from the AccessControl plugin");
        check(crVolatileWriter == 1, "the volatile message writer is named");
        check(crVolatileReader == 1, "the volatile message reader is named");

        // 9.8.6 steps 9 and 10: the DataReader.
        MemorySegment reader = (MemorySegment) readerCreate.invokeExact(rt, topicAllowed, type, 0);
        check(reader.address() != 0, "reader on Allowed");
        check(createReader >= 1, "check_create_datareader");
        check(readerConfigOut >= 1, "get_datareader_security_config");
        check(readerConfigBack == readerConfigOut, "return_datareader_security_config");
        MemorySegment refusedReader = (MemorySegment) readerCreate.invokeExact(rt, topicForbidden, type, 0);
        check(refusedReader.address() == 0, "reader on Forbidden refused");
        check(PluginTest.refusedReader >= 1, "the plugin refused the reader");

        // 9.7: the DataTagging plugin got the tags of the writer and gave the
        // tags it announces.
        check(setTags >= 1, "set_tags");
        check(getTags >= 1, "get_tags");

        // 9.6: the refusal was logged, through the Logging plugin.
        check(messages >= 1, "the Logging plugin was called");
        check(denied >= 1, "the refusal was logged");

        writerDestroy.invokeExact(allowed);
        readerDestroy.invokeExact(reader);
        runtimeDestroy.invokeExact(rt);
        configDestroy.invokeExact(cfg);

        // The runtime released each plugin: destroy ran once, after the
        // participant config went back.
        for (int i = 0; i < 100 && (acDestroyed == 0 || lgDestroyed == 0 || tgDestroyed == 0
                || auDestroyed == 0 || crDestroyed == 0); i++) {
            Thread.sleep(50);
        }
        check(acDestroyed == 1, "AccessControl plugin destroyed once");
        check(participantConfigBack == participantConfigOut, "return_participant_security_config");
        check(lgDestroyed == 1, "Logging plugin destroyed once");
        check(tgDestroyed == 1, "DataTagging plugin destroyed once");
        check(auDestroyed == 1, "Authentication plugin destroyed once");
        check(crDestroyed == 1, "Cryptographic plugin destroyed once");
        System.out.println("OK");
    }
}
