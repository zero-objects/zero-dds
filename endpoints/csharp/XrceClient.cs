// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-XRCE 1.0 session client + wire codec -- a port of
// `crates/xrce-client/src/lib.rs`'s `XrceClient<T>` state machine combined
// with the concrete submessage codecs of `crates/xrce`
// (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA). `crates/xrce-client`
// itself never touches the wire -- its module doc says so directly: "the
// caller delegates submessage encoding; we only track state". This module
// is that combination, matching endpoints/c's `zdwc_*` framing (the port
// every fixture in endpoints/testdata/xrce-client/ was verified against)
// rather than reinventing the byte layout.
//
// Distinct from `Zerodds.cs`'s own XRCE WRITE_DATA framing used by the
// sync/async endpoint examples (the simplified "ZeroDDS Endpoint Profile"
// framing: sample body only, no BaseObjectRequest, no ClientKey) -- this
// module speaks the spec-conformant object-lifecycle frames a real XRCE
// agent expects for CREATE_CLIENT/CREATE/DELETE/READ_DATA, plus the
// conformant WRITE_DATA that carries the target ObjectId (DDS-XRCE 1.0
// Annex B.3.1).
//
// Spec mapping (mirrors crates/xrce-client/src/lib.rs's module doc):
//   XrceClient ctor -> §7.8.2 CREATE_CLIENT
//   Connect          -> §8.4.5 (handshake)
//   CreateObject     -> §7.8.3 CREATE
//   DeleteObject     -> §7.8.3 DELETE
//   RequestWrite     -> §7.8.4 WRITE_DATA
//   RequestRead      -> §7.8.5 READ_DATA
//   Disconnect       -> §8.4.5

using System;
using System.Collections.Generic;
using System.Text;

namespace ZeroDDS
{
    // ObjectKind codes actually exercised by this client (DDS-XRCE 1.0
    // Table 4). A full kind table belongs to a codegen layer, not this
    // hand-written session client.
    public static class XrceObjectKind
    {
        public const byte Participant = 0x01;
        public const byte Topic = 0x02;
        public const byte Publisher = 0x03;
        public const byte Subscriber = 0x04;
        public const byte DataWriter = 0x05;
        public const byte DataReader = 0x06;
    }

    // ObjectId (spec §7.2.1): a 12-bit raw id packed with a 4-bit
    // ObjectKind into a 16-bit wire value, big-endian on the wire
    // (crates/xrce `object_id.rs::to_bytes`).
    public static class XrceObjectId
    {
        public static ushort New(int rawId, byte kind) =>
            (ushort)(((rawId & 0x0FFF) << 4) | (kind & 0x0F));

        public static byte[] Bytes(ushort id) => new byte[] { (byte)(id >> 8), (byte)id };
    }

    public enum XrceClientState { Disconnected, Connecting, Connected }

    // Mirrors `ClientError::InvalidState`: an operation was attempted in
    // the wrong lifecycle state (e.g. CreateObject before Connect).
    public sealed class XrceClientError : Exception
    {
        public XrceClientError(string message) : base(message) { }
    }

    // "Representation timestamps": whether CLIENT_Representation carries a
    // `Time_t client_timestamp` after `xrce_vendor_id`. DDS-XRCE 1.0
    // contradicts itself: the prose of §7.8.2.1 lists it, the IDL of Annex A
    // and the bytes of Annex B.1 do not. Absent (default) is the Annex A
    // form, Present the prose form. The agent has to be set the same way
    // (`representation_timestamps:` in its configuration).
    public enum RepresentationTimestamps { Absent, Present }

    // Extensibility of the unannotated Time_t (DDS-XTypes 1.3
    // 7.3.1.2.1.8): Appendable (default) writes a DHEADER = 8 in front,
    // Final does not. Only used with RepresentationTimestamps.Present.
    public enum XrceUnannotatedTypes { Appendable, Final }

    // Synchronous XRCE client state machine -- a port of
    // crates/xrce-client's `XrceClient<T>`. It does not hold a transport
    // and does not send anything itself, exactly like the Rust crate: pair
    // its request ids with the XrceFrame.* builders below and hand the
    // result to your own transport.
    public sealed class XrceClient
    {
        public byte[] ClientKey { get; }
        public XrceClientState State { get; private set; } = XrceClientState.Disconnected;
        public byte SessionId { get; private set; }

        private ushort _nextRequestId = 1;

        public RepresentationTimestamps RepresentationTimestamps { get; set; } = RepresentationTimestamps.Absent;
        public XrceUnannotatedTypes UnannotatedTypes { get; set; } = XrceUnannotatedTypes.Appendable;

