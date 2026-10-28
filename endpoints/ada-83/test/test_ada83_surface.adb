-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The XCDR surface of the Ada 83 wire-core, measured against octets this
--  repository did not write.
--
--  Every expected string below is a byte string some *other* implementation
--  produced -- Eclipse Cyclone DDS, RTI Connext, eProsima Fast DDS, omniORB or
--  JacORB -- as recorded in
--  `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv` (the 51 cases of
--  `tools/byte-gate/cases/`) and in
--  `tests/interop-results/<case>/<vendor>/<encoding>.<endian>.bin`. The case
--  and the vendors are named at each check. A comparison against this
--  repository's own reference column would say nothing: both sides would come
--  from the same reading of the specification.
--
--  Strict Ada 83: Text_IO and not Ada.Text_IO, no Command_Line, no array
--  aggregates with square brackets, and no reading of an out parameter. A
--  failing run raises, because Ada 83 has no Set_Exit_Status and a test that
--  printed "FAILED" and exited 0 would be a check that cannot fail.

with Text_IO;
with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;

procedure Test_Ada83_Surface is

   Checks_Failed : exception;

   Failures : Integer := 0;

   Hex_Digits : constant String := "0123456789abcdef";

   --  RM83 3.9: in Ada 83 a declarative part is basic declarative items
   --  first and bodies after, so these cannot sit below the helpers.
   W : Writer;
   R : Reader;

   function Hex (W : Writer) return String is
      Out_S : String (1 .. W.Len * 2);
      K     : Integer := 1;
   begin
      for I in 0 .. W.Len - 1 loop
         Out_S (K) := Hex_Digits (Integer (W.Buf (I)) / 16 + 1);
         Out_S (K + 1) := Hex_Digits (Integer (W.Buf (I)) mod 16 + 1);
         K := K + 2;
      end loop;
      return Out_S;
   end Hex;

   function Nibble (C : Character) return Byte is
   begin
      if C >= '0' and then C <= '9' then
         return Byte (Character'Pos (C) - Character'Pos ('0'));
      elsif C >= 'a' and then C <= 'f' then
         return Byte (Character'Pos (C) - Character'Pos ('a') + 10);
      else
         raise Constraint_Error;
      end if;
   end Nibble;

   function From_Hex (S : String) return Byte_Array is
      Out_B : Byte_Array (0 .. S'Length / 2 - 1);
      K     : Integer := S'First;
   begin
      for I in Out_B'Range loop
         Out_B (I) := Nibble (S (K)) * 16 + Nibble (S (K + 1));
         K := K + 2;
      end loop;
      return Out_B;
   end From_Hex;

   --  One write against one foreign byte string. `Source` names the case and
   --  the implementations that produced it.
   procedure Wrote (Tag : String; W : Writer; Want : String; Source : String) is
   begin
      if W.Overflow then
         Text_IO.Put_Line (Tag & ": the writer overran its buffer");
         Failures := Failures + 1;
      elsif Hex (W) /= Want then
         Text_IO.Put_Line (Tag & ": got  " & Hex (W));
         Text_IO.Put_Line (Tag & ": want " & Want & "  (" & Source & ")");
         Failures := Failures + 1;
      else
         Text_IO.Put_Line (Tag & ": ok  (" & Source & ")");
      end if;
   end Wrote;

   procedure Claim (Tag : String; Held : Boolean) is
   begin
      if Held then
         Text_IO.Put_Line (Tag & ": ok");
      else
         Text_IO.Put_Line (Tag & ": FAILED");
         Failures := Failures + 1;
      end if;
   end Claim;

begin
   --------------------------------------------------------------------
   --  The alignment cap the encoding sets. XTypes 1.3 section 7.4.1.
   --------------------------------------------------------------------

   --  `tail_pad` -- @final struct Odd { long a; short b; }. Six octets and
   --  no trailing padding, in both encodings and both byte orders.
   Init (W, Little);
   Put_U32 (W, 1);
   Put_U16 (W, 2);
   Wrote ("tail_pad le", W, "010000000200",
          "byte-gate tail_pad; cyclone, rti, fastdds");

   Init (W, Big);
   Put_U32 (W, 1);
   Put_U16 (W, 2);
   Wrote ("tail_pad be", W, "000000010002",
          "byte-gate tail_pad; cyclone, rti, fastdds");

   --  `ext_final` under XCDR1 -- the `double` aligns to eight. Neither the
   --  writer nor the alignment existed in this binding before: LONG_FLOAT
   --  is RM83 3.5.7 and the cap is a field.
   Init (W, Little);
   W.Max_Align := Xcdr1_Max_Align;
   Put_U32 (W, 7);
   Put_F64 (W, 1.5);
   Put_U16 (W, 16#FFFD#);
   Wrote ("ext_final xcdr1 le", W,
          "0700000000000000000000000000f83ffdff",
          "byte-gate ext_final; cyclone, rti, fastdds");

   Init (W, Big);
   W.Max_Align := Xcdr1_Max_Align;
   Put_U32 (W, 7);
   Put_F64 (W, 1.5);
   Put_U16 (W, 16#FFFD#);
   Wrote ("ext_final xcdr1 be", W,
          "00000007000000003ff8000000000000fffd",
          "byte-gate ext_final; cyclone, rti, fastdds");

   --  A `long long` under XCDR1 takes seven octets of padding after one
   --  octet, and three under XCDR2. Passing 4 to Put_LE was the second
   --  half of the cap being hard-wired.
   Init (W, Little);
   W.Max_Align := Xcdr1_Max_Align;
   Put_U8 (W, 16#AB#);
   Put_U64 (W, 16#01020304#, 16#05060708#);
   Wrote ("u64 aligns to eight under xcdr1", W,
          "ab000000000000000807060504030201", "XTypes 1.3 section 7.4.1");

   Init (W, Little);
   Put_U8 (W, 16#AB#);
   Put_U64 (W, 16#01020304#, 16#05060708#);
   Wrote ("u64 aligns to four under xcdr2", W,
          "ab0000000807060504030201", "XTypes 1.3 section 7.4.1");

   --------------------------------------------------------------------
   --  DHEADER. Section 7.4.3.5.
   --------------------------------------------------------------------

   declare
      Frame : Integer;
   begin
      Init (W, Little);
      DHeader_Begin (W, Frame);
      Put_U32 (W, 7);
      Put_F64 (W, 1.5);
      Put_U16 (W, 16#FFFD#);
      DHeader_End (W, Frame);
      Wrote ("ext_appendable xcdr2 le", W,
             "0e00000007000000000000000000f83ffdff",
             "byte-gate ext_appendable; cyclone, rti, fastdds");

      Init (W, Big);
      DHeader_Begin (W, Frame);
      Put_U32 (W, 7);
      Put_F64 (W, 1.5);
      Put_U16 (W, 16#FFFD#);
      DHeader_End (W, Frame);
      Wrote ("ext_appendable xcdr2 be", W,
             "0000000e000000073ff8000000000000fffd",
             "byte-gate ext_appendable; cyclone, rti, fastdds");

      Init (W, Little);
      DHeader_Begin (W, Frame);
      DHeader_End (W, Frame);
      Wrote ("empty dheader", W, "00000000",
             "XTypes 1.3 section 7.4.3.5");
   end;

   --------------------------------------------------------------------
   --  EMHEADER in every length code. Section 7.4.3.4.2.
   --------------------------------------------------------------------

   declare
      Frame  : Integer;
      Member : Integer;
      Data   : Byte_Array (0 .. 2);
   begin
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 0, False, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      EMHeader_Begin (W, 1, False, LC3, Member);
      Put_F64 (W, 1.5);
      EMHeader_End (W, Member, LC3);
      EMHeader_Begin (W, 2, False, LC1, Member);
      Put_U16 (W, 16#FFFD#);
      EMHeader_End (W, Member, LC1);
      DHeader_End (W, Frame);
      Wrote ("ext_mutable xcdr2 le", W,
             "1a000000" & "00000020" & "07000000" &
             "01000030" & "000000000000f83f" & "02000010" & "fdff",
             "byte-gate ext_mutable; cyclone, rti, fastdds");

      Init (W, Big);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 0, False, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      EMHeader_Begin (W, 1, False, LC3, Member);
      Put_F64 (W, 1.5);
      EMHeader_End (W, Member, LC3);
      EMHeader_Begin (W, 2, False, LC1, Member);
      Put_U16 (W, 16#FFFD#);
      EMHeader_End (W, Member, LC1);
      DHeader_End (W, Frame);
      Wrote ("ext_mutable xcdr2 be", W,
             "0000001a" & "20000000" & "00000007" &
             "30000001" & "3ff8000000000000" & "10000002" & "fffd",
             "byte-gate ext_mutable; cyclone, rti, fastdds");

      --  The flag is bit 31, so member 1 reads 0xA0000001.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 1, True, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      EMHeader_Begin (W, 2, False, LC2, Member);
      Put_U32 (W, 16#FFFFFFFE#);
      EMHeader_End (W, Member, LC2);
      DHeader_End (W, Frame);
      Wrote ("mut_mustunderstand xcdr2 le", W,
             "10000000" & "010000a0" & "07000000" & "02000020" & "feffffff",
             "byte-gate mut_mustunderstand; cyclone, rti");

      --  `lc_5_6_7`: LC5 takes its length from the body's own leading
      --  word, LC6 counts four-octet elements and LC7 eight-octet ones.
      Data (0) := 1;
      Data (1) := 2;
      Data (2) := 3;
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 1, False, LC5, Member);
      Put_String (W, "abc");
      EMHeader_End (W, Member, LC5);
      EMHeader_Begin (W, 2, False, LC5, Member);
      Put_Seq_U8 (W, Data);
      EMHeader_End (W, Member, LC5);
      EMHeader_Begin (W, 3, False, LC6, Member);
      Put_U32 (W, 1);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC6);
      EMHeader_Begin (W, 4, False, LC7, Member);
      Put_U32 (W, 1);
      Put_F64 (W, 1.5);
      EMHeader_End (W, Member, LC7);
      EMHeader_Begin (W, 5, False, LC3, Member);
      Put_U32 (W, 2);
      Put_U16 (W, 4);
      Put_U16 (W, 5);
      EMHeader_End (W, Member, LC3);
      DHeader_End (W, Frame);
      Wrote ("lc_5_6_7 xcdr2 le", W,
             "40000000" &
             "01000050" & "04000000" & "61626300" &
             "02000050" & "03000000" & "010203" & "00" &
             "03000060" & "01000000" & "07000000" &
             "04000070" & "01000000" & "000000000000f83f" &
             "05000030" & "02000000" & "04000500",
             "byte-gate lc_5_6_7; fastdds");

      --  `mut_optional`: the absent member leaves no trace at all.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 0, False, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      DHeader_End (W, Frame);
      Wrote ("mut_optional xcdr2 le", W,
             "08000000" & "00000020" & "07000000",
             "byte-gate mut_optional; cyclone, rti");
   end;

   --------------------------------------------------------------------
   --  The PL_CDR1 parameter list. Section 7.4.1.2.
   --------------------------------------------------------------------

   declare
      P : Pid_Header;
   begin
      --  The `double`'s parameter declares 8 and not 12 (its value aligns
      --  against its own block); the `short`'s declares 4 and not 2 (the
      --  padding to the next header is inside the length); the sentinel
      --  carries the must-understand flag.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 0, False, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Begin (W, 1, False, P);
      Put_F64 (W, 1.5);
      Pid_End (W, P);
      Pid_Begin (W, 2, False, P);
      Put_U16 (W, 16#FFFD#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("ext_mutable pl-cdr1 le", W,
             "00000400" & "07000000" &
             "01000800" & "000000000000f83f" &
             "02000400" & "fdff0000" & "027f0000",
             "byte-gate ext_mutable; rti");

      Init (W, Big);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 0, False, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Begin (W, 1, False, P);
      Put_F64 (W, 1.5);
      Pid_End (W, P);
      Pid_Begin (W, 2, False, P);
      Put_U16 (W, 16#FFFD#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("ext_mutable pl-cdr1 be", W,
             "00000004" & "00000007" &
             "00010008" & "3ff8000000000000" &
             "00020004" & "fffd0000" & "7f020000",
             "byte-gate ext_mutable; rti");

      --  The same sample as Cyclone writes it, every parameter extended.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Extended_Begin (W, 0, False, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Extended_Begin (W, 1, False, P);
      Put_F64 (W, 1.5);
      Pid_End (W, P);
      Pid_Extended_Begin (W, 2, False, P);
      Put_U16 (W, 16#FFFD#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("ext_mutable pl-cdr1 le extended", W,
             "017f0800" & "00000000" & "04000000" & "07000000" &
             "017f0800" & "01000000" & "08000000" & "000000000000f83f" &
             "017f0800" & "02000000" & "04000000" & "fdff0000" &
             "027f0000",
             "byte-gate ext_mutable; cyclone");

      --  `nested_mutable`: a parameter list inside a parameter. The
      --  inner list aligns against the outer block, and the outer
      --  parameter's declared 12 counts the inner list and its sentinel.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 5, False, P);
      declare
         Inner : Pid_Header;
      begin
         Pid_Begin (W, 5, False, Inner);
         Put_U32 (W, 7);
         Pid_End (W, Inner);
         Pid_List_End (W);
      end;
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("nested_mutable pl-cdr1 le", W,
             "05000c00" & "05000400" & "07000000" & "027f0000" & "027f0000",
             "byte-gate nested_mutable; rti");

      --  `pid_3f00`: 0x3F00 is the last id the short header carries.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 16#3EFF#, False, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Begin (W, 16#3F00#, False, P);
      Put_U32 (W, 9);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("pid_3f00 pl-cdr1 le", W,
             "ff3e0400" & "07000000" & "003f0400" & "09000000" & "027f0000",
             "byte-gate pid_3f00; rti");

      --  The flag is bit 14 of the id field.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 1, True, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Begin (W, 2, False, P);
      Put_U32 (W, 16#FFFFFFFE#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("mut_mustunderstand pl-cdr1 le", W,
             "01400400" & "07000000" & "02000400" & "feffffff" & "027f0000",
             "byte-gate mut_mustunderstand; rti");

      --  The extended header's flag is on the marker whatever the member
      --  says; the member's own is bit 30 of the eMemberHeader.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Extended_Begin (W, 1, True, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Extended_Begin (W, 2, False, P);
      Put_U32 (W, 16#FFFFFFFE#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("mut_mustunderstand pl-cdr1 le extended", W,
             "017f0800" & "01000040" & "04000000" & "07000000" &
             "017f0800" & "02000000" & "04000000" & "feffffff" &
             "027f0000",
             "byte-gate mut_mustunderstand; cyclone");
   end;

   --------------------------------------------------------------------
   --  The wide character and the wide string.
   --------------------------------------------------------------------

   declare
      Two   : Code_Unit_Array (0 .. 1);
      None  : Code_Unit_Array (1 .. 0);
   begin
      Init (W, Little);
      Put_Wchar (W, 16#61#);
      Put_U8 (W, 16#AB#);
      Wrote ("wchar_member le", W, "6100ab",
             "byte-gate wchar_member; cyclone, fastdds (xcdr1), all three (xcdr2)");

      Init (W, Big);
      Put_Wchar (W, 16#61#);
      Put_U8 (W, 16#AB#);
      Wrote ("wchar_member be", W, "0061ab",
             "byte-gate wchar_member; cyclone, rti, fastdds");

      Two (0) := 16#61#;
      Two (1) := 16#62#;
      Init (W, Little);
      Put_Wstring (W, Two);
      Put_U8 (W, 16#AB#);
      Wrote ("wstring_member le", W, "0400000061006200ab",
             "byte-gate wstring_member; cyclone, rti");

      Init (W, Big);
      Put_Wstring (W, Two);
      Put_U8 (W, 16#AB#);
      Wrote ("wstring_member be", W, "0000000400610062ab",
             "byte-gate wstring_member; rti");

      --  U+1D11E is the surrogate pair D834 DD1E: two units, four octets.
      Two (0) := 16#D834#;
      Two (1) := 16#DD1E#;
      Init (W, Little);
      Put_Wstring (W, Two);
      Put_U8 (W, 16#AB#);
      Wrote ("wstring_nonbmp le", W, "0400000034d81eddab",
             "byte-gate wstring_nonbmp; cyclone, rti");

      Init (W, Little);
      Put_Wstring (W, None);
      Wrote ("empty wstring", W, "00000000",
             "XTypes 1.3 section 7.4.1.1.1");
   end;

   --------------------------------------------------------------------
   --  The bitset anchor. Section 7.3.1.4.
   --------------------------------------------------------------------

   Init (W, Little);
   Put_Bitset (W, 0, 5 + 17 * 8, 1);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-basic", W, "8dab",
          "interop-results bitset-basic; fastdds");

   Init (W, Little);
   Put_Bitset (W, 0, 5 + 1 * 8 + 4001 * 16, 2);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-typed-fields le", W, "1dfaab",
          "interop-results bitset-typed-fields; fastdds");

   Init (W, Big);
   Put_Bitset (W, 0, 5 + 1 * 8 + 4001 * 16, 2);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-typed-fields be", W, "fa1dab",
          "interop-results bitset-typed-fields; fastdds");

   Init (W, Little);
   W.Max_Align := Xcdr1_Max_Align;
   Put_Bitset (W, 16#BC614E00#, 16#000FFFFF#, 8);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-multiword le", W, "ffff0f00004e61bcab",
          "interop-results bitset-multiword; fastdds");

   Init (W, Big);
   W.Max_Align := Xcdr1_Max_Align;
   Put_Bitset (W, 16#BC614E00#, 16#000FFFFF#, 8);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-multiword be", W, "bc614e00000fffffab",
          "interop-results bitset-multiword; fastdds");

   --------------------------------------------------------------------
   --  The packed decimal. CDR section 9.3.2.8.
   --------------------------------------------------------------------

   Init (W, Little);
   Put_Fixed (W, "123456.789", 9, 3);
   Put_Fixed (W, "-000000.001", 9, 3);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-basic", W, "123456789c000000001dab",
          "interop-results fixed-basic; omniorb");

   Init (W, Little);
   Put_Fixed (W, "7", 1, 0);
   Put_Fixed (W, "-123.45", 5, 2);
   Put_Fixed (W, "-12345678901234567890.1234567890", 31, 10);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-scales le", W,
          "7c" & "12345d" & "0123456789012345678901234567890d" & "ab",
          "interop-results fixed-scales; omniorb, jacorb");

   Init (W, Big);
   Put_Fixed (W, "7", 1, 0);
   Put_Fixed (W, "-123.45", 5, 2);
   Put_Fixed (W, "-12345678901234567890.1234567890", 31, 10);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-scales be", W,
          "7c" & "12345d" & "0123456789012345678901234567890d" & "ab",
          "interop-results fixed-scales; omniorb, jacorb");

   Init (W, Little);
   Put_Fixed (W, "-0.1234", 4, 4);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-scale-equals-digits", W, "01234dab",
          "interop-results fixed-scale-equals-digits; omniorb, jacorb");

   --------------------------------------------------------------------
   --  The ends of every range, and the refusals.
   --------------------------------------------------------------------

   --  The negative-argument decomposition, which is where the last defect
   --  in this binding was: Ada's "/" truncates toward zero and the two's
   --  complement pattern needs the floor. The corpus sweep hands over the
   --  unsigned pattern and never reaches it; a caller writing the value a
   --  `short` holds reaches it at once.
   Init (W, Little);
   Put_U16 (W, -12345);
   Wrote ("short -12345 as a value", W, "c7cf",
          "the two's complement of -12345 is 0xCFC7");

   Init (W, Little);
   Put_U32 (W, -1234567);
   Wrote ("long -1234567 as a value", W, "7929edff",
          "the two's complement of -1234567 is 0xFFED2979");

   Init (W, Little);
   Put_U16 (W, -1);
   Wrote ("short -1 as a value", W, "ffff", "two's complement");

   Init (W, Little);
   Put_U32 (W, -1);
   Wrote ("long -1 as a value", W, "ffffffff", "two's complement");

   Init (W, Little);
   Put_U16 (W, 0);
   Wrote ("short 0", W, "0000", "two's complement");

   --  The ends of the signed ranges an IDL `short` and `long` carry.
   Init (W, Little);
   Put_U16 (W, -32768);
   Wrote ("short at its minimum", W, "0080", "IDL 4.2 short is -2**15 .. 2**15-1");

   Init (W, Little);
   Put_U16 (W, 32767);
   Wrote ("short at its maximum", W, "ff7f", "IDL 4.2 short");

   Init (W, Little);
   Put_U32 (W, -2147483648);
   Wrote ("long at its minimum", W, "00000080", "IDL 4.2 long");

   Init (W, Little);
   Put_U32 (W, 2147483647);
   Wrote ("long at its maximum", W, "ffffff7f", "IDL 4.2 long");

   Init (W, Little);
   Put_Char (W, 16#FF#);
   Wrote ("char at U+00FF", W, "ff",
          "XTypes 1.3 section 7.4.1.1.1 Table 30");

   declare
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Put_Char (W, 16#161#);
   exception
      when Code_Point_Out_Of_Range =>
         Refused := True;
         Claim ("char above U+00FF is refused", Refused);
   end;

   Init (W, Little);
   Put_Wchar (W, 16#FFFF#);
   Wrote ("wchar at U+FFFF", W, "ffff",
          "XTypes 1.3 section 7.4.1.1.1 Table 30");

   declare
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Put_Wchar (W, 16#10000#);
   exception
      when Code_Point_Out_Of_Range =>
         Refused := True;
         Claim ("wchar above the BMP is refused", Refused);
   end;

   declare
      Member  : Integer;
      Refused : Boolean := False;
   begin
      Init (W, Little);
      EMHeader_Begin (W, 16#0FFFFFFF#, False, LC2, Member);
      Put_U32 (W, 0);
      EMHeader_End (W, Member, LC2);
      Wrote ("member id 2**28 - 1", W, "ffffff2f00000000",
             "XTypes 1.3 section 7.4.3.4.2");
      begin
         Init (W, Little);
         EMHeader_Begin (W, 16#10000000#, False, LC2, Member);
      exception
         when Member_Id_Out_Of_Range =>
            Refused := True;
      end;
      Claim ("a 29-bit member id is refused", Refused);
   end;

   declare
      P       : Pid_Header;
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Pid_Begin (W, 16#3F00#, False, P);
      Put_U32 (W, 0);
      Pid_End (W, P);
      Wrote ("short parameter id 0x3F00", W, "003f040000000000",
             "byte-gate pid_3f00; rti, fastdds");
      begin
         Init (W, Little);
         Pid_Begin (W, 16#3F01#, False, P);
      exception
         when Member_Id_Out_Of_Range =>
            Refused := True;
      end;
      Claim ("a short header refuses 0x3F01", Refused);
   end;

   declare
      Member  : Integer;
      Refused : Boolean := False;
   begin
      Init (W, Little);
      EMHeader_Begin (W, 0, False, LC2, Member);
      Put_U16 (W, 1);
      begin
         EMHeader_End (W, Member, LC2);
      exception
         when Length_Code_Mismatch =>
            Refused := True;
      end;
      Claim ("LC2 refuses a body that is not four octets", Refused);

      Init (W, Little);
      EMHeader_Begin (W, 0, False, LC5, Member);
      Put_U16 (W, 1);
      Refused := False;
      begin
         EMHeader_End (W, Member, LC5);
      exception
         when Length_Code_Mismatch =>
            Refused := True;
      end;
      Claim ("LC5 refuses a body shorter than its length word", Refused);
   end;

   declare
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Put_Bitset (W, 0, 0, 3);
   exception
      when Anchor_Not_A_Holder_Width =>
         Refused := True;
         Claim ("a three-octet bitset anchor is refused", Refused);
   end;

   declare
      procedure Fixed_Ok
        (Tag : String; Value : String; D : Integer; S : Integer; Want : String)
      is
      begin
         Init (W, Little);
         Put_Fixed (W, Value, D, S);
         Wrote (Tag, W, Want, "CDR section 9.3.2.8");
         Claim (Tag & " octet count", W.Len = (D + 2) / 2);
      end Fixed_Ok;

      procedure Fixed_Refused
        (Tag : String; Value : String; D : Integer; S : Integer)
      is
         Saw : Boolean := False;
      begin
         Init (W, Little);
         begin
            Put_Fixed (W, Value, D, S);
         exception
            when Not_A_Fixed_Value =>
               Saw := True;
         end;
         Claim (Tag, Saw);
      end Fixed_Refused;
   begin
      Fixed_Ok ("fixed 1 of fixed<1,0>", "1", 1, 0, "1c");
      Fixed_Ok ("fixed 9 of fixed<1,0>", "9", 1, 0, "9c");
      Fixed_Ok ("fixed -9 of fixed<1,0>", "-9", 1, 0, "9d");
      Fixed_Ok ("fixed -0 of fixed<1,0>", "-0", 1, 0, "0c");
      Fixed_Ok ("fixed -0.000 of fixed<3,3>", "-0.000", 3, 3, "000c");
      Fixed_Ok ("fixed 12 of fixed<2,0>", "12", 2, 0, "012c");
      Fixed_Ok ("fixed 0.123 of fixed<3,3>", "0.123", 3, 3, "123c");
      Fixed_Ok ("fixed -0.001 of fixed<3,3>", "-0.001", 3, 3, "001d");
      Fixed_Ok ("fixed 7.5 of fixed<9,3>", "7.5", 9, 3, "000007500c");
      Fixed_Ok ("fixed at 31 digits",
                "1234567890123456789012345678901", 31, 0,
                "1234567890123456789012345678901c");
      Fixed_Ok ("fixed at 31 digits negative",
                "-1234567890123456789012345678901", 31, 0,
                "1234567890123456789012345678901d");
      Fixed_Refused ("fixed 1234567 of fixed<6,0>", "1234567", 6, 0);
      Fixed_Refused ("fixed 1.2345 of fixed<6,3>", "1.2345", 6, 3);
      Fixed_Refused ("fixed 1.0 of fixed<2,3>", "1.0", 2, 3);
      Fixed_Refused ("fixed twelve", "twelve", 4, 0);
      Fixed_Refused ("fixed empty", "", 4, 0);
      Fixed_Refused ("fixed 1.2.3", "1.2.3", 4, 2);
      Fixed_Refused ("fixed at 32 digits", "1", 32, 0);
   end;

   --  A frame the bounded buffer cannot hold sets the sticky flag.
   declare
      Big_One : Byte_Array (0 .. Max_Buffer);
   begin
      for I in Big_One'Range loop
         Big_One (I) := 16#5A#;
      end loop;
      Init (W, Little);
      Put_Bytes (W, Big_One);
      Claim ("a frame past Max_Buffer sets Overflow", W.Overflow);
      Claim ("and writes nothing", W.Len = 0);
   end;

   --------------------------------------------------------------------
   --  The reading direction, over the same foreign octets.
   --------------------------------------------------------------------

   declare
      Body_Len : Long_Integer;
      Id       : Long_Integer;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : Long_Integer;
      A        : Long_Integer;
      B        : Long_Float;
      C        : Long_Integer;
      Ok       : Boolean := True;
   begin
      Init_Reader (R, From_Hex ("1a000000" & "00000020" & "07000000" &
                                "01000030" & "000000000000f83f" &
                                "02000010" & "fdff"), Little);
      DHeader_Read (R, Body_Len);
      Ok := Ok and then Body_Len = 26;
      EMHeader_Read (R, Id, MU, Code, Extent);
      Ok := Ok and then Id = 0 and then not MU
        and then Code = LC2 and then Extent = 4;
      Get_U32 (R, A);
      EMHeader_Read (R, Id, MU, Code, Extent);
      Ok := Ok and then Id = 1 and then Code = LC3 and then Extent = 8;
      Get_F64 (R, B);
      EMHeader_Read (R, Id, MU, Code, Extent);
      Ok := Ok and then Id = 2 and then Code = LC1 and then Extent = 2;
      Get_U16 (R, C);
      Ok := Ok and then A = 7 and then B = 1.5 and then C = 16#FFFD#
        and then not R.Underflow and then R.Pos = R.Len;
      Claim ("read back ext_mutable xcdr2 le (cyclone, rti, fastdds)", Ok);
   end;

   declare
      Body_Len : Long_Integer;
      Id       : Long_Integer;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : Long_Integer;
      Ok       : Boolean := True;
      Want_Id  : constant array (1 .. 5) of Long_Integer := (1, 2, 3, 4, 5);
      Want_LC  : constant array (1 .. 5) of Length_Code :=
        (LC5, LC5, LC6, LC7, LC3);
      Want_Ext : constant array (1 .. 5) of Long_Integer := (8, 7, 8, 12, 8);
   begin
      Init_Reader (R, From_Hex
                     ("40000000" &
                      "01000050" & "04000000" & "61626300" &
                      "02000050" & "03000000" & "010203" & "00" &
                      "03000060" & "01000000" & "07000000" &
                      "04000070" & "01000000" & "000000000000f83f" &
                      "05000030" & "02000000" & "04000500"), Little);
      DHeader_Read (R, Body_Len);
      Ok := Ok and then Body_Len = 64;
      for I in 1 .. 5 loop
         EMHeader_Read (R, Id, MU, Code, Extent);
         Ok := Ok and then Id = Want_Id (I) and then Code = Want_LC (I)
           and then Extent = Want_Ext (I);
         R.Pos := R.Pos + Integer (Extent);
         R.Pos := R.Pos + Padding_For (R.Pos, 4);
      end loop;
      Ok := Ok and then not R.Underflow and then R.Pos = R.Len;
      Claim ("read back lc_5_6_7 xcdr2 le (fastdds)", Ok);
   end;

   declare
      Id       : Long_Integer;
      MU       : Boolean;
      Len      : Long_Integer;
      Last_One : Boolean;
      Body_At  : Integer;
      A        : Long_Integer;
      B        : Long_Float;
      C        : Long_Integer;
      Ok       : Boolean := True;
   begin
      Init_Reader (R, From_Hex ("00000400" & "07000000" &
                                "01000800" & "000000000000f83f" &
                                "02000400" & "fdff0000" & "027f0000"),
                   Little);
      R.Max_Align := Xcdr1_Max_Align;

      Pid_Read (R, Id, MU, Len, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 0 and then not MU and then Len = 4
        and then not Last_One;
      Get_U32 (R, A);
      Pid_Skip (R, Body_At, Len);

      Pid_Read (R, Id, MU, Len, Last_One);
      Body_At := R.Pos;
      --  The value aligns against its own block, so the `double` starts
      --  where the header left the cursor and the length is 8.
      Ok := Ok and then Id = 1 and then Len = 8;
      Get_F64 (R, B);
      Pid_Skip (R, Body_At, Len);

      Pid_Read (R, Id, MU, Len, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 2 and then Len = 4;
      Get_U16 (R, C);
      Pid_Skip (R, Body_At, Len);

      Pid_Read (R, Id, MU, Len, Last_One);
      Ok := Ok and then Last_One;
      Ok := Ok and then A = 7 and then B = 1.5 and then C = 16#FFFD#
        and then not R.Underflow and then R.Pos = R.Len;
      Claim ("read back ext_mutable pl-cdr1 le (rti)", Ok);
   end;

   declare
      Id       : Long_Integer;
      MU       : Boolean;
      Len      : Long_Integer;
      Last_One : Boolean;
      Body_At  : Integer;
      Ok       : Boolean := True;
   begin
      Init_Reader (R, From_Hex
                     ("017f0800" & "01000040" & "04000000" & "07000000" &
                      "017f0800" & "02000000" & "04000000" & "feffffff" &
                      "027f0000"), Little);
      R.Max_Align := Xcdr1_Max_Align;
      Pid_Read (R, Id, MU, Len, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 1 and then MU;
      Pid_Skip (R, Body_At, Len);
      Pid_Read (R, Id, MU, Len, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 2 and then not MU;
      Pid_Skip (R, Body_At, Len);
      Pid_Read (R, Id, MU, Len, Last_One);
      Ok := Ok and then Last_One;
      Claim ("read back mut_mustunderstand pl-cdr1 extended (cyclone)", Ok);
   end;

   declare
      Units : Code_Unit_Array (0 .. 7);
      N     : Integer;
      Tail  : Long_Integer;
      Ok    : Boolean;
   begin
      Init_Reader (R, From_Hex ("0400000034d81eddab"), Little);
      Get_Wstring (R, Units, N);
      Get_U8 (R, Tail);
      Ok := N = 2 and then Units (0) = 16#D834# and then Units (1) = 16#DD1E#
        and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back wstring_nonbmp le (cyclone, rti)", Ok);
   end;

   declare
      Hi, Lo : Long_Integer;
      Tail   : Long_Integer;
      Ok     : Boolean;
   begin
      Init_Reader (R, From_Hex ("ffff0f00004e61bcab"), Little);
      R.Max_Align := Xcdr1_Max_Align;
      Get_Bitset (R, 8, Hi, Lo);
      Get_U8 (R, Tail);
      Ok := Hi = 16#BC614E00# and then Lo = 16#000FFFFF#
        and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back bitset-multiword le (fastdds)", Ok);
   end;

   declare
      Value : String (1 .. 40);
      Last  : Integer;
      Tail  : Long_Integer;
      Ok    : Boolean := True;
   begin
      Init_Reader (R, From_Hex ("7c" & "12345d" &
                                "0123456789012345678901234567890d" & "ab"),
                   Little);
      Get_Fixed (R, 1, 0, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "7";
      Get_Fixed (R, 5, 2, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "-123.45";
      --  The leading zero is on the wire: the writer padded the integer
      --  part to the 21 digits fixed<31,10> declares, and nothing in the
      --  octets says the value was written with 20.
      Get_Fixed (R, 31, 10, Value, Last);
      Ok := Ok and then
        Value (1 .. Last) = "-012345678901234567890.1234567890";
      Get_U8 (R, Tail);
      Ok := Ok and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back fixed-scales (omniorb, jacorb)", Ok);
   end;

   --  Every write in one frame, back out again, under both alignment caps
   --  and both byte orders.
   for Cap in 1 .. 2 loop
      for Endian in Endianness loop
         declare
            Align_Cap : Integer := Xcdr1_Max_Align;
            Ok        : Boolean;
            B8        : Long_Integer;
            Flag      : Boolean;
            Code      : Long_Integer;
            B16       : Long_Integer;
            B32       : Long_Integer;
            Hi, Lo    : Long_Integer;
            F32       : Float;
            F64       : Long_Float;
            Text      : String (1 .. 16);
            Last      : Integer;
            Raw       : Byte_Array (0 .. 3);
            Data      : Byte_Array (0 .. 1);
            N         : Integer;
            Units     : Code_Unit_Array (0 .. 3);
            Pair      : Code_Unit_Array (0 .. 1);
            Dec       : String (1 .. 16);
         begin
            if Cap = 2 then
               Align_Cap := Xcdr2_Max_Align;
            end if;
            Data (0) := 16#DE#;
            Data (1) := 16#AD#;
            Pair (0) := 16#D834#;
            Pair (1) := 16#DD1E#;

            Init (W, Endian);
            W.Max_Align := Align_Cap;
            Put_U8 (W, 16#FF#);
            Put_Bool (W, True);
            Put_Char (W, 16#FF#);
            Put_U16 (W, 16#FFFF#);
            Put_U32 (W, 16#FFFFFFFF#);
            Put_U64 (W, 16#FFFFFFFF#, 16#FFFFFFFF#);
            Put_F32 (W, -3.5);
            Put_F64 (W, 1.5);
            Put_Wchar (W, 16#FFFF#);
            Put_String (W, "abc");
            Put_Seq_U8 (W, Data);
            Put_Wstring (W, Pair);
            Put_Bitset (W, 16#BC614E00#, 16#000FFFFF#, 8);
            Put_Fixed (W, "-000000.001", 9, 3);
            Ok := not W.Overflow;

            Init_Reader (R, W.Buf (0 .. W.Len - 1), Endian);
            R.Max_Align := Align_Cap;
            Get_U8 (R, B8);
            Get_Bool (R, Flag);
            Get_Char (R, Code);
            Get_U16 (R, B16);
            Get_U32 (R, B32);
            Get_U64 (R, Hi, Lo);
            Get_F32 (R, F32);
            Get_F64 (R, F64);
            Get_Wchar (R, Code);
            Get_String (R, Text, Last);
            Get_Seq_U8 (R, Raw, N);
            Get_Wstring (R, Units, N);
            Get_Bitset (R, 8, Hi, Lo);
            Get_Fixed (R, 9, 3, Dec, Last);
            Ok := Ok
              and then B8 = 16#FF# and then Flag and then Code = 16#FFFF#
              and then B16 = 16#FFFF# and then B32 = 16#FFFFFFFF#
              and then F32 = -3.5 and then F64 = 1.5
              and then Raw (0) = 16#DE# and then Raw (1) = 16#AD#
              and then Units (0) = 16#D834# and then Units (1) = 16#DD1E#
              and then Hi = 16#BC614E00# and then Lo = 16#000FFFFF#
              and then Dec (1 .. Last) = "-000000.001"
              and then not R.Underflow and then R.Pos = R.Len;
            Claim ("every write reads back, cap" & Integer'Image (Align_Cap)
                   & " " & Endianness'Image (Endian), Ok);
         end;
      end loop;
   end loop;

   if Failures = 0 then
      Text_IO.Put_Line ("test_ada83_surface: all checks passed");
   else
      Text_IO.Put_Line ("test_ada83_surface:" & Integer'Image (Failures)
                        & " check(s) failed");
      --  Ada 83 has no Set_Exit_Status, and a run that printed a failure
      --  and exited 0 would be a check nothing acts on.
      raise Checks_Failed;
   end if;
end Test_Ada83_Surface;
