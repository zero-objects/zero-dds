// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 session client + wire codec -- a port of
// `crates/xrce-client/src/lib.rs`'s `XrceClient<T>` state machine combined
// with the concrete submessage codecs of `crates/xrce`
// (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). `crates/xrce-client`
// itself never touches the wire -- its module doc says so directly: "the
// caller delegates submessage encoding; we only track state". This file is
// that combination, matching endpoints/c's `zdwc_*` framing (the port
// every fixture in endpoints/testdata/xrce-client/ was verified against)
// rather than reinventing the byte layout.
//
// Distinct from `xrceWriteFrame` in Zerodds.kt, the simplified "ZeroDDS
// Endpoint Profile" framing the sync/async examples use (sample body only,
// no BaseObjectRequest, no ClientKey) -- this file speaks the
// spec-conformant object-lifecycle frames a real XRCE agent expects for
// CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the conformant WRITE_DATA
// that carries the target ObjectId (DDS-XRCE 1.0 Annex B.3.1).
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
//   XrceClient ctor -> §7.8.2 CREATE_CLIENT
//   connect          -> §8.4.5 (handshake)
//   createObject     -> §7.8.3 CREATE
//   deleteObject     -> §7.8.3 DELETE
//   requestWrite     -> §7.8.4 WRITE_DATA
//   requestRead      -> §7.8.5 READ_DATA
//   disconnect       -> §8.4.5

package zerodds

import java.io.ByteArrayOutputStream

// ObjectKind codes actually exercised by this client (DDS-XRCE 1.0 Table 4).
object XrceObjectKind {
    const val PARTICIPANT = 0x01
    const val TOPIC = 0x02
    const val PUBLISHER = 0x03
    const val SUBSCRIBER = 0x04
    const val DATAWRITER = 0x05
    const val DATAREADER = 0x06
}

// ObjectId (spec §7.2.1): a 12-bit raw id packed with a 4-bit ObjectKind
// into a 16-bit wire value, big-endian on the wire (crates/xrce
// object_id.rs::to_bytes). `xrceObjectId` itself is `Zerodds.kt`'s --
// same package, same signature, same computation; this file's own
// callers (and its test) already require Zerodds.kt on the same
// `kotlinc` command line, so redeclaring it here would only be a second
// copy of the same one line, not a second implementation.

private fun objectIdBytes(id: Int): ByteArray =
    byteArrayOf(((id shr 8) and 0xFF).toByte(), (id and 0xFF).toByte())

/**
 * "Representation timestamps": whether CLIENT_Representation carries a
 * `Time_t client_timestamp` after `xrce_vendor_id`. DDS-XRCE 1.0 contradicts
 * itself: the prose of §7.8.2.1 lists it, the IDL of Annex A and the bytes of
 * Annex B.1 do not. ABSENT (default) is the Annex A form, PRESENT the prose
 * form. The agent has to be set the same way (`representation_timestamps:`
 * in its configuration).
 */
enum class RepresentationTimestamps { ABSENT, PRESENT }

/**
 * Extensibility of the unannotated Time_t (DDS-XTypes 1.3 7.3.1.2.1.8):
 * APPENDABLE (default) writes a DHEADER = 8 in front, FINAL does not. Only
 * used with RepresentationTimestamps.PRESENT.
 */
enum class XrceUnannotatedTypes { APPENDABLE, FINAL }

/** Lifecycle state, mirrors crates/xrce-client's ClientState. */
enum class XrceClientState { DISCONNECTED, CONNECTING, CONNECTED }

/** Mirrors ClientError::InvalidState. */
class XrceClientError(message: String) : Exception(message)

