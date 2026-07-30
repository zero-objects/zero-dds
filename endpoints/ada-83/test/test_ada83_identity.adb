-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Byte-identity test for the pre-Object Ada 83 wire-core. Encodes the @final
--  SensorReading in LE and BE and compares to the Rust goldens (build/
--  golden_le.bin / golden_be.bin). Strict Ada 83: no Command_Line, so the
--  golden paths are fixed. Proves the Ada-83 wire is byte-identical.

with Text_IO;
with Sequential_IO;
with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;

procedure Test_Ada83_Identity is

   package Byte_IO is new Sequential_IO (Byte);

   Ok_All : Boolean := True;

   --  Fixture: MUST match endpoints/golden-gen (the @final vector).
   procedure Fixture (W : in out Writer) is
      Raw : Byte_Array (0 .. 3);
   begin
      Put_U32 (W, 16#A1B2C3D4#);
      Put_U16 (W, 16#1234#);
      Put_U8 (W, 16#5A#);
      Put_F32 (W, 3.5);
      Put_U64 (W, 16#01020304#, 16#05060708#);  -- hi, lo
      Put_String (W, "bay-12");
      Raw (0) := 16#DE#;
      Raw (1) := 16#AD#;
      Raw (2) := 16#BE#;
      Raw (3) := 16#EF#;
      Put_Seq_U8 (W, Raw);
   end Fixture;

   procedure Check (Endian : Endianness; Path : String; Tag : String) is
      W      : Writer;
      Golden : Byte_Array (0 .. 511);
      G_Last : Integer := -1;
      F      : Byte_IO.File_Type;
      B      : Byte;
   begin
      Init (W, Endian);
      Fixture (W);

      begin
         Byte_IO.Open (F, Byte_IO.In_File, Path);
      exception
         when others =>
            Text_IO.Put_Line (Tag & ": cannot open " & Path);
            Ok_All := False;
            return;
      end;
      while not Byte_IO.End_Of_File (F) loop
         Byte_IO.Read (F, B);
         G_Last := G_Last + 1;
         Golden (G_Last) := B;
      end loop;
      Byte_IO.Close (F);

      if Length (W) /= G_Last + 1 then
         Text_IO.Put_Line (Tag & ": length mismatch Ada="
                           & Integer'Image (Length (W))
                           & " golden=" & Integer'Image (G_Last + 1));
         Ok_All := False;
         return;
      end if;
      for I in 0 .. Length (W) - 1 loop
         if W.Buf (I) /= Golden (I) then
            Text_IO.Put_Line (Tag & ": byte" & Integer'Image (I) & " differs");
            Ok_All := False;
            return;
         end if;
      end loop;
      Text_IO.Put_Line (Tag & ":" & Integer'Image (Length (W))
                        & " bytes byte-identical to Rust golden");
   end Check;

begin
   Check (Little, "build/golden_le.bin", "LE");
   Check (Big, "build/golden_be.bin", "BE");
   if Ok_All then
      Text_IO.Put_Line ("ALL OK");
   else
      Text_IO.Put_Line ("FAILED");
   end if;
end Test_Ada83_Identity;
