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
      W.Origin := 0;
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
      --  Measured from the origin, which is the stream everywhere except
      --  inside an open PL_CDR1 parameter. §7.4.1.2.
      Pad : Natural := Padding_For (W.Len - W.Origin, Eff);
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

   procedure Put_Char (W : in out Writer; Code_Point : Unsigned_32) is
   begin
      if Code_Point > 16#FF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Put_U8 (W, Byte (Code_Point));
   end Put_Char;

   procedure Put_Wchar (W : in out Writer; Code_Point : Unsigned_32) is
   begin
      if Code_Point > 16#FFFF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Put_U16 (W, Unsigned_16 (Code_Point));
   end Put_Wchar;

   procedure Put_Wstring (W : in out Writer; Units : Code_Unit_Array) is
   begin
      Put_U32 (W, Unsigned_32 (Units'Length) * 2);
      for Unit of Units loop
         Put_U16 (W, Unit);
      end loop;
   end Put_Wstring;

   procedure Put_Bitset
     (W : in out Writer; Bits : Unsigned_64; Anchor : Positive) is
   begin
      case Anchor is
         when 1 => Put_U8  (W, Byte (Bits and 16#FF#));
         when 2 => Put_U16 (W, Unsigned_16 (Bits and 16#FFFF#));
         when 4 => Put_U32 (W, Unsigned_32 (Bits and 16#FFFF_FFFF#));
         when 8 => Put_U64 (W, Bits);
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Put_Bitset;

   --  The nibbles Put_Fixed packs, apart from the writer so the packing
   --  can be read on its own. `Last` indexes the final nibble, and the
   --  count is always even because the sign shares the last octet with
   --  the least significant digit.
   procedure Fixed_Nibbles
     (Value       : String;
      Digit_Count : Positive;
      Scale       : Natural;
      Nibbles     : out Byte_Array;
      Count       : out Natural)
   is
      First     : Natural := Value'First;
      Negative  : Boolean := False;
      Point     : Natural := 0;
      Int_First : Natural;
      Int_Last  : Natural;
      Frc_First : Natural;
      Frc_Last  : Natural;
      Non_Zero  : Boolean := False;
      K         : Natural := Nibbles'First;

      procedure Emit (N : Byte) is
      begin
         Nibbles (K) := N;
         K := K + 1;
      end Emit;
   begin
      Nibbles := [others => 0];
      Count := 0;
      --  IDL 4.2 §7.4.1.4.5 bounds a fixed at 31 digits, and the scale
      --  cannot exceed them. Stated rather than left to the arithmetic:
      --  a Digit_Count of 40 would overrun `Nibbles` silently under
      --  -gnatp.
      if Digit_Count > 31 or else Scale > Digit_Count then
         raise Not_A_Fixed_Value;
      end if;
      if First <= Value'Last
        and then (Value (First) = '+' or else Value (First) = '-')
      then
         Negative := Value (First) = '-';
         First := First + 1;
      end if;
      for I in First .. Value'Last loop
         if Value (I) = '.' then
            if Point /= 0 then
               raise Not_A_Fixed_Value;  -- two points is not a decimal
            end if;
            Point := I;
         elsif Value (I) not in '0' .. '9' then
            raise Not_A_Fixed_Value;
         end if;
      end loop;
      if Point = 0 then
         Int_First := First;
         Int_Last  := Value'Last;
         Frc_First := 1;
         Frc_Last  := 0;
      else
         Int_First := First;
         Int_Last  := Point - 1;
         Frc_First := Point + 1;
         Frc_Last  := Value'Last;
      end if;
      if Int_First > Int_Last and then Frc_First > Frc_Last then
         raise Not_A_Fixed_Value;  -- no digits at all
      end if;
      --  A leading zero carries no value, so it is dropped before the
      --  width is checked: "000000.001" is one integer digit and not
      --  six, which is the form `fixed-basic` writes.
      while Int_First <= Int_Last and then Value (Int_First) = '0' loop
         Int_First := Int_First + 1;
      end loop;
      if Int_Last - Int_First + 1 > Digit_Count - Scale then
         raise Not_A_Fixed_Value;
      end if;
      --  A trailing zero in the fraction is padding either way.
      while Frc_Last >= Frc_First
        and then Frc_Last - Frc_First + 1 > Scale
        and then Value (Frc_Last) = '0'
      loop
         Frc_Last := Frc_Last - 1;
      end loop;
      if Frc_Last - Frc_First + 1 > Scale then
         raise Not_A_Fixed_Value;
      end if;

      if (Digit_Count + 1) mod 2 = 1 then
         Emit (0);  -- pair the digits with the sign nibble
      end if;
      for I in 1 .. Digit_Count - Scale - (Int_Last - Int_First + 1) loop
         Emit (0);
      end loop;
      for I in Int_First .. Int_Last loop
         Emit (Byte (Character'Pos (Value (I)) - Character'Pos ('0')));
      end loop;
      for I in Frc_First .. Frc_Last loop
         Emit (Byte (Character'Pos (Value (I)) - Character'Pos ('0')));
      end loop;
      for I in 1 .. Scale - (Frc_Last - Frc_First + 1) loop
         Emit (0);
      end loop;
      for I in Nibbles'First .. K - 1 loop
         if Nibbles (I) /= 0 then
            Non_Zero := True;
         end if;
      end loop;
      --  A negative zero goes out positive: there is one zero to write.
      Emit (if Negative and then Non_Zero then 16#D# else 16#C#);
      Count := K - Nibbles'First;
   end Fixed_Nibbles;

   procedure Put_Fixed
     (W           : in out Writer;
      Value       : String;
      Digit_Count : Positive;
      Scale       : Natural)
   is
      --  31 digits + a leading zero nibble + the sign nibble.
      Nibbles : Byte_Array (0 .. 32);
      Count   : Natural;
   begin
      Fixed_Nibbles (Value, Digit_Count, Scale, Nibbles, Count);
      declare
         Octets : Byte_Array (0 .. Count / 2 - 1);
      begin
         for I in Octets'Range loop
            Octets (I) :=
              Byte (Natural (Nibbles (2 * I)) * 16
                    + Natural (Nibbles (2 * I + 1)));
         end loop;
         --  Alignment is one and no multi-octet integer is written, so
         --  nothing here consults the stream's byte order.
         Put_Bytes (W, Octets);
      end;
   end Put_Fixed;

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

   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out Natural)
   is
      Em : Unsigned_32;
   begin
      --  §7.4.3.4.2 gives the id 28 bits. Truncating it would put a
      --  different member's id on the wire, so the bound is stated.
      if Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      Em := (if Must_Understand then 16#8000_0000# else 0)
        + Shift_Left (Unsigned_32 (Length_Code'Pos (Code)), 28)
        + Member_Id;
      Put_U32 (W, Em);
      if Code = LC4 then
         --  Only LC4 reserves a NEXTINT. LC0..LC3 take the length from
         --  the code, and LC5..LC7 from the body's own leading word.
         W_Align (W, 4);
         Put_Bytes (W, [0, 0, 0, 0]);
      end if;
      Body_Start := W.Len;
   end EMHeader_Begin;

   procedure EMHeader_End
     (W : in out Writer; Body_Start : Natural; Code : Length_Code)
   is
      Written : constant Natural := W.Len - Body_Start;
   begin
      if W.Overflow then
         return;
      end if;
      case Code is
         when LC4 =>
            DHeader_End (W, Body_Start);
         when LC0 | LC1 | LC2 | LC3 =>
            --  1, 2, 4, 8 -- the code *is* the length, so a body of any
            --  other size means the plan and the member disagree.
            if Written /= 2 ** Length_Code'Pos (Code) then
               raise Length_Code_Mismatch;
            end if;
         when LC5 | LC6 | LC7 =>
            if Written < 4 then
               raise Length_Code_Mismatch;
            end if;
      end case;
   end EMHeader_End;

   --  --- the PL_CDR1 parameter list ---

   --  Writes `V` over the two octets at `At_Pos`, in wire byte order.
   --  The one place besides DHeader_End that writes into an already
   --  written position.
   procedure Patch_U16 (W : in out Writer; At_Pos : Natural; V : Unsigned_16)
   is
      LE : Byte_Array (0 .. 1) :=
        [0 => Byte (V and 16#FF#),
         1 => Byte (Shift_Right (V, 8) and 16#FF#)];
   begin
      if W.Overflow then
         return;
      end if;
      if W.Endian = Big then
         Reverse_In_Place (LE);
      end if;
      W.Buf (At_Pos) := LE (0);
      W.Buf (At_Pos + 1) := LE (1);
   end Patch_U16;

   procedure Patch_U32 (W : in out Writer; At_Pos : Natural; V : Unsigned_32)
   is
      LE : Byte_Array (0 .. 3) :=
        [0 => Byte (V and 16#FF#),
         1 => Byte (Shift_Right (V, 8) and 16#FF#),
         2 => Byte (Shift_Right (V, 16) and 16#FF#),
         3 => Byte (Shift_Right (V, 24) and 16#FF#)];
   begin
      if W.Overflow then
         return;
      end if;
      if W.Endian = Big then
         Reverse_In_Place (LE);
      end if;
      for I in 0 .. 3 loop
         W.Buf (At_Pos + I) := LE (I);
      end loop;
   end Patch_U32;

   Pid_Flag_Must_Understand : constant Unsigned_16 := 16#4000#;
   --  Bit 30 of the eMemberHeader inside an extended parameter header --
   --  not bit 31, which is the implementation-extension flag.
   Em_Flag_Must_Understand  : constant Unsigned_32 := 16#4000_0000#;

   procedure Pid_Begin
     (W               : in out Writer;
      Pid             : Unsigned_16;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      Id : Unsigned_16 := Pid;
   begin
      Header := (At_Pos => 0, Extended => False, Saved_Origin => W.Origin);
      if Pid > Pid_Short_Max_Id then
         raise Member_Id_Out_Of_Range;
      end if;
      W_Align (W, 4);
      if Must_Understand then
         Id := Id or Pid_Flag_Must_Understand;
      end if;
      Put_U16 (W, Id);
      Header.At_Pos := W.Len;
      Put_Bytes (W, [0, 0]);
      W.Origin := W.Len;
   end Pid_Begin;

   procedure Pid_Extended_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      Em : Unsigned_32 := Member_Id;
   begin
      Header := (At_Pos => 0, Extended => True, Saved_Origin => W.Origin);
      if Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      W_Align (W, 4);
      --  The marker carries the must-understand flag under this framing
      --  whatever the member says; the member's own flag is in the
      --  eMemberHeader below.
      Put_U16 (W, Unsigned_16 (Pid_Extended_Id) or Pid_Flag_Must_Understand);
      Put_U16 (W, 8);
      if Must_Understand then
         Em := Em or Em_Flag_Must_Understand;
      end if;
      Put_U32 (W, Em);
      Header.At_Pos := W.Len;
      Put_Bytes (W, [0, 0, 0, 0]);
      W.Origin := W.Len;
   end Pid_Extended_Begin;

   procedure Pid_End (W : in out Writer; Header : Pid_Header) is
      Width  : constant Natural := (if Header.Extended then 4 else 2);
      Length : Natural := W.Len - Header.At_Pos - Width;
      Past   : Natural;
   begin
      if W.Overflow then
         return;
      end if;
      --  The declared length counts the padding that brings the next
      --  header back to a four-octet boundary, measured from the block
      --  the length describes.
      Past := (W.Len - Header.Saved_Origin) mod 4;
      if Past /= 0 then
         Length := Length + (4 - Past);
      end if;
      if Header.Extended then
         Patch_U32 (W, Header.At_Pos, Unsigned_32 (Length));
      else
         if Length > 16#FFFF# then
            raise Parameter_Too_Long;
         end if;
         Patch_U16 (W, Header.At_Pos, Unsigned_16 (Length));
      end if;
      W.Origin := Header.Saved_Origin;
      W_Align (W, 4);
   end Pid_End;

   procedure Pid_List_End (W : in out Writer) is
   begin
      W_Align (W, 4);
      Put_U16 (W, Unsigned_16 (Pid_List_End_Id) or Pid_Flag_Must_Understand);
      Put_U16 (W, 0);
   end Pid_List_End;

   --  --- reader ---

   procedure Init (R : in out Reader; Data : Byte_Array; Endian : Endianness) is
   begin
      R.Pos := 0;
      R.Endian := Endian;
      R.Max_Align := Xcdr2_Max_Align;
      R.Underflow := False;
      R.Origin := 0;
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
      Pad : constant Natural := Padding_For (R.Pos - R.Origin, Eff);
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

   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out Unsigned_32)
   is
      Em   : Unsigned_32;
      Word : Unsigned_32;
      Mark : Natural;
   begin
      Get_U32 (R, Em);
      Must_Understand := (Em and 16#8000_0000#) /= 0;
      Member_Id := Em and 16#0FFF_FFFF#;
      Code := Length_Code'Val (Natural (Shift_Right (Em, 28) and 7));
      case Code is
         when LC0 | LC1 | LC2 | LC3 =>
            Extent := Unsigned_32 (2 ** Length_Code'Pos (Code));
         when LC4 =>
            Get_U32 (R, Extent);
         when LC5 | LC6 | LC7 =>
            --  The word belongs to the body, so it is read and the
            --  cursor put back where it was.
            Mark := R.Pos;
            Get_U32 (R, Word);
            R.Pos := Mark;
            Extent :=
              (case Code is
                 when LC5 => 4 + Word,
                 when LC6 => 4 + 4 * Word,
                 when others => 4 + 8 * Word);
      end case;
   end EMHeader_Read;

   procedure Get_Char (R : in out Reader; Code_Point : out Unsigned_32) is
      B : Byte;
   begin
      Get_U8 (R, B);
      Code_Point := Unsigned_32 (B);
   end Get_Char;

   procedure Get_Wchar (R : in out Reader; Code_Point : out Unsigned_32) is
      U : Unsigned_16;
   begin
      Get_U16 (R, U);
      Code_Point := Unsigned_32 (U);
   end Get_Wchar;

   procedure Get_Wstring
     (R : in out Reader; Units : out Code_Unit_Array; N : out Natural)
   is
      Octets : Unsigned_32;
   begin
      Units := [others => 0];
      Get_U32 (R, Octets);
      N := Natural (Octets) / 2;
      if N > Units'Length then
         R.Underflow := True;
         N := 0;
         return;
      end if;
      for I in 0 .. N - 1 loop
         Get_U16 (R, Units (Units'First + I));
      end loop;
   end Get_Wstring;

   procedure Get_Bitset
     (R : in out Reader; Anchor : Positive; Bits : out Unsigned_64)
   is
      B8  : Byte;
      B16 : Unsigned_16;
      B32 : Unsigned_32;
   begin
      Bits := 0;
      case Anchor is
         when 1 => Get_U8  (R, B8);  Bits := Unsigned_64 (B8);
         when 2 => Get_U16 (R, B16); Bits := Unsigned_64 (B16);
         when 4 => Get_U32 (R, B32); Bits := Unsigned_64 (B32);
         when 8 => Get_U64 (R, Bits);
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Get_Bitset;

   procedure Get_Fixed
     (R           : in out Reader;
      Digit_Count : Positive;
      Scale       : Natural;
      Value       : out String;
      Last        : out Natural)
   is
      Octets  : constant Natural := (Digit_Count + 2) / 2;
      Raw     : Byte_Array (0 .. Octets - 1);
      Nibbles : Byte_Array (0 .. 2 * Octets - 1);
      Sign    : Byte;
      K       : Natural;
   begin
      Value := [others => ' '];
      Last := Value'First - 1;
      if Digit_Count > 31 or else Scale > Digit_Count then
         raise Not_A_Fixed_Value;
      end if;
      --  A sign, Digit_Count digits and a point: the widest a decimal
      --  read back from P nibbles can be.
      if Value'Length < Digit_Count + 2 then
         R.Underflow := True;
         return;
      end if;
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      for I in Raw'Range loop
         Nibbles (2 * I) := Raw (I) / 16;
         Nibbles (2 * I + 1) := Raw (I) mod 16;
      end loop;
      Sign := Nibbles (Nibbles'Last);
      K := Value'First;
      if Sign = 16#D# then
         Value (K) := '-';
         K := K + 1;
      end if;
      for I in 0 .. Digit_Count - 1 loop
         if Scale /= 0 and then I = Digit_Count - Scale then
            Value (K) := '.';
            K := K + 1;
         end if;
         Value (K) :=
           Character'Val
             (Character'Pos ('0')
              + Natural (Nibbles (Nibbles'Last - Digit_Count + I)));
         K := K + 1;
      end loop;
      Last := K - 1;
   end Get_Fixed;

   procedure Pid_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Length          : out Unsigned_32;
      List_End        : out Boolean)
   is
      Raw   : Unsigned_16;
      Short : Unsigned_16;
      Slen  : Unsigned_16;
      Em    : Unsigned_32;
      Len16 : Unsigned_16;
   begin
      Member_Id := 0;
      Must_Understand := False;
      Length := 0;
      List_End := False;
      Get_U16 (R, Raw);
      --  Bits 14 and 15 are the flags, so the id is the low fourteen.
      Short := Raw and 16#3FFF#;
      if Short = Unsigned_16 (Pid_List_End_Id) then
         Get_U16 (R, Slen);
         List_End := True;
         return;
      end if;
      if Short = Unsigned_16 (Pid_Extended_Id) then
         Get_U16 (R, Slen);
         --  §7.4.1.2 fixes the marker's own length at 8. A marker that
         --  says anything else is not this framing, and reading on would
         --  take the next member's octets as an eMemberHeader.
         if Slen /= 8 then
            R.Underflow := True;
            return;
         end if;
         Get_U32 (R, Em);
         Get_U32 (R, Length);
         Member_Id := Em and 16#0FFF_FFFF#;
         Must_Understand := (Em and Em_Flag_Must_Understand) /= 0;
         R.Origin := R.Pos;
         return;
      end if;
      Get_U16 (R, Len16);
      Member_Id := Unsigned_32 (Short);
      Must_Understand := (Raw and Pid_Flag_Must_Understand) /= 0;
      Length := Unsigned_32 (Len16);
      R.Origin := R.Pos;
   end Pid_Read;

   procedure Pid_Skip
     (R : in out Reader; Body_Start : Natural; Length : Unsigned_32) is
   begin
      R.Pos := Body_Start + Natural (Length);
      R.Origin := 0;
      R.Pos := R.Pos + Padding_For (R.Pos, 4);
   end Pid_Skip;

end Zerodds_Native_Wire;
