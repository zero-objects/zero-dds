--  SPDX-License-Identifier: Apache-2.0
--  Copyright 2026 ZeroDDS Contributors
--
--  The Ada 83 wire runtime: DDS-XTypes 1.3 7.4 from the clause, a
--  second time, over strict RM83 — no Interfaces, no modular type, no
--  Ada.Strings.Unbounded, no tagged type, no child unit, no
--  `raise … with "message"`.
--
--  The counterpart of runtime/ada/src/zd_wire.ads and, more directly,
--  of endpoints/ada-83/src/zerodds_ada83_wire.ads: that endpoint SDK
--  already solved the primitive layer of this problem — a 64-bit value
--  as a pair of Long_Integer halves, unsigned width checked rather than
--  represented, packed BCD for fixed<P,S> — and this package reuses its
--  answers rather than re-deriving them. What this package adds beyond
--  the endpoint SDK is the framing the shared codec walk drives through
--  `zerodds_lang_rules::emit::ada83`: DHEADER/EMHEADER with every length
--  code, the PL_CDR1 parameter list in its short and extended forms,
--  try-construct status, bitmask/bitset/union-overlay packing, and the
--  JSON the read-direction probe prints.
--
--  ## What Ada 83 costs this runtime, itemised
--
--  * **No Interfaces (Annex B is Ada 95).** Every integer-holding value
--    this package's public operations take is `Long_Integer`; there is
--    no `Unsigned_8`, no `Unsigned_32`. `unsigned long long`'s full
--    range needs 64 unsigned bits and no signed 64-bit `Long_Integer`
--    reaches past 2**63 - 1, so the top half of that range travels as
--    the two's-complement *pattern* — the negative `Long_Integer` whose
--    bits it is, exactly as a `uint64_t` sits in a signed register.
--    `Write_U64`/`Read_U64` take and answer that pattern and `Json_U64`
--    renders it unsigned again; `Write_U64_Pair`/`Read_U64_Pair` remain
--    for the internal callers that already hold two halves (the bitmask
--    and bitset 64-bit arms, the binary64 writer).
--  * **No Shift_Left/Shift_Right/and/or on an integer type.** Both come
--    from `Interfaces` or from a modular type's own operators, and this
--    runtime has neither. Every packing operation — a bitset's fields
--    into their anchor, a union overlay's pattern, an EMHEADER's four
--    top bits — is written in `+`, `*`, `/`, `mod` and `**`, RM83 4.5's
--    own arithmetic operators, over `Long_Integer`. Where the generated
--    code itself needs one of the four — the shared codec walk asks for
--    `&`, `|`, `~` and `>>` around a bitset and a bitmask — it reaches
--    `Bits_And`, `Bits_Or`, `Bits_Not`, `Shift_Right` and `Shift_Left`
--    below rather than an operator RM83 4.5.1 does not declare for an
--    integer type.
--  * **No `Ada.Strings.Unbounded`.** `Text` is an access to an
--    unconstrained `String` — the same shape a sequence already needs —
--    and every operation that would be a method on `Unbounded_String`
--    in `runtime/ada` is a function here that allocates a fresh one.
--  * **No `raise … with "message"` (RM83 11.3).** Every failure this
--    package can report is one of the exceptions below, by name, and
--    never with formatted text — the same answer
--    `endpoints/ada-83/src/zerodds_ada83_wire.ads` already gives, now
--    covering the framing that SDK does not need.
--  * **`long double` is still refused by name.** `Write_F128`/`Read_F128`
--    raise `Wire_Not_Implemented` unconditionally: binary128 rests with
--    the f128 arc in every column, `runtime/ada` included, and this is
--    not an Ada 83 gap.