        public XrceClient(byte[] clientKey) => ClientKey = clientKey;

        public XrceClient(byte[] clientKey, RepresentationTimestamps representationTimestamps)
        {
            ClientKey = clientKey;
            RepresentationTimestamps = representationTimestamps;
        }

        // CREATE_CLIENT for this client's session id and key in this
        // client's representation timestamps setting; with Present the
        // timestamp is the wall clock at the time of the call.
        public byte[] FrameCreateClient()
        {
            long ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            return XrceFrame.CreateClient(SessionId, ClientKey, RepresentationTimestamps, UnannotatedTypes,
                                          (int)(ms / 1000), (uint)((ms % 1000) * 1000000));
        }

        // §8.4.5 Handshake: proposes `requestedSessionId` (1..127, spec
        // §8.3.2.1) and moves Disconnected -> Connecting. The caller still
        // has to build and deliver CREATE_CLIENT itself, e.g. via
        // FrameCreateClient().
        public void Connect(byte requestedSessionId)
        {
            if (State != XrceClientState.Disconnected)
                throw new XrceClientError($"invalid state: {State}");
            SessionId = requestedSessionId;
            State = XrceClientState.Connecting;
        }

        // Completes the handshake once the caller has received
        // STATUS_AGENT (Connecting -> Connected). §8.4.5.
        public void MarkConnected()
        {
            if (State != XrceClientState.Connecting)
                throw new XrceClientError($"invalid state: {State}");
            State = XrceClientState.Connected;
        }

        // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
        // never a valid request id) -- identical to
        // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
        private ushort AllocRequestId()
        {
            ushort req = _nextRequestId;
            int next = (req + 1) & 0xFFFF;
            _nextRequestId = (ushort)(next == 0 ? 1 : next);
            return req;
        }

        private void RequireConnected()
        {
            if (State != XrceClientState.Connected)
                throw new XrceClientError($"invalid state: {State}");
        }

        // Submits a CREATE (§7.8.3). Returns a request id.
        public ushort CreateObject()
        {
            RequireConnected();
            return AllocRequestId();
        }

        // Submits a DELETE (§7.8.3).
        public ushort DeleteObject()
        {
            RequireConnected();
            return AllocRequestId();
        }

        // Submits a WRITE_DATA (§7.8.4).
        public ushort RequestWrite()
        {
            RequireConnected();
            return AllocRequestId();
        }

        // Submits a READ_DATA (§7.8.5, pull-based).
        public ushort RequestRead()
        {
            RequireConnected();
            return AllocRequestId();
        }

        // §8.4.5: closes the session -- back to Disconnected. Like the
        // Rust client, this sends nothing.
        public void Disconnect() => State = XrceClientState.Disconnected;
    }

    // Wire framing: MessageHeader (spec §8.3.2) + one SubmessageHeader
    // (spec §8.3.4) + body. Every frame this client builds carries the
    // ClientKey (session_id <= 127, §8.3.2.1) -- the fixtures in
    // endpoints/testdata/xrce-client/ use it uniformly, including for
    // WRITE_DATA/READ_DATA, matching endpoints/c's `zdwc_frame_*`.
    public static class XrceFrame
    {
        public const byte SessionIdNoneWithClientKey = 0x00;
        public const byte StreamIdNone = 0x00;
        public const byte StreamIdBestEffort = 0x01;
        public const byte StreamIdBuiltinReliable = 0x80;

        // eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
        // module doc has the byte-oracle measurement backing it).
        public const ushort DefaultMtu = 508;

        private const byte SmCreateClient = 0x00;
        private const byte SmCreate = 0x01;
        private const byte SmDelete = 0x03;
        private const byte SmWriteData = 0x07;
        private const byte SmReadData = 0x08;

        private const byte FlagE = 0x01; // E-flag: submessage body is little-endian (§8.3.4).
        private const byte FlagReuse = 0x02;
        private const byte FlagReplace = 0x04;

        // request_id (spec §7.7.8) as this port's own choice of byte order
        // for a host-side monotonic counter: big-endian, matching
        // ObjectId's own wire order and `crates/xrce-client
        // ::request_id_octets` (the answer this repository settled on).
        private static byte[] RequestIdBytes(ushort requestId) =>
            new byte[] { (byte)(requestId >> 8), (byte)requestId };

        // MessageHeader WITH ClientKey (8 bytes): session_id(1) +
        // stream_id(1) + sequence_nr(2, LE) + ClientKey(4).
        private static byte[] Header(byte sessionId, byte streamId, ushort seq, byte[] clientKey)
        {
            var h = new byte[8];
            h[0] = sessionId;
            h[1] = streamId;
            h[2] = (byte)seq;
            h[3] = (byte)(seq >> 8);
            Array.Copy(clientKey, 0, h, 4, 4);
            return h;
        }

