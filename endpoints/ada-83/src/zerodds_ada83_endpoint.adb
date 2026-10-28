-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Zerodds_Ada83_Endpoint is

   function Xrce_Object_Id
     (Raw_Id : Long_Integer; Kind : Long_Integer) return Long_Integer is
   begin
      return ((Raw_Id mod 4096) * 16) + (Kind mod 16);
   end Xrce_Object_Id;

   procedure Xrce_Write_Frame
     (W         : in out Writer;
      Session   : Long_Integer;
      Stream    : Long_Integer;
      Seq       : Long_Integer;
      Object_Id : Long_Integer;
      Sample    : Byte_Array)
   is
      Len : constant Long_Integer :=
        Long_Integer (Sample'Length) + Xrce_Base_Size;
   begin
      --  Refused rather than truncated. `Len mod 256` and `(Len / 256) mod
      --  256` describe 65536 as zero, and `Put_Bytes` below appends the whole
      --  sample either way, so the frame would claim a length it does not
      --  carry.
      if Len > 65535 then
         raise Sample_Too_Large;
      end if;
      Put_U8 (W, Session);                 -- session
      Put_U8 (W, Stream);                  -- stream
      Put_U8 (W, Seq mod 256);             -- sequence_nr LE (lo)
      Put_U8 (W, (Seq / 256) mod 256);     -- (hi)
      Put_U8 (W, 7);                       -- XRCE WRITE_DATA
      --  E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits
      --  1..3 -- DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 3) would
      --  additionally require a SampleInfo in the body.
      Put_U8 (W, 1);                       -- flags
      Put_U8 (W, Len mod 256);             -- submessage length LE (lo)
      Put_U8 (W, (Len / 256) mod 256);     -- (hi)
      --  BaseObjectRequest (7.7.8): RequestId then ObjectId, most significant
      --  octet first.
      Put_U8 (W, (Seq / 256) mod 256);
      Put_U8 (W, Seq mod 256);
      Put_U8 (W, (Object_Id / 256) mod 256);
      Put_U8 (W, Object_Id mod 256);
      Put_Bytes (W, Sample);
      --  `Put_Bytes` sets `W.Overflow` and returns when the sample does not
      --  fit `Max_Buffer`, which is 2048 here -- so every sample from 2041
      --  octets up left the eight-octet header behind, with a correct length
      --  field and no body, and `Length (W)` reported 8. The flag was
      --  representable and no caller read it: `test_ada83_async` enqueued
      --  `Frame_W.Buf (0 .. Length (Frame_W) - 1)` whatever it was. A frame
      --  that claims a body it does not carry is not a frame.
      if W.Overflow then
         raise Frame_Does_Not_Fit;
      end if;
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
      Body_First := Frame'First + 8 + Xrce_Base_Size;
      Body_Last  := Frame'First + 7 + Xrce_Base_Size;  --  empty slice by default
      Valid      := False;
      if Frame'Length >= 8 + Xrce_Base_Size
        and then (Frame (Frame'First + 4) = 7 or else Frame (Frame'First + 4) = 9)
      then
         Sm_Len := Integer (Frame (Frame'First + 6))
                   + Integer (Frame (Frame'First + 7)) * 256;
         if Sm_Len >= Xrce_Base_Size and then 8 + Sm_Len <= Frame'Length then
            Body_Last := Frame'First + 7 + Sm_Len;
            Valid     := True;
         end if;
      end if;
   end Xrce_Read_Frame;

end Zerodds_Ada83_Endpoint;
