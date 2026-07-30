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
      Sm_Len : Integer;
   begin
      --  Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
      --  len_hi] then the sample body. The 16-bit little-endian
      --  submessage_length (bytes 6..7) bounds the body exactly: the returned
      --  slice is Frame'First + 8 .. Frame'First + 7 + Sm_Len, never
      --  .. Frame'Last, so trailing padding or an appended submessage is not
      --  folded into the sample.
      --
      --  Accept WRITE_DATA (7, endpoint->hub / loopback) and DATA (9,
      --  hub->endpoint). See DDS-XRCE spec 8.3.5. Invalid (rejected) for a
      --  short header, a wrong submessage id, or a declared length that runs
      --  past the datagram (truncation / wrong length).
      Body_First := Frame'First + 8;
      Body_Last  := Frame'First + 7;  --  empty slice by default
      Valid      := False;
      if Frame'Length >= 8
        and then (Frame (Frame'First + 4) = 7 or else Frame (Frame'First + 4) = 9)
      then
         Sm_Len := Integer (Frame (Frame'First + 6))
                   + Integer (Frame (Frame'First + 7)) * 256;
         if 8 + Sm_Len <= Frame'Length then
            Body_Last := Frame'First + 7 + Sm_Len;
            Valid     := True;
         end if;
      end if;
   end Xrce_Read_Frame;

end Zerodds_Ada83_Endpoint;
