-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Byte-identity test for the pure-Ada XRCE + serial framing (ADR 0013 Stage 2)
--  against the Rust goldens (zerodds-xrce). The XRCE frame carries the @final
--  sample; the serial frame HDLC-wraps that XRCE message.
--
--  usage: test_native_framing <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Interfaces;
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
   --  The DataWriter every frame in this test addresses (7.2.1: raw 16#001#
   --  plus OBJK_DATAWRITER).
   Writer_Id : constant Interfaces.Unsigned_16 :=
     Xrce_Object_Id (16#001#, Objk_DataWriter);
   Xrce : constant Byte_Array :=
     Xrce_Write_Frame (16#80#, 16#01#, 1, Writer_Id, Encode_Final (Little));
   Ok   : Boolean := True;

   --  Negative frame vectors (self-contained): the reader must bound the body
   --  to the declared submessage_length and reject malformed frames.
   procedure Negative_Frame_Vectors (Pass : in out Boolean) is
      First  : constant Byte_Array :=
        Xrce_Write_Frame (16#80#, 16#01#, 1, Writer_Id, (16#AA#, 16#BB#, 16#CC#));
      Second : constant Byte_Array :=
        Xrce_Write_Frame (16#80#, 16#01#, 2, Writer_Id, (16#DD#, 16#EE#));
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
      --  A sample the 16-bit submessage_length cannot describe. There was no
      --  bound at all here; the implicit Constraint_Error of
      --  `Unsigned_16 (Sample'Length)` was the only thing standing in the way
      --  and it goes away under -gnatp.
      declare
         Big : constant Byte_Array (1 .. 16#10000#) := (others => 0);
      begin
         declare
            Ignored : constant Byte_Array :=
              Xrce_Write_Frame (16#80#, 16#01#, 1, Writer_Id, Big);
            pragma Unreferenced (Ignored);
         begin
            Put_Line ("negative: sample above 65535 not refused"); Pass := False;
         end;
      exception
         when Sample_Too_Large =>
            Put_Line ("negative: sample above 65535 refused");
      end;

      --  The largest sample the writer's own buffer can carry still frames,
      --  so the two bounds are not confused with one another: 65535 is the
      --  submessage_length field's and 4096 is `Max_Buffer`'s.
      declare
         Largest : constant Byte_Array (1 .. 4084) := (others => 0);
         Framed  : constant Byte_Array :=
           Xrce_Write_Frame (16#80#, 16#01#, 1, Writer_Id, Largest);
      begin
         if Framed'Length /= 8 + XRCE_Base_Size + 4084 then
            Put_Line ("negative: a 4084 sample must still frame"); Pass := False;
         else
            Put_Line ("negative: a 4084 sample still frames");
         end if;
      end;

      --  One octet more than the buffer holds. `Put_Bytes` used to set
      --  `W.Overflow` and return, and `Bytes (W)` handed back the eight-octet
      --  header as if it were a frame.
      declare
         Over : constant Byte_Array (1 .. 4085) := (others => 0);
      begin
         declare
            Ignored : constant Byte_Array :=
              Xrce_Write_Frame (16#80#, 16#01#, 1, Writer_Id, Over);
            pragma Unreferenced (Ignored);
         begin
            Put_Line ("negative: a frame past Max_Buffer not refused"); Pass := False;
         end;
      exception
         when Frame_Does_Not_Fit =>
            Put_Line ("negative: a frame past Max_Buffer refused");
      end;
   end Negative_Frame_Vectors;

begin
   Ok := Compare (Xrce, Dir & "/golden_xrce_le.bin", "xrce") and then Ok;
   Ok := Compare (Serial_Frame (1, 0, Xrce), Dir & "/golden_serial_le.bin",
                  "serial")
     and then Ok;
   declare
      use type Interfaces.Unsigned_16;
      Check : constant Byte_Array (0 .. 8) :=
        [16#31#, 16#32#, 16#33#, 16#34#, 16#35#, 16#36#, 16#37#, 16#38#, 16#39#];
   begin
      if Fcs16 (Check) = 16#906E# then
         Put_Line ("fcs16 check value 0x906E ok");
      else
         Put_Line ("fcs16 check value wrong"); Ok := False;
      end if;
   end;
   Negative_Frame_Vectors (Ok);

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Native_Framing;
