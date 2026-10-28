// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

import java.lang.foreign.AddressLayout;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.zerodds.security.Types.AlgorithmMasks;
import org.zerodds.security.Types.BinaryProperty;
import org.zerodds.security.Types.DataHolder;
import org.zerodds.security.Types.DataTag;
import org.zerodds.security.Types.EncodedPayload;
import org.zerodds.security.Types.EndpointAlgorithmInfo;
import org.zerodds.security.Types.HandshakeStep;
import org.zerodds.security.Types.LocalIdentityValidation;
import org.zerodds.security.Types.PreprocessedSubmessage;
import org.zerodds.security.Types.RemoteIdentityValidation;
import org.zerodds.security.Types.EndpointConfig;
import org.zerodds.security.Types.EndpointRequest;
import org.zerodds.security.Types.LogOptions;
import org.zerodds.security.Types.Masks;
import org.zerodds.security.Types.Pair;
import org.zerodds.security.Types.ParticipantAlgorithmInfo;
import org.zerodds.security.Types.ParticipantConfig;
import org.zerodds.security.Types.ParticipantRequest;
import org.zerodds.security.Types.Property;
import org.zerodds.security.Types.RelayRole;
import org.zerodds.security.Types.RemoteReaderDecision;
import org.zerodds.security.Types.TopicConfig;
import org.zerodds.security.Types.TopicRequest;

/**
 * Places a plugin of the application, written in Java, into a security
 * configuration of the native ZeroDDS library (DDS-Security 1.2 clause 11.5
 * and 11.7). The structs of {@code zerodds_security_plugins.h} are built in
 * native memory with the foreign function interface of the JDK, one upcall
 * stub for each operation; the runtime calls the plugin through them.
 *
 * <p>Needs JDK 21 with {@code --enable-preview} and
 * {@code --enable-native-access=ALL-UNNAMED}.
 */
public final class PluginBinder {
    private static final Linker LINKER = Linker.nativeLinker();
    private static final ValueLayout.OfLong LONG = ValueLayout.JAVA_LONG;
    private static final ValueLayout.OfInt INT = ValueLayout.JAVA_INT;
    private static final ValueLayout.OfBoolean BOOL = ValueLayout.JAVA_BOOLEAN;
    private static final ValueLayout.OfByte BYTE = ValueLayout.JAVA_BYTE;
    private static final AddressLayout ADDRESS = ValueLayout.ADDRESS;

    /** The revision of the structs, ZERODDS_PLUGIN_ABI_VERSION. */
    public static final int ABI_VERSION = 1;

    /** The bindings the native side still has a pointer to, until it destroys the plugin. */
    private static final Set<Object> LIVE = new HashSet<>();

    private final SymbolLookup library;

    /** A binder over the library {@code library} (libzerodds). */
    public PluginBinder(SymbolLookup library) {
        this.library = library;
    }

    // ------------------------------------------------------------------
    // Struct layouts of the header
    // ------------------------------------------------------------------

    /** The offsets of a C struct, laid out as the C compiler does. */
    static final class Struct {
        private final List<Long> offsets = new ArrayList<>();
        private long size;
        private long align = 1;

        private Struct add(long fieldSize, long fieldAlign) {
            size = (size + fieldAlign - 1) / fieldAlign * fieldAlign;
            offsets.add(size);
            size += fieldSize;
            align = Math.max(align, fieldAlign);
            return this;
        }

        Struct bool() {
            return add(1, 1);
        }

        Struct u32() {
            return add(4, 4);
        }

        Struct ptr() {
            return add(8, 8);
        }

        Struct bytes(long n) {
            return add(n, 1);
        }

        Struct nest(Struct inner) {
            return add(inner.size(), inner.align);
        }

        long at(int index) {
            return offsets.get(index);
        }

        long size() {
            return (size + align - 1) / align * align;
        }
    }

    private static Struct struct(String kinds) {
        Struct s = new Struct();
        for (char k : kinds.toCharArray()) {
            switch (k) {
                case 'b' -> s.bool();
                case 'i' -> s.u32();
                case 'p' -> s.ptr();
                case 'g' -> s.bytes(16);
                default -> throw new IllegalArgumentException("kind " + k);
            }
        }
        return s;
    }

    static final Struct ERROR = struct("pi");
    static final Struct PROPERTY = struct("ppb");
    static final Struct BINARY = struct("pppb");
    static final Struct HOLDER = struct("ppppp");
    static final Struct PAIR = struct("pp");
    static final Struct MASKS = struct("ii");
    static final Struct ALGORITHM_MASKS = new Struct().nest(MASKS).nest(MASKS).nest(MASKS);
    static final Struct PARTICIPANT_ALGORITHM_INFO =
            new Struct().nest(MASKS).nest(MASKS).nest(MASKS).u32().u32().u32().u32();
    static final Struct PARTICIPANT_REQUEST =
            new Struct().u32().ptr().bytes(16).ptr().ptr().bool().nest(ALGORITHM_MASKS);
    static final Struct TOPIC_REQUEST = struct("ippp");
    static final Struct ENDPOINT_REQUEST = struct("ipppppp" + "p");
    static final Struct TOPIC_CONFIG = struct("bbbb");
    static final Struct PARTICIPANT_CONFIG =
            new Struct()
                    .bool()
                    .bool()
                    .bool()
                    .bool()
                    .bool()
                    .bool()
                    .bool()
                    .u32()
                    .ptr()
                    .ptr()
                    .nest(PARTICIPANT_ALGORITHM_INFO);
    static final Struct ENDPOINT_CONFIG =
            new Struct().nest(TOPIC_CONFIG).bool().bool().bool().u32().ptr().ptr().u32().u32();
    static final Struct LISTENER = struct("pp");
    static final Struct LOG_OPTIONS = struct("ipb");
    static final Struct DATA_TAG = struct("pp");
    static final Struct HANDSHAKE_STEP = new Struct().u32().nest(struct("pp")).ptr();

    // ------------------------------------------------------------------
    // Reading what the runtime passes in
    // ------------------------------------------------------------------

    private static boolean isNull(MemorySegment p) {
        return p.address() == 0;
    }

    private static String str(MemorySegment p) {
        return isNull(p) ? "" : p.reinterpret(Long.MAX_VALUE).getUtf8String(0);
    }

    private static MemorySegment at(MemorySegment base, long bytes) {
        return base.reinterpret(bytes);
    }

    private static MemorySegment ptr(MemorySegment s, long offset) {
        return s.get(ADDRESS, offset);
    }

    private static List<Property> properties(MemorySegment array, long n) {
        List<Property> out = new ArrayList<>();
        if (isNull(array)) {
            return out;
        }
        MemorySegment all = at(array, n * PROPERTY.size());
        for (long i = 0; i < n; i++) {
            long o = i * PROPERTY.size();
            Property p = new Property(str(ptr(all, o)), str(ptr(all, o + 8)));
            p.propagate = all.get(BOOL, o + PROPERTY.at(2));
            out.add(p);
        }
        return out;
    }

    private static List<Pair> pairs(MemorySegment array, long n) {
        List<Pair> out = new ArrayList<>();
        if (isNull(array)) {
            return out;
        }
        MemorySegment all = at(array, n * PAIR.size());
        for (long i = 0; i < n; i++) {
            out.add(new Pair(str(ptr(all, i * 16)), str(ptr(all, i * 16 + 8))));
        }
        return out;
    }

    private static byte[] bytes(MemorySegment p, long n) {
        if (isNull(p) || n == 0) {
            return new byte[0];
        }
        return at(p, n).toArray(BYTE);
    }

    private static byte[] guid(MemorySegment p) {
        return at(p, 16).toArray(BYTE);
    }

    private static Masks masks(MemorySegment s, long o) {
        Masks m = new Masks();
        m.supportedMask = s.get(INT, o);
        m.requiredMask = s.get(INT, o + 4);
        return m;
    }

    private static DataHolder holder(MemorySegment p) {
        MemorySegment h = at(p, HOLDER.size());
        DataHolder out = new DataHolder();
        out.classId = str(ptr(h, HOLDER.at(0)));
        out.properties = properties(ptr(h, HOLDER.at(1)), h.get(LONG, HOLDER.at(2)));
        long n = h.get(LONG, HOLDER.at(4));
        MemorySegment bins = ptr(h, HOLDER.at(3));
        if (!isNull(bins)) {
            MemorySegment all = at(bins, n * BINARY.size());
            for (long i = 0; i < n; i++) {
                long o = i * BINARY.size();
                BinaryProperty b = new BinaryProperty();
                b.name = str(ptr(all, o));
                b.value = bytes(ptr(all, o + 8), all.get(LONG, o + 16));
                b.propagate = all.get(BOOL, o + BINARY.at(3));
                out.binaryProperties.add(b);
            }
        }
        return out;
    }

