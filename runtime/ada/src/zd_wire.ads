--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  The Ada wire runtime: DDS-XTypes 1.3 7.4 from the clause.
--
--  The counterpart of crates/wire, runtime/c, runtime/d and the rest --
--  a further implementation of the same clauses, not a binding to any
--  of them. Where two columns disagree on an octet, one of them has
--  misread the specification, and that is the point of having both.
--  Nothing here decides a type question: which members are written, in
--  what order, under which header and with which length code arrives as
--  arguments from the generated codec.
--
--  A malformed stream or a value outside its type raises Wire_Error and
--  unwinds to the harness, which turns it into the verdict's exit code;
--  a refusal about the *type* sets the reader's status instead and the
--  generated decoder walks on to the end of the sample.
--
--  One deliberate gap: an IDL `long double` (binary128) is held as
--  decimal text and needs a correctly rounded parse over a big integer;
--  that work rests with the f128 arc, so Write_F128 and Read_F128 raise
--  Wire_Error by name rather than writing approximate octets.
--
--  The reader's Finish_Pid_Read measures the value against its start
--  (Value_At) and aligns to four only under Pad_After_Value -- the
--  framing whose writer padded; under Fast DDS's framing the next
--  parameter aligns itself and a final parameter ends the stream where
--  its value does.

with Ada.Strings.Unbounded;
with Interfaces;

