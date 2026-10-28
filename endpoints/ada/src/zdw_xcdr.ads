-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Ada Stage 1, the part `endpoints/c` has not got.
--
--  `Zdw` binds the C89 wire-core one for one. This package is everything an
--  XCDR writer needs that the C core does not export at all, written in Ada
--  over `Zdw`'s primitives so that the octets still come out of
--  `zerodds_wire.c` and only the framing decisions are made here:
--
--  * the seven length codes `zdw_emheader_begin` has no argument for --
--    it writes LC4 and nothing else, and the recorded octets of Cyclone,
--    RTI and Fast DDS carry LC1, LC2, LC3, LC5, LC6 and LC7;
--  * the PL_CDR1 parameter list (XTypes 1.3 section 7.4.1.2), which the C
--    core has no operation for in either direction;
--  * `char`, `wchar`, `wstring`, `bitset` and `fixed<P,S>`, which the C core
--    has no writer for.
--
--  ## The one place a caller has to do something it would not do elsewhere
--
--  Section 7.4.1.2 aligns a PL_CDR1 parameter's value against the parameter
--  block and not against the stream. `zdw_writer` (zerodds_wire.h) carries
--  `buf`, `cap`, `len`, `endian`, `max_align` and `error` -- there is no
--  origin field, and `zdw_align` measures `w->len % align` from the stream.
--  For alignments of one, two and four that is the same boundary, because a
--  parameter block always starts four-aligned; for eight it is not, and no
--  setting of `max_align` reproduces it.
--
--  So `Pid_Begin` sets `max_align` to one for the length of the parameter --
--  the C writer then aligns nothing -- and every boundary inside the block is
--  written by `Align_Block`, which measures from the block. `Pid_End` puts
--  `max_align` back. The alternative would be for this package to reimplement
--  the primitive writes, which is the thing Stage 1 exists not to do.

with Interfaces.C;
with Zdw;