    private static ParticipantRequest participantRequest(MemorySegment p) {
        MemorySegment r = at(p, PARTICIPANT_REQUEST.size());
        ParticipantRequest out = new ParticipantRequest();
        out.domainId = r.get(INT, PARTICIPANT_REQUEST.at(0));
        out.domainTag = str(ptr(r, PARTICIPANT_REQUEST.at(1)));
        out.participantGuid = r.asSlice(PARTICIPANT_REQUEST.at(2), 16).toArray(BYTE);
        out.properties =
                properties(ptr(r, PARTICIPANT_REQUEST.at(3)), r.get(LONG, PARTICIPANT_REQUEST.at(4)));
        out.hasIdentityAlgorithms = r.get(BOOL, PARTICIPANT_REQUEST.at(5));
        long a = PARTICIPANT_REQUEST.at(6);
        out.identityAlgorithms = new AlgorithmMasks();
        out.identityAlgorithms.trustChain = masks(r, a);
        out.identityAlgorithms.messageAuth = masks(r, a + 8);
        out.identityAlgorithms.keyEstablishment = masks(r, a + 16);
        return out;
    }

    private static TopicRequest topicRequest(MemorySegment p) {
        MemorySegment r = at(p, TOPIC_REQUEST.size());
        TopicRequest out = new TopicRequest();
        out.domainId = r.get(INT, TOPIC_REQUEST.at(0));
        out.topicName = str(ptr(r, TOPIC_REQUEST.at(1)));
        out.qos = pairs(ptr(r, TOPIC_REQUEST.at(2)), r.get(LONG, TOPIC_REQUEST.at(3)));
        return out;
    }

    private static EndpointRequest endpointRequest(MemorySegment p) {
        MemorySegment r = at(p, ENDPOINT_REQUEST.size());
        EndpointRequest out = new EndpointRequest();
        out.domainId = r.get(INT, ENDPOINT_REQUEST.at(0));
        out.topicName = str(ptr(r, ENDPOINT_REQUEST.at(1)));
        MemorySegment parts = ptr(r, ENDPOINT_REQUEST.at(2));
        long nParts = r.get(LONG, ENDPOINT_REQUEST.at(3));
        if (!isNull(parts)) {
            MemorySegment all = at(parts, nParts * 8);
            for (long i = 0; i < nParts; i++) {
                out.partitions.add(str(ptr(all, i * 8)));
            }
        }
        out.dataTags = pairs(ptr(r, ENDPOINT_REQUEST.at(4)), r.get(LONG, ENDPOINT_REQUEST.at(5)));
        out.qos = pairs(ptr(r, ENDPOINT_REQUEST.at(6)), r.get(LONG, ENDPOINT_REQUEST.at(7)));
        return out;
    }

    private static TopicConfig topicConfig(MemorySegment s, long o) {
        TopicConfig c = new TopicConfig();
        c.isReadProtected = s.get(BOOL, o);
        c.isWriteProtected = s.get(BOOL, o + 1);
        c.isDiscoveryProtected = s.get(BOOL, o + 2);
        c.isLivelinessProtected = s.get(BOOL, o + 3);
        return c;
    }

    private static ParticipantConfig participantConfig(MemorySegment p) {
        MemorySegment c = at(p, PARTICIPANT_CONFIG.size());
        ParticipantConfig out = new ParticipantConfig();
        out.allowUnauthenticatedParticipants = c.get(BOOL, 0);
        out.isAccessProtected = c.get(BOOL, 1);
        out.isRtpsAxkProtected = c.get(BOOL, 2);
        out.isRtpsPskProtected = c.get(BOOL, 3);
        out.isDiscoveryProtected = c.get(BOOL, 4);
        out.isLivelinessProtected = c.get(BOOL, 5);
        out.isKeyRevisionEnabled = c.get(BOOL, 6);
        out.pluginParticipantAttributes = c.get(INT, PARTICIPANT_CONFIG.at(7));
        out.acParticipantProperties =
                properties(ptr(c, PARTICIPANT_CONFIG.at(8)), c.get(LONG, PARTICIPANT_CONFIG.at(9)));
        long a = PARTICIPANT_CONFIG.at(10);
        ParticipantAlgorithmInfo info = out.algorithmInfo;
        info.trustChain = masks(c, a);
        info.messageAuth = masks(c, a + 8);
        info.sharedSecret = masks(c, a + 16);
        info.cipherSupportedMask = c.get(INT, a + 24);
        info.cipherBuiltinEndpointsRequiredMask = c.get(INT, a + 28);
        info.cipherBuiltinKxEndpointsRequiredMask = c.get(INT, a + 32);
        info.cipherUserEndpointsDefaultRequiredMask = c.get(INT, a + 36);
        return out;
    }

    private static EndpointConfig endpointConfig(MemorySegment p) {
        MemorySegment c = at(p, ENDPOINT_CONFIG.size());
        EndpointConfig out = new EndpointConfig();
        out.topic = topicConfig(c, ENDPOINT_CONFIG.at(0));
        out.isSubmessageProtected = c.get(BOOL, ENDPOINT_CONFIG.at(1));
        out.isPayloadProtected = c.get(BOOL, ENDPOINT_CONFIG.at(2));
        out.isKeyProtected = c.get(BOOL, ENDPOINT_CONFIG.at(3));
        out.pluginEndpointAttributes = c.get(INT, ENDPOINT_CONFIG.at(4));
        out.acEndpointProperties =
                properties(ptr(c, ENDPOINT_CONFIG.at(5)), c.get(LONG, ENDPOINT_CONFIG.at(6)));
        out.cipherRequiredMask = c.get(INT, ENDPOINT_CONFIG.at(7));
        out.cipherSupportedMask = c.get(INT, ENDPOINT_CONFIG.at(8));
        return out;
    }

    // ------------------------------------------------------------------
    // Writing what a plugin hands out
    // ------------------------------------------------------------------

    private static void writeProperties(
            Arena arena, List<Property> in, MemorySegment target, long ptrOffset, long nOffset) {
        MemorySegment array =
                in.isEmpty() ? MemorySegment.NULL : arena.allocate(in.size() * PROPERTY.size(), 8);
        for (int i = 0; i < in.size(); i++) {
            long o = i * PROPERTY.size();
            array.set(ADDRESS, o, arena.allocateUtf8String(in.get(i).name));
            array.set(ADDRESS, o + 8, arena.allocateUtf8String(in.get(i).value));
            array.set(BOOL, o + PROPERTY.at(2), in.get(i).propagate);
        }
        target.set(ADDRESS, ptrOffset, array);
        target.set(LONG, nOffset, in.size());
    }

    private static void writeMasks(MemorySegment s, long o, Masks m) {
        s.set(INT, o, m.supportedMask);
        s.set(INT, o + 4, m.requiredMask);
    }

    /** Writes {@code value} as a ZeroDdsPluginDataHolder at {@code struct}; the memory is {@code arena}'s. */
    private static void fillHolder(Arena arena, DataHolder value, MemorySegment struct) {
        struct.set(ADDRESS, HOLDER.at(0), arena.allocateUtf8String(value.classId));
        writeProperties(arena, value.properties, struct, HOLDER.at(1), HOLDER.at(2));
        MemorySegment bins =
                value.binaryProperties.isEmpty()
                        ? MemorySegment.NULL
                        : arena.allocate(value.binaryProperties.size() * BINARY.size(), 8);
        for (int i = 0; i < value.binaryProperties.size(); i++) {
            BinaryProperty b = value.binaryProperties.get(i);
            long o = i * BINARY.size();
            bins.set(ADDRESS, o, arena.allocateUtf8String(b.name));
            MemorySegment data = arena.allocate(Math.max(1, b.value.length), 1);
            MemorySegment.copy(b.value, 0, data, BYTE, 0, b.value.length);
            bins.set(ADDRESS, o + 8, data);
            bins.set(LONG, o + 16, b.value.length);
            bins.set(BOOL, o + BINARY.at(3), b.propagate);
        }
        struct.set(ADDRESS, HOLDER.at(3), bins);
        struct.set(LONG, HOLDER.at(4), value.binaryProperties.size());
    }

    /** A token handed out and not yet given back. */
    private static final class IssuedHolder {
        final Arena arena = Arena.ofAuto();
        final DataHolder value;
        final MemorySegment struct;

        IssuedHolder(DataHolder value) {
            this.value = value;
            struct = arena.allocate(HOLDER.size(), 8);
            fillHolder(arena, value, struct);
        }
    }

    /** A config handed out and not yet given back. */
    private static final class Issued<T> {
        final Arena arena = Arena.ofAuto();
        final T value;

        Issued(T value) {
            this.value = value;
        }
    }