        // SubmessageHeader (4 bytes): id + flags + body length (LE, always
        // -- spec §8.3.4).
        private static byte[] SmHeader(byte smId, byte flags, int bodyLen)
        {
            if (bodyLen > 0xFFFF) throw new ArgumentOutOfRangeException(nameof(bodyLen));
            return new byte[] { smId, flags, (byte)bodyLen, (byte)(bodyLen >> 8) };
        }

        private static byte[] Concat(params byte[][] parts)
        {
            int n = 0;
            foreach (var p in parts) n += p.Length;
            var outBuf = new byte[n];
            int off = 0;
            foreach (var p in parts) { Array.Copy(p, 0, outBuf, off, p.Length); off += p.Length; }
            return outBuf;
        }

        private static void PutLe32(byte[] b, int off, uint v)
        {
            b[off] = (byte)v; b[off + 1] = (byte)(v >> 8);
            b[off + 2] = (byte)(v >> 16); b[off + 3] = (byte)(v >> 24);
        }

        // CLIENT_Representation (Annex A, §7.8.2.1): xrce_cookie(4) +
        // xrce_version(2) + xrce_vendor_id(2) + client_key(4) +
        // session_id(1) + a `0` properties-absent octet + requestedMtu (2,
        // little endian) -- eProsima's own non-normative MTU extension
        // (crates/xrce/src/handshake.rs's module doc has the byte-oracle
        // measurement backing DefaultMtu), always sent by this client. No
        // PropertySeq support here -- this client sends none.
        public static byte[] EncodeClientRepresentation(ushort version, ushort vendorId,
                                                          byte[] clientKey, byte sessionId,
                                                          ushort requestedMtu = DefaultMtu,
                                                          RepresentationTimestamps timestamps = RepresentationTimestamps.Absent,
                                                          XrceUnannotatedTypes extensibility = XrceUnannotatedTypes.Appendable,
                                                          int seconds = 0, uint nanoseconds = 0)
        {
            // Present: Time_t after xrce_vendor_id, seconds (i32) and
            // nanoseconds (u32) little endian; Appendable puts a DHEADER = 8
            // in front (12 octets), Final none (8). The following offsets
            // shift. Seconds/nanoseconds are ignored with Absent.
            byte[] time = Array.Empty<byte>();
            if (timestamps == RepresentationTimestamps.Present)
            {
                bool dheader = extensibility == XrceUnannotatedTypes.Appendable;
                time = new byte[dheader ? 12 : 8];
                int o = 0;
                if (dheader) { PutLe32(time, 0, 8); o = 4; }
                PutLe32(time, o, unchecked((uint)seconds));
                PutLe32(time, o + 4, nanoseconds);
            }
            return Concat(
                Encoding.ASCII.GetBytes("XRCE"),
                new byte[] { (byte)(version >> 8), (byte)version },
                new byte[] { (byte)(vendorId >> 8), (byte)vendorId },
                time,
                clientKey,
                new byte[] { sessionId, 0x00 }, // session_id, then properties-absent
                new byte[] { (byte)requestedMtu, (byte)(requestedMtu >> 8) });
        }

        // CREATE_CLIENT (§7.8.2/§8.3.5.1): no session exists yet, so the
        // MessageHeader session_id is SESSION_ID_NONE_WITH_CLIENT_KEY
        // (0x00) when the requested `sessionId` is 0..127 (header carries
        // the ClientKey) and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80)
        // when it is 128..255 (header without ClientKey), DDS-XRCE 1.0
        // §8.3.2.1. The requested `sessionId` itself travels only in the
        // CLIENT_Representation payload. stream_id=0x00 STREAM_ID_NONE;
        // sequence_nr=0; one CREATE_CLIENT submessage (flags=0x01, E-flag
        // only) whose body is a real EncodeClientRepresentation, not an
        // ad hoc placeholder.
        public static byte[] CreateClient(byte sessionId, byte[] clientKey,
                                          RepresentationTimestamps timestamps = RepresentationTimestamps.Absent,
                                          XrceUnannotatedTypes extensibility = XrceUnannotatedTypes.Appendable,
                                          int seconds = 0, uint nanoseconds = 0)
        {
            var header = sessionId < 128
                ? Header(0x00, StreamIdNone, 0, clientKey)
                : new byte[] { 0x80, StreamIdNone, 0, 0 };
            var body = EncodeClientRepresentation(0x0100, 0x0000, clientKey, sessionId, DefaultMtu,
                                                timestamps, extensibility, seconds, nanoseconds);
            return Concat(header, SmHeader(SmCreateClient, FlagE, body.Length), body);
        }

