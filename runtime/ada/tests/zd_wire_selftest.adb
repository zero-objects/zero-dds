--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  A self-test of the Ada wire runtime against fixed octets: alignment,
--  the string convention, the enumeration holder, the PL_CDR1 short
--  header and one round trip through the reader. Exit 0 on agreement,
--  exit 1 with the first disagreement named.
--
--     gprbuild -P ../zd_wire.gpr && ../obj/zd_wire_selftest

with Ada.Command_Line;
with Ada.Text_IO;
with Interfaces;
with Zd_Wire;

procedure Zd_Wire_Selftest is

   use type Interfaces.Unsigned_8;
   use type Interfaces.Unsigned_32;
   use type Interfaces.Integer_64;
   use type Interfaces.Integer_32;
   use type Interfaces.Integer_16;
   use type Zd_Wire.Status;

   Failed : Boolean := False;

   procedure Check (Name : String; Ok : Boolean) is
   begin
      if not Ok then
         Ada.Text_IO.Put_Line (Ada.Text_IO.Standard_Error, "selftest: " & Name);
         Failed := True;
      end if;
   end Check;

   function Same (Left : Zd_Wire.Byte_Array_Access; Right : Zd_Wire.Byte_Array)
     return Boolean is
   begin
      if Left'Length /= Right'Length then
         return False;
      end if;
      for Index in 0 .. Left'Length - 1 loop
         if Left (Left'First + Index) /= Right (Right'First + Index) then
            return False;
         end if;
      end loop;
      return True;
   end Same;

   Framing : constant Zd_Wire.Pl_Cdr1_Framing := (others => <>);

