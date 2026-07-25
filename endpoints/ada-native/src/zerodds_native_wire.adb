-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Ada.Unchecked_Conversion;

package body Zerodds_Native_Wire
  with SPARK_Mode => Off
is

   function F32_Bits is new Ada.Unchecked_Conversion (IEEE_Float_32, Unsigned_32);
   function F64_Bits is new Ada.Unchecked_Conversion (IEEE_Float_64, Unsigned_64);
   function Bits_F32 is new Ada.Unchecked_Conversion (Unsigned_32, IEEE_Float_32);
   function Bits_F64 is new Ada.Unchecked_Conversion (Unsigned_64, IEEE_Float_64);

   function Padding_For (Pos : Natural; Align : Positive) return Natural is
   begin
      return (Align - (Pos mod Align)) mod Align;
   end Padding_For;

   procedure Reverse_In_Place (B : in out Byte_Array) is
      I : Natural := B'First;
      J : Natural := B'Last;
      T : Byte;
   begin
      while I < J loop
         T := B (I);
         B (I) := B (J);
         B (J) := T;
         I := I + 1;
         J := J - 1;
      end loop;
   end Reverse_In_Place;

   --  --- writer ---

   procedure Init (W : in out Writer; Endian : Endianness) is
   begin
      W.Len := 0;
      W.Endian := Endian;
      W.Max_Align := Xcdr2_Max_Align;
      W.Overflow := False;
   end Init;

   function Bytes (W : Writer) return Byte_Array is
   begin
      return W.Buf (0 .. W.Len - 1);
   end Bytes;

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

   procedure W_Align (W : in out Writer; Align : Positive) is
      Eff : constant Positive :=
        (if Align > W.Max_Align then W.Max_Align else Align);
      Pad : Natural := Padding_For (W.Len, Eff);
   begin
      if W.Overflow then
         return;
      end if;
      if W.Len + Pad > Max_Buffer then
         W.Overflow := True;
         return;
      end if;
      while Pad > 0 loop
         W.Buf (W.Len) := 0;
         W.Len := W.Len + 1;
         Pad := Pad - 1;
      end loop;
   end W_Align;

   --  Writes `LE` (logical little-endian) at `Align`, in wire byte order.
   procedure Put_Uint (W : in out Writer; Align : Positive; LE : Byte_Array) is
      Tmp : Byte_Array := LE;
   begin
      if W.Overflow then
         return;
      end if;
      W_Align (W, Align);
      if W.Endian = Big then
         Reverse_In_Place (Tmp);
      end if;
      Put_Bytes (W, Tmp);
   end Put_Uint;

   procedure Put_U8 (W : in out Writer; V : Byte) is
   begin
      Put_Bytes (W, [0 => V]);
   end Put_U8;

   procedure Put_U16 (W : in out Writer; V : Unsigned_16) is
   begin
      Put_Uint (W, 2,
        [0 => Byte (V and 16#FF#),
         1 => Byte (Shift_Right (V, 8) and 16#FF#)]);
   end Put_U16;

   procedure Put_U32 (W : in out Writer; V : Unsigned_32) is
   begin
      Put_Uint (W, 4,
        [0 => Byte (V and 16#FF#),
         1 => Byte (Shift_Right (V, 8) and 16#FF#),
         2 => Byte (Shift_Right (V, 16) and 16#FF#),
         3 => Byte (Shift_Right (V, 24) and 16#FF#)]);
   end Put_U32;

   procedure Put_U64 (W : in out Writer; V : Unsigned_64) is
   begin
      Put_Uint (W, 8,
        [0 => Byte (V and 16#FF#),
         1 => Byte (Shift_Right (V, 8) and 16#FF#),
         2 => Byte (Shift_Right (V, 16) and 16#FF#),
         3 => Byte (Shift_Right (V, 24) and 16#FF#),
         4 => Byte (Shift_Right (V, 32) and 16#FF#),
         5 => Byte (Shift_Right (V, 40) and 16#FF#),
         6 => Byte (Shift_Right (V, 48) and 16#FF#),
         7 => Byte (Shift_Right (V, 56) and 16#FF#)]);
   end Put_U64;

   procedure Put_Bool (W : in out Writer; V : Boolean) is
   begin
      Put_U8 (W, (if V then 1 else 0));
   end Put_Bool;

   procedure Put_F32 (W : in out Writer; V : IEEE_Float_32) is
   begin
      Put_U32 (W, F32_Bits (V));
   end Put_F32;

   procedure Put_F64 (W : in out Writer; V : IEEE_Float_64) is
   begin
      Put_U64 (W, F64_Bits (V));
   end Put_F64;

   procedure Put_String (W : in out Writer; S : String) is
      Body_Bytes : Byte_Array (0 .. S'Length - 1);
      K          : Natural := 0;
   begin
      for C of S loop
         Body_Bytes (K) := Byte (Character'Pos (C));
         K := K + 1;
      end loop;
      Put_U32 (W, Unsigned_32 (S'Length + 1));
      Put_Bytes (W, Body_Bytes);
      Put_U8 (W, 0);
   end Put_String;

   procedure Put_Seq_U8 (W : in out Writer; Data : Byte_Array) is
   begin
      Put_U32 (W, Unsigned_32 (Data'Length));
      Put_Bytes (W, Data);
   end Put_Seq_U8;

   procedure DHeader_Begin (W : in out Writer; Body_Start : out Natural) is
   begin
      W_Align (W, 4);
      Put_Bytes (W, [0, 0, 0, 0]);
      Body_Start := W.Len;  -- body starts here; DHEADER is the 4 bytes before
   end DHeader_Begin;

   procedure DHeader_End (W : in out Writer; Body_Start : Natural) is
      Len : constant Unsigned_32 := Unsigned_32 (W.Len - Body_Start);
      LE  : Byte_Array (0 .. 3) :=
        [0 => Byte (Len and 16#FF#),
         1 => Byte (Shift_Right (Len, 8) and 16#FF#),
         2 => Byte (Shift_Right (Len, 16) and 16#FF#),
         3 => Byte (Shift_Right (Len, 24) and 16#FF#)];
   begin
      if W.Overflow then
         return;
      end if;
      if W.Endian = Big then
         Reverse_In_Place (LE);
      end if;
      for I in 0 .. 3 loop
         W.Buf (Body_Start - 4 + I) := LE (I);
      end loop;
   end DHeader_End;

   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Body_Start      : out Natural)
   is
      Em : constant Unsigned_32 :=
        (if Must_Understand then 16#8000_0000# else 0)
        + Shift_Left (Unsigned_32 (4), 28)
        + (Member_Id and 16#0FFF_FFFF#);
   begin
      Put_U32 (W, Em);
      DHeader_Begin (W, Body_Start);
   end EMHeader_Begin;

   procedure EMHeader_End (W : in out Writer; Body_Start : Natural) is
   begin
      DHeader_End (W, Body_Start);
   end EMHeader_End;

   --  --- reader ---

   procedure Init (R : in out Reader; Data : Byte_Array; Endian : Endianness) is
   begin
      R.Pos := 0;
      R.Endian := Endian;
      R.Max_Align := Xcdr2_Max_Align;
      R.Underflow := False;
      if Data'Length > Max_Buffer then
         R.Len := 0;
         R.Underflow := True;
         return;
      end if;
      R.Len := Data'Length;
      for I in 0 .. Data'Length - 1 loop
         R.Buf (I) := Data (Data'First + I);
      end loop;
   end Init;

   procedure Get_Bytes (R : in out Reader; Out_D : out Byte_Array) is
   begin
      Out_D := [others => 0];
      if R.Underflow then
         return;
      end if;
      if R.Pos + Out_D'Length > R.Len then
         R.Underflow := True;
         return;
      end if;
      for I in Out_D'Range loop
         Out_D (I) := R.Buf (R.Pos);
         R.Pos := R.Pos + 1;
      end loop;
   end Get_Bytes;

   procedure R_Align (R : in out Reader; Align : Positive) is
      Eff : constant Positive :=
        (if Align > R.Max_Align then R.Max_Align else Align);
      Pad : constant Natural := Padding_For (R.Pos, Eff);
   begin
      if R.Underflow then
         return;
      end if;
      if R.Pos + Pad > R.Len then
         R.Underflow := True;
         return;
      end if;
      R.Pos := R.Pos + Pad;
   end R_Align;

   procedure Get_Uint (R : in out Reader; Align : Positive; LE : out Byte_Array)
   is
   begin
      LE := [others => 0];
      R_Align (R, Align);
      Get_Bytes (R, LE);
      if R.Endian = Big then
         Reverse_In_Place (LE);
      end if;
   end Get_Uint;

   procedure Get_U8 (R : in out Reader; V : out Byte) is
      B : Byte_Array (0 .. 0);
   begin
      Get_Bytes (R, B);
      V := B (0);
   end Get_U8;

   procedure Get_U16 (R : in out Reader; V : out Unsigned_16) is
      LE : Byte_Array (0 .. 1);
   begin
      Get_Uint (R, 2, LE);
      V := Unsigned_16 (LE (0)) or Shift_Left (Unsigned_16 (LE (1)), 8);
   end Get_U16;

   procedure Get_U32 (R : in out Reader; V : out Unsigned_32) is
      LE : Byte_Array (0 .. 3);
   begin
      Get_Uint (R, 4, LE);
      V := Unsigned_32 (LE (0))
        or Shift_Left (Unsigned_32 (LE (1)), 8)
        or Shift_Left (Unsigned_32 (LE (2)), 16)
        or Shift_Left (Unsigned_32 (LE (3)), 24);
   end Get_U32;

   procedure Get_U64 (R : in out Reader; V : out Unsigned_64) is
      LE : Byte_Array (0 .. 7);
   begin
      Get_Uint (R, 8, LE);
      V := Unsigned_64 (LE (0))
        or Shift_Left (Unsigned_64 (LE (1)), 8)
        or Shift_Left (Unsigned_64 (LE (2)), 16)
        or Shift_Left (Unsigned_64 (LE (3)), 24)
        or Shift_Left (Unsigned_64 (LE (4)), 32)
        or Shift_Left (Unsigned_64 (LE (5)), 40)
        or Shift_Left (Unsigned_64 (LE (6)), 48)
        or Shift_Left (Unsigned_64 (LE (7)), 56);
   end Get_U64;

   procedure Get_Bool (R : in out Reader; V : out Boolean) is
      B : Byte;
   begin
      Get_U8 (R, B);
      V := B /= 0;
   end Get_Bool;

   procedure Get_F32 (R : in out Reader; V : out IEEE_Float_32) is
      U : Unsigned_32;
   begin
      Get_U32 (R, U);
      V := Bits_F32 (U);
   end Get_F32;

   procedure Get_F64 (R : in out Reader; V : out IEEE_Float_64) is
      U : Unsigned_64;
   begin
      Get_U64 (R, U);
      V := Bits_F64 (U);
   end Get_F64;

   procedure Get_String
     (R : in out Reader; S : out String; Last : out Natural)
   is
      Len : Unsigned_32;
   begin
      S := [others => ' '];
      Last := S'First - 1;
      Get_U32 (R, Len);
      if Len = 0 then
         R.Underflow := True;  -- length must include the NUL
         return;
      end if;
      declare
         Body_Len : constant Natural := Natural (Len) - 1;
         Raw      : Byte_Array (0 .. Natural (Len) - 1);
      begin
         if Body_Len > S'Length then
            R.Underflow := True;
            return;
         end if;
         Get_Bytes (R, Raw);
         if R.Underflow then
            return;
         end if;
         if Raw (Natural (Len) - 1) /= 0 then
            R.Underflow := True;  -- missing NUL terminator
            return;
         end if;
         for I in 0 .. Body_Len - 1 loop
            S (S'First + I) := Character'Val (Integer (Raw (I)));
         end loop;
         Last := S'First + Body_Len - 1;
      end;
   end Get_String;

   procedure Get_Seq_U8
     (R : in out Reader; Out_D : out Byte_Array; N : out Natural)
   is
      Len : Unsigned_32;
   begin
      Out_D := [others => 0];
      Get_U32 (R, Len);
      N := Natural (Len);
      if Natural (Len) > Out_D'Length then
         R.Underflow := True;
         return;
      end if;
      declare
         Raw : Byte_Array (0 .. (if Len = 0 then 0 else Natural (Len) - 1));
      begin
         if Len = 0 then
            return;
         end if;
         Get_Bytes (R, Raw);
         for I in 0 .. Natural (Len) - 1 loop
            Out_D (Out_D'First + I) := Raw (I);
         end loop;
      end;
   end Get_Seq_U8;

   procedure DHeader_Read (R : in out Reader; Len : out Unsigned_32) is
   begin
      Get_U32 (R, Len);
   end DHeader_Read;

   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Nextint         : out Unsigned_32)
   is
      Em : Unsigned_32;
   begin
      Get_U32 (R, Em);
      Must_Understand := (Em and 16#8000_0000#) /= 0;
      Member_Id := Em and 16#0FFF_FFFF#;
      Get_U32 (R, Nextint);
   end EMHeader_Read;

end Zerodds_Native_Wire;
