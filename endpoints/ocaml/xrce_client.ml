(* SPDX-License-Identifier: Apache-2.0 *)
(* Copyright 2026 ZeroDDS Contributors *)
(*
   DDS-XRCE 1.0 client, ported from `crates/xrce-client`. Two layers:

     1. Pure wire encoders (no state, no I/O) for MessageHeader, Submessage-
        Header, ObjectId, BaseObjectRequest, ObjectVariant::ByReference, and
        the five submessage bodies this client speaks
        (CREATE_CLIENT/CREATE/DELETE/WRITE_DATA/READ_DATA).
     2. A synchronous state machine (`create`/`connect`/`mark_connected`/
        `create_object`/`delete_object`/`request_write`/`request_read`/
        `disconnect`) mirroring `crates/xrce-client::XrceClient` 1:1 in its
        lifecycle rules (Disconnected -> Connecting -> Connected, monotonic
        request ids, `disconnect` resetting only `state`).

   This reuses the SDK's existing `Zerodds.transport` type (`{ deliver : bytes
   -> unit; receive : unit -> bytes option }`) as-is. It is additive:
   `Zerodds.Client`/`Zerodds.AsyncReader` speak an older, in-house,
   non-conformant frame on purpose; this module is the conformant DDS-XRCE
   1.0 sibling that can talk to a real agent.

   One difference from the Rust reference is worth flagging up front: in
   `crates/xrce-client`, `create_object`/`delete_object`/`request_write`/
   `request_read` take unused parameters and never touch the wire — they are
   pure request-id bookkeeping, with encoding left to a caller that does not
   exist yet. This port closes that gap: every op here actually encodes its
   submessage with the Layer 1 encoders below and calls `transport.deliver`,
   because "reach a real XRCE agent over the network" is the entire point of
   the port. The state-machine *rules* (refusals, transitions, id
   allocation, what `disconnect` resets) are unchanged.

   Idiom: every fallible operation returns `(_, client_error) result`, not an
   exception — the direct OCaml counterpart of Rust's `Result<_,
   ClientError>`, and the same idiom `crates/xrce-client` itself uses. *)

(* --- ObjectId (spec Sec 7.2.1) ---------------------------------------------- *)

(* 4-bit OBJK_* codes (spec Sec 7.2 Table 4). Values are the wire codes
   themselves, not sequential — 0x07..0x09 have no spec meaning and are
   deliberately absent so no case can round-trip an invalid kind. *)
type object_kind =
  | Participant
  | Topic
  | Publisher
  | Subscriber
  | Datawriter
  | Datareader
  | Type
  | Qosprofile
  | Application
  | Agent
  | Client
  | Domain

let object_kind_code = function
  | Participant -> 1
  | Topic -> 2
  | Publisher -> 3
  | Subscriber -> 4
  | Datawriter -> 5
  | Datareader -> 6
  | Type -> 10
  | Qosprofile -> 11
  | Application -> 12
  | Agent -> 13
  | Client -> 14
  | Domain -> 15

(* The packed 16-bit word: `raw_id` (12 bits) in the high bits, `kind` (4
   bits) in the low bits. Spec Sec 7.2.1. *)
type object_id = int

(* `word = (raw_id lsl 4) lor kind`. *)
let pack_object_id raw_id kind =
  if raw_id < 0 || raw_id > 0x0fff then
    invalid_arg "Xrce_client.pack_object_id: raw_id does not fit 12 bits";
  (raw_id lsl 4) lor object_kind_code kind

(* Constructs directly from an already-packed 16-bit word (e.g. a value read
   off the wire, or a well-known singleton like OBJECTID_AGENT). *)
let object_id_from_raw (word : int) : object_id = word

(* Wire encoding: big-endian — the one field in this spec that is *not*
   little-endian (everything else in a DDS-XRCE message is LE). *)
let object_id_to_bytes (id : object_id) =
  let b = Bytes.create 2 in
  Bytes.set b 0 (Char.chr ((id lsr 8) land 0xff));
  Bytes.set b 1 (Char.chr (id land 0xff));
  b

(* --- ObjectVariant::ByReference (spec Sec 7.7.2) ---------------------------- *)

(* Reference-by-name string — the only representation this port encodes
   (ByXmlString/InBinary are out of scope, same as `crates/xrce-client`). *)
type object_variant = By_reference of string

let byte n = Bytes.make 1 (Char.chr (n land 0xff))

let u16_le v =
  let b = Bytes.create 2 in
  Bytes.set b 0 (Char.chr (v land 0xff));
  Bytes.set b 1 (Char.chr ((v lsr 8) land 0xff));
  b

let u32_le v =
  let b = Bytes.create 4 in
  Bytes.set b 0 (Char.chr (v land 0xff));
  Bytes.set b 1 (Char.chr ((v lsr 8) land 0xff));
  Bytes.set b 2 (Char.chr ((v lsr 16) land 0xff));
  Bytes.set b 3 (Char.chr ((v lsr 24) land 0xff));
  b

(* `discriminator(1)=0x01 ++ `pad_len` padding bytes ++ length(4,
   LE)=strlen+1 ++ name bytes ++ NUL`. The length includes the NUL
   terminator XCDR2 strings carry. `pad_len` is 3 for the
   never-used-on-the-wire single-level form (this structure alone starts
   4-aligned) and 2 for `encode_object_variant_for_kind` (a 1-byte
   ObjectKind precedes it there). *)
let encode_variant_inner pad_len = function
  | By_reference name ->
    let name_bytes = Bytes.of_string name in
    let n = Bytes.length name_bytes + 1 in
    Bytes.concat Bytes.empty
      [ byte 0x01; Bytes.make pad_len '\000'; u32_le n; name_bytes; byte 0 ]

let encode_variant variant = encode_variant_inner 3 variant

(* The real wire ObjectVariant (spec Sec 7.7.3): a **two-level**
   discriminated union — the outer ObjectKind byte (`object_id`'s own low
   4 bits — the outer discriminator and the object being created always
   agree, by construction), then the inner representation
   (`encode_variant_inner`), then that kind's own trailing field:
   `parent_object_id` (an ObjectId, unaligned) for a
   Topic/Publisher/Subscriber/Datawriter/Datareader, `None` for every
   other kind — byte-oracled in `crates/xrce/src/object_repr.rs` against
   real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic (Topic's own
   participant_id: commit 31c62df2a63). This — not `encode_variant`
   alone — is the only form a real agent accepts. *)
let encode_object_variant_for_kind object_id variant ?parent_object_id () =
  let kind_byte = byte (object_id land 0x0f) in
  let inner = encode_variant_inner 2 variant in
  let parent =
    match parent_object_id with Some p -> object_id_to_bytes p | None -> Bytes.empty
  in
  Bytes.concat Bytes.empty [ kind_byte; inner; parent ]

(* --- MessageHeader (spec Sec 8.3.2) ------------------------------------------ *)

(* Length of the ClientKey field, spec Sec 7.7. *)
let client_key_len = 4

(* `session_id <= 0x7F` carries a client_key (8-byte header); `>= 0x80` does
   not (4-byte header). Reserved: SESSION_NONE_WITH_KEY = 0x00,
   SESSION_NONE_WITHOUT_KEY = 0x80. *)
let session_carries_key_max = 0x7f

(* `session_id(1) stream_id(1) sequence_nr(2, LE) [client_key(4)]`. Every
   operation in this port uses a session_id <= 0x7F, so the key is always
   present — callers never construct a keyless header here. *)
let encode_message_header session_id stream_id sequence_nr client_key =
  let hdr = Bytes.concat Bytes.empty [ byte session_id; byte stream_id; u16_le sequence_nr ] in
  if session_id <= session_carries_key_max then Bytes.cat hdr client_key else hdr

(* --- SubmessageHeader (spec Sec 8.3.4) --------------------------------------- *)

(* FLAG_E_LITTLE_ENDIAN, bit 0. Every submessage this client sends sets it —
   the whole port only ever writes little-endian bodies. *)
let flag_e_little_endian = 0x01

(* `submessage_id(1) flags(1) submessage_length(2, always LE)`. The length
   field is always little-endian regardless of the body's own endianness
   flag. *)
let encode_submessage_header id flags body_len =
  Bytes.concat Bytes.empty [ byte id; byte flags; u16_le body_len ]

(* Submessage ids used by this client (spec Sec 8.3.4 Table 8). *)
let sm_create_client = 0x00
let sm_create = 0x01
let sm_delete = 0x03
let sm_write_data = 0x07
let sm_read_data = 0x08

(* --- BaseObjectRequest (spec Sec 7.7.8) -------------------------------------- *)

(* The wire RequestId is two raw, caller-supplied octets — not a
   little-endian encoding of an integer. `crates/xrce`'s own
   `BaseObjectRequest::encode` copies them through unchanged. *)
type request_id_bytes = int * int

(* `request_id(2 raw bytes) ++ object_id.to_bytes()(2, big-endian)`. *)
let encode_base_object_request (req0, req1) object_id =
  Bytes.concat Bytes.empty [ byte req0; byte req1; object_id_to_bytes object_id ]

(* --- Full message encoders ---------------------------------------------------- *)
(*
   A message is `header ++ submessage_header ++ body`, concatenated directly:
   this client only ever sends one submessage per message, so the
   between-submessages alignment padding rule never applies here. *)

let encode_message session_id stream_id sequence_nr client_key submessage_id flags body =
  Bytes.concat Bytes.empty
    [
      encode_message_header session_id stream_id sequence_nr client_key;
      encode_submessage_header submessage_id flags (Bytes.length body);
      body;
    ]

(* eProsima's own hard datagram ceiling (`crates/xrce/src/handshake.rs`'s
   module doc has the byte-oracle measurement backing it), used as this
   port's own CREATE_CLIENT requested_mtu. *)
let default_mtu = 508

(* `CLIENT_Representation` (Annex A, spec Sec 7.8.2.1): `xrce_cookie(4) ++
   xrce_version(2) ++ xrce_vendor_id(2) ++ client_key(4) ++ session_id(1)
   ++ a 0 properties-absent octet ++ requested_mtu(2, LE)` — eProsima's
   own non-normative MTU extension, always sent by this client. No
   PropertySeq support here — this client sends none. *)
(* Form of the `CLIENT_Representation` (DDS-XRCE 1.0 Sec 7.8.2.1). The prose
   lists `Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A
   and the bytes of Annex B.1 do not have it. The bytes do not tell the forms
   apart, so it is a setting; the agent has to be set the same way
   (`representation_timestamps:` in its configuration). `Absent` (default):
   the Annex A form. `Present`: the prose form. *)
