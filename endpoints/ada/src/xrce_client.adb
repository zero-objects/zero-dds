-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Ada.Calendar;

package body Xrce_Client is

   Sm_Create_Client : constant Byte := 16#00#;
   Sm_Create        : constant Byte := 16#01#;
   Sm_Delete        : constant Byte := 16#03#;
   Sm_Write_Data    : constant Byte := 16#07#;
   Sm_Read_Data     : constant Byte := 16#08#;

   Stream_None             : constant Byte := 16#00#;
   Stream_Builtin_Reliable : constant Byte := 16#80#;

   ------------------------------------------------------------------
   --  state machine
   ------------------------------------------------------------------

   function Pack_Object_Id (Raw_Id : Unsigned_16; Kind : Byte) return Unsigned_16 is
   begin
      return Shift_Left (Raw_Id, 4) or Unsigned_16 (Kind);
   end Pack_Object_Id;

   procedure Init (C : out Client; Key : Client_Key_Type) is
   begin
      C.Client_Key := Key;
      C.Status := Disconnected;
      C.Session_Id := 0;
      C.Next_Request_Id := 1;
   end Init;

   procedure Connect
     (C : in out Client; Requested_Session_Id : Byte; Result : out Op_Result)
   is
   begin
      if C.Status /= Disconnected then
         Result := Invalid_State;
         return;
      end if;
      C.Session_Id := Requested_Session_Id;
      C.Status := Connecting;
      Result := Ok;
   end Connect;

   procedure Mark_Connected (C : in out Client; Result : out Op_Result) is
   begin
      if C.Status /= Connecting then
         Result := Invalid_State;
         return;
      end if;
      C.Status := Connected;
      Result := Ok;
   end Mark_Connected;

   --  Next request_id: 1..65535, wrapping without ever landing on 0 -- the
   --  same "wrapping_add(1).max(1)" rule the Rust state machine applies.
   function Next_Id (C : in out Client) return Unsigned_16 is
      Req : constant Unsigned_16 := C.Next_Request_Id;
      Nxt : Unsigned_16;
   begin
      if C.Next_Request_Id = 16#FFFF# then
         Nxt := 1;
      else
         Nxt := C.Next_Request_Id + 1;
      end if;
      C.Next_Request_Id := Nxt;
      return Req;
   end Next_Id;

   function Require_Connected (C : Client) return Op_Result is
   begin
      if C.Status /= Connected then
         return Invalid_State;
      end if;
      return Ok;
   end Require_Connected;

   procedure Create_Object
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result)
   is
   begin
      Result := Require_Connected (C);
      Request_Id := 0;
      if Result = Ok then
         Request_Id := Next_Id (C);
      end if;
   end Create_Object;

   procedure Delete_Object
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result)
   is
   begin
      Result := Require_Connected (C);
      Request_Id := 0;
      if Result = Ok then
         Request_Id := Next_Id (C);
      end if;
   end Delete_Object;

   procedure Request_Write
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result)
   is
   begin
      Result := Require_Connected (C);
      Request_Id := 0;
      if Result = Ok then
         Request_Id := Next_Id (C);
      end if;
   end Request_Write;

   procedure Request_Read
     (C : in out Client; Request_Id : out Unsigned_16; Result : out Op_Result)
   is
   begin
      Result := Require_Connected (C);
      Request_Id := 0;
      if Result = Ok then
         Request_Id := Next_Id (C);
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
   --  8 bytes, at FS.Data (0 .. 7).
   procedure Put_Header_With_Key
     (FS : in out Frame_Store; Session_Id, Stream_Id : Byte;
      Seq : Unsigned_16; Key : Client_Key_Type)
   is
   begin
      FS.Data (0) := Session_Id;
      FS.Data (1) := Stream_Id;
      FS.Data (2) := Byte (Seq and 16#FF#);
      FS.Data (3) := Byte (Shift_Right (Seq, 8) and 16#FF#);
      FS.Data (4) := Key (0);
      FS.Data (5) := Key (1);
      FS.Data (6) := Key (2);
      FS.Data (7) := Key (3);
      FS.Len := 8;
   end Put_Header_With_Key;

   --  SubmessageHeader (4 bytes, section 8.3.4): id + flags +
   --  submessage_length (LE, always -- even under a BE body).
   procedure Put_Sm_Header
     (FS : in out Frame_Store; Sm_Id, Flags : Byte; Body_Len : Natural)
   is
      At_Pos : constant Natural := FS.Len;
   begin
      FS.Data (At_Pos) := Sm_Id;
      FS.Data (At_Pos + 1) := Flags;
      FS.Data (At_Pos + 2) := Byte (Body_Len mod 256);
      FS.Data (At_Pos + 3) := Byte (Body_Len / 256);
      FS.Len := At_Pos + 4;
   end Put_Sm_Header;

   --  request_id (2 octets, spec section 7.7.8) as this SDK's own choice
   --  of byte order for a host-side monotonic counter: big-endian,
   --  matching ObjectId's own wire order and the spec's own Annex B
   --  examples ("request_id = {0xAA, 0x01}"). The wire itself treats
   --  request_id as two opaque octets -- crates/xrce's BaseObjectRequest
   --  never interprets it numerically.
   procedure Put_Request_Id (FS : in out Frame_Store; Request_Id : Unsigned_16) is
      At_Pos : constant Natural := FS.Len;
   begin
      FS.Data (At_Pos) := Byte (Shift_Right (Request_Id, 8) and 16#FF#);
      FS.Data (At_Pos + 1) := Byte (Request_Id and 16#FF#);
      FS.Len := At_Pos + 2;
   end Put_Request_Id;

   --  ObjectId (2 octets, big-endian -- object_id.rs::to_bytes).
   procedure Put_Object_Id (FS : in out Frame_Store; Object_Id : Unsigned_16) is
      At_Pos : constant Natural := FS.Len;
   begin
      FS.Data (At_Pos) := Byte (Shift_Right (Object_Id, 8) and 16#FF#);
      FS.Data (At_Pos + 1) := Byte (Object_Id and 16#FF#);
      FS.Len := At_Pos + 2;
   end Put_Object_Id;

   procedure Put_U32_Le (FS : in out Frame_Store; V : Unsigned_32) is
      At_Pos : constant Natural := FS.Len;
   begin
      FS.Data (At_Pos)     := Byte (V and 16#FF#);
      FS.Data (At_Pos + 1) := Byte (Shift_Right (V, 8) and 16#FF#);
      FS.Data (At_Pos + 2) := Byte (Shift_Right (V, 16) and 16#FF#);
      FS.Data (At_Pos + 3) := Byte (Shift_Right (V, 24) and 16#FF#);
      FS.Len := At_Pos + 4;
   end Put_U32_Le;

   procedure Put_Byte (FS : in out Frame_Store; V : Byte) is
   begin
      FS.Data (FS.Len) := V;
      FS.Len := FS.Len + 1;
   end Put_Byte;

   function Encode_Client_Representation
     (Version, Vendor_Id : Two_Byte_Field;
      Key                : Client_Key_Type;
      Session_Id         : Byte;
      Requested_Mtu      : Unsigned_16 := Default_Mtu;
      Timestamps         : Representation_Timestamps := Absent;
      Seconds            : Integer_32 := 0;
      Nanoseconds        : Unsigned_32 := 0;
      Mode               : Reliable.Unannotated_Types := Reliable.Appendable)
      return Frame_Store
   is
      use type Reliable.Unannotated_Types;
      FS : Frame_Store;
   begin
      Put_Byte (FS, Byte (Character'Pos ('X')));
      Put_Byte (FS, Byte (Character'Pos ('R')));
      Put_Byte (FS, Byte (Character'Pos ('C')));
      Put_Byte (FS, Byte (Character'Pos ('E')));
      Put_Byte (FS, Version (0));
      Put_Byte (FS, Version (1));
      Put_Byte (FS, Vendor_Id (0));
      Put_Byte (FS, Vendor_Id (1));
      if Timestamps = Present then
         --  Time_t is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER
         --  u32 = 8 unless the FINAL form is selected.
         if Mode = Reliable.Appendable then
            Put_U32_Le (FS, 8);
         end if;
         Put_U32_Le (FS, Unsigned_32'Mod (Seconds));
         Put_U32_Le (FS, Nanoseconds);
      end if;
      Put_Byte (FS, Key (0));
      Put_Byte (FS, Key (1));
      Put_Byte (FS, Key (2));
      Put_Byte (FS, Key (3));
      Put_Byte (FS, Session_Id);
      Put_Byte (FS, 0);  --  properties absent
      Put_Byte (FS, Byte (Requested_Mtu and 16#FF#));
      Put_Byte (FS, Byte (Shift_Right (Requested_Mtu, 8) and 16#FF#));
      return FS;
   end Encode_Client_Representation;

   procedure Set_Representation_Timestamps
     (C     : in out Client;
      Value : Representation_Timestamps;
      Mode  : Reliable.Unannotated_Types := Reliable.Appendable)
   is
   begin
      C.Timestamps := Value;
      C.Timestamps_Mode := Mode;
   end Set_Representation_Timestamps;

   function Frame_Connect
     (C                  : Client;
      Version, Vendor_Id : Two_Byte_Field;
      Requested_Mtu      : Unsigned_16 := Default_Mtu) return Frame_Store
   is
      use type Ada.Calendar.Time;
      Secs  : Integer_32 := 0;
      Nanos : Unsigned_32 := 0;
   begin
      if C.Timestamps = Present then
         declare
            Elapsed : constant Duration :=
              Ada.Calendar.Clock - Ada.Calendar.Time_Of (1970, 1, 1, 0.0);
            Whole   : Long_Long_Integer := Long_Long_Integer (Elapsed);
         begin
            --  Long_Long_Integer (Duration) rounds; the second count floors.
            if Duration (Whole) > Elapsed then
               Whole := Whole - 1;
            end if;
            declare
               Frac : constant Duration := Elapsed - Duration (Whole);
               Ns   : Long_Long_Integer :=
                 Long_Long_Integer (Frac * 1_000_000_000);
            begin
               if Ns > 999_999_999 then
                  Ns := 999_999_999;
               end if;
               Secs := Integer_32 (Whole);
               Nanos := Unsigned_32 (Ns);
            end;
         end;
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
      Representation : Frame_Store) return Frame_Store
   is
      FS : Frame_Store;
   begin
      --  DDS-XRCE 1.0 section 8.3.2.1: no session exists yet, so the header
      --  carries SESSION_ID_NONE_WITH_CLIENT_KEY (0x00) + ClientKey (8
      --  bytes) for a requested Session_Id of 0 .. 127, and
      --  SESSION_ID_NONE_WITHOUT_CLIENT_KEY (0x80) without ClientKey (4
      --  bytes) for 128 .. 255. Session_Id itself is in the payload.
      if Session_Id < 128 then
         Put_Header_With_Key (FS, 16#00#, Stream_None, 0, Key);
      else
         FS.Data (0) := 16#80#;
         FS.Data (1) := Stream_None;
         FS.Data (2) := 0;
         FS.Data (3) := 0;
         FS.Len := 4;
      end if;
      Put_Sm_Header (FS, Sm_Create_Client,
                     16#01#, --  E-flag only -- CreateClientPayload::
                             --  into_submessage's hardcoded flags
                     Representation.Len);
      for I in 0 .. Representation.Len - 1 loop
         FS.Data (FS.Len) := Representation.Data (I);
         FS.Len := FS.Len + 1;
      end loop;
      return FS;
   end Frame_Create_Client;

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
      Reuse, Replace     : Boolean) return Frame_Store
   is
      FS       : Frame_Store;
      Ref_Len  : constant Natural := Reference'Length;
      --  kind(1) + disc(1) + 2 pad + len(4) + string + NUL, plus the
      --  trailing parent ObjectId(2, unaligned) when Has_Parent.
      Body_Len : constant Natural :=
        4 + 1 + 1 + 2 + 4 + Ref_Len + 1 + (if Has_Parent then 2 else 0);
      Flags    : Byte := 16#01#;  -- E-flag
      Len_Incl_Nul : constant Unsigned_32 := Unsigned_32 (Ref_Len) + 1;
   begin
      if Reuse then
         Flags := Flags or 16#02#;
      end if;
      if Replace then
         Flags := Flags or 16#04#;
      end if;

      Put_Header_With_Key (FS, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (FS, Sm_Create, Flags, Body_Len);
      Put_Request_Id (FS, Request_Id);
      Put_Object_Id (FS, Object_Id);

      --  ObjectVariant (section 7.7.3, two-level): outer ObjectKind byte,
      --  then disc=0x01 (REPRESENTATION_BY_REFERENCE) + 2 bytes padding +
      --  u32 length (LE, incl. NUL) + string + NUL, then the trailing
      --  parent ObjectId when this kind carries one.
      FS.Data (FS.Len) := Object_Kind;
      declare
         At_Pos : constant Natural := FS.Len + 1;
      begin
         FS.Data (At_Pos) := 16#01#;
         FS.Data (At_Pos + 1) := 0;
         FS.Data (At_Pos + 2) := 0;
         FS.Data (At_Pos + 3) := Byte (Len_Incl_Nul and 16#FF#);
         FS.Data (At_Pos + 4) := Byte (Shift_Right (Len_Incl_Nul, 8) and 16#FF#);
         FS.Data (At_Pos + 5) := Byte (Shift_Right (Len_Incl_Nul, 16) and 16#FF#);
         FS.Data (At_Pos + 6) := Byte (Shift_Right (Len_Incl_Nul, 24) and 16#FF#);
         FS.Len := At_Pos + 7;
      end;
      for I in Reference'Range loop
         FS.Data (FS.Len) := Byte (Character'Pos (Reference (I)));
         FS.Len := FS.Len + 1;
      end loop;
      FS.Data (FS.Len) := 0;  -- NUL terminator
      FS.Len := FS.Len + 1;
      if Has_Parent then
         Put_Object_Id (FS, Parent_Object_Id);
      end if;
      return FS;
   end Frame_Create;

   function Frame_Delete
     (Session_Id : Byte;
      Key        : Client_Key_Type;
      Seq        : Unsigned_16;
      Request_Id : Unsigned_16;
      Object_Id  : Unsigned_16) return Frame_Store
   is
      FS : Frame_Store;
   begin
      Put_Header_With_Key (FS, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (FS, Sm_Delete, 16#01#, 4);
      Put_Request_Id (FS, Request_Id);
      Put_Object_Id (FS, Object_Id);
      return FS;
   end Frame_Delete;

   function Frame_Write_Data
     (Session_Id       : Byte;
      Key              : Client_Key_Type;
      Seq              : Unsigned_16;
      Request_Id       : Unsigned_16;
      Writer_Object_Id : Unsigned_16;
      Sample           : Bytes) return Frame_Store
   is
      FS       : Frame_Store;
      Body_Len : constant Natural := 4 + Sample'Length;
   begin
      Put_Header_With_Key (FS, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      Put_Sm_Header (FS, Sm_Write_Data,
                     16#01#, --  E-flag | FORMAT_DATA(0b000)<<1
                     Body_Len);
      Put_Request_Id (FS, Request_Id);
      Put_Object_Id (FS, Writer_Object_Id);
      for I in Sample'Range loop
         FS.Data (FS.Len) := Sample (I);
         FS.Len := FS.Len + 1;
      end loop;
      return FS;
   end Frame_Write_Data;

   function Frame_Read_Data
     (Session_Id           : Byte;
      Key                  : Client_Key_Type;
      Seq                  : Unsigned_16;
      Request_Id           : Unsigned_16;
      Reader_Object_Id     : Unsigned_16;
      Preferred_Stream_Id  : Byte) return Frame_Store
   is
      FS : Frame_Store;
   begin
      Put_Header_With_Key (FS, Session_Id, Stream_Builtin_Reliable, Seq, Key);
      --  BaseObjectRequest(4) + ReadSpecification(4): preferred_stream_id(1)
      --  + data_format(1) + content_filter?(1)=0 + delivery_control?(1)=0.
      Put_Sm_Header (FS, Sm_Read_Data, 16#01#, 8);
      Put_Request_Id (FS, Request_Id);
      Put_Object_Id (FS, Reader_Object_Id);
      FS.Data (FS.Len) := Preferred_Stream_Id;
      FS.Data (FS.Len + 1) := 16#00#;  -- data_format = FORMAT_DATA
      FS.Data (FS.Len + 2) := 16#00#;  -- content_filter_expression? = absent
      FS.Data (FS.Len + 3) := 16#00#;  -- delivery_control? = absent
      FS.Len := FS.Len + 4;
      return FS;
   end Frame_Read_Data;

end Xrce_Client;