    private static void fillParticipantConfig(Arena arena, ParticipantConfig c, MemorySegment out) {
        MemorySegment s = at(out, PARTICIPANT_CONFIG.size());
        s.set(BOOL, 0, c.allowUnauthenticatedParticipants);
        s.set(BOOL, 1, c.isAccessProtected);
        s.set(BOOL, 2, c.isRtpsAxkProtected);
        s.set(BOOL, 3, c.isRtpsPskProtected);
        s.set(BOOL, 4, c.isDiscoveryProtected);
        s.set(BOOL, 5, c.isLivelinessProtected);
        s.set(BOOL, 6, c.isKeyRevisionEnabled);
        s.set(INT, PARTICIPANT_CONFIG.at(7), c.pluginParticipantAttributes);
        writeProperties(
                arena, c.acParticipantProperties, s, PARTICIPANT_CONFIG.at(8), PARTICIPANT_CONFIG.at(9));
        long a = PARTICIPANT_CONFIG.at(10);
        ParticipantAlgorithmInfo i = c.algorithmInfo;
        writeMasks(s, a, i.trustChain);
        writeMasks(s, a + 8, i.messageAuth);
        writeMasks(s, a + 16, i.sharedSecret);
        s.set(INT, a + 24, i.cipherSupportedMask);
        s.set(INT, a + 28, i.cipherBuiltinEndpointsRequiredMask);
        s.set(INT, a + 32, i.cipherBuiltinKxEndpointsRequiredMask);
        s.set(INT, a + 36, i.cipherUserEndpointsDefaultRequiredMask);
    }

    private static void fillEndpointConfig(Arena arena, EndpointConfig c, MemorySegment out) {
        MemorySegment s = at(out, ENDPOINT_CONFIG.size());
        long t = ENDPOINT_CONFIG.at(0);
        s.set(BOOL, t, c.topic.isReadProtected);
        s.set(BOOL, t + 1, c.topic.isWriteProtected);
        s.set(BOOL, t + 2, c.topic.isDiscoveryProtected);
        s.set(BOOL, t + 3, c.topic.isLivelinessProtected);
        s.set(BOOL, ENDPOINT_CONFIG.at(1), c.isSubmessageProtected);
        s.set(BOOL, ENDPOINT_CONFIG.at(2), c.isPayloadProtected);
        s.set(BOOL, ENDPOINT_CONFIG.at(3), c.isKeyProtected);
        s.set(INT, ENDPOINT_CONFIG.at(4), c.pluginEndpointAttributes);
        writeProperties(arena, c.acEndpointProperties, s, ENDPOINT_CONFIG.at(5), ENDPOINT_CONFIG.at(6));
        s.set(INT, ENDPOINT_CONFIG.at(7), c.cipherRequiredMask);
        s.set(INT, ENDPOINT_CONFIG.at(8), c.cipherSupportedMask);
    }

    // ------------------------------------------------------------------
    // The bindings: one upcall stub for each operation
    // ------------------------------------------------------------------

    /** Keeps what a binding allocates alive as long as the native side may call it. */
    private abstract static class Binding {
        final Arena arena = Arena.ofAuto();
        String lastMessage = "";
        MemorySegment lastMessageSegment = MemorySegment.NULL;

        /** The status of an operation that threw {@code e}; fills in {@code err}. */
        int fail(Throwable e, MemorySegment err) {
            int code = SecurityException.INTERNAL;
            int minor = 0;
            if (e instanceof SecurityException s) {
                code = s.code();
                minor = s.minorCode();
            }
            lastMessage = String.valueOf(e.getMessage());
            lastMessageSegment = arena.allocateUtf8String(lastMessage);
            if (!isNull(err)) {
                MemorySegment s = at(err, ERROR.size());
                s.set(ADDRESS, ERROR.at(0), lastMessageSegment);
                s.set(INT, ERROR.at(1), minor);
            }
            return code;
        }

        /** Builds the function pointers of the table from the public methods named in {@code names}. */
        MemorySegment[] stubs(String[] names) throws ReflectiveOperationException {
            Method[] methods = getClass().getDeclaredMethods();
            MemorySegment[] out = new MemorySegment[names.length];
            for (int i = 0; i < names.length; i++) {
                Method found = null;
                for (Method m : methods) {
                    if (m.getName().equals(names[i])) {
                        found = m;
                    }
                }
                if (found == null) {
                    throw new NoSuchMethodException(names[i]);
                }
                found.setAccessible(true);
                MethodHandle handle = MethodHandles.lookup().unreflect(found).bindTo(this);
                out[i] = LINKER.upcallStub(handle, descriptor(found), arena);
            }
            return out;
        }

        private static FunctionDescriptor descriptor(Method m) {
            Class<?>[] types = m.getParameterTypes();
            MemoryLayout[] args = new MemoryLayout[types.length];
            for (int i = 0; i < types.length; i++) {
                args[i] = layoutOf(types[i]);
            }
            return m.getReturnType() == void.class
                    ? FunctionDescriptor.ofVoid(args)
                    : FunctionDescriptor.of(layoutOf(m.getReturnType()), args);
        }

        private static MemoryLayout layoutOf(Class<?> type) {
            if (type == long.class) {
                return LONG;
            }
            if (type == int.class) {
                return INT;
            }
            if (type == boolean.class) {
                return BOOL;
            }
            if (type == MemorySegment.class) {
                return ADDRESS;
            }
            throw new IllegalArgumentException("no layout for " + type);
        }
    }

    // ---- AccessControl -------------------------------------------------

    private static final String[] ACCESS_CONTROL_OPERATIONS = {
        "pluginClassId", "validateLocal", "validateRemote", "createParticipant", "createDatawriter",
        "createDatareader", "createTopic", "localRegister", "localDispose", "remoteParticipant",
        "remoteDatawriter", "remoteDatareader", "remoteTopic", "localDatawriterMatch",
        "localDatareaderMatch", "remoteRegister", "remoteDispose", "relay", "permissionsToken",
        "credentialToken", "setListener", "returnPermissionsToken", "returnCredentialToken",
        "participantConfig", "topicConfig", "writerConfig", "readerConfig",
        "returnParticipantConfig", "returnTopicConfig", "returnWriterConfig",
        "returnReaderConfig", "destroy"
    };

    private static final class AccessControlBinding extends Binding {
        final AccessControlPlugin plugin;
        final MemorySegment classId;
        final List<IssuedHolder> permissionsTokens = new ArrayList<>();
        final List<IssuedHolder> credentialTokens = new ArrayList<>();
        final List<Issued<ParticipantConfig>> participantConfigs = new ArrayList<>();
        final List<Issued<EndpointConfig>> writerConfigs = new ArrayList<>();
        final List<Issued<EndpointConfig>> readerConfigs = new ArrayList<>();
        final MemorySegment listenerTable = arena.allocate(LISTENER.size(), 8);
        MemorySegment onRevoke;

        AccessControlBinding(AccessControlPlugin plugin) {
            this.plugin = plugin;
            this.classId = arena.allocateUtf8String(plugin.pluginClassId());
        }

        private int guarded(MemorySegment err, Runnable body) {
            try {
                body.run();
                return 0;
            } catch (Throwable e) {
                return fail(e, err);
            }
        }

        private static void permit(MemorySegment out, boolean value) {
            at(out, 1).set(BOOL, 0, value);
        }

        MemorySegment pluginClassId(MemorySegment user) {
            return classId;
        }

        int validateLocal(MemorySegment user, long identity, MemorySegment p, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> at(out, 8).set(LONG, 0, plugin.validateLocalPermissions(identity, participantRequest(p))));
        }

