// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 session client + wire codec, pure Java 8 -- a port of
// `crates/xrce-client/src/lib.rs`'s `XrceClient<T>` state machine combined
// with the concrete submessage codecs of `crates/xrce`
// (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). `crates/xrce-client`
// itself never touches the wire -- its module doc says so directly: "the
// caller delegates submessage encoding; we only track state". This class
// is that combination, matching endpoints/c's `zdwc_*` framing (the port
// every fixture in endpoints/testdata/xrce-client/ was verified against)
// rather than reinventing the byte layout.
//
// Distinct from `ZdwEndpoint.xrceWriteFrame`, the simplified "ZeroDDS
// Endpoint Profile" framing the sync/async examples use (sample body only,
// no BaseObjectRequest, no ClientKey) -- this class speaks the
// spec-conformant object-lifecycle frames a real XRCE agent expects for
// CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the conformant WRITE_DATA
// that carries the target ObjectId (DDS-XRCE 1.0 Annex B.3.1).
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
//   XrceClient ctor -> Sec 7.8.2 CREATE_CLIENT
//   connect          -> Sec 8.4.5 (handshake)
//   createObject     -> Sec 7.8.3 CREATE
//   deleteObject     -> Sec 7.8.3 DELETE
//   requestWrite     -> Sec 7.8.4 WRITE_DATA
//   requestRead      -> Sec 7.8.5 READ_DATA
//   disconnect       -> Sec 8.4.5

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

public final class XrceClient {

    /** Lifecycle state, mirrors crates/xrce-client's ClientState. */
    public enum State { DISCONNECTED, CONNECTING, CONNECTED }

    /** Mirrors ClientError::InvalidState. */
    public static final class ClientError extends RuntimeException {
        /** RuntimeException is Serializable, and the gate builds with -Werror. */
        private static final long serialVersionUID = 1L;

        public ClientError(String message) { super(message); }
    }

    /** ObjectKind codes actually exercised by this client (DDS-XRCE 1.0 Table 4). */
    public static final class ObjectKind {
        public static final int PARTICIPANT = 0x01;
        public static final int TOPIC = 0x02;
        public static final int PUBLISHER = 0x03;
        public static final int SUBSCRIBER = 0x04;
        public static final int DATAWRITER = 0x05;
        public static final int DATAREADER = 0x06;
        private ObjectKind() { }
    }

    // ObjectId (spec Sec 7.2.1): a 12-bit raw id packed with a 4-bit
    // ObjectKind into a 16-bit wire value, big-endian on the wire
    // (crates/xrce object_id.rs::to_bytes).
    public static int objectId(int rawId, int kind) {
        return ((rawId & 0x0FFF) << 4) | (kind & 0x0F);
    }

    private static byte[] objectIdBytes(int id) {
        return new byte[] { (byte) (id >> 8), (byte) id };
    }

    /**
     * "Representation timestamps": whether CLIENT_Representation carries a
     * {@code Time_t client_timestamp} after {@code xrce_vendor_id}. DDS-XRCE
     * 1.0 contradicts itself: the prose of Sec 7.8.2.1 lists it, the IDL of
     * Annex A and the bytes of Annex B.1 do not. ABSENT (default) is the
     * Annex A form; PRESENT is the prose form. The agent has to be set the
     * same way ({@code representation_timestamps:} in its configuration).
     */
    public enum RepresentationTimestamps { ABSENT, PRESENT }

    /**
     * Extensibility of the unannotated Time_t (DDS-XTypes 1.3 7.3.1.2.1.8):
     * APPENDABLE (default) writes a DHEADER = 8 in front, FINAL does not.
     * Only used with {@link RepresentationTimestamps#PRESENT}.
     */
    public enum UnannotatedTypes { APPENDABLE, FINAL }

    private final byte[] clientKey;
    private RepresentationTimestamps representationTimestamps = RepresentationTimestamps.ABSENT;
    private UnannotatedTypes unannotatedTypes = UnannotatedTypes.APPENDABLE;
    private State state = State.DISCONNECTED;
    private int sessionId;
    private int nextRequestId = 1;

    public XrceClient(byte[] clientKey) {
        this.clientKey = clientKey;
    }

    public XrceClient(byte[] clientKey, RepresentationTimestamps representationTimestamps) {
        this.clientKey = clientKey;
        this.representationTimestamps = representationTimestamps;
    }

    public RepresentationTimestamps representationTimestamps() { return representationTimestamps; }
    public void setRepresentationTimestamps(RepresentationTimestamps v) { this.representationTimestamps = v; }
    public UnannotatedTypes unannotatedTypes() { return unannotatedTypes; }
    public void setUnannotatedTypes(UnannotatedTypes v) { this.unannotatedTypes = v; }