type representation_timestamps = Absent | Present

(* `?representation_timestamps:Present` adds the `Time_t` of the client
   (`?timestamp:(seconds, nanoseconds)`, default: the wall clock at the time
   of the call; seconds i32, nanoseconds u32, little-endian) after
   `xrce_vendor_id`. `Time_t` has no `@extensibility`, so it is APPENDABLE
   (DDS-XTypes 1.3 7.3.1.2.1.8): a DHEADER u32 = 8 in front (28 octets in
   all); `?unannotated_types:Reliable.Final` leaves it out (24 octets). *)
let encode_client_representation ~version ~vendor_id ~client_key ~session_id
    ?(requested_mtu = default_mtu) ?(representation_timestamps = Absent) ?timestamp
    ?(unannotated_types = Reliable.Appendable) () =
  let time =
    match representation_timestamps with
    | Absent -> Bytes.empty
    | Present ->
      let sec, nsec =
        match timestamp with
        | Some t -> t
        | None ->
          let now = Unix.gettimeofday () in
          let sec = Float.to_int now in
          (sec, Float.to_int ((now -. Float.of_int sec) *. 1e9))
      in
      let body = Bytes.concat Bytes.empty [ u32_le sec; u32_le nsec ] in
      (match unannotated_types with
       | Reliable.Appendable -> Bytes.cat (u32_le 8) body
       | Reliable.Final -> body)
  in
  Bytes.concat Bytes.empty
    [
      Bytes.of_string "XRCE";
      version;
      vendor_id;
      time;
      client_key;
      byte session_id;
      byte 0;
      u16_le requested_mtu;
    ]

