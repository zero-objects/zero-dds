-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Unchecked_Conversion;

package body Zerodds_Ada83_Wire is

   type Bytes4 is array (0 .. 3) of Byte;
   pragma Pack (Bytes4);
   type Bytes2 is array (0 .. 1) of Byte;
   pragma Pack (Bytes2);

   function F32_To_Bytes is new Unchecked_Conversion (Float, Bytes4);
   function Probe_To_Bytes is new Unchecked_Conversion (Short_Integer, Bytes2);

   --  True on a big-endian host (first storage byte of a small positive value
   --  is the most significant one).
   function Host_Is_Big return Boolean is
      B : constant Bytes2 := Probe_To_Bytes (1);
   begin
      return B (0) = 0;
   end Host_Is_Big;

   procedure Init (W : in out Writer; Endian : Endianness) is
   begin
      W.Len := 0;
      W.Endian := Endian;
      W.Overflow := False;
   end Init;

   function Length (W : Writer) return Integer is
   begin
      return W.Len;
   end Length;

   procedure Put_Bytes (W : in out Writer; Data : Byte_Array) is
   begin
      if W.Overflow then
         return;
      end if;
      if W.Len + Data'Length > Max_Buffer then
         W.Overflow := True;
         return;
      end if;
      for I in Data'Range loop
         W.Buf (W.Len) := Data (I);
         W.Len := W.Len + 1;
      end loop;
   end Put_Bytes;

   procedure W_Align (W : in out Writer; Align : Integer) is
      Eff : Integer := Align;
      Pad : Integer;
   begin
      if Eff > 4 then
         Eff := 4;  -- XCDR2 max alignment
      end if;
      Pad := (Eff - (W.Len mod Eff)) mod Eff;
      while Pad > 0 loop
         if W.Len >= Max_Buffer then
            W.Overflow := True;
            return;
         end if;
         W.Buf (W.Len) := 0;
         W.Len := W.Len + 1;
         Pad := Pad - 1;
      end loop;
   end W_Align;

   --  Append `LE` (logical little-endian) at `Align`, reversing for BE wire.
   procedure Put_LE (W : in out Writer; Align : Integer; LE : Byte_Array) is
      Tmp : Byte_Array (0 .. LE'Length - 1);
      J   : Integer;
   begin
      W_Align (W, Align);
      if W.Endian = Big then
         J := LE'Length - 1;
         for I in LE'Range loop
            Tmp (J) := LE (I);
            J := J - 1;
         end loop;
      else
         J := 0;
         for I in LE'Range loop
            Tmp (J) := LE (I);
            J := J + 1;
         end loop;
      end if;
      Put_Bytes (W, Tmp);
   end Put_LE;

   function Le1 (V : Long_Integer) return Byte is
   begin
      return Byte (V mod 256);
   end Le1;

   procedure Put_U8 (W : in out Writer; V : Long_Integer) is
      B : Byte_Array (0 .. 0);
   begin
      B (0) := Le1 (V);
      Put_Bytes (W, B);  -- 1-byte, no alignment
   end Put_U8;

   procedure Put_U16 (W : in out Writer; V : Long_Integer) is
      LE : Byte_Array (0 .. 1);
   begin
      LE (0) := Le1 (V);
      LE (1) := Le1 (V / 256);
      Put_LE (W, 2, LE);
   end Put_U16;

   procedure Put_U32 (W : in out Writer; V : Long_Integer) is
      LE : Byte_Array (0 .. 3);
   begin
      LE (0) := Le1 (V);
      LE (1) := Le1 (V / 256);
      LE (2) := Le1 (V / 65536);
      LE (3) := Le1 (V / 16777216);
      Put_LE (W, 4, LE);
   end Put_U32;

   procedure Put_U64 (W : in out Writer; Hi, Lo : Long_Integer) is
      LE : Byte_Array (0 .. 7);
   begin
      LE (0) := Le1 (Lo);
      LE (1) := Le1 (Lo / 256);
      LE (2) := Le1 (Lo / 65536);
      LE (3) := Le1 (Lo / 16777216);
      LE (4) := Le1 (Hi);
      LE (5) := Le1 (Hi / 256);
      LE (6) := Le1 (Hi / 65536);
      LE (7) := Le1 (Hi / 16777216);
      Put_LE (W, 4, LE);  -- XCDR2: u64 aligns to 4
   end Put_U64;

   procedure Put_F32 (W : in out Writer; V : Float) is
      Host : constant Bytes4 := F32_To_Bytes (V);
      LE   : Byte_Array (0 .. 3);
   begin
      --  Host bytes -> logical little-endian.
      if Host_Is_Big then
         LE (0) := Host (3);
         LE (1) := Host (2);
         LE (2) := Host (1);
         LE (3) := Host (0);
      else
         LE (0) := Host (0);
         LE (1) := Host (1);
         LE (2) := Host (2);
         LE (3) := Host (3);
      end if;
      Put_LE (W, 4, LE);
   end Put_F32;

   procedure Put_String (W : in out Writer; S : String) is
      Body_Bytes : Byte_Array (0 .. S'Length - 1);
      K          : Integer := 0;
   begin
      for I in S'Range loop
         Body_Bytes (K) := Byte (Character'Pos (S (I)));
         K := K + 1;
      end loop;
      Put_U32 (W, Long_Integer (S'Length + 1));
      Put_Bytes (W, Body_Bytes);
      Put_U8 (W, 0);
   end Put_String;

   procedure Put_Seq_U8 (W : in out Writer; Data : Byte_Array) is
   begin
      Put_U32 (W, Long_Integer (Data'Length));
      Put_Bytes (W, Data);
   end Put_Seq_U8;

   procedure Init_Reader (R : out Reader; Data : Byte_Array; Endian : Endianness) is
      K : Integer := 0;
   begin
      R.Len := Data'Length;
      R.Pos := 0;
      R.Endian := Endian;
      for I in Data'Range loop
         R.Data (K) := Data (I);
         K := K + 1;
      end loop;
   end Init_Reader;

   procedure Get_U32 (R : in out Reader; V : out Long_Integer) is
      Pad : constant Integer := (4 - (R.Pos mod 4)) mod 4;
      B0, B1, B2, B3 : Long_Integer;
   begin
      R.Pos := R.Pos + Pad;  -- 4-align
      if R.Endian = Big then
         B3 := Long_Integer (R.Data (R.Pos));
         B2 := Long_Integer (R.Data (R.Pos + 1));
         B1 := Long_Integer (R.Data (R.Pos + 2));
         B0 := Long_Integer (R.Data (R.Pos + 3));
      else
         B0 := Long_Integer (R.Data (R.Pos));
         B1 := Long_Integer (R.Data (R.Pos + 1));
         B2 := Long_Integer (R.Data (R.Pos + 2));
         B3 := Long_Integer (R.Data (R.Pos + 3));
      end if;
      R.Pos := R.Pos + 4;
      V := B0 + B1 * 256 + B2 * 65536 + B3 * 16777216;
   end Get_U32;

end Zerodds_Ada83_Wire;
