-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Interfaces; use Interfaces;

package body Native_Samples is

   function Encode_Final (Endian : Endianness) return Byte_Array is
      W : Writer;
   begin
      Init (W, Endian);
      Put_U32 (W, 16#A1B2_C3D4#);
      Put_U16 (W, 16#1234#);
      Put_U8 (W, 16#5A#);
      Put_F32 (W, 3.5);
      Put_U64 (W, 16#0102_0304_0506_0708#);
      Put_String (W, "bay-12");
      Put_Seq_U8 (W, [16#DE#, 16#AD#, 16#BE#, 16#EF#]);
      return Bytes (W);
   end Encode_Final;

   --  Inner { uint16 a; uint32 b } as an appendable (DHEADER + tight body).
   procedure Encode_Inner (W : in out Writer; A : Unsigned_16; B : Unsigned_32) is
      Body_Start : Natural;
   begin
      DHeader_Begin (W, Body_Start);
      Put_U16 (W, A);
      Put_U32 (W, B);
      DHeader_End (W, Body_Start);
   end Encode_Inner;

   function Encode_Nested (Endian : Endianness) return Byte_Array is
      W          : Writer;
      Outer_Body : Natural;
   begin
      Init (W, Endian);
      DHeader_Begin (W, Outer_Body);
      Put_U32 (W, 16#CAFE_BABE#);
      Encode_Inner (W, 16#1111#, 16#2222_3333#);

      --  sequence<Inner>: non-primitive element -> collection DHEADER wrapping
      --  (uint32 count + each element), built in a sub-writer whose origin is
      --  4-aligned in the parent, then a u32 body length + the bytes.
      declare
         Sub : Writer;
      begin
         Init (Sub, Endian);
         Put_U32 (Sub, 2);
         Encode_Inner (Sub, 16#AAAA#, 16#BBBB_CCCC#);
         Encode_Inner (Sub, 16#DDDD#, 16#EEEE_FFFF#);
         Put_U32 (W, Unsigned_32 (Sub.Len));
         Put_Bytes (W, Bytes (Sub));
      end;

      Put_String (W, "nested");
      DHeader_End (W, Outer_Body);
      return Bytes (W);
   end Encode_Nested;

   function Encode_Mutable (Endian : Endianness) return Byte_Array is
      W          : Writer;
      Frame      : Natural;
      Member     : Natural;
   begin
      Init (W, Endian);
      DHeader_Begin (W, Frame);

      EMHeader_Begin (W, 10, False, Member);
      Put_U32 (W, 16#DEAD_BEEF#);
      EMHeader_End (W, Member);

      EMHeader_Begin (W, 20, False, Member);
      Put_String (W, "mut");
      EMHeader_End (W, Member);

      EMHeader_Begin (W, 30, False, Member);
      Put_U16 (W, 16#0777#);
      EMHeader_End (W, Member);

      DHeader_End (W, Frame);
      return Bytes (W);
   end Encode_Mutable;

end Native_Samples;