(* Spec Sec 7.8.2. `session_id` is the id this client asks for (it belongs
   in the `CLIENT_Representation`). The header carries
   SESSION_ID_NONE_WITH_CLIENT_KEY (0x00, with the key) when it is 0..127
   and SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80, no key) when it is
   128..255 (DDS-XRCE 1.0 Sec 8.3.2.1), stream=NONE(0x00),
   sequence_nr=0 — the handshake precedes any session. `representation` is the XCDR2-encoded
   `CLIENT_Representation` — build it with `encode_client_representation`
   — passed through byte for byte here. *)
let encode_create_client_message session_id client_key representation =
  let header_session_id = if session_id < 128 then 0x00 else 0x80 in
  encode_message header_session_id 0x00 0 client_key sm_create_client flag_e_little_endian representation

(* Spec Sec 7.8.3. Flags = E | (reuse ? 0x02 : 0) | (replace ? 0x04 : 0). Body
   = BaseObjectRequest ++ the real, two-level ObjectVariant
   (`encode_object_variant_for_kind`) — the only representation this port
   encodes, same as `crates/xrce-client`. *)
let encode_create_message session_id stream_id sequence_nr client_key request_id object_id name
    ?parent_object_id ?(reuse = false) ?(replace = false) () =
  let flags = flag_e_little_endian lor (if reuse then 0x02 else 0) lor (if replace then 0x04 else 0) in
  let body =
    Bytes.concat Bytes.empty
      [
        encode_base_object_request request_id object_id;
        encode_object_variant_for_kind object_id (By_reference name) ?parent_object_id ();
      ]
  in
  encode_message session_id stream_id sequence_nr client_key sm_create flags body

