-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Ada.Strings.Fixed;
with Zdw; use Zdw;

package body Deep_Reading is

   use Interfaces;
   use type Interfaces.C.int;
   use type Interfaces.C.char;

   procedure Set_Label (R : in out Reading; S : String) is
   begin
      R.Label := [others => Interfaces.C.nul];
      for I in S'Range loop
         exit when Natural (I - S'First) >= Label_Cap - 1;
         R.Label (Interfaces.C.size_t (I - S'First)) := Interfaces.C.To_C (S (I));
      end loop;
   end Set_Label;

   function Label_Str (R : Reading) return String is
      Result : String (1 .. Label_Cap);
      N      : Natural := 0;
   begin
      for I in R.Label'Range loop
         exit when R.Label (I) = Interfaces.C.nul;
         N := N + 1;
         Result (N) := Interfaces.C.To_Ada (R.Label (I));
      end loop;
      return Result (1 .. N);
   end Label_Str;

   function Marshal (R : Reading) return Frame_Store is
      FS  : Frame_Store;
      W   : aliased Zdw_Writer;
      Lbl : aliased Interfaces.C.char_array := R.Label;
      Ign : Interfaces.C.int;
   begin
      Writer_Init (W'Access, FS.Data'Address, FS.Data'Length, ZDW_LE);
      Ign := Put_U32 (W'Access, R.Id);
      Ign := Put_F32 (W'Access, R.Value);
      Ign := Put_String (W'Access, Lbl'Address);
      FS.Len := Natural (W.Len);
      return FS;
   end Marshal;

   function Unmarshal (Data : Bytes) return Reading is
      Rd   : aliased Zdw_Reader;
      D    : aliased Bytes := Data;
      R    : Reading;
      VId  : aliased Interfaces.C.unsigned_long;
      VVal : aliased Interfaces.C.C_float;
      Ign  : Interfaces.C.int;
   begin
      Reader_Init (Rd'Access, D'Address, Interfaces.C.size_t (D'Length), ZDW_LE);
      Ign := Get_U32 (Rd'Access, VId'Access);
      R.Id := VId;
      Ign := Get_F32 (Rd'Access, VVal'Access);
      R.Value := VVal;
      Ign := Get_String (Rd'Access, R.Label'Address, Label_Cap);
      return R;
   end Unmarshal;

   function Frame (Seq : Interfaces.Unsigned_16; Body_FS : Frame_Store)
                   return Frame_Store is
      FS : Frame_Store;
      N  : constant Natural := Body_FS.Len;
   begin
      FS.Data (0) := 16#80#;  -- session: no-key
      FS.Data (1) := 16#01#;  -- stream: best-effort
      FS.Data (2) := Byte (Seq and 16#FF#);
      FS.Data (3) := Byte (Shift_Right (Seq, 8) and 16#FF#);
      FS.Data (4) := 16#07#;  -- submessage: WRITE_DATA
      FS.Data (5) := 16#03#;
      FS.Data (6) := Byte (Unsigned_16 (N) and 16#FF#);
      FS.Data (7) := Byte (Shift_Right (Unsigned_16 (N), 8) and 16#FF#);
      for I in 0 .. N - 1 loop
         FS.Data (8 + I) := Body_FS.Data (I);
      end loop;
      FS.Len := 8 + N;
      return FS;
   end Frame;

   --  Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
   --  len_hi] then the sample body. The 16-bit little-endian submessage_length
   --  (bytes 6..7) bounds the body exactly: bytes 8 .. 8 + Sm_Len - 1, never
   --  8 .. F.Len - 1, so trailing padding or an appended submessage is not
   --  folded into the sample.
   --
   --  Accept WRITE_DATA (16#07#, endpoint->hub / loopback) and DATA
   --  (16#09#, hub->endpoint) — the pong the hub sends is DATA. See
   --  DDS-XRCE spec section 8.3.5. Len=0 (reject) for a short header, a wrong
   --  submessage id, or a declared length that runs past the datagram
   --  (truncation / wrong length).
   function Deframe (F : Frame_Store) return Frame_Store is
      FS     : Frame_Store;
      Sm_Len : Natural;
   begin
      FS.Len := 0;
      if F.Len >= 8
        and then (F.Data (4) = 16#07# or else F.Data (4) = 16#09#)
      then
         Sm_Len := Natural (F.Data (6)) + Natural (F.Data (7)) * 256;
         if 8 + Sm_Len <= F.Len then
            for I in 0 .. Sm_Len - 1 loop
               FS.Data (I) := F.Data (8 + I);
            end loop;
            FS.Len := Sm_Len;
         end if;
      end if;
      return FS;
   end Deframe;

   function Hex (V : Interfaces.C.unsigned_long) return String is
      Digs : constant String := "0123456789abcdef";
      Buf  : String (1 .. 16);
      X    : Unsigned_64 := Unsigned_64 (V);
      N    : Natural := 0;
   begin
      if X = 0 then
         return "0";
      end if;
      while X > 0 loop
         N := N + 1;
         Buf (Buf'Last - N + 1) := Digs (Integer (X and 16#F#) + 1);
         X := Shift_Right (X, 4);
      end loop;
      return Buf (Buf'Last - N + 1 .. Buf'Last);
   end Hex;

   function F1 (V : Interfaces.C.C_float) return String is
      use Ada.Strings, Ada.Strings.Fixed;
      Scaled  : constant Integer := Integer (Float'Rounding (Float (V) * 10.0));
      A       : constant Integer := abs Scaled;
      Whole   : constant Integer := A / 10;
      Frac    : constant Integer := A rem 10;
      Sign    : constant String := (if Scaled < 0 then "-" else "");
   begin
      return Sign & Trim (Integer'Image (Whole), Left) & "."
             & Trim (Integer'Image (Frac), Left);
   end F1;

   function Digit2 (N : Natural) return String is
      Digs : constant String := "0123456789";
      Tens : constant Natural := (N / 10) mod 10;
      Ones : constant Natural := N mod 10;
   begin
      return Digs (Tens + 1 .. Tens + 1) & Digs (Ones + 1 .. Ones + 1);
   end Digit2;

   protected body Mailbox is
      procedure Deliver (F : Frame_Store) is
      begin
         Q (Tail) := F;
         Tail := (Tail + 1) mod Q'Length;
         Count := Count + 1;
      end Deliver;

      procedure Try_Receive (F : out Frame_Store; Got : out Boolean) is
      begin
         if Count = 0 then
            F := (Data => [others => 0], Len => 0);
            Got := False;
         else
            F := Q (Head);
            Head := (Head + 1) mod Q'Length;
            Count := Count - 1;
            Got := True;
         end if;
      end Try_Receive;

      entry Receive (F : out Frame_Store) when Count > 0 is
      begin
         F := Q (Head);
         Head := (Head + 1) mod Q'Length;
         Count := Count - 1;
      end Receive;
   end Mailbox;

   task body Reader_Task is
      FS, Body_FS : Frame_Store;
      Got         : Boolean;
      Done        : Natural := 0;
   begin
      while Done < N loop
         Transport.Try_Receive (FS, Got);
         if Got then
            Body_FS := Deframe (FS);
            if Body_FS.Len > 0 then
               Inbox.Deliver (Body_FS);
               Done := Done + 1;
            end if;
         else
            delay 0.001;
         end if;
      end loop;
   end Reader_Task;

end Deep_Reading;
