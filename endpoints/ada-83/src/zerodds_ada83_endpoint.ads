-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 endpoint framing (ADR 0013). XRCE WRITE_DATA framing, all
--  procedural (Ada 83 has no tagged types / access-to-subprogram), so the async
--  model is POLL-based: the application drives a non-blocking receive loop and
--  decodes each frame itself -- no callbacks.

with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;

package Zerodds_Ada83_Endpoint is

   --  A sample the 16-bit XRCE submessage_length cannot describe.
   --
   --  The field holds at most 65535, and this procedure used to write
   --  `Len mod 256` and `(Len / 256) mod 256` into it and then append the
   --  whole of `Sample` regardless -- the one outcome every other SDK's
   --  comment says it refuses, and the only one of the fifteen that had no
   --  bound at all. A 65536-octet sample produced a header claiming zero and
   --  a body of 65536, which a peer reads as an empty sample followed by
   --  65536 octets of the next message.
   Sample_Too_Large : exception;

   --  The frame does not fit the writer's `Max_Buffer` (2048 octets).
   --
   --  `Put_Bytes` sets `W.Overflow` and returns, which left the eight-octet
   --  header behind with a correct length field and no body at all for every
   --  sample from 2041 octets up. The flag was there to be read and no caller
   --  read one.
   Frame_Does_Not_Fit : exception;

   --  Wire size of the BaseObjectRequest preceding the sample (7.7.8).
   Xrce_Base_Size : constant := 4;
   --  OBJK_DATAWRITER -- the ObjectKind of a DataWriter (7.2.2).
   Objk_DataWriter : constant := 16#05#;

   --  An ObjectId (7.2.1) from a 12-bit raw id and a 4-bit ObjectKind.
   function Xrce_Object_Id
     (Raw_Id : Long_Integer; Kind : Long_Integer) return Long_Integer;

   --  Frame `Sample` (an XCDR body) as an XRCE WRITE_DATA message into W: the
   --  submessage body is the BaseObjectRequest (7.7.8: request_id then
   --  object_id, each two octets most significant first) followed by the
   --  sample. `Object_Id` names the target DataWriter -- without it no XRCE
   --  agent can route the sample; the request_id is the message's own `Seq`,
   --  which 7.7.8 only requires to be unique per stream.
   --  Raises Sample_Too_Large when the body would exceed 65535, and
   --  Frame_Does_Not_Fit when the writer overflowed.
   procedure Xrce_Write_Frame
     (W         : in out Writer;
      Session   : Long_Integer;
      Stream    : Long_Integer;
      Seq       : Long_Integer;
      Object_Id : Long_Integer;
      Sample    : Byte_Array);

   --  Locate the sample body inside a received WRITE_DATA frame -- the bytes
   --  after the 4-octet BaseObjectRequest. Valid is False if the frame is too
   --  short, not a WRITE_DATA submessage, or declares a body too short to hold
   --  the BaseObjectRequest.
   procedure Xrce_Read_Frame
     (Frame      : Byte_Array;
      Body_First : out Integer;
      Body_Last  : out Integer;
      Valid      : out Boolean);

end Zerodds_Ada83_Endpoint;