package Zd_Wire83 is

   --  --  bytes and text  --------------------------------------------

   --  One storage octet, held as an unsigned pattern in an eight-bit
   --  representation (RM83 13.1's size clause on a range type, exactly
   --  as `endpoints/ada-83`'s `Byte` already is).
   type Byte is range 0 .. 255;
   for Byte'Size use 8;

   type Byte_Array is array (Natural range <>) of Byte;
   type Byte_Array_Access is access Byte_Array;

   --  An access to an unconstrained `String` — what a `string`, a
   --  `wstring`, a `fixed<P,S>` and a `long double` are all held in,
   --  since there is no `Unbounded_String` to reach for instead. Held
   --  as UTF-8, exactly as `runtime/ada`'s `Text` is: a `wstring`'s
   --  UTF-16 reading happens only at the wire boundary.
   type Text is access String;

   --  An empty `Text`, allocated rather than `null`: a decoder's result
   --  is always a real string, and `null` is reserved for "the read
   --  produced nothing", which never happens here — see
   --  `zerodds_lang_rules::emit::ada83`'s reading of
   --  `CodecTarget::materialised`.
   function Empty_Text return Text;

   --  A `Text` holding a source literal, unchanged.
   function Text_Of (S : String) return Text;

   --  A `Text` from UTF-8 octets — how a generated program spells a
   --  value an Ada string literal cannot carry (GNAT reads an unmarked
   --  source as Latin-1).
   function Text_From_Utf8 (Octets : Byte_Array) return Text;

   --  C11 7.24.4's unsigned-octet order over UTF-8, as the sign the
   --  shared walk compares.
   function Strcmp (A, B : Text) return Integer;

   --  How many UTF-8 octets the value is.
   function Utf8_Octets (Value : Text) return Natural;

   --  How many UTF-16 code units the value is; a scalar above the basic
   --  plane is two.
   function Utf16_Units (Value : Text) return Natural;

   --  --  exceptions  ---------------------------------------------------

   --  A value the wire width cannot hold — an ordinary width overflow,
   --  or the top half of an `unsigned long long`'s range, which no
   --  `Long_Integer` this compiler builds reaches past (see the package
   --  documentation).
   Value_Out_Of_Range : exception;

   --  A code point a `char` or a `wchar` cannot carry.
   Code_Point_Out_Of_Range : exception;

   --  A member id past an EMHEADER's 28 bits, or past a short parameter
   --  header's range.
   Member_Id_Out_Of_Range : exception;

   --  A member body whose octet count is not the one its length code
   --  frames.
   Length_Code_Mismatch : exception;

   --  A `bitset` anchor, or a union overlay, that is not one, two, four
   --  or eight octets.
   Anchor_Not_A_Holder_Width : exception;

   --  A decimal a `fixed<P,S>` cannot hold.
   Not_A_Fixed_Value : exception;

   --  A parameter value longer than a short header can describe.
   Parameter_Too_Long : exception;

   --  A bit position or width outside its bitmask's or bitset's bound.
   Bit_Out_Of_Range : exception;

   --  The stream ended where a value was expected.
   Stream_Truncated : exception;

   --  A stream that is not well-formed independent of length — a UTF-8
   --  sequence that does not decode, a lone UTF-16 surrogate, a
   --  parameter marker naming a length the framing does not admit.
   Malformed_Stream : exception;

   --  binary128 rests with the f128 arc; see the package documentation.
   Wire_Not_Implemented : exception;

   --  --  policy enumerations  ------------------------------------------

   type Byte_Order is (Little, Big);

   --  DDS-XTypes 1.3 7.2.2.7, Table 11.
   type Try_Construct is (Tc_Discard, Tc_Use_Default, Tc_Trim);

   type Status is
     (Status_Ok,
      Status_Truncated,
      Status_Impossible_Length,
      Status_Out_Of_Range,
      Status_Discarded);

   function Status_Text (S : Status) return String;

   type Parameter_Role is (Role_Member, Role_Optional);

   type Pid_Flag_Policy is
     (Flags_Sentinel_Must_Understand, Flags_Bare, Flags_Impl_Extension_Members);

   type Wchar_Framing is (Wchar_Fixed, Wchar_Giop_Length_Prefixed);

   type Unit_Order is
     (Unit_Stream, Unit_Big_Endian_Always, Unit_Byte_Order_Mark, Unit_Host_Unswapped);

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

   type Pl_Cdr1_Framing is record
      Pid_Flags                    : Pid_Flag_Policy := Flags_Sentinel_Must_Understand;
      Highest_Short_Id             : Natural         := 16#3F00#;
      Pad_Declared_Length          : Boolean         := False;
      Pad_Declared_Optional_Length : Boolean         := False;
      Member_Must_Understand       : Boolean         := True;
      Pad_After_Value              : Boolean         := True;
   end record;

   --  --  handles  --------------------------------------------------

   type Dheader is record
      At_Pos : Integer := -1;
   end record;

   function No_Dheader return Dheader;

   type Emheader is record
      Next_Int    : Integer := -1;
      Word        : Natural := 0;
      Length_Code : Natural := 8;
   end record;

   type Pid_Write is record
      At_Pos          : Natural        := 0;
      Extended        : Boolean        := False;
      Saved_Origin    : Natural        := 0;
      Role            : Parameter_Role := Role_Member;
      Pid             : Natural        := 0;
      Must_Understand : Boolean        := False;
   end record;

   type Reader_Mark is record
      At_Pos  : Natural := 0;
      Origin  : Natural := 0;
      Current : Status  := Status_Ok;
   end record;

   type Emheader_Read is record
      Member_Id       : Long_Integer := 0;
      Length_Code     : Natural      := 0;
      Must_Understand : Boolean      := False;
      Next_Int        : Long_Integer := 0;
      Has_Next_Int    : Boolean      := False;
      Extent          : Natural      := 0;
   end record;

   type Pid_Read is record
      Pid             : Long_Integer   := 0;
      Length          : Long_Integer   := 0;
      End_Flag        : Boolean        := False;
      Must_Understand : Boolean        := False;
      Impl_Extension  : Boolean        := False;
      Reserved        : Boolean        := False;
      Ignore          : Boolean        := False;
      Role            : Parameter_Role := Role_Member;
      Saved_Origin    : Natural        := 0;
      Value_At        : Natural        := 0;
   end record;

   function Pid_Is_User_Member (Header : Pid_Read) return Boolean;

   --  --  helpers the generated code names  -----------------------------

   --  A char's code point, refused above U+00FF rather than truncated.
   function Checked_Char (Point : Long_Integer) return Long_Integer;

   --  A wchar's code point, refused outside the BMP. 7.2.2.2.1.2.5.
   function Checked_Wchar (Point : Long_Integer) return Long_Integer;

   --  The octet's two's-complement reading as a `signed char` — the
   --  seam an Ada conversion cannot cross where the held value is
   --  already the unsigned octet pattern. `As_Signed (200) = -56`.
   function As_Signed (Value : Long_Integer) return Long_Integer;

   --  Whether Left is below Right read as *unsigned* 64-bit values.
   --
   --  An `unsigned long long` is held in a `Long_Integer` as its
   --  two's-complement pattern, so a value at or above 2**63 is the
   --  negative `Long_Integer` carrying those bits -- the same carriage
   --  `Write_U64` gives it. RM83 has no modular type to borrow the
   --  order from (Ada 95's `Interfaces.Unsigned_64` is what
   --  `runtime/ada` uses), and the predefined `<` is signed: it puts
   --  16045690984833335998 ahead of 7. This is the unsigned reading --
   --  a negative pattern is the larger value, and two patterns of the
   --  same sign compare as they are.
   --
   --  The `map` ascending-key order is the caller: DDS-XTypes 1.3
   --  7.4.1.4.4 rules (14) to (16) give a map as a length and a run of
   --  pairs and say nothing about their order, and the two
   --  implementations that write one (Fast DDS, OpenDDS) write it by
   --  the key's own value -- unsigned, for an unsigned key type.
   function U64_Less (Left, Right : Long_Integer) return Boolean;

   --  A value read for its effect and not kept: Ada has no `(void)`.
   procedure Discard (Value : Long_Integer);

   --  --  the bitwise operators RM83 has no operator for  ---------------

   --  RM83 4.5.1 declares `and`, `or`, `xor` and `not` for `Boolean`
   --  and for a one-dimensional array of `Boolean`, and for nothing
   --  else: an integer type carries none of them, and Ada 83 has no
   --  `Interfaces` to borrow an intrinsic from. The shared codec walk
   --  still asks for a mask, a join and a shift — a bitset's fields
   --  into their anchor and back out, a bitmask's "no bit outside the
   --  declared set" test — so the five below answer them in RM83 4.5's
   --  `+`, `*`, `/` and `mod`.
   --
   --  Every one of them reads its arguments and answers as the 64-bit
   --  two's-complement *pattern*, which is what a `Long_Integer`
   --  already is for either sign: bit 63 set is a negative value, the
   --  same carriage `Write_U64` gives an `unsigned long long` above
   --  2**63 - 1. So `Bits_Not (0) = -1`, all sixty-four bits set, and
   --  not an overflow.
   function Bits_And (Left, Right : Long_Integer) return Long_Integer;
   function Bits_Or (Left, Right : Long_Integer) return Long_Integer;
   function Bits_Xor (Left, Right : Long_Integer) return Long_Integer;
   function Bits_Not (Value : Long_Integer) return Long_Integer;
   --  Logical, not arithmetic: bit 63 shifts out and zeros come in at
   --  the top, because the value is a pattern and not a magnitude. A
   --  `Distance` of 64 or more answers zero rather than raising.
   function Shift_Right (Value : Long_Integer; Distance : Natural) return Long_Integer;
   function Shift_Left (Value : Long_Integer; Distance : Natural) return Long_Integer;

   --  Whether every set bit of `Value` is also set in `Mask` — the
   --  bitmask-dump probe's "no bit outside the declared set" check.
   --  Both arguments are the 64-bit pattern, so a bitmask declaring a
   --  bit at position 63 states its mask as a negative `Long_Integer`
   --  and a value carrying that bit is a subset of it.
   function Bits_Subset (Value, Mask : Long_Integer) return Boolean;

   --  The low `Octets` of `Value`'s two's-complement bits, zero-extended
   --  — `Write_Union_Overlay`'s pattern for an integer, character,
   --  enumeration or bitmask arm.
   function Bit_Pattern_Int (Value : Long_Integer; Octets : Natural) return Long_Integer;
   function Bit_Pattern_Bool (Value : Boolean) return Long_Integer;
   function Bit_Pattern_F32 (Value : Float) return Long_Integer;
   --  A binary64 pattern needs 64 unsigned bits, which is the same
   --  problem an `unsigned long long` has; answered here as a Hi/Lo
   --  pair, because both halves are wanted separately.
   procedure Bit_Pattern_F64 (Value : Long_Float; Hi, Lo : out Long_Integer);

   --  The same pattern as one `Long_Integer`, for the one caller that
   --  has nowhere to put a Hi/Lo pair: the Cyclone union overlay's
   --  `Pattern` argument is a single value in every column. A `double`
   --  arm whose pattern needs bit 63 — any negative value, or negative
   --  zero — comes back as the negative `Long_Integer` holding those
   --  bits, exactly as `Write_U64`'s argument does, and
   --  `Write_Union_Overlay` splits it apart again.
   function Bit_Pattern_F64_Overlay (Value : Long_Float) return Long_Integer;

   function F32_Nan return Float;
   function F32_Inf return Float;
   function F64_Nan return Long_Float;
   function F64_Inf return Long_Float;

   --  --  the writer  ---------------------------------------------------

   type Writer is record
      Buffer             : Byte_Array_Access := null;
      Count              : Natural           := 0;
      Order              : Byte_Order        := Little;
      Max_Align          : Natural           := 8;
      Origin             : Natural           := 0;
      Framing            : Pl_Cdr1_Framing;
      Short_Length_Codes : Boolean           := False;
   end record;

   --  Every subprogram below takes the writer by access rather than as
   --  `in out`. RM83 6.1 admits only `in` parameters in a function, and
   --  `Write_Dheader` and its three siblings answer a value while moving
   --  the writer on. An access parameter is `in` — what it designates
   --  changes, the parameter itself does not — which is how Ada 83
   --  states this and what `runtime/ada` gets from Ada 2012's `in out`
   --  functions instead.
   type Writer_Access is access Writer;

   function New_Writer
     (Order              : Byte_Order;
      Max_Align          : Natural;
      Framing            : Pl_Cdr1_Framing;
      Short_Length_Codes : Boolean) return Writer_Access;

   function Finish (W : Writer_Access) return Byte_Array_Access;

   procedure Write_Bool (W : Writer_Access; Value : Boolean);
   --  Every integer width, as the range only: `runtime/ada`'s eight
   --  `Write_I8`/`Write_U8`/… become the four below, checked against
   --  the extent `Width` names — 1, 2, 4 or 8 octets — and each takes
   --  the unsigned reading of the value, exactly as
   --  `endpoints/ada-83`'s `Put_U8`/`Put_U16`/`Put_U32` already do; the
   --  admitted range is `Low .. High`, and a `long`'s -2**31 differs
   --  from an `unsigned long`'s 0, which only the caller knows.
   procedure Write_Int
     (W : Writer_Access; Value : Long_Integer; Width : Natural; Low, High : Long_Integer);
   --  `unsigned long long`'s full range as two halves, each already the
   --  unsigned pattern — the shape every internal caller that needs
   --  the top bit takes (the bitmask and bitset 64-bit arms, the
   --  binary64 writer). Raises `Value_Out_Of_Range` for a half outside
   --  0 .. 2**32 - 1.
   procedure Write_U64_Pair (W : Writer_Access; Hi, Lo : Long_Integer);
   --  `unsigned long long` as one `Long_Integer` — the calling
   --  convention every other scalar writer takes, and what the shared
   --  codec walk calls for an ordinary member. The argument is the
   --  64-bit *pattern*, not a value in 0 .. 2**64 - 1: the top half of
   --  the range needs `Long_Integer`'s own sign bit, so a wire value of
   --  2**63 or more arrives as the negative `Long_Integer` whose bits
   --  it is, exactly as a `uint64_t` sits in a signed register. Every
   --  `Long_Integer` is a well-formed pattern, so nothing is refused
   --  here; whether the sample was an `unsigned long long` at all is
   --  the materialiser's question and is answered before the program
   --  is generated.
   procedure Write_U64 (W : Writer_Access; Value : Long_Integer);
   --  A signed 64-bit value: the whole range fits one `Long_Integer`.
   procedure Write_I64 (W : Writer_Access; Value : Long_Integer);
   procedure Write_F32 (W : Writer_Access; Value : Float);
   procedure Write_F64 (W : Writer_Access; Value : Long_Float);
   procedure Write_F128 (W : Writer_Access; Value : Text);
   procedure Write_Char (W : Writer_Access; Point : Long_Integer);
   procedure Write_Wchar
     (W       : Writer_Access;
      Point   : Long_Integer;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order);
   procedure Write_String (W : Writer_Access; Value : Text);

   --  A bounded string: the same bytes Write_String writes. The bound
   --  is a promise about the source and not a difference on the wire,
   --  so this is the only place it can be enforced -- refused rather
   --  than cut at a character boundary, which would put a sample on
   --  the wire nobody wrote. Raises Value_Out_Of_Range for a value
   --  longer than Bound.
   procedure Write_String_Bounded (W : Writer_Access; Value : Text; Bound : Natural);

   procedure Write_Wstring_Utf8
     (W : Writer_Access; Value : Text; Policy : Wstring_Policy; Order : Unit_Order);

   --  A bounded wstring, as Write_String_Bounded is to Write_String.
   --  The bound counts UTF-16 code units, not characters and not
   --  octets, whatever Policy puts on the wire. Raises
   --  Value_Out_Of_Range for a value holding more than Bound UTF-16
   --  code units.
   procedure Write_Wstring_Bounded
     (W      : Writer_Access;
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
   --  Malformed_Stream for a value that is not UTF-8.
   procedure Write_Wstring_Utf8_Octets
     (W : Writer_Access; Value : Text; Policy : Wstring_Policy);

   --  A bounded UTF-8-octet wstring, as Write_Wstring_Bounded is to
   --  Write_Wstring_Utf8: the bound still counts UTF-16 code units,
   --  which is what every wstring<N> bound means, while the wire carries
   --  the UTF-8 octets. Raises Value_Out_Of_Range for a value holding
   --  more than Bound UTF-16 code units.
   procedure Write_Wstring_Utf8_Octets_Bounded
     (W : Writer_Access; Value : Text; Bound : Natural; Policy : Wstring_Policy);

   --  The count check every bounded sequence makes before a byte of it
   --  is written. Table 17 makes O2.length <= T1.length the condition
   --  for constructing a sequence<T, N>, and the wire carries no bound
   --  to compare against: a sender whose count already exceeds its
   --  own declaration has written a value of a type nobody declared.
   --  Raises Value_Out_Of_Range for more than Bound elements.
   procedure Check_Sequence_Bound
     (W : Writer_Access; Actual : Long_Integer; Bound : Long_Integer);

   --  The count check every bounded map makes before a byte of it is
   --  written. The unit is an entry and not an element, per 7.4.1.1.4's
   --  map-as-a-sequence-of-pairs equivalence. Raises
   --  Value_Out_Of_Range for more than Bound entries.
   procedure Check_Map_Bound
     (W : Writer_Access; Actual : Long_Integer; Bound : Long_Integer);
   procedure Write_Fixed
     (W           : Writer_Access;
      Value       : Text;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count);

   --  An enumerator, as the signed holder 7.4.3.5.3 rule (5) gives its
   --  `@bit_bound`. `Value` is the wire code directly — this column
   --  declares no enumeration type to read it from
   --  (`zerodds_lang_rules::adapter::ada83`).
   procedure Write_Enum (W : Writer_Access; Bits : Natural; Value : Long_Integer);
   --  A bitmask, as the unsigned integer Table 33 gives its bound. A
   --  64-bit bitmask's value travels as the pattern `Write_U64` takes,
   --  so a `@position(63)` bit is a negative `Long_Integer` here and
   --  is written rather than refused. A narrower bound is checked
   --  against `2 ** Bits`.
   procedure Write_Bitmask (W : Writer_Access; Bits : Natural; Value : Long_Integer);
   --  One bitfield, shifted into the accumulator a bitset packs into.
   --  `Position + Width` may reach 64: the accumulator is the 64-bit
   --  pattern, so a field ending on bit 63 comes back negative rather
   --  than being refused (DDS-XTypes 1.3 7.4.1.1.3.3 sizes the anchor
   --  over the whole chain, and `bitset-multiword`'s is eight octets).
   function Bitfield
     (Value : Long_Integer; Width : Natural; Position : Natural) return Long_Integer;
   procedure Write_Bitset (W : Writer_Access; Anchor : Natural; Value : Long_Integer);
   procedure Write_Union_Overlay (W : Writer_Access; Pattern : Long_Integer; Octets : Natural);

   function Write_Dheader (W : Writer_Access) return Dheader;
   procedure Finish_Dheader (W : Writer_Access; Header : Dheader);

   function Write_Emheader
     (W               : Writer_Access;
      Member_Id       : Natural;
      Length_Code     : Natural;
      Must_Understand : Boolean) return Emheader;
   procedure Finish_Emheader (W : Writer_Access; Header : Emheader);

   function Write_Pid
     (W               : Writer_Access;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write;
   function Write_Pid_Extended
     (W               : Writer_Access;
      Pid             : Natural;
      Role            : Parameter_Role;
      Must_Understand : Boolean) return Pid_Write;
   procedure Finish_Pid (W : Writer_Access; Header : Pid_Write);
   procedure Write_Sentinel (W : Writer_Access);

   --  --  packed collection elements  ----------------------------------

   --  ElementPadding::Packed is HDDS's measured departure from 7.4.1,
   --  which counts every element's alignment from the encapsulation:
   --  each collection element and each aggregate value counts its own
   --  alignment from its own first octet instead. array-1d-struct-final
   --  is 01000000 0200 03000000 0400 77, the second Cell's long at
   --  offset six where the clause puts it at eight, and typedef-of-struct
   --  is the same inside two @final members.
   --
   --  Begin_Packed_Element moves the origin to the current position and
   --  returns the origin it displaced; the generated codec holds that in
   --  a local and restores it with Finish_Packed_Element.
   function Begin_Packed_Element (W : Writer_Access) return Natural;
   procedure Finish_Packed_Element (W : Writer_Access; Saved : Natural);

   --  --  the reader  ---------------------------------------------------

   type Reader is record
      Data      : Byte_Array_Access := null;
      At_Pos    : Natural           := 0;
      Order     : Byte_Order        := Little;
      Max_Align : Natural           := 8;
      Origin    : Natural           := 0;
      Framing   : Pl_Cdr1_Framing;
      Current   : Status            := Status_Ok;
   end record;

   --  By access, for the reason given at `Writer_Access`: nineteen of
   --  the readers below are functions that move the reader on.
   type Reader_Access is access Reader;

   function New_Reader
     (Data      : Byte_Array_Access;
      Order     : Byte_Order;
      Max_Align : Natural;
      Framing   : Pl_Cdr1_Framing) return Reader_Access;

   function Reader_Status (R : Reader_Access) return Status;
   procedure Reader_Refuse (R : Reader_Access; Refusal : Status);
   function Reader_Clear_Discard (R : Reader_Access) return Boolean;
   function Reader_Position (R : Reader_Access) return Natural;
   function Reader_Remaining (R : Reader_Access) return Natural;
   procedure Reader_Skip_To (R : Reader_Access; Target : Natural);
   function Reader_Save (R : Reader_Access) return Reader_Mark;
   procedure Reader_Restore (R : Reader_Access; Mark : Reader_Mark);

   --  The read-side pair of the writer's packed-element origin. See
   --  Begin_Packed_Element for the axis and the recorded rows.
   function Reader_Begin_Packed_Element (R : Reader_Access) return Natural;
   procedure Reader_Finish_Packed_Element (R : Reader_Access; Saved : Natural);

   function Read_Bool (R : Reader_Access) return Boolean;
   --  The unsigned reading of `Width` octets, zero-extended into
   --  `Long_Integer`.
   function Read_Uint (R : Reader_Access; Width : Natural) return Long_Integer;
   --  The signed reading — the two's-complement decode `Long_Integer`
   --  itself cannot do for anything but the widest width, so this
   --  answers it by hand for 1, 2 and 4 octets.
   function Read_Int (R : Reader_Access; Width : Natural) return Long_Integer;
   function Read_Char (R : Reader_Access) return Long_Integer;
   --  The Hi/Lo pair, for the internal callers that need the top bit
   --  (the bitmask and bitset 64-bit arms).
   procedure Read_U64_Pair (R : Reader_Access; Hi, Lo : out Long_Integer);
   --  `unsigned long long` as one `Long_Integer`, the 64-bit pattern
   --  kept whole: a wire value of 2**63 or more comes back negative,
   --  and `Json_U64` renders it unsigned again. See `Write_U64`.
   function Read_U64 (R : Reader_Access) return Long_Integer;
   function Read_I64 (R : Reader_Access) return Long_Integer;
   function Read_F32 (R : Reader_Access) return Float;
   function Read_F64 (R : Reader_Access) return Long_Float;
   function Read_F128 (R : Reader_Access) return Text;
   function Read_Wchar
     (R       : Reader_Access;
      Width   : Natural;
      Framing : Wchar_Framing;
      Order   : Unit_Order) return Long_Integer;
   function Read_String (R : Reader_Access) return Text;
   function Read_String_Alloc (R : Reader_Access) return Text;
   function Read_Wstring_Utf8
     (R : Reader_Access; Policy : Wstring_Policy; Order : Unit_Order) return Text;

   --  The inverse of Write_Wstring_Utf8_Octets: the declared length is
   --  the UTF-8 octet count, the terminator Policy asks for is the last
   --  of those octets, and a count the stream cannot hold, a missing or
   --  an interior NUL, or an ill-formed UTF-8 sequence raises. See
   --  WstringCodeUnits::Utf8.
   function Read_Wstring_Utf8_Octets
     (R : Reader_Access; Policy : Wstring_Policy) return Text;
   function Read_Fixed
     (R           : Reader_Access;
      Digit_Count : Natural;
      Scale_Count : Natural;
      Count       : Fixed_Count) return Text;
   function Trim_Utf8 (Value : Text; Octets : Natural) return Text;
   function Trim_Utf16 (Value : Text; Units : Natural) return Text;
   function Read_Enum (R : Reader_Access; Bits : Natural) return Long_Integer;
   function Read_Bitmask (R : Reader_Access; Bits : Natural) return Long_Integer;
   function Read_Bitset (R : Reader_Access; Anchor : Natural) return Long_Integer;
   function Read_Dheader (R : Reader_Access) return Long_Integer;
   function Read_Emheader (R : Reader_Access) return Emheader_Read;
   function Read_Pid (R : Reader_Access) return Pid_Read;
   function Read_Pid_As (R : Reader_Access; Role : Parameter_Role) return Pid_Read;
   procedure Finish_Pid_Read (R : Reader_Access; Header : Pid_Read);

   function Padding_For (Pos : Integer; Align : Integer) return Integer;

   --  --  the JSON the probes print  -------------------------------------

   function Json_Bool (Value : Boolean) return Text;
   function Json_Int (Value : Long_Integer) return Text;
   --  `Read_I64`'s answer, quoted for the same reason `Json_U64`'s is:
   --  the dialect `crates/wire-probe` documents puts *both* 64-bit
   --  widths in a string, because a JSON number is a double and loses
   --  the low bits past 2**53. `Json_Int` is the unquoted form every
   --  narrower width takes.
   function Json_I64 (Value : Long_Integer) return Text;
   --  `Read_U64`'s answer as an unsigned decimal, quoted rather than a
   --  bare number: the shared dialect's own reading of every 64-bit
   --  width — a JSON number is a double and loses the low bits past
   --  2**53. A negative argument is the pattern of a wire value at or
   --  above 2**63 and renders as that value, not with a minus sign.
   function Json_U64 (Value : Long_Integer) return Text;
   function Json_F64 (Value : Long_Float; Where : String) return Text;
   function Json_F32 (Value : Float; Where : String) return Text;
   function Json_Char (Point : Long_Integer) return Text;
   function Json_String (Value : Text) return Text;
   procedure Json_Append (Obj : in out Text; More : String);
   procedure Json_Member (Obj : in out Text; Key : String; Value : Text);

end Zd_Wire83;