        int validateRemote(MemorySegment user, long li, long ri, MemorySegment pt, MemorySegment ct, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> at(out, 8).set(LONG, 0, plugin.validateRemotePermissions(li, ri, holder(pt), holder(ct))));
        }

        int createParticipant(MemorySegment user, long perm, MemorySegment p, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkCreateParticipant(perm, participantRequest(p))));
        }

        int createDatawriter(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkCreateDatawriter(perm, endpointRequest(r))));
        }

        int createDatareader(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkCreateDatareader(perm, endpointRequest(r))));
        }

        int createTopic(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkCreateTopic(perm, topicRequest(r))));
        }

        int localRegister(MemorySegment user, long perm, MemorySegment writer, MemorySegment key, long n, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkLocalDatawriterRegisterInstance(perm, guid(writer), bytes(key, n))));
        }

        int localDispose(MemorySegment user, long perm, MemorySegment writer, MemorySegment key, long n, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkLocalDatawriterDisposeInstance(perm, guid(writer), bytes(key, n))));
        }

        int remoteParticipant(MemorySegment user, long perm, MemorySegment p, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRemoteParticipant(perm, participantRequest(p))));
        }

        int remoteDatawriter(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRemoteDatawriter(perm, endpointRequest(r))));
        }

        int remoteDatareader(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                RemoteReaderDecision d = plugin.checkRemoteDatareader(perm, endpointRequest(r));
                at(out, 4).set(INT, 0, d.ordinal());
            });
        }

        int remoteTopic(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRemoteTopic(perm, topicRequest(r))));
        }

        int localDatawriterMatch(MemorySegment user, long wp, long rp, MemorySegment a, MemorySegment b, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkLocalDatawriterMatch(wp, rp, endpointRequest(a), endpointRequest(b))));
        }

        int localDatareaderMatch(MemorySegment user, long wp, long rp, MemorySegment a, MemorySegment b, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkLocalDatareaderMatch(wp, rp, endpointRequest(a), endpointRequest(b))));
        }

        int remoteRegister(MemorySegment user, long perm, MemorySegment reader, MemorySegment publication, MemorySegment key, long n, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRemoteDatawriterRegisterInstance(perm, guid(reader), guid(publication), bytes(key, n))));
        }

        int remoteDispose(MemorySegment user, long perm, MemorySegment reader, MemorySegment publication, MemorySegment key, long n, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRemoteDatawriterDisposeInstance(perm, guid(reader), guid(publication), bytes(key, n))));
        }

        int relay(MemorySegment user, long perm, int role, MemorySegment r, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> permit(out, plugin.checkRelay(perm, role == 1 ? RelayRole.WRITER : RelayRole.READER, endpointRequest(r))));
        }

        private int token(long handle, MemorySegment out, MemorySegment err, boolean credential) {
            return guarded(err, () -> {
                IssuedHolder issued = new IssuedHolder(credential ? plugin.getPermissionsCredentialToken(handle) : plugin.getPermissionsToken(handle));
                MemorySegment.copy(issued.struct, 0, at(out, HOLDER.size()), 0, HOLDER.size());
                (credential ? credentialTokens : permissionsTokens).add(issued);
            });
        }

        int permissionsToken(MemorySegment user, long handle, MemorySegment out, MemorySegment err) {
            return token(handle, out, err, false);
        }

        int credentialToken(MemorySegment user, long handle, MemorySegment out, MemorySegment err) {
            return token(handle, out, err, true);
        }

        int setListener(MemorySegment user, MemorySegment listener, MemorySegment err) {
            return guarded(err, () -> {
                MemorySegment l = at(listener, LISTENER.size());
                MemorySegment context = ptr(l, 0);
                MemorySegment function = ptr(l, 8);
                MethodHandle call = LINKER.downcallHandle(function, FunctionDescriptor.of(BOOL, ADDRESS, ADDRESS, LONG));
                plugin.setListener((name, handle) -> {
                    try (Arena temp = Arena.ofConfined()) {
                        return (boolean) call.invokeExact(context, temp.allocateUtf8String(name), handle);
                    } catch (Throwable e) {
                        throw new SecurityException(SecurityException.INTERNAL, String.valueOf(e));
                    }
                });
            });
        }

        private int returnToken(MemorySegment t, MemorySegment err, boolean credential) {
            return guarded(err, () -> {
                List<IssuedHolder> list = credential ? credentialTokens : permissionsTokens;
                MemorySegment classIdPointer = ptr(at(t, HOLDER.size()), HOLDER.at(0));
                IssuedHolder found = null;
                for (IssuedHolder h : list) {
                    if (ptr(h.struct, HOLDER.at(0)).address() == classIdPointer.address()) {
                        found = h;
                    }
                }
                if (found == null) {
                    throw new SecurityException(SecurityException.BAD_ARGUMENT, "the token was not issued by this plugin");
                }
                list.remove(found);
                if (credential) {
                    plugin.returnPermissionsCredentialToken(found.value);
                } else {
                    plugin.returnPermissionsToken(found.value);
                }
            });
        }

        int returnPermissionsToken(MemorySegment user, MemorySegment t, MemorySegment err) {
            return returnToken(t, err, false);
        }

        int returnCredentialToken(MemorySegment user, MemorySegment t, MemorySegment err) {
            return returnToken(t, err, true);
        }

        int participantConfig(MemorySegment user, long perm, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                Issued<ParticipantConfig> issued = new Issued<>(plugin.getParticipantSecurityConfig(perm));
                fillParticipantConfig(issued.arena, issued.value, out);
                participantConfigs.add(issued);
            });
        }

        int topicConfig(MemorySegment user, long perm, MemorySegment topic, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                TopicConfig c = plugin.getTopicSecurityConfig(perm, str(topic));
                MemorySegment s = at(out, TOPIC_CONFIG.size());
                s.set(BOOL, 0, c.isReadProtected);
                s.set(BOOL, 1, c.isWriteProtected);
                s.set(BOOL, 2, c.isDiscoveryProtected);
                s.set(BOOL, 3, c.isLivelinessProtected);
            });
        }

        private int endpointConfigOf(long perm, MemorySegment r, MemorySegment out, MemorySegment err, boolean writer) {
            return guarded(err, () -> {
                EndpointRequest request = endpointRequest(r);
                Issued<EndpointConfig> issued = new Issued<>(writer ? plugin.getDatawriterSecurityConfig(perm, request) : plugin.getDatareaderSecurityConfig(perm, request));
                fillEndpointConfig(issued.arena, issued.value, out);
                (writer ? writerConfigs : readerConfigs).add(issued);
            });
        }

        int writerConfig(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return endpointConfigOf(perm, r, out, err, true);
        }

        int readerConfig(MemorySegment user, long perm, MemorySegment r, MemorySegment out, MemorySegment err) {
            return endpointConfigOf(perm, r, out, err, false);
        }

        int returnParticipantConfig(MemorySegment user, MemorySegment c, MemorySegment err) {
            return guarded(err, () -> {
                ParticipantConfig value = PluginBinder.participantConfig(c);
                if (!participantConfigs.isEmpty()) {
                    participantConfigs.remove(0);
                }
                plugin.returnParticipantSecurityConfig(value);
            });
        }

        int returnTopicConfig(MemorySegment user, MemorySegment c, MemorySegment err) {
            return guarded(err, () -> plugin.returnTopicSecurityConfig(PluginBinder.topicConfig(c, 0)));
        }

        private int returnEndpointConfig(MemorySegment c, MemorySegment err, boolean writer) {
            return guarded(err, () -> {
                EndpointConfig value = PluginBinder.endpointConfig(c);
                List<Issued<EndpointConfig>> list = writer ? writerConfigs : readerConfigs;
                if (!list.isEmpty()) {
                    list.remove(0);
                }
                if (writer) {
                    plugin.returnDatawriterSecurityConfig(value);
                } else {
                    plugin.returnDatareaderSecurityConfig(value);
                }
            });
        }

        int returnWriterConfig(MemorySegment user, MemorySegment c, MemorySegment err) {
            return returnEndpointConfig(c, err, true);
        }

        int returnReaderConfig(MemorySegment user, MemorySegment c, MemorySegment err) {
            return returnEndpointConfig(c, err, false);
        }

        void destroy(MemorySegment user) {
            try {
                plugin.destroy();
            } finally {
                synchronized (LIVE) {
                    LIVE.remove(this);
                }
            }
        }
    }

    /**
     * Places {@code plugin} into the security configuration {@code cfg}
     * (a {@code zerodds_ZeroDdsSecurityConfig*}). The runtime calls
     * {@code destroy} on the plugin when it lets go of it. Returns the status
     * of {@code zerodds_security_set_access_control_plugin}: 0, or -1 for a
     * NULL argument, -2 for a plugin the runtime refuses.
     */
    public int setAccessControlPlugin(MemorySegment cfg, AccessControlPlugin plugin) throws Throwable {
        AccessControlBinding binding = new AccessControlBinding(plugin);
        MemorySegment[] stubs = binding.stubs(ACCESS_CONTROL_OPERATIONS);
        Arena arena = binding.arena;
        long size = 16 + 8L * stubs.length;
        MemorySegment table = arena.allocate(size, 8);
        table.set(INT, 0, ABI_VERSION);
        for (int i = 0; i < stubs.length; i++) {
            table.set(ADDRESS, 16 + 8L * i, stubs[i]);
        }
        synchronized (LIVE) {
            LIVE.add(binding);
        }
        int status = place("zerodds_security_set_access_control_plugin", cfg, table);
        if (status != 0) {
            synchronized (LIVE) {
                LIVE.remove(binding);
            }
        }
        return status;
    }

    // ---- Logging -------------------------------------------------------

    private static final String[] LOGGING_OPERATIONS = {
        "pluginClassId", "setLogOptions", "enableLogging", "setListener", "log", "destroy"
    };

    private static final class LoggingBinding extends Binding {
        final LoggingPlugin plugin;
        final MemorySegment classId;

        LoggingBinding(LoggingPlugin plugin) {
            this.plugin = plugin;
            this.classId = arena.allocateUtf8String(plugin.pluginClassId());
        }

        private int guarded(MemorySegment err, Runnable body) {
            try {
                body.run();
                return 0;
            } catch (Throwable e) {
                return fail(e, err);
            }
        }

        MemorySegment pluginClassId(MemorySegment user) {
            return classId;
        }

        int setLogOptions(MemorySegment user, MemorySegment o, MemorySegment err) {
            return guarded(err, () -> {
                MemorySegment s = at(o, LOG_OPTIONS.size());
                LogOptions options = new LogOptions();
                options.logLevel = s.get(INT, LOG_OPTIONS.at(0));
                MemorySegment file = ptr(s, LOG_OPTIONS.at(1));
                options.logFile = isNull(file) ? null : str(file);
                options.distribute = s.get(BOOL, LOG_OPTIONS.at(2));
                plugin.setLogOptions(options);
            });
        }

        int enableLogging(MemorySegment user, MemorySegment err) {
            return guarded(err, plugin::enableLogging);
        }

        int setListener(MemorySegment user, MemorySegment listener, MemorySegment err) {
            return guarded(err, () -> {
                if (isNull(listener)) {
                    plugin.setListener(null);
                    return;
                }
                MemorySegment l = at(listener, LISTENER.size());
                MemorySegment context = ptr(l, 0);
                MethodHandle call =
                        LINKER.downcallHandle(
                                ptr(l, 8),
                                FunctionDescriptor.of(INT, ADDRESS, INT, ADDRESS, ADDRESS, ADDRESS));
                plugin.setListener((level, participant, category, message) -> {
                    try (Arena temp = Arena.ofConfined()) {
                        MemorySegment guid = temp.allocate(16, 1);
                        MemorySegment.copy(participant, 0, guid, BYTE, 0, 16);
                        return (int) call.invokeExact(context, level, guid, temp.allocateUtf8String(category), temp.allocateUtf8String(message));
                    } catch (Throwable e) {
                        throw new SecurityException(SecurityException.INTERNAL, String.valueOf(e));
                    }
                });
            });
        }

        void log(MemorySegment user, int level, MemorySegment participant, MemorySegment category, MemorySegment message) {
            try {
                plugin.log(level, guid(participant), str(category), str(message));
            } catch (Throwable e) {
                // Logging does not fail the operation that logs.
            }
        }

        void destroy(MemorySegment user) {
            try {
                plugin.destroy();
            } finally {
                synchronized (LIVE) {
                    LIVE.remove(this);
                }
            }
        }
    }

    /** As {@link #setAccessControlPlugin}, for the Logging plugin. */
    public int setLoggingPlugin(MemorySegment cfg, LoggingPlugin plugin) throws Throwable {
        LoggingBinding binding = new LoggingBinding(plugin);
        MemorySegment[] stubs = binding.stubs(LOGGING_OPERATIONS);
        MemorySegment table = binding.arena.allocate(16 + 8L * stubs.length, 8);
        table.set(INT, 0, ABI_VERSION);
        for (int i = 0; i < stubs.length; i++) {
            table.set(ADDRESS, 16 + 8L * i, stubs[i]);
        }
        synchronized (LIVE) {
            LIVE.add(binding);
        }
        int status = place("zerodds_security_set_logging_plugin", cfg, table);
        if (status != 0) {
            synchronized (LIVE) {
                LIVE.remove(binding);
            }
        }
        return status;
    }

    // ---- DataTagging ---------------------------------------------------

    private static final String[] DATA_TAGGING_OPERATIONS = {
        "pluginClassId", "setTags", "getTags", "destroy"
    };

    private static final class DataTaggingBinding extends Binding {
        final DataTaggingPlugin plugin;
        final MemorySegment classId;
        Arena current = Arena.ofAuto();

        DataTaggingBinding(DataTaggingPlugin plugin) {
            this.plugin = plugin;
            this.classId = arena.allocateUtf8String(plugin.pluginClassId());
        }

        MemorySegment pluginClassId(MemorySegment user) {
            return classId;
        }

        void setTags(MemorySegment user, MemorySegment guid, MemorySegment tags, long n) {
            List<DataTag> list = new ArrayList<>();
            if (!isNull(tags)) {
                MemorySegment all = at(tags, n * DATA_TAG.size());
                for (long i = 0; i < n; i++) {
                    list.add(new DataTag(str(ptr(all, i * 16)), str(ptr(all, i * 16 + 8))));
                }
            }
            try {
                plugin.setTags(guid(guid), list);
            } catch (Throwable e) {
                // The operation has no status.
            }
        }

        void getTags(MemorySegment user, MemorySegment guid, MemorySegment outTags, MemorySegment outN) {
            current = Arena.ofAuto();
            List<DataTag> tags;
            try {
                tags = plugin.getTags(guid(guid));
            } catch (Throwable e) {
                tags = new ArrayList<>();
            }
            MemorySegment array = tags.isEmpty() ? MemorySegment.NULL : current.allocate(tags.size() * DATA_TAG.size(), 8);
            for (int i = 0; i < tags.size(); i++) {
                array.set(ADDRESS, i * 16L, current.allocateUtf8String(tags.get(i).name));
                array.set(ADDRESS, i * 16L + 8, current.allocateUtf8String(tags.get(i).value));
            }
            at(outTags, 8).set(ADDRESS, 0, array);
            at(outN, 8).set(LONG, 0, tags.size());
        }

        void destroy(MemorySegment user) {
            try {
                plugin.destroy();
            } finally {
                synchronized (LIVE) {
                    LIVE.remove(this);
                }
            }
        }
    }

    /** As {@link #setAccessControlPlugin}, for the DataTagging plugin. */
    public int setDataTaggingPlugin(MemorySegment cfg, DataTaggingPlugin plugin) throws Throwable {
        DataTaggingBinding binding = new DataTaggingBinding(plugin);
        MemorySegment[] stubs = binding.stubs(DATA_TAGGING_OPERATIONS);
        MemorySegment table = binding.arena.allocate(16 + 8L * stubs.length, 8);
        table.set(INT, 0, ABI_VERSION);
        for (int i = 0; i < stubs.length; i++) {
            table.set(ADDRESS, 16 + 8L * i, stubs[i]);
        }
        synchronized (LIVE) {
            LIVE.add(binding);
        }
        int status = place("zerodds_security_set_data_tagging_plugin", cfg, table);
        if (status != 0) {
            synchronized (LIVE) {
                LIVE.remove(binding);
            }
        }
        return status;
    }

    private int place(String function, MemorySegment cfg, MemorySegment table) throws Throwable {
        MethodHandle set =
                LINKER.downcallHandle(
                        library.find(function).orElseThrow(),
                        FunctionDescriptor.of(INT, ADDRESS, ADDRESS));
        return (int) set.invokeExact(cfg, table);
    }

    // ------------------------------------------------------------------
    // Authentication
    // ------------------------------------------------------------------

    /** The octets of a ZeroDdsPluginBytes at {@code p}. */
    private static byte[] bytesStruct(MemorySegment p) {
        if (isNull(p)) {
            return new byte[0];
        }
        MemorySegment s = at(p, 16);
        return bytes(ptr(s, 0), s.get(LONG, 8));
    }

    /** Writes {@code value} as ZeroDdsPluginBytes at {@code target}, the octets in {@code arena}. */
    private static MemorySegment writeBytes(Arena arena, byte[] value, MemorySegment target) {
        MemorySegment data = arena.allocate(Math.max(1, value.length), 1);
        MemorySegment.copy(value, 0, data, BYTE, 0, value.length);
        MemorySegment s = at(target, 16);
        s.set(ADDRESS, 0, data);
        s.set(LONG, 8, value.length);
        return data;
    }

    private static void writeStep(Arena arena, HandshakeStep step, MemorySegment out) {
        MemorySegment s = at(out, HANDSHAKE_STEP.size());
        s.set(INT, HANDSHAKE_STEP.at(0), step.kind);
        writeBytes(arena, step.token, s.asSlice(8, 16));
        s.set(LONG, HANDSHAKE_STEP.at(2), step.secret);
    }

    private static List<Property> propertyList(MemorySegment props, long n) {
        return properties(props, n);
    }

    private static final String[] AUTHENTICATION_OPERATIONS = {
        "pluginClassId", "validateLocalIdentity", "getIdentityToken", "getIdentityStatusToken",
        "setParticipantSecurityConfig", "setPermissionsCredentialAndToken", "validateRemoteIdentity",
        "beginHandshakeRequest", "beginHandshakeReply", "processHandshake", "getSharedSecret",
        "getAuthenticatedPeerCredentialToken", "setListener", "returnIdentityToken",
        "returnIdentityStatusToken", "returnAuthenticatedPeerCredentialToken",
        "returnHandshakeHandle", "returnIdentityHandle", "returnSharedsecretHandle",
        "getPermissionsToken", "getSharedSecretBytes", "getSharedSecretChallenges", "destroy"
    };

    /** A token handed out, with the memory behind it. */
    private static final class IssuedBytes {
        final Arena arena = Arena.ofAuto();
        final byte[] value;
        final MemorySegment data;

        IssuedBytes(byte[] value, MemorySegment target) {
            this.value = value;
            this.data = writeBytes(arena, value, target);
        }
    }

    private static final class AuthenticationBinding extends Binding {
        final AuthenticationPlugin plugin;
        final MemorySegment classId;
        final List<IssuedBytes> identityTokens = new ArrayList<>();
        final List<IssuedBytes> statusTokens = new ArrayList<>();
        final List<IssuedBytes> credentialTokens = new ArrayList<>();
        Arena current = Arena.ofAuto();
        final List<Object> listeners = new ArrayList<>();

        AuthenticationBinding(AuthenticationPlugin plugin) {
            this.plugin = plugin;
            this.classId = arena.allocateUtf8String(plugin.pluginClassId());
        }

        private int guarded(MemorySegment err, Runnable body) {
            try {
                body.run();
                return 0;
            } catch (Throwable e) {
                return fail(e, err);
            }
        }

        MemorySegment pluginClassId(MemorySegment user) {
            return classId;
        }

        int validateLocalIdentity(MemorySegment user, int domain, MemorySegment props, long n,
                MemorySegment bins, long nb, MemorySegment guidIn, MemorySegment outResult,
                MemorySegment outIdentity, MemorySegment outGuid, MemorySegment err) {
            return guarded(err, () -> {
                List<BinaryProperty> binary = new ArrayList<>();
                if (!isNull(bins)) {
                    MemorySegment all = at(bins, nb * BINARY.size());
                    for (long i = 0; i < nb; i++) {
                        long o = i * BINARY.size();
                        BinaryProperty b = new BinaryProperty();
                        b.name = str(ptr(all, o));
                        b.value = bytes(ptr(all, o + 8), all.get(LONG, o + 16));
                        b.propagate = all.get(BOOL, o + BINARY.at(3));
                        binary.add(b);
                    }
                }
                LocalIdentityValidation v =
                        plugin.validateLocalIdentity(domain, propertyList(props, n), binary, guid(guidIn));
                at(outResult, 4).set(INT, 0, v.result);
                at(outIdentity, 8).set(LONG, 0, v.identity);
                MemorySegment.copy(v.adjustedGuid, 0, at(outGuid, 16), BYTE, 0, 16);
            });
        }

        private int token(long identity, MemorySegment out, MemorySegment err, int which) {
            return guarded(err, () -> {
                byte[] value = which == 0 ? plugin.getIdentityToken(identity)
                        : which == 1 ? plugin.getIdentityStatusToken(identity) : new byte[0];
                IssuedBytes issued = new IssuedBytes(value, out);
                (which == 0 ? identityTokens : statusTokens).add(issued);
            });
        }

        int getIdentityToken(MemorySegment user, long identity, MemorySegment out, MemorySegment err) {
            return token(identity, out, err, 0);
        }

        int getIdentityStatusToken(MemorySegment user, long identity, MemorySegment out, MemorySegment err) {
            return token(identity, out, err, 1);
        }

        int setParticipantSecurityConfig(MemorySegment user, long identity, MemorySegment config,
                MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                MemorySegment c = at(config, ALGORITHM_MASKS.size());
                AlgorithmMasks in = new AlgorithmMasks();
                in.trustChain = masks(c, 0);
                in.messageAuth = masks(c, 8);
                in.keyEstablishment = masks(c, 16);
                AlgorithmMasks result = plugin.setParticipantSecurityConfig(identity, in);
                MemorySegment o = at(out, ALGORITHM_MASKS.size());
                writeMasks(o, 0, result.trustChain);
                writeMasks(o, 8, result.messageAuth);
                writeMasks(o, 16, result.keyEstablishment);
            });
        }

        int setPermissionsCredentialAndToken(MemorySegment user, long identity, MemorySegment credential,
                MemorySegment token, MemorySegment err) {
            return guarded(err, () -> plugin.setPermissionsCredentialAndToken(
                    identity, bytesStruct(credential), bytesStruct(token)));
        }

        int validateRemoteIdentity(MemorySegment user, long local, MemorySegment remoteGuid,
                MemorySegment token, MemorySegment request, MemorySegment outResult,
                MemorySegment outIdentity, MemorySegment outRequest, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                RemoteIdentityValidation v = plugin.validateRemoteIdentity(
                        local, guid(remoteGuid), bytesStruct(token), isNull(request) ? null : bytesStruct(request));
                at(outResult, 4).set(INT, 0, v.result);
                at(outIdentity, 8).set(LONG, 0, v.identity);
                if (v.localAuthRequestToken != null) {
                    writeBytes(current, v.localAuthRequestToken, outRequest);
                } else {
                    MemorySegment s = at(outRequest, 16);
                    s.set(ADDRESS, 0, MemorySegment.NULL);
                    s.set(LONG, 8, 0L);
                }
            });
        }

        int beginHandshakeRequest(MemorySegment user, long initiator, long replier, MemorySegment data,
                MemorySegment outHandshake, MemorySegment outStep, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                HandshakeStep step = plugin.beginHandshakeRequest(initiator, replier, bytesStruct(data));
                at(outHandshake, 8).set(LONG, 0, step.handshake);
                writeStep(current, step, outStep);
            });
        }

        int beginHandshakeReply(MemorySegment user, long replier, long initiator, MemorySegment request,
                MemorySegment data, MemorySegment outHandshake, MemorySegment outStep, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                HandshakeStep step = plugin.beginHandshakeReply(
                        replier, initiator, bytesStruct(request), bytesStruct(data));
                at(outHandshake, 8).set(LONG, 0, step.handshake);
                writeStep(current, step, outStep);
            });
        }

        int processHandshake(MemorySegment user, long handshake, MemorySegment token, MemorySegment outStep,
                MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                writeStep(current, plugin.processHandshake(handshake, bytesStruct(token)), outStep);
            });
        }

        int getSharedSecret(MemorySegment user, long handshake, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> at(out, 8).set(LONG, 0, plugin.getSharedSecret(handshake)));
        }

        int getAuthenticatedPeerCredentialToken(MemorySegment user, long handshake, MemorySegment out,
                MemorySegment err) {
            return guarded(err, () -> credentialTokens.add(
                    new IssuedBytes(plugin.getAuthenticatedPeerCredentialToken(handshake), out)));
        }

        int setListener(MemorySegment user, MemorySegment listener, MemorySegment err) {
            return guarded(err, () -> {
                if (isNull(listener)) {
                    plugin.setListener(null);
                    return;
                }
                MemorySegment l = at(listener, 24);
                MemorySegment context = ptr(l, 0);
                MethodHandle revoke = LINKER.downcallHandle(ptr(l, 8), FunctionDescriptor.of(INT, ADDRESS, LONG));
                MethodHandle status = LINKER.downcallHandle(ptr(l, 16), FunctionDescriptor.ofVoid(ADDRESS, LONG, INT));
                plugin.setListener(new AuthenticationListener() {
                    public int onRevokeIdentity(long handle) {
                        try {
                            return (int) revoke.invokeExact(context, handle);
                        } catch (Throwable e) {
                            throw new SecurityException(SecurityException.INTERNAL, String.valueOf(e));
                        }
                    }

                    public void onStatusChanged(long handle, int kind) {
                        try {
                            status.invokeExact(context, handle, kind);
                        } catch (Throwable e) {
                            throw new SecurityException(SecurityException.INTERNAL, String.valueOf(e));
                        }
                    }
                });
            });
        }

        private int giveBack(MemorySegment t, MemorySegment err, int which) {
            return guarded(err, () -> {
                List<IssuedBytes> list = which == 0 ? identityTokens : which == 1 ? statusTokens : credentialTokens;
                long address = ptr(at(t, 16), 0).address();
                IssuedBytes found = null;
                for (IssuedBytes b : list) {
                    if (b.data.address() == address) {
                        found = b;
                    }
                }
                if (found == null) {
                    throw new SecurityException(SecurityException.BAD_ARGUMENT, "the token was not issued by this plugin");
                }
                list.remove(found);
                if (which == 0) {
                    plugin.returnIdentityToken(found.value);
                } else if (which == 1) {
                    plugin.returnIdentityStatusToken(found.value);
                } else {
                    plugin.returnAuthenticatedPeerCredentialToken(found.value);
                }
            });
        }

        int returnIdentityToken(MemorySegment user, MemorySegment t, MemorySegment err) {
            return giveBack(t, err, 0);
        }

        int returnIdentityStatusToken(MemorySegment user, MemorySegment t, MemorySegment err) {
            return giveBack(t, err, 1);
        }

        int returnAuthenticatedPeerCredentialToken(MemorySegment user, MemorySegment t, MemorySegment err) {
            return giveBack(t, err, 2);
        }

        int returnHandshakeHandle(MemorySegment user, long handle, MemorySegment err) {
            return guarded(err, () -> plugin.returnHandshakeHandle(handle));
        }

        int returnIdentityHandle(MemorySegment user, long handle, MemorySegment err) {
            return guarded(err, () -> plugin.returnIdentityHandle(handle));
        }

        int returnSharedsecretHandle(MemorySegment user, long handle, MemorySegment err) {
            return guarded(err, () -> plugin.returnSharedsecretHandle(handle));
        }

        void getPermissionsToken(MemorySegment user, MemorySegment out) {
            current = Arena.ofAuto();
            byte[] token;
            try {
                token = plugin.getPermissionsToken();
            } catch (Throwable e) {
                token = new byte[0];
            }
            writeBytes(current, token, out);
        }

        boolean getSharedSecretBytes(MemorySegment user, long secret, MemorySegment out) {
            try {
                byte[] value = plugin.getSharedSecretBytes(secret);
                if (value == null) {
                    return false;
                }
                current = Arena.ofAuto();
                writeBytes(current, value, out);
                return true;
            } catch (Throwable e) {
                return false;
            }
        }

        boolean getSharedSecretChallenges(MemorySegment user, long secret, MemorySegment c1, MemorySegment c2) {
            try {
                byte[] a = new byte[32];
                byte[] b = new byte[32];
                if (!plugin.getSharedSecretChallenges(secret, a, b)) {
                    return false;
                }
                MemorySegment.copy(a, 0, at(c1, 32), BYTE, 0, 32);
                MemorySegment.copy(b, 0, at(c2, 32), BYTE, 0, 32);
                return true;
            } catch (Throwable e) {
                return false;
            }
        }

        void destroy(MemorySegment user) {
            try {
                plugin.destroy();
            } finally {
                synchronized (LIVE) {
                    LIVE.remove(this);
                }
            }
        }
    }

    /** As {@link #setAccessControlPlugin}, for the Authentication plugin. */
    public int setAuthenticationPlugin(MemorySegment cfg, AuthenticationPlugin plugin) throws Throwable {
        AuthenticationBinding binding = new AuthenticationBinding(plugin);
        MemorySegment[] stubs = binding.stubs(AUTHENTICATION_OPERATIONS);
        MemorySegment table = binding.arena.allocate(16 + 8L * stubs.length, 8);
        table.set(INT, 0, ABI_VERSION);
        for (int i = 0; i < stubs.length; i++) {
            table.set(ADDRESS, 16 + 8L * i, stubs[i]);
        }
        synchronized (LIVE) {
            LIVE.add(binding);
        }
        int status = place("zerodds_security_set_authentication_plugin", cfg, table);
        if (status != 0) {
            synchronized (LIVE) {
                LIVE.remove(binding);
            }
        }
        return status;
    }

    // ------------------------------------------------------------------
    // Cryptographic
    // ------------------------------------------------------------------

    private static void writeAlgorithmInfo(MemorySegment s, long a, ParticipantAlgorithmInfo i) {
        writeMasks(s, a, i.trustChain);
        writeMasks(s, a + 8, i.messageAuth);
        writeMasks(s, a + 16, i.sharedSecret);
        s.set(INT, a + 24, i.cipherSupportedMask);
        s.set(INT, a + 28, i.cipherBuiltinEndpointsRequiredMask);
        s.set(INT, a + 32, i.cipherBuiltinKxEndpointsRequiredMask);
        s.set(INT, a + 36, i.cipherUserEndpointsDefaultRequiredMask);
    }

    private static List<DataHolder> holders(MemorySegment tokens, long n) {
        List<DataHolder> out = new ArrayList<>();
        if (isNull(tokens)) {
            return out;
        }
        MemorySegment all = at(tokens, n * HOLDER.size());
        for (long i = 0; i < n; i++) {
            out.add(holder(all.asSlice(i * HOLDER.size(), HOLDER.size())));
        }
        return out;
    }

    private static long[] handles(MemorySegment array, long n) {
        long[] out = new long[(int) n];
        if (!isNull(array)) {
            MemorySegment all = at(array, n * 8);
            for (int i = 0; i < n; i++) {
                out[i] = all.get(LONG, i * 8L);
            }
        }
        return out;
    }

    private static boolean sameHolders(List<DataHolder> a, List<DataHolder> b) {
        if (a.size() != b.size()) {
            return false;
        }
        for (int i = 0; i < a.size(); i++) {
            if (!a.get(i).classId.equals(b.get(i).classId)
                    || a.get(i).properties.size() != b.get(i).properties.size()) {
                return false;
            }
            for (int j = 0; j < a.get(i).properties.size(); j++) {
                Property x = a.get(i).properties.get(j);
                Property y = b.get(i).properties.get(j);
                if (!x.name.equals(y.name) || !x.value.equals(y.value)) {
                    return false;
                }
            }
        }
        return true;
    }

    private static final String[] CRYPTOGRAPHIC_OPERATIONS = {
        "registerLocalParticipant", "registerRemoteParticipant", "registerDatawriter",
        "registerRemoteDatareader", "registerDatareader", "registerRemoteDatawriter", "revise",
        "activate", "unregisterParticipant", "unregisterDatawriter", "unregisterDatareader",
        "createParticipantTokens", "setParticipantTokens", "createDatawriterTokens",
        "setDatawriterTokens", "createDatareaderTokens", "setDatareaderTokens", "returnTokens",
        "encodePayload", "encodeDatawriter", "encodeDatareader", "encodeMessage", "decodeMessage",
        "preprocess", "decodeDatawriter", "decodeDatareader", "decodePayload", "destroy"
    };

    /** A list of tokens handed out and not yet given back. */
    private static final class IssuedTokens {
        final Arena arena = Arena.ofAuto();
        final List<DataHolder> value;
        final MemorySegment array;

        IssuedTokens(List<DataHolder> value) {
            this.value = value;
            array = arena.allocate(Math.max(1, value.size()) * HOLDER.size(), 8);
            for (int i = 0; i < value.size(); i++) {
                fillHolder(arena, value.get(i), array.asSlice(i * HOLDER.size(), HOLDER.size()));
            }
        }
    }

    private static final class CryptographicBinding extends Binding {
        final CryptographicPlugin plugin;
        final List<IssuedTokens> issued = new ArrayList<>();
        Arena current = Arena.ofAuto();

        CryptographicBinding(CryptographicPlugin plugin) {
            this.plugin = plugin;
        }

        private int guarded(MemorySegment err, Runnable body) {
            try {
                body.run();
                return 0;
            } catch (Throwable e) {
                return fail(e, err);
            }
        }

        private static void flag(MemorySegment out, boolean value) {
            at(out, 1).set(BOOL, 0, value);
        }

        private static void handle(MemorySegment out, long value) {
            at(out, 8).set(LONG, 0, value);
        }

        int registerLocalParticipant(MemorySegment user, long identity, long permissions, MemorySegment props,
                long n, MemorySegment config, MemorySegment outHandle, MemorySegment outInfo, MemorySegment err) {
            return guarded(err, () -> {
                ParticipantAlgorithmInfo info = new ParticipantAlgorithmInfo();
                handle(outHandle, plugin.registerLocalParticipant(
                        identity, permissions, propertyList(props, n), PluginBinder.participantConfig(config), info));
                writeAlgorithmInfo(at(outInfo, PARTICIPANT_ALGORITHM_INFO.size()), 0, info);
            });
        }

        int registerRemoteParticipant(MemorySegment user, long local, long identity, long permissions,
                long secret, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> handle(out, plugin.registerMatchedRemoteParticipant(
                    local, identity, permissions, secret)));
        }

        private int registerEndpoint(long participant, MemorySegment props, long n, MemorySegment config,
                MemorySegment guidIn, MemorySegment outHandle, MemorySegment outInfo, MemorySegment err,
                boolean writer) {
            return guarded(err, () -> {
                EndpointAlgorithmInfo info = new EndpointAlgorithmInfo();
                EndpointConfig c = PluginBinder.endpointConfig(config);
                List<Property> p = propertyList(props, n);
                byte[] g = guid(guidIn);
                handle(outHandle, writer
                        ? plugin.registerLocalDatawriter(participant, p, c, g, info)
                        : plugin.registerLocalDatareader(participant, p, c, g, info));
                MemorySegment o = at(outInfo, 8);
                o.set(INT, 0, info.requiredMask);
                o.set(INT, 4, info.supportedMask);
            });
        }

        int registerDatawriter(MemorySegment user, long participant, MemorySegment props, long n,
                MemorySegment config, MemorySegment guidIn, MemorySegment outHandle, MemorySegment outInfo,
                MemorySegment err) {
            return registerEndpoint(participant, props, n, config, guidIn, outHandle, outInfo, err, true);
        }

        int registerDatareader(MemorySegment user, long participant, MemorySegment props, long n,
                MemorySegment config, MemorySegment guidIn, MemorySegment outHandle, MemorySegment outInfo,
                MemorySegment err) {
            return registerEndpoint(participant, props, n, config, guidIn, outHandle, outInfo, err, false);
        }

        int registerRemoteDatareader(MemorySegment user, long writer, long participant, long secret,
                boolean relayOnly, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> handle(out, plugin.registerMatchedRemoteDatareader(
                    writer, participant, secret, relayOnly)));
        }

        int registerRemoteDatawriter(MemorySegment user, long reader, long participant, long secret,
                MemorySegment out, MemorySegment err) {
            return guarded(err, () -> handle(out, plugin.registerMatchedRemoteDatawriter(
                    reader, participant, secret)));
        }

        int revise(MemorySegment user, long participant, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> at(out, 4).set(INT, 0, plugin.reviseLocalEntityKeys(participant)));
        }

        int activate(MemorySegment user, long participant, int revision, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> flag(out, plugin.activateKeyRevision(participant, revision)));
        }

        int unregisterParticipant(MemorySegment user, long h, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> flag(out, plugin.unregisterParticipant(h)));
        }

        int unregisterDatawriter(MemorySegment user, long h, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> flag(out, plugin.unregisterDatawriter(h)));
        }

        int unregisterDatareader(MemorySegment user, long h, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> flag(out, plugin.unregisterDatareader(h)));
        }

        private int createTokens(long first, long second, int revision, MemorySegment out, MemorySegment err,
                int which) {
            return guarded(err, () -> {
                List<DataHolder> tokens = which == 0
                        ? plugin.createLocalParticipantCryptoTokens(first, second, revision)
                        : which == 1 ? plugin.createLocalDatawriterCryptoTokens(first, second, revision)
                                : plugin.createLocalDatareaderCryptoTokens(first, second, revision);
                MemorySegment o = at(out, 24);
                if (tokens == null) {
                    o.set(BOOL, 0, false);
                    o.set(ADDRESS, 8, MemorySegment.NULL);
                    o.set(LONG, 16, 0L);
                    return;
                }
                IssuedTokens list = new IssuedTokens(tokens);
                o.set(BOOL, 0, true);
                o.set(ADDRESS, 8, list.array);
                o.set(LONG, 16, tokens.size());
                issued.add(list);
            });
        }

        int createParticipantTokens(MemorySegment user, long a, long b, int r, MemorySegment out, MemorySegment err) {
            return createTokens(a, b, r, out, err, 0);
        }

        int createDatawriterTokens(MemorySegment user, long a, long b, int r, MemorySegment out, MemorySegment err) {
            return createTokens(a, b, r, out, err, 1);
        }

        int createDatareaderTokens(MemorySegment user, long a, long b, int r, MemorySegment out, MemorySegment err) {
            return createTokens(a, b, r, out, err, 2);
        }

        private int setTokens(long first, long second, MemorySegment tokens, long n, MemorySegment out,
                MemorySegment err, int which) {
            return guarded(err, () -> {
                List<DataHolder> list = holders(tokens, n);
                flag(out, which == 0 ? plugin.setRemoteParticipantCryptoTokens(first, second, list)
                        : which == 1 ? plugin.setRemoteDatawriterCryptoTokens(first, second, list)
                                : plugin.setRemoteDatareaderCryptoTokens(first, second, list));
            });
        }

        int setParticipantTokens(MemorySegment user, long a, long b, MemorySegment t, long n, MemorySegment out,
                MemorySegment err) {
            return setTokens(a, b, t, n, out, err, 0);
        }

        int setDatawriterTokens(MemorySegment user, long a, long b, MemorySegment t, long n, MemorySegment out,
                MemorySegment err) {
            return setTokens(a, b, t, n, out, err, 1);
        }

        int setDatareaderTokens(MemorySegment user, long a, long b, MemorySegment t, long n, MemorySegment out,
                MemorySegment err) {
            return setTokens(a, b, t, n, out, err, 2);
        }

        int returnTokens(MemorySegment user, MemorySegment tokens, long n, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                List<DataHolder> list = holders(tokens, n);
                for (IssuedTokens i : issued) {
                    if (sameHolders(i.value, list)) {
                        issued.remove(i);
                        break;
                    }
                }
                flag(out, plugin.returnCryptoTokens(list));
            });
        }

        private static void bytesOut(Arena arena, byte[] value, MemorySegment target) {
            if (value == null) {
                MemorySegment s = at(target, 16);
                s.set(ADDRESS, 0, MemorySegment.NULL);
                s.set(LONG, 8, 0L);
            } else {
                writeBytes(arena, value, target);
            }
        }

        int encodePayload(MemorySegment user, MemorySegment plain, long writer, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                EncodedPayload payload = plugin.encodeSerializedPayload(bytesStruct(plain), writer);
                MemorySegment o = at(out, 40);
                o.set(BOOL, 0, payload != null);
                bytesOut(current, payload == null ? null : payload.encoded, o.asSlice(8, 16));
                bytesOut(current, payload == null ? null : payload.extraInlineQos, o.asSlice(24, 16));
            });
        }

        int encodeDatawriter(MemorySegment user, MemorySegment plain, long writer, MemorySegment readers, long n,
                MemorySegment index, MemorySegment outEncoded, MemorySegment outOk, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                int[] position = {(int) at(index, 8).get(LONG, 0)};
                byte[] encoded = plugin.encodeDatawriterSubmessage(bytesStruct(plain), writer, handles(readers, n), position);
                at(index, 8).set(LONG, 0, position[0]);
                flag(outOk, encoded != null);
                bytesOut(current, encoded, outEncoded);
            });
        }

        int encodeDatareader(MemorySegment user, MemorySegment plain, long reader, MemorySegment writers, long n,
                MemorySegment outPresent, MemorySegment outEncoded, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                byte[] encoded = plugin.encodeDatareaderSubmessage(bytesStruct(plain), reader, handles(writers, n));
                flag(outPresent, encoded != null);
                bytesOut(current, encoded, outEncoded);
            });
        }

        int encodeMessage(MemorySegment user, MemorySegment plain, long participant, MemorySegment receivers, long n,
                MemorySegment index, boolean psk, MemorySegment outEncoded, MemorySegment outOk, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                int[] position = {(int) at(index, 8).get(LONG, 0)};
                byte[] encoded = plugin.encodeRtpsMessage(bytesStruct(plain), participant, handles(receivers, n), position, psk);
                at(index, 8).set(LONG, 0, position[0]);
                flag(outOk, encoded != null);
                bytesOut(current, encoded, outEncoded);
            });
        }

        int decodeMessage(MemorySegment user, MemorySegment encoded, long receiving, long sending,
                MemorySegment outPresent, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                byte[] plain = plugin.decodeRtpsMessage(bytesStruct(encoded), receiving, sending);
                flag(outPresent, plain != null);
                bytesOut(current, plain, out);
            });
        }

        int preprocess(MemorySegment user, MemorySegment encoded, long receiving, long sending,
                MemorySegment outPresent, MemorySegment outWriter, MemorySegment outReader,
                MemorySegment outCategory, MemorySegment err) {
            return guarded(err, () -> {
                PreprocessedSubmessage r = plugin.preprocessSecureSubmsg(bytesStruct(encoded), receiving, sending);
                flag(outPresent, r != null);
                if (r != null) {
                    handle(outWriter, r.datawriter);
                    handle(outReader, r.datareader);
                    at(outCategory, 4).set(INT, 0, r.category);
                }
            });
        }

        private int decodeSubmessage(MemorySegment encoded, long first, long second, MemorySegment outPresent,
                MemorySegment out, MemorySegment err, boolean writer) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                byte[] plain = writer
                        ? plugin.decodeDatawriterSubmessage(bytesStruct(encoded), first, second)
                        : plugin.decodeDatareaderSubmessage(bytesStruct(encoded), first, second);
                flag(outPresent, plain != null);
                bytesOut(current, plain, out);
            });
        }

        int decodeDatawriter(MemorySegment user, MemorySegment encoded, long first, long second,
                MemorySegment outPresent, MemorySegment out, MemorySegment err) {
            return decodeSubmessage(encoded, first, second, outPresent, out, err, true);
        }

        int decodeDatareader(MemorySegment user, MemorySegment encoded, long first, long second,
                MemorySegment outPresent, MemorySegment out, MemorySegment err) {
            return decodeSubmessage(encoded, first, second, outPresent, out, err, false);
        }

        int decodePayload(MemorySegment user, MemorySegment encoded, MemorySegment inlineQos, long reader,
                long writer, MemorySegment outPresent, MemorySegment out, MemorySegment err) {
            return guarded(err, () -> {
                current = Arena.ofAuto();
                byte[] plain = plugin.decodeSerializedPayload(
                        bytesStruct(encoded), bytesStruct(inlineQos), reader, writer);
                flag(outPresent, plain != null);
                bytesOut(current, plain, out);
            });
        }

        void destroy(MemorySegment user) {
            try {
                plugin.destroy();
            } finally {
                synchronized (LIVE) {
                    LIVE.remove(this);
                }
            }
        }
    }

    /** As {@link #setAuthenticationPlugin}, for the Cryptographic plugin. */
    public int setCryptographicPlugin(MemorySegment cfg, CryptographicPlugin plugin) throws Throwable {
        CryptographicBinding binding = new CryptographicBinding(plugin);
        MemorySegment[] stubs = binding.stubs(CRYPTOGRAPHIC_OPERATIONS);
        MemorySegment table = binding.arena.allocate(16 + 8L * stubs.length, 8);
        table.set(INT, 0, ABI_VERSION);
        for (int i = 0; i < stubs.length; i++) {
            table.set(ADDRESS, 16 + 8L * i, stubs[i]);
        }
        synchronized (LIVE) {
            LIVE.add(binding);
        }
        int status = place("zerodds_security_set_cryptographic_plugin", cfg, table);
        if (status != 0) {
            synchronized (LIVE) {
                LIVE.remove(binding);
            }
        }
        return status;
    }
}
