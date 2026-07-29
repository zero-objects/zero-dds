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

package Zerodds_Ada83_Wire is

   type Byte is range 0 .. 255;
   for Byte'Size use 8;  -- one storage byte: Sequential_IO reads 1 file byte per
                         -- element, and Unchecked_Conversion sizes line up.
   type Byte_Array is array (Integer range <>) of Byte;
   pragma Pack (Byte_Array);
   type Endianness is (Little, Big);

   Max_Buffer : constant := 2048;

   type Writer is record
      Buf      : Byte_Array (0 .. Max_Buffer - 1);
      Len      : Integer := 0;
      Endian   : Endianness := Little;
      Overflow : Boolean := False;
   end record;

   procedure Init (W : in out Writer; Endian : Endianness);

   --  Aligned primitives. Values are passed as Long_Integer (Ada 83 has no
   --  unsigned type); the caller supplies the numeric value.
   procedure Put_U8  (W : in out Writer; V : Long_Integer);
   procedure Put_U16 (W : in out Writer; V : Long_Integer);
   procedure Put_U32 (W : in out Writer; V : Long_Integer);
   --  uint64 as two 32-bit halves (like the C wire-core's zdw_u64_t).
   procedure Put_U64 (W : in out Writer; Hi, Lo : Long_Integer);
   procedure Put_F32 (W : in out Writer; V : Float);
   procedure Put_String (W : in out Writer; S : String);
   procedure Put_Seq_U8 (W : in out Writer; Data : Byte_Array);
   procedure Put_Bytes (W : in out Writer; Data : Byte_Array);

   function Length (W : Writer) return Integer;

   --  Minimal reader (enough to decode a received sample's leading fields).
   type Reader is record
      Data   : Byte_Array (0 .. Max_Buffer - 1);
      Len    : Integer := 0;
      Pos    : Integer := 0;
      Endian : Endianness := Little;
   end record;

   procedure Init_Reader (R : out Reader; Data : Byte_Array; Endian : Endianness);
   procedure Get_U32 (R : in out Reader; V : out Long_Integer);

end Zerodds_Ada83_Wire;
