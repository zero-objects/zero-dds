-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reflective-codec byte-identity test (ADR 0013 Stage 2 CP-C4). Builds the
--  @final, @appendable(nested) and @mutable samples as reflective value trees
--  and encodes them through the descriptor-driven codec, asserting the bytes
--  match the Rust goldens -- i.e. the reflective path is byte-identical to the
--  fixed codegen path.
--
--  usage: test_native_reflect <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams;           use Ada.Streams;
with Ada.Streams.Stream_IO;
with Interfaces;            use Interfaces;
with Zerodds_Native_Wire;   use Zerodds_Native_Wire;
with Native_Reflect;        use Native_Reflect;

procedure Test_Native_Reflect is

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
         G_Len : constant Natural := Natural (Last) + 1;
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
               Put_Line (Tag & ": byte" & Natural'Image (I) & " differs");
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

   function Encode (S : Dyn_Struct; Endian : Endianness) return Byte_Array is
      W : Writer;
   begin
      Init (W, Endian);
      Encode_Struct (W, S);
      return Bytes (W);
   end Encode;

   --  @final SensorReading value tree.
   Final_Tree : constant Dyn_Struct :=
     (Ext    => X_Final,
      Ids    => null,
      Fields => new Field_Array'
        (0 => (Kind => K_U32, U32v => 16#A1B2_C3D4#),
         1 => (Kind => K_U16, U16v => 16#1234#),
         2 => (Kind => K_U8,  U8v => 16#5A#),
         3 => (Kind => K_F32, F32v => 3.5),
         4 => (Kind => K_U64, U64v => 16#0102_0304_0506_0708#),
         5 => (Kind => K_String, Strv => new String'("bay-12")),
         6 => (Kind => K_Seq_U8,
               Bytesv => new Byte_Array'(16#DE#, 16#AD#, 16#BE#, 16#EF#))));

   --  @appendable Inner { uint16 a; uint32 b }.
   function Inner (A : Unsigned_16; B : Unsigned_32) return Struct_Access is
     (new Dyn_Struct'
        (Ext    => X_Appendable,
         Ids    => null,
         Fields => new Field_Array'
           (0 => (Kind => K_U16, U16v => A),
            1 => (Kind => K_U32, U32v => B))));

   Nested_Tree : constant Dyn_Struct :=
     (Ext    => X_Appendable,
      Ids    => null,
      Fields => new Field_Array'
        (0 => (Kind => K_U32, U32v => 16#CAFE_BABE#),
         1 => (Kind => K_Nested, Nestedv => Inner (16#1111#, 16#2222_3333#)),
         2 => (Kind => K_Seq_Struct,
               Elemsv => new Struct_Ptr_Array'
                 (0 => Inner (16#AAAA#, 16#BBBB_CCCC#),
                  1 => Inner (16#DDDD#, 16#EEEE_FFFF#))),
         3 => (Kind => K_String, Strv => new String'("nested"))));

   --  @mutable M { @10 uint32; @20 string; @30 uint16 }.
   Mutable_Tree : constant Dyn_Struct :=
     (Ext    => X_Mutable,
      Ids    => new Id_Array'(0 => 10, 1 => 20, 2 => 30),
      Fields => new Field_Array'
        (0 => (Kind => K_U32, U32v => 16#DEAD_BEEF#),
         1 => (Kind => K_String, Strv => new String'("mut")),
         2 => (Kind => K_U16, U16v => 16#0777#)));

   Dir : constant String := (if Argument_Count >= 1 then Argument (1) else ".");
   Ok  : Boolean := True;

begin
   Ok := Compare (Encode (Final_Tree, Little),   Dir & "/golden_le.bin",         "reflect final LE")   and then Ok;
   Ok := Compare (Encode (Final_Tree, Big),      Dir & "/golden_be.bin",         "reflect final BE")   and then Ok;
   Ok := Compare (Encode (Nested_Tree, Little),  Dir & "/golden_nested_le.bin",  "reflect nested LE")  and then Ok;
   Ok := Compare (Encode (Nested_Tree, Big),     Dir & "/golden_nested_be.bin",  "reflect nested BE")  and then Ok;
   Ok := Compare (Encode (Mutable_Tree, Little), Dir & "/golden_mutable_le.bin", "reflect mutable LE") and then Ok;
   Ok := Compare (Encode (Mutable_Tree, Big),    Dir & "/golden_mutable_be.bin", "reflect mutable BE") and then Ok;

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Native_Reflect;
