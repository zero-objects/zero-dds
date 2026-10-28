-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 XRCE session client (ADR 0013). Strict Ada 83
--  (-gnat83): no modular types, no Interfaces, no tagged types, no
--  access-to-subprogram, no child units, no Ada.Containers,
--  no `raise ... with "text"`. Follows Zerodds_Ada83_Wire and
--  Zerodds_Ada83_Endpoint's own solutions for each: unsigned arithmetic on
--  Long_Integer via `+`/`*`/`mod` (never Shift_Left/Interfaces), a flat
--  package name standing in for a child unit, `Ok : out Boolean` in place
--  of an exception for an expected-false control path (the same shape
--  Zerodds_Ada83_Reliable.Submit/Recv_Data already answer with).
--
--  Two halves, kept apart on purpose (the same split `crates/xrce-client`
--  and `crates/xrce` keep):
--
--   1. The STATE MACHINE (Client + the procedures below) mirrors
--      `crates/xrce-client/src/lib.rs::XrceClient` field for field and
--      operation for operation: Disconnected -> Connecting -> Connected,
--      the same Next_Request_Id counter (wraps 1..65535, never 0). Every
--      operation below cites the OMG DDS-XRCE 1.0 clause its Rust
--      counterpart cites.
--   2. The FRAMING (Frame_* functions) builds the actual octets, in the
--      same manual-byte-layout style Zerodds_Ada83_Endpoint.Xrce_Write_Frame
--      already uses.
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
--  choice, <= 127).
--  Fixtures: endpoints/testdata/xrce-client/ (*.bin).
--
--  All multi-byte header/submessage fields are little-endian (spec
--  8.3.2.3/8.3.4); ObjectId is big-endian on the wire (spec Annex B
--  examples). request_id (2 octets, section 7.7.8) is carried here as a
--  Long_Integer (Ada 83 has no unsigned 16-bit type) and encoded
--  big-endian -- crates/xrce-client::request_id_octets fixes this choice
--  for every port of this client (the wire itself treats request_id as two
--  opaque octets with no byte order of its own).

with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Reliable;

