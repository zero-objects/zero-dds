-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 endpoint wire-core (ADR 0013). The OLDEST-legacy variant:
--  strict Ada 83 -- no modular types, no Interfaces, no tagged types, no
--  access-to-subprogram, no child units. Byte manipulation is done with
--  Long_Integer div/mod (host-independent) so the output is byte-identical to
--  the Rust core and the other SDKs. For toolchains that predate Object Ada.
--
--  Compiled with -gnat83. Additive: the modern procedural (ada-native) and
--  Object-Ada variants stay untouched.
--
--  ## What the 1983 standard costs this binding, and what it does not
--
--  Three of the operations below have a shape they would not have in a later
--  Ada, and the reason is the language and not the wire:
--
--  * A 64-bit value is a pair of Long_Integer halves. ANSI/MIL-STD-1815A has
--    no modular type and no guaranteed 64-bit integer, so `Put_U64` takes
--    `Hi, Lo` -- the same shape the C89 wire-core's `zdw_u64_t` takes, for the
--    same reason.
--  * A `wstring` is an array of `Code_Unit` and not of `Wide_Character`.
--    RM83 3.5.2 defines CHARACTER over 128 positions and defines no wide
--    character type at all; WIDE_CHARACTER and WIDE_STRING are Ada 95
--    additions. So the unit type is declared here.
--  * A `fixed<P,S>` value arrives as a String. Ada 83 has fixed point types
--    (RM83 3.5.9) but no decimal fixed point -- `type ... is delta ... digits
--    ...` is an Ada 95 addition -- and no 31-digit numeric type of any kind.
--
--  Nothing else is missing. In particular LONG_FLOAT is a predefined type of
--  the 1983 standard (RM83 3.5.7), so `double` has a writer here, and the
--  eight-octet alignment XCDR1 requires is a field like any other.

