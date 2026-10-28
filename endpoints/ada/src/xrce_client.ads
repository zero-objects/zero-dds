-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  XRCE session client for the Ada Stage 1 endpoint SDK (ADR 0013).
--
--  Two halves, kept apart on purpose (the same split `crates/xrce-client`
--  and `crates/xrce` keep):
--
--   1. The STATE MACHINE (Client + the procedures below) mirrors
--      `crates/xrce-client/src/lib.rs::XrceClient` field for field and
--      operation for operation: Disconnected -> Connecting -> Connected,
--      the same Next_Request_Id counter (wraps 1..65535, never 0, via the
--      same "+1, then max(1)" rule), the same four gated operations. Every
--      operation below cites the OMG DDS-XRCE 1.0 clause its Rust
--      counterpart cites.
--   2. The FRAMING (Frame_* functions) builds the actual octets -- the
--      Rust state machine deliberately does not ("the caller delegates
--      submessage encoding; we only track state"), but a port whose
--      purpose is "a program in this language can talk to a ZeroDDS
--      instance" has to produce real wire bytes somewhere, so this SDK
--      provides that half too, in the same manual-byte-layout style
--      Reliable's Write_Frame/Control_Frame already use.
--
--  `crates/xrce` treats the CREATE_CLIENT body's XCDR2 layout as
--  crates/xrce/src/handshake.rs::ClientRepresentation (Annex A,
--  CLIENT_Representation): xrce_cookie(4) + xrce_version(2) +
--  xrce_vendor_id(2) + client_key(4) + session_id(1) + a 0
--  properties-absent octet + eProsima's own non-normative requested_mtu
--  extension (2, little endian) -- byte-oracled there against a real
--  eProsima Micro-XRCE-DDS Client 3.0.1 capture, not a hand-picked
--  placeholder. Frame_Create_Client below builds exactly that, with the
--  MessageHeader carrying the ClientKey (Session_Id is this client's own
--  choice, <= 127 per MessageHeader::with_client_key).
--  Fixtures: endpoints/testdata/xrce-client/ (*.bin).
--
--  All multi-byte header/submessage fields are little-endian (spec
--  8.3.2.3/8.3.4); ObjectId is big-endian on the wire (spec Annex B
--  examples, object_id.rs::to_bytes). Request_Id (spec 7.7.8) is an opaque
--  octet[2] on the wire -- the wire itself has no byte order for it, only
--  crates/xrce-client's u16 counter does, and
--  crates/xrce-client::request_id_octets fixes this SDK's choice:
--  big-endian (matching ObjectId's own order and the spec's own Annex B
--  examples, e.g. "request_id = {0xAA, 0x01}").

with Interfaces;   use Interfaces;
with Deep_Reading; use Deep_Reading;
with Reliable;

package Xrce_Client is

   --  --- lifecycle state (mirrors ClientState) ---
   type Client_State is (Disconnected, Connecting, Connected);

   --  --- result codes (mirrors ClientError) ---
   type Op_Result is (Ok, Invalid_State);

   type Client_Key_Type is array (0 .. 3) of Byte;

   --  Representation timestamps setting. DDS-XRCE 1.0 contradicts itself on
   --  CLIENT_Representation: the prose of section 7.8.2.1 lists
   --  `Time_t client_timestamp` after xrce_vendor_id, the IDL of Annex A and
   --  the bytes of Annex B.1 do not have it. The two forms cannot be told
   --  apart by their bytes, so it is a named setting:
   --    Absent  (default) Annex A / B.1 form, no timestamp (16 octets).
   --    Present           the prose form of 7.8.2.1: Time_t (seconds i32,
   --                      nanoseconds u32, little endian) after
   --                      xrce_vendor_id. Time_t carries no @extensibility,
   --                      so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8):
   --                      DHEADER u32 = 8 in front, 12 octets in all. With
   --                      Reliable.Final there is no DHEADER (8 octets).
   --  The agent has to be set the same way (`representation_timestamps:
   --  absent | present` in its configuration).
   type Representation_Timestamps is (Absent, Present);

   --  A 2-byte field (xrce_version or xrce_vendor_id in
   --  CLIENT_Representation, Annex A).
   type Two_Byte_Field is array (0 .. 1) of Byte;

   --  ObjectKind codes actually exercised by this client (DDS-XRCE 1.0
   --  Table 4, crates/xrce/src/object_kind.rs). A full kind table belongs
   --  to a codegen layer, not this hand-written session client.
   Objk_Participant : constant Byte := 16#01#;
   Objk_Topic       : constant Byte := 16#02#;
   Objk_Publisher   : constant Byte := 16#03#;
   Objk_Subscriber  : constant Byte := 16#04#;
   Objk_Datawriter  : constant Byte := 16#05#;
   Objk_Datareader  : constant Byte := 16#06#;

   --  Packs a 12-bit Raw_Id + 4-bit ObjectKind into the 16-bit wire
   --  ObjectId (object_id.rs::ObjectId::new).
   function Pack_Object_Id (Raw_Id : Unsigned_16; Kind : Byte) return Unsigned_16;

   --  --- state machine (mirrors crates/xrce-client::XrceClient) ---

   type Client is record
      Client_Key      : Client_Key_Type := (others => 0);
      Status          : Client_State := Disconnected;
      Session_Id      : Byte := 0;        --  proposed at Connect, spec-adopted
                                           --  verbatim by the agent
                                           --  (section 7.8.2.1) -- valid once
                                           --  Mark_Connected has been called
      Next_Request_Id : Unsigned_16 := 1; --  1..65535, wraps skipping 0
      Timestamps      : Representation_Timestamps := Absent;
      Timestamps_Mode : Reliable.Unannotated_Types := Reliable.Appendable;
   end record;

   --  Sets the representation timestamps option used by Frame_Connect.
   --  Mode selects the Time_t form (Appendable: DHEADER, Final: none) and
   --  only matters when Value is Present.
   procedure Set_Representation_Timestamps
     (C     : in out Client;
      Value : Representation_Timestamps;
      Mode  : Reliable.Unannotated_Types := Reliable.Appendable);

   --  Constructor. Section 7.8.2 CREATE_CLIENT (via Connect below); this
   --  call only initialises local state -- Disconnected, Next_Request_Id=1.
   procedure Init (C : out Client; Key : Client_Key_Type);

   --  Initiates the handshake: proposes Requested_Session_Id (1..127, spec
   --  section 8.3.2.1 -- a ClientKey-carrying session) and moves
   --  Disconnected -> Connecting. Section 8.4.5 (Handshake). This call does
   --  not touch the wire -- see Frame_Create_Client for the CREATE_CLIENT
   --  octets.
   procedure Connect
     (C : in out Client; Requested_Session_Id : Byte; Result : out Op_Result);

   --  Marks the handshake complete -- the caller has received STATUS_AGENT
   --  (recognise it by submessage_id = 4, spec section 8.3.5.5; this client
   --  does not decode AGENT_Representation, matching crates/xrce-client's
   --  own scope). Section 8.4.5.
   procedure Mark_Connected (C : in out Client; Result : out Op_Result);

   --  Submits a CREATE operation: gates on Connected and returns the next
   --  request_id. Section 7.8.3 CREATE.
   procedure Create_Object
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result);

   --  Submits a DELETE operation. Section 7.8.3 DELETE.
   procedure Delete_Object
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result);

   --  Submits a WRITE_DATA operation. Section 7.8.4 WRITE_DATA.
   procedure Request_Write
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result);

   --  Submits a READ_DATA operation (pull-based). Section 7.8.5 READ_DATA.
   procedure Request_Read
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result);

   --  Closes the session -- back to Disconnected. Section 8.4.5. Local
   --  state only, exactly like crates/xrce-client::disconnect: no
   --  DELETE_CLIENT is sent.
   procedure Disconnect (C : in out Client);

   --  --- framing (the octets a real transport sends) ---

   --  eProsima's own hard datagram ceiling (crates/xrce/src/handshake.rs's
   --  module doc has the byte-oracle measurement backing it), used as
   --  this SDK's own CREATE_CLIENT requested_mtu.
   Default_Mtu : constant Unsigned_16 := 508;

   --  CLIENT_Representation (Annex A, section 7.8.2.1): xrce_cookie(4) +
   --  xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1)
   --  + a 0 properties-absent octet + requested_mtu(2, LE) -- eProsima's
   --  own non-normative MTU extension, always sent by this client. No
   --  PropertySeq support here -- this client sends none.
   function Encode_Client_Representation
     (Version, Vendor_Id : Two_Byte_Field;
      Key                : Client_Key_Type;
      Session_Id         : Byte;
      Requested_Mtu      : Unsigned_16 := Default_Mtu;
      Timestamps         : Representation_Timestamps := Absent;
      Seconds            : Integer_32 := 0;
      Nanoseconds        : Unsigned_32 := 0;
      Mode               : Reliable.Unannotated_Types := Reliable.Appendable)
      return Frame_Store;
   --  Timestamps = Absent (default) is the 16-octet Annex A form; Seconds,
   --  Nanoseconds and Mode are then ignored. Timestamps = Present inserts
   --  Time_t after xrce_vendor_id (DDS-XRCE 1.0 section 7.8.2.1 prose):
   --  Appendable: DHEADER (u32 = 8), seconds, nanoseconds -- 28 octets in
   --  all; Final: seconds, nanoseconds -- 24 octets. key, session_id, the
   --  properties octet and requested_mtu follow, shifted by 12 or 8.

   --  The CREATE_CLIENT frame of a connecting client: reads the wall clock
   --  at the time of the call when C.Timestamps is Present (see
   --  Set_Representation_Timestamps), encodes the CLIENT_Representation with
   --  C.Session_Id and C.Client_Key and frames it with Frame_Create_Client.
   function Frame_Connect
     (C                  : Client;
      Version, Vendor_Id : Two_Byte_Field;
      Requested_Mtu      : Unsigned_16 := Default_Mtu) return Frame_Store;

   --  CREATE_CLIENT (id=0x00): MessageHeader with session_id =
   --  SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) plus the ClientKey when the
   --  requested Session_Id is 0 .. 127, or SESSION_ID_NONE_WITHOUT_CLIENT_KEY
   --  (0x80) without ClientKey when it is 128 .. 255 (DDS-XRCE 1.0
   --  section 8.3.2.1; the requested Session_Id itself is in the
   --  Representation payload; stream_id=0x00 STREAMID_NONE,
   --  sequence_nr=0) + one
   --  CREATE_CLIENT submessage (flags=0x01, E-flag only --
   --  CreateClientPayload::into_submessage hardcodes this, no
   --  reuse/replace bits) whose body is a real Encode_Client_Representation,
   --  not an ad hoc placeholder -- byte-identical to
   --  endpoints/testdata/xrce-client/xrce_client_create_client.bin.
   function Frame_Create_Client
     (Session_Id     : Byte;
      Key            : Client_Key_Type;
      Representation : Frame_Store) return Frame_Store;

   --  CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
   --  WITH the ClientKey (session_id <= 127, section 8.3.2.1) + one CREATE
   --  submessage (section 7.7.8 BaseObjectRequest + the real, two-level
   --  section 7.7.3 ObjectVariant: the outer Object_Kind byte, then the
   --  inner REPRESENTATION_BY_REFERENCE payload, then that kind's own
   --  trailing field -- Parent_Object_Id (when Has_Parent) for a
   --  Topic/Publisher/Subscriber/DataWriter/DataReader, absent for every
   --  other kind -- byte-oracled in crates/xrce/src/object_repr.rs
   --  against real eProsima Micro-XRCE-DDS Client 3.0.1 CREATE traffic
   --  (Topic's own participant_id: commit 31c62df2a63). Reference is
   --  the object_reference string (spec section 7.7.3.1.1). Request_Id is
   --  encoded big-endian (2 bytes), matching ObjectId's own wire byte
   --  order and the spec's own Annex B examples.
   function Frame_Create
     (Session_Id        : Byte;
      Key               : Client_Key_Type;
      Seq               : Unsigned_16;
      Request_Id        : Unsigned_16;
      Object_Id         : Unsigned_16;
      Object_Kind       : Byte;
      Reference         : String;
      Has_Parent        : Boolean := False;
      Parent_Object_Id  : Unsigned_16 := 0;
      Reuse, Replace     : Boolean) return Frame_Store;

   --  DELETE on an open session: MessageHeader WITH the ClientKey + one
   --  DELETE submessage (section 7.7.8 BaseObjectRequest only).
   function Frame_Delete
     (Session_Id : Byte;
      Key        : Client_Key_Type;
      Seq        : Unsigned_16;
      Request_Id : Unsigned_16;
      Object_Id  : Unsigned_16) return Frame_Store;

   --  WRITE_DATA (FORMAT_DATA, section 8.3.5.8): MessageHeader WITH the
   --  ClientKey + one WRITE_DATA submessage (section 7.7.8
   --  BaseObjectRequest + the raw sample body, verbatim, no length of its
   --  own). Distinct from Reliable's Write_Frame, which frames the
   --  non-conformant "ZeroDDS Endpoint Profile" no-key WRITE_DATA the
   --  direct-publish path still speaks.
   function Frame_Write_Data
     (Session_Id       : Byte;
      Key              : Client_Key_Type;
      Seq              : Unsigned_16;
      Request_Id       : Unsigned_16;
      Writer_Object_Id : Unsigned_16;
      Sample           : Bytes) return Frame_Store;

   --  READ_DATA (section 8.3.5.9): MessageHeader WITH the ClientKey + one
   --  READ_DATA submessage (section 7.7.8 BaseObjectRequest + a minimal
   --  ReadSpecification: no content filter, no delivery control -- both
   --  @optional members absent). Preferred_Stream_Id selects the StreamId
   --  the agent should use for the DATA reply (0 = STREAMID_NONE, "leave
   --  the decision to the Agent", spec Annex B.4.1).
   function Frame_Read_Data
     (Session_Id           : Byte;
      Key                  : Client_Key_Type;
      Seq                  : Unsigned_16;
      Request_Id           : Unsigned_16;
      Reader_Object_Id     : Unsigned_16;
      Preferred_Stream_Id  : Byte) return Frame_Store;

end Xrce_Client;
