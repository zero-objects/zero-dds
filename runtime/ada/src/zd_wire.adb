--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  The Ada wire runtime's body. Every clause is implemented from the
--  specification text; runtime/d/zdwire.d is the closest sibling and
--  the two are compared through the corpus, never through each other.

with Ada.Text_IO;
with Ada.Unchecked_Conversion;

package body Zd_Wire is

   package SU renames Ada.Strings.Unbounded;

   subtype U8 is Interfaces.Unsigned_8;
   subtype U16 is Interfaces.Unsigned_16;
   subtype U32 is Interfaces.Unsigned_32;
   subtype U64 is Interfaces.Unsigned_64;
   subtype I8 is Interfaces.Integer_8;
   subtype I16 is Interfaces.Integer_16;
   subtype I32 is Interfaces.Integer_32;
   subtype I64 is Interfaces.Integer_64;
   subtype F32 is Interfaces.IEEE_Float_32;
   subtype F64 is Interfaces.IEEE_Float_64;

   use type Interfaces.Integer_8;
   use type Interfaces.Integer_16;
   use type Interfaces.Integer_32;
   use type Interfaces.IEEE_Float_32;
   use type Interfaces.IEEE_Float_64;

   function To_I8 is new Ada.Unchecked_Conversion (U8, I8);
   function To_I16 is new Ada.Unchecked_Conversion (U16, I16);
   function To_I32 is new Ada.Unchecked_Conversion (U32, I32);
   function To_I64 is new Ada.Unchecked_Conversion (U64, I64);
   function To_U16 is new Ada.Unchecked_Conversion (I16, U16);
   function To_U32 is new Ada.Unchecked_Conversion (I32, U32);
   function To_U64 is new Ada.Unchecked_Conversion (I64, U64);
   function F32_Bits is new Ada.Unchecked_Conversion (F32, U32);
   function F64_Bits is new Ada.Unchecked_Conversion (F64, U64);
   function Bits_F32 is new Ada.Unchecked_Conversion (U32, F32);
   function Bits_F64 is new Ada.Unchecked_Conversion (U64, F64);

   package F64_IO is new Ada.Text_IO.Float_IO (F64);

   --  The parameter ids and flags of XTypes 1.3 7.4.1.2.2 and
   --  DDSI-RTPS 2.5 9.4.2.11.2.
   Pid_Extended_Id      : constant U32 := 16#3F01#;
   Pid_List_End_Id      : constant U32 := 16#3F02#;
   Pid_Ignore_Id        : constant U32 := 16#3F03#;
   Pid_Must_Understand  : constant U32 := 16#4000#;
   Pid_Impl_Extension   : constant U32 := 16#8000#;
   Pid_Short_Max_Length : constant Natural := 16#FFFF#;
   Pid_Id_Mask          : constant U32 := 16#3FFF#;
   E_Flag_Must_Understand : constant U32 := 16#4000_0000#;
   E_Flag_Impl_Extension  : constant U32 := 16#8000_0000#;
   E_Member_Id_Mask       : constant U32 := 16#0FFF_FFFF#;

   Byte_Order_Mark_Unit         : constant U16 := 16#FEFF#;
   Swapped_Byte_Order_Mark_Unit : constant U16 := 16#FFFE#;

   --  Where a PL_CDR1 parameter flag is asked for.
   type Wire_Position is (Member_Pos, Sentinel_Pos, Extended_Marker_Pos);

   function Flags_At (Policy : Pid_Flag_Policy; Position : Wire_Position) return U32 is
   begin
      case Policy is
         when Flags_Sentinel_Must_Understand =>
            if Position = Sentinel_Pos or else Position = Extended_Marker_Pos then
               return Pid_Must_Understand;
            end if;
            return 0;
         when Flags_Bare =>
            return 0;
         when Flags_Impl_Extension_Members =>
            if Position = Member_Pos or else Position = Extended_Marker_Pos then
               return Pid_Impl_Extension or Pid_Must_Understand;
            end if;
            return 0;
      end case;
   end Flags_At;

   --  One cell of the wstring C1 x C2 x C3 product.
   type Wstring_Cell is record
      Counts_Characters : Boolean;
      Terminated        : Boolean;
      Width             : Natural;
   end record;

   function Cell_Of (Policy : Wstring_Policy) return Wstring_Cell is
   begin
      case Policy is
         when Wstr_Octets_Unterminated_2     => return (False, False, 2);
         when Wstr_Characters_Unterminated_2 => return (True, False, 2);
         when Wstr_Characters_Terminated_4   => return (True, True, 4);
         when Wstr_Octets_Unterminated_4     => return (False, False, 4);
         when Wstr_Characters_Unterminated_4 => return (True, False, 4);
         when Wstr_Octets_Terminated_2       => return (False, True, 2);
         when Wstr_Octets_Terminated_4       => return (False, True, 4);
         when Wstr_Characters_Terminated_2   => return (True, True, 2);
      end case;
   end Cell_Of;

   --  A run of ASCII zeros, for the decimal paddings.
   function Zeros (Count : Natural) return String is
      Run : constant String (1 .. Count) := (others => '0');
   begin
      return Run;
   end Zeros;

   function Image (Value : Natural) return String is
      Raw : constant String := Natural'Image (Value);
   begin
      return Raw (Raw'First + 1 .. Raw'Last);
   end Image;

   --  -- UTF-8 --------------------------------------------------------

   --  The code points of a UTF-8 string, decoded strictly: overlong
   --  forms, surrogates, values past U+10FFFF and truncated tails are
   --  refused -- the same reading std.utf.validate and the sibling
   --  runtimes take.
   type Point_Array is array (Natural range <>) of U32;

   function Decode_Utf8 (Value : String; What : String) return Point_Array is
      Out_Points : Point_Array (0 .. Value'Length);
      Count      : Natural := 0;
      Index      : Natural := Value'First;

      function Byte_At (Position : Natural) return U32 is
      begin
         return U32 (Character'Pos (Value (Position)));
      end Byte_At;

      procedure Refuse is
      begin
         raise Wire_Error with What & ": the value is not UTF-8";
      end Refuse;
   begin
      while Index <= Value'Last loop
         declare
            First : constant U32 := Byte_At (Index);
            Point : U32;
            Tail  : Natural;
            Low   : U32;
         begin
            if First < 16#80# then
               Point := First;
               Tail := 0;
               Low := 0;
            elsif (First and 16#E0#) = 16#C0# then
               Point := First and 16#1F#;
               Tail := 1;
               Low := 16#80#;
            elsif (First and 16#F0#) = 16#E0# then
               Point := First and 16#0F#;
               Tail := 2;
               Low := 16#800#;
            elsif (First and 16#F8#) = 16#F0# then
               Point := First and 16#07#;
               Tail := 3;
               Low := 16#1_0000#;
            else
               Refuse;
               return Out_Points (0 .. 0);
            end if;
            if Index + Tail > Value'Last then
               Refuse;
            end if;
            for Extra in 1 .. Tail loop
               declare
                  Cont : constant U32 := Byte_At (Index + Extra);
               begin
                  if (Cont and 16#C0#) /= 16#80# then
                     Refuse;
                  end if;
                  Point := Interfaces.Shift_Left (Point, 6) or (Cont and 16#3F#);
               end;
            end loop;
            if Tail > 0 and then Point < Low then
               Refuse;
            end if;
            if Point > 16#10_FFFF# or else (Point >= 16#D800# and then Point <= 16#DFFF#) then
               Refuse;
            end if;
            Out_Points (Count) := Point;
            Count := Count + 1;
            Index := Index + 1 + Tail;
         end;
      end loop;
      return Out_Points (0 .. Count - 1);
   end Decode_Utf8;

   procedure Append_Point (Value : in out Text; Point : U32) is
      procedure Raw (Octet : U32) is
      begin
         SU.Append (Value, Character'Val (Natural (Octet)));
      end Raw;
   begin
      if Point < 16#80# then
         Raw (Point);
      elsif Point < 16#800# then
         Raw (16#C0# or Interfaces.Shift_Right (Point, 6));
         Raw (16#80# or (Point and 16#3F#));
      elsif Point < 16#1_0000# then
         Raw (16#E0# or Interfaces.Shift_Right (Point, 12));
         Raw (16#80# or (Interfaces.Shift_Right (Point, 6) and 16#3F#));
         Raw (16#80# or (Point and 16#3F#));
      else
         Raw (16#F0# or Interfaces.Shift_Right (Point, 18));
         Raw (16#80# or (Interfaces.Shift_Right (Point, 12) and 16#3F#));
         Raw (16#80# or (Interfaces.Shift_Right (Point, 6) and 16#3F#));
         Raw (16#80# or (Point and 16#3F#));
      end if;
   end Append_Point;

   --  The UTF-16 code units of a UTF-8 value, without a BOM.
   type Unit_Array is array (Natural range <>) of U16;

   function Utf16_Of (Value : Text; What : String) return Unit_Array is
      Raw    : constant String := SU.To_String (Value);
      Points : constant Point_Array := Decode_Utf8 (Raw, What);
      Units  : Unit_Array (0 .. 2 * Points'Length);
      Count  : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant U32 := Points (Index);
         begin
            if Point < 16#1_0000# then
               Units (Count) := U16 (Point);
               Count := Count + 1;
            else
               declare
                  Offset : constant U32 := Point - 16#1_0000#;
               begin
                  Units (Count) := U16 (16#D800# + Interfaces.Shift_Right (Offset, 10));
                  Units (Count + 1) := U16 (16#DC00# + (Offset and 16#3FF#));
                  Count := Count + 2;
               end;
            end if;
         end;
      end loop;
      return Units (0 .. Count - 1);
   end Utf16_Of;

   --  -- helpers the generated code names -----------------------------

   function To_Text (S : String) return Text is
   begin
      return SU.To_Unbounded_String (S);
   end To_Text;

   function From_Utf8 (Octets : Byte_Array) return Text is
      Out_Text : Text := SU.Null_Unbounded_String;
   begin
      for Index in Octets'Range loop
         SU.Append (Out_Text, Character'Val (Natural (Octets (Index))));
      end loop;
      return Out_Text;
   end From_Utf8;

   function Strcmp (A, B : Text) return Integer is
      Left  : constant String := SU.To_String (A);
      Right : constant String := SU.To_String (B);
   begin
      --  Ada's String "<" compares Character'Pos, which over UTF-8
      --  bytes is exactly C11 7.24.4's unsigned-octet order.
      if Left < Right then
         return -1;
      elsif Left > Right then
         return 1;
      end if;
      return 0;
   end Strcmp;

   function Utf8_Octets (Value : Text) return Natural is
   begin
      return SU.Length (Value);
   end Utf8_Octets;

   function Utf16_Units (Value : Text) return Natural is
      Points : constant Point_Array :=
        Decode_Utf8 (SU.To_String (Value), "utf16-units");
      Count  : Natural := 0;
   begin
      for Index in Points'Range loop
         Count := Count + (if Points (Index) > 16#FFFF# then 2 else 1);
      end loop;
      return Count;
   end Utf16_Units;

   function Checked_Char (Point : U32) return U8 is
   begin
      if Point > 16#FF# then
         raise Wire_Error with "a character below U+0100";
      end if;
      return U8 (Point);
   end Checked_Char;

   function Checked_Wchar (Point : U32) return U16 is
   begin
      if Point > 16#FFFF# then
         raise Wire_Error with "a BMP character";
      end if;
      return U16 (Point);
   end Checked_Wchar;

   function As_Signed (Value : U8) return I8 is
   begin
      return To_I8 (Value);
   end As_Signed;

   procedure Discard_U32 (Value : U32) is
      pragma Unreferenced (Value);
   begin
      null;
   end Discard_U32;

   function Bit_Pattern_Int (Value : I64; Octets : Natural) return U64 is
      Pattern : constant U64 := To_U64 (Value);
   begin
      if Octets >= 8 then
         return Pattern;
      end if;
      return Pattern and (Interfaces.Shift_Left (U64'(1), Octets * 8) - 1);
   end Bit_Pattern_Int;

   function Bit_Pattern_Bool (Value : Boolean) return U64 is
   begin
      return (if Value then 1 else 0);
   end Bit_Pattern_Bool;

   function Bit_Pattern_F32 (Value : F32) return U64 is
   begin
      return U64 (F32_Bits (Value));
   end Bit_Pattern_F32;

   function Bit_Pattern_F64 (Value : F64) return U64 is
   begin
      return F64_Bits (Value);
   end Bit_Pattern_F64;

   function F32_Nan return F32 is
   begin
      return Bits_F32 (16#7FC0_0000#);
   end F32_Nan;

   function F32_Inf return F32 is
   begin
      return Bits_F32 (16#7F80_0000#);
   end F32_Inf;

   function F64_Nan return F64 is
   begin
      return Bits_F64 (16#7FF8_0000_0000_0000#);
   end F64_Nan;

   function F64_Inf return F64 is
   begin
      return Bits_F64 (16#7FF0_0000_0000_0000#);
   end F64_Inf;

   function Status_Text (S : Status) return String is
   begin
      case S is
         when Status_Ok                => return "ok";
         when Status_Truncated         => return "truncated";
         when Status_Impossible_Length => return "impossible-length";
         when Status_Out_Of_Range      => return "out-of-range";
         when Status_Discarded         => return "discarded";
      end case;
   end Status_Text;

   function Pid_Is_User_Member (Header : Pid_Read) return Boolean is
   begin
      return not Header.Impl_Extension
        and then not Header.Reserved
        and then not Header.Ignore;
   end Pid_Is_User_Member;

   function No_Dheader return Dheader is
   begin
      return (At_Pos => -1);
   end No_Dheader;

   --  -- the writer ---------------------------------------------------

   function New_Writer
     (Order              : Byte_Order;
      Max_Align          : Natural;
      Framing            : Pl_Cdr1_Framing;
      Short_Length_Codes : Boolean) return Writer is
   begin
      return
        (Buffer             => null,
         Count              => 0,
         Order              => Order,
         Max_Align          => Natural'Max (1, Max_Align),
         Origin             => 0,
         Framing            => Framing,
         Short_Length_Codes => Short_Length_Codes);
   end New_Writer;

   procedure Put (W : in out Writer; Octet : U8) is
   begin
      if W.Buffer = null or else W.Count > W.Buffer'Last then
         declare
            Wanted : constant Natural :=
              (if W.Buffer = null then 256 else 2 * W.Buffer'Length);
            Grown  : constant Byte_Array_Access := new Byte_Array (0 .. Wanted - 1);
         begin
            if W.Buffer /= null then
               Grown (0 .. W.Count - 1) := W.Buffer (0 .. W.Count - 1);
            end if;
            W.Buffer := Grown;
         end;
      end if;
      W.Buffer (W.Count) := Octet;
      W.Count := W.Count + 1;
   end Put;

   function Finish (W : Writer) return Byte_Array_Access is
   begin
      if W.Count = 0 then
         return new Byte_Array (0 .. -1);
      end if;
      return new Byte_Array'(W.Buffer (0 .. W.Count - 1));
   end Finish;

   procedure Align_To (W : in out Writer; Alignment : Natural) is
      Effective : constant Natural := Natural'Min (Alignment, W.Max_Align);
   begin
      if Effective <= 1 then
         return;
      end if;
      declare
         Offset  : constant Natural := (W.Count - W.Origin) mod Effective;
         Padding : constant Natural := (if Offset = 0 then 0 else Effective - Offset);
      begin
         for Extra in 1 .. Padding loop
            Put (W, 0);
         end loop;
      end;
   end Align_To;

   procedure Write_Raw (W : in out Writer; Value : U64; Width : Natural) is
   begin
      if W.Order = Little then
         for Index in 0 .. Width - 1 loop
            Put (W, U8 (Interfaces.Shift_Right (Value, 8 * Index) and 16#FF#));
         end loop;
      else
         for Index in reverse 0 .. Width - 1 loop
            Put (W, U8 (Interfaces.Shift_Right (Value, 8 * Index) and 16#FF#));
         end loop;
      end if;
   end Write_Raw;

   procedure Write_Int (W : in out Writer; Value : U64; Width : Natural) is
   begin
      Align_To (W, Width);
      Write_Raw (W, Value, Width);
   end Write_Int;

   procedure Patch (W : in out Writer; At_Pos : Natural; Value : U64; Width : Natural) is
   begin
      if Width < 8 and then Value >= Interfaces.Shift_Left (U64'(1), Width * 8) then
         raise Wire_Error with
           "at " & Image (At_Pos) & ": a " & Image (Width) & "-byte length";
      end if;
      for Index in 0 .. Width - 1 loop
         declare
            Octet : constant U8 :=
              U8 (Interfaces.Shift_Right (Value, 8 * Index) and 16#FF#);
         begin
            if W.Order = Little then
               W.Buffer (At_Pos + Index) := Octet;
            else
               W.Buffer (At_Pos + Width - 1 - Index) := Octet;
            end if;
         end;
      end loop;
   end Patch;

   function Read_Patched (W : Writer; At_Pos : Natural) return U32 is
      Value : U32 := 0;
   begin
      if W.Order = Little then
         for Index in reverse 0 .. 3 loop
            Value := Interfaces.Shift_Left (Value, 8) or U32 (W.Buffer (At_Pos + Index));
         end loop;
      else
         for Index in 0 .. 3 loop
            Value := Interfaces.Shift_Left (Value, 8) or U32 (W.Buffer (At_Pos + Index));
         end loop;
      end if;
      return Value;
   end Read_Patched;

   procedure Write_Bool (W : in out Writer; Value : Boolean) is
   begin
      Put (W, (if Value then 1 else 0));
   end Write_Bool;

   procedure Write_U8 (W : in out Writer; Value : U8) is
   begin
      Put (W, Value);
   end Write_U8;

   procedure Write_I8 (W : in out Writer; Value : I8) is
   begin
      Put (W, U8'Mod (Value));
   end Write_I8;

   procedure Write_Char (W : in out Writer; Point : U32) is
   begin
      --  7.2.2.2.1.2.2: a CHAR_8_TYPE "shall be representable using 8
      --  bits". A code point above U+00FF is not, and `Point and 16#FF#`
      --  put a different character on the wire with nothing said at
      --  either end. Checked_Char, in this same file, has always given
      --  the answer; the runtime's own entry point did not.
      Put (W, Checked_Char (Point));
   end Write_Char;

   procedure Write_I16 (W : in out Writer; Value : I16) is
   begin
      Write_Int (W, U64 (To_U16 (Value)), 2);
   end Write_I16;

   procedure Write_U16 (W : in out Writer; Value : U16) is
   begin
      Write_Int (W, U64 (Value), 2);
   end Write_U16;

   procedure Write_I32 (W : in out Writer; Value : I32) is
   begin
      Write_Int (W, U64 (To_U32 (Value)), 4);
   end Write_I32;

   procedure Write_U32 (W : in out Writer; Value : U32) is
   begin
      Write_Int (W, U64 (Value), 4);
   end Write_U32;

   procedure Write_I64 (W : in out Writer; Value : I64) is
   begin
      Write_Int (W, To_U64 (Value), 8);
   end Write_I64;

   procedure Write_U64 (W : in out Writer; Value : U64) is
   begin
      Write_Int (W, Value, 8);
   end Write_U64;

   procedure Write_F32 (W : in out Writer; Value : F32) is
   begin
      Write_Int (W, U64 (F32_Bits (Value)), 4);
   end Write_F32;

   procedure Write_F64 (W : in out Writer; Value : F64) is
   begin
      Write_Int (W, F64_Bits (Value), 8);
   end Write_F64;

   procedure Write_F128 (W : in out Writer; Value : Text) is
      pragma Unreferenced (W, Value);
   begin
      raise Wire_Error with
        "long double: binary128 rests with the f128 arc and the Ada runtime "
        & "refuses it by name rather than writing approximate octets";
   end Write_F128;

   --  A 16-bit value in a fixed order whatever the stream's is.
   procedure Write_U16_Little (W : in out Writer; Value : U16) is
   begin
      Align_To (W, 2);
      Put (W, U8 (Value and 16#FF#));
      Put (W, U8 (Interfaces.Shift_Right (Value, 8)));
   end Write_U16_Little;

   procedure Write_U16_Big (W : in out Writer; Value : U16) is
   begin
      Align_To (W, 2);
      Put (W, U8 (Interfaces.Shift_Right (Value, 8)));
      Put (W, U8 (Value and 16#FF#));
   end Write_U16_Big;

   procedure Write_U32_Little (W : in out Writer; Value : U32) is
   begin
      Align_To (W, 4);
      for Index in 0 .. 3 loop
         Put (W, U8 (Interfaces.Shift_Right (Value, 8 * Index) and 16#FF#));
      end loop;
   end Write_U32_Little;

   procedure Write_U32_Big (W : in out Writer; Value : U32) is
   begin
      Align_To (W, 4);
      for Index in reverse 0 .. 3 loop
         Put (W, U8 (Interfaces.Shift_Right (Value, 8 * Index) and 16#FF#));
      end loop;
   end Write_U32_Big;

   procedure Write_Wchar
     (W       : in out Writer;
      Point   : U32;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) is
   begin
      if Width /= 2 and then Width /= 4 then
         raise Wire_Error with "a wchar width of 2 or 4";
      end if;
      if Point > 16#FFFF# then
         raise Wire_Error with "a wchar holding one UTF-16 code unit";
      end if;
      if Framing = Wchar_Giop_Length_Prefixed then
         Put (W, U8 (Width));
         declare
            Big : constant Boolean := Order /= Unit_Stream or else W.Order /= Little;
         begin
            if Big then
               for Index in reverse 0 .. Width - 1 loop
                  Put (W, U8 (Interfaces.Shift_Right (Point, 8 * Index) and 16#FF#));
               end loop;
            else
               for Index in 0 .. Width - 1 loop
                  Put (W, U8 (Interfaces.Shift_Right (Point, 8 * Index) and 16#FF#));
               end loop;
            end if;
         end;
         return;
      end if;
      if Width = 4 then
         Write_U32 (W, Point);
      else
         Write_U16 (W, U16 (Point));
      end if;
   end Write_Wchar;

   procedure Write_String (W : in out Writer; Value : Text) is
      Raw : constant String := SU.To_String (Value);
   begin
      Write_U32 (W, U32 (Raw'Length + 1));
      for Index in Raw'Range loop
         Put (W, U8 (Character'Pos (Raw (Index))));
      end loop;
      Put (W, 0);
   end Write_String;

   procedure Write_String_Bounded (W : in out Writer; Value : Text; Bound : Natural) is
      Octets : constant Natural := Utf8_Octets (Value);
   begin
      if Octets > Bound then
         raise Wire_Error with
           "at most " & Image (Bound) & " octets in a bounded string, and this one has "
           & Image (Octets);
      end if;
      Write_String (W, Value);
   end Write_String_Bounded;

   procedure Check_Sequence_Bound
     (W : in out Writer; Actual : Interfaces.Unsigned_32; Bound : Interfaces.Unsigned_32)
   is
      pragma Unreferenced (W);
   begin
      if Actual > Bound then
         raise Wire_Error with
           "at most " & Image (Natural (Bound)) & " elements in a bounded sequence, and "
           & "this one has " & Image (Natural (Actual));
      end if;
   end Check_Sequence_Bound;

   procedure Check_Map_Bound
     (W : in out Writer; Actual : Interfaces.Unsigned_32; Bound : Interfaces.Unsigned_32)
   is
      pragma Unreferenced (W);
   begin
      if Actual > Bound then
         raise Wire_Error with
           "at most " & Image (Natural (Bound)) & " entries in a bounded map, and this "
           & "one has " & Image (Natural (Actual));
      end if;
   end Check_Map_Bound;

   procedure Write_Wstring_Unit
     (W : in out Writer; Unit : U16; Width : Natural; Order : Unit_Order)
   is
      Fixed_Little : constant Boolean :=
        Order = Unit_Byte_Order_Mark or else Order = Unit_Host_Unswapped;
   begin
      if Width = 4 then
         if Order = Unit_Big_Endian_Always then
            Write_U32_Big (W, U32 (Unit));
         elsif Fixed_Little then
            Write_U32_Little (W, U32 (Unit));
         else
            Write_U32 (W, U32 (Unit));
         end if;
         return;
      end if;
      if Order = Unit_Big_Endian_Always then
         Write_U16_Big (W, Unit);
      elsif Fixed_Little then
         Write_U16_Little (W, Unit);
      else
         Write_U16 (W, Unit);
      end if;
   end Write_Wstring_Unit;

   procedure Write_Wstring_Utf8
     (W : in out Writer; Value : Text; Policy : Wstring_Policy; Order : Unit_Order)
   is
      Cell  : constant Wstring_Cell := Cell_Of (Policy);
      Bare  : constant Unit_Array := Utf16_Of (Value, "wstring");
      Units : Unit_Array (0 .. Bare'Length + 1);
      Count : Natural := 0;
   begin
      if Cell.Counts_Characters and then Cell.Width = 2 then
         --  A character count over two-octet units has no way to carry
         --  a code point outside the BMP.
         for Index in Bare'Range loop
            if Bare (Index) >= 16#D800# and then Bare (Index) <= 16#DFFF# then
               raise Wire_Error with
                 "a code point inside the BMP: a character count over two-octet "
                 & "units has no way to carry one outside it";
            end if;
         end loop;
      end if;
      --  An empty wstring carries no mark.
      if Order = Unit_Byte_Order_Mark and then Bare'Length > 0 then
         Units (Count) := Byte_Order_Mark_Unit;
         Count := Count + 1;
      end if;
      for Index in Bare'Range loop
         Units (Count) := Bare (Index);
         Count := Count + 1;
      end loop;
      if Cell.Terminated then
         Units (Count) := 0;
         Count := Count + 1;
      end if;
      if Cell.Counts_Characters then
         Write_U32 (W, U32 (Count));
      else
         Write_U32 (W, U32 (Count * Cell.Width));
      end if;
      for Index in 0 .. Count - 1 loop
         Write_Wstring_Unit (W, Units (Index), Cell.Width, Order);
      end loop;
   end Write_Wstring_Utf8;

   procedure Write_Wstring_Bounded
     (W      : in out Writer;
      Value  : Text;
      Bound  : Natural;
      Policy : Wstring_Policy;
      Order  : Unit_Order)
   is
      Units : constant Natural := Utf16_Units (Value);
   begin
      if Units > Bound then
         raise Wire_Error with
           "at most " & Image (Bound) & " UTF-16 code units in a bounded wstring, and this "
           & "one has " & Image (Units);
      end if;
      Write_Wstring_Utf8 (W, Value, Policy, Order);
   end Write_Wstring_Bounded;

   procedure Write_Wstring_Utf8_Octets
     (W : in out Writer; Value : Text; Policy : Wstring_Policy)
   is
      Cell   : constant Wstring_Cell := Cell_Of (Policy);
      Raw    : constant String := SU.To_String (Value);
      Length : constant Natural := Raw'Length + (if Cell.Terminated then 1 else 0);
   begin
      --  A value that is not UTF-8 is refused, the same reading the
      --  UTF-16 writer's Decode_Utf8 makes. A Text is not a String, so
      --  this direction has to check what the Rust column's type checked
      --  for it.
      declare
         Points : constant Point_Array := Decode_Utf8 (Raw, "wstring");
         pragma Unreferenced (Points);
      begin
         null;
      end;
      --  The length is the UTF-8 octet count plus the terminator; C1 and
      --  C3 have nothing to say, because a byte with no fixed width has
      --  nothing for a character count to count and no width for an octet
      --  count to multiply by.
      Write_U32 (W, U32 (Length));
      for Index in Raw'Range loop
         Put (W, U8 (Character'Pos (Raw (Index))));
      end loop;
      if Cell.Terminated then
         Put (W, 0);
      end if;
   end Write_Wstring_Utf8_Octets;

   procedure Write_Wstring_Utf8_Octets_Bounded
     (W : in out Writer; Value : Text; Bound : Natural; Policy : Wstring_Policy)
   is
      Units : constant Natural := Utf16_Units (Value);
   begin
      if Units > Bound then
         raise Wire_Error with
           "at most " & Image (Bound) & " UTF-16 code units in a bounded wstring, and this "
           & "one has " & Image (Units);
      end if;
      Write_Wstring_Utf8_Octets (W, Value, Policy);
   end Write_Wstring_Utf8_Octets_Bounded;

   --  -- fixed BCD ----------------------------------------------------

   type Nibble_Array is array (Natural range <>) of Natural;

   --  The nibbles a fixed value packs into, sign included.
   --  [CDR] 9.3.2.8; the digit budget is the type's.
   function Fixed_Nibbles
     (Value       : String;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Nibble_Array
   is
      Negative  : Boolean := False;
      First     : Natural := Value'First;
      Dot       : Natural := 0;

      procedure Refuse is
      begin
         raise Wire_Error with
           "a decimal that fits fixed<" & Image (Digit_Count) & ", "
           & Image (Scale_Count) & ">, not `" & Value & "`";
      end Refuse;
   begin
      if Digit_Count = 0 then
         raise Wire_Error with "a fixed with at least one digit";
      end if;
      --  OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
      --  fixed<31, S> is the widest declaration IDL admits. A wider one
      --  has no packing defined for it, so it is refused rather than
      --  packed into octets no reader can be told the width of.
      if Digit_Count > 31 then
         raise Wire_Error with
           "a fixed of at most 31 digits, not " & Image (Digit_Count);
      end if;
      if Scale_Count > Digit_Count then
         raise Wire_Error with "a scale no larger than the digit count";
      end if;
      if First <= Value'Last and then (Value (First) = '-' or else Value (First) = '+') then
         Negative := Value (First) = '-';
         First := First + 1;
      end if;
      for Index in First .. Value'Last loop
         if Value (Index) = '.' then
            Dot := Index;
            exit;
         end if;
      end loop;
      declare
         Whole_Last  : constant Natural := (if Dot = 0 then Value'Last else Dot - 1);
         Frac_First  : constant Natural := (if Dot = 0 then Value'Last + 1 else Dot + 1);
         Whole_Len   : constant Integer := Whole_Last - First + 1;
         Frac_Len    : constant Integer := Value'Last - Frac_First + 1;
         Integral    : constant Natural := Digit_Count - Scale_Count;
         Lead        : Natural := First;
      begin
         if Whole_Len <= 0 and then Frac_Len <= 0 then
            Refuse;
         end if;
         for Index in First .. Whole_Last loop
            if Value (Index) < '0' or else Value (Index) > '9' then
               Refuse;
            end if;
         end loop;
         for Index in Frac_First .. Value'Last loop
            if Value (Index) < '0' or else Value (Index) > '9' then
               Refuse;
            end if;
         end loop;
         while Lead <= Whole_Last and then Value (Lead) = '0' loop
            Lead := Lead + 1;
         end loop;
         if Whole_Last - Lead + 1 > Integral then
            raise Wire_Error with
              "a decimal whose integer part fits " & Image (Integral)
              & " digits, not `" & Value & "`";
         end if;
         if Frac_Len > Scale_Count then
            raise Wire_Error with
              "a decimal whose fraction fits " & Image (Scale_Count)
              & " digits, not `" & Value & "`";
         end if;
         declare
            Every       : Nibble_Array (0 .. Digit_Count - 1) := (others => 0);
            At_Digit    : Natural := 0;
            Significant : Natural := 0;
            Wire        : Natural;
         begin
            At_Digit := Integral - Natural'Max (0, Whole_Last - Lead + 1);
            for Index in Lead .. Whole_Last loop
               Every (At_Digit) := Character'Pos (Value (Index)) - Character'Pos ('0');
               At_Digit := At_Digit + 1;
            end loop;
            for Index in Frac_First .. Value'Last loop
               Every (At_Digit) := Character'Pos (Value (Index)) - Character'Pos ('0');
               At_Digit := At_Digit + 1;
            end loop;
            for Index in Every'Range loop
               if Every (Index) /= 0 then
                  Significant := Every'Length - Index;
                  exit;
               end if;
            end loop;
            Wire := Every'Length;
            if Count = Fixed_Value_Precision then
               Wire := Natural'Max (1, Significant);
            elsif Count = Fixed_Value_String_Digits then
               --  ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one
               --  leading zero. Past 31 no reader can be told the width.
               declare
                  As_Written : constant Natural :=
                    Natural'Max (0, Whole_Len) + Scale_Count;
               begin
                  Wire := As_Written + (if As_Written mod 2 = 0 then 1 else 0);
               end;
               if Wire > 31 then
                  raise Wire_Error with
                    "a fixed of at most 31 digits on the wire, not "
                    & Image (Wire) & " for `" & Value & "`";
               end if;
            end if;
            declare
               --  A count wider than P is filled with leading zeros.
               Kept   : constant Natural := Natural'Min (Wire, Every'Length);
               Padded : constant Natural := (if (Wire + 1) mod 2 = 1 then 1 else 0);
               Result : Nibble_Array (0 .. Padded + Wire);
               Sign   : Natural := 16#C#;
            begin
               if Negative and then Significant /= 0 then
                  Sign := 16#D#;
               end if;
               for Index in 0 .. Padded + (Wire - Kept) - 1 loop
                  Result (Index) := 0;
               end loop;
               for Index in 0 .. Kept - 1 loop
                  Result (Padded + (Wire - Kept) + Index) :=
                    Every (Every'Length - Kept + Index);
               end loop;
               Result (Result'Last) := Sign;
               return Result;
            end;
         end;
      end;
   end Fixed_Nibbles;

   procedure Write_Fixed
     (W           : in out Writer;
      Value       : Text;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count)
   is
      Nibbles : constant Nibble_Array :=
        Fixed_Nibbles (SU.To_String (Value), Digit_Count, Scale_Count, Count);
      Index   : Natural := Nibbles'First;
   begin
      while Index < Nibbles'Last loop
         Put (W, U8 (Nibbles (Index) * 16 + Nibbles (Index + 1)));
         Index := Index + 2;
      end loop;
   end Write_Fixed;

   --  -- the bit-bounded kinds ----------------------------------------

   procedure Write_Enum (W : in out Writer; Bits : Natural; Value : I64) is
   begin
      if Bits = 0 or else Bits > 32 then
         raise Wire_Error with "an @bit_bound between 1 and 32, not " & Image (Bits);
      end if;
      if Value < -To_I64 (Interfaces.Shift_Left (U64'(1), Bits - 1))
        or else Value > To_I64 (Interfaces.Shift_Left (U64'(1), Bits) - 1)
      then
         raise Wire_Error with
           "an enumerator a " & Image (Bits) & "-bit holder can hold";
      end if;
      declare
         Pattern : constant U64 :=
           To_U64 (Value) and (Interfaces.Shift_Left (U64'(1), Bits) - 1);
      begin
         if Bits <= 8 then
            Write_U8 (W, U8 (Pattern));
         elsif Bits <= 16 then
            Write_U16 (W, U16 (Pattern));
         else
            Write_U32 (W, U32 (Pattern));
         end if;
      end;
   end Write_Enum;

   procedure Write_Bitmask (W : in out Writer; Bits : Natural; Value : U64) is
   begin
      if Bits = 0 or else Bits > 64 then
         raise Wire_Error with "an @bit_bound between 1 and 64, not " & Image (Bits);
      end if;
      if Bits < 64 and then Value >= Interfaces.Shift_Left (U64'(1), Bits) then
         raise Wire_Error with "a bitmask below 2**" & Image (Bits);
      end if;
      if Bits <= 8 then
         Write_U8 (W, U8 (Value));
      elsif Bits <= 16 then
         Write_U16 (W, U16 (Value));
      elsif Bits <= 32 then
         Write_U32 (W, U32 (Value));
      else
         Write_U64 (W, Value);
      end if;
   end Write_Bitmask;

   function Bitfield
     (W        : in out Writer;
      Value    : U64;
      Width    : Natural;
      Position : Natural) return U64
   is
      pragma Unreferenced (W);
   begin
      if Width = 0 or else Width > 64 or else Position + Width > 64 then
         raise Wire_Error with
           "a bitfield of " & Image (Width) & " bits at " & Image (Position)
           & ", inside 64";
      end if;
      if Width < 64 and then Value > Interfaces.Shift_Left (U64'(1), Width) - 1 then
         raise Wire_Error with "a value inside " & Image (Width) & " bits";
      end if;
      return Interfaces.Shift_Left (Value, Position);
   end Bitfield;

   procedure Write_Bitset (W : in out Writer; Anchor : Natural; Value : U64) is
   begin
      case Anchor is
         when 1 => Write_U8 (W, U8 (Value and 16#FF#));
         when 2 => Write_U16 (W, U16 (Value and 16#FFFF#));
         when 4 => Write_U32 (W, U32 (Value and 16#FFFF_FFFF#));
         when 8 => Write_U64 (W, Value);
         when others =>
            raise Wire_Error with
              "a bitset anchor of 1, 2, 4 or 8 bytes, not " & Image (Anchor);
      end case;
   end Write_Bitset;

   procedure Write_Union_Overlay (W : in out Writer; Pattern : U64; Octets : Natural) is
   begin
      case Octets is
         when 1 => Write_U8 (W, U8 (Pattern and 16#FF#));
         when 2 => Write_U16 (W, U16 (Pattern and 16#FFFF#));
         when 4 => Write_U32 (W, U32 (Pattern and 16#FFFF_FFFF#));
         when 8 => Write_U64 (W, Pattern);
         when others =>
            raise Wire_Error with
              "a union arm of 1, 2, 4 or 8 octets, not " & Image (Octets);
      end case;
   end Write_Union_Overlay;

   --  -- the writer's framing -----------------------------------------

   function Write_Dheader (W : in out Writer) return Dheader is
   begin
      Align_To (W, 4);
      declare
         At_Pos : constant Natural := W.Count;
      begin
         for Extra in 1 .. 4 loop
            Put (W, 0);
         end loop;
         return (At_Pos => At_Pos);
      end;
   end Write_Dheader;

   procedure Finish_Dheader (W : in out Writer; Header : Dheader) is
   begin
      if Header.At_Pos < 0 then
         return;
      end if;
      Patch (W, Header.At_Pos, U64 (W.Count - Header.At_Pos - 4), 4);
   end Finish_Dheader;

   function Write_Emheader
     (W               : in out Writer;
      Member_Id       : Natural;
      Length_Code     : Natural;
      Must_Understand : Boolean) return Emheader is
   begin
      --  7.2.2.4.4.4.4 runs member ids from 0 to 16#0FFF_FFFF# and
      --  7.4.3.4.2 leaves them the low 28 bits of the word. Masking a
      --  larger one writes a well-formed header for `Id and 16#0FFF_FFFF#`:
      --  the peer reads a member the caller did not name, and nothing at
      --  either end says so.
      if U32 (Member_Id) > E_Member_Id_Mask then
         raise Wire_Error with
           "a member id inside the 28 bits of an EMHEADER, not "
           & Image (Member_Id);
      end if;
      Align_To (W, 4);
      declare
         Word : U32 :=
           Interfaces.Shift_Left (U32 (Length_Code) and 7, 28)
           or (U32 (Member_Id) and E_Member_Id_Mask);
      begin
         if Must_Understand then
            Word := Word or 16#8000_0000#;
         end if;
         Write_U32 (W, Word);
         declare
            Word_At : constant Natural := W.Count - 4;
         begin
            if Length_Code = 4 then
               declare
                  At_Pos : constant Natural := W.Count;
               begin
                  for Extra in 1 .. 4 loop
                     Put (W, 0);
                  end loop;
                  return (Next_Int => At_Pos, Word => Word_At,
                          Length_Code => Length_Code);
               end;
            end if;
            return (Next_Int => -1, Word => Word_At, Length_Code => Length_Code);
         end;
      end;
   end Write_Emheader;

   procedure Shorten_Emheader (W : in out Writer; Header : Emheader; Length : Natural) is
      Code : U32;
   begin
      case Length is
         when 1      => Code := 0;
         when 2      => Code := 1;
         when 4      => Code := 2;
         when others => Code := 3;
      end case;
      --  The NEXTINT goes; everything after it moves down four.
      for Index in Header.Next_Int .. W.Count - 5 loop
         W.Buffer (Index) := W.Buffer (Index + 4);
      end loop;
      W.Count := W.Count - 4;
      declare
         Previous : constant U32 := Read_Patched (W, Header.Word);
         Cleared  : constant U32 :=
           Previous and not Interfaces.Shift_Left (U32'(7), 28);
      begin
         Patch (W, Header.Word, U64 (Cleared or Interfaces.Shift_Left (Code, 28)), 4);
      end;
   end Shorten_Emheader;

   procedure Finish_Emheader (W : in out Writer; Header : Emheader) is
   begin
      if Header.Next_Int < 0 then
         --  Under codes 0 to 3 the code *is* the length -- 1, 2, 4 or 8
         --  octets, 7.4.3.4.2. A value of another size misdeclares it and
         --  the reader's skip lands inside data, so the mismatch is
         --  refused here rather than written. Codes 5 to 7 declare
         --  nothing of their own: the member's leading length word is the
         --  NEXTINT.
         if Header.Length_Code <= 3 and then Header.Word + 4 <= W.Count then
            declare
               Declared : constant Natural := 2 ** Header.Length_Code;
               Body_Len : constant Natural := W.Count - Header.Word - 4;
            begin
               if Body_Len /= Declared then
                  raise Wire_Error with
                    "a member of " & Image (Declared)
                    & " octets under length code " & Image (Header.Length_Code)
                    & ", not " & Image (Body_Len);
               end if;
            end;
         end if;
         return;
      end if;
      declare
         Length : constant Natural := W.Count - Natural (Header.Next_Int) - 4;
      begin
         if W.Short_Length_Codes
           and then (Length = 1 or else Length = 2 or else Length = 4 or else Length = 8)
         then
            Shorten_Emheader (W, Header, Length);
            return;
         end if;
         Patch (W, Natural (Header.Next_Int), U64 (Length), 4);
      end;
   end Finish_Emheader;

   function Begin_Parameter_Value (W : in out Writer) return Natural is
      Saved : constant Natural := W.Origin;
   begin
      W.Origin := W.Count;
      return Saved;
   end Begin_Parameter_Value;

   --  -- packed collection elements ----------------------------------

   --  The same origin move Begin_Parameter_Value makes, under the name
   --  the shared walk calls for ElementPadding::Packed. Kept apart
   --  because the two callers mean different things: a PL_CDR1 parameter
   --  value is its own block by 7.4.1.2, a collection element under a
   --  packed key is its own block by HDDS's measured departure.
   function Begin_Packed_Element (W : in out Writer) return Natural is
      Saved : constant Natural := W.Origin;
   begin
      W.Origin := W.Count;
      return Saved;
   end Begin_Packed_Element;

   procedure Finish_Packed_Element (W : in out Writer; Saved : Natural) is
   begin
      W.Origin := Saved;
   end Finish_Packed_Element;

   function Write_Pid
     (W               : in out Writer;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write is
   begin
      --  7.4.1.2.2 gives the short form the ids 0 to Highest_Short_Id
      --  inclusive and reserves what is above it -- PID_EXTENDED is
      --  16#3F01# itself. A short header with a reserved id collides with
      --  the protocol's own parameters, so such an id takes the extended
      --  form or none. This is the write side of the field the reader has
      --  always consulted.
      if Pid > W.Framing.Highest_Short_Id then
         raise Wire_Error with
           "a parameter id the short header carries, not " & Image (Pid)
           & " -- the extended header takes it";
      end if;
      Align_To (W, 4);
      declare
         Word : U32 := U32 (Pid);
         Bare : U32;
      begin
         if Must_Understand and then W.Framing.Member_Must_Understand then
            Word := Word or Pid_Must_Understand;
         end if;
         Word := Word or Flags_At (W.Framing.Pid_Flags, Member_Pos);
         Bare := Word and not (Pid_Must_Understand or Pid_Impl_Extension);
         Write_U16 (W, U16 (Word and 16#FFFF#));
         declare
            At_Pos : constant Natural := W.Count;
         begin
            Put (W, 0);
            Put (W, 0);
            return
              (At_Pos          => At_Pos,
               Extended        => False,
               Saved_Origin    => Begin_Parameter_Value (W),
               Role            => Role,
               Pid             => Natural (Bare),
               Must_Understand => Must_Understand);
         end;
      end;
   end Write_Pid;

   function Write_Pid_Extended
     (W               : in out Writer;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write is
   begin
      --  The same 28 bits as an EMHEADER's, here in the eMemberHeader of
      --  7.4.1.2.1. Masking aliases the parameter onto another member
      --  exactly as it does there.
      if U32 (Pid) > E_Member_Id_Mask then
         raise Wire_Error with
           "a member id inside the 28 bits of an eMemberHeader, not "
           & Image (Pid);
      end if;
      Align_To (W, 4);
      declare
         Marker : constant U32 :=
           Pid_Extended_Id or Flags_At (W.Framing.Pid_Flags, Extended_Marker_Pos);
         Member_Header : U32 := U32 (Pid) and E_Member_Id_Mask;
      begin
         Write_U16 (W, U16 (Marker and 16#FFFF#));
         Write_U16 (W, 8);
         if Must_Understand and then W.Framing.Member_Must_Understand then
            Member_Header := Member_Header or E_Flag_Must_Understand;
         end if;
         Write_U32 (W, Member_Header);
         declare
            At_Pos : constant Natural := W.Count;
         begin
            for Extra in 1 .. 4 loop
               Put (W, 0);
            end loop;
            return
              (At_Pos          => At_Pos,
               Extended        => True,
               Saved_Origin    => Begin_Parameter_Value (W),
               Role            => Role,
               Pid             => 0,
               Must_Understand => False);
         end;
      end;
   end Write_Pid_Extended;

   procedure Promote_Pid (W : in out Writer; Header : Pid_Write; Length : Natural) is
      Start     : constant Natural := Header.At_Pos - 2;
      Value_At  : constant Natural := Header.At_Pos + 2;
      Old_Count : constant Natural := W.Count;
   begin
      --  Room for the eight extra octets of the extended form, then the
      --  value moves up to make the gap after the short header.
      for Extra in 1 .. 8 loop
         Put (W, 0);
      end loop;
      for Index in reverse Value_At .. Old_Count - 1 loop
         W.Buffer (Index + 8) := W.Buffer (Index);
      end loop;
      declare
         Marker : constant U32 :=
           Pid_Extended_Id or Flags_At (W.Framing.Pid_Flags, Extended_Marker_Pos);
         Member_Header : U32 := U32 (Header.Pid) and E_Member_Id_Mask;
      begin
         if Header.Must_Understand and then W.Framing.Member_Must_Understand then
            Member_Header := Member_Header or E_Flag_Must_Understand;
         end if;
         Patch (W, Start, U64 (Marker), 2);
         Patch (W, Start + 2, 8, 2);
         Patch (W, Start + 4, U64 (Member_Header), 4);
         Patch (W, Start + 8, U64 (Length), 4);
      end;
   end Promote_Pid;

   procedure Finish_Pid (W : in out Writer; Header : Pid_Write) is
      Width  : constant Natural := (if Header.Extended then 4 else 2);
      Length : Natural := W.Count - Header.At_Pos - Width;
      Pads   : constant Boolean :=
        (if Header.Role = Role_Optional then W.Framing.Pad_Declared_Optional_Length
         else W.Framing.Pad_Declared_Length);
   begin
      if Pads then
         declare
            Past : constant Natural := (W.Count - Header.Saved_Origin) mod 4;
         begin
            if Past /= 0 then
               Length := Length + (4 - Past);
            end if;
         end;
      end if;
      if not Header.Extended and then Length > Pid_Short_Max_Length then
         Promote_Pid (W, Header, Length);
      else
         Patch (W, Header.At_Pos, U64 (Length), Width);
      end if;
      W.Origin := Header.Saved_Origin;
      --  Under Fast DDS's framing the parameter writes no padding of
      --  its own -- the next header aligns itself, and a final
      --  parameter ends the stream where its value does.
      if W.Framing.Pad_After_Value then
         Align_To (W, 4);
      end if;
   end Finish_Pid;

   procedure Write_Sentinel (W : in out Writer) is
      Sentinel : constant U32 :=
        Pid_List_End_Id or Flags_At (W.Framing.Pid_Flags, Sentinel_Pos);
   begin
      Align_To (W, 4);
      Write_U16 (W, U16 (Sentinel and 16#FFFF#));
      Write_U16 (W, 0);
   end Write_Sentinel;

   --  -- the reader ---------------------------------------------------

   function New_Reader
     (Data      : Byte_Array_Access;
      Order     : Byte_Order;
      Max_Align : Natural;
      Framing   : Pl_Cdr1_Framing) return Reader is
   begin
      return
        (Data      => Data,
         At_Pos    => 0,
         Order     => Order,
         Max_Align => Natural'Max (1, Max_Align),
         Origin    => 0,
         Framing   => Framing,
         Current   => Status_Ok);
   end New_Reader;

   function Data_Length (R : Reader) return Natural is
   begin
      if R.Data = null then
         return 0;
      end if;
      return R.Data'Length;
   end Data_Length;

   function Reader_Status (R : in out Reader) return Status is
   begin
      return R.Current;
   end Reader_Status;

   procedure Reader_Refuse (R : in out Reader; Refusal : Status) is
   begin
      if Refusal /= Status_Ok and then R.Current = Status_Ok then
         R.Current := Refusal;
      end if;
   end Reader_Refuse;

   function Reader_Clear_Discard (R : in out Reader) return Boolean is
   begin
      if R.Current = Status_Discarded then
         R.Current := Status_Ok;
         return True;
      end if;
      return False;
   end Reader_Clear_Discard;

   function Reader_Position (R : in out Reader) return Natural is
   begin
      return R.At_Pos;
   end Reader_Position;

   function Reader_Remaining (R : in out Reader) return Natural is
      Whole : constant Natural := Data_Length (R);
   begin
      if R.At_Pos > Whole then
         return 0;
      end if;
      return Whole - R.At_Pos;
   end Reader_Remaining;

   procedure Reader_Skip_To (R : in out Reader; Target : Natural) is
   begin
      if R.Current /= Status_Ok then
         return;
      end if;
      if Target < R.At_Pos then
         Reader_Refuse (R, Status_Out_Of_Range);
         return;
      end if;
      if Target > Data_Length (R) then
         raise Wire_Error with
           "at " & Image (R.At_Pos) & ": " & Image (Target - R.At_Pos)
           & " bytes needed, " & Image (Reader_Remaining (R)) & " available";
      end if;
      R.At_Pos := Target;
   end Reader_Skip_To;

   function Reader_Save (R : in out Reader) return Reader_Mark is
   begin
      return (At_Pos => R.At_Pos, Origin => R.Origin, Current => R.Current);
   end Reader_Save;

   procedure Reader_Restore (R : in out Reader; Mark : Reader_Mark) is
   begin
      R.At_Pos := Mark.At_Pos;
      R.Origin := Mark.Origin;
      R.Current := Mark.Current;
   end Reader_Restore;

   --  The read-side pair of Begin_Packed_Element.
   function Reader_Begin_Packed_Element (R : in out Reader) return Natural is
      Saved : constant Natural := R.Origin;
   begin
      R.Origin := R.At_Pos;
      return Saved;
   end Reader_Begin_Packed_Element;

   procedure Reader_Finish_Packed_Element (R : in out Reader; Saved : Natural) is
   begin
      R.Origin := Saved;
   end Reader_Finish_Packed_Element;

   procedure Reader_Align (R : in out Reader; Alignment : Natural) is
      Effective : constant Natural := Natural'Min (Alignment, R.Max_Align);
   begin
      if Effective <= 1 then
         return;
      end if;
      declare
         Offset : constant Natural := (R.At_Pos - R.Origin) mod Effective;
      begin
         if Offset /= 0 then
            R.At_Pos := R.At_Pos + (Effective - Offset);
         end if;
      end;
   end Reader_Align;

   --  One octet off the stream; a short stream raises.
   function Take (R : in out Reader) return U8 is
   begin
      if R.At_Pos >= Data_Length (R) then
         raise Wire_Error with
           "at " & Image (R.At_Pos) & ": 1 byte needed, 0 available";
      end if;
      declare
         Octet : constant U8 := R.Data (R.Data'First + R.At_Pos);
      begin
         R.At_Pos := R.At_Pos + 1;
         return Octet;
      end;
   end Take;

   procedure Need (R : in out Reader; Count : Natural) is
   begin
      if R.At_Pos + Count > Data_Length (R) then
         raise Wire_Error with
           "at " & Image (R.At_Pos) & ": " & Image (Count) & " bytes needed, "
           & Image (Reader_Remaining (R)) & " available";
      end if;
   end Need;

   function Read_Unsigned (R : in out Reader; Width : Natural) return U64 is
      Value : U64 := 0;
   begin
      Reader_Align (R, Width);
      Need (R, Width);
      if R.Order = Little then
         for Index in reverse 0 .. Width - 1 loop
            Value :=
              Interfaces.Shift_Left (Value, 8)
              or U64 (R.Data (R.Data'First + R.At_Pos + Index));
         end loop;
      else
         for Index in 0 .. Width - 1 loop
            Value :=
              Interfaces.Shift_Left (Value, 8)
              or U64 (R.Data (R.Data'First + R.At_Pos + Index));
         end loop;
      end if;
      R.At_Pos := R.At_Pos + Width;
      return Value;
   end Read_Unsigned;

   function Read_Bool (R : in out Reader) return Boolean is
      Where : constant Natural := R.At_Pos;
      Value : constant U8 := Take (R);
   begin
      if Value = 0 then
         return False;
      elsif Value = 1 then
         return True;
      end if;
      raise Wire_Error with "at " & Image (Where) & ": a boolean of 0 or 1";
   end Read_Bool;

   function Read_U8 (R : in out Reader) return U8 is
   begin
      return Take (R);
   end Read_U8;

   function Read_I8 (R : in out Reader) return I8 is
   begin
      return To_I8 (Take (R));
   end Read_I8;

   function Read_Char (R : in out Reader) return U32 is
   begin
      return U32 (Take (R));
   end Read_Char;

   function Read_I16 (R : in out Reader) return I16 is
   begin
      return To_I16 (U16 (Read_Unsigned (R, 2)));
   end Read_I16;

   function Read_U16 (R : in out Reader) return U16 is
   begin
      return U16 (Read_Unsigned (R, 2));
   end Read_U16;

   function Read_I32 (R : in out Reader) return I32 is
   begin
      return To_I32 (U32 (Read_Unsigned (R, 4)));
   end Read_I32;

   function Read_U32 (R : in out Reader) return U32 is
   begin
      return U32 (Read_Unsigned (R, 4));
   end Read_U32;

   function Read_I64 (R : in out Reader) return I64 is
   begin
      return To_I64 (Read_Unsigned (R, 8));
   end Read_I64;

   function Read_U64 (R : in out Reader) return U64 is
   begin
      return Read_Unsigned (R, 8);
   end Read_U64;

   function Read_F32 (R : in out Reader) return F32 is
   begin
      return Bits_F32 (U32 (Read_Unsigned (R, 4)));
   end Read_F32;

   function Read_F64 (R : in out Reader) return F64 is
   begin
      return Bits_F64 (Read_Unsigned (R, 8));
   end Read_F64;

   function Read_F128 (R : in out Reader) return Text is
      pragma Unreferenced (R);
   begin
      raise Wire_Error with
        "long double: binary128 rests with the f128 arc and the Ada runtime "
        & "refuses it by name rather than answering approximate text";
      return SU.Null_Unbounded_String;
   end Read_F128;

   --  A fixed-order 16- or 32-bit unit, for the wstring orders.
   function Read_U16_Little (R : in out Reader) return U16 is
   begin
      Reader_Align (R, 2);
      declare
         Low  : constant U8 := Take (R);
         High : constant U8 := Take (R);
      begin
         return U16 (Low) or Interfaces.Shift_Left (U16 (High), 8);
      end;
   end Read_U16_Little;

   function Read_U16_Big (R : in out Reader) return U16 is
   begin
      Reader_Align (R, 2);
      declare
         High : constant U8 := Take (R);
         Low  : constant U8 := Take (R);
      begin
         return Interfaces.Shift_Left (U16 (High), 8) or U16 (Low);
      end;
   end Read_U16_Big;

   function Read_U32_Little (R : in out Reader) return U32 is
      Value : U32 := 0;
   begin
      Reader_Align (R, 4);
      for Index in 0 .. 3 loop
         Value := Value or Interfaces.Shift_Left (U32 (Take (R)), 8 * Index);
      end loop;
      return Value;
   end Read_U32_Little;

   function Read_U32_Big (R : in out Reader) return U32 is
      Value : U32 := 0;
   begin
      Reader_Align (R, 4);
      for Index in 0 .. 3 loop
         Value := Interfaces.Shift_Left (Value, 8) or U32 (Take (R));
      end loop;
      return Value;
   end Read_U32_Big;

   function Read_Wchar
     (R       : in out Reader;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) return U32
   is
      Where : constant Natural := R.At_Pos;
      Value : U32 := 0;
   begin
      if Width /= 2 and then Width /= 4 then
         raise Wire_Error with "a wchar width of 2 or 4";
      end if;
      if Framing = Wchar_Giop_Length_Prefixed then
         declare
            Declared : constant U8 := Read_U8 (R);
            Big      : constant Boolean :=
              Order /= Unit_Stream or else R.Order /= Little;
         begin
            if Natural (Declared) /= Width then
               raise Wire_Error with
                 "at " & Image (Where) & ": a GIOP 1.2 wchar of " & Image (Width)
                 & " octets, the configured transmission code set, and not "
                 & Image (Natural (Declared));
            end if;
            Need (R, Width);
            if Big then
               for Index in 1 .. Width loop
                  Value := Interfaces.Shift_Left (Value, 8) or U32 (Take (R));
               end loop;
            else
               for Index in 0 .. Width - 1 loop
                  Value := Value or Interfaces.Shift_Left (U32 (Take (R)), 8 * Index);
               end loop;
            end if;
         end;
      elsif Width = 4 then
         Value := Read_U32 (R);
         if Value > 16#FFFF# then
            raise Wire_Error with
              "at " & Image (Where) & ": a four-byte wchar holding one UTF-16 code unit";
         end if;
      else
         Value := U32 (Read_U16 (R));
      end if;
      if Value >= 16#D800# and then Value <= 16#DFFF# then
         raise Wire_Error with "at " & Image (Where) & ": a Unicode code point";
      end if;
      return Value;
   end Read_Wchar;

   function Read_String (R : in out Reader) return Text is
      Where  : constant Natural := R.At_Pos;
      Length : constant Natural := Natural (Read_U32 (R));
   begin
      if Length = 0 or else Length > Reader_Remaining (R) + 1 then
         raise Wire_Error with
           "at " & Image (Where) & ": a length of " & Image (Length) & " with "
           & Image (Reader_Remaining (R)) & " bytes left";
      end if;
      declare
         Value : Text := SU.Null_Unbounded_String;
      begin
         for Index in 1 .. Length - 1 loop
            SU.Append (Value, Character'Val (Natural (Take (R))));
         end loop;
         declare
            Points : constant Point_Array :=
              Decode_Utf8 (SU.To_String (Value), "at " & Image (Where));
            pragma Unreferenced (Points);
         begin
            null;
         end;
         Discard_U32 (U32 (Read_U8 (R)));
         return Value;
      end;
   end Read_String;

   function Read_String_Alloc (R : in out Reader) return Text is
   begin
      return Read_String (R);
   end Read_String_Alloc;

   function Read_Wstring_Utf8
     (R : in out Reader; Policy : Wstring_Policy; Order : Unit_Order) return Text
   is
      Where    : constant Natural := R.At_Pos;
      Declared : constant Natural := Natural (Read_U32 (R));
      Cell     : constant Wstring_Cell := Cell_Of (Policy);
      Count    : Natural := Declared;
      Fixed_Little : constant Boolean :=
        Order = Unit_Byte_Order_Mark or else Order = Unit_Host_Unswapped;
   begin
      if not Cell.Counts_Characters then
         if Declared mod Cell.Width /= 0 then
            raise Wire_Error with
              "at " & Image (Where) & ": a length of " & Image (Declared)
              & " with " & Image (Reader_Remaining (R)) & " bytes left";
         end if;
         Count := Declared / Cell.Width;
      end if;
      if Count * Cell.Width > Reader_Remaining (R) then
         raise Wire_Error with
           "at " & Image (Where) & ": a length of " & Image (Count * Cell.Width)
           & " with " & Image (Reader_Remaining (R)) & " bytes left";
      end if;
      declare
         Units      : Unit_Array (0 .. Count);
         Have       : Natural := 0;
         First_Unit : Natural := 0;
         Swap_Rest  : Boolean := False;
      begin
         for Index in 0 .. Count - 1 loop
            if Cell.Width = 4 then
               declare
                  Wide : U32;
               begin
                  if Order = Unit_Big_Endian_Always then
                     Wide := Read_U32_Big (R);
                  elsif Fixed_Little then
                     Wide := Read_U32_Little (R);
                  else
                     Wide := Read_U32 (R);
                  end if;
                  if Wide > 16#FFFF# then
                     raise Wire_Error with
                       "at " & Image (Where)
                       & ": a four-byte wstring character holding one UTF-16 unit";
                  end if;
                  Units (Have) := U16 (Wide);
               end;
            else
               if Order = Unit_Big_Endian_Always then
                  Units (Have) := Read_U16_Big (R);
               elsif Fixed_Little then
                  Units (Have) := Read_U16_Little (R);
               else
                  Units (Have) := Read_U16 (R);
               end if;
            end if;
            Have := Have + 1;
         end loop;
         if Cell.Terminated then
            if Declared < 1 or else Have = 0 then
               raise Wire_Error with
                 "at " & Image (Where) & ": a length of " & Image (Declared)
                 & " with " & Image (Reader_Remaining (R)) & " bytes left";
            end if;
            if Units (Have - 1) /= 0 then
               raise Wire_Error with
                 "at " & Image (Where)
                 & ": a NUL character ending the wstring, as its count includes one";
            end if;
            Have := Have - 1;
            for Index in 0 .. Have - 1 loop
               if Units (Index) = 0 then
                  raise Wire_Error with
                    "at " & Image (Where)
                    & ": no NUL inside a wstring a C peer would stop at";
               end if;
            end loop;
         end if;
         if Order = Unit_Byte_Order_Mark and then Have > 0 then
            if Units (0) = Byte_Order_Mark_Unit then
               First_Unit := 1;
            elsif Units (0) = Swapped_Byte_Order_Mark_Unit then
               First_Unit := 1;
               Swap_Rest := True;
            else
               Swap_Rest := True;
            end if;
         end if;
         if Swap_Rest then
            for Index in First_Unit .. Have - 1 loop
               Units (Index) :=
                 Interfaces.Shift_Left (Units (Index) and 16#FF#, 8)
                 or Interfaces.Shift_Right (Units (Index), 8);
            end loop;
         end if;
         declare
            Built : Text := SU.Null_Unbounded_String;
            Index : Natural := First_Unit;
         begin
            while Index < Have loop
               declare
                  Unit : constant U16 := Units (Index);
               begin
                  if Unit >= 16#D800# and then Unit <= 16#DBFF#
                    and then Index + 1 < Have
                    and then Units (Index + 1) >= 16#DC00#
                    and then Units (Index + 1) <= 16#DFFF#
                  then
                     Append_Point
                       (Built,
                        16#1_0000#
                        + Interfaces.Shift_Left (U32 (Unit) - 16#D800#, 10)
                        + (U32 (Units (Index + 1)) - 16#DC00#));
                     Index := Index + 2;
                  elsif Unit >= 16#D800# and then Unit <= 16#DFFF# then
                     raise Wire_Error with
                       "at " & Image (Where)
                       & ": a UTF-16 sequence with no lone surrogate";
                  else
                     Append_Point (Built, U32 (Unit));
                     Index := Index + 1;
                  end if;
               end;
            end loop;
            return Built;
         end;
      end;
   end Read_Wstring_Utf8;

   function Read_Wstring_Utf8_Octets
     (R : in out Reader; Policy : Wstring_Policy) return Text
   is
      Where    : constant Natural := R.At_Pos;
      Declared : constant Natural := Natural (Read_U32 (R));
      Cell     : constant Wstring_Cell := Cell_Of (Policy);
   begin
      if Declared > Reader_Remaining (R) then
         raise Wire_Error with
           "at " & Image (Where) & ": a length of " & Image (Declared)
           & " with " & Image (Reader_Remaining (R)) & " bytes left";
      end if;
      declare
         Raw   : String (1 .. Declared);
         Value : Text := SU.Null_Unbounded_String;
      begin
         for Index in 1 .. Declared loop
            Raw (Index) := Character'Val (Natural (Take (R)));
         end loop;
         if Cell.Terminated then
            if Declared = 0 or else Raw (Declared) /= Character'Val (0) then
               raise Wire_Error with
                 "at " & Image (Where)
                 & ": a NUL octet ending the wstring, as its count includes one";
            end if;
            --  A NUL inside the text is the narrow string's problem in
            --  wide form: every C peer stops at it, so the value this
            --  reader returns and the value the peer returns would
            --  differ for bytes both accepted.
            for Index in 1 .. Declared - 1 loop
               if Raw (Index) = Character'Val (0) then
                  raise Wire_Error with
                    "at " & Image (Where)
                    & ": no NUL inside a wstring a C peer would stop at";
               end if;
            end loop;
            Value := SU.To_Unbounded_String (Raw (1 .. Declared - 1));
         else
            Value := SU.To_Unbounded_String (Raw);
         end if;
         --  The declared length is the UTF-8 octet count: a value that
         --  is not UTF-8 is refused rather than carried in a Text no
         --  reader could tell was never a wstring.
         declare
            Points : constant Point_Array :=
              Decode_Utf8 (SU.To_String (Value), "at " & Image (Where));
            pragma Unreferenced (Points);
         begin
            null;
         end;
         return Value;
      end;
   end Read_Wstring_Utf8_Octets;

   function Read_Fixed
     (R           : in out Reader;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Text
   is
      Nibbles : Nibble_Array (0 .. 63);
      Have    : Natural := 0;
      Sign    : Natural;
   begin
      --  The same bound as the writer's, on this side too: OMG IDL 4.2
      --  rule (42) caps the BCD rule at 31 digits, and a reader asked for
      --  a wider field would take octets no declaration can name.
      if Digit_Count = 0 or else Digit_Count > 31
        or else Scale_Count > Digit_Count
      then
         raise Wire_Error with
           "at " & Image (R.At_Pos)
           & ": a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<"
           & Image (Digit_Count) & ", " & Image (Scale_Count) & ">";
      end if;
      if Count /= Fixed_Declared then
         loop
            if Have >= 32 then
               raise Wire_Error with
                 "at " & Image (R.At_Pos) & ": a sign nibble within sixteen octets";
            end if;
            declare
               Octet : constant U8 := Take (R);
            begin
               Nibbles (Have) := Natural (Interfaces.Shift_Right (Octet, 4));
               Nibbles (Have + 1) := Natural (Octet and 16#0F#);
               Have := Have + 2;
               exit when (Octet and 16#0F#) >= 16#0A#;
            end;
         end loop;
      else
         declare
            Octets : constant Natural := (Digit_Count + 2) / 2;
         begin
            for Index in 1 .. Octets loop
               declare
                  Octet : constant U8 := Take (R);
               begin
                  Nibbles (Have) := Natural (Interfaces.Shift_Right (Octet, 4));
                  Nibbles (Have + 1) := Natural (Octet and 16#0F#);
                  Have := Have + 2;
               end;
            end loop;
         end;
      end if;
      Sign := Nibbles (Have - 1);
      Have := Have - 1;
      declare
         First_Digit : Natural := 0;
      begin
         if Count = Fixed_Declared and then (Digit_Count + 1) mod 2 = 1 then
            First_Digit := 1;
         end if;
         for Index in First_Digit .. Have - 1 loop
            if Nibbles (Index) > 9 then
               raise Wire_Error with "at " & Image (R.At_Pos) & ": BCD digits";
            end if;
         end loop;
         declare
            Digits_Text : String (1 .. Have - First_Digit);
            Zero        : Boolean := True;
            Split       : Natural;
            Lead        : Natural := 1;
            Result      : Text := SU.Null_Unbounded_String;
         begin
            for Index in Digits_Text'Range loop
               Digits_Text (Index) :=
                 Character'Val
                   (Character'Pos ('0') + Nibbles (First_Digit + Index - 1));
            end loop;
            declare
               Padded : constant String :=
                 (if Digits_Text'Length < Scale_Count + 1
                  then Zeros (Scale_Count + 1 - Digits_Text'Length) & Digits_Text
                  else Digits_Text);
            begin
               Split := Padded'Length - Scale_Count;
               while Lead < Split and then Padded (Lead) = '0' loop
                  Lead := Lead + 1;
               end loop;
               for Index in Lead .. Split loop
                  if Padded (Index) /= '0' then
                     Zero := False;
                  end if;
               end loop;
               for Index in Split + 1 .. Padded'Length loop
                  if Padded (Index) /= '0' then
                     Zero := False;
                  end if;
               end loop;
               if Sign = 16#0D# and then not Zero then
                  SU.Append (Result, '-');
               end if;
               SU.Append (Result, Padded (Lead .. Split));
               if Scale_Count > 0 then
                  SU.Append (Result, '.');
                  SU.Append (Result, Padded (Split + 1 .. Padded'Length));
               end if;
               return Result;
            end;
         end;
      end;
   end Read_Fixed;

   function Trim_Utf8 (R : in out Reader; Value : Text; Octets : Natural) return Text is
      pragma Unreferenced (R);
      Points : constant Point_Array :=
        Decode_Utf8 (SU.To_String (Value), "trim-utf8");
      Kept   : Text := SU.Null_Unbounded_String;
      Used   : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant U32 := Points (Index);
            Cost  : constant Natural :=
              (if Point < 16#80# then 1
               elsif Point < 16#800# then 2
               elsif Point < 16#1_0000# then 3
               else 4);
         begin
            exit when Used + Cost > Octets;
            Append_Point (Kept, Point);
            Used := Used + Cost;
         end;
      end loop;
      return Kept;
   end Trim_Utf8;

   function Trim_Utf16 (R : in out Reader; Value : Text; Units : Natural) return Text is
      pragma Unreferenced (R);
      Points : constant Point_Array :=
        Decode_Utf8 (SU.To_String (Value), "trim-utf16");
      Kept   : Text := SU.Null_Unbounded_String;
      Used   : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant U32 := Points (Index);
            Cost  : constant Natural := (if Point > 16#FFFF# then 2 else 1);
         begin
            exit when Used + Cost > Units;
            Append_Point (Kept, Point);
            Used := Used + Cost;
         end;
      end loop;
      return Kept;
   end Trim_Utf16;

   function Read_Enum (R : in out Reader; Bits : Natural) return I64 is
   begin
      if Bits = 0 or else Bits > 32 then
         raise Wire_Error with "an @bit_bound between 1 and 32, not " & Image (Bits);
      end if;
      if Bits <= 8 then
         return I64 (To_I8 (Read_U8 (R)));
      elsif Bits <= 16 then
         return I64 (To_I16 (Read_U16 (R)));
      end if;
      return I64 (To_I32 (Read_U32 (R)));
   end Read_Enum;

   function Read_Bitmask (R : in out Reader; Bits : Natural) return U64 is
   begin
      if Bits = 0 or else Bits > 64 then
         raise Wire_Error with "an @bit_bound between 1 and 64, not " & Image (Bits);
      end if;
      if Bits <= 8 then
         return U64 (Read_U8 (R));
      elsif Bits <= 16 then
         return U64 (Read_U16 (R));
      elsif Bits <= 32 then
         return U64 (Read_U32 (R));
      end if;
      return Read_U64 (R);
   end Read_Bitmask;

   function Read_Bitset (R : in out Reader; Anchor : Natural) return U64 is
   begin
      case Anchor is
         when 1 => return U64 (Read_U8 (R));
         when 2 => return U64 (Read_U16 (R));
         when 4 => return U64 (Read_U32 (R));
         when 8 => return Read_U64 (R);
         when others =>
            raise Wire_Error with
              "a bitset anchor of 1, 2, 4 or 8 bytes, not " & Image (Anchor);
      end case;
   end Read_Bitset;

   function Read_Dheader (R : in out Reader) return U32 is
      Where  : constant Natural := R.At_Pos;
      Length : constant U32 := Read_U32 (R);
   begin
      if Length > U32 (Reader_Remaining (R)) then
         raise Wire_Error with
           "at " & Image (Where) & ": a length of " & Image (Natural (Length))
           & " with " & Image (Reader_Remaining (R)) & " bytes left";
      end if;
      return Length;
   end Read_Dheader;

   function Em_Extent (Header : Emheader_Read) return Natural is
   begin
      if Header.Length_Code <= 3 then
         return 2 ** Header.Length_Code;
      end if;
      if not Header.Has_Next_Int then
         return 0;
      end if;
      if Header.Length_Code = 4 then
         return Natural (Header.Next_Int);
      end if;
      declare
         Scale : constant Natural :=
           (if Header.Length_Code = 6 then 4 elsif Header.Length_Code = 7 then 8 else 1);
      begin
         return 4 + Natural (Header.Next_Int) * Scale;
      end;
   end Em_Extent;

   function Read_Emheader (R : in out Reader) return Emheader_Read is
      Word   : constant U32 := Read_U32 (R);
      Header : Emheader_Read;
   begin
      Header.Member_Id := Word and E_Member_Id_Mask;
      Header.Length_Code := Natural (Interfaces.Shift_Right (Word, 28) and 7);
      Header.Must_Understand := (Word and 16#8000_0000#) /= 0;
      Header.Has_Next_Int := False;
      if Header.Length_Code = 4 then
         Header.Next_Int := Read_U32 (R);
         Header.Has_Next_Int := True;
      elsif Header.Length_Code >= 5 then
         declare
            Where : constant Natural := R.At_Pos;
         begin
            Header.Next_Int := Read_U32 (R);
            Header.Has_Next_Int := True;
            R.At_Pos := Where;
         end;
      end if;
      Header.Extent := Em_Extent (Header);
      return Header;
   end Read_Emheader;

   function Read_Pid_Header (R : in out Reader; Role : Parameter_Role) return Pid_Read is
      Header : Pid_Read;
   begin
      Reader_Align (R, 4);
      declare
         Word     : constant U32 := U32 (Read_U16 (R));
         Declared : constant U32 := U32 (Read_U16 (R));
      begin
         if (Word and Pid_Id_Mask) = (Pid_Extended_Id and Pid_Id_Mask) then
            declare
               Member_Header : constant U32 := Read_U32 (R);
               Wide_Length   : constant U32 := Read_U32 (R);
            begin
               Header.Pid := Member_Header and E_Member_Id_Mask;
               Header.Length := Wide_Length;
               Header.End_Flag := False;
               Header.Must_Understand := (Member_Header and E_Flag_Must_Understand) /= 0;
               --  FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15
               --  is in the short header; the extended id is 28 bits
               --  and has no reserved range.
               Header.Impl_Extension := (Member_Header and E_Flag_Impl_Extension) /= 0;
               Header.Reserved := False;
               Header.Ignore := False;
               Header.Role := Role;
               return Header;
            end;
         end if;
         declare
            Identifier : constant U32 := Word and Pid_Id_Mask;
         begin
            Header.Pid := Identifier;
            Header.Length := Declared;
            Header.End_Flag := Identifier = (Pid_List_End_Id and Pid_Id_Mask);
            Header.Must_Understand := (Word and Pid_Must_Understand) /= 0;
            --  Under a framing that puts FLAG_IMPL_EXTENSION on every
            --  member id the bit carries no information and is reported
            --  absent.
            Header.Impl_Extension :=
              (Word and Pid_Impl_Extension) /= 0
              and then (Flags_At (R.Framing.Pid_Flags, Member_Pos) and Pid_Impl_Extension) = 0;
            Header.Reserved :=
              not Header.End_Flag
              and then Identifier > U32 (R.Framing.Highest_Short_Id);
            Header.Ignore := Identifier = Pid_Ignore_Id;
            Header.Role := Role;
            return Header;
         end;
      end;
   end Read_Pid_Header;

   function Read_Pid_As (R : in out Reader; Role : Parameter_Role) return Pid_Read is
      Header : Pid_Read := Read_Pid_Header (R, Role);
   begin
      --  The parameter's value is its own stream block: 7.4.1.2 counts
      --  a member's internal alignment from the first octet after the
      --  header. Finish_Pid_Read puts the enclosing origin back.
      Header.Saved_Origin := R.Origin;
      Header.Value_At := R.At_Pos;
      R.Origin := R.At_Pos;
      return Header;
   end Read_Pid_As;

   function Read_Pid (R : in out Reader) return Pid_Read is
   begin
      return Read_Pid_As (R, Role_Member);
   end Read_Pid;

   procedure Finish_Pid_Read (R : in out Reader; Header : Pid_Read) is
   begin
      R.Origin := Header.Saved_Origin;
      if R.Current /= Status_Ok or else Header.End_Flag then
         return;
      end if;
      declare
         Used     : constant Natural := R.At_Pos - Header.Value_At;
         Declared : constant U32 := Header.Length;
         Padded   : constant Natural := ((Used + 3) / 4) * 4;
         Pads     : constant Boolean :=
           (if Header.Role = Role_Optional then R.Framing.Pad_Declared_Optional_Length
            else R.Framing.Pad_Declared_Length);
         Expected : constant Natural := (if Pads then Padded else Used);
      begin
         --  Both readings of the same value are accepted where they
         --  coincide, which is every value already a multiple of four.
         if Declared /= U32 (Expected)
           and then Declared /= U32 (Used)
           and then Declared /= U32 (Padded)
         then
            Reader_Refuse (R, Status_Out_Of_Range);
         end if;
      end;
      --  The padding after a value belongs to the framing that wrote
      --  one; under Fast DDS's the next header aligns itself and a
      --  final parameter ends the stream where its value does.
      if R.Framing.Pad_After_Value then
         Reader_Align (R, 4);
      end if;
   end Finish_Pid_Read;

   --  -- the JSON the probes print ------------------------------------

   function Json_Bool (Value : Boolean) return Text is
   begin
      return To_Text (if Value then "true" else "false");
   end Json_Bool;

   function Json_Int (Value : I64) return Text is
      Raw : constant String := I64'Image (Value);
   begin
      if Raw (Raw'First) = ' ' then
         return To_Text (Raw (Raw'First + 1 .. Raw'Last));
      end if;
      return To_Text (Raw);
   end Json_Int;

   function Json_I64 (Value : I64) return Text is
      Bare : constant Text := Json_Int (Value);
   begin
      return To_Text ("""" & SU.To_String (Bare) & """");
   end Json_I64;

   function Json_U64 (Value : U64) return Text is
      Raw : constant String := U64'Image (Value);
      First : Natural := Raw'First;
   begin
      if Raw (First) = ' ' then
         First := First + 1;
      end if;
      return To_Text ("""" & Raw (First .. Raw'Last) & """");
   end Json_U64;

   function Json_F64 (Value : F64; Where : String) return Text is
   begin
      if Value /= Value or else Value > F64'Last or else Value < F64'First then
         raise Wire_Error with Where & ": the value has no JSON form";
      end if;
      declare
         Line  : String (1 .. 40);
      begin
         --  Seventeen significant digits: one ahead of the point and
         --  sixteen after, which is what makes a binary64 read back as
         --  the value that was printed.
         F64_IO.Put (To => Line, Item => Value, Aft => 16, Exp => 3);
         declare
            First : Natural := Line'First;
         begin
            while First <= Line'Last and then Line (First) = ' ' loop
               First := First + 1;
            end loop;
            return To_Text (Line (First .. Line'Last));
         end;
      end;
   end Json_F64;

   function Json_F32 (Value : F32; Where : String) return Text is
   begin
      return Json_F64 (F64 (Value), Where);
   end Json_F32;

   function Json_Char (Point : U32) return Text is
      Bare : Text := SU.Null_Unbounded_String;
   begin
      Append_Point (Bare, Point);
      return Json_String (Bare);
   end Json_Char;

   function Json_String (Value : Text) return Text is
      Raw : constant String := SU.To_String (Value);
      Out_Text : Text := To_Text ("""");
      Hex : constant String := "0123456789abcdef";
   begin
      for Index in Raw'Range loop
         declare
            Octet : constant Natural := Character'Pos (Raw (Index));
         begin
            if Raw (Index) = '"' then
               SU.Append (Out_Text, "\""");
            elsif Raw (Index) = '\' then
               SU.Append (Out_Text, "\\");
            elsif Octet = 10 then
               SU.Append (Out_Text, "\n");
            elsif Octet = 13 then
               SU.Append (Out_Text, "\r");
            elsif Octet = 9 then
               SU.Append (Out_Text, "\t");
            elsif Octet < 16#20# then
               SU.Append (Out_Text, "\u00");
               SU.Append (Out_Text, Hex (Hex'First + Octet / 16));
               SU.Append (Out_Text, Hex (Hex'First + Octet mod 16));
            else
               SU.Append (Out_Text, Raw (Index));
            end if;
         end;
      end loop;
      SU.Append (Out_Text, '"');
      return Out_Text;
   end Json_String;

   procedure Json_Member (Obj : in out Text; Key : String; Value : Text) is
   begin
      if SU.Length (Obj) > 0
        and then SU.Element (Obj, SU.Length (Obj)) /= '{'
      then
         SU.Append (Obj, ',');
      end if;
      SU.Append (Obj, '"');
      SU.Append (Obj, Key);
      SU.Append (Obj, """:");
      SU.Append (Obj, Value);
   end Json_Member;

end Zd_Wire;
