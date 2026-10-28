-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  ZeroDDS native Ada endpoint SDK -- Stage 2 (ADR 0013): a **pure Ada** XCDR
--  wire-core, no C, no FFI. Byte-for-byte identical to the Rust core
--  (`zerodds-cdr`) and the C SDK (`endpoints/c`): serialization is by explicit
--  byte order, so the output is independent of the host endianness and a
--  big-endian target produces the same wire as an x86-64 host. The wire byte
--  order (LE/BE) is an explicit parameter honoring the XCDR encapsulation
--  byte-order flag (DDSI-RTPS 2.5 section 10.5).
--
--  XCDR2 alignment (OMG XTypes 1.3 section 7.4.1.1.1) is relative to the
--  stream start and capped at 4. Written in a restricted, contract-carrying
--  subset so it is amenable to SPARK analysis.

with Interfaces; use Interfaces;

package Zerodds_Native_Wire
  with SPARK_Mode => On
is

   type Byte is new Interfaces.Unsigned_8;
   type Byte_Array is array (Natural range <>) of Byte;

   --  The code units a `wstring` is a sequence of. XTypes 1.3 §7.4.1.1.1
   --  gives `wchar` two octets on the wire, so the unit is 16 bits and a
   --  code point outside the BMP is the surrogate pair the caller hands
   --  over -- U+1D11E is two units, and `wstring_nonbmp` measures Cyclone
   --  and RTI writing `34d8 1edd` for it.
   type Code_Unit_Array is array (Natural range <>) of Interfaces.Unsigned_16;

   type Endianness is (Little, Big);

   --  The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34.
   --  The code is bits 28..30 of the EMHEADER and decides how a reader
   --  recovers the member's byte length: LC0..LC3 from the code alone,
   --  LC4 from a NEXTINT that follows the header, LC5..LC7 from the
   --  member body's own leading word, which stays part of the body.
   --  'Pos is the code, so no table is needed to convert one.
   type Length_Code is (LC0, LC1, LC2, LC3, LC4, LC5, LC6, LC7);

   --  PL_CDR1 parameter ids. XTypes 1.3 §7.4.1.2 Table 31.
   Pid_List_End_Id  : constant := 16#3F02#;
   Pid_Extended_Id  : constant := 16#3F01#;
   --  The highest member id a short parameter header carries, inclusive.
   --  `pid_3f00` measures RTI and Fast DDS writing `003f 0400` for id
   --  16128, so 0x3F00 is inside the short range and not above it.
   Pid_Short_Max_Id : constant := 16#3F00#;

   --  A code point a one-octet `char` or a two-octet `wchar` cannot
   --  carry. Refused and never truncated: U+0161's low octet is 0x61,
   --  which is `a`, so a truncating writer produces a frame of the right
   --  length holding a different, plausible character and nothing
   --  downstream can tell.
   Code_Point_Out_Of_Range : exception;

   --  A member id past the 28 bits of an EMHEADER (§7.4.3.4.2) or past
   --  `Pid_Short_Max_Id` in a short parameter header (§7.4.1.2). A
   --  truncating header puts a *different* member's id on the wire.
   Member_Id_Out_Of_Range : exception;

   --  A member body whose octet count is not the one its length code
   --  frames: LC0..LC3 are 1, 2, 4 and 8 exactly, and LC5..LC7 need at
   --  least the four octets they take their length from. Silence here
   --  puts a length on the wire that no octet count backs.
   Length_Code_Mismatch : exception;

   --  A `bitset` anchor that is not one, two, four or eight octets.
   --  XTypes 1.3 §7.3.1.4 gives the holder those four widths and no
   --  other.
   Anchor_Not_A_Holder_Width : exception;

   --  A decimal a `fixed<P,S>` cannot hold: not a decimal at all, or
   --  more integer digits than P - S, or more fraction digits than S.
   Not_A_Fixed_Value : exception;

   --  A parameter value longer than the 65535 octets a short parameter
   --  header's `unsigned short` length describes. §7.4.1.2 answers this
   --  with the extended header, which `Pid_Extended_Begin` writes.
   Parameter_Too_Long : exception;

   --  Alignment caps: XCDR2 = 4 (PLAIN_CDR2), XCDR1 = 8 (PLAIN_CDR).
   Xcdr2_Max_Align : constant := 4;
   Xcdr1_Max_Align : constant := 8;

   --  Fixed maximum frame size -- an endpoint serializes into a bounded buffer
   --  (no heap). Raise this if a larger single sample is ever needed.
   Max_Buffer : constant := 4096;

   --  --- writer over a self-owned, bounded buffer ---

   type Writer is limited record
      Buf       : Byte_Array (0 .. Max_Buffer - 1) := [others => 0];
      Len       : Natural   := 0;
      Endian    : Endianness := Little;
      Max_Align : Positive  := Xcdr2_Max_Align;
      Overflow  : Boolean   := False;
      --  The position alignment is measured from. Zero for the stream,
      --  and moved to the start of a PL_CDR1 parameter's value for the
      --  length of that parameter: §7.4.1.2 aligns a parameter's value
      --  against its own block and not against the stream. Without it
      --  the `double` of `ext_mutable` takes four octets of padding and
      --  its header declares twelve, where Cyclone, RTI and Fast DDS all
      --  declare eight.
      Origin    : Natural   := 0;
   end record;

   procedure Init (W : in out Writer; Endian : Endianness)
     with Post => W.Len = 0 and not W.Overflow and W.Endian = Endian;

   --  The bytes written so far.
   function Bytes (W : Writer) return Byte_Array
     with Post => Bytes'Result'Length = W.Len;

   --  Aligned primitives (set Overflow on a full buffer).
   procedure Put_U8   (W : in out Writer; V : Byte);
   procedure Put_U16  (W : in out Writer; V : Unsigned_16);
   procedure Put_U32  (W : in out Writer; V : Unsigned_32);
   procedure Put_U64  (W : in out Writer; V : Unsigned_64);
   procedure Put_Bool (W : in out Writer; V : Boolean);
   procedure Put_F32  (W : in out Writer; V : IEEE_Float_32);
   procedure Put_F64  (W : in out Writer; V : IEEE_Float_64);

   --  An IDL `char`: the one octet §7.4.1.1.1 Table 30 gives it.
   --  Raises Code_Point_Out_Of_Range above U+00FF.
   procedure Put_Char (W : in out Writer; Code_Point : Unsigned_32);

   --  An IDL `wchar`: one two-octet code unit, 2-aligned, in the
   --  stream's byte order.
   --
   --  The four policy axes a wide character carries are answered here
   --  and not by the caller: the unit is two octets, it goes out in the
   --  stream's order, there is no GIOP length octet before it and no
   --  byte-order mark. `wchar_member` measures Cyclone and Fast DDS
   --  writing `6100` for U+0061 in a little-endian XCDR1 stream and
   --  `0061` in a big-endian one. (RTI writes four octets there, which
   --  is the one place the three vendors disagree; two is the majority
   --  and the only form all three write under XCDR2.)
   --
   --  Raises Code_Point_Out_Of_Range above U+FFFF: a wider write would
   --  zero-extend a code *unit*, not a code point.
   procedure Put_Wchar (W : in out Writer; Code_Point : Unsigned_32);

   --  An IDL `wstring`, from its UTF-16 code units.
   --
   --  The length word counts **octets** and not units, there is no
   --  terminator and no byte-order mark. `wstring_member` measures
   --  Cyclone and RTI writing `04000000 6100 6200` for the two-unit
   --  "ab"; Fast DDS writes `02000000` there and is the minority.
   procedure Put_Wstring (W : in out Writer; Units : Code_Unit_Array);

   --  A `bitset` as its anchor word: the holder the declared bits pack
   --  into, written at the anchor's own width and therefore at the
   --  anchor's own alignment. XTypes 1.3 §7.3.1.4 and §7.4.1.
   --
   --  `Bits` is the packed holder and `Anchor` its width in octets, both
   --  properties of the declared type. What this call decides is the
   --  pair the type does not: which write the holder goes out through
   --  and what that write aligns to. Raises
   --  Anchor_Not_A_Holder_Width for any other width.
   procedure Put_Bitset
     (W : in out Writer; Bits : Unsigned_64; Anchor : Positive);

   --  An IDL `fixed<P,S>` as packed BCD. CDR (OMG CORBA 3.3) §9.3.2.8 /
   --  IDL 4.2 §7.4.1.4.5.
   --
   --  `Value` is the decimal as the sample wrote it -- an optional sign,
   --  an integer part and an optional fraction. Everything after that is
   --  this call's: the integer part is left-padded and the fraction
   --  right-padded to exactly `Digit_Count` digits with the point
   --  implied `Scale` from the right; a leading zero nibble is prepended
   --  when Digit_Count + 1 is odd so digits and sign pair up; two
   --  nibbles to the octet, most significant first; the last nibble is
   --  the sign, 16#D# negative and 16#C# otherwise, and a negative zero
   --  goes out positive because there is one zero to write. Alignment is
   --  one and no multi-octet integer is written, so the byte order never
   --  enters. The octet count is (Digit_Count + 2) / 2 whatever the
   --  value. Raises Not_A_Fixed_Value for a decimal the type cannot
   --  hold.
   procedure Put_Fixed
     (W           : in out Writer;
      Value       : String;
      Digit_Count : Positive;
      Scale       : Natural);

   --  CDR string: u32 length (incl. NUL) + bytes + one NUL.
   procedure Put_String (W : in out Writer; S : String);
   --  sequence<octet>: u32 length + raw bytes.
   procedure Put_Seq_U8 (W : in out Writer; Data : Byte_Array);
   --  Raw bytes, no alignment (building block for framing/sub-buffers).
   procedure Put_Bytes (W : in out Writer; Data : Byte_Array);

   --  DHEADER (XCDR2 delimited, appendable/mutable struct + non-primitive
   --  collection): 4-align + reserve a u32, back-patched with the body length.
   --  `Body_Start` is passed back to DHeader_End.
   procedure DHeader_Begin (W : in out Writer; Body_Start : out Natural);
   procedure DHeader_End (W : in out Writer; Body_Start : Natural);

   --  EMHEADER (XCDR2 @mutable, length code LC4): a u32
   --  (M<<31)+(LC4<<28)+member_id then a NEXTINT (u32 body length).
   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Body_Start      : out Natural);
   procedure EMHeader_End (W : in out Writer; Body_Start : Natural);

   --  The same, in any of the eight length codes of §7.4.3.4.2:
   --
   --     EMHEADER = (must_understand << 31) | (code << 28) | member_id
   --
   --  with a 28-bit member id. LC4 reserves a NEXTINT after the header
   --  and EMHeader_End fills it in. LC0..LC3 write nothing after the
   --  header, because the code alone gives the length. LC5..LC7 write
   --  nothing either: the member body's own leading four octets *are*
   --  the length and belong to the body, so a second NEXTINT would put
   --  every following member four octets late -- `lc_5_6_7` measures
   --  Fast DDS writing `01000050 04000000 61626300` for a string member,
   --  with one length word and not two.
   --
   --  Raises Member_Id_Out_Of_Range above 2**28 - 1.
   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out Natural);

   --  Closes the member EMHeader_Begin opened. The code is passed back
   --  rather than remembered, so a caller that paired an LC4 begin with
   --  a differently coded end is refused instead of having a NEXTINT
   --  patched over the first four octets of a member body. Raises
   --  Length_Code_Mismatch when the body's octet count is not the one
   --  the code frames.
   procedure EMHeader_End
     (W : in out Writer; Body_Start : Natural; Code : Length_Code);

   --  --- the PL_CDR1 parameter list (XTypes 1.3 §7.4.1.2) ---

   --  An open parameter, handed back to Pid_End.
   type Pid_Header is private;

   --  A short parameter header: at a four-octet boundary an `unsigned
   --  short` id then an `unsigned short` byte length, both in the
   --  stream's order. The must-understand flag is bit 14 of the id
   --  field; bit 15 is the implementation-extension flag and is not set
   --  here.
   --
   --  The value's alignment origin moves to the block, and Pid_End
   --  restores it. Raises Member_Id_Out_Of_Range above
   --  Pid_Short_Max_Id.
   procedure Pid_Begin
     (W               : in out Writer;
      Pid             : Unsigned_16;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  The twelve-octet header §7.4.1.2 requires for a member id above
   --  Pid_Short_Max_Id or a value longer than 65535 octets:
   --
   --     [PID_EXTENDED | MU : u16][8 : u16][eMemberHeader : u32][len : u32]
   --
   --  The flag on the marker is not the member's -- it is on the marker
   --  under this framing whatever the member says, and the member's own
   --  flag is bit 30 of the eMemberHeader. `mut_mustunderstand`
   --  measures Cyclone writing `017f 0800 40000001` for a
   --  must-understand member and `017f 0800 00000002` for a plain one,
   --  with the marker the same in both.
   procedure Pid_Extended_Begin
     (W               : in out Writer;
      Member_Id       : Unsigned_32;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  Closes the parameter either Begin opened.
   --
   --  The declared length counts the value's octets **and** the padding
   --  that brings the next header back to a four-octet boundary: a
   --  reader skips `length` from the end of the header and has to land
   --  on the next parameter. `ext_mutable` measures RTI writing
   --  `0200 0400 fdff 0000` for a `short` -- four, not two -- and
   --  Cyclone declaring the same four in its extended header. (Fast DDS
   --  declares two and is the minority.) Raises Parameter_Too_Long when
   --  a short header cannot describe the result.
   procedure Pid_End (W : in out Writer; Header : Pid_Header);

   --  The sentinel that closes a parameter list: PID_LIST_END with a
   --  zero length, four-aligned.
   --
   --  The must-understand flag is set, so the octets are 0x7F02, which
   --  is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02;
   --  the flag is not the sentinel's meaning either way and this is the
   --  majority form.
   procedure Pid_List_End (W : in out Writer);

   --  --- reader over a bounded, self-owned buffer ---

   type Reader is limited record
      Buf       : Byte_Array (0 .. Max_Buffer - 1) := [others => 0];
      Len       : Natural   := 0;
      Pos       : Natural   := 0;
      Endian    : Endianness := Little;
      Max_Align : Positive  := Xcdr2_Max_Align;
      Underflow : Boolean   := False;
      --  The writer's Origin, mirrored: a PL_CDR1 parameter's value
      --  aligns against its own block on the way in as well, or the
      --  reader lands on a different octet than the writer left.
      Origin    : Natural   := 0;
   end record;

   procedure Init (R : in out Reader; Data : Byte_Array; Endian : Endianness);

   procedure Get_U8   (R : in out Reader; V : out Byte);
   procedure Get_U16  (R : in out Reader; V : out Unsigned_16);
   procedure Get_U32  (R : in out Reader; V : out Unsigned_32);
   procedure Get_U64  (R : in out Reader; V : out Unsigned_64);
   procedure Get_Bool (R : in out Reader; V : out Boolean);
   procedure Get_F32  (R : in out Reader; V : out IEEE_Float_32);
   procedure Get_F64  (R : in out Reader; V : out IEEE_Float_64);

   --  Reads a CDR string into `S` (Last = index of last char, may be 0-length
   --  body). Sets Underflow on a malformed/oversize string.
   procedure Get_String
     (R : in out Reader; S : out String; Last : out Natural);
   procedure Get_Seq_U8
     (R : in out Reader; Out_D : out Byte_Array; N : out Natural);

   --  Raw octets at the cursor, no alignment. The counterpart of
   --  Put_Bytes, and what a caller needs to step over a member body an
   --  EMHeader_Read measured.
   procedure Get_Bytes (R : in out Reader; Out_D : out Byte_Array);

   --  An IDL `char` and an IDL `wchar`, back as code points.
   procedure Get_Char  (R : in out Reader; Code_Point : out Unsigned_32);
   procedure Get_Wchar (R : in out Reader; Code_Point : out Unsigned_32);

   --  A `wstring` back into its code units. The length word counts
   --  octets, so the unit count is half of it; `N` is that count and
   --  Underflow is set when `Units` is too small to hold it.
   procedure Get_Wstring
     (R : in out Reader; Units : out Code_Unit_Array; N : out Natural);

   --  A `bitset` anchor of the given width, back into its holder word.
   procedure Get_Bitset
     (R : in out Reader; Anchor : Positive; Bits : out Unsigned_64);

   --  A `fixed<P,S>` back into the decimal it was written from: a
   --  leading '-' when the sign nibble is 16#D#, Digit_Count - Scale
   --  integer digits, and a '.' with Scale fraction digits where the
   --  scale is not zero. `Last` indexes the final character written.
   --  The scale is not on the wire, so it is a parameter here for the
   --  same reason it is one in Put_Fixed.
   procedure Get_Fixed
     (R           : in out Reader;
      Digit_Count : Positive;
      Scale       : Natural;
      Value       : out String;
      Last        : out Natural);

   procedure DHeader_Read (R : in out Reader; Len : out Unsigned_32);
   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Nextint         : out Unsigned_32);

   --  A member header in any length code, with the body's extent in
   --  octets measured from the position this call leaves the cursor at.
   --
   --  LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT.
   --  LC5..LC7 consume nothing: the next four octets are the member's
   --  own leading word and belong to the body, so they are read and the
   --  cursor put back, and the extent is 4 + word for LC5, 4 + 4*word
   --  for LC6 and 4 + 8*word for LC7 -- the last two count elements and
   --  not octets.
   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out Unsigned_32);

   --  A PL_CDR1 parameter header in either form: the member id with its
   --  flags stripped, the member's own must-understand flag, the
   --  declared byte length, and whether the list ends here.
   --
   --  The extended form is recognised by the marker PID_EXTENDED, and
   --  the must-understand answer then comes from bit 30 of the
   --  eMemberHeader and never from the marker -- reading it off the
   --  marker makes every parameter must-understand.
   --
   --  The value's alignment origin moves to the position this call
   --  leaves, and Pid_Skip restores it.
   procedure Pid_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Length          : out Unsigned_32;
      List_End        : out Boolean);

   --  Past a parameter value of `Length` octets from `Body_Start`, and
   --  the stream's alignment origin back.
   procedure Pid_Skip
     (R : in out Reader; Body_Start : Natural; Length : Unsigned_32);

   --  Padding to push `Pos` to the next multiple of `Align` (power of two).
   function Padding_For (Pos : Natural; Align : Positive) return Natural
     with Post => Padding_For'Result < Align;

private

   type Pid_Header is record
      --  Where the length field starts.
      At_Pos        : Natural := 0;
      --  Whether that field is four octets or two.
      Extended      : Boolean := False;
      --  The alignment origin to put back when the parameter closes.
      Saved_Origin  : Natural := 0;
   end record;

end Zerodds_Native_Wire;
