-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Native_Framing is

   function Xrce_Write_Frame
     (Session : Byte;
      Stream  : Byte;
      Seq     : Unsigned_16;
      Sample  : Byte_Array) return Byte_Array
   is
      W : Writer;
   begin
      Init (W, Little);
      Put_U8 (W, Session);
      Put_U8 (W, Stream);
      Put_U8 (W, Byte (Seq and 16#FF#));              -- sequence_nr LE
      Put_U8 (W, Byte (Shift_Right (Seq, 8) and 16#FF#));
      Put_U8 (W, XRCE_SM_Write_Data);
      Put_U8 (W, XRCE_Write_Flags);
      Put_U8 (W, Byte (Unsigned_16 (Sample'Length) and 16#FF#));  -- submsg len LE
      Put_U8 (W, Byte (Shift_Right (Unsigned_16 (Sample'Length), 8) and 16#FF#));
      Put_Bytes (W, Sample);
      return Bytes (W);
   end Xrce_Write_Frame;

   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Natural;
      Body_Last  : out Integer;
      Valid      : out Boolean)
   is
   begin
      Body_First := Frame'First + 8;
      Body_Last  := Frame'Last;
      --  Too short, or submessage id (5th byte) is not WRITE_DATA.
      if Frame'Length < 8
        or else Frame (Frame'First + 4) /= XRCE_SM_Write_Data
      then
         Valid := False;
      else
         Valid := True;
      end if;
   end Xrce_Read_Frame;

   function Crc16_Ccitt_False (Data : Byte_Array) return Unsigned_16 is
      Crc : Unsigned_16 := 16#FFFF#;
   begin
      for B of Data loop
         Crc := Crc xor Shift_Left (Unsigned_16 (B), 8);
         for K in 1 .. 8 loop
            if (Crc and 16#8000#) /= 0 then
               Crc := (Shift_Left (Crc, 1) xor 16#1021#) and 16#FFFF#;
            else
               Crc := Shift_Left (Crc, 1) and 16#FFFF#;
            end if;
         end loop;
      end loop;
      return Crc;
   end Crc16_Ccitt_False;

   --  Byte-stuff `Data` into `W`: FLAG/ESC bytes are escaped as ESC + (b xor
   --  STUFF).
   procedure Stuff_Into (W : in out Writer; Data : Byte_Array) is
   begin
      for B of Data loop
         if B = XRCE_Flag or else B = XRCE_Esc then
            Put_U8 (W, XRCE_Esc);
            Put_U8 (W, B xor XRCE_Stuff);
         else
            Put_U8 (W, B);
         end if;
      end loop;
   end Stuff_Into;

   function Serial_Frame (Payload : Byte_Array) return Byte_Array is
      W    : Writer;
      Crc  : constant Unsigned_16 := Crc16_Ccitt_False (Payload);
      Crcb : constant Byte_Array (0 .. 1) :=
        [0 => Byte (Shift_Right (Crc, 8) and 16#FF#),   -- CRC big-endian
         1 => Byte (Crc and 16#FF#)];
   begin
      Init (W, Little);
      Put_U8 (W, XRCE_Flag);
      Stuff_Into (W, Payload);
      Stuff_Into (W, Crcb);
      Put_U8 (W, XRCE_Flag);
      return Bytes (W);
   end Serial_Frame;

end Native_Framing;
