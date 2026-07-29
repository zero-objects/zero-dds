-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Pure-Ada endpoint framing (ADR 0013 Stage 2): the XRCE-DDS WRITE_DATA frame
--  and the HDLC-style serial framing (byte stuffing + CRC-16/CCITT-FALSE) an
--  endpoint uses over a UART / RS-485 link. Byte-identical to endpoints/c
--  (`zerodds_endpoint.c`) and the Rust `zerodds-xrce` transport.

with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Native_Framing is

   XRCE_SM_Write_Data : constant := 16#07#;
   XRCE_Write_Flags   : constant := 16#03#;
   XRCE_Flag          : constant := 16#7E#;
   XRCE_Esc           : constant := 16#7D#;
   XRCE_Stuff         : constant := 16#20#;

   --  An XRCE WRITE_DATA message: 4-byte header (session, stream, seq LE u16) +
   --  4-byte submessage header (id, flags, length LE u16) + the sample bytes.
   function Xrce_Write_Frame
     (Session : Byte;
      Stream  : Byte;
      Seq     : Unsigned_16;
      Sample  : Byte_Array) return Byte_Array;

   --  Locate the sample body inside a received WRITE_DATA frame. The 8-byte
   --  envelope (header + submessage header) is skipped; `Body_First .. Body_Last`
   --  index into `Frame` (empty body -> Body_Last = Body_First - 1). `Valid` is
   --  False if the frame is too short or not a WRITE_DATA submessage.
   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Natural;
      Body_Last  : out Integer;
      Valid      : out Boolean);

   --  CRC-16/CCITT-FALSE (poly 0x1021, init 0xFFFF, no reflection).
   function Crc16_Ccitt_False (Data : Byte_Array) return Unsigned_16;

   --  HDLC-style serial frame: FLAG + stuffed(payload) + stuffed(CRC big-endian)
   --  + FLAG.
   function Serial_Frame (Payload : Byte_Array) return Byte_Array;

end Native_Framing;
