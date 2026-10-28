-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Native_Framing is

   function Xrce_Object_Id
     (Raw_Id : Unsigned_16; Kind : Unsigned_16) return Unsigned_16 is
     (Shift_Left (Raw_Id and 16#0FFF#, 4) or (Kind and 16#000F#));

   function Xrce_Write_Frame
     (Session   : Byte;
      Stream    : Byte;
      Seq       : Unsigned_16;
      Object_Id : Unsigned_16;
      Sample    : Byte_Array) return Byte_Array
   is
      W        : Writer;
      Body_Len : Unsigned_16;
   begin
      if Sample'Length > 65535 - XRCE_Base_Size then
         raise Sample_Too_Large;
      end if;
      Body_Len := Unsigned_16 (XRCE_Base_Size + Sample'Length);
      Init (W, Little);
      Put_U8 (W, Session);
      Put_U8 (W, Stream);
      Put_U8 (W, Byte (Seq and 16#FF#));              -- sequence_nr LE
      Put_U8 (W, Byte (Shift_Right (Seq, 8) and 16#FF#));
      Put_U8 (W, XRCE_SM_Write_Data);
      Put_U8 (W, XRCE_Write_Flags);
      Put_U8 (W, Byte (Body_Len and 16#FF#));         -- submsg len LE
      Put_U8 (W, Byte (Shift_Right (Body_Len, 8) and 16#FF#));
      --  BaseObjectRequest (7.7.8): RequestId then ObjectId, most significant
      --  octet first.
      Put_U8 (W, Byte (Shift_Right (Seq, 8) and 16#FF#));
      Put_U8 (W, Byte (Seq and 16#FF#));
      Put_U8 (W, Byte (Shift_Right (Object_Id, 8) and 16#FF#));
      Put_U8 (W, Byte (Object_Id and 16#FF#));
      Put_Bytes (W, Sample);
      --  `Put_Bytes` sets `W.Overflow` and returns when the frame does not
      --  fit `Max_Buffer`, which is 4096 here, so every sample from 4089
      --  octets up left the eight-octet header behind -- a correct length
      --  field and no body -- and `Bytes (W)` handed those eight octets
      --  back as a frame. The flag was representable and no caller in the
      --  tree read one.
      if W.Overflow then
         raise Frame_Does_Not_Fit;
      end if;
      return Bytes (W);
   end Xrce_Write_Frame;

   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Natural;
      Body_Last  : out Integer;
      Valid      : out Boolean)
   is
      Sm_Len : Natural;
   begin
      --  Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
      --  len_hi], then the 4-octet BaseObjectRequest (7.7.8), then the sample.
      --  The 16-bit little-endian submessage_length (bytes 6..7) bounds the
      --  body exactly: the returned slice is Frame'First + 12 ..
      --  Frame'First + 7 + Sm_Len, never .. Frame'Last, so trailing padding or
      --  an appended submessage is not folded into the sample.
      Body_First := Frame'First + 8 + XRCE_Base_Size;
      Body_Last  := Frame'First + 7 + XRCE_Base_Size;  --  empty slice by default
      Valid      := False;
      --  Too short, or submessage id (5th byte) is neither WRITE_DATA
      --  (endpoint->hub / loopback) nor DATA (hub->endpoint). See DDS-XRCE
      --  spec 8.3.5. A declared length that runs past the datagram
      --  (truncation / wrong length) is rejected.
      if Frame'Length >= 8 + XRCE_Base_Size
        and then (Frame (Frame'First + 4) = XRCE_SM_Write_Data
                  or else Frame (Frame'First + 4) = XRCE_SM_Data)
      then
         Sm_Len := Natural (Frame (Frame'First + 6))
                   + Natural (Frame (Frame'First + 7)) * 256;
         if Sm_Len >= XRCE_Base_Size and then 8 + Sm_Len <= Frame'Length then
            Body_Last := Frame'First + 7 + Sm_Len;
            Valid     := True;
         end if;
      end if;
   end Xrce_Read_Frame;

   function Fcs16 (Data : Byte_Array) return Unsigned_16 is
      Crc : Unsigned_16 := 16#FFFF#;
   begin
      for B of Data loop
         Crc := Crc xor Unsigned_16 (B);
         for K in 1 .. 8 loop
            if (Crc and 16#0001#) /= 0 then
               Crc := Shift_Right (Crc, 1) xor 16#8408#;
            else
               Crc := Shift_Right (Crc, 1);
            end if;
         end loop;
      end loop;
      return not Crc;
   end Fcs16;

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

   function Serial_Frame
     (Source, Remote : Byte; Payload : Byte_Array) return Byte_Array
   is
      Len  : constant Natural := Payload'Length;
      Head : constant Byte_Array (0 .. 3) :=
        [0 => Source,
         1 => Remote,
         2 => Byte (Len mod 256),                        -- payloadLength LE
         3 => Byte ((Len / 256) mod 256)];
      Crc  : constant Unsigned_16 := Fcs16 (Head & Payload);
      Crcb : constant Byte_Array (0 .. 1) :=
        [0 => Byte (Crc and 16#FF#),                     -- frameCheck LE
         1 => Byte (Shift_Right (Crc, 8) and 16#FF#)];
      W    : Writer;
   begin
      --  Worst case every octet after the flag is stuffed.
      if Len > 16#FFFF# or else 1 + 2 * (Len + 6) > Max_Buffer then
         raise Frame_Does_Not_Fit;
      end if;
      Init (W, Little);
      Put_U8 (W, XRCE_Flag);
      Stuff_Into (W, Head);
      Stuff_Into (W, Payload);
      Stuff_Into (W, Crcb);
      return Bytes (W);
   end Serial_Frame;

end Native_Framing;