package Zerodds_Ada83_Xrce_Client is

   --  --- lifecycle state (mirrors ClientState) ---
   type Client_State is (Disconnected, Connecting, Connected);

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
   --                      Zerodds_Ada83_Reliable.Final there is no DHEADER
   --                      (8 octets).
   --  The agent has to be set the same way (`representation_timestamps:
   --  absent | present` in its configuration).
   type Representation_Timestamps is (Absent, Present);

   --  A 2-byte field (xrce_version or xrce_vendor_id in
   --  CLIENT_Representation, Annex A).
   type Two_Byte_Field is array (0 .. 1) of Byte;

   --  ObjectKind codes actually exercised by this client (DDS-XRCE 1.0
   --  Table 4). A full kind table belongs to a codegen layer, not this
   --  hand-written session client.
   Objk_Participant : constant Byte := 1;
   Objk_Topic       : constant Byte := 2;
   Objk_Publisher   : constant Byte := 3;
   Objk_Subscriber  : constant Byte := 4;
   Objk_Datawriter  : constant Byte := 5;
   Objk_Datareader  : constant Byte := 6;

   --  Packs a 12-bit Raw_Id + 4-bit ObjectKind into the 16-bit wire
   --  ObjectId. `*16` stands in for the 4-bit shift (RM83 has no
   --  Shift_Left): Raw_Id must be 0 .. 4095 for the result to fit 16 bits,
   --  the same bound object_id.rs::ObjectId::new enforces.
   function Pack_Object_Id (Raw_Id : Long_Integer; Kind : Byte) return Long_Integer;

   --  --- state machine (mirrors crates/xrce-client::XrceClient) ---

   type Client is record
      Client_Key      : Client_Key_Type := (others => 0);
      Status          : Client_State := Disconnected;
      Session_Id      : Byte := 0;         --  proposed at Connect, spec-adopted
                                            --  verbatim by the agent
                                            --  (section 7.8.2.1) -- valid once
                                            --  Mark_Connected has been called
      Next_Request_Id : Long_Integer := 1; --  1 .. 65535, wraps skipping 0
      Timestamps      : Representation_Timestamps := Absent;
      Timestamps_Mode : Zerodds_Ada83_Reliable.Unannotated_Types :=
        Zerodds_Ada83_Reliable.Appendable;
   end record;

   --  Sets the representation timestamps option used by Frame_Connect.
   --  Mode selects the Time_t form (Appendable: DHEADER, Final: none) and
   --  only matters when Value is Present.
   procedure Set_Representation_Timestamps
     (C     : in out Client;
      Value : Representation_Timestamps;
      Mode  : Zerodds_Ada83_Reliable.Unannotated_Types :=
        Zerodds_Ada83_Reliable.Appendable);

   --  Constructor. Section 7.8.2 CREATE_CLIENT (via Connect below); this
   --  call only initialises local state -- Disconnected, Next_Request_Id=1.
   procedure Init (C : out Client; Key : Client_Key_Type);

   --  Initiates the handshake: proposes Requested_Session_Id (1..127, spec
   --  section 8.3.2.1 -- a ClientKey-carrying session) and moves
   --  Disconnected -> Connecting. Section 8.4.5 (Handshake). Ok=False when
   --  not Disconnected. This call does not touch the wire -- see
   --  Frame_Create_Client for the CREATE_CLIENT octets.
   procedure Connect
     (C : in out Client; Requested_Session_Id : Byte; Ok : out Boolean);

   --  Marks the handshake complete -- the caller has received STATUS_AGENT
   --  (recognise it by submessage_id = 4, spec section 8.3.5.5; this
   --  client does not decode AGENT_Representation, matching
   --  crates/xrce-client's own scope). Section 8.4.5. Ok=False when not
   --  Connecting.
   procedure Mark_Connected (C : in out Client; Ok : out Boolean);

   --  Submits a CREATE operation: gates on Connected and returns the next
   --  request_id. Section 7.8.3 CREATE. Ok=False (Request_Id undefined)
   --  when not Connected.
   procedure Create_Object
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean);

   --  Submits a DELETE operation. Section 7.8.3 DELETE.
   procedure Delete_Object
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean);

   --  Submits a WRITE_DATA operation. Section 7.8.4 WRITE_DATA.
   procedure Request_Write
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean);

   --  Submits a READ_DATA operation (pull-based). Section 7.8.5 READ_DATA.
   procedure Request_Read
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean);

   --  Closes the session -- back to Disconnected. Section 8.4.5. Local
   --  state only, exactly like crates/xrce-client::disconnect: no
   --  DELETE_CLIENT is sent.
   procedure Disconnect (C : in out Client);

   --  --- framing (the octets a real transport sends) ---

   Max_Buffer : constant := 128;
   type Frame is record
      Data : Byte_Array (0 .. Max_Buffer - 1);
      Len  : Integer := 0;
   end record;

   --  eProsima's own hard datagram ceiling, used as this SDK's own
   --  CREATE_CLIENT requested_mtu.
   Default_Mtu : constant Long_Integer := 508;

   --  CLIENT_Representation (Annex A, section 7.8.2.1): xrce_cookie(4) +
   --  xrce_version(2) + xrce_vendor_id(2) + client_key(4) + session_id(1)
   --  + a 0 properties-absent octet + requested_mtu(2, LE) -- eProsima's
   --  own non-normative MTU extension, always sent by this client. No
   --  PropertySeq support here -- this client sends none.
   function Encode_Client_Representation
     (Version, Vendor_Id : Two_Byte_Field;
      Key                : Client_Key_Type;
      Session_Id         : Byte;
      Requested_Mtu      : Long_Integer := Default_Mtu;
      Timestamps         : Representation_Timestamps := Absent;
      Seconds            : Long_Integer := 0;
      Nanoseconds        : Long_Integer := 0;
      Mode               : Zerodds_Ada83_Reliable.Unannotated_Types :=
        Zerodds_Ada83_Reliable.Appendable) return Frame;
   --  Timestamps = Absent (default) is the 16-octet Annex A form; Seconds,
   --  Nanoseconds and Mode are then ignored. Timestamps = Present inserts
   --  Time_t after xrce_vendor_id (DDS-XRCE 1.0 section 7.8.2.1 prose):
   --  Appendable: DHEADER (u32 = 8), seconds (i32, -2**31 .. 2**31-1),
   --  nanoseconds (u32, 0 .. 2**32-1) -- 28 octets in all; Final: seconds,
   --  nanoseconds -- 24 octets. key, session_id, the properties octet and
   --  requested_mtu follow, shifted by 12 or 8.

   --  The CREATE_CLIENT frame of a connecting client: reads the wall clock
   --  at the time of the call when C.Timestamps is Present (see
   --  Set_Representation_Timestamps), encodes the CLIENT_Representation with
   --  C.Session_Id and C.Client_Key and frames it with Frame_Create_Client.
   function Frame_Connect
     (C                  : Client;
      Version, Vendor_Id : Two_Byte_Field;
      Requested_Mtu      : Long_Integer := Default_Mtu) return Frame;

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
      Representation : Frame) return Frame;

   --  CREATE(REPRESENTATION_BY_REFERENCE) on an open session: MessageHeader
   --  WITH the ClientKey (session_id <= 127, section 8.3.2.1) + one CREATE
   --  submessage (section 7.7.8 BaseObjectRequest + the real, two-level
   --  section 7.7.3 ObjectVariant: the outer Object_Kind byte, then the
   --  inner REPRESENTATION_BY_REFERENCE payload, then that kind's own
   --  trailing field -- Parent_Object_Id (when Has_Parent) for a
   --  Topic/Publisher/Subscriber/DataWriter/DataReader, absent for every
   --  other kind (Topic's own participant_id: commit 31c62df2a63).
   --  Reference is the object_reference string (spec section
   --  7.7.3.1.1). Reference'Length + 22 must not exceed Max_Buffer.
   function Frame_Create
     (Session_Id       : Byte;
      Key              : Client_Key_Type;
      Seq              : Long_Integer;
      Request_Id       : Long_Integer;
      Object_Id        : Long_Integer;
      Object_Kind      : Byte;
      Reference        : String;
      Has_Parent       : Boolean := False;
      Parent_Object_Id : Long_Integer := 0;
      Reuse, Replace   : Boolean) return Frame;

   --  DELETE on an open session: MessageHeader WITH the ClientKey + one
   --  DELETE submessage (section 7.7.8 BaseObjectRequest only).
   function Frame_Delete
     (Session_Id : Byte;
      Key        : Client_Key_Type;
      Seq        : Long_Integer;
      Request_Id : Long_Integer;
      Object_Id  : Long_Integer) return Frame;

   --  WRITE_DATA (FORMAT_DATA, section 8.3.5.8): MessageHeader WITH the
   --  ClientKey + one WRITE_DATA submessage (section 7.7.8
   --  BaseObjectRequest + the raw sample body, verbatim, no length of its
   --  own). Sample'Length + 4 must not exceed Max_Buffer.
   function Frame_Write_Data
     (Session_Id       : Byte;
      Key              : Client_Key_Type;
      Seq              : Long_Integer;
      Request_Id       : Long_Integer;
      Writer_Object_Id : Long_Integer;
      Sample           : Byte_Array) return Frame;

   --  READ_DATA (section 8.3.5.9): MessageHeader WITH the ClientKey + one
   --  READ_DATA submessage (section 7.7.8 BaseObjectRequest + a minimal
   --  ReadSpecification: no content filter, no delivery control -- both
   --  @optional members absent). Preferred_Stream_Id selects the StreamId
   --  the agent should use for the DATA reply (0 = STREAMID_NONE, "leave
   --  the decision to the Agent", spec Annex B.4.1).
   function Frame_Read_Data
     (Session_Id          : Byte;
      Key                 : Client_Key_Type;
      Seq                 : Long_Integer;
      Request_Id          : Long_Integer;
      Reader_Object_Id    : Long_Integer;
      Preferred_Stream_Id : Byte) return Frame;

end Zerodds_Ada83_Xrce_Client;
