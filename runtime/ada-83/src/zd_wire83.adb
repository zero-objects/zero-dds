--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  The Ada 83 wire runtime's body. Every clause is implemented from the
--  specification text, over RM83's arithmetic: no Interfaces, no
--  modular type, no bitwise operator on an integer. `Le1`/`Shr8` below
--  are `endpoints/ada-83/src/zerodds_ada83_wire.adb`'s own, reused
--  rather than re-derived; the framing above the primitive layer —
--  DHEADER, EMHEADER, the PL_CDR1 parameter list, try-construct status,
--  bitset/bitmask/union-overlay packing, the JSON dialect — is this
--  package's own second reading of `runtime/ada/src/zd_wire.adb`, in
--  RM83 4.5's `+`, `*`, `/` and `mod` in place of every `Interfaces`
--  call there.

with Text_IO;
with Unchecked_Conversion;

package body Zd_Wire83 is

   type Bytes8 is array (0 .. 7) of Byte;
   pragma Pack (Bytes8);
   type Bytes4 is array (0 .. 3) of Byte;
   pragma Pack (Bytes4);

   function F32_To_Bytes is new Unchecked_Conversion (Float, Bytes4);
   function F64_To_Bytes is new Unchecked_Conversion (Long_Float, Bytes8);
   function Bytes_To_F32 is new Unchecked_Conversion (Bytes4, Float);
   function Bytes_To_F64 is new Unchecked_Conversion (Bytes8, Long_Float);
   function Bits_To_F32 is new Unchecked_Conversion (Integer, Float);

   Pid_Extended_Id      : constant := 16#3F01#;
   Pid_List_End_Id      : constant := 16#3F02#;
   Pid_Ignore_Id        : constant := 16#3F03#;
   Pid_Must_Understand  : constant := 16#4000#;
   Pid_Impl_Extension   : constant := 16#8000#;
   Pid_Short_Max_Length : constant := 16#FFFF#;
   Pid_Id_Mask          : constant := 16#3FFF#;
   E_Flag_Must_Understand : constant := 16#4000_0000#;
   E_Flag_Impl_Extension  : constant := 16#0000_0000#;  --  see Read_Emheader
   E_Member_Id_Mask        : constant := 16#0FFF_FFFF#;

   Byte_Order_Mark_Unit         : constant := 16#FEFF#;
   Swapped_Byte_Order_Mark_Unit : constant := 16#FFFE#;

   type Wire_Position is (Member_Pos, Sentinel_Pos, Extended_Marker_Pos);

   --  The types this body needs, all of them above the first body.
   --  RM83 3.9 orders a declarative part: basic declarative items,
   --  then later ones, and a subprogram body is a later one. These
   --  four stood between the bodies that use them, which Ada 95
   --  allows and Ada 83 does not — GNAT answers "decl cannot appear
   --  after body".
   type Wstring_Cell is record
      Counts_Characters : Boolean;
      Terminated        : Boolean;
      Width             : Natural;
   end record;

   type Point_Array is array (Natural range <>) of Long_Integer;

   type Unit_Array is array (Natural range <>) of Long_Integer;

   type Nibble_Array is array (Natural range <>) of Natural;

   --  Ada 83 has no if expression; these four stand in its place.
   --  `(if C then A else B)` is an Ada 2012 form, and this body used it
   --  nineteen times — GNAT answers "if expression is an Ada 2012
   --  feature" for each. A function call says the same thing in the
   --  language this runtime is named after, and unlike an if statement
   --  it still fits where a constant declaration or an argument needs a
   --  value. One per result type, because Ada 83 resolves an overload
   --  by the parameter types and a numeric literal fits every one of
   --  them.
   function Sel_Nat (Cond : Boolean; Yes, No : Natural) return Natural is
   begin
      if Cond then
         return Yes;
      end if;
      return No;
   end Sel_Nat;

   function Sel_Int (Cond : Boolean; Yes, No : Integer) return Integer is
   begin
      if Cond then
         return Yes;
      end if;
      return No;
   end Sel_Int;

   function Sel_Long (Cond : Boolean; Yes, No : Long_Integer) return Long_Integer is
   begin
      if Cond then
         return Yes;
      end if;
      return No;
   end Sel_Long;

   function Sel_Bool (Cond : Boolean; Yes, No : Boolean) return Boolean is
   begin
      if Cond then
         return Yes;
      end if;
      return No;
   end Sel_Bool;

   --  The three chains that were `elsif` expressions, as ordinary
   --  functions. `Sel_*` above takes one condition; these take a value
   --  and answer over a range of it, which an if statement says more
   --  plainly than nested calls would.

   --  The octets a code point costs in UTF-8, per 7.4.1.1.2.
   function Utf8_Cost (Point : Long_Integer) return Natural is
   begin
      if Point < 16#80# then
         return 1;
      elsif Point < 16#800# then
         return 2;
      elsif Point < 16#1_0000# then
         return 3;
      end if;
      return 4;
   end Utf8_Cost;

   --  What a length code scales the following count by, XTypes 1.3
   --  7.4.3.4.2: codes 6 and 7 count 4- and 8-octet units, the rest
   --  count octets.
   function Length_Code_Scale (Code : Natural) return Natural is
   begin
      if Code = 6 then
         return 4;
      elsif Code = 7 then
         return 8;
      end if;
      return 1;
   end Length_Code_Scale;

   function Flags_At (Policy : Pid_Flag_Policy; Position : Wire_Position) return Long_Integer is
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
               return Pid_Impl_Extension + Pid_Must_Understand;
            end if;
            return 0;
      end case;
   end Flags_At;


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

   function Zeros (Count : Natural) return String is
      Run : constant String (1 .. Count) := (others => '0');
   begin
      return Run;
   end Zeros;

   --  `Value` with leading zeros until it is at least `Width` long.
   function Left_Padded (Value : String; Width : Natural) return String is
   begin
      if Value'Length < Width then
         return Zeros (Width - Value'Length) & Value;
      end if;
      return Value;
   end Left_Padded;

   function Image (Value : Integer) return String is
      Raw : constant String := Integer'Image (Value);
   begin
      if Raw (Raw'First) = ' ' then
         return Raw (Raw'First + 1 .. Raw'Last);
      end if;
      return Raw;
   end Image;

   --  2**N as a Long_Integer, for N in 0 .. 62. Bounded below 63: a
   --  caller checking a 64-bit width against its own ceiling has
   --  nothing to check against, because `Long_Integer` itself already
   --  stops at 2**63 - 1 — the same guard `Write_Bitmask`'s "Bits < 64"
   --  takes below.
   function Pow2 (N : Natural) return Long_Integer is
      Acc : Long_Integer := 1;
   begin
      for I in 1 .. N loop
         Acc := Acc * 2;
      end loop;
      return Acc;
   end Pow2;

   --  A 64-bit pattern as its two unsigned halves, each in
   --  0 .. 2**32 - 1. RM83 4.5.5 gives `mod` the sign of its right
   --  operand, so `Lo` is the low half for a negative `Value` as well;
   --  taking it out first makes the division of the rest exact, which
   --  RM83 4.5.5's `/` — truncating toward zero — would otherwise get
   --  wrong below zero.
   --  The halves are built in locals and handed over at the end: RM83
   --  6.2 makes a scalar `out` parameter write-only inside the
   --  subprogram, and GNAT answers "illegal reading of out parameter"
   --  for every use of one on the right of an assignment. Ada 95
   --  dropped that restriction, which is why the shape reads oddly
   --  beside `runtime/ada`'s.
   procedure Split_Pattern (Value : Long_Integer; Hi, Lo : out Long_Integer) is
      Low  : constant Long_Integer := Value mod Pow2 (32);
      High : Long_Integer := (Value - Low) / Pow2 (32);
   begin
      if High < 0 then
         High := High + Pow2 (32);
      end if;
      Hi := High;
      Lo := Low;
   end Split_Pattern;

   --  The inverse: the halves back into the one `Long_Integer` whose
   --  bits they are. A high half at or above 2**31 is bit 63 set, which
   --  has no non-negative `Long_Integer`, so the answer is negative.
   function Join_Pattern (Hi, Lo : Long_Integer) return Long_Integer is
   begin
      if Hi >= Pow2 (31) then
         return (Hi - Pow2 (32)) * Pow2 (32) + Lo;
      end if;
      return Hi * Pow2 (32) + Lo;
   end Join_Pattern;

   --  One half's `and`, `or` and `not`, over the thirty-two bits a
   --  `Long_Integer` multiplication cannot overflow on. The loop is
   --  what RM83 4.5.1 leaves: the operators it declares are Boolean's,
   --  and a modular type — whose own operators would do this — is an
   --  Ada 95 addition.
   function And32 (Left, Right : Long_Integer) return Long_Integer is
      A      : Long_Integer := Left;
      B      : Long_Integer := Right;
      Result : Long_Integer := 0;
      Weight : Long_Integer := 1;
   begin
      for Bit in 0 .. 31 loop
         if A mod 2 = 1 and then B mod 2 = 1 then
            Result := Result + Weight;
         end if;
         A := A / 2;
         B := B / 2;
         Weight := Weight * 2;
      end loop;
      return Result;
   end And32;

   function Or32 (Left, Right : Long_Integer) return Long_Integer is
      A      : Long_Integer := Left;
      B      : Long_Integer := Right;
      Result : Long_Integer := 0;
      Weight : Long_Integer := 1;
   begin
      for Bit in 0 .. 31 loop
         if A mod 2 = 1 or else B mod 2 = 1 then
            Result := Result + Weight;
         end if;
         A := A / 2;
         B := B / 2;
         Weight := Weight * 2;
      end loop;
      return Result;
   end Or32;

   function Xor32 (Left, Right : Long_Integer) return Long_Integer is
      A      : Long_Integer := Left;
      B      : Long_Integer := Right;
      Result : Long_Integer := 0;
      Weight : Long_Integer := 1;
   begin
      for Bit in 0 .. 31 loop
         if A mod 2 /= B mod 2 then
            Result := Result + Weight;
         end if;
         A := A / 2;
         B := B / 2;
         Weight := Weight * 2;
      end loop;
      return Result;
   end Xor32;

   function Not32 (Value : Long_Integer) return Long_Integer is
   begin
      return Pow2 (32) - 1 - Value;
   end Not32;

   --  The low octet of V's two's-complement pattern. Ada's `mod` floors
   --  (RM83 4.5.5), which is what makes this the low octet for a
   --  negative V as well as a non-negative one.
   function Le1 (V : Long_Integer) return Byte is
   begin
      return Byte (V mod 256);
   end Le1;

   --  V shifted right by eight, flooring — RM83 4.5.5's `/` truncates
   --  toward zero, so the remainder is taken out first to make the
   --  division exact for either sign. See
   --  `endpoints/ada-83/src/zerodds_ada83_wire.adb`'s `Shr8`, whose
   --  comment carries the measured failure this answers.
   function Shr8 (V : Long_Integer) return Long_Integer is
   begin
      return (V - (V mod 256)) / 256;
   end Shr8;

   --  --  text  -----------------------------------------------------

   function Empty_Text return Text is
   begin
      return new String'("");
   end Empty_Text;

   function Text_Of (S : String) return Text is
   begin
      return new String'(S);
   end Text_Of;

   function Text_From_Utf8 (Octets : Byte_Array) return Text is
      Out_S : String (1 .. Octets'Length);
   begin
      for I in Octets'Range loop
         Out_S (Out_S'First + (I - Octets'First)) := Character'Val (Integer (Octets (I)));
      end loop;
      return new String'(Out_S);
   end Text_From_Utf8;

   function Strcmp (A, B : Text) return Integer is
   begin
      --  Ada's String "<" compares Character'Pos, which over UTF-8
      --  bytes is C11 7.24.4's unsigned-octet order.
      if A.all < B.all then
         return -1;
      elsif A.all > B.all then
         return 1;
      end if;
      return 0;
   end Strcmp;

   function Utf8_Octets (Value : Text) return Natural is
   begin
      return Value.all'Length;
   end Utf8_Octets;

   --  --  UTF-8 / UTF-16  ------------------------------------------------

   --  The code points of a UTF-8 string, decoded strictly: overlong
   --  forms, surrogates, values past U+10FFFF and truncated tails are
   --  refused, exactly as `runtime/ada`'s `Decode_Utf8` reads them.

   function Decode_Utf8 (Value : String) return Point_Array is
      Out_Points : Point_Array (0 .. Value'Length);
      Count      : Natural := 0;
      Index      : Integer := Value'First;

      function Byte_At (Position : Integer) return Long_Integer is
      begin
         return Long_Integer (Character'Pos (Value (Position)));
      end Byte_At;
   begin
      while Index <= Value'Last loop
         declare
            First : constant Long_Integer := Byte_At (Index);
            Point : Long_Integer;
            Tail  : Natural;
            Low   : Long_Integer;
         begin
            if First < 16#80# then
               Point := First;
               Tail := 0;
               Low := 0;
            elsif (First - (First mod 32)) = 16#C0# then
               Point := First mod 32;
               Tail := 1;
               Low := 16#80#;
            elsif (First - (First mod 16)) = 16#E0# then
               Point := First mod 16;
               Tail := 2;
               Low := 16#800#;
            elsif (First - (First mod 8)) = 16#F0# then
               Point := First mod 8;
               Tail := 3;
               Low := 16#1_0000#;
            else
               raise Malformed_Stream;
            end if;
            if Index + Tail > Value'Last then
               raise Malformed_Stream;
            end if;
            for Extra in 1 .. Tail loop
               declare
                  Cont : constant Long_Integer := Byte_At (Index + Extra);
               begin
                  if (Cont - (Cont mod 64)) /= 16#80# then
                     raise Malformed_Stream;
                  end if;
                  Point := Point * 64 + (Cont mod 64);
               end;
            end loop;
            if Tail > 0 and then Point < Low then
               raise Malformed_Stream;
            end if;
            if Point > 16#10_FFFF#
              or else (Point >= 16#D800# and then Point <= 16#DFFF#)
            then
               raise Malformed_Stream;
            end if;
            Out_Points (Count) := Point;
            Count := Count + 1;
            Index := Index + 1 + Tail;
         end;
      end loop;
      return Out_Points (0 .. Count - 1);
   end Decode_Utf8;

   procedure Append_Point (Value : in out Text; Point : Long_Integer) is
      procedure Raw (Octet : Long_Integer) is
         Grown : constant Text := new String (1 .. Value.all'Length + 1);
      begin
         Grown (1 .. Value.all'Length) := Value.all;
         Grown (Grown'Last) := Character'Val (Integer (Octet mod 256));
         Value := Grown;
      end Raw;
   begin
      if Point < 16#80# then
         Raw (Point);
      elsif Point < 16#800# then
         Raw (16#C0# + Point / 64);
         Raw (16#80# + (Point mod 64));
      elsif Point < 16#1_0000# then
         Raw (16#E0# + Point / 4096);
         Raw (16#80# + ((Point / 64) mod 64));
         Raw (16#80# + (Point mod 64));
      else
         Raw (16#F0# + Point / 262144);
         Raw (16#80# + ((Point / 4096) mod 64));
         Raw (16#80# + ((Point / 64) mod 64));
         Raw (16#80# + (Point mod 64));
      end if;
   end Append_Point;


   function Utf16_Of (Value : Text) return Unit_Array is
      Points : constant Point_Array := Decode_Utf8 (Value.all);
      Units  : Unit_Array (0 .. 2 * Points'Length);
      Count  : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant Long_Integer := Points (Index);
         begin
            if Point < 16#1_0000# then
               Units (Count) := Point;
               Count := Count + 1;
            else
               declare
                  Offset : constant Long_Integer := Point - 16#1_0000#;
               begin
                  Units (Count) := 16#D800# + Offset / 1024;
                  Units (Count + 1) := 16#DC00# + (Offset mod 1024);
                  Count := Count + 2;
               end;
            end if;
         end;
      end loop;
      return Units (0 .. Count - 1);
   end Utf16_Of;

   function Utf16_Units (Value : Text) return Natural is
      Points : constant Point_Array := Decode_Utf8 (Value.all);
      Count  : Natural := 0;
   begin
      for Index in Points'Range loop
         Count := Count + Sel_Nat (Points (Index) > 16#FFFF#, 2, 1);
      end loop;
      return Count;
   end Utf16_Units;

   --  --  helpers the generated code names  -----------------------------

   function Checked_Char (Point : Long_Integer) return Long_Integer is
   begin
      if Point < 0 or else Point > 16#FF# then
         raise Code_Point_Out_Of_Range;
      end if;
      return Point;
   end Checked_Char;

   function Checked_Wchar (Point : Long_Integer) return Long_Integer is
   begin
      if Point < 0 or else Point > 16#FFFF# then
         raise Code_Point_Out_Of_Range;
      end if;
      return Point;
   end Checked_Wchar;

   function As_Signed (Value : Long_Integer) return Long_Integer is
      Octet : constant Long_Integer := Value mod 256;
   begin
      if Octet > 127 then
         return Octet - 256;
      end if;
      return Octet;
   end As_Signed;

   function U64_Less (Left, Right : Long_Integer) return Boolean is
   begin
      --  Bit 63 set is a value at or above 2**63, which under the
      --  unsigned reading is above every pattern that has it clear.
      if (Left < 0) /= (Right < 0) then
         return Right < 0;
      end if;
      --  Same sign: the signed order and the unsigned order agree.
      return Left < Right;
   end U64_Less;

   procedure Discard (Value : Long_Integer) is
      pragma Unreferenced (Value);
   begin
      null;
   end Discard;

   function Bits_And (Left, Right : Long_Integer) return Long_Integer is
      Left_Hi, Left_Lo, Right_Hi, Right_Lo : Long_Integer;
   begin
      Split_Pattern (Left, Left_Hi, Left_Lo);
      Split_Pattern (Right, Right_Hi, Right_Lo);
      return Join_Pattern (And32 (Left_Hi, Right_Hi), And32 (Left_Lo, Right_Lo));
   end Bits_And;

   function Bits_Or (Left, Right : Long_Integer) return Long_Integer is
      Left_Hi, Left_Lo, Right_Hi, Right_Lo : Long_Integer;
   begin
      Split_Pattern (Left, Left_Hi, Left_Lo);
      Split_Pattern (Right, Right_Hi, Right_Lo);
      return Join_Pattern (Or32 (Left_Hi, Right_Hi), Or32 (Left_Lo, Right_Lo));
   end Bits_Or;

   function Bits_Xor (Left, Right : Long_Integer) return Long_Integer is
      Left_Hi, Left_Lo, Right_Hi, Right_Lo : Long_Integer;
   begin
      Split_Pattern (Left, Left_Hi, Left_Lo);
      Split_Pattern (Right, Right_Hi, Right_Lo);
      return Join_Pattern (Xor32 (Left_Hi, Right_Hi), Xor32 (Left_Lo, Right_Lo));
   end Bits_Xor;

   function Bits_Not (Value : Long_Integer) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      Split_Pattern (Value, Hi, Lo);
      return Join_Pattern (Not32 (Hi), Not32 (Lo));
   end Bits_Not;

   function Shift_Right (Value : Long_Integer; Distance : Natural) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      if Distance = 0 then
         return Value;
      end if;
      if Distance >= 64 then
         return 0;
      end if;
      Split_Pattern (Value, Hi, Lo);
      if Distance >= 32 then
         return Join_Pattern (0, Hi / Pow2 (Distance - 32));
      end if;
      --  The bits leaving the high half arrive at the top of the low
      --  one; both products stay below 2**32, so neither overflows.
      return Join_Pattern
               (Hi / Pow2 (Distance),
                Lo / Pow2 (Distance) + (Hi mod Pow2 (Distance)) * Pow2 (32 - Distance));
   end Shift_Right;

   function Shift_Left (Value : Long_Integer; Distance : Natural) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      if Distance = 0 then
         return Value;
      end if;
      if Distance >= 64 then
         return 0;
      end if;
      Split_Pattern (Value, Hi, Lo);
      if Distance >= 32 then
         return Join_Pattern ((Lo mod Pow2 (64 - Distance)) * Pow2 (Distance - 32), 0);
      end if;
      --  The high half is cut to what still fits before it is shifted,
      --  so the multiplication stays below 2**32 rather than reaching
      --  past `Long_Integer'Last` on the way to being masked.
      return Join_Pattern
               ((Hi mod Pow2 (32 - Distance)) * Pow2 (Distance) + Lo / Pow2 (32 - Distance),
                (Lo mod Pow2 (32 - Distance)) * Pow2 (Distance));
   end Shift_Left;

   function Bits_Subset (Value, Mask : Long_Integer) return Boolean is
   begin
      return Bits_And (Value, Bits_Not (Mask)) = 0;
   end Bits_Subset;

   function Bit_Pattern_Int (Value : Long_Integer; Octets : Natural) return Long_Integer is
   begin
      --  Eight octets: a `Long_Integer` already *is* its own 64-bit
      --  two's-complement pattern, for either sign, so there is nothing
      --  to mask off. A negative result is bit 63 set, and
      --  `Write_Union_Overlay` splits it apart the same way `Write_U64`
      --  does.
      if Octets >= 8 then
         return Value;
      end if;
      --  The two's-complement bit pattern of a negative value at a
      --  narrower width, built by hand: RM83 has no `and`/`or` on an
      --  integer type to mask one with, so `+` and `mod` do the work.
      declare
         Pattern : Long_Integer := Value;
      begin
         if Pattern < 0 then
            Pattern := Pattern + Pow2 (Octets * 8);
         end if;
         return Pattern mod Pow2 (Octets * 8);
      end;
   end Bit_Pattern_Int;

   function Bit_Pattern_Bool (Value : Boolean) return Long_Integer is
   begin
      return Sel_Long (Value, 1, 0);
   end Bit_Pattern_Bool;

   function Bit_Pattern_F32 (Value : Float) return Long_Integer is
      Host : constant Bytes4 := F32_To_Bytes (Value);
      Acc  : Long_Integer := 0;
   begin
      for I in reverse 0 .. 3 loop
         Acc := Acc * 256 + Long_Integer (Host (I));
      end loop;
      return Acc;
   end Bit_Pattern_F32;

   procedure Bit_Pattern_F64 (Value : Long_Float; Hi, Lo : out Long_Integer) is
      Host   : constant Bytes8 := F64_To_Bytes (Value);
      Hi_Acc : Long_Integer := 0;
      Lo_Acc : Long_Integer := 0;
   begin
      for I in reverse 4 .. 7 loop
         Hi_Acc := Hi_Acc * 256 + Long_Integer (Host (I));
      end loop;
      for I in reverse 0 .. 3 loop
         Lo_Acc := Lo_Acc * 256 + Long_Integer (Host (I));
      end loop;
      Hi := Hi_Acc;
      Lo := Lo_Acc;
   end Bit_Pattern_F64;

   function Bit_Pattern_F64_Overlay (Value : Long_Float) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      Bit_Pattern_F64 (Value, Hi, Lo);
      --  Bit 63 set — a negative `double`, negative zero included — is
      --  the pattern whose only `Long_Integer` form is the negative one
      --  with those bits, which is what `Write_Union_Overlay` splits
      --  back apart. The multiplication is done on the signed half so
      --  it stays inside `Long_Integer`.
      if Hi >= 16#8000_0000# then
         return (Hi - 16#1_0000_0000#) * Pow2 (32) + Lo;
      end if;
      return Hi * Pow2 (32) + Lo;
   end Bit_Pattern_F64_Overlay;

   function F32_Nan return Float is
      --  The objects above the nested body, as RM83 3.9 orders a
      --  declarative part.
      Le : constant Bytes4 := (16#00#, 16#00#, 16#C0#, 16#7F#);
      Host_Bytes : Bytes4;
      --  1.0 is 16#3F80_0000#, so a first octet that is not zero is
      --  the one carrying the exponent's high bits and the machine is
      --  big-endian. See `Host_Is_Big_64`, which had the test the
      --  wrong way round and made every little-endian machine look big.
      function Host_Is_Big return Boolean is
         Probe : constant Bytes4 := F32_To_Bytes (1.0);
      begin
         return Probe (0) /= 0;
      end Host_Is_Big;
   begin
      if Host_Is_Big then
         for I in 0 .. 3 loop
            Host_Bytes (I) := Le (3 - I);
         end loop;
      else
         Host_Bytes := Le;
      end if;
      return Bytes_To_F32 (Host_Bytes);
   end F32_Nan;

   function F32_Inf return Float is
      --  The objects above the nested body, as RM83 3.9 orders a
      --  declarative part.
      Le : constant Bytes4 := (16#00#, 16#00#, 16#80#, 16#7F#);
      Host_Bytes : Bytes4;
      --  1.0 is 16#3F80_0000#, so a first octet that is not zero is
      --  the one carrying the exponent's high bits and the machine is
      --  big-endian. See `Host_Is_Big_64`, which had the test the
      --  wrong way round and made every little-endian machine look big.
      function Host_Is_Big return Boolean is
         Probe : constant Bytes4 := F32_To_Bytes (1.0);
      begin
         return Probe (0) /= 0;
      end Host_Is_Big;
   begin
      if Host_Is_Big then
         for I in 0 .. 3 loop
            Host_Bytes (I) := Le (3 - I);
         end loop;
      else
         Host_Bytes := Le;
      end if;
      return Bytes_To_F32 (Host_Bytes);
   end F32_Inf;

   --  Whether this machine lays a `Long_Float` down most significant
   --  octet first. 1.0 is 16#3FF0_0000_0000_0000#, so the octet
   --  carrying the exponent's high bits is 16#3F# and every other one
   --  but the second is zero: a first octet that is not zero is that
   --  one, and the machine is big-endian. Reading the test the other
   --  way round — first octet zero means big — called every
   --  little-endian machine big and made `Read_F64` reverse a pattern
   --  that was already in host order, which is how a `double` written
   --  as 18 2D 44 54 FB 21 09 40 came back as 3.2E-192.
   function Host_Is_Big_64 return Boolean is
      Probe : constant Bytes8 := F64_To_Bytes (1.0);
   begin
      return Probe (0) /= 0;
   end Host_Is_Big_64;

   function F64_Nan return Long_Float is
      Le : constant Bytes8 :=
        (16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#F8#, 16#7F#);
      Host_Bytes : Bytes8;
   begin
      if Host_Is_Big_64 then
         for I in 0 .. 7 loop
            Host_Bytes (I) := Le (7 - I);
         end loop;
      else
         Host_Bytes := Le;
      end if;
      return Bytes_To_F64 (Host_Bytes);
   end F64_Nan;

   function F64_Inf return Long_Float is
      Le : constant Bytes8 :=
        (16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#F0#, 16#7F#);
      Host_Bytes : Bytes8;
   begin
      if Host_Is_Big_64 then
         for I in 0 .. 7 loop
            Host_Bytes (I) := Le (7 - I);
         end loop;
      else
         Host_Bytes := Le;
      end if;
      return Bytes_To_F64 (Host_Bytes);
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

   function Padding_For (Pos : Integer; Align : Integer) return Integer is
   begin
      return (Align - (Pos mod Align)) mod Align;
   end Padding_For;

   --  --  the writer  ---------------------------------------------------

   function New_Writer
     (Order              : Byte_Order;
      Max_Align          : Natural;
      Framing            : Pl_Cdr1_Framing;
      Short_Length_Codes : Boolean) return Writer_Access is
   begin
      --  On the heap, because the writer is now handed on by access and
      --  Ada 83 has no `'Access` to take one of a local object.
      return
        new Writer'
        (Buffer             => null,
         Count              => 0,
         Order              => Order,
         Max_Align          => Sel_Nat (Max_Align > 1, Max_Align, 1),
         Origin             => 0,
         Framing            => Framing,
         Short_Length_Codes => Short_Length_Codes);
   end New_Writer;

   procedure Put (W : Writer_Access; Octet : Byte) is
      --  As a statement, not as a `Sel_Nat` call. A call evaluates both
      --  its arguments, and `W.Buffer'Length` on a null buffer raises
      --  `Constraint_Error` — precisely when the condition says to take
      --  the other branch. The `(if …)` expression this replaced
      --  evaluated only the branch it chose; anywhere a branch is
      --  well-defined only under its own condition, the two forms are
      --  not interchangeable.
      Wanted : Natural := 256;
   begin
      if W.Buffer = null or else W.Count > W.Buffer'Last then
         if W.Buffer /= null then
            Wanted := 2 * W.Buffer'Length;
         end if;
         declare
            Grown : constant Byte_Array_Access := new Byte_Array (0 .. Wanted - 1);
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

   function Finish (W : Writer_Access) return Byte_Array_Access is
   begin
      if W.Count = 0 then
         return new Byte_Array (0 .. -1);
      end if;
      return new Byte_Array'(W.Buffer (0 .. W.Count - 1));
   end Finish;

   procedure Align_To (W : Writer_Access; Alignment : Natural) is
      Effective : constant Natural := Sel_Nat (Alignment < W.Max_Align, Alignment, W.Max_Align);
   begin
      if Effective <= 1 then
         return;
      end if;
      declare
         Pad : Integer := Padding_For (W.Count - W.Origin, Effective);
      begin
         while Pad > 0 loop
            Put (W, 0);
            Pad := Pad - 1;
         end loop;
      end;
   end Align_To;

   procedure Write_Raw (W : Writer_Access; Value : Long_Integer; Width : Natural) is
      V : Long_Integer := Value;
   begin
      if W.Order = Little then
         for Index in 1 .. Width loop
            Put (W, Le1 (V));
            V := Shr8 (V);
         end loop;
      else
         declare
            Octets : array (0 .. Width - 1) of Byte;
         begin
            for Index in 0 .. Width - 1 loop
               Octets (Index) := Le1 (V);
               V := Shr8 (V);
            end loop;
            for Index in reverse 0 .. Width - 1 loop
               Put (W, Octets (Index));
            end loop;
         end;
      end if;
   end Write_Raw;

   procedure Write_Unsigned (W : Writer_Access; Value : Long_Integer; Width : Natural) is
   begin
      Align_To (W, Width);
      Write_Raw (W, Value, Width);
   end Write_Unsigned;

   procedure Patch (W : Writer_Access; At_Pos : Natural; Value : Long_Integer; Width : Natural) is
      V : Long_Integer := Value;
      Octets : array (0 .. Width - 1) of Byte;
   begin
      for Index in 0 .. Width - 1 loop
         Octets (Index) := Le1 (V);
         V := Shr8 (V);
      end loop;
      for Index in 0 .. Width - 1 loop
         if W.Order = Little then
            W.Buffer (At_Pos + Index) := Octets (Index);
         else
            W.Buffer (At_Pos + Width - 1 - Index) := Octets (Index);
         end if;
      end loop;
   end Patch;

   function Read_Patched (W : Writer_Access; At_Pos : Natural) return Long_Integer is
      Value : Long_Integer := 0;
   begin
      if W.Order = Little then
         for Index in reverse 0 .. 3 loop
            Value := Value * 256 + Long_Integer (W.Buffer (At_Pos + Index));
         end loop;
      else
         for Index in 0 .. 3 loop
            Value := Value * 256 + Long_Integer (W.Buffer (At_Pos + Index));
         end loop;
      end if;
      return Value;
   end Read_Patched;

   procedure Write_Bool (W : Writer_Access; Value : Boolean) is
   begin
      Put (W, Byte (Sel_Nat (Value, 1, 0)));
   end Write_Bool;

   procedure Write_Int
     (W : Writer_Access; Value : Long_Integer; Width : Natural; Low, High : Long_Integer) is
   begin
      if Value < Low or else Value > High then
         raise Value_Out_Of_Range;
      end if;
      if Width = 1 then
         Put (W, Le1 (Value));  --  one octet: no alignment
      else
         Write_Unsigned (W, Value, Width);
      end if;
   end Write_Int;

   procedure Write_U64_Pair (W : Writer_Access; Hi, Lo : Long_Integer) is
      Le : Bytes8;
      H  : Long_Integer := Hi;
      L  : Long_Integer := Lo;
   begin
      if Hi < 0 or else Hi > 16#FFFF_FFFF# or else Lo < 0 or else Lo > 16#FFFF_FFFF# then
         raise Value_Out_Of_Range;
      end if;
      for Index in 0 .. 3 loop
         Le (Index) := Le1 (L);
         L := Shr8 (L);
      end loop;
      for Index in 4 .. 7 loop
         Le (Index) := Le1 (H);
         H := Shr8 (H);
      end loop;
      Align_To (W, 8);
      if W.Order = Little then
         for Index in 0 .. 7 loop
            Put (W, Le (Index));
         end loop;
      else
         for Index in reverse 0 .. 7 loop
            Put (W, Le (Index));
         end loop;
      end if;
   end Write_U64_Pair;

   procedure Write_U64 (W : Writer_Access; Value : Long_Integer) is
      --  The split of the 64-bit *pattern*, not of the value. RM83
      --  4.5.5 gives `mod` the sign of its right operand, so `Lo` is
      --  the low 32 bits for a negative `Value` as much as for a
      --  positive one, and the subtraction makes the division of the
      --  rest exact where RM83 4.5.5's `/` would otherwise truncate
      --  toward zero. `Hi` comes out negative exactly when bit 63 is
      --  set, which is the half of `unsigned long long`'s range that
      --  has no `Long_Integer` value of its own; adding 2**32 back
      --  turns it into the unsigned half `Write_U64_Pair` wants.
      Lo : constant Long_Integer := Value mod Pow2 (32);
      Hi : Long_Integer := (Value - Lo) / Pow2 (32);
   begin
      if Hi < 0 then
         Hi := Hi + Pow2 (32);
      end if;
      Write_U64_Pair (W, Hi, Lo);
   end Write_U64;

   procedure Write_I64 (W : Writer_Access; Value : Long_Integer) is
      Pattern : Long_Integer := Value;
      Le      : Bytes8;
   begin
      for Index in 0 .. 7 loop
         Le (Index) := Le1 (Pattern);
         Pattern := Shr8 (Pattern);
      end loop;
      Align_To (W, 8);
      if W.Order = Little then
         for Index in 0 .. 7 loop
            Put (W, Le (Index));
         end loop;
      else
         for Index in reverse 0 .. 7 loop
            Put (W, Le (Index));
         end loop;
      end if;
   end Write_I64;

   procedure Write_F32 (W : Writer_Access; Value : Float) is
      Pattern : constant Long_Integer := Bit_Pattern_F32 (Value);
   begin
      Write_Unsigned (W, Pattern, 4);
   end Write_F32;

   procedure Write_F64 (W : Writer_Access; Value : Long_Float) is
      Hi, Lo : Long_Integer;
   begin
      Bit_Pattern_F64 (Value, Hi, Lo);
      Write_U64_Pair (W, Hi, Lo);
   end Write_F64;

   procedure Write_F128 (W : Writer_Access; Value : Text) is
      pragma Unreferenced (W, Value);
   begin
      raise Wire_Not_Implemented;
   end Write_F128;

   --  Least significant octet first, whatever the stream's byte order
   --  is — the counterpart of `Read_Unit_Little`, and what
   --  `runtime/ada`'s `Write_U16_Little`/`Write_U32_Little` are. Not
   --  `Write_Raw`, which follows `W.Order`: `Unit_Host_Unswapped` and
   --  `Unit_Byte_Order_Mark` are exactly the two policies that fix the
   --  unit's order against the stream's, and writing them through the
   --  stream's order made a big-endian stream's units disagree with the
   --  reader that took them little-endian back (`sequence-wstring`
   --  under `ros-fastrtps-xcdr2-be`: the four-octet unit came back
   --  byte-swapped and past 16#FFFF#).
   procedure Write_Unit_Little (W : Writer_Access; Value : Long_Integer; Width : Natural) is
      V : Long_Integer := Value;
   begin
      Align_To (W, Width);
      for Index in 1 .. Width loop
         Put (W, Le1 (V));
         V := Shr8 (V);
      end loop;
   end Write_Unit_Little;

   procedure Write_Unit_Big (W : Writer_Access; Value : Long_Integer; Width : Natural) is
      V : Long_Integer := Value;
      Octets : Bytes4;
   begin
      Align_To (W, Width);
      for Index in 0 .. Width - 1 loop
         Octets (Index) := Le1 (V);
         V := Shr8 (V);
      end loop;
      for Index in reverse 0 .. Width - 1 loop
         Put (W, Octets (Index));
      end loop;
   end Write_Unit_Big;

   procedure Write_Char (W : Writer_Access; Point : Long_Integer) is
   begin
      Put (W, Byte (Checked_Char (Point)));
   end Write_Char;

   procedure Write_Wchar
     (W       : Writer_Access;
      Point   : Long_Integer;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) is
   begin
      if Width /= 2 and then Width /= 4 then
         raise Malformed_Stream;
      end if;
      if Point > 16#FFFF# then
         raise Code_Point_Out_Of_Range;
      end if;
      if Framing = Wchar_Giop_Length_Prefixed then
         Put (W, Byte (Width));
         declare
            Big    : constant Boolean := Order /= Unit_Stream or else W.Order /= Little;
            V      : Long_Integer := Point;
            Octets : Bytes4;
         begin
            --  `Put`, not `Write_Unit_Big`/`Write_Unit_Little`: those
            --  align to the unit's width first, and a GIOP 1.2 wchar's
            --  octets follow its length octet immediately (GIOP 1.2
            --  15.3.1.6 gives the value an octet count and no alignment
            --  of its own; the length octet has already put the stream
            --  at an odd offset in the ordinary case). Aligning here
            --  wrote one pad octet per two-octet wchar and three per
            --  four-octet one — eight octets where every other column
            --  wrote six for `primitive-wchar` under `jacorb-xcdr1-be`
            --  — and `Read_Wchar` below, which takes the octets
            --  unaligned, then could not read the column's own bytes
            --  back.
            for Index in 0 .. Width - 1 loop
               Octets (Index) := Le1 (V);
               V := Shr8 (V);
            end loop;
            if Big then
               for Index in reverse 0 .. Width - 1 loop
                  Put (W, Octets (Index));
               end loop;
            else
               for Index in 0 .. Width - 1 loop
                  Put (W, Octets (Index));
               end loop;
            end if;
         end;
         return;
      end if;
      --  A `wchar` outside GIOP's framing is an ordinary two- or
      --  four-octet unsigned value in the stream's own byte order —
      --  `runtime/ada` writes it with `Write_U16`/`Write_U32` — and
      --  `Read_Wchar` reads it back with `Read_Uint`, which follows
      --  `R.Order`. Not `Write_Unit_Little`, whose order is fixed
      --  against the stream and belongs to the two `wstring` policies
      --  that ask for that.
      Write_Unsigned (W, Point, Width);
   end Write_Wchar;

   procedure Write_String (W : Writer_Access; Value : Text) is
      Raw : constant String := Value.all;
   begin
      Write_Int (W, Long_Integer (Raw'Length + 1), 4, 0, 16#FFFF_FFFF#);
      for Index in Raw'Range loop
         Put (W, Byte (Character'Pos (Raw (Index))));
      end loop;
      Put (W, 0);
   end Write_String;

   procedure Write_String_Bounded (W : Writer_Access; Value : Text; Bound : Natural) is
   begin
      if Utf8_Octets (Value) > Bound then
         raise Value_Out_Of_Range;
      end if;
      Write_String (W, Value);
   end Write_String_Bounded;

   procedure Check_Sequence_Bound
     (W : Writer_Access; Actual : Long_Integer; Bound : Long_Integer)
   is
      pragma Unreferenced (W);
   begin
      if Actual > Bound then
         raise Value_Out_Of_Range;
      end if;
   end Check_Sequence_Bound;

   procedure Check_Map_Bound
     (W : Writer_Access; Actual : Long_Integer; Bound : Long_Integer)
   is
      pragma Unreferenced (W);
   begin
      if Actual > Bound then
         raise Value_Out_Of_Range;
      end if;
   end Check_Map_Bound;

   procedure Write_Wstring_Unit
     (W : Writer_Access; Unit : Long_Integer; Width : Natural; Order : Unit_Order)
   is
      Fixed_Little : constant Boolean :=
        Order = Unit_Byte_Order_Mark or else Order = Unit_Host_Unswapped;
   begin
      if Order = Unit_Big_Endian_Always then
         Write_Unit_Big (W, Unit, Width);
      elsif Fixed_Little then
         Write_Unit_Little (W, Unit, Width);
      else
         Write_Unsigned (W, Unit, Width);
      end if;
   end Write_Wstring_Unit;

   procedure Write_Wstring_Utf8
     (W : Writer_Access; Value : Text; Policy : Wstring_Policy; Order : Unit_Order)
   is
      Cell  : constant Wstring_Cell := Cell_Of (Policy);
      Bare  : constant Unit_Array := Utf16_Of (Value);
      Units : Unit_Array (0 .. Bare'Length + 1);
      Count : Natural := 0;
   begin
      if Cell.Counts_Characters and then Cell.Width = 2 then
         for Index in Bare'Range loop
            if Bare (Index) >= 16#D800# and then Bare (Index) <= 16#DFFF# then
               raise Malformed_Stream;
            end if;
         end loop;
      end if;
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
         Write_Int (W, Long_Integer (Count), 4, 0, 16#FFFF_FFFF#);
      else
         Write_Int (W, Long_Integer (Count * Cell.Width), 4, 0, 16#FFFF_FFFF#);
      end if;
      for Index in 0 .. Count - 1 loop
         Write_Wstring_Unit (W, Units (Index), Cell.Width, Order);
      end loop;
   end Write_Wstring_Utf8;

   procedure Write_Wstring_Bounded
     (W      : Writer_Access;
      Value  : Text;
      Bound  : Natural;
      Policy : Wstring_Policy;
      Order  : Unit_Order)
   is
   begin
      if Utf16_Units (Value) > Bound then
         raise Value_Out_Of_Range;
      end if;
      Write_Wstring_Utf8 (W, Value, Policy, Order);
   end Write_Wstring_Bounded;

   procedure Write_Wstring_Utf8_Octets
     (W : Writer_Access; Value : Text; Policy : Wstring_Policy)
   is
      Cell : constant Wstring_Cell := Cell_Of (Policy);
      Raw  : constant String := Value.all;
   begin
      --  A value that is not UTF-8 is refused, the same reading the
      --  UTF-16 writer's Decode_Utf8 makes. A Text is not a String, so
      --  this direction has to check what the Rust column's type checked
      --  for it.
      declare
         Ignored : constant Point_Array := Decode_Utf8 (Raw);
         pragma Unreferenced (Ignored);
      begin
         null;
      end;
      --  The length is the UTF-8 octet count plus the terminator; C1 and
      --  C3 have nothing to say, because a byte with no fixed width has
      --  nothing for a character count to count and no width for an octet
      --  count to multiply by.
      --  An if statement and not an if expression: the latter is Ada 2012,
      --  and this runtime is compiled with `-gnat83`.
      if Cell.Terminated then
         Write_Int (W, Long_Integer (Raw'Length + 1), 4, 0, 16#FFFF_FFFF#);
      else
         Write_Int (W, Long_Integer (Raw'Length), 4, 0, 16#FFFF_FFFF#);
      end if;
      for Index in Raw'Range loop
         Put (W, Byte (Character'Pos (Raw (Index))));
      end loop;
      if Cell.Terminated then
         Put (W, 0);
      end if;
   end Write_Wstring_Utf8_Octets;

   procedure Write_Wstring_Utf8_Octets_Bounded
     (W : Writer_Access; Value : Text; Bound : Natural; Policy : Wstring_Policy)
   is
   begin
      if Utf16_Units (Value) > Bound then
         raise Value_Out_Of_Range;
      end if;
      Write_Wstring_Utf8_Octets (W, Value, Policy);
   end Write_Wstring_Utf8_Octets_Bounded;

   --  --  fixed BCD  ----------------------------------------------------


   function Fixed_Nibbles
     (Value       : String;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Nibble_Array
   is
      Negative : Boolean := False;
      First    : Integer := Value'First;
      Dot      : Integer := 0;
   begin
      if Digit_Count = 0 or else Digit_Count > 31 or else Scale_Count > Digit_Count then
         raise Not_A_Fixed_Value;
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
         Whole_Last : constant Integer := Sel_Int (Dot = 0, Value'Last, Dot - 1);
         Frac_First : constant Integer :=
           Sel_Int (Dot = 0, Value'Last + 1, Dot + 1);
         Whole_Len  : constant Integer := Whole_Last - First + 1;
         Frac_Len   : constant Integer := Value'Last - Frac_First + 1;
         Integral   : constant Natural := Digit_Count - Scale_Count;
         Lead       : Integer := First;
      begin
         if Whole_Len <= 0 and then Frac_Len <= 0 then
            raise Not_A_Fixed_Value;
         end if;
         for Index in First .. Whole_Last loop
            if Value (Index) < '0' or else Value (Index) > '9' then
               raise Not_A_Fixed_Value;
            end if;
         end loop;
         for Index in Frac_First .. Value'Last loop
            if Value (Index) < '0' or else Value (Index) > '9' then
               raise Not_A_Fixed_Value;
            end if;
         end loop;
         while Lead <= Whole_Last and then Value (Lead) = '0' loop
            Lead := Lead + 1;
         end loop;
         if Whole_Last - Lead + 1 > Integral then
            raise Not_A_Fixed_Value;
         end if;
         if Frac_Len > Scale_Count then
            raise Not_A_Fixed_Value;
         end if;
         declare
            Every       : Nibble_Array (0 .. Digit_Count - 1) := (others => 0);
            At_Digit    : Natural := 0;
            Significant : Natural := 0;
            Wire        : Natural;
         begin
            --  `Sel_Int` and not `Sel_Nat`: a value that is all leading
            --  zeros leaves `Lead` past `Whole_Last`, so the length is
            --  negative, and a call evaluates and converts both
            --  arguments whether or not the condition picks them. A
            --  `Natural` parameter would raise `Constraint_Error` on
            --  the argument the condition rejects.
            At_Digit :=
              Integral - Natural (Sel_Int (Whole_Last - Lead + 1 > 0, Whole_Last - Lead + 1, 0));
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
               Wire := Sel_Nat (Significant > 1, Significant, 1);
            elsif Count = Fixed_Value_String_Digits then
               --  ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one
               --  leading zero. Past 31 no reader can be told the width.
               --  `Sel_Int` for the lengths, as above: an absent part
               --  has a length of zero or below.
               Wire := Natural (Sel_Int (Whole_Len > 0, Whole_Len, 0))
                 + Scale_Count;
               Wire := Wire + Sel_Nat (Wire mod 2 = 0, 1, 0);
               if Wire > 31 then
                  raise Not_A_Fixed_Value;
               end if;
            end if;
            declare
               --  A count wider than P is filled with leading zeros.
               Kept   : constant Natural := Sel_Nat (Wire < Every'Length, Wire, Every'Length);
               Padded : constant Natural :=
                 Sel_Nat ((Wire + 1) mod 2 = 1, 1, 0);
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
     (W           : Writer_Access;
      Value       : Text;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count)
   is
      Nibbles : constant Nibble_Array :=
        Fixed_Nibbles (Value.all, Digit_Count, Scale_Count, Count);
      Index   : Natural := Nibbles'First;
   begin
      while Index < Nibbles'Last loop
         Put (W, Byte (Nibbles (Index) * 16 + Nibbles (Index + 1)));
         Index := Index + 2;
      end loop;
   end Write_Fixed;

   --  --  the bit-bounded kinds  ----------------------------------------

   procedure Write_Enum (W : Writer_Access; Bits : Natural; Value : Long_Integer) is
   begin
      if Bits = 0 or else Bits > 32 then
         raise Bit_Out_Of_Range;
      end if;
      if Value < -Pow2 (Bits - 1) or else Value > Pow2 (Bits) - 1 then
         raise Value_Out_Of_Range;
      end if;
      declare
         --  A statement for the same reason as at `Put`: for a `Value`
         --  near `Long_Integer'Last`, `Value + Pow2 (Bits)` overflows,
         --  and a call would evaluate it even where the condition sends
         --  the other way.
         Pattern : Long_Integer := Value;
      begin
         if Value < 0 then
            Pattern := Value + Pow2 (Bits);
         end if;
         Pattern := Pattern mod Pow2 (Bits);
         if Bits <= 8 then
            Write_Int (W, Pattern, 1, 0, 255);
         elsif Bits <= 16 then
            Write_Int (W, Pattern, 2, 0, 16#FFFF#);
         else
            Write_Int (W, Pattern, 4, 0, 16#FFFF_FFFF#);
         end if;
      end;
   end Write_Enum;

   procedure Write_Bitmask (W : Writer_Access; Bits : Natural; Value : Long_Integer) is
   begin
      if Bits = 0 or else Bits > 64 then
         raise Bit_Out_Of_Range;
      end if;
      --  A bound of 63 has no ceiling to state — `Pow2 (63)` is past
      --  `Long_Integer'Last` — and every non-negative `Long_Integer` is
      --  inside it, so the test below is the whole check for it too.
      if Bits < 63 and then (Value < 0 or else Value >= Pow2 (Bits)) then
         raise Value_Out_Of_Range;
      end if;
      if Bits = 63 and then Value < 0 then
         raise Value_Out_Of_Range;
      end if;
      if Bits <= 8 then
         Write_Int (W, Value, 1, 0, 255);
      elsif Bits <= 16 then
         Write_Int (W, Value, 2, 0, 16#FFFF#);
      elsif Bits <= 32 then
         Write_Int (W, Value, 4, 0, 16#FFFF_FFFF#);
      else
         --  Eight octets, as the 64-bit *pattern*: a bitmask declaring
         --  a bit at position 63 (Table 33 admits a bound of 64) has a
         --  value at or above 2**63, which no `Long_Integer` holds as a
         --  magnitude and every `Long_Integer` holds as a pattern. That
         --  is the same carriage `Write_U64` gives an `unsigned long
         --  long` past the same ceiling, and refusing it here would put
         --  half of a 64-bit bitmask's values out of reach of a column
         --  whose materialiser has no wider type to offer.
         Write_U64 (W, Value);
      end if;
   end Write_Bitmask;

   function Bitfield
     (Value : Long_Integer; Width : Natural; Position : Natural) return Long_Integer is
   begin
      if Width = 0 or else Width > 64 or else Position + Width > 64 then
         raise Bit_Out_Of_Range;
      end if;
      --  A field 63 or 64 bits wide has no ceiling this type can state
      --  — `Pow2 (63)` is already past `Long_Integer'Last` — and at 64
      --  bits the field *is* the anchor, so every pattern is inside it.
      if Width < 63 and then (Value < 0 or else Value > Pow2 (Width) - 1) then
         raise Value_Out_Of_Range;
      end if;
      if Width = 63 and then Value < 0 then
         raise Value_Out_Of_Range;
      end if;
      --  Not `Value * Pow2 (Position)`: a field that ends on bit 63 —
      --  `bitset-multiword`'s third one does, at position 40 with
      --  twenty-four bits — has a product past `Long_Integer'Last`,
      --  which the multiplication answers with Constraint_Error rather
      --  than with the pattern the anchor wants.
      return Shift_Left (Value, Position);
   end Bitfield;

   procedure Write_Bitset (W : Writer_Access; Anchor : Natural; Value : Long_Integer) is
   begin
      case Anchor is
         when 1 => Write_Int (W, Value mod 256, 1, 0, 255);
         when 2 => Write_Int (W, Value mod 65536, 2, 0, 16#FFFF#);
         when 4 => Write_Int (W, Value mod Pow2 (32), 4, 0, 16#FFFF_FFFF#);
         --  The pattern whole, through the split `Write_U64` already
         --  floors: an anchor whose top field reaches bit 63 arrives
         --  here negative, and RM83 4.5.5's `/` truncating toward zero
         --  would have put the wrong value in the high half.
         when 8 => Write_U64 (W, Value);
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Write_Bitset;

   procedure Write_Union_Overlay (W : Writer_Access; Pattern : Long_Integer; Octets : Natural) is
   begin
      case Octets is
         when 1 => Write_Int (W, Pattern mod 256, 1, 0, 255);
         when 2 => Write_Int (W, Pattern mod 65536, 2, 0, 16#FFFF#);
         when 4 => Write_Int (W, Pattern mod Pow2 (32), 4, 0, 16#FFFF_FFFF#);
         when 8 =>
            --  The same floor split `Write_U64` takes, and for the same
            --  reason: a pattern with bit 63 set arrives as a negative
            --  `Long_Integer`, and RM83 4.5.5's `/` truncates toward
            --  zero, which would put the wrong value in the high half.
            declare
               Lo : constant Long_Integer := Pattern mod Pow2 (32);
               Hi : Long_Integer := (Pattern - Lo) / Pow2 (32);
            begin
               if Hi < 0 then
                  Hi := Hi + Pow2 (32);
               end if;
               Write_U64_Pair (W, Hi, Lo);
            end;
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Write_Union_Overlay;

   --  --  the writer's framing  -----------------------------------------

   function Write_Dheader (W : Writer_Access) return Dheader is
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

   procedure Finish_Dheader (W : Writer_Access; Header : Dheader) is
   begin
      if Header.At_Pos < 0 then
         return;
      end if;
      Patch (W, Header.At_Pos, Long_Integer (W.Count - Header.At_Pos - 4), 4);
   end Finish_Dheader;

   function Write_Emheader
     (W               : Writer_Access;
      Member_Id       : Natural;
      Length_Code     : Natural;
      Must_Understand : Boolean) return Emheader is
   begin
      if Long_Integer (Member_Id) > E_Member_Id_Mask then
         raise Member_Id_Out_Of_Range;
      end if;
      Align_To (W, 4);
      declare
         Word : Long_Integer :=
           Long_Integer (Length_Code mod 8) * Pow2 (28) + Long_Integer (Member_Id);
      begin
         if Must_Understand then
            Word := Word + 16#8000_0000#;
         end if;
         Write_Int (W, Word, 4, 0, 16#FFFF_FFFF#);
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
                  return (Next_Int => At_Pos, Word => Word_At, Length_Code => Length_Code);
               end;
            end if;
            return (Next_Int => -1, Word => Word_At, Length_Code => Length_Code);
         end;
      end;
   end Write_Emheader;

   procedure Shorten_Emheader (W : Writer_Access; Header : Emheader; Length : Natural) is
      Code : Natural;
   begin
      case Length is
         when 1      => Code := 0;
         when 2      => Code := 1;
         when 4      => Code := 2;
         when others => Code := 3;
      end case;
      for Index in Header.Next_Int .. W.Count - 5 loop
         W.Buffer (Index) := W.Buffer (Index + 4);
      end loop;
      W.Count := W.Count - 4;
      declare
         Previous : constant Long_Integer := Read_Patched (W, Header.Word);
         --  Clear the top three bits of the length-code nibble (bits 28
         --  to 30) and set the new code: `Previous mod 2**28` keeps the
         --  member id, and the must-understand bit (31) is untouched
         --  because it is above where the code sits.
         Below    : constant Long_Integer := Previous mod Pow2 (28);
         Above    : constant Long_Integer :=
           Sel_Long (Previous >= 16#8000_0000#, 16#8000_0000#, 0);
      begin
         Patch (W, Header.Word, Below + Long_Integer (Code) * Pow2 (28) + Above, 4);
      end;
   end Shorten_Emheader;

   procedure Finish_Emheader (W : Writer_Access; Header : Emheader) is
   begin
      if Header.Next_Int < 0 then
         if Header.Length_Code <= 3 and then Header.Word + 4 <= W.Count then
            declare
               Declared : constant Natural := 2 ** Header.Length_Code;
               Body_Len : constant Natural := W.Count - Header.Word - 4;
            begin
               if Body_Len /= Declared then
                  raise Length_Code_Mismatch;
               end if;
            end;
         end if;
         return;
      end if;
      declare
         Length : constant Natural := W.Count - Header.Next_Int - 4;
      begin
         if W.Short_Length_Codes
           and then (Length = 1 or else Length = 2 or else Length = 4 or else Length = 8)
         then
            Shorten_Emheader (W, Header, Length);
            return;
         end if;
         Patch (W, Header.Next_Int, Long_Integer (Length), 4);
      end;
   end Finish_Emheader;

   function Begin_Parameter_Value (W : Writer_Access) return Natural is
      Saved : constant Natural := W.Origin;
   begin
      W.Origin := W.Count;
      return Saved;
   end Begin_Parameter_Value;

   --  --  packed collection elements  ----------------------------------

   --  The same origin move Begin_Parameter_Value makes, under the name
   --  the shared walk calls for ElementPadding::Packed. Kept apart
   --  because the two callers mean different things: a PL_CDR1 parameter
   --  value is its own block by 7.4.1.2, a collection element under a
   --  packed key is its own block by HDDS's measured departure.
   function Begin_Packed_Element (W : Writer_Access) return Natural is
      Saved : constant Natural := W.Origin;
   begin
      W.Origin := W.Count;
      return Saved;
   end Begin_Packed_Element;

   procedure Finish_Packed_Element (W : Writer_Access; Saved : Natural) is
   begin
      W.Origin := Saved;
   end Finish_Packed_Element;

   function Write_Pid
     (W               : Writer_Access;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write is
   begin
      if Pid > W.Framing.Highest_Short_Id then
         raise Member_Id_Out_Of_Range;
      end if;
      Align_To (W, 4);
      declare
         Word : Long_Integer := Long_Integer (Pid);
         --  The id with the must-understand (bit 14) and
         --  implementation-extension (bit 15) flags cleared: both are
         --  at or above `Pid_Must_Understand`, so `mod` that value
         --  removes them the way `and not (…)` would in a language
         --  with a bitwise operator on an integer type.
         Bare : constant Long_Integer := Word mod Pid_Must_Understand;
      begin
         if Must_Understand and then W.Framing.Member_Must_Understand then
            Word := Word + Pid_Must_Understand;
         end if;
         Word := Word + Flags_At (W.Framing.Pid_Flags, Member_Pos);
         Write_Int (W, Word mod 65536, 2, 0, 16#FFFF#);
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
     (W               : Writer_Access;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write is
   begin
      if Long_Integer (Pid) > E_Member_Id_Mask then
         raise Member_Id_Out_Of_Range;
      end if;
      Align_To (W, 4);
      declare
         Marker : constant Long_Integer :=
           Pid_Extended_Id + Flags_At (W.Framing.Pid_Flags, Extended_Marker_Pos);
         Member_Header : Long_Integer := Long_Integer (Pid) mod (E_Member_Id_Mask + 1);
      begin
         Write_Int (W, Marker mod 65536, 2, 0, 16#FFFF#);
         Write_Int (W, 8, 2, 0, 16#FFFF#);
         if Must_Understand and then W.Framing.Member_Must_Understand then
            Member_Header := Member_Header + E_Flag_Must_Understand;
         end if;
         Write_Int (W, Member_Header, 4, 0, 16#FFFF_FFFF#);
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

   procedure Promote_Pid (W : Writer_Access; Header : Pid_Write; Length : Natural) is
      Start     : constant Natural := Header.At_Pos - 2;
      Value_At  : constant Natural := Header.At_Pos + 2;
      Old_Count : constant Natural := W.Count;
   begin
      for Extra in 1 .. 8 loop
         Put (W, 0);
      end loop;
      for Index in reverse Value_At .. Old_Count - 1 loop
         W.Buffer (Index + 8) := W.Buffer (Index);
      end loop;
      declare
         Marker : constant Long_Integer :=
           Pid_Extended_Id + Flags_At (W.Framing.Pid_Flags, Extended_Marker_Pos);
         Member_Header : Long_Integer := Long_Integer (Header.Pid) mod (E_Member_Id_Mask + 1);
      begin
         if Header.Must_Understand and then W.Framing.Member_Must_Understand then
            Member_Header := Member_Header + E_Flag_Must_Understand;
         end if;
         Patch (W, Start, Marker mod 65536, 2);
         Patch (W, Start + 2, 8, 2);
         Patch (W, Start + 4, Member_Header, 4);
         Patch (W, Start + 8, Long_Integer (Length), 4);
      end;
   end Promote_Pid;

   procedure Finish_Pid (W : Writer_Access; Header : Pid_Write) is
      Width  : constant Natural := Sel_Nat (Header.Extended, 4, 2);
      Length : Natural := W.Count - Header.At_Pos - Width;
      Pads   : constant Boolean :=
        Sel_Bool (Header.Role = Role_Optional,
                  W.Framing.Pad_Declared_Optional_Length,
                  W.Framing.Pad_Declared_Length);
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
         Patch (W, Header.At_Pos, Long_Integer (Length), Width);
      end if;
      W.Origin := Header.Saved_Origin;
      if W.Framing.Pad_After_Value then
         Align_To (W, 4);
      end if;
   end Finish_Pid;

   procedure Write_Sentinel (W : Writer_Access) is
      Sentinel : constant Long_Integer :=
        Pid_List_End_Id + Flags_At (W.Framing.Pid_Flags, Sentinel_Pos);
   begin
      Align_To (W, 4);
      Write_Int (W, Sentinel mod 65536, 2, 0, 16#FFFF#);
      Write_Int (W, 0, 2, 0, 16#FFFF#);
   end Write_Sentinel;

   --  --  the reader  ---------------------------------------------------

   function New_Reader
     (Data      : Byte_Array_Access;
      Order     : Byte_Order;
      Max_Align : Natural;
      Framing   : Pl_Cdr1_Framing) return Reader_Access is
   begin
      --  On the heap, as at `New_Writer`.
      return
        new Reader'
        (Data      => Data,
         At_Pos    => 0,
         Order     => Order,
         Max_Align => Sel_Nat (Max_Align > 1, Max_Align, 1),
         Origin    => 0,
         Framing   => Framing,
         Current   => Status_Ok);
   end New_Reader;

   function Data_Length (R : Reader_Access) return Natural is
   begin
      if R.Data = null then
         return 0;
      end if;
      return R.Data'Length;
   end Data_Length;

   function Reader_Status (R : Reader_Access) return Status is
   begin
      return R.Current;
   end Reader_Status;

   procedure Reader_Refuse (R : Reader_Access; Refusal : Status) is
   begin
      if Refusal /= Status_Ok and then R.Current = Status_Ok then
         R.Current := Refusal;
      end if;
   end Reader_Refuse;

   function Reader_Clear_Discard (R : Reader_Access) return Boolean is
   begin
      if R.Current = Status_Discarded then
         R.Current := Status_Ok;
         return True;
      end if;
      return False;
   end Reader_Clear_Discard;

   function Reader_Position (R : Reader_Access) return Natural is
   begin
      return R.At_Pos;
   end Reader_Position;

   function Reader_Remaining (R : Reader_Access) return Natural is
      Whole : constant Natural := Data_Length (R);
   begin
      if R.At_Pos > Whole then
         return 0;
      end if;
      return Whole - R.At_Pos;
   end Reader_Remaining;

   procedure Reader_Skip_To (R : Reader_Access; Target : Natural) is
   begin
      if R.Current /= Status_Ok then
         return;
      end if;
      if Target < R.At_Pos then
         Reader_Refuse (R, Status_Out_Of_Range);
         return;
      end if;
      if Target > Data_Length (R) then
         raise Stream_Truncated;
      end if;
      R.At_Pos := Target;
   end Reader_Skip_To;

   function Reader_Save (R : Reader_Access) return Reader_Mark is
   begin
      return (At_Pos => R.At_Pos, Origin => R.Origin, Current => R.Current);
   end Reader_Save;

   procedure Reader_Restore (R : Reader_Access; Mark : Reader_Mark) is
   begin
      R.At_Pos := Mark.At_Pos;
      R.Origin := Mark.Origin;
      R.Current := Mark.Current;
   end Reader_Restore;

   --  The read-side pair of Begin_Packed_Element.
   function Reader_Begin_Packed_Element (R : Reader_Access) return Natural is
      Saved : constant Natural := R.Origin;
   begin
      R.Origin := R.At_Pos;
      return Saved;
   end Reader_Begin_Packed_Element;

   procedure Reader_Finish_Packed_Element (R : Reader_Access; Saved : Natural) is
   begin
      R.Origin := Saved;
   end Reader_Finish_Packed_Element;

   procedure Reader_Align (R : Reader_Access; Alignment : Natural) is
      Effective : constant Natural := Sel_Nat (Alignment < R.Max_Align, Alignment, R.Max_Align);
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

   function Take (R : Reader_Access) return Byte is
   begin
      if R.At_Pos >= Data_Length (R) then
         raise Stream_Truncated;
      end if;
      declare
         Octet : constant Byte := R.Data (R.Data'First + R.At_Pos);
      begin
         R.At_Pos := R.At_Pos + 1;
         return Octet;
      end;
   end Take;

   procedure Need (R : Reader_Access; Count : Natural) is
   begin
      if R.At_Pos + Count > Data_Length (R) then
         raise Stream_Truncated;
      end if;
   end Need;

   function Read_Uint (R : Reader_Access; Width : Natural) return Long_Integer is
      Value : Long_Integer := 0;
   begin
      Reader_Align (R, Width);
      Need (R, Width);
      if R.Order = Little then
         declare
            Octets : array (0 .. Width - 1) of Byte;
         begin
            for Index in 0 .. Width - 1 loop
               Octets (Index) := Take (R);
            end loop;
            for Index in reverse 0 .. Width - 1 loop
               Value := Value * 256 + Long_Integer (Octets (Index));
            end loop;
         end;
      else
         for Index in 1 .. Width loop
            Value := Value * 256 + Long_Integer (Take (R));
         end loop;
      end if;
      return Value;
   end Read_Uint;

   function Read_Int (R : Reader_Access; Width : Natural) return Long_Integer is
      Value : constant Long_Integer := Read_Uint (R, Width);
      Ceiling : constant Long_Integer := Pow2 (Width * 8);
   begin
      if Value >= Ceiling / 2 then
         return Value - Ceiling;
      end if;
      return Value;
   end Read_Int;

   function Read_Bool (R : Reader_Access) return Boolean is
      Value : constant Byte := Take (R);
   begin
      if Value = 0 then
         return False;
      elsif Value = 1 then
         return True;
      end if;
      raise Malformed_Stream;
   end Read_Bool;

   function Read_Char (R : Reader_Access) return Long_Integer is
   begin
      return Long_Integer (Take (R));
   end Read_Char;

   procedure Read_U64_Pair (R : Reader_Access; Hi, Lo : out Long_Integer) is
      Octets : Bytes8;
      Le     : Bytes8;
      Hi_Acc, Lo_Acc : Long_Integer := 0;
   begin
      Reader_Align (R, 8);
      Need (R, 8);
      for Index in 0 .. 7 loop
         Octets (Index) := Take (R);
      end loop;
      if R.Order = Big then
         for Index in 0 .. 7 loop
            Le (Index) := Octets (7 - Index);
         end loop;
      else
         Le := Octets;
      end if;
      for Index in reverse 0 .. 3 loop
         Lo_Acc := Lo_Acc * 256 + Long_Integer (Le (Index));
      end loop;
      for Index in reverse 4 .. 7 loop
         Hi_Acc := Hi_Acc * 256 + Long_Integer (Le (Index));
      end loop;
      Hi := Hi_Acc;
      Lo := Lo_Acc;
   end Read_U64_Pair;

   function Read_U64 (R : Reader_Access) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      Read_U64_Pair (R, Hi, Lo);
      --  The pattern, kept whole: a wire value at or above 2**63 comes
      --  back as the negative `Long_Integer` whose bits it is, which is
      --  what `Json_U64` renders unsigned again. Refusing it here would
      --  make half of `unsigned long long`'s range unreadable in a
      --  column whose materialiser has no wider type to offer.
      if Hi >= 16#8000_0000# then
         return (Hi - 16#1_0000_0000#) * Pow2 (32) + Lo;
      end if;
      return Hi * Pow2 (32) + Lo;
   end Read_U64;

   function Read_I64 (R : Reader_Access) return Long_Integer is
      Hi, Lo : Long_Integer;
   begin
      Read_U64_Pair (R, Hi, Lo);
      --  The signed reading of the 64-bit pattern: `Hi * 2**32 + Lo`
      --  wraps to negative once `Hi` carries the sign bit.
      if Hi >= 16#8000_0000# then
         return (Hi - 16#1_0000_0000#) * Pow2 (32) + Lo;
      end if;
      return Hi * Pow2 (32) + Lo;
   end Read_I64;

   function Read_F32 (R : Reader_Access) return Float is
      Pattern : constant Long_Integer := Read_Uint (R, 4);
   begin
      return Bits_To_F32
        (Integer (Sel_Long (Pattern >= 16#8000_0000#,
                            Pattern - 16#1_0000_0000#,
                            Pattern)));
   end Read_F32;

   function Read_F64 (R : Reader_Access) return Long_Float is
      Le, Host : Bytes8;
   begin
      Reader_Align (R, 8);
      Need (R, 8);
      declare
         Octets : Bytes8;
      begin
         for Index in 0 .. 7 loop
            Octets (Index) := Take (R);
         end loop;
         if R.Order = Big then
            for Index in 0 .. 7 loop
               Le (Index) := Octets (7 - Index);
            end loop;
         else
            Le := Octets;
         end if;
      end;
      if Host_Is_Big_64 then
         for Index in 0 .. 7 loop
            Host (Index) := Le (7 - Index);
         end loop;
      else
         Host := Le;
      end if;
      return Bytes_To_F64 (Host);
   end Read_F64;

   function Read_F128 (R : Reader_Access) return Text is
      pragma Unreferenced (R);
   begin
      raise Wire_Not_Implemented;
      return Empty_Text;
   end Read_F128;

   function Read_Unit_Little (R : Reader_Access; Width : Natural) return Long_Integer is
      Value : Long_Integer := 0;
   begin
      Reader_Align (R, Width);
      declare
         Octets : array (0 .. Width - 1) of Byte;
      begin
         for Index in 0 .. Width - 1 loop
            Octets (Index) := Take (R);
         end loop;
         for Index in reverse 0 .. Width - 1 loop
            Value := Value * 256 + Long_Integer (Octets (Index));
         end loop;
      end;
      return Value;
   end Read_Unit_Little;

   function Read_Unit_Big (R : Reader_Access; Width : Natural) return Long_Integer is
      Value : Long_Integer := 0;
   begin
      Reader_Align (R, Width);
      for Index in 1 .. Width loop
         Value := Value * 256 + Long_Integer (Take (R));
      end loop;
      return Value;
   end Read_Unit_Big;

   function Read_Wchar
     (R       : Reader_Access;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) return Long_Integer
   is
      Value : Long_Integer := 0;
   begin
      if Width /= 2 and then Width /= 4 then
         raise Malformed_Stream;
      end if;
      if Framing = Wchar_Giop_Length_Prefixed then
         declare
            Declared : constant Byte := Take (R);
            Big      : constant Boolean := Order /= Unit_Stream or else R.Order /= Little;
         begin
            if Natural (Declared) /= Width then
               raise Malformed_Stream;
            end if;
            Need (R, Width);
            if Big then
               for Index in 1 .. Width loop
                  Value := Value * 256 + Long_Integer (Take (R));
               end loop;
            else
               declare
                  Octets : array (0 .. Width - 1) of Byte;
               begin
                  for Index in 0 .. Width - 1 loop
                     Octets (Index) := Take (R);
                  end loop;
                  for Index in reverse 0 .. Width - 1 loop
                     Value := Value * 256 + Long_Integer (Octets (Index));
                  end loop;
               end;
            end if;
         end;
      elsif Width = 4 then
         Value := Read_Uint (R, 4);
         if Value > 16#FFFF# then
            raise Code_Point_Out_Of_Range;
         end if;
      else
         Value := Read_Uint (R, 2);
      end if;
      if Value >= 16#D800# and then Value <= 16#DFFF# then
         raise Malformed_Stream;
      end if;
      return Value;
   end Read_Wchar;

   function Read_String (R : Reader_Access) return Text is
      Length : constant Natural := Natural (Read_Uint (R, 4));
   begin
      if Length = 0 or else Length > Reader_Remaining (R) + 1 then
         raise Malformed_Stream;
      end if;
      declare
         Value : String (1 .. Length - 1);
      begin
         for Index in Value'Range loop
            Value (Index) := Character'Val (Integer (Take (R)));
         end loop;
         declare
            Ignored : constant Point_Array := Decode_Utf8 (Value);
            pragma Unreferenced (Ignored);
         begin
            null;
         end;
         if Take (R) /= 0 then
            raise Malformed_Stream;
         end if;
         return new String'(Value);
      end;
   end Read_String;

   function Read_String_Alloc (R : Reader_Access) return Text is
   begin
      return Read_String (R);
   end Read_String_Alloc;

   function Read_Wstring_Utf8
     (R : Reader_Access; Policy : Wstring_Policy; Order : Unit_Order) return Text
   is
      Declared : constant Natural := Natural (Read_Uint (R, 4));
      Cell     : constant Wstring_Cell := Cell_Of (Policy);
      Count    : Natural := Declared;
      Fixed_Little : constant Boolean :=
        Order = Unit_Byte_Order_Mark or else Order = Unit_Host_Unswapped;
   begin
      if not Cell.Counts_Characters then
         if Declared mod Cell.Width /= 0 then
            raise Malformed_Stream;
         end if;
         Count := Declared / Cell.Width;
      end if;
      if Count * Cell.Width > Reader_Remaining (R) then
         raise Stream_Truncated;
      end if;
      declare
         Units      : Unit_Array (0 .. Count);
         Have       : Natural := 0;
         First_Unit : Natural := 0;
         Swap_Rest  : Boolean := False;
      begin
         for Index in 0 .. Count - 1 loop
            if Order = Unit_Big_Endian_Always then
               Units (Have) := Read_Unit_Big (R, Cell.Width);
            elsif Fixed_Little then
               Units (Have) := Read_Unit_Little (R, Cell.Width);
            else
               Units (Have) := Read_Uint (R, Cell.Width);
            end if;
            if Units (Have) > 16#FFFF# then
               raise Malformed_Stream;
            end if;
            Have := Have + 1;
         end loop;
         if Cell.Terminated then
            if Declared < 1 or else Have = 0 then
               raise Malformed_Stream;
            end if;
            if Units (Have - 1) /= 0 then
               raise Malformed_Stream;
            end if;
            Have := Have - 1;
            for Index in 0 .. Have - 1 loop
               if Units (Index) = 0 then
                  raise Malformed_Stream;
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
               Units (Index) := ((Units (Index) mod 256) * 256) + Units (Index) / 256;
            end loop;
         end if;
         declare
            Built : Text := Empty_Text;
            Index : Natural := First_Unit;
         begin
            while Index < Have loop
               declare
                  Unit : constant Long_Integer := Units (Index);
               begin
                  if Unit >= 16#D800# and then Unit <= 16#DBFF#
                    and then Index + 1 < Have
                    and then Units (Index + 1) >= 16#DC00#
                    and then Units (Index + 1) <= 16#DFFF#
                  then
                     Append_Point
                       (Built,
                        16#1_0000# + (Unit - 16#D800#) * 1024 + (Units (Index + 1) - 16#DC00#));
                     Index := Index + 2;
                  elsif Unit >= 16#D800# and then Unit <= 16#DFFF# then
                     raise Malformed_Stream;
                  else
                     Append_Point (Built, Unit);
                     Index := Index + 1;
                  end if;
               end;
            end loop;
            return Built;
         end;
      end;
   end Read_Wstring_Utf8;

   function Read_Wstring_Utf8_Octets
     (R : Reader_Access; Policy : Wstring_Policy) return Text
   is
      Declared : constant Natural := Natural (Read_Uint (R, 4));
      Cell     : constant Wstring_Cell := Cell_Of (Policy);
   begin
      if Declared > Reader_Remaining (R) then
         raise Stream_Truncated;
      end if;
      declare
         Raw : String (1 .. Declared);
      begin
         for Index in 1 .. Declared loop
            Raw (Index) := Character'Val (Integer (Take (R)));
         end loop;
         if Cell.Terminated then
            if Declared = 0 or else Raw (Declared) /= Character'Val (0) then
               raise Malformed_Stream;
            end if;
            --  A NUL inside the text is the narrow string's problem in
            --  wide form: every C peer stops at it, so the value this
            --  reader returns and the value the peer returns would
            --  differ for bytes both accepted.
            for Index in 1 .. Declared - 1 loop
               if Raw (Index) = Character'Val (0) then
                  raise Malformed_Stream;
               end if;
            end loop;
            declare
               Ignored : constant Point_Array := Decode_Utf8 (Raw (1 .. Declared - 1));
               pragma Unreferenced (Ignored);
            begin
               null;
            end;
            return new String'(Raw (1 .. Declared - 1));
         end if;
         --  The declared length is the UTF-8 octet count: a value that
         --  is not UTF-8 is refused rather than carried in a Text no
         --  reader could tell was never a wstring.
         declare
            Ignored : constant Point_Array := Decode_Utf8 (Raw);
            pragma Unreferenced (Ignored);
         begin
            null;
         end;
         return new String'(Raw);
      end;
   end Read_Wstring_Utf8_Octets;

   function Read_Fixed
     (R           : Reader_Access;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Text
   is
      Nibbles : Nibble_Array (0 .. 63);
      Have    : Natural := 0;
      Sign    : Natural;
   begin
      if Digit_Count = 0 or else Digit_Count > 31 or else Scale_Count > Digit_Count then
         raise Not_A_Fixed_Value;
      end if;
      if Count /= Fixed_Declared then
         loop
            if Have >= 32 then
               raise Malformed_Stream;
            end if;
            declare
               Octet : constant Byte := Take (R);
            begin
               Nibbles (Have) := Natural (Octet) / 16;
               Nibbles (Have + 1) := Natural (Octet) mod 16;
               Have := Have + 2;
               exit when (Natural (Octet) mod 16) >= 16#0A#;
            end;
         end loop;
      else
         declare
            Octets : constant Natural := (Digit_Count + 2) / 2;
         begin
            for Index in 1 .. Octets loop
               declare
                  Octet : constant Byte := Take (R);
               begin
                  Nibbles (Have) := Natural (Octet) / 16;
                  Nibbles (Have + 1) := Natural (Octet) mod 16;
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
               raise Malformed_Stream;
            end if;
         end loop;
         declare
            Digits_Text : String (1 .. Have - First_Digit);
            Zero        : Boolean := True;
            Split       : Natural;
            Lead        : Natural := 1;
            Result      : Text := Empty_Text;
         begin
            for Index in Digits_Text'Range loop
               Digits_Text (Index) :=
                 Character'Val
                   (Character'Pos ('0') + Nibbles (First_Digit + Index - 1));
            end loop;
            declare
               Padded : constant String :=
                 Left_Padded (Digits_Text, Scale_Count + 1);
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
                  Append_Point (Result, Character'Pos ('-'));
               end if;
               for Index in Lead .. Split loop
                  Append_Point (Result, Character'Pos (Padded (Index)));
               end loop;
               if Scale_Count > 0 then
                  Append_Point (Result, Character'Pos ('.'));
                  for Index in Split + 1 .. Padded'Length loop
                     Append_Point (Result, Character'Pos (Padded (Index)));
                  end loop;
               end if;
               return Result;
            end;
         end;
      end;
   end Read_Fixed;

   function Trim_Utf8 (Value : Text; Octets : Natural) return Text is
      Points : constant Point_Array := Decode_Utf8 (Value.all);
      Kept   : Text := Empty_Text;
      Used   : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant Long_Integer := Points (Index);
            Cost  : constant Natural := Utf8_Cost (Point);
         begin
            exit when Used + Cost > Octets;
            Append_Point (Kept, Point);
            Used := Used + Cost;
         end;
      end loop;
      return Kept;
   end Trim_Utf8;

   function Trim_Utf16 (Value : Text; Units : Natural) return Text is
      Points : constant Point_Array := Decode_Utf8 (Value.all);
      Kept   : Text := Empty_Text;
      Used   : Natural := 0;
   begin
      for Index in Points'Range loop
         declare
            Point : constant Long_Integer := Points (Index);
            Cost  : constant Natural := Sel_Nat (Point > 16#FFFF#, 2, 1);
         begin
            exit when Used + Cost > Units;
            Append_Point (Kept, Point);
            Used := Used + Cost;
         end;
      end loop;
      return Kept;
   end Trim_Utf16;

   function Read_Enum (R : Reader_Access; Bits : Natural) return Long_Integer is
   begin
      if Bits = 0 or else Bits > 32 then
         raise Bit_Out_Of_Range;
      end if;
      if Bits <= 8 then
         return Read_Int (R, 1);
      elsif Bits <= 16 then
         return Read_Int (R, 2);
      end if;
      return Read_Int (R, 4);
   end Read_Enum;

   function Read_Bitmask (R : Reader_Access; Bits : Natural) return Long_Integer is
   begin
      if Bits = 0 or else Bits > 64 then
         raise Bit_Out_Of_Range;
      end if;
      if Bits <= 8 then
         return Read_Uint (R, 1);
      elsif Bits <= 16 then
         return Read_Uint (R, 2);
      elsif Bits <= 32 then
         return Read_Uint (R, 4);
      end if;
      declare
         --  The 64-bit pattern, kept whole, exactly as `Read_U64`
         --  keeps an `unsigned long long`'s: a bitmask whose bound is
         --  64 may set bit 63 (Table 33), and that value is the
         --  negative `Long_Integer` whose bits it is.
         Value : constant Long_Integer := Read_U64 (R);
      begin
         --  A bound between 33 and 63 still has a ceiling, and a
         --  stream naming a bit past it is out of range rather than
         --  quietly wider than the type.
         if Bits < 63 and then (Value < 0 or else Value >= Pow2 (Bits)) then
            raise Value_Out_Of_Range;
         end if;
         if Bits = 63 and then Value < 0 then
            raise Value_Out_Of_Range;
         end if;
         return Value;
      end;
   end Read_Bitmask;

   function Read_Bitset (R : Reader_Access; Anchor : Natural) return Long_Integer is
   begin
      case Anchor is
         when 1 => return Read_Uint (R, 1);
         when 2 => return Read_Uint (R, 2);
         when 4 => return Read_Uint (R, 4);
         --  The pattern whole: an eight-octet anchor packs fields up to
         --  bit 63 (`bitset-multiword`'s three fill all sixty-four),
         --  and refusing a high half here made every such bitset
         --  unreadable in a column that writes it happily.
         when 8 => return Read_U64 (R);
         when others => raise Anchor_Not_A_Holder_Width;
      end case;
   end Read_Bitset;

   function Read_Dheader (R : Reader_Access) return Long_Integer is
      Length : constant Long_Integer := Read_Uint (R, 4);
   begin
      if Length > Long_Integer (Reader_Remaining (R)) then
         raise Stream_Truncated;
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
         Scale : constant Natural := Length_Code_Scale (Header.Length_Code);
      begin
         return 4 + Natural (Header.Next_Int) * Scale;
      end;
   end Em_Extent;

   function Read_Emheader (R : Reader_Access) return Emheader_Read is
      Word   : constant Long_Integer := Read_Uint (R, 4);
      Header : Emheader_Read;
   begin
      Header.Member_Id := Word mod (E_Member_Id_Mask + 1);
      Header.Length_Code := Natural ((Word / Pow2 (28)) mod 8);
      Header.Must_Understand := Word >= 16#8000_0000#;
      Header.Has_Next_Int := False;
      if Header.Length_Code = 4 then
         Header.Next_Int := Read_Uint (R, 4);
         Header.Has_Next_Int := True;
      elsif Header.Length_Code >= 5 then
         declare
            Where : constant Natural := R.At_Pos;
         begin
            Header.Next_Int := Read_Uint (R, 4);
            Header.Has_Next_Int := True;
            R.At_Pos := Where;
         end;
      end if;
      Header.Extent := Em_Extent (Header);
      return Header;
   end Read_Emheader;

   function Read_Pid_Header (R : Reader_Access; Role : Parameter_Role) return Pid_Read is
      Header : Pid_Read;
   begin
      Reader_Align (R, 4);
      declare
         Word     : constant Long_Integer := Read_Uint (R, 2);
         Declared : constant Long_Integer := Read_Uint (R, 2);
      begin
         if (Word mod (Pid_Id_Mask + 1)) = (Pid_Extended_Id mod (Pid_Id_Mask + 1)) then
            declare
               Member_Header : constant Long_Integer := Read_Uint (R, 4);
               Wide_Length   : constant Long_Integer := Read_Uint (R, 4);
            begin
               Header.Pid := Member_Header mod (E_Member_Id_Mask + 1);
               Header.Length := Wide_Length;
               Header.End_Flag := False;
               Header.Must_Understand := Member_Header >= E_Flag_Must_Understand;
               Header.Impl_Extension := False;
               Header.Reserved := False;
               Header.Ignore := False;
               Header.Role := Role;
               return Header;
            end;
         end if;
         declare
            Identifier : constant Long_Integer := Word mod (Pid_Id_Mask + 1);
         begin
            Header.Pid := Identifier;
            Header.Length := Declared;
            Header.End_Flag := Identifier = (Pid_List_End_Id mod (Pid_Id_Mask + 1));
            Header.Must_Understand := (Word / Pid_Must_Understand) mod 2 = 1;
            Header.Impl_Extension :=
              Word >= Pid_Impl_Extension
              and then Flags_At (R.Framing.Pid_Flags, Member_Pos) < Pid_Impl_Extension;
            Header.Reserved :=
              not Header.End_Flag
              and then Identifier > Long_Integer (R.Framing.Highest_Short_Id);
            Header.Ignore := Identifier = Pid_Ignore_Id;
            Header.Role := Role;
            return Header;
         end;
      end;
   end Read_Pid_Header;

   function Read_Pid_As (R : Reader_Access; Role : Parameter_Role) return Pid_Read is
      Header : Pid_Read := Read_Pid_Header (R, Role);
   begin
      Header.Saved_Origin := R.Origin;
      Header.Value_At := R.At_Pos;
      R.Origin := R.At_Pos;
      return Header;
   end Read_Pid_As;

   function Read_Pid (R : Reader_Access) return Pid_Read is
   begin
      return Read_Pid_As (R, Role_Member);
   end Read_Pid;

   procedure Finish_Pid_Read (R : Reader_Access; Header : Pid_Read) is
   begin
      R.Origin := Header.Saved_Origin;
      if R.Current /= Status_Ok or else Header.End_Flag then
         return;
      end if;
      declare
         Used     : constant Natural := R.At_Pos - Header.Value_At;
         Declared : constant Long_Integer := Header.Length;
         Padded   : constant Natural := ((Used + 3) / 4) * 4;
         Pads     : constant Boolean :=
           Sel_Bool (Header.Role = Role_Optional,
                     R.Framing.Pad_Declared_Optional_Length,
                     R.Framing.Pad_Declared_Length);
         Expected : constant Natural := Sel_Nat (Pads, Padded, Used);
      begin
         if Declared /= Long_Integer (Expected)
           and then Declared /= Long_Integer (Used)
           and then Declared /= Long_Integer (Padded)
         then
            Reader_Refuse (R, Status_Out_Of_Range);
         end if;
      end;
      if R.Framing.Pad_After_Value then
         Reader_Align (R, 4);
      end if;
   end Finish_Pid_Read;

   --  --  the JSON the probes print  -------------------------------------

   function Json_Bool (Value : Boolean) return Text is
   begin
      if Value then
         return Text_Of ("true");
      end if;
      return Text_Of ("false");
   end Json_Bool;

   function Json_Int (Value : Long_Integer) return Text is
      Raw : constant String := Long_Integer'Image (Value);
   begin
      if Raw (Raw'First) = ' ' then
         return Text_Of (Raw (Raw'First + 1 .. Raw'Last));
      end if;
      return Text_Of (Raw);
   end Json_Int;

   function Json_I64 (Value : Long_Integer) return Text is
      Digits_Text : constant Text := Json_Int (Value);
   begin
      return Text_Of ("""" & Digits_Text.all & """");
   end Json_I64;

   function Json_U64 (Value : Long_Integer) return Text is
      --  The unsigned decimal of a 64-bit pattern, which is not
      --  `Json_Int`'s answer: `Value` is negative exactly where the
      --  wire value is 2**63 or more, and no `Long_Integer` holds that
      --  value to divide by ten. The division is done on the pattern's
      --  two 32-bit limbs instead — schoolbook long division, one
      --  decimal digit per pass, least significant first. `Cur` stays
      --  below 10 * 2**32 + 2**32, so every intermediate fits.
      Base   : constant Long_Integer := Pow2 (32);
      Lo     : Long_Integer := Value mod Base;
      Hi     : Long_Integer := (Value - (Value mod Base)) / Base;
      --  2**64 - 1 is twenty decimal digits, and nothing longer can
      --  reach here.
      Buffer : String (1 .. 20);
      Next   : Natural := 20;
      Cur    : Long_Integer;
      Rest   : Long_Integer;
   begin
      if Hi < 0 then
         Hi := Hi + Base;
      end if;
      loop
         Cur := Hi;
         Hi := Cur / 10;
         Rest := Cur mod 10;
         Cur := Rest * Base + Lo;
         Lo := Cur / 10;
         Rest := Cur mod 10;
         Buffer (Next) := Character'Val (Character'Pos ('0') + Integer (Rest));
         Next := Next - 1;
         exit when Hi = 0 and then Lo = 0;
      end loop;
      --  Quoted, which is the one thing this shares with the reading
      --  every other column gives a 64-bit width: a JSON number is a
      --  double and loses the low bits past 2**53.
      return Text_Of ("""" & Buffer (Next + 1 .. 20) & """");
   end Json_U64;

   function Json_F64 (Value : Long_Float; Where : String) return Text is
      pragma Unreferenced (Where);
      package F64_IO is new Text_IO.Float_IO (Long_Float);
      Line : String (1 .. 40);
   begin
      if Value /= Value or else Value > Long_Float'Last or else Value < Long_Float'First then
         raise Malformed_Stream;
      end if;
      F64_IO.Put (To => Line, Item => Value, Aft => 16, Exp => 3);
      declare
         First : Natural := Line'First;
      begin
         while First <= Line'Last and then Line (First) = ' ' loop
            First := First + 1;
         end loop;
         return Text_Of (Line (First .. Line'Last));
      end;
   end Json_F64;

   function Json_F32 (Value : Float; Where : String) return Text is
   begin
      return Json_F64 (Long_Float (Value), Where);
   end Json_F32;

   function Json_Char (Point : Long_Integer) return Text is
      Bare : Text := Empty_Text;
   begin
      Append_Point (Bare, Point);
      return Json_String (Bare);
   end Json_Char;

   function Json_String (Value : Text) return Text is
      Raw    : constant String := Value.all;
      Out_S  : Text := Text_Of ("""");
      Hex    : constant String := "0123456789abcdef";
   begin
      for Index in Raw'Range loop
         declare
            Octet : constant Natural := Character'Pos (Raw (Index));
         begin
            if Raw (Index) = '"' then
               Json_Append (Out_S, "\""");
            elsif Raw (Index) = '\' then
               Json_Append (Out_S, "\\");
            elsif Octet = 10 then
               Json_Append (Out_S, "\n");
            elsif Octet = 13 then
               Json_Append (Out_S, "\r");
            elsif Octet = 9 then
               Json_Append (Out_S, "\t");
            elsif Octet < 16#20# then
               Json_Append (Out_S, "\u00" & Hex (Hex'First + Octet / 16 .. Hex'First + Octet / 16)
                            & Hex (Hex'First + Octet mod 16 .. Hex'First + Octet mod 16));
            else
               Json_Append (Out_S, Raw (Index .. Index));
            end if;
         end;
      end loop;
      Json_Append (Out_S, """");
      return Out_S;
   end Json_String;

   procedure Json_Append (Obj : in out Text; More : String) is
      Grown : constant Text := new String (1 .. Obj.all'Length + More'Length);
   begin
      Grown (1 .. Obj.all'Length) := Obj.all;
      Grown (Obj.all'Length + 1 .. Grown'Last) := More;
      Obj := Grown;
   end Json_Append;

   procedure Json_Member (Obj : in out Text; Key : String; Value : Text) is
   begin
      if Obj.all'Length > 0 and then Obj (Obj.all'Last) /= '{' then
         Json_Append (Obj, ",");
      end if;
      Json_Append (Obj, """" & Key & """:");
      Json_Append (Obj, Value.all);
   end Json_Member;

end Zd_Wire83;
