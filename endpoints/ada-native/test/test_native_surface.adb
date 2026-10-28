-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The XCDR surface of the pure-Ada wire-core, measured against octets this
--  repository did not write.
--
--  Every expected string below is a byte string some *other* implementation
--  produced: Eclipse Cyclone DDS, RTI Connext, eProsima Fast DDS, omniORB or
--  JacORB, as recorded in `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`
--  (the 51 cases of `tools/byte-gate/cases/`) and in
--  `tests/interop-results/<case>/<vendor>/<encoding>.<endian>.bin`. The case
--  name and the vendors that wrote it are named at each check, so a reader can
--  go and look. A comparison against this repository's own reference column
--  would say nothing: both sides would come from the same reading of the
--  specification.
--
--  The reading direction takes the same foreign octets in and asks for the
--  sample back, so a writer and a reader that agreed with each other and not
--  with the wire cannot both pass.
--
--  Each operation is checked at the ends of its range as well as in the
--  middle. That is where the last three defects in this binding were: a
--  truncating division for negative values, a frame that overran its buffer
--  and left a header behind, and a bound that only existed while runtime
--  checks were on.

with Ada.Text_IO;         use Ada.Text_IO;
with Ada.Command_Line;    use Ada.Command_Line;
with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

procedure Test_Native_Surface is

   Failures : Natural := 0;

   Hex_Digits : constant String := "0123456789abcdef";

   function Hex (B : Byte_Array) return String is
      Out_S : String (1 .. B'Length * 2);
      K     : Natural := 1;
   begin
      for X of B loop
         Out_S (K)     := Hex_Digits (Natural (X) / 16 + 1);
         Out_S (K + 1) := Hex_Digits (Natural (X) mod 16 + 1);
         K := K + 2;
      end loop;
      return Out_S;
   end Hex;

   function From_Hex (S : String) return Byte_Array is
      Out_B : Byte_Array (0 .. S'Length / 2 - 1);
      K     : Natural := S'First;
      function Nibble (C : Character) return Byte is
      begin
         case C is
            when '0' .. '9' =>
               return Byte (Character'Pos (C) - Character'Pos ('0'));
            when 'a' .. 'f' =>
               return Byte (Character'Pos (C) - Character'Pos ('a') + 10);
            when others =>
               raise Constraint_Error;
         end case;
      end Nibble;
   begin
      for I in Out_B'Range loop
         Out_B (I) := Nibble (S (K)) * 16 + Nibble (S (K + 1));
         K := K + 2;
      end loop;
      return Out_B;
   end From_Hex;

   --  One write checked against one foreign byte string. `Source` names the
   --  case and the implementations that produced it.
   procedure Wrote (Tag : String; W : Writer; Want : String; Source : String) is
      Got : constant String := Hex (Bytes (W));
   begin
      if W.Overflow then
         Put_Line (Tag & ": the writer overran its buffer");
         Failures := Failures + 1;
      elsif Got /= Want then
         Put_Line (Tag & ": got " & Got);
         Put_Line (Tag & ": want " & Want & "  (" & Source & ")");
         Failures := Failures + 1;
      else
         Put_Line (Tag & ": ok  (" & Source & ")");
      end if;
   end Wrote;

   procedure Claim (Tag : String; Held : Boolean) is
   begin
      if Held then
         Put_Line (Tag & ": ok");
      else
         Put_Line (Tag & ": FAILED");
         Failures := Failures + 1;
      end if;
   end Claim;

   W : Writer;
   R : Reader;

begin
   -----------------------------------------------------------------------
   --  The alignment cap the encoding sets. XTypes 1.3 §7.4.1.
   -----------------------------------------------------------------------

   --  `tail_pad` -- @final struct Odd { long a; short b; } with 1 and 2. All
   --  three DDS vendors write six octets and no trailing padding, in both
   --  encodings and both byte orders.
   Init (W, Little);
   Put_U32 (W, 1);
   Put_U16 (W, 2);
   Wrote ("tail_pad le", W, "010000000200", "byte-gate tail_pad; cyclone, rti, fastdds");

   Init (W, Big);
   Put_U32 (W, 1);
   Put_U16 (W, 2);
   Wrote ("tail_pad be", W, "000000010002", "byte-gate tail_pad; cyclone, rti, fastdds");

   --  `ext_final` under XCDR1 -- the `double` aligns to eight, which is the
   --  only thing that separates the XCDR1 frame from the XCDR2 one here.
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

   -----------------------------------------------------------------------
   --  DHEADER. XTypes 1.3 §7.4.3.5.
   -----------------------------------------------------------------------

   declare
      Frame : Natural;
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

      --  The DHEADER counts the body and not itself, so an empty body is
      --  zero and not four.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      DHeader_End (W, Frame);
      Wrote ("empty dheader", W, "00000000", "XTypes 1.3 §7.4.3.5");
   end;

   -----------------------------------------------------------------------
   --  EMHEADER in every length code. §7.4.3.4.2.
   -----------------------------------------------------------------------

   declare
      Frame, Member : Natural;
   begin
      --  `ext_mutable` -- @mutable struct Ext { long a; double b; short c; }.
      --  LC2, LC3 and LC1: the length is in the code and no NEXTINT is
      --  written for any of the three.
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

      --  `mut_mustunderstand` -- the flag is bit 31 of the EMHEADER, so the
      --  header of member 1 reads 0xA0000001 and not 0x20000001.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 1, True, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      EMHeader_Begin (W, 2, False, LC2, Member);
      Put_U32 (W, 16#FFFF_FFFE#);
      EMHeader_End (W, Member, LC2);
      DHeader_End (W, Frame);
      Wrote ("mut_mustunderstand xcdr2 le", W,
             "10000000" & "010000a0" & "07000000" & "02000020" & "feffffff",
             "byte-gate mut_mustunderstand; cyclone, rti");

      --  `lc_5_6_7` -- @mutable struct M with a string, a sequence<octet>, a
      --  sequence<long>, a sequence<double> and a sequence<short>. LC5 takes
      --  its length from the body's own leading word; LC6 counts four-octet
      --  elements and LC7 eight-octet ones; the last member is LC3, whose
      --  eight octets are in the code.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 1, False, LC5, Member);
      Put_String (W, "abc");
      EMHeader_End (W, Member, LC5);
      EMHeader_Begin (W, 2, False, LC5, Member);
      Put_Seq_U8 (W, [1, 2, 3]);
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

      --  `nested_mutable` -- @mutable Outer { @id(5) Inner nested; } around
      --  @mutable Inner { @id(5) long a; }. RTI frames the nested aggregate
      --  with LC5, whose length word is the inner DHEADER itself.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 5, False, LC5, Member);
      declare
         Inner : Natural;
         Field : Natural;
      begin
         DHeader_Begin (W, Inner);
         EMHeader_Begin (W, 5, False, LC2, Field);
         Put_U32 (W, 7);
         EMHeader_End (W, Field, LC2);
         DHeader_End (W, Inner);
      end;
      EMHeader_End (W, Member, LC5);
      DHeader_End (W, Frame);
      Wrote ("nested_mutable xcdr2 le", W,
             "10000000" & "05000050" & "08000000" & "05000020" & "07000000",
             "byte-gate nested_mutable; rti");

      --  `mut_string` -- LC5 over a string, then LC2 over a long.
      Init (W, Little);
      DHeader_Begin (W, Frame);
      EMHeader_Begin (W, 1, False, LC5, Member);
      Put_String (W, "abc");
      EMHeader_End (W, Member, LC5);
      EMHeader_Begin (W, 2, False, LC2, Member);
      Put_U32 (W, 7);
      EMHeader_End (W, Member, LC2);
      DHeader_End (W, Frame);
      Wrote ("mut_string xcdr2 le", W,
             "14000000" & "01000050" & "04000000" & "61626300" &
             "02000020" & "07000000",
             "byte-gate mut_string; cyclone, rti, fastdds");

      --  `mut_optional` -- the absent @optional member leaves no trace at
      --  all in a PL_CDR2 parameter list, so the DHEADER is eight and not
      --  twelve.
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

   -----------------------------------------------------------------------
   --  The PL_CDR1 parameter list. §7.4.1.2.
   -----------------------------------------------------------------------

   declare
      P : Pid_Header;
   begin
      --  `ext_mutable` under XCDR1. Three things this pins, and RTI wrote
      --  all three: the `double`'s parameter declares 8 and not 12, because
      --  its value aligns against its own block; the `short`'s declares 4
      --  and not 2, because the padding that brings the next header back to
      --  a four-octet boundary is inside the length; and the sentinel
      --  carries the must-understand flag, so it is 0x7F02.
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

      --  The same sample as Cyclone writes it: every parameter in the
      --  extended twelve-octet form, the marker carrying the flag and the
      --  member's own flag clear inside the eMemberHeader.
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

      --  `pid_3f00` -- @id(16127) and @id(16128). 0x3F00 is the last id the
      --  short header carries, and RTI and Fast DDS both put it there.
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

      --  `nested_mutable` under XCDR1 -- a parameter list inside a
      --  parameter. The inner list's alignment is measured from the
      --  outer block, and the outer parameter's declared 12 counts the
      --  inner list *and* its sentinel.
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

      --  `mut_mustunderstand` under XCDR1 -- the flag is bit 14 of the id
      --  field, so member 1 reads `0140` in a little-endian stream.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Begin (W, 1, True, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Begin (W, 2, False, P);
      Put_U32 (W, 16#FFFF_FFFE#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("mut_mustunderstand pl-cdr1 le", W,
             "01400400" & "07000000" & "02000400" & "feffffff" & "027f0000",
             "byte-gate mut_mustunderstand; rti");

      --  The extended header's own flag is on the marker whatever the
      --  member says, and the member's is bit 30 of the eMemberHeader:
      --  Cyclone writes `40000001` for the must-understand member and
      --  `00000002` for the plain one, with the same marker before both.
      Init (W, Little);
      W.Max_Align := Xcdr1_Max_Align;
      Pid_Extended_Begin (W, 1, True, P);
      Put_U32 (W, 7);
      Pid_End (W, P);
      Pid_Extended_Begin (W, 2, False, P);
      Put_U32 (W, 16#FFFF_FFFE#);
      Pid_End (W, P);
      Pid_List_End (W);
      Wrote ("mut_mustunderstand pl-cdr1 le extended", W,
             "017f0800" & "01000040" & "04000000" & "07000000" &
             "017f0800" & "02000000" & "04000000" & "feffffff" &
             "027f0000",
             "byte-gate mut_mustunderstand; cyclone");
   end;

   -----------------------------------------------------------------------
   --  The wide character and the wide string.
   -----------------------------------------------------------------------

   --  `wchar_member` -- @final struct WCh { wchar c; octet tail; } with
   --  U+0061 and 0xAB. Two octets in the stream's own order.
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

   --  `wstring_member` -- the length counts octets, so two code units read
   --  4 and not 2, and there is no terminator.
   Init (W, Little);
   Put_Wstring (W, [16#61#, 16#62#]);
   Put_U8 (W, 16#AB#);
   Wrote ("wstring_member le", W, "0400000061006200ab",
          "byte-gate wstring_member; cyclone, rti");

   Init (W, Big);
   Put_Wstring (W, [16#61#, 16#62#]);
   Put_U8 (W, 16#AB#);
   Wrote ("wstring_member be", W, "0000000400610062ab",
          "byte-gate wstring_member; rti");

   --  `wstring_nonbmp` -- U+1D11E is the surrogate pair D834 DD1E, which is
   --  two units and therefore four octets.
   Init (W, Little);
   Put_Wstring (W, [16#D834#, 16#DD1E#]);
   Put_U8 (W, 16#AB#);
   Wrote ("wstring_nonbmp le", W, "0400000034d81eddab",
          "byte-gate wstring_nonbmp; cyclone, rti");

   Init (W, Little);
   Put_Wstring (W, [1 .. 0 => 0]);
   Wrote ("empty wstring", W, "00000000", "XTypes 1.3 §7.4.1.1.1");

   -----------------------------------------------------------------------
   --  The bitset anchor. XTypes 1.3 §7.3.1.4.
   -----------------------------------------------------------------------

   --  `bitset-basic` -- bitfield<3> low = 5 and bitfield<5> high = 17 into a
   --  one-octet anchor: 5 or 17*8 = 0x8D.
   Init (W, Little);
   Put_Bitset (W, 5 + 17 * 8, 1);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-basic", W, "8dab",
          "interop-results bitset-basic; fastdds");

   --  `bitset-typed-fields` -- 3 + 1 + 12 bits into a two-octet anchor, so
   --  the anchor's own byte order is the stream's.
   Init (W, Little);
   Put_Bitset (W, 5 + 1 * 8 + 4001 * 16, 2);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-typed-fields le", W, "1dfaab",
          "interop-results bitset-typed-fields; fastdds");

   Init (W, Big);
   Put_Bitset (W, 5 + 1 * 8 + 4001 * 16, 2);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-typed-fields be", W, "fa1dab",
          "interop-results bitset-typed-fields; fastdds");

   --  `bitset-multiword` -- 20 + 20 + 24 bits into one eight-octet anchor.
   Init (W, Little);
   W.Max_Align := Xcdr1_Max_Align;
   Put_Bitset (W, 16#BC61_4E00_000F_FFFF#, 8);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-multiword le", W, "ffff0f00004e61bcab",
          "interop-results bitset-multiword; fastdds");

   Init (W, Big);
   W.Max_Align := Xcdr1_Max_Align;
   Put_Bitset (W, 16#BC61_4E00_000F_FFFF#, 8);
   Put_U8 (W, 16#AB#);
   Wrote ("bitset-multiword be", W, "bc614e00000fffffab",
          "interop-results bitset-multiword; fastdds");

   -----------------------------------------------------------------------
   --  The packed decimal. CDR §9.3.2.8.
   -----------------------------------------------------------------------

   --  `fixed-basic` -- two fixed<9,3>. omniORB writes both in five octets;
   --  JacORB trims the leading zeros of the second one and is the one
   --  implementation of the five that does.
   Init (W, Little);
   Put_Fixed (W, "123456.789", 9, 3);
   Put_Fixed (W, "-000000.001", 9, 3);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-basic", W, "123456789c000000001dab",
          "interop-results fixed-basic; omniorb");

   --  `fixed-scales` -- fixed<1,0>, fixed<5,2> and fixed<31,10> in one
   --  sample. omniORB and JacORB agree octet for octet, in both byte
   --  orders, which is the whole point: no multi-octet integer is written.
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

   --  `fixed-scale-equals-digits` -- fixed<4,4>, all fraction. P + 1 is odd,
   --  so a zero nibble is prepended and the value is three octets.
   Init (W, Little);
   Put_Fixed (W, "-0.1234", 4, 4);
   Put_U8 (W, 16#AB#);
   Wrote ("fixed-scale-equals-digits", W, "01234dab",
          "interop-results fixed-scale-equals-digits; omniorb, jacorb");

   -----------------------------------------------------------------------
   --  The ends of every range, and the refusals.
   -----------------------------------------------------------------------

   --  `char`: U+00FF is the last one that fits. U+0161's low octet is 0x61,
   --  which is `a` -- a truncating writer would produce a frame of the right
   --  length holding a plausible, wrong character.
   Init (W, Little);
   Put_Char (W, 16#FF#);
   Wrote ("char at U+00FF", W, "ff", "XTypes 1.3 §7.4.1.1.1 Table 30");
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
   if Bytes (W)'Length /= 0 then
      Claim ("char above U+00FF wrote nothing", False);
   end if;

   --  `wchar`: U+FFFF is the last one that fits and U+10000 the first that
   --  does not. The refusal is the code point's, not the width's.
   Init (W, Little);
   Put_Wchar (W, 16#FFFF#);
   Wrote ("wchar at U+FFFF", W, "ffff", "XTypes 1.3 §7.4.1.1.1 Table 30");
   declare
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Put_Wchar (W, 16#1_0000#);
   exception
      when Code_Point_Out_Of_Range =>
         Refused := True;
         Claim ("wchar above the BMP is refused", Refused);
   end;

   --  The member id: 2**28 - 1 fits the EMHEADER and 2**28 does not.
   declare
      Member  : Natural;
      Refused : Boolean := False;
   begin
      Init (W, Little);
      EMHeader_Begin (W, 16#0FFF_FFFF#, False, LC2, Member);
      Put_U32 (W, 0);
      EMHeader_End (W, Member, LC2);
      Wrote ("member id 2**28 - 1", W, "ffffff2f00000000",
             "XTypes 1.3 §7.4.3.4.2");
      begin
         Init (W, Little);
         EMHeader_Begin (W, 16#1000_0000#, False, LC2, Member);
      exception
         when Member_Id_Out_Of_Range =>
            Refused := True;
      end;
      Claim ("a 29-bit member id is refused", Refused);
   end;

   --  The short parameter id: 0x3F00 is inside the range and 0x3F01 is
   --  PID_EXTENDED itself.
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

   --  A length code that disagrees with the body it framed.
   declare
      Member  : Natural;
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

   --  A bitset anchor that is not a holder width.
   declare
      Refused : Boolean := False;
   begin
      Init (W, Little);
      Put_Bitset (W, 0, 3);
   exception
      when Anchor_Not_A_Holder_Width =>
         Refused := True;
         Claim ("a three-octet bitset anchor is refused", Refused);
   end;

   --  `fixed` at the ends of its range.
   declare
      type Case_Kind is (Ok, Refused);
      procedure Fixed_Case
        (Tag         : String;
         Value       : String;
         Digit_Count : Positive;
         Scale       : Natural;
         Want        : String;
         Kind        : Case_Kind)
      is
         Saw : Case_Kind := Ok;
      begin
         Init (W, Little);
         begin
            Put_Fixed (W, Value, Digit_Count, Scale);
         exception
            when Not_A_Fixed_Value =>
               Saw := Refused;
         end;
         if Kind = Refused then
            Claim (Tag, Saw = Refused);
         elsif Saw = Refused then
            Claim (Tag & " (unexpectedly refused)", False);
         else
            Wrote (Tag, W, Want, "CDR §9.3.2.8");
            --  The octet count is ceil((P + 1) / 2) whatever the value.
            Claim (Tag & " octet count",
                   Bytes (W)'Length = (Digit_Count + 2) / 2);
         end if;
      end Fixed_Case;
   begin
      Fixed_Case ("fixed 1 of fixed<1,0>", "1", 1, 0, "1c", Ok);
      Fixed_Case ("fixed 9 of fixed<1,0>", "9", 1, 0, "9c", Ok);
      Fixed_Case ("fixed -9 of fixed<1,0>", "-9", 1, 0, "9d", Ok);
      --  A negative zero goes out positive: there is one zero to write.
      Fixed_Case ("fixed -0 of fixed<1,0>", "-0", 1, 0, "0c", Ok);
      Fixed_Case ("fixed -0.000 of fixed<3,3>", "-0.000", 3, 3, "000c", Ok);
      --  P + 1 odd -> a prepended zero nibble.
      Fixed_Case ("fixed 12 of fixed<2,0>", "12", 2, 0, "012c", Ok);
      Fixed_Case ("fixed 0.123 of fixed<3,3>", "0.123", 3, 3, "123c", Ok);
      Fixed_Case ("fixed -0.001 of fixed<3,3>", "-0.001", 3, 3, "001d", Ok);
      --  Padding on both sides at once.
      Fixed_Case ("fixed 7.5 of fixed<9,3>", "7.5", 9, 3, "000007500c", Ok);
      --  The widest an IDL fixed goes.
      Fixed_Case ("fixed at 31 digits",
                  "1234567890123456789012345678901", 31, 0,
                  "1234567890123456789012345678901c", Ok);
      Fixed_Case ("fixed at 31 digits negative",
                  "-1234567890123456789012345678901", 31, 0,
                  "1234567890123456789012345678901d", Ok);
      --  One integer digit too many, one fraction digit too many, a scale
      --  wider than the type, and two things that are not decimals.
      Fixed_Case ("fixed 1234567 of fixed<6,0>", "1234567", 6, 0, "", Refused);
      Fixed_Case ("fixed 1.2345 of fixed<6,3>", "1.2345", 6, 3, "", Refused);
      Fixed_Case ("fixed 1.0 of fixed<2,3>", "1.0", 2, 3, "", Refused);
      Fixed_Case ("fixed twelve", "twelve", 4, 0, "", Refused);
      Fixed_Case ("fixed empty", "", 4, 0, "", Refused);
      Fixed_Case ("fixed 1.2.3", "1.2.3", 4, 2, "", Refused);
      Fixed_Case ("fixed at 32 digits", "1", 32, 0, "", Refused);
   end;

   --  A frame the bounded buffer cannot hold sets the sticky flag, and the
   --  flag is the caller's answer -- `Wrote` fails any case that sets it.
   declare
      Big_One : constant Byte_Array (0 .. Max_Buffer) := [others => 16#5A#];
   begin
      Init (W, Little);
      Put_Bytes (W, Big_One);
      Claim ("a frame past Max_Buffer sets Overflow", W.Overflow);
      Claim ("and writes nothing", Bytes (W)'Length = 0);
   end;

   -----------------------------------------------------------------------
   --  The reading direction, over the same foreign octets.
   -----------------------------------------------------------------------

   --  `ext_mutable` under XCDR2, as Cyclone, RTI and Fast DDS all wrote it.
   declare
      Body_Len : Unsigned_32;
      Id       : Unsigned_32;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : Unsigned_32;
      A        : Unsigned_32;
      B        : IEEE_Float_64;
      C        : Unsigned_16;
      Ok       : Boolean := True;
   begin
      Init (R, From_Hex ("1a000000" & "00000020" & "07000000" &
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

   --  `lc_5_6_7` as Fast DDS wrote it: the extents LC5, LC6 and LC7 announce
   --  are 8, 8 and 12, and none of the three consumed a NEXTINT.
   declare
      Body_Len : Unsigned_32;
      Id       : Unsigned_32;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : Unsigned_32;
      Ok       : Boolean := True;
      Frame    : constant Byte_Array := From_Hex
        ("40000000" &
         "01000050" & "04000000" & "61626300" &
         "02000050" & "03000000" & "010203" & "00" &
         "03000060" & "01000000" & "07000000" &
         "04000070" & "01000000" & "000000000000f83f" &
         "05000030" & "02000000" & "04000500");
      Want_Id  : constant array (1 .. 5) of Unsigned_32 := [1, 2, 3, 4, 5];
      Want_LC  : constant array (1 .. 5) of Length_Code :=
        [LC5, LC5, LC6, LC7, LC3];
      Want_Ext : constant array (1 .. 5) of Unsigned_32 := [8, 7, 8, 12, 8];
   begin
      Init (R, Frame, Little);
      DHeader_Read (R, Body_Len);
      Ok := Ok and then Body_Len = 64;
      for I in 1 .. 5 loop
         EMHeader_Read (R, Id, MU, Code, Extent);
         Ok := Ok and then Id = Want_Id (I) and then Code = Want_LC (I)
           and then Extent = Want_Ext (I);
         --  The extent is measured from where the header left the cursor,
         --  and the next header is four-aligned after it.
         R.Pos := R.Pos + Natural (Extent);
         R.Pos := R.Pos + Padding_For (R.Pos, 4);
      end loop;
      Ok := Ok and then not R.Underflow and then R.Pos = R.Len;
      Claim ("read back lc_5_6_7 xcdr2 le (fastdds)", Ok);
   end;

   --  `ext_mutable` under XCDR1 as RTI wrote it, through the parameter
   --  reader: the ids, the flags, the declared lengths and the sentinel.
   declare
      Id       : Unsigned_32;
      MU       : Boolean;
      Length   : Unsigned_32;
      Last_One : Boolean;
      Body_At  : Natural;
      A        : Unsigned_32;
      B        : IEEE_Float_64;
      C        : Unsigned_16;
      Ok       : Boolean := True;
   begin
      Init (R, From_Hex ("00000400" & "07000000" &
                         "01000800" & "000000000000f83f" &
                         "02000400" & "fdff0000" & "027f0000"), Little);
      R.Max_Align := Xcdr1_Max_Align;

      Pid_Read (R, Id, MU, Length, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 0 and then not MU and then Length = 4
        and then not Last_One;
      Get_U32 (R, A);
      Pid_Skip (R, Body_At, Length);

      Pid_Read (R, Id, MU, Length, Last_One);
      Body_At := R.Pos;
      --  The value aligns against its own block, so the `double` starts
      --  where the header left the cursor and the length is 8.
      Ok := Ok and then Id = 1 and then Length = 8;
      Get_F64 (R, B);
      Pid_Skip (R, Body_At, Length);

      Pid_Read (R, Id, MU, Length, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 2 and then Length = 4;
      Get_U16 (R, C);
      Pid_Skip (R, Body_At, Length);

      Pid_Read (R, Id, MU, Length, Last_One);
      Ok := Ok and then Last_One;
      Ok := Ok and then A = 7 and then B = 1.5 and then C = 16#FFFD#
        and then not R.Underflow and then R.Pos = R.Len;
      Claim ("read back ext_mutable pl-cdr1 le (rti)", Ok);
   end;

   --  The same sample in Cyclone's extended form: the must-understand
   --  answer comes from the eMemberHeader and not from the marker, so every
   --  member reads False here.
   declare
      Id       : Unsigned_32;
      MU       : Boolean;
      Length   : Unsigned_32;
      Last_One : Boolean;
      Body_At  : Natural;
      Ok       : Boolean := True;
   begin
      Init (R, From_Hex
              ("017f0800" & "00000000" & "04000000" & "07000000" &
               "017f0800" & "01000000" & "08000000" & "000000000000f83f" &
               "017f0800" & "02000000" & "04000000" & "fdff0000" &
               "027f0000"), Little);
      R.Max_Align := Xcdr1_Max_Align;
      for Want_Id in Unsigned_32 range 0 .. 2 loop
         Pid_Read (R, Id, MU, Length, Last_One);
         Body_At := R.Pos;
         Ok := Ok and then Id = Want_Id and then not MU
           and then not Last_One;
         Pid_Skip (R, Body_At, Length);
      end loop;
      Pid_Read (R, Id, MU, Length, Last_One);
      Ok := Ok and then Last_One and then R.Pos = R.Len;
      Claim ("read back ext_mutable pl-cdr1 le extended (cyclone)", Ok);
   end;

   --  Cyclone's must-understand member, where the marker's flag is set and
   --  the member's is not: reading the flag off the marker makes every
   --  parameter must-understand.
   declare
      Id       : Unsigned_32;
      MU       : Boolean;
      Length   : Unsigned_32;
      Last_One : Boolean;
      Body_At  : Natural;
      Ok       : Boolean := True;
   begin
      Init (R, From_Hex ("017f0800" & "01000040" & "04000000" & "07000000" &
                         "017f0800" & "02000000" & "04000000" & "feffffff" &
                         "027f0000"), Little);
      R.Max_Align := Xcdr1_Max_Align;
      Pid_Read (R, Id, MU, Length, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 1 and then MU;
      Pid_Skip (R, Body_At, Length);
      Pid_Read (R, Id, MU, Length, Last_One);
      Body_At := R.Pos;
      Ok := Ok and then Id = 2 and then not MU;
      Pid_Skip (R, Body_At, Length);
      Pid_Read (R, Id, MU, Length, Last_One);
      Ok := Ok and then Last_One;
      Claim ("read back mut_mustunderstand pl-cdr1 extended (cyclone)", Ok);
   end;

   --  The wide string, the bitset anchor and the packed decimal, each from
   --  the octets a vendor wrote.
   declare
      Units : Code_Unit_Array (0 .. 7);
      N     : Natural;
      Tail  : Byte;
      Ok    : Boolean := True;
   begin
      Init (R, From_Hex ("0400000034d81eddab"), Little);
      Get_Wstring (R, Units, N);
      Get_U8 (R, Tail);
      Ok := N = 2 and then Units (0) = 16#D834# and then Units (1) = 16#DD1E#
        and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back wstring_nonbmp le (cyclone, rti)", Ok);
   end;

   declare
      Bits : Unsigned_64;
      Tail : Byte;
      Ok   : Boolean;
   begin
      Init (R, From_Hex ("ffff0f00004e61bcab"), Little);
      R.Max_Align := Xcdr1_Max_Align;
      Get_Bitset (R, 8, Bits);
      Get_U8 (R, Tail);
      Ok := Bits = 16#BC61_4E00_000F_FFFF# and then Tail = 16#AB#
        and then R.Pos = R.Len;
      Claim ("read back bitset-multiword le (fastdds)", Ok);
   end;

   declare
      Value : String (1 .. 40);
      Last  : Natural;
      Tail  : Byte;
      Ok    : Boolean := True;
   begin
      Init (R, From_Hex ("7c" & "12345d" &
                         "0123456789012345678901234567890d" & "ab"), Little);
      Get_Fixed (R, 1, 0, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "7";
      Get_Fixed (R, 5, 2, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "-123.45";
      --  The leading zero is on the wire: the writer padded the integer
      --  part to the 21 digits `fixed<31,10>` declares, and nothing in the
      --  octets says the value was written with 20. So the decimal that
      --  comes back is the padded one, and a reader that trimmed it would
      --  be guessing.
      Get_Fixed (R, 31, 10, Value, Last);
      Ok := Ok and then
        Value (1 .. Last) = "-012345678901234567890.1234567890";
      Get_U8 (R, Tail);
      Ok := Ok and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back fixed-scales (omniorb, jacorb)", Ok);
   end;

   --  Every write in one frame, back out again, in both encodings and both
   --  byte orders: a reader that aligned differently from the writer would
   --  land on the wrong octet here and nowhere else.
   for Cap in Positive range 1 .. 2 loop
      for Endian in Endianness loop
         declare
            Align_Cap : constant Positive :=
              (if Cap = 1 then Xcdr1_Max_Align else Xcdr2_Max_Align);
            Ok    : Boolean;
            B8    : Byte;
            Flag  : Boolean;
            Code  : Unsigned_32;
            B16   : Unsigned_16;
            B32   : Unsigned_32;
            B64   : Unsigned_64;
            F32   : IEEE_Float_32;
            F64   : IEEE_Float_64;
            Text  : String (1 .. 16);
            Last  : Natural;
            Raw   : Byte_Array (0 .. 3);
            N     : Natural;
            Units : Code_Unit_Array (0 .. 3);
            Bits  : Unsigned_64;
            Dec   : String (1 .. 16);
         begin
            Init (W, Endian);
            W.Max_Align := Align_Cap;
            Put_U8 (W, 16#FF#);
            Put_Bool (W, True);
            Put_Char (W, 16#FF#);
            Put_U16 (W, 16#FFFF#);
            Put_U32 (W, 16#FFFF_FFFF#);
            Put_U64 (W, 16#FFFF_FFFF_FFFF_FFFF#);
            Put_F32 (W, -3.5);
            Put_F64 (W, 1.5);
            Put_Wchar (W, 16#FFFF#);
            Put_String (W, "abc");
            Put_Seq_U8 (W, [16#DE#, 16#AD#]);
            Put_Wstring (W, [16#D834#, 16#DD1E#]);
            Put_Bitset (W, 16#BC61_4E00_000F_FFFF#, 8);
            Put_Fixed (W, "-000000.001", 9, 3);
            Ok := not W.Overflow;

            Init (R, Bytes (W), Endian);
            R.Max_Align := Align_Cap;
            Get_U8 (R, B8);
            Get_Bool (R, Flag);
            Get_Char (R, Code);
            Get_U16 (R, B16);
            Get_U32 (R, B32);
            Get_U64 (R, B64);
            Get_F32 (R, F32);
            Get_F64 (R, F64);
            Get_Wchar (R, Code);
            Get_String (R, Text, Last);
            Get_Seq_U8 (R, Raw, N);
            Get_Wstring (R, Units, N);
            Get_Bitset (R, 8, Bits);
            Get_Fixed (R, 9, 3, Dec, Last);
            Ok := Ok
              and then B8 = 16#FF# and then Flag and then Code = 16#FFFF#
              and then B16 = 16#FFFF# and then B32 = 16#FFFF_FFFF#
              and then B64 = 16#FFFF_FFFF_FFFF_FFFF#
              and then F32 = -3.5 and then F64 = 1.5
              and then Raw (0) = 16#DE# and then Raw (1) = 16#AD#
              and then Units (0) = 16#D834# and then Units (1) = 16#DD1E#
              and then Bits = 16#BC61_4E00_000F_FFFF#
              and then Dec (1 .. Last) = "-000000.001"
              and then not R.Underflow and then R.Pos = R.Len;
            Claim ("every write reads back, cap" & Positive'Image (Align_Cap)
                   & " " & Endianness'Image (Endian), Ok);
         end;
      end loop;
   end loop;

   if Failures = 0 then
      Put_Line ("test_native_surface: all checks passed");
      Set_Exit_Status (Success);
   else
      Put_Line ("test_native_surface:" & Natural'Image (Failures)
                & " check(s) failed");
      Set_Exit_Status (Failure);
   end if;
end Test_Native_Surface;
