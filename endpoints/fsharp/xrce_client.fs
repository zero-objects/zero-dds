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
// Distinct from `ZeroDDS`'s own XRCE WRITE_DATA framing used by the
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

module ZeroDDSXrce

open System
open System.Text

/// ObjectKind codes actually exercised by this client (DDS-XRCE 1.0 Table 4).
module XrceObjectKind =
    let Participant = 0x01uy
    let Topic = 0x02uy
    let Publisher = 0x03uy
    let Subscriber = 0x04uy
    let DataWriter = 0x05uy
    let DataReader = 0x06uy

/// ObjectId (spec §7.2.1): a 12-bit raw id packed with a 4-bit ObjectKind
/// into a 16-bit wire value, big-endian on the wire (crates/xrce
/// object_id.rs::to_bytes).
let xrceObjectId (rawId: int) (kind: byte) : uint16 =
    uint16 (((rawId &&& 0x0FFF) <<< 4) ||| int (kind &&& 0x0Fuy))

let private objectIdBytes (id: uint16) : byte[] =
    [| byte (id >>> 8); byte id |]

/// Lifecycle state, mirrors crates/xrce-client's ClientState.
type XrceClientState =
    | Disconnected
    | Connecting
    | Connected

/// Mirrors ClientError::InvalidState.
exception XrceClientError of string

/// "Representation timestamps": whether CLIENT_Representation carries a
/// `Time_t client_timestamp` after `xrce_vendor_id`. DDS-XRCE 1.0 contradicts
/// itself: the prose of §7.8.2.1 lists it, the IDL of Annex A and the bytes
/// of Annex B.1 do not. Absent (default) is the Annex A form, Present the
/// prose form. The agent has to be set the same way
/// (`representation_timestamps:` in its configuration).
[<RequireQualifiedAccess>]
type RepresentationTimestamps =
    | Absent
    | Present

/// Extensibility of the unannotated Time_t (DDS-XTypes 1.3 7.3.1.2.1.8):
/// Appendable (default) writes a DHEADER = 8 in front, Final does not. Only
/// used with RepresentationTimestamps.Present.
[<RequireQualifiedAccess>]
type XrceUnannotatedTypes =
    | Appendable
    | Final

/// Synchronous XRCE client state machine -- a port of crates/xrce-client's
/// `XrceClient<T>`. It does not hold a transport and does not send
/// anything itself, exactly like the Rust crate: pair its request ids with
/// the XrceFrame functions below and hand the result to your own
/// transport.
type XrceClient(clientKey: byte[]) =
    let mutable state = Disconnected
    let mutable sessionId = 0uy
    let mutable nextRequestId = 1us
    let mutable representationTimestamps = RepresentationTimestamps.Absent
    let mutable unannotatedTypes = XrceUnannotatedTypes.Appendable

    member _.ClientKey = clientKey
    member _.State = state
    member _.SessionId = sessionId

    /// Representation timestamps setting, default Absent.
    member _.RepresentationTimestamps
        with get () = representationTimestamps
        and set (v: RepresentationTimestamps) = representationTimestamps <- v

    /// Extensibility of Time_t with Present, default Appendable.
    member _.UnannotatedTypes
        with get () = unannotatedTypes
        and set (v: XrceUnannotatedTypes) = unannotatedTypes <- v

    /// §8.4.5 handshake: proposes `requestedSessionId` (1..127, §8.3.2.1)
    /// and moves Disconnected -> Connecting. The caller still has to build
    /// and deliver CREATE_CLIENT itself, e.g. via XrceFrame.createClientFor.
    member _.Connect(requestedSessionId: byte) =
        if state <> Disconnected then raise (XrceClientError(sprintf "invalid state: %A" state))
        sessionId <- requestedSessionId
        state <- Connecting

    /// Completes the handshake once the caller has received STATUS_AGENT
    /// (Connecting -> Connected). §8.4.5.
    member _.MarkConnected() =
        if state <> Connecting then raise (XrceClientError(sprintf "invalid state: %A" state))
        state <- Connected

    // Hands out the next request id, wrapping at 0xFFFF back to 1 (0 is
    // never a valid request id) -- identical to
    // `next_request_id.wrapping_add(1).max(1)` in crates/xrce-client.
    member private _.AllocRequestId() =
        let req = nextRequestId
        let next = (int req + 1) &&& 0xFFFF
        nextRequestId <- if next = 0 then 1us else uint16 next
        req

    member private _.RequireConnected() =
        if state <> Connected then raise (XrceClientError(sprintf "invalid state: %A" state))

    /// Submits a CREATE (§7.8.3). Returns a request id.
    member this.CreateObject() =
        this.RequireConnected()
        this.AllocRequestId()

    /// Submits a DELETE (§7.8.3).
    member this.DeleteObject() =
        this.RequireConnected()
        this.AllocRequestId()

    /// Submits a WRITE_DATA (§7.8.4).
    member this.RequestWrite() =
        this.RequireConnected()
        this.AllocRequestId()

    /// Submits a READ_DATA (§7.8.5, pull-based).
    member this.RequestRead() =
        this.RequireConnected()
        this.AllocRequestId()

    /// §8.4.5: closes the session -- back to Disconnected. Local state
    /// only.
    member _.Disconnect() = state <- Disconnected

