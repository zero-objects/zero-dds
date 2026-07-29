-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Zerodds_Ada83_Endpoint is

   procedure Xrce_Write_Frame
     (W       : in out Writer;
      Session : Long_Integer;
      Stream  : Long_Integer;
      Seq     : Long_Integer;
      Sample  : Byte_Array)
   is
      Len : constant Long_Integer := Long_Integer (Sample'Length);
   begin
      Put_U8 (W, Session);                 -- session
      Put_U8 (W, Stream);                  -- stream
      Put_U8 (W, Seq mod 256);             -- sequence_nr LE (lo)
      Put_U8 (W, (Seq / 256) mod 256);     -- (hi)
      Put_U8 (W, 7);                       -- XRCE WRITE_DATA
      Put_U8 (W, 3);                       -- flags
      Put_U8 (W, Len mod 256);             -- submessage length LE (lo)
      Put_U8 (W, (Len / 256) mod 256);     -- (hi)
      Put_Bytes (W, Sample);
   end Xrce_Write_Frame;

   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Integer;
      Body_Last  : out Integer;
      Valid      : out Boolean)
   is
   begin
      Body_First := Frame'First + 8;
      Body_Last  := Frame'Last;
      if Frame'Length >= 8 and then Frame (Frame'First + 4) = 7 then
         Valid := True;
      else
         Valid := False;
      end if;
   end Xrce_Read_Frame;

end Zerodds_Ada83_Endpoint;