        // CREATE(REPRESENTATION_BY_REFERENCE) on an open session:
        // MessageHeader WITH the ClientKey + one CREATE submessage
        // (§7.7.8 BaseObjectRequest + the real, two-level §7.7.3
        // ObjectVariant: the outer `objectKind` byte, then the inner
        // REPRESENTATION_BY_REFERENCE payload, then that kind's own
        // trailing field -- `parentObjectId` for a
        // Topic/Publisher/Subscriber/DataWriter/DataReader, `null` for
        // every other kind -- byte-oracled in
        // crates/xrce/src/object_repr.rs against real eProsima
        // Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
        // participant_id: commit 31c62df2a63). `reference` is a
        // NUL-terminated object_reference string (§7.7.3.1.1).
        public static byte[] Create(byte sessionId, byte[] clientKey, ushort seq,
                                     ushort requestId, ushort objectId, byte objectKind,
                                     string reference, ushort? parentObjectId,
                                     bool reuse, bool replace)
        {
            var header = Header(sessionId, StreamIdBuiltinReliable, seq, clientKey);
            var refBytes = Encoding.UTF8.GetBytes(reference);
            int lenInclNul = refBytes.Length + 1;
            var variantParts = new List<byte[]>
            {
                new byte[] { objectKind }, // outer ObjectKind byte (§7.7.3)
                new byte[] { 0x01, 0x00, 0x00 }, // inner disc=BY_REFERENCE + 2 padding
                new byte[] { (byte)lenInclNul, (byte)(lenInclNul >> 8),
                             (byte)(lenInclNul >> 16), (byte)(lenInclNul >> 24) },
                refBytes,
                new byte[] { 0x00 }, // NUL terminator
            };
            if (parentObjectId.HasValue) variantParts.Add(XrceObjectId.Bytes(parentObjectId.Value));
            var variant = Concat(variantParts.ToArray());
            var body = Concat(RequestIdBytes(requestId), XrceObjectId.Bytes(objectId), variant);
            byte flags = FlagE;
            if (reuse) flags |= FlagReuse;
            if (replace) flags |= FlagReplace;
            return Concat(header, SmHeader(SmCreate, flags, body.Length), body);
        }

        // DELETE (§7.8.3/§8.3.5.4): MessageHeader WITH the ClientKey + one
        // DELETE submessage (§7.7.8 BaseObjectRequest only).
        public static byte[] Delete(byte sessionId, byte[] clientKey, ushort seq,
                                     ushort requestId, ushort objectId)
        {
            var header = Header(sessionId, StreamIdBuiltinReliable, seq, clientKey);
            var body = Concat(RequestIdBytes(requestId), XrceObjectId.Bytes(objectId));
            return Concat(header, SmHeader(SmDelete, FlagE, body.Length), body);
        }

        // WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA): MessageHeader WITH
        // the ClientKey + one WRITE_DATA submessage (§7.7.8
        // BaseObjectRequest + `sample` verbatim, no length of its own --
        // DDS-XRCE 1.0 Annex B.3.1).
        public static byte[] WriteData(byte sessionId, byte[] clientKey, ushort seq,
                                        ushort requestId, ushort writerObjectId, byte[] sample)
        {
            var header = Header(sessionId, StreamIdBuiltinReliable, seq, clientKey);
            var body = Concat(RequestIdBytes(requestId), XrceObjectId.Bytes(writerObjectId), sample);
            return Concat(header, SmHeader(SmWriteData, FlagE, body.Length), body);
        }

        // READ_DATA (§7.8.5/§8.3.5.9): MessageHeader WITH the ClientKey +
        // one READ_DATA submessage (§7.7.8 BaseObjectRequest + a minimal
        // ReadSpecification: no content filter, no delivery control --
        // both @optional members absent). `preferredStreamId` selects the
        // StreamId the agent should use for the DATA reply.
        public static byte[] ReadData(byte sessionId, byte[] clientKey, ushort seq,
                                       ushort requestId, ushort readerObjectId,
                                       byte preferredStreamId)
        {
            var header = Header(sessionId, StreamIdBuiltinReliable, seq, clientKey);
            var body = Concat(
                RequestIdBytes(requestId),
                XrceObjectId.Bytes(readerObjectId),
                new byte[] {
                    preferredStreamId,
                    0x00, // data_format = FORMAT_DATA
                    0x00, // content_filter_expression? = absent
                    0x00, // delivery_control? = absent
                });
            return Concat(header, SmHeader(SmReadData, FlagE, body.Length), body);
        }
    }
}