    /**
     * CREATE_CLIENT for this client's session id and key, in this client's
     * representation timestamps setting; with PRESENT the timestamp is the
     * wall clock at the time of the call.
     */
    public byte[] frameCreateClient() {
        long now = System.currentTimeMillis();
        return frameCreateClient(sessionId, clientKey, representationTimestamps, unannotatedTypes,
                (int) (now / 1000L), (int) ((now % 1000L) * 1000000L));
    }

    public State state() { return state; }
    public byte[] clientKey() { return clientKey; }
    public int sessionId() { return sessionId; }

    /**
     * Sec 8.4.5 handshake: proposes {@code requestedSessionId} (1..127, Sec
     * 8.3.2.1) and moves DISCONNECTED -> CONNECTING. The caller still has
     * to build and deliver CREATE_CLIENT itself, e.g. via
     * {@link #frameCreateClient()}.
     */
    public void connect(int requestedSessionId) {
        if (state != State.DISCONNECTED) throw new ClientError("invalid state: " + state);
        this.sessionId = requestedSessionId;
        this.state = State.CONNECTING;
    }

    /** Completes the handshake once the caller has received STATUS_AGENT. Sec 8.4.5. */
    public void markConnected() {
        if (state != State.CONNECTING) throw new ClientError("invalid state: " + state);
        this.state = State.CONNECTED;
    }

    // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
    // never a valid request id) -- identical to
    // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
    private int allocRequestId() {
        int req = nextRequestId;
        int next = (req + 1) & 0xFFFF;
        nextRequestId = (next == 0) ? 1 : next;
        return req;
    }

    private void requireConnected() {
        if (state != State.CONNECTED) throw new ClientError("invalid state: " + state);
    }

    /** Submits a CREATE (Sec 7.8.3). Returns a request id. */
    public int createObject() { requireConnected(); return allocRequestId(); }

    /** Submits a DELETE (Sec 7.8.3). */
    public int deleteObject() { requireConnected(); return allocRequestId(); }

    /** Submits a WRITE_DATA (Sec 7.8.4). */
    public int requestWrite() { requireConnected(); return allocRequestId(); }

    /** Submits a READ_DATA (Sec 7.8.5, pull-based). */
    public int requestRead() { requireConnected(); return allocRequestId(); }

    /** Sec 8.4.5: closes the session -- back to DISCONNECTED. Local state only. */
    public void disconnect() { this.state = State.DISCONNECTED; }

    // ---- framing: the octets a real transport sends -----------------------

    public static final int SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00;
    public static final int STREAM_ID_NONE = 0x00;
    public static final int STREAM_ID_BEST_EFFORT = 0x01;
    public static final int STREAM_ID_BUILTIN_RELIABLE = 0x80;

    /** eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's module doc has the byte-oracle measurement backing it). */
    public static final int DEFAULT_MTU = 508;

    private static final int SM_CREATE_CLIENT = 0x00;
    private static final int SM_CREATE = 0x01;
    private static final int SM_DELETE = 0x03;
    private static final int SM_WRITE_DATA = 0x07;
    private static final int SM_READ_DATA = 0x08;

    private static final int FLAG_E = 0x01; // E-flag: submessage body is little-endian (Sec 8.3.4).
    private static final int FLAG_REUSE = 0x02;
    private static final int FLAG_REPLACE = 0x04;

    // request_id (spec Sec 7.7.8) as this port's own choice of byte order
    // for a host-side monotonic counter: big-endian, matching ObjectId's
    // own wire order and crates/xrce-client::request_id_octets (the answer
    // this repository settled on).
    private static byte[] requestIdBytes(int requestId) {
        return new byte[] { (byte) (requestId >> 8), (byte) requestId };
    }

    // MessageHeader WITH ClientKey (8 bytes): session_id(1) + stream_id(1)
    // + sequence_nr(2, LE) + ClientKey(4).
    private static void putHeader(ByteArrayOutputStream out, int sessionId, int streamId,
                                   int seq, byte[] clientKey) {
        out.write(sessionId);
        out.write(streamId);
        out.write(seq & 0xFF);
        out.write((seq >> 8) & 0xFF);
        out.write(clientKey, 0, 4);
    }

    // SubmessageHeader (4 bytes): id + flags + body length (LE, always --
    // spec Sec 8.3.4).
    private static void putSmHeader(ByteArrayOutputStream out, int smId, int flags, int bodyLen) {
        if (bodyLen > 0xFFFF) throw new IllegalArgumentException("body too large: " + bodyLen);
        out.write(smId);
        out.write(flags);
        out.write(bodyLen & 0xFF);
        out.write((bodyLen >> 8) & 0xFF);
    }

