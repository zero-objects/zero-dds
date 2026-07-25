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

   type Endianness is (Little, Big);

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

   --  --- reader over a bounded, self-owned buffer ---

   type Reader is limited record
      Buf       : Byte_Array (0 .. Max_Buffer - 1) := [others => 0];
      Len       : Natural   := 0;
      Pos       : Natural   := 0;
      Endian    : Endianness := Little;
      Max_Align : Positive  := Xcdr2_Max_Align;
      Underflow : Boolean   := False;
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

   procedure DHeader_Read (R : in out Reader; Len : out Unsigned_32);
   procedure EMHeader_Read
     (R               : in out Reader;
      Member_Id       : out Unsigned_32;
      Must_Understand : out Boolean;
      Nextint         : out Unsigned_32);

   --  Padding to push `Pos` to the next multiple of `Align` (power of two).
   function Padding_For (Pos : Natural; Align : Positive) return Natural
     with Post => Padding_For'Result < Align;

end Zerodds_Native_Wire;