package Zerodds_Ada83_Wire is

   type Byte is range 0 .. 255;
   for Byte'Size use 8;  -- one storage byte: Sequential_IO reads 1 file byte per
                         -- element, and Unchecked_Conversion sizes line up.
   type Byte_Array is array (Integer range <>) of Byte;
   pragma Pack (Byte_Array);
   type Endianness is (Little, Big);

   --  One UTF-16 code unit. A code point outside the BMP is the surrogate
   --  pair the caller hands over: U+1D11E is two units, and Cyclone and RTI
   --  both write `34d8 1edd` for it in a little-endian stream.
   type Code_Unit is range 0 .. 65535;
   type Code_Unit_Array is array (Integer range <>) of Code_Unit;

   --  The member header length codes of XTypes 1.3 section 7.4.3.4.2
   --  Table 34. 'Pos is the code.
   type Length_Code is (LC0, LC1, LC2, LC3, LC4, LC5, LC6, LC7);

   --  Alignment caps: XCDR2 = 4 (PLAIN_CDR2), XCDR1 = 8 (PLAIN_CDR).
   --  XTypes 1.3 section 7.4.1.
   Xcdr2_Max_Align : constant := 4;
   Xcdr1_Max_Align : constant := 8;

   --  PL_CDR1 parameter ids. XTypes 1.3 section 7.4.1.2 Table 31.
   Pid_List_End_Id  : constant := 16#3F02#;
   Pid_Extended_Id  : constant := 16#3F01#;
   --  The highest member id a short parameter header carries, inclusive:
   --  RTI and Fast DDS both write `003f 0400` for id 16128.
   Pid_Short_Max_Id : constant := 16#3F00#;

   Max_Buffer : constant := 2048;

   type Writer is record
      Buf      : Byte_Array (0 .. Max_Buffer - 1);
      Len      : Integer := 0;
      Endian   : Endianness := Little;
      Overflow : Boolean := False;
      --  The alignment cap. Four is XCDR2's; XCDR1 needs eight, and
      --  without the field every `long long` and every `double` in an
      --  XCDR1 sample landed four octets early.
      Max_Align : Integer := Xcdr2_Max_Align;
      --  The position alignment is measured from: the stream, except
      --  inside an open PL_CDR1 parameter, whose value aligns against
      --  its own block (section 7.4.1.2).
      Origin    : Integer := 0;
   end record;

   --  A code point a one-octet `char` or a two-octet `wchar` cannot carry.
   --  Refused and never truncated: U+0161's low octet is 16#61#, which is
   --  'a', so a truncating writer would produce a frame of the right length
   --  holding a different, plausible character.
   Code_Point_Out_Of_Range : exception;

   --  A member id past the 28 bits of an EMHEADER, or past
   --  Pid_Short_Max_Id in a short parameter header.
   Member_Id_Out_Of_Range : exception;

   --  A member body whose octet count is not the one its length code
   --  frames.
   Length_Code_Mismatch : exception;

   --  A `bitset` anchor that is not one, two, four or eight octets.
   Anchor_Not_A_Holder_Width : exception;

   --  A decimal a `fixed<P,S>` cannot hold.
   Not_A_Fixed_Value : exception;

   --  A parameter value longer than the 65535 octets a short parameter
   --  header describes.
   Parameter_Too_Long : exception;

   --  A value the wire width cannot hold. Ada 83 has no unsigned type,
   --  so the primitives below take a Long_Integer and the range is
   --  theirs to check: the low octets of 256 written as a `uint8` are
   --  a plausible zero, and nothing on the receiving side can tell it
   --  from a written zero. XTypes 1.3 Table 31 gives each width its
   --  extent; a negative value is admitted as the two's complement
   --  pattern of that width, which is what the corpus records for
   --  `negative-u16` and `negative-u32`.
   Value_Out_Of_Range : exception;

   procedure Init (W : in out Writer; Endian : Endianness);

   --  Aligned primitives. Values are passed as Long_Integer (Ada 83 has no
   --  unsigned type); the caller supplies the numeric value.
   procedure Put_U8  (W : in out Writer; V : Long_Integer);
   procedure Put_U16 (W : in out Writer; V : Long_Integer);
   procedure Put_U32 (W : in out Writer; V : Long_Integer);
   --  uint64 as two 32-bit halves (like the C wire-core's zdw_u64_t).
   procedure Put_U64 (W : in out Writer; Hi, Lo : Long_Integer);
   --  A `boolean` as the one octet section 7.4.1.1.1 Table 30 gives it:
   --  0 or 1, never any other pattern.
   procedure Put_Bool (W : in out Writer; V : Boolean);
   procedure Put_F32 (W : in out Writer; V : Float);
   --  LONG_FLOAT is a predefined type of the 1983 standard (RM83 3.5.7)
   --  and is the IEEE binary64 an IDL `double` needs.
   procedure Put_F64 (W : in out Writer; V : Long_Float);
   procedure Put_String (W : in out Writer; S : String);
   procedure Put_Seq_U8 (W : in out Writer; Data : Byte_Array);
   procedure Put_Bytes (W : in out Writer; Data : Byte_Array);

   --  An IDL `char`: one octet. Raises Code_Point_Out_Of_Range above
   --  U+00FF.
   procedure Put_Char (W : in out Writer; Code_Point : Long_Integer);

   --  An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's
   --  byte order. There is no GIOP length octet and no byte-order mark.
   --  Raises Code_Point_Out_Of_Range above U+FFFF.
   procedure Put_Wchar (W : in out Writer; Code_Point : Long_Integer);

   --  An IDL `wstring`. The length word counts octets and not units, and
   --  there is no terminator: Cyclone and RTI both write `04000000` for a
   --  two-unit string.
   procedure Put_Wstring (W : in out Writer; Units : Code_Unit_Array);

   --  A `bitset` as its anchor word, at the anchor's own width and
   --  therefore at the anchor's own alignment. The holder is a pair of
   --  halves for the same reason Put_U64's is. Raises
   --  Anchor_Not_A_Holder_Width for any width but 1, 2, 4 and 8.
   procedure Put_Bitset
     (W : in out Writer; Hi, Lo : Long_Integer; Anchor : Integer);

   --  An IDL `fixed<P,S>` as packed BCD. CDR (OMG CORBA 3.3) section
   --  9.3.2.8. The integer part is left-padded and the fraction
   --  right-padded to exactly Digit_Count digits; a zero nibble is
   --  prepended when Digit_Count + 1 is odd; the last nibble is the sign,
   --  16#D# negative and 16#C# otherwise, and a negative zero goes out
   --  positive. Alignment is one and no multi-octet integer is written,
   --  so the byte order never enters -- which is why omniORB records the
   --  same octets in both. Raises Not_A_Fixed_Value for a decimal the
   --  type cannot hold.
   procedure Put_Fixed
     (W           : in out Writer;
      Value       : String;
      Digit_Count : Integer;
      Scale       : Integer);

   --  DHEADER (XCDR2 delimited): 4-align, reserve a u32, back-patch with
   --  the body's byte length. The DHEADER itself is not counted.
   --  Section 7.4.3.5.
   procedure DHeader_Begin (W : in out Writer; Body_Start : out Integer);
   procedure DHeader_End (W : in out Writer; Body_Start : Integer);

   --  EMHEADER (XCDR2 @mutable) in any of the eight length codes:
   --
   --     EMHEADER = (must_understand * 2**31) + (code * 2**28) + member_id
   --
   --  LC4 reserves a NEXTINT after the header; LC0..LC3 take the length
   --  from the code; LC5..LC7 take it from the member body's own leading
   --  word, which stays part of the body. Raises Member_Id_Out_Of_Range
   --  above 2**28 - 1.
   procedure EMHeader_Begin
     (W               : in out Writer;
      Member_Id       : Long_Integer;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out Integer);

   --  Closes the member. The code is passed back rather than remembered,
   --  so an LC4 begin paired with another code's end is refused instead
   --  of patching a NEXTINT over a member body. Raises
   --  Length_Code_Mismatch when the body's octet count is not the one the
   --  code frames.
   procedure EMHeader_End
     (W : in out Writer; Body_Start : Integer; Code : Length_Code);

   --  --- the PL_CDR1 parameter list (section 7.4.1.2) ---

   type Pid_Header is private;

   --  A short parameter header: a 4-aligned u16 id then a u16 byte
   --  length, both in the stream's order. The must-understand flag is bit
   --  14 of the id field; bit 15 is the implementation-extension flag and
   --  is not set here. The value's alignment origin moves to the block.
   --  Raises Member_Id_Out_Of_Range above Pid_Short_Max_Id.
   procedure Pid_Begin
     (W               : in out Writer;
      Pid             : Long_Integer;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  The twelve-octet header for a member id above Pid_Short_Max_Id or a
   --  value past 65535 octets. The flag on the marker is not the
   --  member's: the member's is bit 30 of the eMemberHeader.
   procedure Pid_Extended_Begin
     (W               : in out Writer;
      Member_Id       : Long_Integer;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  Closes the parameter. The declared length counts the value's octets
   --  and the padding that brings the next header back to a four-octet
   --  boundary -- RTI writes 4 and not 2 for a `short`. Raises
   --  Parameter_Too_Long when a short header cannot describe the result.
   procedure Pid_End (W : in out Writer; Header : Pid_Header);

   --  The sentinel: PID_LIST_END with a zero length, four-aligned, with
   --  the must-understand flag set -- 0x7F02, which is what Cyclone and
   --  RTI write.
   procedure Pid_List_End (W : in out Writer);

   function Length (W : Writer) return Integer;

   --  --- reader ---

   type Reader is record
      Data   : Byte_Array (0 .. Max_Buffer - 1);
      Len    : Integer := 0;
      Pos    : Integer := 0;
      Endian : Endianness := Little;
      --  The writer's two, mirrored: a reader that aligned differently
      --  lands on a different octet than the writer left.
      Max_Align : Integer := Xcdr2_Max_Align;
      Origin    : Integer := 0;
      --  Sticky: set when a read ran past the octets there are.
      Underflow : Boolean := False;
   end record;

   procedure Init_Reader (R : out Reader; Data : Byte_Array; Endian : Endianness);

   procedure Get_U8  (R : in out Reader; V : out Long_Integer);
   procedure Get_U16 (R : in out Reader; V : out Long_Integer);
   procedure Get_U32 (R : in out Reader; V : out Long_Integer);
   procedure Get_U64 (R : in out Reader; Hi, Lo : out Long_Integer);
   procedure Get_Bool (R : in out Reader; V : out Boolean);
   procedure Get_F32 (R : in out Reader; V : out Float);
   procedure Get_F64 (R : in out Reader; V : out Long_Float);
   procedure Get_Bytes (R : in out Reader; Out_D : out Byte_Array);
   procedure Get_String (R : in out Reader; S : out String; Last : out Integer);
   procedure Get_Seq_U8
     (R : in out Reader; Out_D : out Byte_Array; N : out Integer);

   procedure Get_Char  (R : in out Reader; Code_Point : out Long_Integer);
   procedure Get_Wchar (R : in out Reader; Code_Point : out Long_Integer);
   procedure Get_Wstring
     (R : in out Reader; Units : out Code_Unit_Array; N : out Integer);
   procedure Get_Bitset
     (R : in out Reader; Anchor : Integer; Hi, Lo : out Long_Integer);
   procedure Get_Fixed
     (R           : in out Reader;
      Digit_Count : Integer;
      Scale       : Integer;
      Value       : out String;
      Last        : out Integer);

   procedure DHeader_Read (R : in out Reader; Len : out Long_Integer);

   --  A member header with the body's extent in octets, measured from the
   --  position this call leaves the cursor at. LC5..LC7 consume nothing:
   --  the next four octets belong to the body.
   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Long_Integer;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out Long_Integer);

   --  A parameter header in either form. The must-understand answer for
   --  the extended form comes from the eMemberHeader and never from the
   --  marker.
   procedure Pid_Read
     (R               : in out Reader;
      Member_Id       : out Long_Integer;
      Must_Understand : out Boolean;
      Length          : out Long_Integer;
      List_End        : out Boolean);

   procedure Pid_Skip
     (R : in out Reader; Body_Start : Integer; Length : Long_Integer);

   --  Padding to push `Pos` to the next multiple of `Align`.
   function Padding_For (Pos : Integer; Align : Integer) return Integer;

private

   type Pid_Header is record
      At_Pos       : Integer := 0;
      Extended     : Boolean := False;
      Saved_Origin : Integer := 0;
   end record;

end Zerodds_Ada83_Wire;