(* Spec Sec 7.8.3. Body = BaseObjectRequest only — no ObjectVariant. *)
let encode_delete_message session_id stream_id sequence_nr client_key request_id object_id =
  let body = encode_base_object_request request_id object_id in
  encode_message session_id stream_id sequence_nr client_key sm_delete flag_e_little_endian body

(* Spec Sec 7.8.4. Body = BaseObjectRequest ++ serialized_data, verbatim and
   with no length prefix of its own — `submessage_length` is what bounds the
   sample. *)
let encode_write_data_message session_id stream_id sequence_nr client_key request_id object_id
    serialized_data =
  let body =
    Bytes.concat Bytes.empty [ encode_base_object_request request_id object_id; serialized_data ]
  in
  encode_message session_id stream_id sequence_nr client_key sm_write_data flag_e_little_endian body

(* Spec Sec 7.8.5. Body = BaseObjectRequest ++ ReadSpecification. This port
   never sets a content filter or delivery control, so ReadSpecification is
   always `preferred_stream_id, data_format=0x00, content_filter_present=0x00,
   delivery_control_present=0x00`. *)
let encode_read_data_message session_id stream_id sequence_nr client_key request_id object_id
    preferred_stream_id =
  let read_spec = Bytes.concat Bytes.empty [ byte preferred_stream_id; byte 0; byte 0; byte 0 ] in
  let body =
    Bytes.concat Bytes.empty [ encode_base_object_request request_id object_id; read_spec ]
  in
  encode_message session_id stream_id sequence_nr client_key sm_read_data flag_e_little_endian body

(* --- Layer 2: the state machine (mirrors crates/xrce-client) --------------- *)

(* Spec Sec 7.2 lifecycle. *)
type client_state =
  | Disconnected (* Initial: the client knows its ClientKey but is not connected. *)
  | Connecting (* CREATE_CLIENT sent, STATUS_AGENT not yet received. *)
  | Connected (* Connected — operations are possible. *)

(* Mirrors `crates/xrce-client::ClientError`. Only `Invalid_state` is
   reachable from this port: `Zerodds.transport.deliver` has no failure
   channel at all, and the Rust reference's `QueueFull` is reserved/unused
   even there (its `out_queue` has no capacity cap), so neither
   `Transport`/`QueueFull` has a counterpart here. *)
type client_error = Invalid_state

type client = {
  client_key : bytes;
  mutable state : client_state;
  transport : Zerodds.transport;
  mutable next_request_id : int;
  (* Defaults match the fixtures: session_id=0x01 (carries client_key),
     stream_id=0x80 (builtin-reliable — every post-handshake operation is
     reliable). *)
  mutable session_id : int;
  mutable stream_id : int;
  (* next_seq starts at 0 — the CREATE_CLIENT handshake always sends
     sequence_nr=0 on stream_id=NONE and does not consume this counter; the
     first reliable-stream operation gets sequence_nr 0. *)
  mutable next_seq : int;
  (* Form of the CLIENT_Representation, see `representation_timestamps`. *)
  mutable representation_timestamps : representation_timestamps;
}

(* Constructor. `state = Disconnected`; `next_request_id` starts at 1. *)
let create ?(representation_timestamps = Absent) client_key transport =
  if Bytes.length client_key <> client_key_len then
    invalid_arg "Xrce_client.create: client_key must be 4 bytes";
  {
    client_key;
    state = Disconnected;
    transport;
    next_request_id = 1;
    session_id = 0x01;
    stream_id = 0x80;
    next_seq = 0;
    representation_timestamps;
  }