/// Wire framing: MessageHeader (spec §8.3.2) + one SubmessageHeader (spec
/// §8.3.4) + body. Every frame carries the ClientKey (session_id <= 127,
/// §8.3.2.1) -- the fixtures in endpoints/testdata/xrce-client/ use it
/// uniformly, including for WRITE_DATA/READ_DATA, matching endpoints/c's
/// `zdwc_frame_*`.
module XrceFrame =
    let SessionIdNoneWithClientKey = 0x00uy
    let StreamIdNone = 0x00uy
    let StreamIdBestEffort = 0x01uy
    let StreamIdBuiltinReliable = 0x80uy

    /// eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
    /// module doc has the byte-oracle measurement backing it).
    let DefaultMtu = 508us

    let private smCreateClient = 0x00uy
    let private smCreate = 0x01uy
    let private smDelete = 0x03uy
    let private smWriteData = 0x07uy
    let private smReadData = 0x08uy

    let private flagE = 0x01uy // E-flag: body is little-endian (§8.3.4).
    let private flagReuse = 0x02uy
    let private flagReplace = 0x04uy

    // request_id (spec §7.7.8) as this port's own choice of byte order for
    // a host-side monotonic counter: big-endian, matching ObjectId's own
    // wire order and crates/xrce-client::request_id_octets (the answer
    // this repository settled on).
    let private requestIdBytes (requestId: uint16) : byte[] =
        [| byte (requestId >>> 8); byte requestId |]

    let private header (sessionId: byte) (streamId: byte) (seq: uint16) (clientKey: byte[]) : byte[] =
        [| sessionId; streamId; byte seq; byte (seq >>> 8); clientKey.[0]; clientKey.[1]; clientKey.[2]; clientKey.[3] |]

    let private smHeader (smId: byte) (flags: byte) (bodyLen: int) : byte[] =
        if bodyLen > 0xFFFF then invalidArg "bodyLen" (sprintf "body too large: %d" bodyLen)
        [| smId; flags; byte bodyLen; byte (bodyLen >>> 8) |]

    /// CLIENT_Representation (Annex A, §7.8.2.1): xrce_cookie(4) +
    /// xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1)
    /// + a `0` properties-absent octet + requestedMtu (2, little endian)
    /// -- eProsima's own non-normative MTU extension (crates/xrce/src/
    /// handshake.rs's module doc has the byte-oracle measurement backing
    /// DefaultMtu), always sent by this client. No PropertySeq support
    /// here -- this client sends none.
    ///
    /// With representation timestamps Present, the Time_t of the client
    /// follows xrce_vendor_id: seconds (i32) and nanoseconds (u32) little
    /// endian; Appendable puts a DHEADER = 8 in front (12 octets), Final
    /// none (8). The following offsets shift. Seconds/nanoseconds are
    /// ignored with Absent.
    let encodeClientRepresentationWith
        (version: uint16) (vendorId: uint16) (clientKey: byte[]) (sessionId: byte) (requestedMtu: uint16)
        (timestamps: RepresentationTimestamps) (extensibility: XrceUnannotatedTypes)
        (seconds: int) (nanoseconds: uint32)
        : byte[] =
        let le32 (v: uint32) = [| byte v; byte (v >>> 8); byte (v >>> 16); byte (v >>> 24) |]
        let time =
            match timestamps with
            | RepresentationTimestamps.Absent -> [||]
            | RepresentationTimestamps.Present ->
                Array.concat [
                    (match extensibility with
                     | XrceUnannotatedTypes.Appendable -> le32 8u // DHEADER of Time_t
                     | XrceUnannotatedTypes.Final -> [||])
                    le32 (uint32 seconds)
                    le32 nanoseconds
                ]
        Array.concat [
            Encoding.ASCII.GetBytes "XRCE"
            [| byte (version >>> 8); byte version |]
            [| byte (vendorId >>> 8); byte vendorId |]
            time
            clientKey
            [| sessionId; 0x00uy |] // session_id, then properties-absent
            [| byte requestedMtu; byte (requestedMtu >>> 8) |]
        ]

    /// Same with representation timestamps Absent (bytes unchanged).
    let encodeClientRepresentation
        (version: uint16) (vendorId: uint16) (clientKey: byte[]) (sessionId: byte) (requestedMtu: uint16)
        : byte[] =
        encodeClientRepresentationWith version vendorId clientKey sessionId requestedMtu
            RepresentationTimestamps.Absent XrceUnannotatedTypes.Appendable 0 0u

    /// CREATE_CLIENT (§7.8.2/§8.3.5.1): no session exists yet, so the
    /// MessageHeader session_id is SESSION_ID_NONE_WITH_CLIENT_KEY (0x00)
    /// when the requested `sessionId` is 0..127 (header carries the
    /// ClientKey) and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) when it is
    /// 128..255 (header without ClientKey), DDS-XRCE 1.0 §8.3.2.1. The
    /// requested `sessionId` itself travels only in the CLIENT_Representation
    /// payload. stream_id=0x00 STREAM_ID_NONE; sequence_nr=0; one
    /// CREATE_CLIENT submessage (flags=0x01, E-flag only) whose body is a
    /// real encodeClientRepresentation, not an ad hoc placeholder.
    ///
    /// `createClientWith` takes the representation timestamps setting, the
    /// extensibility of Time_t and explicit timestamp values.
    let createClientWith
        (sessionId: byte) (clientKey: byte[])
        (timestamps: RepresentationTimestamps) (extensibility: XrceUnannotatedTypes)
        (seconds: int) (nanoseconds: uint32)
        : byte[] =
        let h =
            if sessionId < 128uy then header 0x00uy StreamIdNone 0us clientKey
            else [| 0x80uy; StreamIdNone; 0uy; 0uy |]
        let body =
            encodeClientRepresentationWith 0x0100us 0x0000us clientKey sessionId DefaultMtu
                timestamps extensibility seconds nanoseconds
        Array.concat [ h; smHeader smCreateClient flagE body.Length; body ]

    /// CREATE_CLIENT with representation timestamps Absent (bytes unchanged).
    let createClient (sessionId: byte) (clientKey: byte[]) : byte[] =
        createClientWith sessionId clientKey RepresentationTimestamps.Absent XrceUnannotatedTypes.Appendable 0 0u

    /// CREATE_CLIENT for a client's session id and key in the client's
    /// representation timestamps setting; with Present the timestamp is the
    /// wall clock at the time of the call.
    let createClientFor (c: XrceClient) : byte[] =
        let ms = System.DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()
        createClientWith c.SessionId c.ClientKey c.RepresentationTimestamps c.UnannotatedTypes
            (int (ms / 1000L)) (uint32 ((ms % 1000L) * 1000000L))

    /// CREATE(REPRESENTATION_BY_REFERENCE) on an open session:
    /// MessageHeader WITH the ClientKey + one CREATE submessage (§7.7.8
    /// BaseObjectRequest + the real, two-level §7.7.3 ObjectVariant: the
    /// outer `objectKind` byte, then the inner REPRESENTATION_BY_REFERENCE
    /// payload, then that kind's own trailing field -- `parentObjectId`
    /// for a Topic/Publisher/Subscriber/DataWriter/DataReader, `None` for
    /// every other kind -- byte-oracled in crates/xrce/src/object_repr.rs
    /// against real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic
    /// (Topic's own participant_id: commit 31c62df2a63). `reference` is a
    /// NUL-terminated object_reference string (§7.7.3.1.1).
    let create
        (sessionId: byte) (clientKey: byte[]) (seq: uint16) (requestId: uint16) (objectId: uint16)
        (objectKind: byte) (reference: string) (parentObjectId: uint16 option) (reuse: bool) (replace: bool)
        : byte[] =
        let refBytes = Encoding.UTF8.GetBytes reference
        let lenInclNul = refBytes.Length + 1
        let parentBytes =
            match parentObjectId with
            | Some p -> objectIdBytes p
            | None -> [||]
        let variant =
            Array.concat [
                [| objectKind |] // outer ObjectKind byte (§7.7.3)
                [| 0x01uy; 0x00uy; 0x00uy |] // inner disc=BY_REFERENCE + 2 padding
                [| byte lenInclNul; byte (lenInclNul >>> 8); byte (lenInclNul >>> 16); byte (lenInclNul >>> 24) |]
                refBytes
                [| 0x00uy |] // NUL terminator
                parentBytes
            ]
        let body = Array.concat [ requestIdBytes requestId; objectIdBytes objectId; variant ]
        let flags =
            flagE
            ||| (if reuse then flagReuse else 0uy)
            ||| (if replace then flagReplace else 0uy)
        Array.concat [ header sessionId StreamIdBuiltinReliable seq clientKey; smHeader smCreate flags body.Length; body ]

    /// DELETE (§7.8.3/§8.3.5.4): MessageHeader WITH the ClientKey + one
    /// DELETE submessage (§7.7.8 BaseObjectRequest only).
    let delete (sessionId: byte) (clientKey: byte[]) (seq: uint16) (requestId: uint16) (objectId: uint16) : byte[] =
        let body = Array.concat [ requestIdBytes requestId; objectIdBytes objectId ]
        Array.concat [ header sessionId StreamIdBuiltinReliable seq clientKey; smHeader smDelete flagE body.Length; body ]

    /// WRITE_DATA (§7.8.4/§8.3.5.8, FORMAT_DATA): MessageHeader WITH the
    /// ClientKey + one WRITE_DATA submessage (§7.7.8 BaseObjectRequest +
    /// `sample` verbatim, no length of its own -- DDS-XRCE 1.0 Annex
    /// B.3.1).
    let writeData
        (sessionId: byte) (clientKey: byte[]) (seq: uint16) (requestId: uint16) (writerObjectId: uint16)
        (sample: byte[])
        : byte[] =
        let body = Array.concat [ requestIdBytes requestId; objectIdBytes writerObjectId; sample ]
        Array.concat [ header sessionId StreamIdBuiltinReliable seq clientKey; smHeader smWriteData flagE body.Length; body ]

    /// READ_DATA (§7.8.5/§8.3.5.9): MessageHeader WITH the ClientKey + one
    /// READ_DATA submessage (§7.7.8 BaseObjectRequest + a minimal
    /// ReadSpecification: no content filter, no delivery control -- both
    /// optional members absent). `preferredStreamId` selects the StreamId
    /// the agent should use for the DATA reply.
    let readData
        (sessionId: byte) (clientKey: byte[]) (seq: uint16) (requestId: uint16) (readerObjectId: uint16)
        (preferredStreamId: byte)
        : byte[] =
        let body =
            Array.concat [
                requestIdBytes requestId
                objectIdBytes readerObjectId
                [| preferredStreamId; 0x00uy; 0x00uy; 0x00uy |]
            ]
        Array.concat [ header sessionId StreamIdBuiltinReliable seq clientKey; smHeader smReadData flagE body.Length; body ]