// Synchronous XRCE client state machine -- a port of crates/xrce-client's
// `XrceClient<T>`. It does not hold a transport and does not send anything
// itself, exactly like the Rust crate: pair its request ids with the
// XrceFrame.* builders below and hand the result to your own transport.
class XrceClient(
    val clientKey: ByteArray,
    var representationTimestamps: RepresentationTimestamps = RepresentationTimestamps.ABSENT,
    var unannotatedTypes: XrceUnannotatedTypes = XrceUnannotatedTypes.APPENDABLE,
) {

    var state: XrceClientState = XrceClientState.DISCONNECTED
        private set
    var sessionId: Int = 0
        private set
    private var nextRequestId = 1

    // §8.4.5 handshake: proposes `requestedSessionId` (1..127, §8.3.2.1)
    // and moves DISCONNECTED -> CONNECTING. The caller still has to build
    // and deliver CREATE_CLIENT itself, e.g. via frameCreateClient().
    fun connect(requestedSessionId: Int) {
        if (state != XrceClientState.DISCONNECTED) throw XrceClientError("invalid state: $state")
        this.sessionId = requestedSessionId
        this.state = XrceClientState.CONNECTING
    }

    // CREATE_CLIENT for this client's session id and key in this client's
    // representation timestamps setting; with PRESENT the timestamp is the
    // wall clock at the time of the call.
    fun frameCreateClient(): ByteArray {
        val now = System.currentTimeMillis()
        return XrceFrame.createClient(
            sessionId, clientKey, representationTimestamps, unannotatedTypes,
            (now / 1000L).toInt(), ((now % 1000L) * 1000000L).toInt(),
        )
    }

    // Completes the handshake once the caller has received STATUS_AGENT
    // (CONNECTING -> CONNECTED). §8.4.5.
    fun markConnected() {
        if (state != XrceClientState.CONNECTING) throw XrceClientError("invalid state: $state")
        this.state = XrceClientState.CONNECTED
    }

    // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
    // never a valid request id) -- identical to
    // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
    private fun allocRequestId(): Int {
        val req = nextRequestId
        val next = (req + 1) and 0xFFFF
        nextRequestId = if (next == 0) 1 else next
        return req
    }

    private fun requireConnected() {
        if (state != XrceClientState.CONNECTED) throw XrceClientError("invalid state: $state")
    }

    /** Submits a CREATE (§7.8.3). Returns a request id. */
    fun createObject(): Int { requireConnected(); return allocRequestId() }

    /** Submits a DELETE (§7.8.3). */
    fun deleteObject(): Int { requireConnected(); return allocRequestId() }

    /** Submits a WRITE_DATA (§7.8.4). */
    fun requestWrite(): Int { requireConnected(); return allocRequestId() }

    /** Submits a READ_DATA (§7.8.5, pull-based). */
    fun requestRead(): Int { requireConnected(); return allocRequestId() }

    /** §8.4.5: closes the session -- back to DISCONNECTED. Local state only. */
    fun disconnect() { this.state = XrceClientState.DISCONNECTED }
}

// Wire framing: MessageHeader (spec §8.3.2) + one SubmessageHeader (spec
// §8.3.4) + body. Every frame carries the ClientKey (session_id <= 127,
// §8.3.2.1) -- the fixtures in endpoints/testdata/xrce-client/ use it
// uniformly, including for WRITE_DATA/READ_DATA, matching endpoints/c's
// `zdwc_frame_*`.
object XrceFrame {
    const val SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00
    const val STREAM_ID_NONE = 0x00
    const val STREAM_ID_BEST_EFFORT = 0x01
    const val STREAM_ID_BUILTIN_RELIABLE = 0x80

    // eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
    // module doc has the byte-oracle measurement backing it).
    const val DEFAULT_MTU = 508

    private const val SM_CREATE_CLIENT = 0x00
    private const val SM_CREATE = 0x01
    private const val SM_DELETE = 0x03
    private const val SM_WRITE_DATA = 0x07
    private const val SM_READ_DATA = 0x08

    private const val FLAG_E = 0x01 // E-flag: body is little-endian (§8.3.4).
    private const val FLAG_REUSE = 0x02
    private const val FLAG_REPLACE = 0x04

    // request_id (spec §7.7.8) as this port's own choice of byte order for
    // a host-side monotonic counter: big-endian, matching ObjectId's own
    // wire order and crates/xrce-client::request_id_octets (the answer
    // this repository settled on).
    private fun requestIdBytes(requestId: Int): ByteArray =
        byteArrayOf(((requestId shr 8) and 0xFF).toByte(), (requestId and 0xFF).toByte())

    private fun putHeader(out: ByteArrayOutputStream, sessionId: Int, streamId: Int, seq: Int, clientKey: ByteArray) {
        out.write(sessionId)
        out.write(streamId)
        out.write(seq and 0xFF)
        out.write((seq shr 8) and 0xFF)
        out.write(clientKey, 0, 4)
    }

    private fun putSmHeader(out: ByteArrayOutputStream, smId: Int, flags: Int, bodyLen: Int) {
        require(bodyLen <= 0xFFFF) { "body too large: $bodyLen" }
        out.write(smId)
        out.write(flags)
        out.write(bodyLen and 0xFF)
        out.write((bodyLen shr 8) and 0xFF)
    }

