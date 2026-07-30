-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 endpoint framing (ADR 0013). XRCE WRITE_DATA framing, all
--  procedural (Ada 83 has no tagged types / access-to-subprogram), so the async
--  model is POLL-based: the application drives a non-blocking receive loop and
--  decodes each frame itself -- no callbacks.

with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;

package Zerodds_Ada83_Endpoint is

   --  Frame `Sample` (an XCDR body) as an XRCE WRITE_DATA message into W.
   procedure Xrce_Write_Frame
     (W       : in out Writer;
      Session : Long_Integer;
      Stream  : Long_Integer;
      Seq     : Long_Integer;
      Sample  : Byte_Array);

   --  Locate the sample body inside a received WRITE_DATA frame. Valid is False
   --  if the frame is too short or not a WRITE_DATA submessage.
   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Integer;
      Body_Last  : out Integer;
      Valid      : out Boolean);

end Zerodds_Ada83_Endpoint;