package Zd_Wire is

   use type Interfaces.Unsigned_8;
   use type Interfaces.Unsigned_16;
   use type Interfaces.Unsigned_32;
   use type Interfaces.Unsigned_64;
   use type Interfaces.Integer_64;

   subtype Text is Ada.Strings.Unbounded.Unbounded_String;

   type Byte_Array is array (Natural range <>) of Interfaces.Unsigned_8;
   type Byte_Array_Access is access Byte_Array;

   --  Every wire failure: a malformed stream, a value outside its type,
   --  a policy name no clause defines.
   Wire_Error : exception;

   --  The byte order a stream is written or read in.
   type Byte_Order is (Little, Big);

   --  What a receiving position does with a value it cannot hold.
   --  DDS-XTypes 1.3 7.2.2.7, Table 11.
   type Try_Construct is (Tc_Discard, Tc_Use_Default, Tc_Trim);

   --  What a reader answers about the type, without raising.
   type Status is
     (Status_Ok,
      Status_Truncated,
      Status_Impossible_Length,
      Status_Out_Of_Range,
      Status_Discarded);

   --  A status, spelled the way the sibling runtimes log it.
   function Status_Text (S : Status) return String;

   --  Which of 7.4.1.2's two roles a parameter header stands in.
   type Parameter_Role is (Role_Member, Role_Optional);

   --  What flags a PL_CDR1 parameter id carries, per position.
   type Pid_Flag_Policy is
     (Flags_Sentinel_Must_Understand, Flags_Bare, Flags_Impl_Extension_Members);

   --  How a scalar wchar is framed. Wchar_Giop_Length_Prefixed is
   --  CORBA 3.3 part 2 9.3.1.6's GIOP 1.2 form: the octet count first,
   --  then the code unit's octets, aligned to one.
   type Wchar_Framing is (Wchar_Fixed, Wchar_Giop_Length_Prefixed);

   --  The order a wstring's code units go out in.
   type Unit_Order is
     (Unit_Stream, Unit_Big_Endian_Always, Unit_Byte_Order_Mark, Unit_Host_Unswapped);

   --  The C1 x C2 x C3 product of wstring length conventions: what the
   --  length counts, whether a NUL travels, and the unit's width.
   type Wstring_Policy is
     (Wstr_Octets_Unterminated_2,
      Wstr_Characters_Unterminated_2,
      Wstr_Characters_Terminated_4,
      Wstr_Octets_Unterminated_4,
      Wstr_Characters_Unterminated_4,
      Wstr_Octets_Terminated_2,
      Wstr_Octets_Terminated_4,
      Wstr_Characters_Terminated_2);

   --  How many digits a fixed writes: the ones its type declared, as
   --  many as the value has (JacORB 3.9), or the digits of the decimal
   --  as written, made odd by one leading zero -- what ACE/TAO 2.5.24's
   --  ACE_CDR::Fixed::from_string writes, where OpenDDS writes the declared count (0.12345 under
   --  fixed<5,5> is 0012345c; past 31 digits it is refused). The last
   --  two are read up to the sign nibble.
   type Fixed_Count is
     (Fixed_Declared, Fixed_Value_Precision, Fixed_Value_String_Digits);

   --  How a PL_CDR1 parameter list is framed -- the ways the measured
   --  implementations differ inside an XCDR1 sample.
   type Pl_Cdr1_Framing is record
      Pid_Flags                    : Pid_Flag_Policy := Flags_Sentinel_Must_Understand;
      --  The highest member id a short parameter header carries. Both
      --  sides: Write_Pid refuses an id above it rather than putting a
      --  reserved parameter on the wire.
      Highest_Short_Id             : Natural         := 16#3F00#;
      Pad_Declared_Length          : Boolean         := False;
      Pad_Declared_Optional_Length : Boolean         := False;
      Member_Must_Understand       : Boolean         := True;
      Pad_After_Value              : Boolean         := True;
   end record;

   --  -- handles ------------------------------------------------------

   --  A DHEADER waiting for its length; At_Pos below zero is the
   --  delimiter a @final aggregate does not write.
   type Dheader is record
      At_Pos : Integer := -1;
   end record;

   --  The DHEADER a value that takes no delimiter does not write.
   function No_Dheader return Dheader;

   --  An EMHEADER waiting for its NEXTINT; Next_Int is -1 where the
   --  length code carries the length itself.
   type Emheader is record
      Next_Int : Integer := -1;
      Word     : Natural := 0;
      --  The code the header was written with, so that Finish_Emheader
      --  can hold the member's value to the length the code declares.
      --  Codes 0 to 3 are lengths; anything else declares nothing of its
      --  own.
      Length_Code : Natural := 8;
   end record;

   --  A parameter header waiting for its length.
   type Pid_Write is record
      At_Pos          : Natural        := 0;
      Extended        : Boolean        := False;
      Saved_Origin    : Natural        := 0;
      Role            : Parameter_Role := Role_Member;
      Pid             : Natural        := 0;
      Must_Understand : Boolean        := False;
   end record;

   --  Everything a two-pass read has to put back.
   type Reader_Mark is record
      At_Pos  : Natural := 0;
      Origin  : Natural := 0;
      Current : Status  := Status_Ok;
   end record;

   --  An EMHEADER as read. Extent is how many octets the member
   --  occupies after the header word, the NEXTINT included.
   --  XTypes 1.3 7.4.3.4.2.
   type Emheader_Read is record
      Member_Id       : Interfaces.Unsigned_32 := 0;
      Length_Code     : Natural                := 0;
      Must_Understand : Boolean                := False;
      Next_Int        : Interfaces.Unsigned_32 := 0;
      Has_Next_Int    : Boolean                := False;
      Extent          : Natural                := 0;
   end record;

   --  A parameter header as read.
   type Pid_Read is record
      Pid             : Interfaces.Unsigned_32 := 0;
      Length          : Interfaces.Unsigned_32 := 0;
      End_Flag        : Boolean                := False;
      Must_Understand : Boolean                := False;
      Impl_Extension  : Boolean                := False;
      Reserved        : Boolean                := False;
      Ignore          : Boolean                := False;
      Role            : Parameter_Role         := Role_Member;
      Saved_Origin    : Natural                := 0;
      Value_At        : Natural                := 0;
   end record;

   --  Whether this parameter can be a member of the reading type:
   --  false for a vendor-private parameter and for a reserved id.
   function Pid_Is_User_Member (Header : Pid_Read) return Boolean;

   --  -- helpers the generated code names -----------------------------

   --  An Unbounded_String from a source literal.
   function To_Text (S : String) return Text;

   --  An Unbounded_String from UTF-8 octets -- how a generated program
   --  spells a value an Ada string literal cannot carry (GNAT reads an
   --  unmarked source as Latin-1).
   function From_Utf8 (Octets : Byte_Array) return Text;

   --  C11 7.24.4's unsigned-octet order over UTF-8, as the sign the
   --  shared walk compares.
   function Strcmp (A, B : Text) return Integer;

   --  How many octets the value is, which is what a string<N> bound
   --  counts.
   function Utf8_Octets (Value : Text) return Natural;

   --  How many UTF-16 code units, which is what a wstring<N> bound
   --  counts; a scalar above the basic plane is two.
   function Utf16_Units (Value : Text) return Natural;

   --  The code point as the one octet an IDL char is, refused above
   --  U+00FF rather than truncated.
   function Checked_Char (Point : Interfaces.Unsigned_32) return Interfaces.Unsigned_8;

   --  The code point as the one UTF-16 unit an IDL wchar is, refused
   --  outside the BMP. 7.2.2.2.1.2.5.
   function Checked_Wchar (Point : Interfaces.Unsigned_32) return Interfaces.Unsigned_16;

   --  The octet's two's-complement reading -- an Ada conversion checks
   --  the range C reinterprets, so the map-key order crosses here.
   function As_Signed (Value : Interfaces.Unsigned_8) return Interfaces.Integer_8;

   --  A value read for its effect and not kept: Ada has no `(void)`.
   procedure Discard_U32 (Value : Interfaces.Unsigned_32);

   --  The low Octets of Value's two's-complement bits, zero-extended --
   --  Write_Union_Overlay's pattern for an integer, character,
   --  enumeration or bitmask arm. The mask is the function: a plain
   --  widening of a negative value sign-extends.
   function Bit_Pattern_Int
     (Value : Interfaces.Integer_64; Octets : Natural) return Interfaces.Unsigned_64;

   --  One octet, 0 or 1 -- the boolean overlay pattern.
   function Bit_Pattern_Bool (Value : Boolean) return Interfaces.Unsigned_64;

   --  The IEEE 754 binary32 bits, zero-extended.
   function Bit_Pattern_F32 (Value : Interfaces.IEEE_Float_32) return Interfaces.Unsigned_64;

   --  The IEEE 754 binary64 bits.
   function Bit_Pattern_F64 (Value : Interfaces.IEEE_Float_64) return Interfaces.Unsigned_64;

   --  The three values a float literal cannot spell, built from their
   --  bit patterns: what a sample file's "NaN" and "inf" fill with.
   function F32_Nan return Interfaces.IEEE_Float_32;
   function F32_Inf return Interfaces.IEEE_Float_32;
   function F64_Nan return Interfaces.IEEE_Float_64;
   function F64_Inf return Interfaces.IEEE_Float_64;

   --  -- the writer ---------------------------------------------------

   --  Accumulates the bytes of one sample. Alignment is relative to the
   --  start of the stream: XCDR aligns each primitive to its own size
   --  counted from the encapsulation. XTypes 1.3 7.4.1.1.
   type Writer is record
      Buffer             : Byte_Array_Access := null;
      Count              : Natural           := 0;
      Order              : Byte_Order        := Little;
      Max_Align          : Natural           := 8;
      Origin             : Natural           := 0;
      Framing            : Pl_Cdr1_Framing;
      --  Whether an LC4 header whose member turns out to be 1, 2, 4 or
      --  8 bytes is rewritten with a short length code (Fast DDS).
      Short_Length_Codes : Boolean           := False;
   end record;

   function New_Writer
     (Order              : Byte_Order;
      Max_Align          : Natural;
      Framing            : Pl_Cdr1_Framing;
      Short_Length_Codes : Boolean) return Writer;

   --  The bytes written, as a fresh copy; empty when nothing was.
   function Finish (W : Writer) return Byte_Array_Access;

   procedure Write_Bool (W : in out Writer; Value : Boolean);
   procedure Write_U8 (W : in out Writer; Value : Interfaces.Unsigned_8);
   procedure Write_I8 (W : in out Writer; Value : Interfaces.Integer_8);
   --  A char: the code point's low octet -- which is the whole member.
   procedure Write_Char (W : in out Writer; Point : Interfaces.Unsigned_32);
   procedure Write_I16 (W : in out Writer; Value : Interfaces.Integer_16);
   procedure Write_U16 (W : in out Writer; Value : Interfaces.Unsigned_16);
   procedure Write_I32 (W : in out Writer; Value : Interfaces.Integer_32);
   procedure Write_U32 (W : in out Writer; Value : Interfaces.Unsigned_32);
   procedure Write_I64 (W : in out Writer; Value : Interfaces.Integer_64);
   procedure Write_U64 (W : in out Writer; Value : Interfaces.Unsigned_64);
   procedure Write_F32 (W : in out Writer; Value : Interfaces.IEEE_Float_32);
   procedure Write_F64 (W : in out Writer; Value : Interfaces.IEEE_Float_64);
   --  Raises Wire_Error by name: binary128 rests with the f128 arc.
   procedure Write_F128 (W : in out Writer; Value : Text);

   --  A wchar, at Width octets and in Framing's shape; every order but
   --  Unit_Stream means big-endian under the GIOP form.
   procedure Write_Wchar
     (W       : in out Writer;
      Point   : Interfaces.Unsigned_32;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order);

   --  A string: a four-byte length including the terminator, the UTF-8
   --  bytes, then a NUL. The length counts bytes. XTypes 1.3 7.4.1.1.
   procedure Write_String (W : in out Writer; Value : Text);

   --  A bounded string: the same bytes Write_String writes. The bound
   --  is a promise about the source and not a difference on the wire,
   --  so this is the only place it can be enforced -- refused rather
   --  than cut at a character boundary, which would put a sample on
   --  the wire nobody wrote. Raises Wire_Error for a value longer than
   --  Bound.
   procedure Write_String_Bounded (W : in out Writer; Value : Text; Bound : Natural);

   --  A wstring, in the form Policy and Order name. The value arrives
   --  as UTF-8 and the conversion to UTF-16 units happens here, which
   --  is what makes 7.4.1.1.2's surrogate rule this runtime's own
   --  reading.
   procedure Write_Wstring_Utf8
     (W : in out Writer; Value : Text; Policy : Wstring_Policy; Order : Unit_Order);

   --  A bounded wstring, as Write_String_Bounded is to Write_String.
   --  The bound counts UTF-16 code units, not characters and not
   --  octets, whatever Policy puts on the wire. Raises Wire_Error for
   --  a value holding more than Bound UTF-16 code units.
   procedure Write_Wstring_Bounded
     (W      : in out Writer;
      Value  : Text;
      Bound  : Natural;
      Policy : Wstring_Policy;
      Order  : Unit_Order);

   --  A wstring whose characters go out as their own UTF-8 octets.
   --
   --  WstringCodeUnits::Utf8 is the measured departure of one product's
   --  generated serialiser: XTypes 1.3 Table 31 gives a wstring's code
   --  unit as Char16 and 7.4.1.1.2 makes the length "twice the number of
   --  characters", so the clause has no UTF-8 form. The declared length
   --  is the UTF-8 octet count and only C2 -- the terminator -- still
   --  reaches the wire. wstring-member is 04 00 00 00 61 62 63 00 for
   --  "abc": three octets, a NUL, and the four declares both. Raises
   --  Wire_Error for a value that is not UTF-8.
   procedure Write_Wstring_Utf8_Octets
     (W : in out Writer; Value : Text; Policy : Wstring_Policy);

   --  A bounded UTF-8-octet wstring, as Write_Wstring_Bounded is to
   --  Write_Wstring_Utf8: the bound still counts UTF-16 code units,
   --  which is what every wstring<N> bound means, while the wire carries
   --  the UTF-8 octets. Raises Wire_Error for a value holding more than
   --  Bound UTF-16 code units.
   procedure Write_Wstring_Utf8_Octets_Bounded
     (W : in out Writer; Value : Text; Bound : Natural; Policy : Wstring_Policy);

   --  The count check every bounded sequence makes before a byte of it
   --  is written. Table 17 makes O2.length <= T1.length the condition
   --  for constructing a sequence<T, N>, and the wire carries no bound
   --  to compare against: a sender whose count already exceeds its
   --  own declaration has written a value of a type nobody declared.
   --  Raises Wire_Error for more than Bound elements.
   procedure Check_Sequence_Bound
     (W : in out Writer; Actual : Interfaces.Unsigned_32; Bound : Interfaces.Unsigned_32);

   --  The count check every bounded map makes before a byte of it is
   --  written. Its own message and not Check_Sequence_Bound's: the
   --  unit is an entry and not an element, per 7.4.1.1.4's
   --  map-as-a-sequence-of-pairs equivalence. Raises Wire_Error for
   --  more than Bound entries.
   procedure Check_Map_Bound
     (W : in out Writer; Actual : Interfaces.Unsigned_32; Bound : Interfaces.Unsigned_32);

   --  fixed<Digit_Count, Scale_Count> as packed BCD, [CDR] 9.3.2.8.
   procedure Write_Fixed
     (W           : in out Writer;
      Value       : Text;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count);

   --  An enumerator, as the integer its bit bound calls for. The holder
   --  is signed -- 7.4.3.5.3 rule (5).
   procedure Write_Enum (W : in out Writer; Bits : Natural; Value : Interfaces.Integer_64);

   --  A bitmask, as the unsigned integer Table 33 gives its bound.
   procedure Write_Bitmask (W : in out Writer; Bits : Natural; Value : Interfaces.Unsigned_64);

   --  One bitfield, shifted into the accumulator a bitset packs into; a
   --  value wider than the field is refused rather than masked.
   function Bitfield
     (W        : in out Writer;
      Value    : Interfaces.Unsigned_64;
      Width    : Natural;
      Position : Natural) return Interfaces.Unsigned_64;

   --  A bitset, as the anchor integer its bitfields pack into.
   procedure Write_Bitset (W : in out Writer; Anchor : Natural; Value : Interfaces.Unsigned_64);

   --  The Cyclone union overlay: the low Octets of Pattern, written as
   --  one unsigned integer of that width in the stream's byte order.
   procedure Write_Union_Overlay
     (W : in out Writer; Pattern : Interfaces.Unsigned_64; Octets : Natural);

   --  A DHEADER: a four-byte length placeholder, patched when the
   --  members are done.
   function Write_Dheader (W : in out Writer) return Dheader;
   procedure Finish_Dheader (W : in out Writer; Header : Dheader);

   --  An XCDR2 EMHEADER: id, length code and must-understand in one
   --  32-bit word. 7.4.3.4.2.
   function Write_Emheader
     (W               : in out Writer;
      Member_Id       : Natural;
      Length_Code     : Natural;
      Must_Understand : Boolean) return Emheader;
   procedure Finish_Emheader (W : in out Writer; Header : Emheader);

   --  A PL_CDR1 parameter header, short form: [id:u16][length:u16].
   function Write_Pid
     (W               : in out Writer;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write;

   --  The extended form: [PID_EXTENDED][8][id:u32][length:u32].
   function Write_Pid_Extended
     (W               : in out Writer;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write;

   procedure Finish_Pid (W : in out Writer; Header : Pid_Write);

   --  The PID_LIST_END sentinel a PL_CDR1 parameter list ends with.
   procedure Write_Sentinel (W : in out Writer);

   --  -- packed collection elements ----------------------------------

   --  ElementPadding::Packed is HDDS's measured departure from 7.4.1,
   --  which counts every element's alignment from the encapsulation:
   --  each collection element and each aggregate value counts its own
   --  alignment from its own first octet instead. array-1d-struct-final
   --  is 01000000 0200 03000000 0400 77, the second Cell's long at
   --  offset six where the clause puts it at eight, and typedef-of-struct
   --  is the same inside two @final members.
   --
   --  Begin_Packed_Element moves the origin to the current
   --  position and returns the origin it displaced; the generated codec
   --  holds that in a local and restores it with Finish_Packed_Element.
   function Begin_Packed_Element (W : in out Writer) return Natural;
   procedure Finish_Packed_Element (W : in out Writer; Saved : Natural);

   --  -- the reader ---------------------------------------------------

   --  Reads one sample's bytes: the mirror of the writer clause for
   --  clause. A malformed stream raises; a refusal about the type sets
   --  the status and the generated decoder walks on.
   type Reader is record
      Data      : Byte_Array_Access := null;
      At_Pos    : Natural           := 0;
      Order     : Byte_Order        := Little;
      Max_Align : Natural           := 8;
      Origin    : Natural           := 0;
      Framing   : Pl_Cdr1_Framing;
      Current   : Status            := Status_Ok;
   end record;

   function New_Reader
     (Data      : Byte_Array_Access;
      Order     : Byte_Order;
      Max_Align : Natural;
      Framing   : Pl_Cdr1_Framing) return Reader;

   function Reader_Status (R : in out Reader) return Status;
   --  The first refusal sticks, and Status_Ok refuses nothing.
   procedure Reader_Refuse (R : in out Reader; Refusal : Status);
   --  Take back a Status_Discarded, and only that one: a truncated
   --  stream is not a narrowing.
   function Reader_Clear_Discard (R : in out Reader) return Boolean;
   function Reader_Position (R : in out Reader) return Natural;
   function Reader_Remaining (R : in out Reader) return Natural;
   --  Forward only: a framing word pointing behind the cursor is a
   --  refusal, and past the end raises.
   procedure Reader_Skip_To (R : in out Reader; Target : Natural);
   function Reader_Save (R : in out Reader) return Reader_Mark;
   procedure Reader_Restore (R : in out Reader; Mark : Reader_Mark);

   --  The read-side pair of the writer's packed-element origin. See
   --  Begin_Packed_Element for the axis and the recorded rows.
   function Reader_Begin_Packed_Element (R : in out Reader) return Natural;
   procedure Reader_Finish_Packed_Element (R : in out Reader; Saved : Natural);

   --  A byte that is neither 0 nor 1 is refused rather than coerced.
   function Read_Bool (R : in out Reader) return Boolean;
   function Read_U8 (R : in out Reader) return Interfaces.Unsigned_8;
   function Read_I8 (R : in out Reader) return Interfaces.Integer_8;
   --  A char: one octet, answered as the code point.
   function Read_Char (R : in out Reader) return Interfaces.Unsigned_32;
   function Read_I16 (R : in out Reader) return Interfaces.Integer_16;
   function Read_U16 (R : in out Reader) return Interfaces.Unsigned_16;
   function Read_I32 (R : in out Reader) return Interfaces.Integer_32;
   function Read_U32 (R : in out Reader) return Interfaces.Unsigned_32;
   function Read_I64 (R : in out Reader) return Interfaces.Integer_64;
   function Read_U64 (R : in out Reader) return Interfaces.Unsigned_64;
   function Read_F32 (R : in out Reader) return Interfaces.IEEE_Float_32;
   function Read_F64 (R : in out Reader) return Interfaces.IEEE_Float_64;
   --  Raises Wire_Error by name: binary128 rests with the f128 arc.
   function Read_F128 (R : in out Reader) return Text;

   --  A wchar in Framing's shape; the four-byte form zero-extends one
   --  UTF-16 unit, and a value using the high bits, or a lone
   --  surrogate, is refused.
   function Read_Wchar
     (R       : in out Reader;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) return Interfaces.Unsigned_32;

   --  A string: a length including the terminator, the bytes, the NUL.
   function Read_String (R : in out Reader) return Text;
   --  Read_String under the name the shared walk calls it by: an
   --  Unbounded_String owns its storage, so the two names are one
   --  operation.
   function Read_String_Alloc (R : in out Reader) return Text;

   --  A wstring in the form Policy and Order name, answered as UTF-8;
   --  the form is never inferred from the stream.
   function Read_Wstring_Utf8
     (R : in out Reader; Policy : Wstring_Policy; Order : Unit_Order) return Text;

   --  The inverse of Write_Wstring_Utf8_Octets: the declared length is
   --  the UTF-8 octet count, the terminator Policy asks for is the last
   --  of those octets, and a count the stream cannot hold, a missing or
   --  an interior NUL, or an ill-formed UTF-8 sequence raises. See
   --  WstringCodeUnits::Utf8.
   function Read_Wstring_Utf8_Octets
     (R : in out Reader; Policy : Wstring_Policy) return Text;

   --  The decimal the writer took: leading zeros go, and a negative
   --  zero loses its sign.
   function Read_Fixed
     (R           : in out Reader;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Text;

   --  The trims a bound reaches for, as the shared walk calls them.
   function Trim_Utf8 (R : in out Reader; Value : Text; Octets : Natural) return Text;
   function Trim_Utf16 (R : in out Reader; Value : Text; Units : Natural) return Text;

   --  An enumerator's holder, as the signed integer 7.4.3.5.3 makes it.
   function Read_Enum (R : in out Reader; Bits : Natural) return Interfaces.Integer_64;

   --  A bitmask's holder. Table 33 of 7.4.1.1.3.2.
   function Read_Bitmask (R : in out Reader; Bits : Natural) return Interfaces.Unsigned_64;

   --  A bitset's anchor integer, of the width the planner sized.
   function Read_Bitset (R : in out Reader; Anchor : Natural) return Interfaces.Unsigned_64;

   --  A DHEADER: the length the members that follow occupy, refusing
   --  one the stream cannot hold.
   function Read_Dheader (R : in out Reader) return Interfaces.Unsigned_32;

   --  An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they read,
   --  but the reader is put back before it: that word is the member's
   --  own leading length. 7.4.3.4.2.
   function Read_Emheader (R : in out Reader) return Emheader_Read;

   --  A parameter header, and the value block it opens: 7.4.1.2 makes a
   --  parameter's value its own stream block.
   function Read_Pid (R : in out Reader) return Pid_Read;
   function Read_Pid_As (R : in out Reader; Role : Parameter_Role) return Pid_Read;

   --  End a parameter's value and step to the next header: the origin
   --  goes back, the declared length is measured against the value's
   --  start rather than believed, and the padding to four is stepped
   --  only under a framing whose writer wrote it (Pad_After_Value).
   procedure Finish_Pid_Read (R : in out Reader; Header : Pid_Read);

   --  -- the JSON the probes print ------------------------------------

   --  The dialect crates/wire-probe documents: a 64-bit integer is a
   --  string, an absent @optional member is a key left out. Not
   --  canonical form -- the caller canonicalises.

   function Json_Bool (Value : Boolean) return Text;
   function Json_Int (Value : Interfaces.Integer_64) return Text;
   --  Both 64-bit widths go out as decimal in a string, for every
   --  value: a JSON number is a double and loses the low bits past 2^53.
   function Json_I64 (Value : Interfaces.Integer_64) return Text;
   function Json_U64 (Value : Interfaces.Unsigned_64) return Text;
   --  Raises where the value has no JSON form (NaN, the infinities),
   --  naming Where.
   function Json_F64 (Value : Interfaces.IEEE_Float_64; Where : String) return Text;
   function Json_F32 (Value : Interfaces.IEEE_Float_32; Where : String) return Text;
   --  The one-character string the dialect writes for a character.
   function Json_Char (Point : Interfaces.Unsigned_32) return Text;
   --  The value quoted and escaped, UTF-8 passed through.
   function Json_String (Value : Text) return Text;
   --  Append `"Key": Value` to an object under construction, with the
   --  comma where one is due; Obj arrives holding at least `{`.
   procedure Json_Member (Obj : in out Text; Key : String; Value : Text);

end Zd_Wire;
