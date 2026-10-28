-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Zdw_Xcdr is

   type Octets is array (Natural range <>) of aliased unsigned_char;
   pragma Convention (C, Octets);

   --  `Zdw`'s writers return a code; every operation below is several of
   --  them, and a code from the middle of one says nothing about the frame,
   --  so a refusal stops here.
   procedure Check (Code : int) is
   begin
      if Code /= Zdw.ZDW_OK then
         raise Wire_Refused;
      end if;
   end Check;

   procedure Put_Raw (W : access Zdw.Zdw_Writer; Data : Octets) is
   begin
      if Data'Length = 0 then
         return;
      end if;
      Check (Zdw.Put_Bytes (W, Data (Data'First)'Address,
                            size_t (Data'Length)));
   end Put_Raw;

   --  Writes `V` over `Width` octets already in the buffer, in wire byte
   --  order. The C core has no back-patch of its own; the buffer is the
   --  caller's and `W.Buf` points at it, so the overlay is over storage this
   --  program owns.
   procedure Patch
     (W      : access Zdw.Zdw_Writer;
      At_Pos : size_t;
      V      : unsigned_long;
      Width  : Natural)
   is
      LE   : Octets (0 .. Width - 1);
      Rest : unsigned_long := V;
      Buf  : Octets (0 .. Natural (W.Cap) - 1);
      for Buf'Address use W.Buf;
      pragma Import (Ada, Buf);
   begin
      for I in LE'Range loop
         LE (I) := unsigned_char (Rest mod 256);
         Rest := Rest / 256;
      end loop;
      for I in 0 .. Width - 1 loop
         if W.Endian = Zdw.ZDW_BE then
            Buf (Natural (At_Pos) + I) := LE (Width - 1 - I);
         else
            Buf (Natural (At_Pos) + I) := LE (I);
         end if;
      end loop;
   end Patch;

   --  Pads to the next `Align` boundary of the *stream*. `zdw_align` is
   --  internal to the C core, so the padding is written here with the count
   --  `zdw_padding_for` gives.
   procedure Align_Stream (W : access Zdw.Zdw_Writer; Align : size_t) is
      Pad : constant size_t := Zdw.Padding_For (W.Len, Align);
   begin
      if Pad > 0 then
         declare
            Zeros : constant Octets (0 .. Natural (Pad) - 1) := [others => 0];
         begin
            Put_Raw (W, Zeros);
         end;
      end if;
   end Align_Stream;

   --  --- the member header ---

   procedure EMHeader_Begin
     (W               : access Zdw.Zdw_Writer;
      Member_Id       : unsigned_long;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out size_t)
   is
      Em : unsigned_long;
   begin
      Body_Start := W.Len;
      --  Section 7.4.3.4.2 gives the id 28 bits. Truncating it would put a
      --  different member's id on the wire.
      if Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      Em := unsigned_long (Length_Code'Pos (Code)) * 16#1000_0000# + Member_Id;
      if Must_Understand then
         Em := Em + 16#8000_0000#;
      end if;
      Check (Zdw.Put_U32 (W, Em));
      if Code = LC4 then
         --  Only LC4 reserves a NEXTINT. LC5..LC7 take the length from the
         --  body's own leading word, and writing it twice would put every
         --  following member four octets late.
         Check (Zdw.Put_U32 (W, 0));
      end if;
      Body_Start := W.Len;
   end EMHeader_Begin;

   procedure EMHeader_End
     (W : access Zdw.Zdw_Writer; Body_Start : size_t; Code : Length_Code)
   is
      Written : constant size_t := W.Len - Body_Start;
   begin
      case Code is
         when LC4 =>
            Check (Zdw.DHeader_End (W, Body_Start));
         when LC0 | LC1 | LC2 | LC3 =>
            --  1, 2, 4, 8 -- the code *is* the length, so a body of any
            --  other size means the plan and the member disagree.
            if Written /= size_t (2 ** Length_Code'Pos (Code)) then
               raise Length_Code_Mismatch;
            end if;
         when LC5 | LC6 | LC7 =>
            if Written < 4 then
               raise Length_Code_Mismatch;
            end if;
      end case;
   end EMHeader_End;

   --  --- the PL_CDR1 parameter list ---

   Pid_Flag_Must_Understand : constant unsigned_long := 16#4000#;
   --  Bit 30 of the eMemberHeader, not bit 31, which is the
   --  implementation-extension flag.
   Em_Flag_Must_Understand  : constant unsigned_long := 16#4000_0000#;

   procedure Pid_Begin
     (W               : access Zdw.Zdw_Writer;
      Pid             : unsigned_long;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      Id   : unsigned_long := Pid;
      Slot : constant Octets (0 .. 1) := [0, 0];
   begin
      Header := (At_Pos => 0, Origin => 0, Extended => False,
                 Saved_Cap => W.Max_Align);
      if Pid > Pid_Short_Max_Id then
         raise Member_Id_Out_Of_Range;
      end if;
      --  The header itself is written with the stream's own alignment
      --  still in force; only the value's is the block's.
      Align_Stream (W, 4);
      if Must_Understand then
         Id := Id + Pid_Flag_Must_Understand;
      end if;
      Check (Zdw.Put_U16 (W, unsigned (Id)));
      Header.At_Pos := W.Len;
      Put_Raw (W, Slot);
      Header.Origin := W.Len;
      --  From here the C writer aligns nothing: every boundary inside the
      --  block is Align_Block's, measured from Header.Origin.
      W.Max_Align := 1;
   end Pid_Begin;

   procedure Pid_Extended_Begin
     (W               : access Zdw.Zdw_Writer;
      Member_Id       : unsigned_long;
      Must_Understand : Boolean;
      Header          : out Pid_Header)
   is
      Em   : unsigned_long := Member_Id;
      Slot : constant Octets (0 .. 3) := [0, 0, 0, 0];
   begin
      Header := (At_Pos => 0, Origin => 0, Extended => True,
                 Saved_Cap => W.Max_Align);
      if Member_Id > 16#0FFF_FFFF# then
         raise Member_Id_Out_Of_Range;
      end if;
      Align_Stream (W, 4);
      --  The marker carries the must-understand flag under this framing
      --  whatever the member says.
      Check (Zdw.Put_U16
               (W, unsigned (unsigned_long (Pid_Extended_Id)
                             + Pid_Flag_Must_Understand)));
      Check (Zdw.Put_U16 (W, 8));
      if Must_Understand then
         Em := Em + Em_Flag_Must_Understand;
      end if;
      Check (Zdw.Put_U32 (W, Em));
      Header.At_Pos := W.Len;
      Put_Raw (W, Slot);
      Header.Origin := W.Len;
      W.Max_Align := 1;
   end Pid_Extended_Begin;

   procedure Align_Block
     (W : access Zdw.Zdw_Writer; Header : Pid_Header; Align : size_t)
   is
      Eff : size_t := Align;
      Pad : size_t;
   begin
      --  Capped at the encoding's, which the parameter saved: XCDR1 is
      --  eight and XCDR2 four (section 7.4.1).
      if Eff > size_t (Header.Saved_Cap) then
         Eff := size_t (Header.Saved_Cap);
      end if;
      Pad := Zdw.Padding_For (W.Len - Header.Origin, Eff);
      if Pad > 0 then
         declare
            Zeros : constant Octets (0 .. Natural (Pad) - 1) := [others => 0];
         begin
            Put_Raw (W, Zeros);
         end;
      end if;
   end Align_Block;

   procedure Pid_End (W : access Zdw.Zdw_Writer; Header : Pid_Header) is
      Width  : constant size_t := (if Header.Extended then 4 else 2);
      Length : size_t := W.Len - Header.At_Pos - Width;
      Past   : size_t;
   begin
      --  The declared length counts the padding that brings the next
      --  header back to a four-octet boundary, measured from the block the
      --  length describes.
      Past := (W.Len - Header.Origin) mod 4;
      if Past /= 0 then
         Length := Length + (4 - Past);
      end if;
      if Header.Extended then
         Patch (W, Header.At_Pos, unsigned_long (Length), 4);
      else
         if Length > 16#FFFF# then
            raise Parameter_Too_Long;
         end if;
         Patch (W, Header.At_Pos, unsigned_long (Length), 2);
      end if;
      W.Max_Align := Header.Saved_Cap;
      --  The padding itself, after the length that already counts it.
      Align_Stream (W, 4);
   end Pid_End;

   procedure Pid_List_End (W : access Zdw.Zdw_Writer) is
   begin
      Align_Stream (W, 4);
      Check (Zdw.Put_U16
               (W, unsigned (unsigned_long (Pid_List_End_Id)
                             + Pid_Flag_Must_Understand)));
      Check (Zdw.Put_U16 (W, 0));
   end Pid_List_End;

   --  --- the scalars ---

   procedure Put_Char
     (W : access Zdw.Zdw_Writer; Code_Point : unsigned_long) is
   begin
      if Code_Point > 16#FF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Check (Zdw.Put_U8 (W, unsigned_char (Code_Point)));
   end Put_Char;

   procedure Put_Wchar
     (W : access Zdw.Zdw_Writer; Code_Point : unsigned_long) is
   begin
      --  The refusal is the code point's and not the width's: a wider
      --  write would zero-extend a code *unit*, not a code point.
      if Code_Point > 16#FFFF# then
         raise Code_Point_Out_Of_Range;
      end if;
      Check (Zdw.Put_U16 (W, unsigned (Code_Point)));
   end Put_Wchar;

   procedure Put_Wstring
     (W : access Zdw.Zdw_Writer; Units : Code_Unit_Array) is
   begin
      Check (Zdw.Put_U32 (W, unsigned_long (Units'Length) * 2));
      for I in Units'Range loop
         Check (Zdw.Put_U16 (W, unsigned (Units (I))));
      end loop;
   end Put_Wstring;

   procedure Put_Bitset
     (W      : access Zdw.Zdw_Writer;
      Bits   : Zdw.Zdw_U64;
      Anchor : Natural)
   is
   begin
      case Anchor is
         when 1 => Check (Zdw.Put_U8  (W, unsigned_char (Bits.Lo mod 256)));
         when 2 => Check (Zdw.Put_U16 (W, unsigned (Bits.Lo mod 65536)));
         when 4 => Check (Zdw.Put_U32 (W, Bits.Lo));
         when 8 => Check (Zdw.Put_U64 (W, Bits));
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Put_Bitset;

   --  The nibbles Put_Fixed packs. `Count` is how many were written, and it
   --  is always even because the sign nibble shares the last octet with the
   --  least significant digit.
   procedure Fixed_Nibbles
     (Value       : String;
      Digit_Count : Positive;
      Scale       : Natural;
      Nibbles     : out Octets;
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

      procedure Emit (N : unsigned_char) is
      begin
         Nibbles (K) := N;
         K := K + 1;
      end Emit;
   begin
      Nibbles := [others => 0];
      Count := 0;
      --  IDL 4.2 section 7.4.1.4.5 bounds a fixed at 31 digits. Stated
      --  rather than left to the arithmetic: a Digit_Count of 40 would
      --  overrun `Nibbles` silently under -gnatp.
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
      --  width is checked: "000000.001" is one integer digit, not six.
      while Int_First <= Int_Last and then Value (Int_First) = '0' loop
         Int_First := Int_First + 1;
      end loop;
      if Int_Last - Int_First + 1 > Digit_Count - Scale then
         raise Not_A_Fixed_Value;
      end if;
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
         Emit (unsigned_char (Character'Pos (Value (I))
                              - Character'Pos ('0')));
      end loop;
      for I in Frc_First .. Frc_Last loop
         Emit (unsigned_char (Character'Pos (Value (I))
                              - Character'Pos ('0')));
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
     (W           : access Zdw.Zdw_Writer;
      Value       : String;
      Digit_Count : Positive;
      Scale       : Natural)
   is
      Nibbles : Octets (0 .. 32);  -- 31 digits + a pad + the sign
      Count   : Natural;
   begin
      Fixed_Nibbles (Value, Digit_Count, Scale, Nibbles, Count);
      declare
         Packed : Octets (0 .. Count / 2 - 1);
      begin
         for I in Packed'Range loop
            Packed (I) := Nibbles (2 * I) * 16 + Nibbles (2 * I + 1);
         end loop;
         --  Alignment is one and no multi-octet integer is written, so
         --  nothing here consults the stream's byte order.
         Put_Raw (W, Packed);
      end;
   end Put_Fixed;

   --  --- the reading direction ---

   procedure EMHeader_Read
     (R               : access Zdw.Zdw_Reader;
      Member_Id       : out unsigned_long;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out unsigned_long)
   is
      Em   : aliased unsigned_long;
      Word : aliased unsigned_long;
      Mark : size_t;
   begin
      Extent := 0;
      Check (Zdw.Get_U32 (R, Em'Access));
      Must_Understand := (Em / 16#8000_0000#) mod 2 = 1;
      Member_Id := Em mod 16#1000_0000#;
      Code := Length_Code'Val (Natural ((Em / 16#1000_0000#) mod 8));
      case Code is
         when LC0 | LC1 | LC2 | LC3 =>
            Extent := unsigned_long (2 ** Length_Code'Pos (Code));
         when LC4 =>
            Check (Zdw.Get_U32 (R, Word'Access));
            Extent := Word;
         when LC5 | LC6 | LC7 =>
            --  The word belongs to the body, so it is read and the cursor
            --  put back where it was.
            Mark := R.Pos;
            Check (Zdw.Get_U32 (R, Word'Access));
            R.Pos := Mark;
            case Code is
               when LC5 => Extent := 4 + Word;
               when LC6 => Extent := 4 + 4 * Word;
               when others => Extent := 4 + 8 * Word;
            end case;
      end case;
   end EMHeader_Read;

   procedure Pid_Read
     (R               : access Zdw.Zdw_Reader;
      Member_Id       : out unsigned_long;
      Must_Understand : out Boolean;
      Length          : out unsigned_long;
      List_End        : out Boolean;
      Header          : out Pid_Header)
   is
      Raw   : aliased unsigned;
      Slen  : aliased unsigned;
      Len16 : aliased unsigned;
      Em    : aliased unsigned_long;
      Len32 : aliased unsigned_long;
      Short : unsigned_long;
   begin
      Member_Id := 0;
      Must_Understand := False;
      Length := 0;
      List_End := False;
      Header := (At_Pos => 0, Origin => 0, Extended => False,
                 Saved_Cap => R.Max_Align);
      Check (Zdw.Get_U16 (R, Raw'Access));
      --  Bits 14 and 15 are the flags, so the id is the low fourteen.
      Short := unsigned_long (Raw) mod 16#4000#;
      if Short = Pid_List_End_Id then
         Check (Zdw.Get_U16 (R, Slen'Access));
         List_End := True;
         return;
      end if;
      if Short = Pid_Extended_Id then
         Check (Zdw.Get_U16 (R, Slen'Access));
         --  Section 7.4.1.2 fixes the marker's own length at 8. A marker
         --  that says anything else is not this framing.
         if unsigned_long (Slen) /= 8 then
            raise Wire_Refused;
         end if;
         Check (Zdw.Get_U32 (R, Em'Access));
         Check (Zdw.Get_U32 (R, Len32'Access));
         Length := Len32;
         Member_Id := Em mod 16#1000_0000#;
         --  Bit 30 of the eMemberHeader, never the marker's bit 14.
         Must_Understand := (Em / Em_Flag_Must_Understand) mod 2 = 1;
         Header.Extended := True;
         Header.Origin := R.Pos;
         R.Max_Align := 1;
         return;
      end if;
      Check (Zdw.Get_U16 (R, Len16'Access));
      Member_Id := Short;
      Must_Understand := (unsigned_long (Raw) / 16#4000#) mod 2 = 1;
      Length := unsigned_long (Len16);
      Header.Origin := R.Pos;
      R.Max_Align := 1;
   end Pid_Read;

   procedure Align_Block_In
     (R : access Zdw.Zdw_Reader; Header : Pid_Header; Align : size_t)
   is
      Eff : size_t := Align;
   begin
      if Eff > size_t (Header.Saved_Cap) then
         Eff := size_t (Header.Saved_Cap);
      end if;
      R.Pos := R.Pos + Zdw.Padding_For (R.Pos - Header.Origin, Eff);
   end Align_Block_In;

   procedure Pid_Skip
     (R : access Zdw.Zdw_Reader; Header : Pid_Header; Length : unsigned_long)
   is
   begin
      R.Max_Align := Header.Saved_Cap;
      R.Pos := Header.Origin + size_t (Length);
      R.Pos := R.Pos + Zdw.Padding_For (R.Pos, 4);
   end Pid_Skip;

   procedure Get_Char
     (R : access Zdw.Zdw_Reader; Code_Point : out unsigned_long)
   is
      V : aliased unsigned_char;
   begin
      Check (Zdw.Get_U8 (R, V'Access));
      Code_Point := unsigned_long (V);
   end Get_Char;

   procedure Get_Wchar
     (R : access Zdw.Zdw_Reader; Code_Point : out unsigned_long)
   is
      V : aliased unsigned;
   begin
      Check (Zdw.Get_U16 (R, V'Access));
      Code_Point := unsigned_long (V);
   end Get_Wchar;

   procedure Get_Wstring
     (R : access Zdw.Zdw_Reader; Units : out Code_Unit_Array; N : out Natural)
   is
      Octet_Count : aliased unsigned_long;
      Unit        : aliased unsigned;
      Count       : Natural;
   begin
      Units := [others => 0];
      N := 0;
      Check (Zdw.Get_U32 (R, Octet_Count'Access));
      --  The length counts octets, so the unit count is half of it.
      Count := Natural (Octet_Count / 2);
      if Count > Units'Length then
         raise Wire_Refused;
      end if;
      for I in 0 .. Count - 1 loop
         Check (Zdw.Get_U16 (R, Unit'Access));
         Units (Units'First + I) := Code_Unit (Unit);
      end loop;
      N := Count;
   end Get_Wstring;

   procedure Get_Bitset
     (R : access Zdw.Zdw_Reader; Anchor : Natural; Bits : out Zdw.Zdw_U64)
   is
      B8  : aliased unsigned_char;
      B16 : aliased unsigned;
      B32 : aliased unsigned_long;
      B64 : aliased Zdw.Zdw_U64;
   begin
      Bits := (Hi => 0, Lo => 0);
      case Anchor is
         when 1 =>
            Check (Zdw.Get_U8 (R, B8'Access));
            Bits.Lo := unsigned_long (B8);
         when 2 =>
            Check (Zdw.Get_U16 (R, B16'Access));
            Bits.Lo := unsigned_long (B16);
         when 4 =>
            Check (Zdw.Get_U32 (R, B32'Access));
            Bits.Lo := B32;
         when 8 =>
            Check (Zdw.Get_U64 (R, B64'Access));
            Bits := B64;
         when others =>
            raise Anchor_Not_A_Holder_Width;
      end case;
   end Get_Bitset;

   procedure Get_Fixed
     (R           : access Zdw.Zdw_Reader;
      Digit_Count : Positive;
      Scale       : Natural;
      Value       : out String;
      Last        : out Natural)
   is
      Octet_Count : constant Natural := (Digit_Count + 2) / 2;
      Raw         : Octets (0 .. Octet_Count - 1);
      Nibbles     : Octets (0 .. 2 * Octet_Count - 1);
      Sign        : unsigned_char;
      K           : Natural;
   begin
      Value := [others => ' '];
      Last := Value'First - 1;
      if Digit_Count > 31 or else Scale > Digit_Count then
         raise Not_A_Fixed_Value;
      end if;
      --  A sign, Digit_Count digits and a point.
      if Value'Length < Digit_Count + 2 then
         raise Wire_Refused;
      end if;
      Check (Zdw.Get_Bytes (R, Raw (0)'Address, size_t (Raw'Length)));
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
            + Natural (Nibbles (Nibbles'Last - Digit_Count + I)));
         K := K + 1;
      end loop;
      Last := K - 1;
   end Get_Fixed;

end Zdw_Xcdr;