begin
   --  A bool then an aligned u32, little-endian: 1, three pad octets,
   --  then 7 00 00 00. XTypes 1.3 7.4.1.1's alignment from the origin.
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
   begin
      Zd_Wire.Write_Bool (W, True);
      Zd_Wire.Write_U32 (W, 7);
      Check
        ("bool + aligned u32",
         Same (Zd_Wire.Finish (W), (1, 0, 0, 0, 7, 0, 0, 0)));
   end;

   --  A string: four-byte length including the NUL, the bytes, the NUL.
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Big, 8, Framing, False);
   begin
      Zd_Wire.Write_String (W, Zd_Wire.To_Text ("ok"));
      Check
        ("string, big-endian length",
         Same (Zd_Wire.Finish (W), (0, 0, 0, 3, 111, 107, 0)));
   end;

   --  An enumerator under @bit_bound(16): the low sixteen bits of the
   --  pattern, aligned to two.
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
   begin
      Zd_Wire.Write_Enum (W, 16, 65535);
      Check ("enum bit_bound 16", Same (Zd_Wire.Finish (W), (255, 255)));
   end;

   --  A short parameter header: id 2, a four-byte value, the sentinel
   --  carrying the clause's must-understand flag.
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
      H : Zd_Wire.Pid_Write;
   begin
      H := Zd_Wire.Write_Pid (W, 2, Zd_Wire.Role_Member, False);
      Zd_Wire.Write_U32 (W, 9);
      Zd_Wire.Finish_Pid (W, H);
      Zd_Wire.Write_Sentinel (W);
      Check
        ("pl_cdr1 short header + sentinel",
         Same
           (Zd_Wire.Finish (W),
            (16#02#, 16#00#, 16#04#, 16#00#,
             16#09#, 16#00#, 16#00#, 16#00#,
             16#02#, 16#7F#, 16#00#, 16#00#)));
   end;

   --  The reader walks the same octets back.
   declare
      Data : constant Zd_Wire.Byte_Array_Access :=
        new Zd_Wire.Byte_Array'(1, 0, 0, 0, 7, 0, 0, 0);
      R : Zd_Wire.Reader := Zd_Wire.New_Reader (Data, Zd_Wire.Little, 8, Framing);
      Flag : Boolean;
      Value : Interfaces.Unsigned_32;
   begin
      Flag := Zd_Wire.Read_Bool (R);
      Value := Zd_Wire.Read_U32 (R);
      Check ("read bool", Flag);
      Check ("read aligned u32", Value = 7);
      Check ("status stays ok", Zd_Wire.Reader_Status (R) = Zd_Wire.Status_Ok);
      Check ("nothing remains", Zd_Wire.Reader_Remaining (R) = 0);
   end;

   --  The enumeration holder is signed: 0xC8 under eight bits reads
   --  back as -56. 7.4.3.5.3 rule (5).
   declare
      Data : constant Zd_Wire.Byte_Array_Access :=
        new Zd_Wire.Byte_Array'(0 => 16#C8#);
      R : Zd_Wire.Reader := Zd_Wire.New_Reader (Data, Zd_Wire.Little, 8, Framing);
   begin
      Check ("enum holder is signed", Zd_Wire.Read_Enum (R, 8) = -56);
   end;

   --  SequenceElementDelimiter::BeforeEachNonFinalElement: a sequence
   --  element of a non-@final type carries its own octet length in
   --  DHEADER form. Expected octets from
   --  tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
   --  (06 00 00 00 01 00 00 00 02 00, the length of the first `many` Cell
   --  and the Cell itself).
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
      H : Zd_Wire.Dheader;
   begin
      H := Zd_Wire.Write_Dheader (W);
      Zd_Wire.Write_I32 (W, 1);
      Zd_Wire.Write_I16 (W, 2);
      Zd_Wire.Finish_Dheader (W, H);
      Check
        ("a sequence element's own length in DHEADER form",
         Same
           (Zd_Wire.Finish (W),
            (16#06#, 16#00#, 16#00#, 16#00#,
             16#01#, 16#00#, 16#00#, 16#00#,
             16#02#, 16#00#)));
   end;

   --  WstringCodeUnits::Utf8: the characters' own octets, with the
   --  length an octet count rather than 7.4.1.1.2's "twice the number
   --  of characters", and with C2 still reached. Expected octets from
   --  tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
   --  (04 00 00 00 61 62 63 00 for "abc") and
   --  tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin
   --  (0d 00 00 00 then the twelve octets of a multi-byte text and a
   --  NUL).
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
   begin
      Zd_Wire.Write_Wstring_Utf8_Octets
        (W, Zd_Wire.To_Text ("abc"), Zd_Wire.Wstr_Octets_Terminated_2);
      Check
        ("a UTF-8 wstring is its octets and a NUL",
         Same (Zd_Wire.Finish (W), (16#04#, 0, 0, 0, 16#61#, 16#62#, 16#63#, 0)));
   end;
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
   begin
      Zd_Wire.Write_Wstring_Utf8_Octets
        (W,
         Zd_Wire.From_Utf8
           ((16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
             16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#)),
         Zd_Wire.Wstr_Octets_Terminated_2);
      Check
        ("a multi-byte UTF-8 wstring counts its octets",
         Same
           (Zd_Wire.Finish (W),
            (16#0D#, 0, 0, 0,
             16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
             16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#, 0)));
   end;

   --  A bounded UTF-8 wstring: the bound counts UTF-16 code units while
   --  the wire carries the octets. Expected octets from
   --  tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin
   --  (07 00 00 00 e1 88 b4 c3 a4 5a 00 for wstring<16> "ሴäZ").
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
   begin
      Zd_Wire.Write_Wstring_Utf8_Octets_Bounded
        (W,
         Zd_Wire.From_Utf8 ((16#E1#, 16#88#, 16#B4#, 16#C3#, 16#A4#, 16#5A#)),
         3, Zd_Wire.Wstr_Octets_Terminated_2);
      Check
        ("a bounded UTF-8 wstring holds three units",
         Same
           (Zd_Wire.Finish (W),
            (16#07#, 0, 0, 0,
             16#E1#, 16#88#, 16#B4#, 16#C3#, 16#A4#, 16#5A#, 0)));
   end;

   --  The read side of the UTF-8 axis.
   declare
      Data : constant Zd_Wire.Byte_Array_Access :=
        new Zd_Wire.Byte_Array'
          (16#0D#, 0, 0, 0,
           16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
           16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#, 0);
      R : Zd_Wire.Reader := Zd_Wire.New_Reader (Data, Zd_Wire.Little, 8, Framing);
      Value : Zd_Wire.Text;
   begin
      Value := Zd_Wire.Read_Wstring_Utf8_Octets (R, Zd_Wire.Wstr_Octets_Terminated_2);
      Check
        ("a UTF-8 wstring reads back",
         Zd_Wire.Strcmp
           (Value,
            Zd_Wire.From_Utf8
              ((16#C3#, 16#A4#, 16#C3#, 16#B6#, 16#C3#, 16#BC#, 16#20#,
                16#E1#, 16#88#, 16#B4#, 16#20#, 16#5A#)))
         = 0);
      Check ("a UTF-8 wstring is well formed", Zd_Wire.Reader_Status (R) = Zd_Wire.Status_Ok);
   end;

   --  ElementPadding::Packed: each collection element and each aggregate
   --  value counts alignment from its own first octet. Expected octets
   --  from tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
   --  (01 00 00 00 02 00 03 00 00 00 04 00 77, the second Cell's long at
   --  offset six).
   declare
      W : Zd_Wire.Writer := Zd_Wire.New_Writer (Zd_Wire.Little, 8, Framing, False);
      Saved : Natural;
   begin
      Saved := Zd_Wire.Begin_Packed_Element (W);
      Zd_Wire.Write_I32 (W, 1);
      Zd_Wire.Write_I16 (W, 2);
      Zd_Wire.Finish_Packed_Element (W, Saved);
      Saved := Zd_Wire.Begin_Packed_Element (W);
      Zd_Wire.Write_I32 (W, 3);
      Zd_Wire.Write_I16 (W, 4);
      Zd_Wire.Finish_Packed_Element (W, Saved);
      Zd_Wire.Write_U8 (W, 119);
      Check
        ("a packed element starts where the last ended",
         Same
           (Zd_Wire.Finish (W),
            (16#01#, 0, 0, 0, 16#02#, 0,
             16#03#, 0, 0, 0, 16#04#, 0, 16#77#)));
   end;
   declare
      Data : constant Zd_Wire.Byte_Array_Access :=
        new Zd_Wire.Byte_Array'
          (16#01#, 0, 0, 0, 16#02#, 0,
           16#03#, 0, 0, 0, 16#04#, 0, 16#77#);
      R : Zd_Wire.Reader := Zd_Wire.New_Reader (Data, Zd_Wire.Little, 8, Framing);
      Saved : Natural;
      A1, A2 : Interfaces.Integer_32;
      B1, B2 : Interfaces.Integer_16;
   begin
      Saved := Zd_Wire.Reader_Begin_Packed_Element (R);
      A1 := Zd_Wire.Read_I32 (R);
      B1 := Zd_Wire.Read_I16 (R);
      Zd_Wire.Reader_Finish_Packed_Element (R, Saved);
      Saved := Zd_Wire.Reader_Begin_Packed_Element (R);
      A2 := Zd_Wire.Read_I32 (R);
      B2 := Zd_Wire.Read_I16 (R);
      Zd_Wire.Reader_Finish_Packed_Element (R, Saved);
      Check
        ("a packed array element reads back",
         A1 = 1 and then B1 = 2 and then A2 = 3 and then B2 = 4
         and then Zd_Wire.Read_U8 (R) = 119);
   end;

   if Failed then
      Ada.Command_Line.Set_Exit_Status (1);
   else
      Ada.Text_IO.Put_Line ("zd_wire_selftest: ok");
   end if;
end Zd_Wire_Selftest;
