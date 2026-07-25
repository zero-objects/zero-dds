-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Round-trip test for the pure-Ada reader: encode the @final sample, decode it
--  back through the native Reader, and verify every field survives in both LE
--  and BE wire order.

with Ada.Text_IO;         use Ada.Text_IO;
with Ada.Command_Line;    use Ada.Command_Line;
with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;
with Native_Samples;      use Native_Samples;

procedure Test_Native_Roundtrip is

   function Roundtrip (Endian : Endianness; Tag : String) return Boolean is
      Enc : constant Byte_Array := Encode_Final (Endian);
      R   : Reader;

      Id    : Unsigned_32;
      Kind  : Unsigned_16;
      Flags : Byte;
      Value : IEEE_Float_32;
      Stamp : Unsigned_64;
      Label : String (1 .. 64);
      Last  : Natural;
      Raw   : Byte_Array (0 .. 63);
      N     : Natural;
   begin
      Init (R, Enc, Endian);
      Get_U32 (R, Id);
      Get_U16 (R, Kind);
      Get_U8 (R, Flags);
      Get_F32 (R, Value);
      Get_U64 (R, Stamp);
      Get_String (R, Label, Last);
      Get_Seq_U8 (R, Raw, N);

      if R.Underflow then
         Put_Line (Tag & ": reader underflow");
         return False;
      end if;
      if Id /= 16#A1B2_C3D4# or else Kind /= 16#1234#
        or else Flags /= 16#5A# or else Value /= 3.5
        or else Stamp /= 16#0102_0304_0506_0708#
        or else Label (Label'First .. Last) /= "bay-12"
        or else N /= 4
        or else Raw (0) /= 16#DE# or else Raw (3) /= 16#EF#
      then
         Put_Line (Tag & ": field mismatch after round-trip");
         return False;
      end if;
      Put_Line (Tag & ": round-trip decode ok");
      return True;
   end Roundtrip;

   Ok : Boolean := True;

begin
   Ok := Roundtrip (Little, "LE") and then Ok;
   Ok := Roundtrip (Big, "BE") and then Ok;
   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Native_Roundtrip;