    private static void putLe32(ByteArrayOutputStream out, int v) {
        out.write(v & 0xFF);
        out.write((v >> 8) & 0xFF);
        out.write((v >> 16) & 0xFF);
        out.write((v >> 24) & 0xFF);
    }

    /**
     * CLIENT_Representation (Annex A, Sec 7.8.2.1): xrce_cookie(4) +
     * xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1) +
     * a {@code 0} properties-absent octet + requestedMtu (2, little
     * endian) -- eProsima's own non-normative MTU extension
     * (crates/xrce/src/handshake.rs's module doc has the byte-oracle
     * measurement backing {@link #DEFAULT_MTU}), always sent by this
     * client. No PropertySeq support here -- this client sends none.
     */
    public static byte[] encodeClientRepresentation(int version, int vendorId, byte[] clientKey,
                                                      int sessionId, int requestedMtu) {
        return encodeClientRepresentation(version, vendorId, clientKey, sessionId, requestedMtu,
                RepresentationTimestamps.ABSENT, UnannotatedTypes.APPENDABLE, 0, 0);
    }

    /**
     * Same, with the representation timestamps setting. PRESENT writes the
     * Time_t of the client after xrce_vendor_id: seconds (i32) and
     * nanoseconds (u32), little endian; with APPENDABLE a DHEADER = 8 in
     * front (12 octets in all), with FINAL none (8 octets). All following
     * offsets shift accordingly. {@code seconds} / {@code nanoseconds} are
     * ignored with ABSENT.
     */
    public static byte[] encodeClientRepresentation(int version, int vendorId, byte[] clientKey,
                                                      int sessionId, int requestedMtu,
                                                      RepresentationTimestamps timestamps,
                                                      UnannotatedTypes extensibility,
                                                      int seconds, int nanoseconds) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write("XRCE".getBytes(StandardCharsets.US_ASCII), 0, 4);
        out.write((version >> 8) & 0xFF);
        out.write(version & 0xFF);
        out.write((vendorId >> 8) & 0xFF);
        out.write(vendorId & 0xFF);
        if (timestamps == RepresentationTimestamps.PRESENT) {
            if (extensibility == UnannotatedTypes.APPENDABLE) {
                putLe32(out, 8); // DHEADER of Time_t
            }
            putLe32(out, seconds);
            putLe32(out, nanoseconds);
        }
        out.write(clientKey, 0, 4);
        out.write(sessionId & 0xFF);
        out.write(0); // properties absent
        out.write(requestedMtu & 0xFF);
        out.write((requestedMtu >> 8) & 0xFF);
        return out.toByteArray();
    }

    /**
     * CREATE_CLIENT (Sec 7.8.2/Sec 8.3.5.1): no session exists yet, so the
     * MessageHeader session_id is SESSION_ID_NONE_WITH_CLIENT_KEY (0x00)
     * when the requested {@code sessionId} is 0..127 (header carries the
     * ClientKey) and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) when it is
     * 128..255 (header without ClientKey), DDS-XRCE 1.0 Sec 8.3.2.1. The
     * requested {@code sessionId} itself travels only in the
     * CLIENT_Representation payload. stream_id=0x00 STREAM_ID_NONE;
     * sequence_nr=0; one CREATE_CLIENT submessage (flags=0x01, E-flag
     * only) whose body is a real {@link #encodeClientRepresentation}, not
     * an ad hoc placeholder.
     */
    public static byte[] frameCreateClient(int sessionId, byte[] clientKey) {
        return frameCreateClient(sessionId, clientKey, RepresentationTimestamps.ABSENT,
                UnannotatedTypes.APPENDABLE, 0, 0);
    }

    /** CREATE_CLIENT with explicit representation timestamps setting and timestamp values. */
    public static byte[] frameCreateClient(int sessionId, byte[] clientKey,
                                            RepresentationTimestamps timestamps,
                                            UnannotatedTypes extensibility,
                                            int seconds, int nanoseconds) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        if (sessionId < 128) {
            putHeader(out, 0x00, STREAM_ID_NONE, 0, clientKey);
        } else {
            out.write(0x80); out.write(STREAM_ID_NONE); out.write(0); out.write(0);
        }
        byte[] body = encodeClientRepresentation(0x0100, 0x0000, clientKey, sessionId, DEFAULT_MTU,
                timestamps, extensibility, seconds, nanoseconds);
        putSmHeader(out, SM_CREATE_CLIENT, FLAG_E, body.length);
        out.write(body, 0, body.length);
        return out.toByteArray();
    }

    /**
     * CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
     * WITH the ClientKey + one CREATE submessage (Sec 7.7.8
     * BaseObjectRequest + the real, two-level Sec 7.7.3 ObjectVariant: the
     * outer {@code objectKind} byte, then the inner
     * REPRESENTATION_BY_REFERENCE payload, then that kind's own trailing
     * field -- {@code parentObjectId} for a Topic/Publisher/Subscriber/
     * DataWriter/DataReader, {@code null} for every other kind --
     * byte-oracled in crates/xrce/src/object_repr.rs against real
     * eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
     * participant_id: commit 31c62df2a63). {@code reference} is a
     * NUL-terminated object_reference string (Sec 7.7.3.1.1).
     */
    public static byte[] frameCreate(int sessionId, byte[] clientKey, int seq, int requestId,
                                      int objectId, int objectKind, String reference,
                                      Integer parentObjectId, boolean reuse, boolean replace) {
        byte[] refBytes = reference.getBytes(StandardCharsets.UTF_8);
        int lenInclNul = refBytes.length + 1;
        // kind(1) + disc(1) + 2 pad + len(4) + payload + nul, then the
        // trailing parent ObjectId (2, unaligned) when present.
        int bodyLen = 4 + 1 + 1 + 2 + 4 + refBytes.length + 1 + (parentObjectId != null ? 2 : 0);
        int flags = FLAG_E | (reuse ? FLAG_REUSE : 0) | (replace ? FLAG_REPLACE : 0);

        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey);
        putSmHeader(out, SM_CREATE, flags, bodyLen);
        out.write(requestIdBytes(requestId), 0, 2);
        out.write(objectIdBytes(objectId), 0, 2);
        out.write(objectKind & 0xFF); // outer ObjectKind byte (Sec 7.7.3)
        out.write(0x01); // inner disc = REPRESENTATION_BY_REFERENCE
        out.write(0); out.write(0); // 2 bytes padding (the outer kind byte already took one)
        out.write(lenInclNul & 0xFF);
        out.write((lenInclNul >> 8) & 0xFF);
        out.write((lenInclNul >> 16) & 0xFF);
        out.write((lenInclNul >> 24) & 0xFF);
        out.write(refBytes, 0, refBytes.length);
        out.write(0); // NUL terminator
        if (parentObjectId != null) {
            out.write(objectIdBytes(parentObjectId), 0, 2);
        }
        return out.toByteArray();
    }

    /**
     * DELETE (Sec 7.8.3/Sec 8.3.5.4): MessageHeader WITH the ClientKey +
     * one DELETE submessage (Sec 7.7.8 BaseObjectRequest only).
     */
    public static byte[] frameDelete(int sessionId, byte[] clientKey, int seq, int requestId, int objectId) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey);
        putSmHeader(out, SM_DELETE, FLAG_E, 4);
        out.write(requestIdBytes(requestId), 0, 2);
        out.write(objectIdBytes(objectId), 0, 2);
        return out.toByteArray();
    }

    /**
     * WRITE_DATA (Sec 7.8.4/Sec 8.3.5.8, FORMAT_DATA): MessageHeader WITH
     * the ClientKey + one WRITE_DATA submessage (Sec 7.7.8
     * BaseObjectRequest + {@code sample} verbatim, no length of its own --
     * DDS-XRCE 1.0 Annex B.3.1).
     */
    public static byte[] frameWriteData(int sessionId, byte[] clientKey, int seq, int requestId,
                                         int writerObjectId, byte[] sample) {
        int bodyLen = 4 + sample.length;
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey);
        putSmHeader(out, SM_WRITE_DATA, FLAG_E, bodyLen);
        out.write(requestIdBytes(requestId), 0, 2);
        out.write(objectIdBytes(writerObjectId), 0, 2);
        out.write(sample, 0, sample.length);
        return out.toByteArray();
    }

    /**
     * READ_DATA (Sec 7.8.5/Sec 8.3.5.9): MessageHeader WITH the ClientKey +
     * one READ_DATA submessage (Sec 7.7.8 BaseObjectRequest + a minimal
     * ReadSpecification: no content filter, no delivery control -- both
     * optional members absent). {@code preferredStreamId} selects the
     * StreamId the agent should use for the DATA reply.
     */
    public static byte[] frameReadData(int sessionId, byte[] clientKey, int seq, int requestId,
                                        int readerObjectId, int preferredStreamId) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey);
        putSmHeader(out, SM_READ_DATA, FLAG_E, 8);
        out.write(requestIdBytes(requestId), 0, 2);
        out.write(objectIdBytes(readerObjectId), 0, 2);
        out.write(preferredStreamId & 0xFF);
        out.write(0x00); // data_format = FORMAT_DATA
        out.write(0x00); // content_filter_expression? = absent
        out.write(0x00); // delivery_control? = absent
        return out.toByteArray();
    }
}
