-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Byte-identity test for the pure-Ada wire-core (ADR 0013 Stage 2). Encodes
--  the @final, @appendable(nested) and @mutable samples in LE and BE wire order
--  and compares each to the Rust goldens (endpoints/golden-gen). Because the
--  Ada serialization is by explicit byte order, the BE output produced on this
--  little-endian host proves host-endian independence -- the native + BE claim.
--
--  usage: test_native_identity <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams;           use Ada.Streams;
with Ada.Streams.Stream_IO;
with Zerodds_Native_Wire;   use Zerodds_Native_Wire;
with Native_Samples;        use Native_Samples;

procedure Test_Native_Identity is

   package SIO renames Ada.Streams.Stream_IO;

   function Compare (Actual : Byte_Array; Golden_Path : String; Tag : String)
      return Boolean
   is
      F      : SIO.File_Type;
      Golden : Stream_Element_Array (0 .. 511);
      Last   : Stream_Element_Offset;
   begin
      SIO.Open (F, SIO.In_File, Golden_Path);
      SIO.Read (F, Golden, Last);
      SIO.Close (F);

      declare
         G_Len : constant Natural := Natural (Last) + 1;  -- 'First = 0
      begin
         if Actual'Length /= G_Len then
            Put_Line (Tag & ": length mismatch Ada=" & Natural'Image (Actual'Length)
                      & " golden=" & Natural'Image (G_Len));
            return False;
         end if;
         for I in 0 .. Actual'Length - 1 loop
            if Integer (Actual (Actual'First + I))
              /= Integer (Golden (Stream_Element_Offset (I)))
            then
               Put_Line (Tag & ": byte" & Natural'Image (I) & " differs Ada="
                         & Integer'Image (Integer (Actual (Actual'First + I)))
                         & " golden="
                         & Integer'Image (Integer (Golden (Stream_Element_Offset (I)))));
               return False;
            end if;
         end loop;
         Put_Line (Tag & ":" & Natural'Image (Actual'Length)
                   & " bytes byte-identical to Rust golden");
         return True;
      end;
   exception
      when others =>
         Put_Line (Tag & ": cannot read " & Golden_Path);
         return False;
   end Compare;

   Dir : constant String := (if Argument_Count >= 1 then Argument (1) else ".");
   Ok  : Boolean := True;

begin
   Ok := Compare (Encode_Final (Little),  Dir & "/golden_le.bin",         "final LE")   and then Ok;
   Ok := Compare (Encode_Final (Big),     Dir & "/golden_be.bin",         "final BE")   and then Ok;
   Ok := Compare (Encode_Nested (Little), Dir & "/golden_nested_le.bin",  "nested LE")  and then Ok;
   Ok := Compare (Encode_Nested (Big),    Dir & "/golden_nested_be.bin",  "nested BE")  and then Ok;
   Ok := Compare (Encode_Mutable (Little), Dir & "/golden_mutable_le.bin", "mutable LE") and then Ok;
   Ok := Compare (Encode_Mutable (Big),    Dir & "/golden_mutable_be.bin", "mutable BE") and then Ok;

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Native_Identity;
