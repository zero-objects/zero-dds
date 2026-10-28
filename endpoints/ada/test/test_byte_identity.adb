-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Byte-identity test for the Ada Stage 1 wire-core binding (ADR 0013).
--  Encodes the fixed SensorReading in LE and BE wire order through the Ada
--  bindings and compares the bytes to the same Rust goldens the C SDK uses
--  (endpoints/golden-gen). Then round-trips a decode.
--
--  usage: test_byte_identity <golden_le.bin> <golden_be.bin>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams;           use Ada.Streams;
with Ada.Streams.Stream_IO;
with Interfaces.C;          use Interfaces.C;
with Zdw;                   use Zdw;
with Sample_Sensor;         use Sample_Sensor;
with Sample_Fixtures;       use Sample_Fixtures;
with Deep_Reading;

procedure Test_Byte_Identity is

   package SIO renames Ada.Streams.Stream_IO;

   function Read_Golden
     (Path : String; Buf : out Stream_Element_Array; Last : out Stream_Element_Offset)
      return Boolean
   is
      F : SIO.File_Type;
   begin
      SIO.Open (F, SIO.In_File, Path);
      SIO.Read (F, Buf, Last);
      SIO.Close (F);
      return True;
   exception
      when others =>
         Put_Line ("cannot open " & Path);
         return False;
   end Read_Golden;

   function Check_Wire (Endian : int; Golden_Path : String; Tag : String) return Boolean is
      S      : constant Sensor_Reading := Sensor_Fixture;
      Out_Buf : array (0 .. 255) of aliased unsigned_char := [others => 0];
      W       : aliased Zdw_Writer;
      Golden  : Stream_Element_Array (0 .. 255);
      G_Last  : Stream_Element_Offset;
   begin
      Writer_Init (W'Access, Out_Buf'Address, Out_Buf'Length, Endian);
      if Encode (W'Access, S) /= ZDW_OK then
         Put_Line (Tag & ": encode error" & int'Image (W.Error));
         return False;
      end if;

      if not Read_Golden (Golden_Path, Golden, G_Last) then
         return False;
      end if;

      declare
         G_Len : constant Natural := Natural (G_Last) + 1;  -- Golden'First = 0
         W_Len : constant Natural := Natural (W.Len);
      begin
         if W_Len /= G_Len then
            Put_Line (Tag & ": length mismatch Ada=" & Natural'Image (W_Len)
                      & " golden=" & Natural'Image (G_Len));
            return False;
         end if;
         for I in 0 .. W_Len - 1 loop
            if Integer (Out_Buf (I)) /= Integer (Golden (Stream_Element_Offset (I))) then
               Put_Line (Tag & ": byte" & Natural'Image (I) & " differs Ada="
                         & Integer'Image (Integer (Out_Buf (I))) & " golden="
                         & Integer'Image (Integer (Golden (Stream_Element_Offset (I)))));
               return False;
            end if;
         end loop;
         Put_Line (Tag & ":" & Natural'Image (W_Len) & " bytes byte-identical to Rust golden");
      end;

      --  round-trip decode
      declare
         R : aliased Zdw_Reader;
         D : Sensor_Reading;
      begin
         Reader_Init (R'Access, Out_Buf'Address, W.Len, Endian);
         if Decode (R'Access, D) /= ZDW_OK then
            Put_Line (Tag & ": decode error" & int'Image (R.Error));
            return False;
         end if;
         if D.Id /= S.Id or else D.Kind /= S.Kind or else D.Flags /= S.Flags
           or else D.Value /= S.Value or else D.Stamp /= S.Stamp
           or else D.Raw_Len /= S.Raw_Len
           or else D.Raw (0) /= S.Raw (0) or else D.Raw (3) /= S.Raw (3)
         then
            Put_Line (Tag & ": round-trip mismatch");
            return False;
         end if;
         Put_Line (Tag & ": round-trip decode ok");
      end;
      return True;
   end Check_Wire;

   Ok : Boolean := True;

begin
   if Argument_Count < 2 then
      Put_Line ("usage: test_byte_identity <golden_le.bin> <golden_be.bin>");
      Set_Exit_Status (2);
      return;
   end if;

   Ok := Check_Wire (ZDW_LE, Argument (1), "LE") and then Ok;
   Ok := Check_Wire (ZDW_BE, Argument (2), "BE") and then Ok;

   --  `Deep_Reading.Frame` writes an eight-octet envelope and then the body
   --  into a `Frame_Store`, whose `Data` is `Max_Frame` long. It had no bound:
   --  a body of `Max_Frame - 7` octets indexed past the end of the array,
   --  which is a Constraint_Error while checks are on and a write into the
   --  neighbouring `Len` field under -gnatp. It is a named refusal now.
   declare
      Over : Deep_Reading.Frame_Store;
   begin
      Over.Len := Deep_Reading.Max_Frame - 7 - Deep_Reading.Base_Size;
      declare
         Ignored : constant Deep_Reading.Frame_Store :=
           Deep_Reading.Frame (1, 16#0015#, Over);
         pragma Unreferenced (Ignored);
      begin
         Put_Line ("negative: a body past Max_Frame not refused");
         Ok := False;
      end;
   exception
      when Deep_Reading.Body_Too_Large =>
         Put_Line ("negative: a body past Max_Frame refused");
   end;

   --  The bound is the store's and not one off it.
   declare
      Largest : Deep_Reading.Frame_Store;
   begin
      Largest.Len := Deep_Reading.Max_Frame - 8 - Deep_Reading.Base_Size;
      if Deep_Reading.Frame (1, 16#0015#, Largest).Len /= Deep_Reading.Max_Frame then
         Put_Line ("negative: the largest body must still frame");
         Ok := False;
      else
         Put_Line ("negative: the largest body still frames");
      end if;
   end;

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Byte_Identity;
