-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The XCDR surface of Ada Stage 1, measured against octets this repository
--  did not write.
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
--  The octets come out of `endpoints/c` through `Zdw`; `Zdw_Xcdr` decides the
--  framing the C core has no operation for. Nothing here decides a wire
--  question.

with Ada.Text_IO;      use Ada.Text_IO;
with Ada.Command_Line; use Ada.Command_Line;
with Interfaces.C;     use Interfaces.C;
with Zdw;
with Zdw_Xcdr;         use Zdw_Xcdr;

procedure Test_Ada_Surface is

   type Octets is array (Natural range <>) of aliased unsigned_char;
   pragma Convention (C, Octets);

   Failures : Natural := 0;

   Hex_Digits : constant String := "0123456789abcdef";

   Buf : aliased Octets (0 .. 4095) := [others => 0];
   W   : aliased Zdw.Zdw_Writer;

   In_Buf : aliased Octets (0 .. 4095) := [others => 0];
   R      : aliased Zdw.Zdw_Reader;

   procedure Check (Code : int) is
   begin
      if Code /= Zdw.ZDW_OK then
         raise Program_Error;
      end if;
   end Check;

   function Hex return String is
      Out_S : String (1 .. Natural (W.Len) * 2);
      K     : Natural := 1;
   begin
      for I in 0 .. Natural (W.Len) - 1 loop
         Out_S (K)     := Hex_Digits (Natural (Buf (I)) / 16 + 1);
         Out_S (K + 1) := Hex_Digits (Natural (Buf (I)) mod 16 + 1);
         K := K + 2;
      end loop;
      return Out_S;
   end Hex;

   function Nibble (C : Character) return unsigned_char is
   begin
      case C is
         when '0' .. '9' =>
            return unsigned_char (Character'Pos (C) - Character'Pos ('0'));
         when 'a' .. 'f' =>
            return unsigned_char
              (Character'Pos (C) - Character'Pos ('a') + 10);
         when others =>
            raise Constraint_Error;
      end case;
   end Nibble;

   --  Loads `S` into the reader's buffer and opens a reader over it.
   procedure Feed (S : String; Endian : int; Cap : int) is
      K : Natural := S'First;
   begin
      for I in 0 .. S'Length / 2 - 1 loop
         In_Buf (I) := Nibble (S (K)) * 16 + Nibble (S (K + 1));
         K := K + 2;
      end loop;
      Zdw.Reader_Init
        (R'Access, In_Buf'Address, size_t (S'Length / 2), Endian);
      R.Max_Align := Cap;
   end Feed;

   procedure Start (Endian : int; Cap : int) is
   begin
      Zdw.Writer_Init (W'Access, Buf'Address, Buf'Length, Endian);
      W.Max_Align := Cap;
   end Start;

   procedure Wrote (Tag : String; Want : String; Source : String) is
      Got : constant String := Hex;
   begin
      if W.Error /= Zdw.ZDW_OK then
         Put_Line (Tag & ": the C writer refused");
         Failures := Failures + 1;
      elsif Got /= Want then
         Put_Line (Tag & ": got  " & Got);
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

   LE : constant int := Zdw.ZDW_LE;
   BE : constant int := Zdw.ZDW_BE;
   X1 : constant int := Zdw.ZDW_XCDR1_MAX_ALIGN;
   X2 : constant int := Zdw.ZDW_XCDR2_MAX_ALIGN;

begin
   -----------------------------------------------------------------------
   --  The alignment cap the encoding sets. XTypes 1.3 §7.4.1.
   -----------------------------------------------------------------------

   Start (LE, X2);
   Check (Zdw.Put_U32 (W'Access, 1));
   Check (Zdw.Put_U16 (W'Access, 2));
   Wrote ("tail_pad le", "010000000200",
          "byte-gate tail_pad; cyclone, rti, fastdds");

   Start (BE, X2);
   Check (Zdw.Put_U32 (W'Access, 1));
   Check (Zdw.Put_U16 (W'Access, 2));
   Wrote ("tail_pad be", "000000010002",
          "byte-gate tail_pad; cyclone, rti, fastdds");

   --  `ext_final` under XCDR1: the `double` aligns to eight. The cap was
   --  reachable before only through a literal, `Zdw` binding no constant.
   Start (LE, X1);
   Check (Zdw.Put_U32 (W'Access, 7));
   Check (Zdw.Put_F64 (W'Access, 1.5));
   Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
   Wrote ("ext_final xcdr1 le", "0700000000000000000000000000f83ffdff",
          "byte-gate ext_final; cyclone, rti, fastdds");

   Start (BE, X1);
   Check (Zdw.Put_U32 (W'Access, 7));
   Check (Zdw.Put_F64 (W'Access, 1.5));
   Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
   Wrote ("ext_final xcdr1 be", "00000007000000003ff8000000000000fffd",
          "byte-gate ext_final; cyclone, rti, fastdds");

   -----------------------------------------------------------------------
   --  DHEADER, through the C core's own helpers. §7.4.3.5.
   -----------------------------------------------------------------------

   declare
      Frame : size_t;
   begin
      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      Check (Zdw.Put_U32 (W'Access, 7));
      Check (Zdw.Put_F64 (W'Access, 1.5));
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("ext_appendable xcdr2 le",
             "0e00000007000000000000000000f83ffdff",
             "byte-gate ext_appendable; cyclone, rti, fastdds");

      Start (BE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      Check (Zdw.Put_U32 (W'Access, 7));
      Check (Zdw.Put_F64 (W'Access, 1.5));
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("ext_appendable xcdr2 be",
             "0000000e000000073ff8000000000000fffd",
             "byte-gate ext_appendable; cyclone, rti, fastdds");

      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("empty dheader", "00000000", "XTypes 1.3 §7.4.3.5");
   end;

   -----------------------------------------------------------------------
   --  EMHEADER in every length code. §7.4.3.4.2.
   -----------------------------------------------------------------------

   declare
      Frame  : size_t;
      Member : size_t;
      Data   : aliased constant Octets (0 .. 2) := [1, 2, 3];
      Abc    : aliased constant char_array := To_C ("abc");
   begin
      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      EMHeader_Begin (W'Access, 0, False, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 7));
      EMHeader_End (W'Access, Member, LC2);
      EMHeader_Begin (W'Access, 1, False, LC3, Member);
      Check (Zdw.Put_F64 (W'Access, 1.5));
      EMHeader_End (W'Access, Member, LC3);
      EMHeader_Begin (W'Access, 2, False, LC1, Member);
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      EMHeader_End (W'Access, Member, LC1);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("ext_mutable xcdr2 le",
             "1a000000" & "00000020" & "07000000" &
             "01000030" & "000000000000f83f" & "02000010" & "fdff",
             "byte-gate ext_mutable; cyclone, rti, fastdds");

      Start (BE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      EMHeader_Begin (W'Access, 0, False, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 7));
      EMHeader_End (W'Access, Member, LC2);
      EMHeader_Begin (W'Access, 1, False, LC3, Member);
      Check (Zdw.Put_F64 (W'Access, 1.5));
      EMHeader_End (W'Access, Member, LC3);
      EMHeader_Begin (W'Access, 2, False, LC1, Member);
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      EMHeader_End (W'Access, Member, LC1);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("ext_mutable xcdr2 be",
             "0000001a" & "20000000" & "00000007" &
             "30000001" & "3ff8000000000000" & "10000002" & "fffd",
             "byte-gate ext_mutable; cyclone, rti, fastdds");

      --  The flag is bit 31: member 1 reads 0xA0000001.
      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      EMHeader_Begin (W'Access, 1, True, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 7));
      EMHeader_End (W'Access, Member, LC2);
      EMHeader_Begin (W'Access, 2, False, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 16#FFFF_FFFE#));
      EMHeader_End (W'Access, Member, LC2);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("mut_mustunderstand xcdr2 le",
             "10000000" & "010000a0" & "07000000" & "02000020" & "feffffff",
             "byte-gate mut_mustunderstand; cyclone, rti");

      --  `lc_5_6_7`: LC5 takes its length from the body's own leading word,
      --  LC6 counts four-octet elements and LC7 eight-octet ones.
      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      EMHeader_Begin (W'Access, 1, False, LC5, Member);
      Check (Zdw.Put_String (W'Access, Abc'Address));
      EMHeader_End (W'Access, Member, LC5);
      EMHeader_Begin (W'Access, 2, False, LC5, Member);
      Check (Zdw.Put_Seq_U8 (W'Access, Data'Address, 3));
      EMHeader_End (W'Access, Member, LC5);
      EMHeader_Begin (W'Access, 3, False, LC6, Member);
      Check (Zdw.Put_U32 (W'Access, 1));
      Check (Zdw.Put_U32 (W'Access, 7));
      EMHeader_End (W'Access, Member, LC6);
      EMHeader_Begin (W'Access, 4, False, LC7, Member);
      Check (Zdw.Put_U32 (W'Access, 1));
      Check (Zdw.Put_F64 (W'Access, 1.5));
      EMHeader_End (W'Access, Member, LC7);
      EMHeader_Begin (W'Access, 5, False, LC3, Member);
      Check (Zdw.Put_U32 (W'Access, 2));
      Check (Zdw.Put_U16 (W'Access, 4));
      Check (Zdw.Put_U16 (W'Access, 5));
      EMHeader_End (W'Access, Member, LC3);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("lc_5_6_7 xcdr2 le",
             "40000000" &
             "01000050" & "04000000" & "61626300" &
             "02000050" & "03000000" & "010203" & "00" &
             "03000060" & "01000000" & "07000000" &
             "04000070" & "01000000" & "000000000000f83f" &
             "05000030" & "02000000" & "04000500",
             "byte-gate lc_5_6_7; fastdds");

      --  The C core's own LC4 helper and this package's LC4 must be the
      --  same octets, or the two disagree about the same length code.
      Start (LE, X2);
      Member := Zdw.EMHeader_Begin (W'Access, 3, 0);
      Check (Zdw.Put_U32 (W'Access, 7));
      Check (Zdw.EMHeader_End (W'Access, Member));
      declare
         From_C : constant String := Hex;
      begin
         Start (LE, X2);
         EMHeader_Begin (W'Access, 3, False, LC4, Member);
         Check (Zdw.Put_U32 (W'Access, 7));
         EMHeader_End (W'Access, Member, LC4);
         Claim ("LC4 here is LC4 in the C core", Hex = From_C);
         Wrote ("LC4 member header", "03000040" & "04000000" & "07000000",
                "XTypes 1.3 §7.4.3.4.2");
      end;

      --  `mut_optional`: the absent member leaves no trace at all.
      Start (LE, X2);
      Frame := Zdw.DHeader_Begin (W'Access);
      EMHeader_Begin (W'Access, 0, False, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 7));
      EMHeader_End (W'Access, Member, LC2);
      Check (Zdw.DHeader_End (W'Access, Frame));
      Wrote ("mut_optional xcdr2 le",
             "08000000" & "00000020" & "07000000",
             "byte-gate mut_optional; cyclone, rti");
   end;

   -----------------------------------------------------------------------
   --  The PL_CDR1 parameter list. §7.4.1.2.
   -----------------------------------------------------------------------

   declare
      P : Pid_Header;
   begin
      --  The `double`'s parameter declares 8 and not 12 -- its value aligns
      --  against its own block, which is what `Align_Block` is for -- the
      --  `short`'s declares 4 and not 2, and the sentinel is 0x7F02.
      Start (LE, X1);
      Pid_Begin (W'Access, 0, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 1, False, P);
      Align_Block (W'Access, P, 8);
      Check (Zdw.Put_F64 (W'Access, 1.5));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 2, False, P);
      Align_Block (W'Access, P, 2);
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("ext_mutable pl-cdr1 le",
             "00000400" & "07000000" &
             "01000800" & "000000000000f83f" &
             "02000400" & "fdff0000" & "027f0000",
             "byte-gate ext_mutable; rti");

      Start (BE, X1);
      Pid_Begin (W'Access, 0, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 1, False, P);
      Align_Block (W'Access, P, 8);
      Check (Zdw.Put_F64 (W'Access, 1.5));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 2, False, P);
      Align_Block (W'Access, P, 2);
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("ext_mutable pl-cdr1 be",
             "00000004" & "00000007" &
             "00010008" & "3ff8000000000000" &
             "00020004" & "fffd0000" & "7f020000",
             "byte-gate ext_mutable; rti");

      --  The same sample as Cyclone writes it, every parameter extended.
      Start (LE, X1);
      Pid_Extended_Begin (W'Access, 0, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Extended_Begin (W'Access, 1, False, P);
      Align_Block (W'Access, P, 8);
      Check (Zdw.Put_F64 (W'Access, 1.5));
      Pid_End (W'Access, P);
      Pid_Extended_Begin (W'Access, 2, False, P);
      Align_Block (W'Access, P, 2);
      Check (Zdw.Put_U16 (W'Access, 16#FFFD#));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("ext_mutable pl-cdr1 le extended",
             "017f0800" & "00000000" & "04000000" & "07000000" &
             "017f0800" & "01000000" & "08000000" & "000000000000f83f" &
             "017f0800" & "02000000" & "04000000" & "fdff0000" &
             "027f0000",
             "byte-gate ext_mutable; cyclone");

      --  `nested_mutable`: a parameter list inside a parameter. The
      --  inner list aligns against the outer block, and the outer
      --  parameter's declared 12 counts the inner list and its sentinel.
      Start (LE, X1);
      Pid_Begin (W'Access, 5, False, P);
      declare
         Inner : Pid_Header;
      begin
         Pid_Begin (W'Access, 5, False, Inner);
         Align_Block (W'Access, Inner, 4);
         Check (Zdw.Put_U32 (W'Access, 7));
         Pid_End (W'Access, Inner);
         Pid_List_End (W'Access);
      end;
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("nested_mutable pl-cdr1 le",
             "05000c00" & "05000400" & "07000000" & "027f0000" & "027f0000",
             "byte-gate nested_mutable; rti");

      --  `pid_3f00`: 0x3F00 is the last id the short header carries.
      Start (LE, X1);
      Pid_Begin (W'Access, 16#3EFF#, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 16#3F00#, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 9));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("pid_3f00 pl-cdr1 le",
             "ff3e0400" & "07000000" & "003f0400" & "09000000" & "027f0000",
             "byte-gate pid_3f00; rti");

      --  The flag is bit 14 of the id field.
      Start (LE, X1);
      Pid_Begin (W'Access, 1, True, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Begin (W'Access, 2, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 16#FFFF_FFFE#));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("mut_mustunderstand pl-cdr1 le",
             "01400400" & "07000000" & "02000400" & "feffffff" & "027f0000",
             "byte-gate mut_mustunderstand; rti");

      --  On the marker the flag is set whatever the member says; the
      --  member's own is bit 30 of the eMemberHeader.
      Start (LE, X1);
      Pid_Extended_Begin (W'Access, 1, True, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 7));
      Pid_End (W'Access, P);
      Pid_Extended_Begin (W'Access, 2, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 16#FFFF_FFFE#));
      Pid_End (W'Access, P);
      Pid_List_End (W'Access);
      Wrote ("mut_mustunderstand pl-cdr1 le extended",
             "017f0800" & "01000040" & "04000000" & "07000000" &
             "017f0800" & "02000000" & "04000000" & "feffffff" &
             "027f0000",
             "byte-gate mut_mustunderstand; cyclone");
   end;

   -----------------------------------------------------------------------
   --  The wide character and the wide string.
   -----------------------------------------------------------------------

   Start (LE, X2);
   Put_Wchar (W'Access, 16#61#);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("wchar_member le", "6100ab",
          "byte-gate wchar_member; cyclone, fastdds (xcdr1), all three (xcdr2)");

   Start (BE, X2);
   Put_Wchar (W'Access, 16#61#);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("wchar_member be", "0061ab",
          "byte-gate wchar_member; cyclone, rti, fastdds");

   Start (LE, X2);
   Put_Wstring (W'Access, [16#61#, 16#62#]);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("wstring_member le", "0400000061006200ab",
          "byte-gate wstring_member; cyclone, rti");

   Start (BE, X2);
   Put_Wstring (W'Access, [16#61#, 16#62#]);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("wstring_member be", "0000000400610062ab",
          "byte-gate wstring_member; rti");

   --  U+1D11E is the surrogate pair D834 DD1E: two units, four octets.
   Start (LE, X2);
   Put_Wstring (W'Access, [16#D834#, 16#DD1E#]);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("wstring_nonbmp le", "0400000034d81eddab",
          "byte-gate wstring_nonbmp; cyclone, rti");

   Start (LE, X2);
   Put_Wstring (W'Access, [1 .. 0 => 0]);
   Wrote ("empty wstring", "00000000", "XTypes 1.3 §7.4.1.1.1");

   -----------------------------------------------------------------------
   --  The bitset anchor. §7.3.1.4.
   -----------------------------------------------------------------------

   Start (LE, X2);
   Put_Bitset (W'Access, (Hi => 0, Lo => 5 + 17 * 8), 1);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("bitset-basic", "8dab", "interop-results bitset-basic; fastdds");

   Start (LE, X2);
   Put_Bitset (W'Access, (Hi => 0, Lo => 5 + 1 * 8 + 4001 * 16), 2);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("bitset-typed-fields le", "1dfaab",
          "interop-results bitset-typed-fields; fastdds");

   Start (BE, X2);
   Put_Bitset (W'Access, (Hi => 0, Lo => 5 + 1 * 8 + 4001 * 16), 2);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("bitset-typed-fields be", "fa1dab",
          "interop-results bitset-typed-fields; fastdds");

   Start (LE, X1);
   Put_Bitset (W'Access, (Hi => 16#BC61_4E00#, Lo => 16#000F_FFFF#), 8);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("bitset-multiword le", "ffff0f00004e61bcab",
          "interop-results bitset-multiword; fastdds");

   Start (BE, X1);
   Put_Bitset (W'Access, (Hi => 16#BC61_4E00#, Lo => 16#000F_FFFF#), 8);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("bitset-multiword be", "bc614e00000fffffab",
          "interop-results bitset-multiword; fastdds");

   -----------------------------------------------------------------------
   --  The packed decimal. CDR §9.3.2.8.
   -----------------------------------------------------------------------

   Start (LE, X2);
   Put_Fixed (W'Access, "123456.789", 9, 3);
   Put_Fixed (W'Access, "-000000.001", 9, 3);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("fixed-basic", "123456789c000000001dab",
          "interop-results fixed-basic; omniorb");

   Start (LE, X2);
   Put_Fixed (W'Access, "7", 1, 0);
   Put_Fixed (W'Access, "-123.45", 5, 2);
   Put_Fixed (W'Access, "-12345678901234567890.1234567890", 31, 10);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("fixed-scales le",
          "7c" & "12345d" & "0123456789012345678901234567890d" & "ab",
          "interop-results fixed-scales; omniorb, jacorb");

   Start (BE, X2);
   Put_Fixed (W'Access, "7", 1, 0);
   Put_Fixed (W'Access, "-123.45", 5, 2);
   Put_Fixed (W'Access, "-12345678901234567890.1234567890", 31, 10);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("fixed-scales be",
          "7c" & "12345d" & "0123456789012345678901234567890d" & "ab",
          "interop-results fixed-scales; omniorb, jacorb");

   Start (LE, X2);
   Put_Fixed (W'Access, "-0.1234", 4, 4);
   Check (Zdw.Put_U8 (W'Access, 16#AB#));
   Wrote ("fixed-scale-equals-digits", "01234dab",
          "interop-results fixed-scale-equals-digits; omniorb, jacorb");

   -----------------------------------------------------------------------
   --  The ends of every range, and the refusals.
   -----------------------------------------------------------------------

   Start (LE, X2);
   Put_Char (W'Access, 16#FF#);
   Wrote ("char at U+00FF", "ff", "XTypes 1.3 §7.4.1.1.1 Table 30");

   declare
      Refused : Boolean := False;
   begin
      Start (LE, X2);
      Put_Char (W'Access, 16#161#);
   exception
      when Code_Point_Out_Of_Range =>
         Refused := True;
         Claim ("char above U+00FF is refused", Refused);
   end;

   Start (LE, X2);
   Put_Wchar (W'Access, 16#FFFF#);
   Wrote ("wchar at U+FFFF", "ffff", "XTypes 1.3 §7.4.1.1.1 Table 30");

   declare
      Refused : Boolean := False;
   begin
      Start (LE, X2);
      Put_Wchar (W'Access, 16#1_0000#);
   exception
      when Code_Point_Out_Of_Range =>
         Refused := True;
         Claim ("wchar above the BMP is refused", Refused);
   end;

   declare
      Member  : size_t;
      Refused : Boolean := False;
   begin
      Start (LE, X2);
      EMHeader_Begin (W'Access, 16#0FFF_FFFF#, False, LC2, Member);
      Check (Zdw.Put_U32 (W'Access, 0));
      EMHeader_End (W'Access, Member, LC2);
      Wrote ("member id 2**28 - 1", "ffffff2f00000000",
             "XTypes 1.3 §7.4.3.4.2");
      begin
         Start (LE, X2);
         EMHeader_Begin (W'Access, 16#1000_0000#, False, LC2, Member);
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
      Start (LE, X1);
      Pid_Begin (W'Access, 16#3F00#, False, P);
      Align_Block (W'Access, P, 4);
      Check (Zdw.Put_U32 (W'Access, 0));
      Pid_End (W'Access, P);
      Wrote ("short parameter id 0x3F00", "003f040000000000",
             "byte-gate pid_3f00; rti, fastdds");
      begin
         Start (LE, X1);
         Pid_Begin (W'Access, 16#3F01#, False, P);
      exception
         when Member_Id_Out_Of_Range =>
            Refused := True;
      end;
      Claim ("a short header refuses 0x3F01", Refused);
   end;

   declare
      Member  : size_t;
      Refused : Boolean := False;
   begin
      Start (LE, X2);
      EMHeader_Begin (W'Access, 0, False, LC2, Member);
      Check (Zdw.Put_U16 (W'Access, 1));
      begin
         EMHeader_End (W'Access, Member, LC2);
      exception
         when Length_Code_Mismatch =>
            Refused := True;
      end;
      Claim ("LC2 refuses a body that is not four octets", Refused);

      Start (LE, X2);
      EMHeader_Begin (W'Access, 0, False, LC5, Member);
      Check (Zdw.Put_U16 (W'Access, 1));
      Refused := False;
      begin
         EMHeader_End (W'Access, Member, LC5);
      exception
         when Length_Code_Mismatch =>
            Refused := True;
      end;
      Claim ("LC5 refuses a body shorter than its length word", Refused);
   end;

   declare
      Refused : Boolean := False;
   begin
      Start (LE, X2);
      Put_Bitset (W'Access, (Hi => 0, Lo => 0), 3);
   exception
      when Anchor_Not_A_Holder_Width =>
         Refused := True;
         Claim ("a three-octet bitset anchor is refused", Refused);
   end;

   declare
      procedure Fixed_Ok
        (Tag : String; Value : String; D : Positive; S : Natural;
         Want : String) is
      begin
         Start (LE, X2);
         Put_Fixed (W'Access, Value, D, S);
         Wrote (Tag, Want, "CDR §9.3.2.8");
         Claim (Tag & " octet count", Natural (W.Len) = (D + 2) / 2);
      end Fixed_Ok;

      procedure Fixed_Refused
        (Tag : String; Value : String; D : Positive; S : Natural)
      is
         Saw : Boolean := False;
      begin
         Start (LE, X2);
         begin
            Put_Fixed (W'Access, Value, D, S);
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

   -----------------------------------------------------------------------
   --  The reading direction, over the same foreign octets.
   -----------------------------------------------------------------------

   declare
      Body_Len : aliased unsigned_long;
      Id       : unsigned_long;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : unsigned_long;
      A        : aliased unsigned_long;
      B        : aliased double;
      C        : aliased unsigned;
      Ok       : Boolean := True;
   begin
      Feed ("1a000000" & "00000020" & "07000000" &
            "01000030" & "000000000000f83f" &
            "02000010" & "fdff", LE, X2);
      Check (Zdw.DHeader_Read (R'Access, Body_Len'Access));
      Ok := Ok and then Body_Len = 26;
      EMHeader_Read (R'Access, Id, MU, Code, Extent);
      Ok := Ok and then Id = 0 and then not MU
        and then Code = LC2 and then Extent = 4;
      Check (Zdw.Get_U32 (R'Access, A'Access));
      EMHeader_Read (R'Access, Id, MU, Code, Extent);
      Ok := Ok and then Id = 1 and then Code = LC3 and then Extent = 8;
      Check (Zdw.Get_F64 (R'Access, B'Access));
      EMHeader_Read (R'Access, Id, MU, Code, Extent);
      Ok := Ok and then Id = 2 and then Code = LC1 and then Extent = 2;
      Check (Zdw.Get_U16 (R'Access, C'Access));
      Ok := Ok and then A = 7 and then B = 1.5 and then C = 16#FFFD#
        and then R.Pos = R.Len;
      Claim ("read back ext_mutable xcdr2 le (cyclone, rti, fastdds)", Ok);
   end;

   declare
      Body_Len : aliased unsigned_long;
      Id       : unsigned_long;
      MU       : Boolean;
      Code     : Length_Code;
      Extent   : unsigned_long;
      Ok       : Boolean := True;
      Want_Id  : constant array (1 .. 5) of unsigned_long := [1, 2, 3, 4, 5];
      Want_LC  : constant array (1 .. 5) of Length_Code :=
        [LC5, LC5, LC6, LC7, LC3];
      Want_Ext : constant array (1 .. 5) of unsigned_long := [8, 7, 8, 12, 8];
   begin
      Feed ("40000000" &
            "01000050" & "04000000" & "61626300" &
            "02000050" & "03000000" & "010203" & "00" &
            "03000060" & "01000000" & "07000000" &
            "04000070" & "01000000" & "000000000000f83f" &
            "05000030" & "02000000" & "04000500", LE, X2);
      Check (Zdw.DHeader_Read (R'Access, Body_Len'Access));
      Ok := Ok and then Body_Len = 64;
      for I in 1 .. 5 loop
         EMHeader_Read (R'Access, Id, MU, Code, Extent);
         Ok := Ok and then Id = Want_Id (I) and then Code = Want_LC (I)
           and then Extent = Want_Ext (I);
         R.Pos := R.Pos + size_t (Extent);
         R.Pos := R.Pos + Zdw.Padding_For (R.Pos, 4);
      end loop;
      Ok := Ok and then R.Pos = R.Len;
      Claim ("read back lc_5_6_7 xcdr2 le (fastdds)", Ok);
   end;

   declare
      Id       : unsigned_long;
      MU       : Boolean;
      Length   : unsigned_long;
      Last_One : Boolean;
      P        : Pid_Header;
      A        : aliased unsigned_long;
      B        : aliased double;
      C        : aliased unsigned;
      Ok       : Boolean := True;
   begin
      Feed ("00000400" & "07000000" &
            "01000800" & "000000000000f83f" &
            "02000400" & "fdff0000" & "027f0000", LE, X1);
      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Id = 0 and then not MU and then Length = 4
        and then not Last_One;
      Align_Block_In (R'Access, P, 4);
      Check (Zdw.Get_U32 (R'Access, A'Access));
      Pid_Skip (R'Access, P, Length);

      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      --  The value aligns against its own block, so the `double` starts
      --  where the header left the cursor and the length is 8.
      Ok := Ok and then Id = 1 and then Length = 8;
      Align_Block_In (R'Access, P, 8);
      Check (Zdw.Get_F64 (R'Access, B'Access));
      Pid_Skip (R'Access, P, Length);

      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Id = 2 and then Length = 4;
      Align_Block_In (R'Access, P, 2);
      Check (Zdw.Get_U16 (R'Access, C'Access));
      Pid_Skip (R'Access, P, Length);

      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Last_One;
      Ok := Ok and then A = 7 and then B = 1.5 and then C = 16#FFFD#
        and then R.Pos = R.Len;
      Claim ("read back ext_mutable pl-cdr1 le (rti)", Ok);
   end;

   declare
      Id       : unsigned_long;
      MU       : Boolean;
      Length   : unsigned_long;
      Last_One : Boolean;
      P        : Pid_Header;
      Ok       : Boolean := True;
   begin
      Feed ("017f0800" & "01000040" & "04000000" & "07000000" &
            "017f0800" & "02000000" & "04000000" & "feffffff" &
            "027f0000", LE, X1);
      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Id = 1 and then MU;
      Pid_Skip (R'Access, P, Length);
      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Id = 2 and then not MU;
      Pid_Skip (R'Access, P, Length);
      Pid_Read (R'Access, Id, MU, Length, Last_One, P);
      Ok := Ok and then Last_One;
      Claim ("read back mut_mustunderstand pl-cdr1 extended (cyclone)", Ok);
   end;

   declare
      Units : Code_Unit_Array (0 .. 7);
      N     : Natural;
      Tail  : aliased unsigned_char;
      Ok    : Boolean;
   begin
      Feed ("0400000034d81eddab", LE, X2);
      Get_Wstring (R'Access, Units, N);
      Check (Zdw.Get_U8 (R'Access, Tail'Access));
      Ok := N = 2 and then Units (0) = 16#D834# and then Units (1) = 16#DD1E#
        and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back wstring_nonbmp le (cyclone, rti)", Ok);
   end;

   declare
      Bits : Zdw.Zdw_U64;
      Tail : aliased unsigned_char;
      Ok   : Boolean;
   begin
      Feed ("ffff0f00004e61bcab", LE, X1);
      Get_Bitset (R'Access, 8, Bits);
      Check (Zdw.Get_U8 (R'Access, Tail'Access));
      Ok := Bits.Hi = 16#BC61_4E00# and then Bits.Lo = 16#000F_FFFF#
        and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back bitset-multiword le (fastdds)", Ok);
   end;

   declare
      Value : String (1 .. 40);
      Last  : Natural;
      Tail  : aliased unsigned_char;
      Ok    : Boolean := True;
   begin
      Feed ("7c" & "12345d" & "0123456789012345678901234567890d" & "ab",
            LE, X2);
      Get_Fixed (R'Access, 1, 0, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "7";
      Get_Fixed (R'Access, 5, 2, Value, Last);
      Ok := Ok and then Value (1 .. Last) = "-123.45";
      --  The leading zero is on the wire: the writer padded the integer
      --  part to the 21 digits fixed<31,10> declares.
      Get_Fixed (R'Access, 31, 10, Value, Last);
      Ok := Ok and then
        Value (1 .. Last) = "-012345678901234567890.1234567890";
      Check (Zdw.Get_U8 (R'Access, Tail'Access));
      Ok := Ok and then Tail = 16#AB# and then R.Pos = R.Len;
      Claim ("read back fixed-scales (omniorb, jacorb)", Ok);
   end;

   if Failures = 0 then
      Put_Line ("test_ada_surface: all checks passed");
      Set_Exit_Status (Success);
   else
      Put_Line ("test_ada_surface:" & Natural'Image (Failures)
                & " check(s) failed");
      Set_Exit_Status (Failure);
   end if;
end Test_Ada_Surface;