    private fun putLe32(out: ByteArrayOutputStream, v: Int) {
        out.write(v and 0xFF); out.write((v shr 8) and 0xFF)
        out.write((v shr 16) and 0xFF); out.write((v shr 24) and 0xFF)
    }

    // CLIENT_Representation (Annex A, §7.8.2.1): xrce_cookie(4) +
    // xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1) +
    // a `0` properties-absent octet + requestedMtu (2, little endian) --
    // eProsima's own non-normative MTU extension (crates/xrce/src/
    // handshake.rs's module doc has the byte-oracle measurement backing
    // DEFAULT_MTU), always sent by this client. No PropertySeq support
    // here -- this client sends none.
    //
    // Representation timestamps: PRESENT writes the Time_t of the client
    // after xrce_vendor_id, seconds (i32) and nanoseconds (u32) little
    // endian; APPENDABLE puts a DHEADER = 8 in front (12 octets), FINAL
    // none (8). The following offsets shift. `seconds`/`nanoseconds` are
    // ignored with ABSENT (the default, bytes unchanged).
    fun encodeClientRepresentation(
        version: Int, vendorId: Int, clientKey: ByteArray, sessionId: Int, requestedMtu: Int = DEFAULT_MTU,
        timestamps: RepresentationTimestamps = RepresentationTimestamps.ABSENT,
        extensibility: XrceUnannotatedTypes = XrceUnannotatedTypes.APPENDABLE,
        seconds: Int = 0, nanoseconds: Int = 0,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write("XRCE".toByteArray(Charsets.US_ASCII))
        out.write((version shr 8) and 0xFF); out.write(version and 0xFF)
        out.write((vendorId shr 8) and 0xFF); out.write(vendorId and 0xFF)
        if (timestamps == RepresentationTimestamps.PRESENT) {
            if (extensibility == XrceUnannotatedTypes.APPENDABLE) putLe32(out, 8) // DHEADER of Time_t
            putLe32(out, seconds)
            putLe32(out, nanoseconds)
        }
        out.write(clientKey, 0, 4)
        out.write(sessionId and 0xFF)
        out.write(0) // properties absent
        out.write(requestedMtu and 0xFF); out.write((requestedMtu shr 8) and 0xFF)
        return out.toByteArray()
    }

    // CREATE_CLIENT (§7.8.2/§8.3.5.1): no session exists yet, so the
    // MessageHeader session_id is SESSION_ID_NONE_WITH_CLIENT_KEY (0x00)
    // when the requested `sessionId` is 0..127 (header carries the
    // ClientKey) and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) when it is
    // 128..255 (header without ClientKey), DDS-XRCE 1.0 §8.3.2.1. The
    // requested `sessionId` itself travels only in the CLIENT_Representation
    // payload. stream_id=0x00 STREAM_ID_NONE; sequence_nr=0; one
    // CREATE_CLIENT submessage (flags=0x01, E-flag only) whose body is a
    // real encodeClientRepresentation, not an ad hoc placeholder.
    fun createClient(
        sessionId: Int, clientKey: ByteArray,
        timestamps: RepresentationTimestamps = RepresentationTimestamps.ABSENT,
        extensibility: XrceUnannotatedTypes = XrceUnannotatedTypes.APPENDABLE,
        seconds: Int = 0, nanoseconds: Int = 0,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        if (sessionId < 128) {
            putHeader(out, 0x00, STREAM_ID_NONE, 0, clientKey)
        } else {
            out.write(0x80); out.write(STREAM_ID_NONE); out.write(0); out.write(0)
        }
        val bodyBytes = encodeClientRepresentation(
            0x0100, 0x0000, clientKey, sessionId, DEFAULT_MTU, timestamps, extensibility, seconds, nanoseconds,
        )
        putSmHeader(out, SM_CREATE_CLIENT, FLAG_E, bodyBytes.size)
        out.write(bodyBytes)
        return out.toByteArray()
    }

    // CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
    // WITH the ClientKey + one CREATE submessage (§7.7.8 BaseObjectRequest
    // + the real, two-level §7.7.3 ObjectVariant: the outer `objectKind`
    // byte, then the inner REPRESENTATION_BY_REFERENCE payload, then that
    // kind's own trailing field -- `parentObjectId` for a
    // Topic/Publisher/Subscriber/DataWriter/DataReader, `null` for every
    // other kind -- byte-oracled in crates/xrce/src/object_repr.rs against
    // real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's
    // own participant_id: commit 31c62df2a63). `reference` is a
    // NUL-terminated object_reference string (§7.7.3.1.1).
    fun create(
        sessionId: Int, clientKey: ByteArray, seq: Int, requestId: Int, objectId: Int, objectKind: Int,
        reference: String, parentObjectId: Int? = null, reuse: Boolean = false, replace: Boolean = false,
    ): ByteArray {
        val refBytes = reference.toByteArray(Charsets.UTF_8)
        val lenInclNul = refBytes.size + 1
        // kind(1) + disc(1) + 2 pad + len(4) + payload + nul, then the
        // trailing parent ObjectId (2, unaligned) when present.
        val bodyLen = 4 + 1 + 1 + 2 + 4 + refBytes.size + 1 + (if (parentObjectId != null) 2 else 0)
        val flags = FLAG_E or (if (reuse) FLAG_REUSE else 0) or (if (replace) FLAG_REPLACE else 0)

        val out = ByteArrayOutputStream()
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey)
        putSmHeader(out, SM_CREATE, flags, bodyLen)
        out.write(requestIdBytes(requestId))
        out.write(objectIdBytes(objectId))
        out.write(objectKind and 0xFF) // outer ObjectKind byte (§7.7.3)
        out.write(0x01) // inner disc = REPRESENTATION_BY_REFERENCE
        out.write(0); out.write(0) // 2 bytes padding (the outer kind byte already took one)
        out.write(lenInclNul and 0xFF)
        out.write((lenInclNul shr 8) and 0xFF)
        out.write((lenInclNul shr 16) and 0xFF)
        out.write((lenInclNul shr 24) and 0xFF)
        out.write(refBytes)
        out.write(0) // NUL terminator
        if (parentObjectId != null) {
            out.write(objectIdBytes(parentObjectId))
        }
        return out.toByteArray()
    }

    // DELETE (§7.8.3/§8.3.5.4): MessageHeader WITH the ClientKey + one
    // DELETE submessage (§7.7.8 BaseObjectRequest only).
    fun delete(sessionId: Int, clientKey: ByteArray, seq: Int, requestId: Int, objectId: Int): ByteArray {
        val out = ByteArrayOutputStream()
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey)
        putSmHeader(out, SM_DELETE, FLAG_E, 4)
        out.write(requestIdBytes(requestId))
        out.write(objectIdBytes(objectId))
        return out.toByteArray()
    }

    // WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA): MessageHeader WITH the
    // ClientKey + one WRITE_DATA submessage (§7.7.8 BaseObjectRequest +
    // `sample` verbatim, no length of its own -- DDS-XRCE 1.0 Annex B.3.1).
    fun writeData(
        sessionId: Int, clientKey: ByteArray, seq: Int, requestId: Int, writerObjectId: Int, sample: ByteArray,
    ): ByteArray {
        val bodyLen = 4 + sample.size
        val out = ByteArrayOutputStream()
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey)
        putSmHeader(out, SM_WRITE_DATA, FLAG_E, bodyLen)
        out.write(requestIdBytes(requestId))
        out.write(objectIdBytes(writerObjectId))
        out.write(sample)
        return out.toByteArray()
    }

    // READ_DATA (§7.8.5/§8.3.5.9): MessageHeader WITH the ClientKey + one
    // READ_DATA submessage (§7.7.8 BaseObjectRequest + a minimal
    // ReadSpecification: no content filter, no delivery control -- both
    // optional members absent). `preferredStreamId` selects the StreamId
    // the agent should use for the DATA reply.
    fun readData(
        sessionId: Int, clientKey: ByteArray, seq: Int, requestId: Int, readerObjectId: Int, preferredStreamId: Int,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        putHeader(out, sessionId, STREAM_ID_BUILTIN_RELIABLE, seq, clientKey)
        putSmHeader(out, SM_READ_DATA, FLAG_E, 8)
        out.write(requestIdBytes(requestId))
        out.write(objectIdBytes(readerObjectId))
        out.write(preferredStreamId and 0xFF)
        out.write(0x00) // data_format = FORMAT_DATA
        out.write(0x00) // content_filter_expression? = absent
        out.write(0x00) // delivery_control? = absent
        return out.toByteArray()
    }
}
