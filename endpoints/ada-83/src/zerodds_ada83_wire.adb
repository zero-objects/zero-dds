-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Unchecked_Conversion;

package body Zerodds_Ada83_Wire is

   type Bytes8 is array (0 .. 7) of Byte;
   pragma Pack (Bytes8);
   type Bytes4 is array (0 .. 3) of Byte;
   pragma Pack (Bytes4);
   type Bytes2 is array (0 .. 1) of Byte;
   pragma Pack (Bytes2);

   function F32_To_Bytes is new Unchecked_Conversion (Float, Bytes4);
   function F64_To_Bytes is new Unchecked_Conversion (Long_Float, Bytes8);
   function Bytes_To_F32 is new Unchecked_Conversion (Bytes4, Float);
   function Bytes_To_F64 is new Unchecked_Conversion (Bytes8, Long_Float);
   function Probe_To_Bytes is new Unchecked_Conversion (Short_Integer, Bytes2);

   --  True on a big-endian host (first storage byte of a small positive value
   --  is the most significant one).
   function Host_Is_Big return Boolean is
      B : constant Bytes2 := Probe_To_Bytes (1);
   begin
      return B (0) = 0;
   end Host_Is_Big;

   function Padding_For (Pos : Integer; Align : Integer) return Integer is
   begin
      return (Align - (Pos mod Align)) mod Align;
   end Padding_For;

   procedure Init (W : in out Writer; Endian : Endianness) is
   begin
      W.Len := 0;
      W.Endian := Endian;
      W.Overflow := False;
      W.Max_Align := Xcdr2_Max_Align;
      W.Origin := 0;
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
      if Eff > W.Max_Align then
         Eff := W.Max_Align;  -- XCDR2 caps at 4, XCDR1 at 8
      end if;
      --  Measured from the origin: inside a PL_CDR1 parameter that is the
      --  parameter's own block and not the stream (section 7.4.1.2).
      Pad := Padding_For (W.Len - W.Origin, Eff);
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
      --  Ada's "mod" floors, so this is the low octet of the two's
      --  complement pattern for a negative V as well as for a positive
      --  one.
      return Byte (V mod 256);
   end Le1;

   --  V shifted right by eight, flooring.
   --
   --  Ada's "/" truncates toward zero, so `V / 256` for a negative V is
   --  one *higher* than the two's complement decomposition needs, and
   --  every octet above the lowest came out one too high: `Put_U16` on a
   --  `short` of -12345 wrote `c7 d0` where the pattern is 0xCFC7 and
   --  every other endpoint SDK writes `c7 cf`, and `Put_U32` on a `long`
   --  of -1234567 wrote `79 2a ee 00` for 0xFFED2979. Taking the
   --  remainder out first makes the division exact, so the result is the
   --  floor for either sign.
   --
   --  Nothing in the corpus reaches it: `wireprobe-endpoint-gen` hands
   --  every binding the unsigned pattern, and for a non-negative V the
   --  two forms are the same number. A caller of this binding writing
   --  the value a `short` holds reaches it immediately, which is what
   --  `tools/wireprobe/endpoint-number-surface` measures.
   function Shr8 (V : Long_Integer) return Long_Integer is
   begin
      return (V - (V mod 256)) / 256;
   end Shr8;

   --  The extent each width holds, checked before any octet is written.
   --  Ada 83 has no unsigned type, so a Long_Integer carries the value
   --  and nothing but this stops 256 from reaching the wire as the zero
   --  `Le1` would make of it. A negative value is admitted down to the
   --  signed floor of the width: the corpus records `negative-u16` as
   --  `c7cf` and `negative-u32` as `7929edff`, the two's complement
   --  patterns, so refusing those would refuse a value the type holds.
   procedure Check_Width (V : Long_Integer; Low, High : Long_Integer) is
   begin
      if V < Low or else V > High then
         raise Value_Out_Of_Range;
      end if;
   end Check_Width;

   procedure Put_U8 (W : in out Writer; V : Long_Integer) is
      B : Byte_Array (0 .. 0);
   begin
      Check_Width (V, -128, 255);
      B (0) := Le1 (V);
      Put_Bytes (W, B);  -- 1-byte, no alignment
   end Put_U8;

   procedure Put_U16 (W : in out Writer; V : Long_Integer) is
      LE : Byte_Array (0 .. 1);
   begin
      Check_Width (V, -32768, 65535);
      LE (0) := Le1 (V);
      LE (1) := Le1 (Shr8 (V));
      Put_LE (W, 2, LE);
   end Put_U16;

   procedure Put_U32 (W : in out Writer; V : Long_Integer) is
      LE : Byte_Array (0 .. 3);
   begin
      Check_Width (V, -2_147_483_648, 4_294_967_295);
      LE (0) := Le1 (V);
      LE (1) := Le1 (Shr8 (V));
      LE (2) := Le1 (Shr8 (Shr8 (V)));
      LE (3) := Le1 (Shr8 (Shr8 (Shr8 (V))));
      Put_LE (W, 4, LE);
   end Put_U32;

   procedure Put_U64 (W : in out Writer; Hi, Lo : Long_Integer) is
      LE : Byte_Array (0 .. 7);
   begin
      LE (0) := Le1 (Lo);
      LE (1) := Le1 (Shr8 (Lo));
      LE (2) := Le1 (Shr8 (Shr8 (Lo)));
      LE (3) := Le1 (Shr8 (Shr8 (Shr8 (Lo))));
      LE (4) := Le1 (Hi);
      LE (5) := Le1 (Shr8 (Hi));
      LE (6) := Le1 (Shr8 (Shr8 (Hi)));
      LE (7) := Le1 (Shr8 (Shr8 (Shr8 (Hi))));
      --  Eight, capped by Max_Align: four under XCDR2 and eight under
      --  XCDR1 (section 7.4.1). Passing 4 here was the second half of the
      --  cap being hard-wired -- a settable cap alone would not have
      --  moved this write.
      Put_LE (W, 8, LE);
   end Put_U64;

   procedure Put_Bool (W : in out Writer; V : Boolean) is
   begin
      if V then
         Put_U8 (W, 1);
      else
         Put_U8 (W, 0);
      end if;
   end Put_Bool;

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

   procedure Put_F64 (W : in out Writer; V : Long_Float) is
      Host : constant Bytes8 := F64_To_Bytes (V);
      LE   : Byte_Array (0 .. 7);
   begin
      if Host_Is_Big then
         for I in 0 .. 7 loop
            LE (I) := Host (7 - I);
         end loop;
      else
         for I in 0 .. 7 loop
            LE (I) := Host (I);
         end loop;
      end if;
      Put_LE (W, 8, LE);
   end Put_F64;

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

   procedure Put_Char (W : in out Writer; Code_Point : Long_Integer) is
   begin
      if Code_Point < 0 or else Code_Point > 16#FF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Put_U8 (W, Code_Point);
   end Put_Char;

   procedure Put_Wchar (W : in out Writer; Code_Point : Long_Integer) is
   begin
      if Code_Point < 0 or else Code_Point > 16#FFFF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Put_U16 (W, Code_Point);
   end Put_Wchar;

   procedure Put_Wstring (W : in out Writer; Units : Code_Unit_Array) is
   begin
      --  The length counts octets, not units, and no terminator follows.
      Put_U32 (W, Long_Integer (Units'Length) * 2);
      for I in Units'Range loop
         Put_U16 (W, Long_Integer (Units (I)));
      end loop;
   end Put_Wstring;

   procedure Put_Bitset
     (W : in out Writer; Hi, Lo : Long_Integer; Anchor : Integer) is
   begin
      if Anchor = 1 then
         Put_U8 (W, Lo);
      elsif Anchor = 2 then
         Put_U16 (W, Lo);
      elsif Anchor = 4 then
         Put_U32 (W, Lo);
      elsif Anchor = 8 then
         Put_U64 (W, Hi, Lo);
      else
         raise Anchor_Not_A_Holder_Width;
      end if;
   end Put_Bitset;

   --  The nibbles Put_Fixed packs. `Count` is how many were written, and
   --  it is always even because the sign nibble shares the last octet
   --  with the least significant digit.
   procedure Fixed_Nibbles
     (Value       : String;
      Digit_Count : Integer;
      Scale       : Integer;
      Nibbles     : out Byte_Array;
      Count       : out Integer)
   is
      First     : Integer := Value'First;
      Negative  : Boolean := False;
      Point     : Integer := 0;
      Int_First : Integer;
      Int_Last  : Integer;
      Frc_First : Integer;
      Frc_Last  : Integer;
      Non_Zero  : Boolean := False;
      K         : Integer := Nibbles'First;

      procedure Emit (N : Byte) is
      begin
         Nibbles (K) := N;
         K := K + 1;
      end Emit;
   begin
      for I in Nibbles'Range loop
         Nibbles (I) := 0;
      end loop;
      Count := 0;
      --  IDL 4.2 section 7.4.1.4.5 bounds a fixed at 31 digits, and the
      --  scale cannot exceed them. Stated rather than left to the
      --  arithmetic: a Digit_Count of 40 would overrun `Nibbles`
      --  silently under -gnatp.
      if Digit_Count < 1 or else Digit_Count > 31
        or else Scale < 0 or else Scale > Digit_Count
      then
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
         elsif Value (I) < '0' or else Value (I) > '9' then
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
      --  width is checked: "000000.001" is one integer digit and not six.
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
      if Negative and then Non_Zero then
         Emit (16#D#);
      else
         Emit (16#C#);
      end if;
      Count := K - Nibbles'First;
   end Fixed_Nibbles;

   procedure Put_Fixed
     (W           : in out Writer;
      Value       : String;
      Digit_Count : Integer;
      Scale       : Integer)
   is
      Nibbles : Byte_Array (0 .. 32);  -- 31 digits + a pad + the sign
      Count   : Integer;
   begin
      Fixed_Nibbles (Value, Digit_Count, Scale, Nibbles, Count);
      declare
         Octets : Byte_Array (0 .. Count / 2 - 1);
      begin
         for I in Octets'Range loop
            Octets (I) := Nibbles (2 * I) * 16 + Nibbles (2 * I + 1);
         end loop;
         Put_Bytes (W, Octets);
      end;
   end Put_Fixed;

   --  --- back-patched framing ---

   procedure Patch (W : in out Writer; At_Pos : Integer; V : Long_Integer;
                    Width : Integer)
   is
      LE : Byte_Array (0 .. Width - 1);
      X  : Long_Integer := V;
   begin
      if W.Overflow then
         return;
      end if;
      for I in LE'Range loop
         LE (I) := Le1 (X);
         X := Shr8 (X);
      end loop;
      for I in 0 .. Width - 1 loop
         if W.Endian = Big then
            W.Buf (At_Pos + I) := LE (Width - 1 - I);
         else
            W.Buf (At_Pos + I) := LE (I);
         end if;
      end loop;
   end Patch;

   procedure DHeader_Begin (W : in out Writer; Body_Start : out Integer) is
      Slot : constant Byte_Array (0 .. 3) := (0, 0, 0, 0);
   begin
      W_Align (W, 4);
      Put_Bytes (W, Slot);
      Body_Start := W.Len;  -- the DHEADER is the four octets before this
   end DHeader_Begin;

   procedure DHeader_End (W : in out Writer; Body_Start : Integer) is
   begin
      --  Section 7.4.3.5 measures the content that follows the DHEADER,
      --  so the four octets it occupies are not in it.
      Patch (W, Body_Start - 4, Long_Integer (W.Len - Body_Start), 4);
   end DHeader_End;

   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Long_Integer;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out Integer)
   is
      LE   : Byte_Array (0 .. 3);
      Top  : Long_Integer;
      Slot : constant Byte_Array (0 .. 3) := (0, 0, 0, 0);
   begin
      Body_Start := W.Len;
      if Member_Id < 0 or else Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      --  Assembled octet by octet rather than as one number: Ada 83 has
      --  no modular type, and 2**31 is past Long_Integer'Last on a
      --  toolchain whose Long_Integer is 32 bits.
      LE (0) := Le1 (Member_Id);
      LE (1) := Le1 (Shr8 (Member_Id));
      LE (2) := Le1 (Shr8 (Shr8 (Member_Id)));
      Top := (Member_Id / 16#100_0000#) mod 16
        + Long_Integer (Length_Code'Pos (Code)) * 16;
      if Must_Understand then
         Top := Top + 128;  -- bit 31
      end if;
      LE (3) := Byte (Top);
      Put_LE (W, 4, LE);
      if Code = LC4 then
         --  Only LC4 reserves a NEXTINT. LC0..LC3 take the length from
         --  the code, and LC5..LC7 from the body's own leading word --
         --  writing it twice would put every following member four
         --  octets late.
         W_Align (W, 4);
         Put_Bytes (W, Slot);
      end if;
      Body_Start := W.Len;
   end EMHeader_Begin;

   procedure EMHeader_End
     (W : in out Writer; Body_Start : Integer; Code : Length_Code)
   is
      Written : constant Integer := W.Len - Body_Start;
   begin
      if W.Overflow then
         return;
      end if;
      if Code = LC4 then
         Patch (W, Body_Start - 4, Long_Integer (Written), 4);
      elsif Code = LC0 or else Code = LC1
        or else Code = LC2 or else Code = LC3
      then
         --  1, 2, 4, 8 -- the code *is* the length.
         if Written /= 2 ** Length_Code'Pos (Code) then
            raise Length_Code_Mismatch;
         end if;
      else
         if Written < 4 then
            raise Length_Code_Mismatch;
         end if;
      end if;
   end EMHeader_End;

   procedure Pid_Begin
     (W               : in out Writer;
      Pid             : Long_Integer;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      Id   : Long_Integer := Pid;
      Slot : constant Byte_Array (0 .. 1) := (0, 0);
   begin
      Header.At_Pos := 0;
      Header.Extended := False;
      Header.Saved_Origin := W.Origin;
      if Pid < 0 or else Pid > Pid_Short_Max_Id then
         raise Member_Id_Out_Of_Range;
      end if;
      W_Align (W, 4);
      if Must_Understand then
         Id := Id + 16#4000#;  -- bit 14; bit 15 is the impl-extension flag
      end if;
      Put_U16 (W, Id);
      Header.At_Pos := W.Len;
      Put_Bytes (W, Slot);
      W.Origin := W.Len;
   end Pid_Begin;

   procedure Pid_Extended_Begin
     (W               : in out Writer;
      Member_Id       : Long_Integer;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      LE   : Byte_Array (0 .. 3);
      Top  : Long_Integer;
      Slot : constant Byte_Array (0 .. 3) := (0, 0, 0, 0);
   begin
      Header.At_Pos := 0;
      Header.Extended := True;
      Header.Saved_Origin := W.Origin;
      if Member_Id < 0 or else Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      W_Align (W, 4);
      --  The marker carries the must-understand flag under this framing
      --  whatever the member says.
      Put_U16 (W, Pid_Extended_Id + 16#4000#);
      Put_U16 (W, 8);
      LE (0) := Le1 (Member_Id);
      LE (1) := Le1 (Shr8 (Member_Id));
      LE (2) := Le1 (Shr8 (Shr8 (Member_Id)));
      Top := (Member_Id / 16#100_0000#) mod 16;
      if Must_Understand then
         Top := Top + 64;  -- bit 30, not bit 31
      end if;
      LE (3) := Byte (Top);
      Put_LE (W, 4, LE);
      Header.At_Pos := W.Len;
      Put_Bytes (W, Slot);
      W.Origin := W.Len;
   end Pid_Extended_Begin;

   procedure Pid_End (W : in out Writer; Header : Pid_Header) is
      Width  : Integer := 2;
      Length : Integer;
      Past   : Integer;
   begin
      if W.Overflow then
         return;
      end if;
      if Header.Extended then
         Width := 4;
      end if;
      Length := W.Len - Header.At_Pos - Width;
      --  The declared length counts the padding that brings the next
      --  header back to a four-octet boundary, measured from the block
      --  the length describes.
      Past := (W.Len - Header.Saved_Origin) mod 4;
      if Past /= 0 then
         Length := Length + (4 - Past);
      end if;
      if Header.Extended then
         Patch (W, Header.At_Pos, Long_Integer (Length), 4);
      else
         if Length > 16#FFFF# then
            raise Parameter_Too_Long;
         end if;
         Patch (W, Header.At_Pos, Long_Integer (Length), 2);
      end if;
      W.Origin := Header.Saved_Origin;
      W_Align (W, 4);
   end Pid_End;

   procedure Pid_List_End (W : in out Writer) is
   begin
      W_Align (W, 4);
      Put_U16 (W, Pid_List_End_Id + 16#4000#);
      Put_U16 (W, 0);
   end Pid_List_End;

   --  --- reader ---

   procedure Init_Reader (R : out Reader; Data : Byte_Array; Endian : Endianness) is
      K : Integer := 0;
   begin
      R.Len := Data'Length;
      R.Pos := 0;
      R.Endian := Endian;
      R.Max_Align := Xcdr2_Max_Align;
      R.Origin := 0;
      R.Underflow := False;
      for I in Data'Range loop
         R.Data (K) := Data (I);
         K := K + 1;
      end loop;
   end Init_Reader;

   procedure R_Align (R : in out Reader; Align : Integer) is
      Eff : Integer := Align;
      Pad : Integer;
   begin
      if Eff > R.Max_Align then
         Eff := R.Max_Align;
      end if;
      Pad := Padding_For (R.Pos - R.Origin, Eff);
      if R.Underflow then
         return;
      end if;
      if R.Pos + Pad > R.Len then
         R.Underflow := True;
         return;
      end if;
      R.Pos := R.Pos + Pad;
   end R_Align;

   procedure Get_Bytes (R : in out Reader; Out_D : out Byte_Array) is
   begin
      for I in Out_D'Range loop
         Out_D (I) := 0;
      end loop;
      if R.Underflow then
         return;
      end if;
      if R.Pos + Out_D'Length > R.Len then
         R.Underflow := True;
         return;
      end if;
      for I in Out_D'Range loop
         Out_D (I) := R.Data (R.Pos);
         R.Pos := R.Pos + 1;
      end loop;
   end Get_Bytes;

   --  `Width` octets at `Align`, back as a non-negative Long_Integer built
   --  from the logical little-endian order.
   procedure Get_Uint
     (R : in out Reader; Align : Integer; Width : Integer; V : out Long_Integer)
   is
      Raw : Byte_Array (0 .. Width - 1);
      --  Accumulated in a local: RM83 6.2 forbids reading an out
      --  parameter, so the running total cannot be `V` itself.
      Acc : Long_Integer := 0;
   begin
      V := 0;
      R_Align (R, Align);
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      if R.Endian = Big then
         for I in 0 .. Width - 1 loop
            Acc := Acc * 256 + Long_Integer (Raw (I));
         end loop;
      else
         for I in reverse 0 .. Width - 1 loop
            Acc := Acc * 256 + Long_Integer (Raw (I));
         end loop;
      end if;
      V := Acc;
   end Get_Uint;

   procedure Get_U8 (R : in out Reader; V : out Long_Integer) is
      Raw : Byte_Array (0 .. 0);
   begin
      Get_Bytes (R, Raw);
      V := Long_Integer (Raw (0));
   end Get_U8;

   procedure Get_U16 (R : in out Reader; V : out Long_Integer) is
   begin
      Get_Uint (R, 2, 2, V);
   end Get_U16;

   procedure Get_U32 (R : in out Reader; V : out Long_Integer) is
   begin
      Get_Uint (R, 4, 4, V);
   end Get_U32;

   procedure Get_U64 (R : in out Reader; Hi, Lo : out Long_Integer) is
      Raw   : Byte_Array (0 .. 7);
      LE    : Byte_Array (0 .. 7);
      Hi_Ac : Long_Integer := 0;
      Lo_Ac : Long_Integer := 0;
   begin
      Hi := 0;
      Lo := 0;
      R_Align (R, 8);
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      if R.Endian = Big then
         for I in 0 .. 7 loop
            LE (I) := Raw (7 - I);
         end loop;
      else
         for I in 0 .. 7 loop
            LE (I) := Raw (I);
         end loop;
      end if;
      for I in reverse 0 .. 3 loop
         Lo_Ac := Lo_Ac * 256 + Long_Integer (LE (I));
      end loop;
      for I in reverse 4 .. 7 loop
         Hi_Ac := Hi_Ac * 256 + Long_Integer (LE (I));
      end loop;
      Lo := Lo_Ac;
      Hi := Hi_Ac;
   end Get_U64;

   procedure Get_Bool (R : in out Reader; V : out Boolean) is
      X : Long_Integer;
   begin
      Get_U8 (R, X);
      V := X /= 0;
   end Get_Bool;

   procedure Get_F32 (R : in out Reader; V : out Float) is
      Raw  : Byte_Array (0 .. 3);
      LE   : Bytes4;
      Host : Bytes4;
   begin
      V := 0.0;
      R_Align (R, 4);
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      if R.Endian = Big then
         for I in 0 .. 3 loop
            LE (I) := Raw (3 - I);
         end loop;
      else
         for I in 0 .. 3 loop
            LE (I) := Raw (I);
         end loop;
      end if;
      if Host_Is_Big then
         for I in 0 .. 3 loop
            Host (I) := LE (3 - I);
         end loop;
      else
         Host := LE;
      end if;
      V := Bytes_To_F32 (Host);
   end Get_F32;

   procedure Get_F64 (R : in out Reader; V : out Long_Float) is
      Raw  : Byte_Array (0 .. 7);
      LE   : Bytes8;
      Host : Bytes8;
   begin
      V := 0.0;
      R_Align (R, 8);
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      if R.Endian = Big then
         for I in 0 .. 7 loop
            LE (I) := Raw (7 - I);
         end loop;
      else
         for I in 0 .. 7 loop
            LE (I) := Raw (I);
         end loop;
      end if;
      if Host_Is_Big then
         for I in 0 .. 7 loop
            Host (I) := LE (7 - I);
         end loop;
      else
         Host := LE;
      end if;
      V := Bytes_To_F64 (Host);
   end Get_F64;

   procedure Get_String (R : in out Reader; S : out String; Last : out Integer)
   is
      Len : Long_Integer;
   begin
      for I in S'Range loop
         S (I) := ' ';
      end loop;
      Last := S'First - 1;
      Get_U32 (R, Len);
      if Len = 0 then
         R.Underflow := True;  -- the length must include the NUL
         return;
      end if;
      declare
         Body_Len : constant Integer := Integer (Len) - 1;
         Raw      : Byte_Array (0 .. Integer (Len) - 1);
      begin
         if Body_Len > S'Length then
            R.Underflow := True;
            return;
         end if;
         Get_Bytes (R, Raw);
         if R.Underflow then
            return;
         end if;
         if Raw (Integer (Len) - 1) /= 0 then
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
     (R : in out Reader; Out_D : out Byte_Array; N : out Integer)
   is
      Len   : Long_Integer;
      Count : Integer;
   begin
      for I in Out_D'Range loop
         Out_D (I) := 0;
      end loop;
      Get_U32 (R, Len);
      Count := Integer (Len);
      N := Count;
      if Count > Out_D'Length then
         R.Underflow := True;
         N := 0;
         return;
      end if;
      if Count = 0 then
         return;
      end if;
      declare
         Raw : Byte_Array (0 .. Count - 1);
      begin
         Get_Bytes (R, Raw);
         for I in 0 .. Count - 1 loop
            Out_D (Out_D'First + I) := Raw (I);
         end loop;
      end;
   end Get_Seq_U8;

   procedure Get_Char (R : in out Reader; Code_Point : out Long_Integer) is
   begin
      Get_U8 (R, Code_Point);
   end Get_Char;

   procedure Get_Wchar (R : in out Reader; Code_Point : out Long_Integer) is
   begin
      Get_U16 (R, Code_Point);
   end Get_Wchar;

   procedure Get_Wstring
     (R : in out Reader; Units : out Code_Unit_Array; N : out Integer)
   is
      Octets : Long_Integer;
      Unit   : Long_Integer;
      Count  : Integer;
   begin
      for I in Units'Range loop
         Units (I) := 0;
      end loop;
      Get_U32 (R, Octets);
      Count := Integer (Octets) / 2;
      N := Count;
      if Count > Units'Length then
         R.Underflow := True;
         N := 0;
         return;
      end if;
      for I in 0 .. Count - 1 loop
         Get_U16 (R, Unit);
         Units (Units'First + I) := Code_Unit (Unit);
      end loop;
   end Get_Wstring;

   procedure Get_Bitset
     (R : in out Reader; Anchor : Integer; Hi, Lo : out Long_Integer) is
   begin
      Hi := 0;
      Lo := 0;
      if Anchor = 1 then
         Get_U8 (R, Lo);
      elsif Anchor = 2 then
         Get_U16 (R, Lo);
      elsif Anchor = 4 then
         Get_U32 (R, Lo);
      elsif Anchor = 8 then
         Get_U64 (R, Hi, Lo);
      else
         raise Anchor_Not_A_Holder_Width;
      end if;
   end Get_Bitset;

   procedure Get_Fixed
     (R           : in out Reader;
      Digit_Count : Integer;
      Scale       : Integer;
      Value       : out String;
      Last        : out Integer)
   is
      Octets  : constant Integer := (Digit_Count + 2) / 2;
      Raw     : Byte_Array (0 .. Octets - 1);
      Nibbles : Byte_Array (0 .. 2 * Octets - 1);
      Sign    : Byte;
      K       : Integer;
   begin
      for I in Value'Range loop
         Value (I) := ' ';
      end loop;
      Last := Value'First - 1;
      if Digit_Count < 1 or else Digit_Count > 31
        or else Scale < 0 or else Scale > Digit_Count
      then
         raise Not_A_Fixed_Value;
      end if;
      --  A sign, Digit_Count digits and a point.
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
         Value (K) := Character'Val
           (Character'Pos ('0')
            + Integer (Nibbles (Nibbles'Last - Digit_Count + I)));
         K := K + 1;
      end loop;
      Last := K - 1;
   end Get_Fixed;

   procedure DHeader_Read (R : in out Reader; Len : out Long_Integer) is
   begin
      Get_U32 (R, Len);
   end DHeader_Read;

   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Long_Integer;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out Long_Integer)
   is
      Raw  : Byte_Array (0 .. 3);
      Top  : Integer;
      Word : Long_Integer;
      Mark : Integer;
      LC   : Length_Code := LC0;
   begin
      Member_Id := 0;
      Must_Understand := False;
      Code := LC0;
      Extent := 0;
      R_Align (R, 4);
      Get_Bytes (R, Raw);
      if R.Underflow then
         return;
      end if;
      if R.Endian = Big then
         declare
            Swap : Byte;
         begin
            Swap := Raw (0); Raw (0) := Raw (3); Raw (3) := Swap;
            Swap := Raw (1); Raw (1) := Raw (2); Raw (2) := Swap;
         end;
      end if;
      Top := Integer (Raw (3));
      Must_Understand := Top >= 128;
      LC := Length_Code'Val ((Top / 16) mod 8);
      Code := LC;
      Member_Id := Long_Integer (Raw (0))
        + Long_Integer (Raw (1)) * 256
        + Long_Integer (Raw (2)) * 65536
        + Long_Integer (Top mod 16) * 16#100_0000#;
      if LC = LC0 or else LC = LC1 or else LC = LC2 or else LC = LC3 then
         Extent := 2 ** Length_Code'Pos (LC);
      elsif LC = LC4 then
         Get_U32 (R, Extent);
      else
         --  The word belongs to the body, so it is read and the cursor
         --  put back where it was.
         Mark := R.Pos;
         Get_U32 (R, Word);
         R.Pos := Mark;
         if LC = LC5 then
            Extent := 4 + Word;
         elsif LC = LC6 then
            --  LC6 counts four-octet elements and LC7 eight-octet ones;
            --  neither counts octets.
            Extent := 4 + 4 * Word;
         else
            Extent := 4 + 8 * Word;
         end if;
      end if;
   end EMHeader_Read;

   procedure Pid_Read
     (R               : in out Reader;
      Member_Id       : out Long_Integer;
      Must_Understand : out Boolean;
      Length          : out Long_Integer;
      List_End        : out Boolean)
   is
      Raw   : Long_Integer;
      Short : Long_Integer;
      Slen  : Long_Integer;
      Raw4  : Byte_Array (0 .. 3);
      Top   : Integer;
   begin
      Member_Id := 0;
      Must_Understand := False;
      Length := 0;
      List_End := False;
      Get_U16 (R, Raw);
      --  Bits 14 and 15 are the flags, so the id is the low fourteen.
      Short := Raw mod 16#4000#;
      if Short = Pid_List_End_Id then
         Get_U16 (R, Slen);
         List_End := True;
         return;
      end if;
      if Short = Pid_Extended_Id then
         Get_U16 (R, Slen);
         --  Section 7.4.1.2 fixes the marker's own length at 8. A marker
         --  that says anything else is not this framing, and reading on
         --  would take the next member's octets as an eMemberHeader.
         if Slen /= 8 then
            R.Underflow := True;
            return;
         end if;
         R_Align (R, 4);
         Get_Bytes (R, Raw4);
         if R.Underflow then
            return;
         end if;
         if R.Endian = Big then
            declare
               Swap : Byte;
            begin
               Swap := Raw4 (0); Raw4 (0) := Raw4 (3); Raw4 (3) := Swap;
               Swap := Raw4 (1); Raw4 (1) := Raw4 (2); Raw4 (2) := Swap;
            end;
         end if;
         Top := Integer (Raw4 (3));
         --  Bit 30 of the eMemberHeader, never the marker's bit 14.
         Must_Understand := (Top / 64) mod 2 = 1;
         Member_Id := Long_Integer (Raw4 (0))
           + Long_Integer (Raw4 (1)) * 256
           + Long_Integer (Raw4 (2)) * 65536
           + Long_Integer (Top mod 16) * 16#100_0000#;
         Get_U32 (R, Length);
         R.Origin := R.Pos;
         return;
      end if;
      Get_U16 (R, Length);
      Member_Id := Short;
      Must_Understand := (Raw / 16#4000#) mod 2 = 1;
      R.Origin := R.Pos;
   end Pid_Read;

   procedure Pid_Skip
     (R : in out Reader; Body_Start : Integer; Length : Long_Integer) is
   begin
      R.Pos := Body_Start + Integer (Length);
      R.Origin := 0;
      R.Pos := R.Pos + Padding_For (R.Pos, 4);
   end Pid_Skip;

end Zerodds_Ada83_Wire;
