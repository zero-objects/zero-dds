--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  A self-test of the Ada 83 wire runtime against fixed octets. It
--  follows `runtime/ada/tests/zd_wire_selftest.adb` in what it checks
--  and differs in what Ada 83 has: no `Ada.Command_Line` and therefore
--  no `Set_Exit_Status` — the C library's own `exit` is imported, as
--  the generated probes do — no `Ada.Text_IO`, and no `Interfaces`.
--
--     gprbuild -P ../zd_wire83.gpr && ../obj/zd_wire83_selftest
--
--  Exit 0 on agreement, exit 1 with the first disagreement named. Every
--  step says what it is about to do before doing it: this runtime had
--  no test at all when it was written, and the first failure it ever
--  produced was a `Constraint_Error` with nothing to say where.

with Text_IO;
with Zd_Wire83;

--  `use`, not `use type`: the latter is Ada 95. Without it the "=" on
--  `Byte`, `Status` and `Byte_Array_Access` is not directly visible and
--  every comparison below is an error.
use Zd_Wire83;

procedure Zd_Wire83_Selftest is

   procedure Zd83_Exit (Status : Integer);
   pragma Import (C, Zd83_Exit, "exit");

   Failed : Boolean := False;
   Step   : Natural := 0;

   --  Above the bodies below, as RM83 3.9 orders a declarative part —
   --  the same rule this runtime broke eight times.
   W : Zd_Wire83.Writer_Access;
   R : Zd_Wire83.Reader_Access;
   B : Zd_Wire83.Byte_Array_Access;
   D : Zd_Wire83.Dheader;

   procedure Note (Name : String) is
   begin
      Step := Step + 1;
      Text_IO.Put_Line ("step" & Natural'Image (Step) & ": " & Name);
   end Note;

   procedure Check (Name : String; Ok : Boolean) is
   begin
      if not Ok then
         Text_IO.Put_Line (Text_IO.Standard_Error, "selftest: " & Name);
         Failed := True;
      end if;
   end Check;

   --  The framing the specification dialect uses, as the generated
   --  probes spell it.
   function Spec_Framing return Zd_Wire83.Pl_Cdr1_Framing is
      F : Zd_Wire83.Pl_Cdr1_Framing;
   begin
      F.Pid_Flags := Zd_Wire83.Flags_Sentinel_Must_Understand;
      F.Highest_Short_Id := 16128;
      F.Pad_Declared_Length := True;
      F.Pad_Declared_Optional_Length := True;
      F.Member_Must_Understand := True;
      F.Pad_After_Value := True;
      return F;
   end Spec_Framing;

   function New_Le_Writer return Zd_Wire83.Writer_Access is
   begin
      return Zd_Wire83.New_Writer (Zd_Wire83.Little, 8, Spec_Framing, False);
   end New_Le_Writer;

   --  The same framing over a big-endian stream: the two `wstring`
   --  policies that fix their unit order against the stream's, and the
   --  `wchar` that does not, only differ where the stream is not
   --  little-endian.
   function New_Be_Writer return Zd_Wire83.Writer_Access is
   begin
      return Zd_Wire83.New_Writer (Zd_Wire83.Big, 8, Spec_Framing, False);
   end New_Be_Writer;

   procedure Check_Octets
     (Name     : String;
      Actual   : Zd_Wire83.Byte_Array_Access;
      Expected : Zd_Wire83.Byte_Array) is
   begin
      if Actual = null then
         Check (Name & ": no octets at all", False);
         return;
      end if;
      if Actual'Length /= Expected'Length then
         Check
           (Name & ":" & Natural'Image (Actual'Length) & " octets, expected"
            & Natural'Image (Expected'Length),
            False);
         return;
      end if;
      for Index in Natural range 0 .. Expected'Length - 1 loop
         if Actual (Actual'First + Index) /= Expected (Expected'First + Index) then
            Check
              (Name & ": octet" & Natural'Image (Index) & " is"
               & Zd_Wire83.Byte'Image (Actual (Actual'First + Index))
               & ", expected" & Zd_Wire83.Byte'Image (Expected (Expected'First + Index)),
               False);
            return;
         end if;
      end loop;
   end Check_Octets;

