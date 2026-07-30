-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Byte-identity test for the pure-Ada XRCE + serial framing (ADR 0013 Stage 2)
--  against the Rust goldens (zerodds-xrce). The XRCE frame carries the @final
--  sample; the serial frame HDLC-wraps that XRCE message.
--
--  usage: test_native_framing <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams;           use Ada.Streams;
with Ada.Streams.Stream_IO;
with Zerodds_Native_Wire;   use Zerodds_Native_Wire;
with Native_Samples;        use Native_Samples;
with Native_Framing;        use Native_Framing;

procedure Test_Native_Framing is

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

   Dir  : constant String := (if Argument_Count >= 1 then Argument (1) else ".");
   Xrce : constant Byte_Array :=
     Xrce_Write_Frame (16#80#, 16#01#, 1, Encode_Final (Little));
   Ok   : Boolean := True;

   --  Negative frame vectors (self-contained): the reader must bound the body
   --  to the declared submessage_length and reject malformed frames.
   procedure Negative_Frame_Vectors (Pass : in out Boolean) is
      First  : constant Byte_Array := Xrce_Write_Frame (16#80#, 16#01#, 1, (16#AA#, 16#BB#, 16#CC#));
      Second : constant Byte_Array := Xrce_Write_Frame (16#80#, 16#01#, 2, (16#DD#, 16#EE#));
      Concat : constant Byte_Array := First & Second;
      Bf     : Natural;
      Bl     : Integer;
      Valid  : Boolean;
   begin
      Xrce_Read_Frame (Concat, Bf, Bl, Valid);
      if not Valid or else Bl - Bf + 1 /= 3
        or else Concat (Bf) /= 16#AA# or else Concat (Bf + 1) /= 16#BB#
        or else Concat (Bf + 2) /= 16#CC#
      then
         Put_Line ("negative: appended submessage leaked"); Pass := False;
      else
         Put_Line ("negative: appended submessage bounded out");
      end if;

      declare
         Overlong : Byte_Array := First;
      begin
         Overlong (Overlong'First + 6) := 16#FF#;
         Overlong (Overlong'First + 7) := 16#FF#;
         Xrce_Read_Frame (Overlong, Bf, Bl, Valid);
         if Valid then
            Put_Line ("negative: over-long length not rejected"); Pass := False;
         else
            Put_Line ("negative: over-long length rejected");
         end if;
      end;

      declare
         Trunc : constant Byte_Array := (16#80#, 16#01#, 16#00#, 16#00#, 16#07#);
      begin
         Xrce_Read_Frame (Trunc, Bf, Bl, Valid);
         if Valid then
            Put_Line ("negative: truncated header not rejected"); Pass := False;
         else
            Put_Line ("negative: truncated header rejected");
         end if;
      end;
   end Negative_Frame_Vectors;

begin
   Ok := Compare (Xrce, Dir & "/golden_xrce_le.bin", "xrce") and then Ok;
   Ok := Compare (Serial_Frame (Xrce), Dir & "/golden_serial_le.bin", "serial")
     and then Ok;
   Negative_Frame_Vectors (Ok);

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Native_Framing;