(* The id for this call, then advance — wraps at 16 bits and skips 0 (mirrors
   Rust's `next_request_id.wrapping_add(1).max(1)`). *)
let alloc_request_id c =
  let req = c.next_request_id in
  let next = (c.next_request_id + 1) land 0xffff in
  c.next_request_id <- (if next = 0 then 1 else next);
  req

(* Big-endian encoding of the auto-allocated request id — matching
   `object_id_to_bytes` (also big-endian) so both halves of a
   BaseObjectRequest use the same byte order (id 1 -> `(0x00, 0x01)`,
   matching the same choice the other ports made). *)
let request_id_bytes id : request_id_bytes = ((id lsr 8) land 0xff, id land 0xff)

let set_representation_timestamps c form = c.representation_timestamps <- form

(* The CLIENT_Representation of this client in its `representation_timestamps`
   form; with `Present` the timestamp is the wall clock at the time of the call. *)
let client_representation c ~version ~vendor_id ?requested_mtu () =
  encode_client_representation ~version ~vendor_id ~client_key:c.client_key
    ~session_id:c.session_id ?requested_mtu
    ~representation_timestamps:c.representation_timestamps ()

(* Initiates the handshake: encodes and delivers CREATE_CLIENT, then
   transitions to `Connecting`.

   Returns `Error Invalid_state` unless `state = Disconnected`. *)
let connect c representation =
  match c.state with
  | Disconnected ->
    let msg = encode_create_client_message c.session_id c.client_key representation in
    c.transport.deliver msg;
    c.state <- Connecting;
    Ok ()
  | Connecting | Connected -> Error Invalid_state

(* `connect` with the representation built by `client_representation`. *)
let connect_default c ~version ~vendor_id ?requested_mtu () =
  connect c (client_representation c ~version ~vendor_id ?requested_mtu ())

(* Marks the handshake complete (the caller has received STATUS_AGENT
   externally). Pure bookkeeping — no wire I/O, matching the Rust reference
   exactly.

   Returns `Error Invalid_state` unless `state = Connecting`. *)
let mark_connected c =
  match c.state with
  | Connecting ->
    c.state <- Connected;
    Ok ()
  | Disconnected | Connected -> Error Invalid_state

(* Submits a CREATE operation for an `ObjectVariant::ByReference(name)`.
   Returns `Ok request_id`.

   Returns `Error Invalid_state` unless `state = Connected`. *)
let create_object c object_id name ?parent_object_id ?(reuse = false) ?(replace = false) () =
  match c.state with
  | Connected ->
    let req = alloc_request_id c in
    let msg =
      encode_create_message c.session_id c.stream_id c.next_seq c.client_key (request_id_bytes req)
        object_id name ?parent_object_id ~reuse ~replace ()
    in
    c.transport.deliver msg;
    c.next_seq <- (c.next_seq + 1) land 0xffff;
    Ok req
  | Disconnected | Connecting -> Error Invalid_state

(* Submits a DELETE operation. Returns `Ok request_id`.

   Returns `Error Invalid_state` unless connected. *)
let delete_object c object_id =
  match c.state with
  | Connected ->
    let req = alloc_request_id c in
    let msg =
      encode_delete_message c.session_id c.stream_id c.next_seq c.client_key (request_id_bytes req)
        object_id
    in
    c.transport.deliver msg;
    c.next_seq <- (c.next_seq + 1) land 0xffff;
    Ok req
  | Disconnected | Connecting -> Error Invalid_state

(* Submits a WRITE_DATA operation. Returns `Ok request_id`.

   Returns `Error Invalid_state` unless connected. *)
let request_write c writer_object_id payload =
  match c.state with
  | Connected ->
    let req = alloc_request_id c in
    let msg =
      encode_write_data_message c.session_id c.stream_id c.next_seq c.client_key
        (request_id_bytes req) writer_object_id payload
    in
    c.transport.deliver msg;
    c.next_seq <- (c.next_seq + 1) land 0xffff;
    Ok req
  | Disconnected | Connecting -> Error Invalid_state

(* Submits a READ_DATA operation (pull-based). Returns `Ok request_id`.
   `preferred_stream_id` is the client's own `stream_id`, `data_format` is
   always Data (0x00).

   Returns `Error Invalid_state` unless connected. *)
let request_read c reader_object_id =
  match c.state with
  | Connected ->
    let req = alloc_request_id c in
    let msg =
      encode_read_data_message c.session_id c.stream_id c.next_seq c.client_key
        (request_id_bytes req) reader_object_id c.stream_id
    in
    c.transport.deliver msg;
    c.next_seq <- (c.next_seq + 1) land 0xffff;
    Ok req
  | Disconnected | Connecting -> Error Invalid_state

(* Closes the session — goes back to `Disconnected`. That is all: does NOT
   reset `next_request_id`, `session_id`, `stream_id`, or `next_seq` —
   `crates/xrce-client::disconnect` only resets `state`. Matching this
   exactly matters, per the reference's own comment: a "helpful" reset of
   more than `state` is a divergence, not an improvement. Infallible, like
   the Rust reference — no `result` here. *)
let disconnect c = c.state <- Disconnected