package Zdw_Xcdr is

   use Interfaces.C;

   --  The member header length codes of XTypes 1.3 section 7.4.3.4.2
   --  Table 34. 'Pos is the code.
   type Length_Code is (LC0, LC1, LC2, LC3, LC4, LC5, LC6, LC7);

   --  PL_CDR1 parameter ids. Section 7.4.1.2 Table 31.
   Pid_List_End_Id  : constant := 16#3F02#;
   Pid_Extended_Id  : constant := 16#3F01#;
   --  Inclusive: RTI and Fast DDS both put member id 16128 in the short
   --  form (`pid_3f00`).
   Pid_Short_Max_Id : constant := 16#3F00#;

   --  One UTF-16 code unit. A code point outside the BMP is the surrogate
   --  pair the caller hands over.
   type Code_Unit is range 0 .. 65535;
   type Code_Unit_Array is array (Natural range <>) of Code_Unit;

   --  A code point a one-octet `char` or a two-octet `wchar` cannot carry.
   --  U+0161's low octet is 16#61#, which is 'a': a truncating writer would
   --  produce a frame of the right length holding a plausible, wrong
   --  character.
   Code_Point_Out_Of_Range : exception;

   --  A member id past the 28 bits of an EMHEADER, or past
   --  Pid_Short_Max_Id in a short parameter header.
   Member_Id_Out_Of_Range : exception;

   --  A member body whose octet count is not the one its length code frames.
   Length_Code_Mismatch : exception;

   --  A `bitset` anchor that is not one, two, four or eight octets.
   Anchor_Not_A_Holder_Width : exception;

   --  A decimal a `fixed<P,S>` cannot hold.
   Not_A_Fixed_Value : exception;

   --  A parameter value past the 65535 octets a short header describes.
   Parameter_Too_Long : exception;

   --  The C core refused a write. Raised rather than returned, because
   --  every operation here is several C calls and a code from the middle of
   --  one says nothing about the frame.
   Wire_Refused : exception;

   --  --- the member header, in any length code ---

   --  LC4 reserves a NEXTINT after the header; LC0..LC3 write nothing after
   --  it, the code being the length; LC5..LC7 write nothing either,
   --  because the body's own leading four octets are the length and belong
   --  to the body. Raises Member_Id_Out_Of_Range above 2**28 - 1.
   procedure EMHeader_Begin
     (W               : access Zdw.Zdw_Writer;
      Member_Id       : unsigned_long;
      Must_Understand : Boolean;
      Code            : Length_Code;
      Body_Start      : out size_t);

   --  Closes the member. The code comes back rather than being remembered,
   --  so an LC4 begin paired with another code's end is refused instead of
   --  patching a NEXTINT over a member body. Raises Length_Code_Mismatch
   --  when the body's octet count is not the one the code frames.
   procedure EMHeader_End
     (W : access Zdw.Zdw_Writer; Body_Start : size_t; Code : Length_Code);

   --  --- the PL_CDR1 parameter list ---

   type Pid_Header is private;

   --  A short parameter header: a 4-aligned u16 id then a u16 byte length.
   --  The must-understand flag is bit 14; bit 15 is the
   --  implementation-extension flag and is not set here.
   procedure Pid_Begin
     (W               : access Zdw.Zdw_Writer;
      Pid             : unsigned_long;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  The twelve-octet header for an id above Pid_Short_Max_Id or a value
   --  past 65535 octets. The marker carries the flag under this framing
   --  whatever the member says; the member's own flag is bit 30 of the
   --  eMemberHeader.
   procedure Pid_Extended_Begin
     (W               : access Zdw.Zdw_Writer;
      Member_Id       : unsigned_long;
      Must_Understand : Boolean;
      Header          : out Pid_Header);

   --  A boundary inside the open parameter, measured from the parameter
   --  block. See the note at the top of this package: the C writer has no
   --  origin field, so this is where a parameter body's padding comes from.
   procedure Align_Block
     (W : access Zdw.Zdw_Writer; Header : Pid_Header; Align : size_t);

   --  Closes the parameter. The declared length counts the value's octets
   --  and the padding that brings the next header back to a four-octet
   --  boundary -- RTI writes 4 and not 2 for a `short`. Raises
   --  Parameter_Too_Long when a short header cannot describe the result.
   procedure Pid_End
     (W : access Zdw.Zdw_Writer; Header : Pid_Header);

   --  The sentinel, with the must-understand flag set: 0x7F02, which is
   --  what Cyclone and RTI write.
   procedure Pid_List_End (W : access Zdw.Zdw_Writer);

   --  --- the scalars the C core has no writer for ---

   procedure Put_Char
     (W : access Zdw.Zdw_Writer; Code_Point : unsigned_long);

   --  Two octets, 2-aligned, in the stream's order. No GIOP length octet
   --  and no byte-order mark. Raises Code_Point_Out_Of_Range above U+FFFF.
   procedure Put_Wchar
     (W : access Zdw.Zdw_Writer; Code_Point : unsigned_long);

   --  The length word counts octets and not units, and there is no
   --  terminator: Cyclone and RTI both write `04000000` for two units.
   procedure Put_Wstring
     (W : access Zdw.Zdw_Writer; Units : Code_Unit_Array);

   --  A `bitset` anchor, at the anchor's own width and therefore at the
   --  anchor's own alignment. The holder is a pair of halves for the same
   --  reason `Zdw.Zdw_U64` is one.
   procedure Put_Bitset
     (W      : access Zdw.Zdw_Writer;
      Bits   : Zdw.Zdw_U64;
      Anchor : Natural);

   --  Packed BCD. CDR (OMG CORBA 3.3) section 9.3.2.8: the integer part
   --  left-padded and the fraction right-padded to Digit_Count digits, a
   --  zero nibble prepended when Digit_Count + 1 is odd, the sign in the
   --  last nibble (16#D# negative, 16#C# otherwise), and a negative zero
   --  written positive. Alignment is one and no multi-octet integer is
   --  written, so the byte order never enters.
   procedure Put_Fixed
     (W           : access Zdw.Zdw_Writer;
      Value       : String;
      Digit_Count : Positive;
      Scale       : Natural);

   --  --- the reading direction ---

   --  A member header in any length code, with the body's extent in octets
   --  measured from where this call leaves the cursor. LC5..LC7 consume
   --  nothing: the next four octets belong to the body.
   procedure EMHeader_Read
     (R               : access Zdw.Zdw_Reader;
      Member_Id       : out unsigned_long;
      Must_Understand : out Boolean;
      Code            : out Length_Code;
      Extent          : out unsigned_long);

   --  A parameter header in either form. For the extended one the
   --  must-understand answer comes from the eMemberHeader and never from
   --  the marker.
   --
   --  `Header` carries the block the value aligns against, for the same
   --  reason the writer's does: `zdw_reader` has no origin field either, so
   --  the reader's alignment cap goes to one for the length of the
   --  parameter and `Align_Block_In` writes the boundary. A `double` in a
   --  parameter whose block starts four octets past an eight boundary is
   --  read four octets late otherwise, which is what `ext_mutable` under
   --  PL_CDR1 does.
   procedure Pid_Read
     (R               : access Zdw.Zdw_Reader;
      Member_Id       : out unsigned_long;
      Must_Understand : out Boolean;
      Length          : out unsigned_long;
      List_End        : out Boolean;
      Header          : out Pid_Header);

   --  A boundary inside the parameter Pid_Read opened, measured from its
   --  block.
   procedure Align_Block_In
     (R : access Zdw.Zdw_Reader; Header : Pid_Header; Align : size_t);

   --  Past a parameter value of `Length` octets, and the reader's
   --  alignment cap back.
   procedure Pid_Skip
     (R : access Zdw.Zdw_Reader; Header : Pid_Header; Length : unsigned_long);

   procedure Get_Char
     (R : access Zdw.Zdw_Reader; Code_Point : out unsigned_long);
   procedure Get_Wchar
     (R : access Zdw.Zdw_Reader; Code_Point : out unsigned_long);
   procedure Get_Wstring
     (R : access Zdw.Zdw_Reader; Units : out Code_Unit_Array; N : out Natural);
   procedure Get_Bitset
     (R : access Zdw.Zdw_Reader; Anchor : Natural; Bits : out Zdw.Zdw_U64);
   procedure Get_Fixed
     (R           : access Zdw.Zdw_Reader;
      Digit_Count : Positive;
      Scale       : Natural;
      Value       : out String;
      Last        : out Natural);

private

   type Pid_Header is record
      --  Where the length field starts.
      At_Pos     : size_t := 0;
      --  Where the parameter's value starts: the origin its own alignment
      --  is measured from.
      Origin     : size_t := 0;
      Extended   : Boolean := False;
      --  The writer's alignment cap, to put back when the parameter
      --  closes.
      Saved_Cap  : int := Zdw.ZDW_XCDR2_MAX_ALIGN;
   end record;

end Zdw_Xcdr;
