-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Pure-Ada endpoint framing (ADR 0013 Stage 2): the XRCE-DDS WRITE_DATA frame
--  and the serial framing of DDS-XRCE 1.0 Annex C.1 (addresses, length, byte
--  stuffing, RFC 1662 FCS-16) an endpoint uses over a UART / RS-485 link.
--  Byte-identical to endpoints/c (`zerodds_endpoint.c`) and the Rust
--  `zerodds-xrce` transport.

with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Native_Framing is

   XRCE_SM_Write_Data : constant := 16#07#;  --  endpoint -> hub
   XRCE_SM_Data       : constant := 16#09#;  --  hub -> endpoint
   --  E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits
   --  1..3 -- DDS-XRCE 1.0 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 16#03#) would
   --  additionally require a SampleInfo in the body, which this profile does
   --  not carry.
   XRCE_Write_Flags   : constant := 16#01#;
   --  Wire size of the BaseObjectRequest preceding the sample (7.7.8).
   XRCE_Base_Size     : constant := 4;
   --  OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
   Objk_DataWriter    : constant := 16#05#;
   XRCE_Flag          : constant := 16#7E#;
   XRCE_Esc           : constant := 16#7D#;
   XRCE_Stuff         : constant := 16#20#;

   --  A sample the 16-bit XRCE submessage_length cannot describe.
   --
   --  The bound was never written down here. `Unsigned_16 (Sample'Length)`
   --  below does raise Constraint_Error for a sample above 65535, so this
   --  binding was not silent -- but only while checks are on, and the reason
   --  never reached the caller: `-gnatp` suppresses the check and the
   --  conversion then wraps, which writes a length the frame does not carry.
   --  The bound is stated instead of relied upon.
   Sample_Too_Large : exception;

   --  The frame does not fit the writer's `Max_Buffer` (4096 octets).
   --
   --  `Put_Bytes` sets `W.Overflow` and returns, which left the eight-octet
   --  header behind with a correct length field and no body at all for every
   --  sample from 4089 octets up, and `Bytes (W)` handed those eight octets
   --  back as a frame. The same shape as `Zerodds_Ada83_Endpoint`'s, one
   --  buffer size over.
   Frame_Does_Not_Fit : exception;

   --  An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
   function Xrce_Object_Id
     (Raw_Id : Unsigned_16; Kind : Unsigned_16) return Unsigned_16;

   --  An XRCE WRITE_DATA message: 4-byte header (session, stream, seq LE u16) +
   --  4-byte submessage header (id, flags, length LE u16) + the
   --  BaseObjectRequest (7.7.8: request_id then object_id, each two octets most
   --  significant first) + the sample bytes. `Object_Id` names the target
   --  DataWriter -- without it no XRCE agent can route the sample; the
   --  request_id is the message's own `Seq`, which 7.7.8 only requires to be
   --  unique per stream. Raises Sample_Too_Large when the body exceeds 65535,
   --  and Frame_Does_Not_Fit when the frame does not fit Max_Buffer.
   function Xrce_Write_Frame
     (Session   : Byte;
      Stream    : Byte;
      Seq       : Unsigned_16;
      Object_Id : Unsigned_16;
      Sample    : Byte_Array) return Byte_Array;

   --  Locate the sample body inside a received WRITE_DATA (0x07) or DATA (0x09)
   --  frame. The 8-byte envelope (header + submessage header) AND the 4-octet
   --  BaseObjectRequest (7.7.8) are skipped;
   --  `Body_First .. Body_Last` index into `Frame` (empty body -> Body_Last =
   --  Body_First - 1). The body is bounded by the 16-bit little-endian
   --  submessage_length (bytes 6..7), not by Frame'Last, so a trailing appended
   --  submessage is not folded in. `Valid` is False if the frame is too short,
   --  is not a WRITE_DATA/DATA submessage, declares a length that runs past the
   --  datagram (truncation / wrong length), or declares a body too short to
   --  hold the BaseObjectRequest.
   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Natural;
      Body_Last  : out Integer;
      Valid      : out Boolean);

   --  FCS-16 of RFC 1662 (polynomial x^16 + x^12 + x^5 + 1, reflected 0x8408,
   --  init 0xFFFF, complemented); 16#906E# for "123456789".
   function Fcs16 (Data : Byte_Array) return Unsigned_16;

   --  Annex C.1.1 serial frame from Source to Remote: FLAG, then stuffed
   --  Source, Remote, payloadLength (little endian), Payload and the FCS-16
   --  of those (little endian). Raises Frame_Does_Not_Fit for a frame past
   --  Max_Buffer.
   function Serial_Frame
     (Source, Remote : Byte; Payload : Byte_Array) return Byte_Array;

end Native_Framing;