begin
   --  One `long`, little-endian: 7.4.1.2.2's four octets, least
   --  significant first.
   Note ("write one long");
   W := New_Le_Writer;
   Zd_Wire83.Write_Int (W, 1, 4, -2147483648, 2147483647);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("one long", B, (16#01#, 16#00#, 16#00#, 16#00#));

   --  A negative one: two's complement, which the runtime reaches
   --  through `mod` rather than a shift.
   Note ("write a negative long");
   W := New_Le_Writer;
   Zd_Wire83.Write_Int (W, -1234567, 4, -2147483648, 2147483647);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("negative long", B, (16#79#, 16#29#, 16#ED#, 16#FF#));

   --  The widest positive one a `long` holds.
   Note ("write long'last");
   W := New_Le_Writer;
   Zd_Wire83.Write_Int (W, 2147483647, 4, -2147483648, 2147483647);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("long'last", B, (16#FF#, 16#FF#, 16#FF#, 16#7F#));

   --  The pair the `primitive-long` case writes, alignment included.
   Note ("write the primitive-long pair");
   W := New_Le_Writer;
   D := Zd_Wire83.No_Dheader;
   Zd_Wire83.Write_Int (W, -1234567, 4, -2147483648, 2147483647);
   Zd_Wire83.Write_Int (W, 2147483647, 4, -2147483648, 2147483647);
   Zd_Wire83.Finish_Dheader (W, D);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("primitive-long pair", B,
      (16#79#, 16#29#, 16#ED#, 16#FF#, 16#FF#, 16#FF#, 16#FF#, 16#7F#));

   --  The narrow widths, each against the range it admits.
   Note ("write the narrow widths");
   W := New_Le_Writer;
   Zd_Wire83.Write_Int (W, -1, 1, -128, 127);
   Zd_Wire83.Write_Int (W, 255, 1, 0, 255);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("octet pair", B, (16#FF#, 16#FF#));

   --  A value outside the range is refused rather than truncated.
   Note ("refuse a value out of range");
   W := New_Le_Writer;
   declare
      Raised : Boolean := False;
   begin
      Zd_Wire83.Write_Int (W, 128, 1, -128, 127);
   exception
      when Zd_Wire83.Value_Out_Of_Range =>
         Raised := True;
      when others =>
         Raised := False;
   end;

   --  A boolean is one octet.
   Note ("write a boolean");
   W := New_Le_Writer;
   Zd_Wire83.Write_Bool (W, True);
   Zd_Wire83.Write_Bool (W, False);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("boolean pair", B, (16#01#, 16#00#));

   --  And back: what the writer wrote, the reader reads.
   Note ("read the pair back");
   W := New_Le_Writer;
   Zd_Wire83.Write_Int (W, -1234567, 4, -2147483648, 2147483647);
   Zd_Wire83.Write_Int (W, 2147483647, 4, -2147483648, 2147483647);
   B := Zd_Wire83.Finish (W);
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check ("round trip: first long", Zd_Wire83.Read_Int (R, 4) = -1234567);
   Check ("round trip: second long", Zd_Wire83.Read_Int (R, 4) = 2147483647);
   Check ("round trip: nothing left", Zd_Wire83.Reader_Remaining (R) = 0);
   Check
     ("round trip: status ok",
      Zd_Wire83.Reader_Status (R) = Zd_Wire83.Status_Ok);

   --  The top half of `unsigned long long`'s range. No signed 64-bit
   --  `Long_Integer` reaches past 2**63 - 1, so a value from that half
   --  travels as the two's-complement pattern — the negative
   --  `Long_Integer` whose bits it is. 2**64 - 1 is all ones; 2**63 is
   --  the sign bit alone, which is `Long_Integer'First`.
   Note ("write the top half of unsigned long long");
   W := New_Le_Writer;
   Zd_Wire83.Write_U64 (W, -1);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("2**64 - 1", B,
      (16#FF#, 16#FF#, 16#FF#, 16#FF#, 16#FF#, 16#FF#, 16#FF#, 16#FF#));

   Note ("write 2**63");
   W := New_Le_Writer;
   Zd_Wire83.Write_U64 (W, Long_Integer'First);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("2**63", B,
      (16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#80#));

   --  And a value from the lower half, which is its own pattern: the
   --  split must not have changed for it.
   Note ("write an unsigned long long below 2**63");
   W := New_Le_Writer;
   Zd_Wire83.Write_U64 (W, 16#0102_0304_0506_0708#);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("below 2**63", B,
      (16#08#, 16#07#, 16#06#, 16#05#, 16#04#, 16#03#, 16#02#, 16#01#));

   Note ("read the top half back");
   W := New_Le_Writer;
   Zd_Wire83.Write_U64 (W, -1);
   Zd_Wire83.Write_U64 (W, Long_Integer'First);
   Zd_Wire83.Write_U64 (W, 16#0102_0304_0506_0708#);
   B := Zd_Wire83.Finish (W);
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check ("u64 round trip: 2**64 - 1", Zd_Wire83.Read_U64 (R) = -1);
   Check ("u64 round trip: 2**63", Zd_Wire83.Read_U64 (R) = Long_Integer'First);
   Check
     ("u64 round trip: below 2**63",
      Zd_Wire83.Read_U64 (R) = 16#0102_0304_0506_0708#);
   Check ("u64 round trip: nothing left", Zd_Wire83.Reader_Remaining (R) = 0);

   --  The JSON reading of that pattern is the unsigned decimal, not the
   --  negative number the pattern is held as, and it is quoted because
   --  a JSON number is a double and loses the low bits past 2**53.
   Note ("render the top half as JSON");
   Check
     ("json 2**64 - 1",
      Zd_Wire83.Json_U64 (-1).all = """18446744073709551615""");
   Check
     ("json 2**63",
      Zd_Wire83.Json_U64 (Long_Integer'First).all = """9223372036854775808""");
   Check ("json zero", Zd_Wire83.Json_U64 (0).all = """0""");
   Check
     ("json below 2**63",
      Zd_Wire83.Json_U64 (16#0102_0304_0506_0708#).all = """72623859790382856""");

   --  A `long long` is quoted too, and keeps its sign: both 64-bit
   --  widths are strings in the dialect `crates/wire-probe` documents,
   --  while every narrower width is a bare number.
   Note ("render a long long as JSON");
   Check
     ("json i64 negative",
      Zd_Wire83.Json_I64 (-987654321987).all = """-987654321987""");
   Check
     ("json i64 long_integer'last",
      Zd_Wire83.Json_I64 (Long_Integer'Last).all = """9223372036854775807""");
   Check ("json int stays bare", Zd_Wire83.Json_Int (-1).all = "-1");

   --  The five operators RM83 4.5.1 declares for `Boolean` and not for
   --  an integer type. Every one of them reads and answers the 64-bit
   --  pattern, so `Bits_Not (0)` is all sixty-four bits set — which is
   --  -1 — rather than an overflow.
   Note ("mask, join, complement and shift a 64-bit pattern");
   Check ("and of disjoint halves", Zd_Wire83.Bits_And (16#F0F0#, 16#0F0F#) = 0);
   Check
     ("and keeps the overlap",
      Zd_Wire83.Bits_And (16#FFFF_FFFF_FFFF#, 16#00FF_00FF_00FF#) = 16#00FF_00FF_00FF#);
   Check ("and across the halves", Zd_Wire83.Bits_And (-1, 16#1_0000_0000#) = 16#1_0000_0000#);
   Check ("or of disjoint halves", Zd_Wire83.Bits_Or (16#F0F0#, 16#0F0F#) = 16#FFFF#);
   Check ("or across the halves", Zd_Wire83.Bits_Or (16#1_0000_0000#, 1) = 16#1_0000_0001#);
   Check ("not of zero is every bit", Zd_Wire83.Bits_Not (0) = -1);
   Check ("not of every bit is zero", Zd_Wire83.Bits_Not (-1) = 0);
   Check ("not of one", Zd_Wire83.Bits_Not (1) = -2);
   Check ("and with not is the subset test", Zd_Wire83.Bits_And (16#8001#, Zd_Wire83.Bits_Not (16#8001#)) = 0);
   Check ("subset of itself", Zd_Wire83.Bits_Subset (16#8001#, 16#8001#));
   Check ("not a subset", not Zd_Wire83.Bits_Subset (16#8003#, 16#8001#));
   --  The one the old division loop answered wrong: a mask whose bit 63
   --  is declared is a negative `Long_Integer`, and a value carrying
   --  that bit is inside it.
   Check ("subset with bit 63 declared", Zd_Wire83.Bits_Subset (Long_Integer'First, Long_Integer'First));
   Check
     ("bit 63 outside a narrow mask",
      not Zd_Wire83.Bits_Subset (Long_Integer'First, 16#FFFF#));

   --  The unsigned order over the same pattern. `Long_Integer` is
   --  signed and a `map` key at or above 2**63 is the negative value
   --  carrying those bits, so the predefined `<` puts
   --  16045690984833335998 -- 16#DEAD_BEEF_DEAD_C2BE#, and negative
   --  here -- ahead of 7, where Fast DDS and OpenDDS write 7 first.
   Note ("compare two 64-bit patterns as unsigned values");
   Check
     ("the pattern with bit 63 set is the larger",
      Zd_Wire83.U64_Less (7, Zd_Wire83.Bits_Or (16#5EAD_BEEF_DEAD_C2BE#, Long_Integer'First)));
   Check
     ("and not the smaller",
      not Zd_Wire83.U64_Less
        (Zd_Wire83.Bits_Or (16#5EAD_BEEF_DEAD_C2BE#, Long_Integer'First), 7));
   Check ("2**64 - 1 is above everything", Zd_Wire83.U64_Less (Long_Integer'Last, -1));
   Check ("and nothing is above it", not Zd_Wire83.U64_Less (-1, Long_Integer'Last));
   Check ("two patterns below 2**63 compare as they are", Zd_Wire83.U64_Less (7, 8));
   Check ("two patterns above 2**63 compare as they are", Zd_Wire83.U64_Less (-2, -1));
   Check ("no value is below itself", not Zd_Wire83.U64_Less (-1, -1));
   Check ("zero is below every other pattern", Zd_Wire83.U64_Less (0, Long_Integer'First));

   Note ("shift a 64-bit pattern either way");
   Check ("shift left by nothing", Zd_Wire83.Shift_Left (12345, 0) = 12345);
   Check ("shift left inside a half", Zd_Wire83.Shift_Left (1, 31) = 16#8000_0000#);
   Check ("shift left across the halves", Zd_Wire83.Shift_Left (1, 32) = 16#1_0000_0000#);
   --  Bit 63 as a product would be past `Long_Integer'Last`; as a shift
   --  it is `Long_Integer'First`, which is the pattern.
   Check ("shift left onto bit 63", Zd_Wire83.Shift_Left (1, 63) = Long_Integer'First);
   Check ("shift left past the width", Zd_Wire83.Shift_Left (1, 64) = 0);
   Check ("shift right by nothing", Zd_Wire83.Shift_Right (12345, 0) = 12345);
   Check ("shift right inside a half", Zd_Wire83.Shift_Right (16#8000_0000#, 31) = 1);
   Check ("shift right across the halves", Zd_Wire83.Shift_Right (16#1_0000_0000#, 32) = 1);
   --  Logical and not arithmetic: bit 63 leaves and a zero arrives.
   Check ("shift right off bit 63", Zd_Wire83.Shift_Right (Long_Integer'First, 63) = 1);
   Check ("shift right off bit 63 by one", Zd_Wire83.Shift_Right (-1, 1) = 16#7FFF_FFFF_FFFF_FFFF#);
   Check ("shift right past the width", Zd_Wire83.Shift_Right (-1, 64) = 0);

   --  A bitset whose fields fill all sixty-four bits — `bitset-multiword`
   --  is 20 + 20 + 24 — packs a field that ends on bit 63, and the
   --  anchor it packs into is the pattern.
   Note ("pack and unpack an eight-octet bitset anchor");
   Check ("first field", Zd_Wire83.Bitfield (16#FFFFF#, 20, 0) = 16#FFFFF#);
   Check ("second field", Zd_Wire83.Bitfield (1, 20, 20) = 16#10_0000#);
   Check ("third field reaches bit 63", Zd_Wire83.Bitfield (16#80_0000#, 24, 40) = Long_Integer'First);
   W := New_Le_Writer;
   Zd_Wire83.Write_Bitset
     (W,
      8,
      Zd_Wire83.Bits_Or
        (Zd_Wire83.Bits_Or (Zd_Wire83.Bitfield (16#FFFFF#, 20, 0), Zd_Wire83.Bitfield (1, 20, 20)),
         Zd_Wire83.Bitfield (16#80_0000#, 24, 40)));
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("bitset anchor with bit 63 set", B,
      (16#FF#, 16#FF#, 16#1F#, 16#00#, 16#00#, 16#00#, 16#00#, 16#80#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   declare
      Anchor : constant Long_Integer := Zd_Wire83.Read_Bitset (R, 8);
   begin
      Check ("bitset anchor round trip: low field",
             Zd_Wire83.Bits_And (Anchor, 16#FFFFF#) = 16#FFFFF#);
      Check ("bitset anchor round trip: high field",
             Zd_Wire83.Shift_Right (Anchor, 40) = 16#80_0000#);
   end;

   --  A 64-bit bitmask may declare a bit at position 63 (Table 33), so
   --  its value reaches the same half of the range an `unsigned long
   --  long` does and travels the same way.
   Note ("write and read a bitmask with bit 63 set");
   W := New_Le_Writer;
   Zd_Wire83.Write_Bitmask (W, 64, Zd_Wire83.Bits_Or (Long_Integer'First, 3));
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("bitmask with bit 63 set", B,
      (16#03#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#00#, 16#80#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check
     ("bitmask round trip with bit 63",
      Zd_Wire83.Read_Bitmask (R, 64) = Zd_Wire83.Bits_Or (Long_Integer'First, 3));

   --  A GIOP 1.2 `wchar`: one octet of length and then the code unit,
   --  with nothing between them. `primitive-wchar`'s two scalars are
   --  U+00E4 and U+1234, which is six octets in every column and was
   --  eight in this one — the writer aligned the unit to its own width
   --  and the reader did not.
   Note ("write and read a GIOP length-prefixed wchar");
   W := New_Le_Writer;
   Zd_Wire83.Write_Wchar
     (W, 16#00E4#, 2, Zd_Wire83.Wchar_Giop_Length_Prefixed, Zd_Wire83.Unit_Stream);
   Zd_Wire83.Write_Wchar
     (W, 16#1234#, 2, Zd_Wire83.Wchar_Giop_Length_Prefixed, Zd_Wire83.Unit_Stream);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("giop wchar pair", B, (16#02#, 16#E4#, 16#00#, 16#02#, 16#34#, 16#12#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check
     ("giop wchar round trip: U+00E4",
      Zd_Wire83.Read_Wchar
        (R, 2, Zd_Wire83.Wchar_Giop_Length_Prefixed, Zd_Wire83.Unit_Stream) = 16#00E4#);
   Check
     ("giop wchar round trip: U+1234",
      Zd_Wire83.Read_Wchar
        (R, 2, Zd_Wire83.Wchar_Giop_Length_Prefixed, Zd_Wire83.Unit_Stream) = 16#1234#);
   Check ("giop wchar round trip: nothing left", Zd_Wire83.Reader_Remaining (R) = 0);

   --  A `wstring` whose units are fixed little-endian while the stream
   --  is big-endian: `Unit_Host_Unswapped`, which `ros-fastrtps` asks
   --  for. The unit order is fixed *against* the stream's, so writing
   --  it in the stream's order — which is what the writer did — put
   --  four octets down that the reader, taking them little-endian,
   --  read as a value past 16#FFFF#.
   Note ("write a wstring whose unit order is fixed against the stream");
   W := New_Be_Writer;
   Zd_Wire83.Write_Wstring_Utf8
     (W,
      Zd_Wire83.Text_Of ("AB"),
      Zd_Wire83.Wstr_Characters_Unterminated_4,
      Zd_Wire83.Unit_Host_Unswapped);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("host-unswapped wstring over a big-endian stream", B,
      (16#00#, 16#00#, 16#00#, 16#02#,
       16#41#, 16#00#, 16#00#, 16#00#,
       16#42#, 16#00#, 16#00#, 16#00#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Big, 8, Spec_Framing);
   Check
     ("host-unswapped wstring round trip",
      Zd_Wire83.Read_Wstring_Utf8
        (R, Zd_Wire83.Wstr_Characters_Unterminated_4, Zd_Wire83.Unit_Host_Unswapped).all
      = "AB");

   --  A `wchar` outside GIOP's framing goes in the stream's own order,
   --  which is the one thing `Write_Unit_Little` must not be used for.
   Note ("write a fixed-width wchar over a big-endian stream");
   W := New_Be_Writer;
   Zd_Wire83.Write_Wchar (W, 16#1234#, 2, Zd_Wire83.Wchar_Fixed, Zd_Wire83.Unit_Stream);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("fixed wchar over a big-endian stream", B, (16#12#, 16#34#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Big, 8, Spec_Framing);
   Check
     ("fixed wchar round trip over a big-endian stream",
      Zd_Wire83.Read_Wchar (R, 2, Zd_Wire83.Wchar_Fixed, Zd_Wire83.Unit_Stream) = 16#1234#);

   --  A `double`, 7.4.1.2.2's eight octets. The pattern of
   --  3.141592653589793 is 16#4009_21FB_5444_2D18#, least significant
   --  octet first.
   Note ("write a double");
   W := New_Le_Writer;
   Zd_Wire83.Write_F64 (W, 3.141592653589793);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("double pi", B,
      (16#18#, 16#2D#, 16#44#, 16#54#, 16#FB#, 16#21#, 16#09#, 16#40#));

   --  And back. This is the step that catches a host-order probe read
   --  the wrong way round: the octets above came back as 3.2E-192,
   --  which is the same eight octets in the opposite order.
   Note ("read a double back");
   W := New_Le_Writer;
   Zd_Wire83.Write_F64 (W, 3.141592653589793);
   Zd_Wire83.Write_F64 (W, -2.5);
   Zd_Wire83.Write_F64 (W, 1.0);
   B := Zd_Wire83.Finish (W);
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check ("double round trip: pi", Zd_Wire83.Read_F64 (R) = 3.141592653589793);
   Check ("double round trip: -2.5", Zd_Wire83.Read_F64 (R) = -2.5);
   Check ("double round trip: 1.0", Zd_Wire83.Read_F64 (R) = 1.0);
   Check ("double round trip: nothing left", Zd_Wire83.Reader_Remaining (R) = 0);

   --  The same probe builds the two values that have no literal. A NaN
   --  is the only value not equal to itself, and an infinity is past
   --  every finite one.
   Note ("build the values that have no literal");
   Check ("f64 nan", Zd_Wire83.F64_Nan /= Zd_Wire83.F64_Nan);
   Check ("f64 inf", Zd_Wire83.F64_Inf > 1.0E300);
   Check ("f32 nan", Zd_Wire83.F32_Nan /= Zd_Wire83.F32_Nan);
   Check ("f32 inf", Zd_Wire83.F32_Inf > 1.0E30);

   --  A `float` for the same reason, which goes through a different
   --  route in this runtime and stayed right while the `double` did
   --  not.
   Note ("write and read a float");
   W := New_Le_Writer;
   Zd_Wire83.Write_F32 (W, -3.75);
   B := Zd_Wire83.Finish (W);
   Check_Octets ("float -3.75", B, (16#00#, 16#00#, 16#70#, 16#C0#));
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check ("float round trip", Zd_Wire83.Read_F32 (R) = -3.75);

   --  SequenceElementDelimiter::BeforeEachNonFinalElement: a sequence
   --  element of a non-@final type carries its own octet length in
   --  DHEADER form. Expected octets from
   --  tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
   --  (06 00 00 00 01 00 00 00 02 00, the length of the first `many` Cell
   --  and the Cell itself).
   Note ("write a sequence element's own length in DHEADER form");
   W := New_Le_Writer;
   D := Zd_Wire83.Write_Dheader (W);
   Zd_Wire83.Write_Int (W, 1, 4, -2147483648, 2147483647);
   Zd_Wire83.Write_Int (W, 2, 2, -32768, 32767);
   Zd_Wire83.Finish_Dheader (W, D);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("sequence element length", B,
      (16#06#, 0, 0, 0, 16#01#, 0, 0, 0, 16#02#, 0));

   --  WstringCodeUnits::Utf8: the characters' own octets, with the
   --  length an octet count rather than 7.4.1.1.2's "twice the number
   --  of characters". Expected octets from
   --  tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
   --  (04 00 00 00 61 62 63 00 for "abc"),
   --  tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin
   --  (0d 00 00 00, twelve octets, a NUL) and
   --  tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
   --  (07 00 00 00 e1 88 b4 c3 a4 5a 00 for wstring<16> "X").
   Note ("write a UTF-8-octet wstring");
   W := New_Le_Writer;
   Zd_Wire83.Write_Wstring_Utf8_Octets
     (W, Zd_Wire83.Text_Of ("abc"), Zd_Wire83.Wstr_Octets_Terminated_2);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("utf8 wstring abc", B,
      (16#04#, 0, 0, 0, 16#61#, 16#62#, 16#63#, 0));

   Note ("write a multi-byte UTF-8-octet wstring");
   W := New_Le_Writer;
   Zd_Wire83.Write_Wstring_Utf8_Octets
     (W,
      Zd_Wire83.Text_From_Utf8
        ((16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
          16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#)),
      Zd_Wire83.Wstr_Octets_Terminated_2);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("utf8 wstring multibyte", B,
      (16#0D#, 0, 0, 0,
       16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
       16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#, 0));

   Note ("write a bounded UTF-8-octet wstring");
   W := New_Le_Writer;
   Zd_Wire83.Write_Wstring_Utf8_Octets_Bounded
     (W,
      Zd_Wire83.Text_From_Utf8 ((16#E1#, 16#88#, 16#B4#, 16#C3#, 16#A4#, 16#5A#)),
      3, Zd_Wire83.Wstr_Octets_Terminated_2);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("utf8 wstring bounded", B,
      (16#07#, 0, 0, 0, 16#E1#, 16#88#, 16#B4#, 16#C3#, 16#A4#, 16#5A#, 0));

   Note ("read a UTF-8-octet wstring back");
   W := New_Le_Writer;
   Zd_Wire83.Write_Wstring_Utf8_Octets
     (W,
      Zd_Wire83.Text_From_Utf8
        ((16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
          16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#)),
      Zd_Wire83.Wstr_Octets_Terminated_2);
   B := Zd_Wire83.Finish (W);
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   Check
     ("utf8 wstring round trip",
      Zd_Wire83.Strcmp
        (Zd_Wire83.Read_Wstring_Utf8_Octets (R, Zd_Wire83.Wstr_Octets_Terminated_2),
         Zd_Wire83.Text_From_Utf8
           ((16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
             16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#)))
      = 0);
   Check ("utf8 wstring read: nothing left", Zd_Wire83.Reader_Remaining (R) = 0);

   --  ElementPadding::Packed: each collection element and each aggregate
   --  value counts alignment from its own first octet. Expected octets
   --  from tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
   --  (01 00 00 00 02 00 03 00 00 00 04 00 77, the second Cell's long at
   --  offset six).
   Note ("write packed collection elements");
   W := New_Le_Writer;
   declare
      Saved : Natural;
   begin
      Saved := Zd_Wire83.Begin_Packed_Element (W);
      Zd_Wire83.Write_Int (W, 1, 4, -2147483648, 2147483647);
      Zd_Wire83.Write_Int (W, 2, 2, -32768, 32767);
      Zd_Wire83.Finish_Packed_Element (W, Saved);
      Saved := Zd_Wire83.Begin_Packed_Element (W);
      Zd_Wire83.Write_Int (W, 3, 4, -2147483648, 2147483647);
      Zd_Wire83.Write_Int (W, 4, 2, -32768, 32767);
      Zd_Wire83.Finish_Packed_Element (W, Saved);
   end;
   Zd_Wire83.Write_Int (W, 119, 1, 0, 255);
   B := Zd_Wire83.Finish (W);
   Check_Octets
     ("packed array elements", B,
      (16#01#, 0, 0, 0, 16#02#, 0, 16#03#, 0, 0, 0, 16#04#, 0, 16#77#));

   Note ("read packed collection elements back");
   R := Zd_Wire83.New_Reader (B, Zd_Wire83.Little, 8, Spec_Framing);
   declare
      Saved  : Natural;
      A1, A2 : Long_Integer;
      B1, B2 : Long_Integer;
   begin
      Saved := Zd_Wire83.Reader_Begin_Packed_Element (R);
      A1 := Zd_Wire83.Read_Int (R, 4);
      B1 := Zd_Wire83.Read_Int (R, 2);
      Zd_Wire83.Reader_Finish_Packed_Element (R, Saved);
      Saved := Zd_Wire83.Reader_Begin_Packed_Element (R);
      A2 := Zd_Wire83.Read_Int (R, 4);
      B2 := Zd_Wire83.Read_Int (R, 2);
      Zd_Wire83.Reader_Finish_Packed_Element (R, Saved);
      Check
        ("packed read back",
         A1 = 1 and then B1 = 2 and then A2 = 3 and then B2 = 4);
      Check ("packed read: the last byte", Zd_Wire83.Read_Uint (R, 1) = 119);
   end;

   if Failed then
      Text_IO.Put_Line (Text_IO.Standard_Error, "selftest: failed");
      Zd83_Exit (1);
   end if;
   Text_IO.Put_Line ("selftest: ok");
   Zd83_Exit (0);
end Zd_Wire83_Selftest;
