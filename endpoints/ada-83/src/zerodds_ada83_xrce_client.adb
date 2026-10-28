-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Calendar;

package body Zerodds_Ada83_Xrce_Client is

   Sm_Create_Client : constant Byte := 0;
   Sm_Create        : constant Byte := 1;
   Sm_Delete        : constant Byte := 3;
   Sm_Write_Data    : constant Byte := 7;
   Sm_Read_Data     : constant Byte := 8;

   Stream_None             : constant Byte := 0;
   Stream_Builtin_Reliable : constant Byte := 128;

   ------------------------------------------------------------------
   --  state machine
   ------------------------------------------------------------------

   function Pack_Object_Id (Raw_Id : Long_Integer; Kind : Byte) return Long_Integer is
   begin
      return Raw_Id * 16 + Long_Integer (Kind);
   end Pack_Object_Id;

   procedure Init (C : out Client; Key : Client_Key_Type) is
   begin
      C.Client_Key := Key;
      C.Status := Disconnected;
      C.Session_Id := 0;
      C.Next_Request_Id := 1;
   end Init;

   procedure Connect
     (C : in out Client; Requested_Session_Id : Byte; Ok : out Boolean)
   is
   begin
      if C.Status /= Disconnected then
         Ok := False;
         return;
      end if;
      C.Session_Id := Requested_Session_Id;
      C.Status := Connecting;
      Ok := True;
   end Connect;

   procedure Mark_Connected (C : in out Client; Ok : out Boolean) is
   begin
      if C.Status /= Connecting then
         Ok := False;
         return;
      end if;
      C.Status := Connected;
      Ok := True;
   end Mark_Connected;

   --  Next request_id: 1 .. 65535, wrapping without ever landing on 0 --
   --  the same rule the Rust state machine's
   --  `wrapping_add(1).max(1)` applies. No modular type (RM83): a plain
   --  conditional in place of a wraparound `mod`.
   --
   --  A procedure and not a function returning the value: RM83 6.5
   --  restricts a function's formal parameters to mode `in` -- `in out`/
   --  `out` on a function parameter is an Ada 95 relaxation this column
   --  does not have. Every other RM83 primitive layer here that advances
   --  state through a return value (Zerodds_Ada83_Wire's Length, this
   --  package's own Pack_Object_Id) takes only `in` parameters for the
   --  same reason.
   procedure Next_Id (C : in out Client; Id : out Long_Integer) is
   begin
      Id := C.Next_Request_Id;
      if C.Next_Request_Id = 65535 then
         C.Next_Request_Id := 1;
      else
         C.Next_Request_Id := C.Next_Request_Id + 1;
      end if;
   end Next_Id;

   function Is_Connected (C : Client) return Boolean is
   begin
      return C.Status = Connected;
   end Is_Connected;

   procedure Create_Object
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean)
   is
   begin
      Ok := Is_Connected (C);
      Request_Id := 0;
      if Ok then
         Next_Id (C, Request_Id);
      end if;
   end Create_Object;

   procedure Delete_Object
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean)
   is
   begin
      Ok := Is_Connected (C);
      Request_Id := 0;
      if Ok then
         Next_Id (C, Request_Id);
      end if;
   end Delete_Object;

   procedure Request_Write
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean)
   is
   begin
      Ok := Is_Connected (C);
      Request_Id := 0;
      if Ok then
         Next_Id (C, Request_Id);
      end if;
   end Request_Write;

   procedure Request_Read
     (C : in out Client; Request_Id : out Long_Integer; Ok : out Boolean)
   is
   begin
      Ok := Is_Connected (C);
      Request_Id := 0;
      if Ok then
         Next_Id (C, Request_Id);
      end if;
   end Request_Read;

   procedure Disconnect (C : in out Client) is
   begin
      C.Status := Disconnected;
   end Disconnect;

   ------------------------------------------------------------------
   --  framing
   ------------------------------------------------------------------

   --  MessageHeader WITH ClientKey (session_id <= 127, section 8.3.2.1):
   --  8 bytes, at F.Data (0 .. 7).
   procedure Put_Header_With_Key
     (F : in out Frame; Session_Id, Stream_Id : Byte;
      Seq : Long_Integer; Key : Client_Key_Type)
   is
   begin
      F.Data (0) := Session_Id;
      F.Data (1) := Stream_Id;
      F.Data (2) := Byte (Seq mod 256);
      F.Data (3) := Byte ((Seq / 256) mod 256);
      F.Data (4) := Key (0);
      F.Data (5) := Key (1);
      F.Data (6) := Key (2);
      F.Data (7) := Key (3);
      F.Len := 8;
   end Put_Header_With_Key;

   --  SubmessageHeader (4 bytes, section 8.3.4): id + flags +
   --  submessage_length (LE, always -- even under a BE body).
   procedure Put_Sm_Header
     (F : in out Frame; Sm_Id, Flags : Byte; Body_Len : Integer)
   is
      At_Pos : constant Integer := F.Len;
   begin
      F.Data (At_Pos) := Sm_Id;
      F.Data (At_Pos + 1) := Flags;
      F.Data (At_Pos + 2) := Byte (Body_Len mod 256);
      F.Data (At_Pos + 3) := Byte ((Body_Len / 256) mod 256);
      F.Len := At_Pos + 4;
   end Put_Sm_Header;

   --  request_id (2 octets, spec section 7.7.8) as this SDK's own choice
   --  of byte order for a host-side monotonic counter: big-endian,
   --  matching ObjectId's own wire order and the spec's own Annex B
   --  examples ("request_id = {0xAA, 0x01}"). The wire itself treats
   --  request_id as two opaque octets -- crates/xrce's BaseObjectRequest
   --  never interprets it numerically.
   procedure Put_Request_Id (F : in out Frame; Request_Id : Long_Integer) is
      At_Pos : constant Integer := F.Len;
   begin
      F.Data (At_Pos) := Byte ((Request_Id / 256) mod 256);
      F.Data (At_Pos + 1) := Byte (Request_Id mod 256);
      F.Len := At_Pos + 2;
   end Put_Request_Id;

   --  ObjectId (2 octets, big-endian).
   procedure Put_Object_Id (F : in out Frame; Object_Id : Long_Integer) is
      At_Pos : constant Integer := F.Len;
   begin
      F.Data (At_Pos) := Byte ((Object_Id / 256) mod 256);
      F.Data (At_Pos + 1) := Byte (Object_Id mod 256);
      F.Len := At_Pos + 2;
   end Put_Object_Id;

   procedure Put_U32_Le (F : in out Frame; V : Long_Integer) is
      At_Pos : constant Integer := F.Len;
   begin
      F.Data (At_Pos)     := Byte (V mod 256);
      F.Data (At_Pos + 1) := Byte ((V / 256) mod 256);
      F.Data (At_Pos + 2) := Byte ((V / 65536) mod 256);
      F.Data (At_Pos + 3) := Byte ((V / 16777216) mod 256);
      F.Len := At_Pos + 4;
   end Put_U32_Le;

   procedure Put_Byte (F : in out Frame; V : Byte) is
   begin
      F.Data (F.Len) := V;
      F.Len := F.Len + 1;
   end Put_Byte;

   function Encode_Client_Representation
     (Version, Vendor_Id : Two_Byte_Field;
      Key                : Client_Key_Type;
      Session_Id         : Byte;
      Requested_Mtu      : Long_Integer := Default_Mtu;
      Timestamps         : Representation_Timestamps := Absent;
      Seconds            : Long_Integer := 0;
      Nanoseconds        : Long_Integer := 0;
      Mode               : Zerodds_Ada83_Reliable.Unannotated_Types :=
        Zerodds_Ada83_Reliable.Appendable) return Frame
   is
      --  Ada 83 has no `use type`: the equality of the mode is renamed.
      function "="
        (L, R : Zerodds_Ada83_Reliable.Unannotated_Types) return Boolean
        renames Zerodds_Ada83_Reliable."=";
      F    : Frame;
      Secs : Long_Integer := Seconds;
   begin
      F.Len := 0;
      Put_Byte (F, Byte (Character'Pos ('X')));
      Put_Byte (F, Byte (Character'Pos ('R')));
      Put_Byte (F, Byte (Character'Pos ('C')));
      Put_Byte (F, Byte (Character'Pos ('E')));
      Put_Byte (F, Version (0));
      Put_Byte (F, Version (1));
      Put_Byte (F, Vendor_Id (0));
      Put_Byte (F, Vendor_Id (1));
      if Timestamps = Present then
         --  Time_t is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER
         --  u32 = 8 unless the FINAL form is selected.
         if Mode = Zerodds_Ada83_Reliable.Appendable then
            Put_U32_Le (F, 8);
         end if;
         if Secs < 0 then
            Secs := Secs + 4294967296;  --  two's complement i32
         end if;
         Put_U32_Le (F, Secs);
         Put_U32_Le (F, Nanoseconds);
      end if;
      Put_Byte (F, Key (0));
      Put_Byte (F, Key (1));
      Put_Byte (F, Key (2));
      Put_Byte (F, Key (3));
      Put_Byte (F, Session_Id);
      Put_Byte (F, 0);  --  properties absent
      Put_Byte (F, Byte (Requested_Mtu mod 256));
      Put_Byte (F, Byte ((Requested_Mtu / 256) mod 256));
      return F;
   end Encode_Client_Representation;

   procedure Set_Representation_Timestamps
     (C     : in out Client;
      Value : Representation_Timestamps;
      Mode  : Zerodds_Ada83_Reliable.Unannotated_Types :=
        Zerodds_Ada83_Reliable.Appendable)
   is
   begin
      C.Timestamps := Value;
      C.Timestamps_Mode := Mode;
   end Set_Representation_Timestamps;

   function Frame_Connect
     (C                  : Client;
      Version, Vendor_Id : Two_Byte_Field;
      Requested_Mtu      : Long_Integer := Default_Mtu) return Frame
   is
      Secs  : Long_Integer := 0;
      Nanos : Long_Integer := 0;
      Elapsed : Duration;
      Frac    : Duration;
   begin
      if C.Timestamps = Present then
         --  Seconds since 1970-01-01 (Calendar's own epoch is 1901).
         Elapsed := Calendar."-" (Calendar.Clock,
                                  Calendar.Time_Of (1970, 1, 1, 0.0));
         Secs := Long_Integer (Elapsed);  --  rounds; floor below
         if Duration (Secs) > Elapsed then
            Secs := Secs - 1;
         end if;
         Frac := Elapsed - Duration (Secs);
         Nanos := Long_Integer (Frac * 1_000_000_000);
         if Nanos > 999_999_999 then
            Nanos := 999_999_999;
         end if;
      end if;
      return Frame_Create_Client
        (C.Session_Id, C.Client_Key,
         Encode_Client_Representation
           (Version, Vendor_Id, C.Client_Key, C.Session_Id, Requested_Mtu,
            C.Timestamps, Secs, Nanos, C.Timestamps_Mode));
   end Frame_Connect;

   function Frame_Create_Client
     (Session_Id     : Byte;
      Key            : Client_Key_Type;
      Representation : Frame) return Frame
   is
      F : Frame;
   begin
      --  DDS-XRCE 1.0 section 8.3.2.1: no session exists yet, so the header
      --  carries SESSION_ID_NONE_WITH_CLIENT_KEY (0) + ClientKey (8 bytes)
      --  for a requested Session_Id of 0 .. 127, and
      --  SESSION_ID_NONE_WITHOUT_CLIENT_KEY (128) without ClientKey (4
      --  bytes) for 128 .. 255. Session_Id itself is in the payload.
      if Session_Id < 128 then
         Put_Header_With_Key (F, 0, Stream_None, 0, Key);
      else
         F.Data (0) := 128;
         F.Data (1) := Stream_None;
         F.Data (2) := 0;
         F.Data (3) := 0;
         F.Len := 4;
      end if;
      Put_Sm_Header (F, Sm_Create_Client,
                     1, --  E-flag only -- CreateClientPayload::
                        --  into_submessage's hardcoded flags
                     Representation.Len);
      for I in 0 .. Representation.Len - 1 loop
         F.Data (F.Len) := Representation.Data (I);
         F.Len := F.Len + 1;
      end loop;
      return F;
   end Frame_Create_Client;

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
      Reuse, Replace   : Boolean) return Frame
   is
      F            : Frame;
      Ref_Len      : constant Integer := Reference'Length;
      --  kind(1) + disc(1) + 2 pad + len(4) + string + NUL, plus the
      --  trailing parent ObjectId(2, unaligned) when Has_Parent.
      Parent_Bytes : constant Integer := Boolean'Pos (Has_Parent) * 2;
      Body_Len     : constant Integer := 4 + 1 + 1 + 2 + 4 + Ref_Len + 1 + Parent_Bytes;
      Flags        : Byte := 1;  -- E-flag
      Len_Incl_Nul : constant Long_Integer := Long_Integer (Ref_Len) + 1;
   begin
      if Reuse then
         Flags := Flags + 2;
      end if;
      if Replace then
         Flags := Flags + 4;
      end if;

      Put_Header_With_Key (F, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (F, Sm_Create, Flags, Body_Len);
      Put_Request_Id (F, Request_Id);
      Put_Object_Id (F, Object_Id);

      --  ObjectVariant (section 7.7.3, two-level): outer ObjectKind byte,
      --  then disc=0x01 (REPRESENTATION_BY_REFERENCE) + 2 bytes padding +
      --  u32 length (LE, incl. NUL) + string + NUL, then the trailing
      --  parent ObjectId when this kind carries one.
      F.Data (F.Len) := Object_Kind;
      declare
         At_Pos : constant Integer := F.Len + 1;
      begin
         F.Data (At_Pos) := 1;
         F.Data (At_Pos + 1) := 0;
         F.Data (At_Pos + 2) := 0;
         F.Data (At_Pos + 3) := Byte (Len_Incl_Nul mod 256);
         F.Data (At_Pos + 4) := Byte ((Len_Incl_Nul / 256) mod 256);
         F.Data (At_Pos + 5) := Byte ((Len_Incl_Nul / 65536) mod 256);
         F.Data (At_Pos + 6) := Byte ((Len_Incl_Nul / 16777216) mod 256);
         F.Len := At_Pos + 7;
      end;
      for I in Reference'Range loop
         F.Data (F.Len) := Byte (Character'Pos (Reference (I)));
         F.Len := F.Len + 1;
      end loop;
      F.Data (F.Len) := 0;  -- NUL terminator
      F.Len := F.Len + 1;
      if Has_Parent then
         Put_Object_Id (F, Parent_Object_Id);
      end if;
      return F;
   end Frame_Create;

   function Frame_Delete
     (Session_Id : Byte;
      Key        : Client_Key_Type;
      Seq        : Long_Integer;
      Request_Id : Long_Integer;
      Object_Id  : Long_Integer) return Frame
   is
      F : Frame;
   begin
      Put_Header_With_Key (F, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (F, Sm_Delete, 1, 4);
      Put_Request_Id (F, Request_Id);
      Put_Object_Id (F, Object_Id);
      return F;
   end Frame_Delete;

   function Frame_Write_Data
     (Session_Id       : Byte;
      Key              : Client_Key_Type;
      Seq              : Long_Integer;
      Request_Id       : Long_Integer;
      Writer_Object_Id : Long_Integer;
      Sample           : Byte_Array) return Frame
   is
      F        : Frame;
      Body_Len : constant Integer := 4 + Sample'Length;
   begin
      Put_Header_With_Key (F, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (F, Sm_Write_Data,
                     1, --  E-flag | FORMAT_DATA(0b000)<<1
                     Body_Len);
      Put_Request_Id (F, Request_Id);
      Put_Object_Id (F, Writer_Object_Id);
      for I in Sample'Range loop
         F.Data (F.Len) := Sample (I);
         F.Len := F.Len + 1;
      end loop;
      return F;
   end Frame_Write_Data;

   function Frame_Read_Data
     (Session_Id          : Byte;
      Key                  : Client_Key_Type;
      Seq                  : Long_Integer;
      Request_Id           : Long_Integer;
      Reader_Object_Id     : Long_Integer;
      Preferred_Stream_Id  : Byte) return Frame
   is
      F : Frame;
   begin
      Put_Header_With_Key (F, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      --  BaseObjectRequest(4) + ReadSpecification(4): preferred_stream_id(1)
      --  + data_format(1) + content_filter?(1)=0 + delivery_control?(1)=0.
      Put_Sm_Header (F, Sm_Read_Data, 1, 8);
      Put_Request_Id (F, Request_Id);
      Put_Object_Id (F, Reader_Object_Id);
      F.Data (F.Len) := Preferred_Stream_Id;
      F.Data (F.Len + 1) := 0;  -- data_format = FORMAT_DATA
      F.Data (F.Len + 2) := 0;  -- content_filter_expression? = absent
      F.Data (F.Len + 3) := 0;  -- delivery_control? = absent
      F.Len := F.Len + 4;
      return F;
   end Frame_Read_Data;

end Zerodds_Ada83_Xrce_Client;
