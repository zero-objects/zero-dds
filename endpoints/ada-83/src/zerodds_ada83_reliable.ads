-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 reliable XRCE stream (DDS-XRCE 1.0 §8.4.10/§8.4.11), the
--  OLDEST-legacy variant. Strict Ada 83 (-gnat83): no modular types, no
--  Interfaces, no Shift_Left/Shift_Right, no bitwise `and`/`or` on integers, no
--  tagged types, no tasking/protected objects. RFC-1982 16-bit sequence numbers
--  are Integer subtypes and every wrap/bit operation is Long_Integer/Integer
--  div/mod arithmetic, so the wire is byte-identical to the Rust core
--  (crates/xrce), endpoints/c and the modern Ada endpoints.
--
--  State machine only -- no I/O, no sockets. This mirrors the C split
--  (`zerodds_endpoint.c` reliable core vs. `reliable_udp_app.c` POSIX sockets):
--  a driver app owns the transport and the poll loop and calls into this core.
--
--  The window base is the OLDEST unacked sequence, compared with RFC-1982 Seq_Lt
--  (never a numeric min), so HEARTBEAT/ACKNACK stay correct across the 16-bit
--  wrap -- the same rule the modern Ada endpoint uses.

with Zerodds_Ada83_Wire; use Zerodds_Ada83_Wire;

package Zerodds_Ada83_Reliable is

   Window          : constant := 16;      -- in-flight cap == 16-bit ACKNACK bitmap
   Recv_Buffer     : constant := 64;       -- out-of-order reorder cap (DoS bound)
   Slot_Cap        : constant := 512;      -- per-sample payload bound (embedded)
   Frame_Cap       : constant := 600;      -- framed message bound (8 hdr + payload)
   Heartbeat_Ms    : constant := 500;      -- spec 100ms; 500ms without Tx pacing
   Reliable_Stream : constant Byte := 16#80#;

   --  RFC-1982 16-bit sequence numbers and the 16-bit NACK bitmap are held as
   --  plain Integer ranges (Ada 83 has no modular type); arithmetic wraps with
   --  `mod 65536` and bits are read/written with div/mod by powers of two.
   subtype Seq_Type    is Integer range 0 .. 65535;
   subtype Bitmap_Type is Integer range 0 .. 65535;

   --  RFC-1982: True iff A < B on the 16-bit ring (half-window rule).
   function Seq_Lt (A, B : Seq_Type) return Boolean;
   --  True iff bit K (0..15) of Bitmap is set.
   function Nack_Bit (Bitmap : Bitmap_Type; K : Integer) return Boolean;

   ------------------------------------------------------------------
   --  A framed or payload message (fixed storage, no heap).
   ------------------------------------------------------------------
   type Frame_Store is record
      Data : Byte_Array (0 .. Frame_Cap - 1);
      Len  : Integer := 0;
   end record;

   ------------------------------------------------------------------
   --  Wire frames (little-endian, byte-identical to the reference).
   ------------------------------------------------------------------
   --  WRITE_DATA id=0x07 flags=0x03; the 2-byte header sequence carries the
   --  sample's RFC-1982 seq. Session 0x80 (no key), stream 0x80 (reliable).
   function Write_Frame (Seq : Seq_Type; Body_FS : Frame_Store) return Frame_Store;
   --  Deframe a reliable WRITE_DATA -> (Seq, body). Body Len=0 if invalid.
   procedure Read_Frame (F : Frame_Store; Seq : out Seq_Type; Body_FS : out Frame_Store);

   --  ACKNACK id=0x0A flags=0x01, body(5)= first(i16 LE)*nack[0]*nack[1]*stream.
   --  The header uses the XRCE control convention (stream=NONE, message seq) so
   --  the bytes match the reference golden; the target stream is in the body.
   function Acknack_Frame (First_Unacked : Seq_Type; Bitmap : Bitmap_Type;
                           Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                           return Frame_Store;
   procedure Parse_Acknack (F : Frame_Store; Ok : out Boolean;
                            First_Unacked : out Seq_Type; Bitmap : out Bitmap_Type);

   --  HEARTBEAT id=0x0B flags=0x01, body(5)= first(i16 LE)*last(i16 LE)*stream.
   function Heartbeat_Frame (First_Unacked, Last_Unacked : Seq_Type;
                             Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                             return Frame_Store;
   procedure Parse_Heartbeat (F : Frame_Store; Ok : out Boolean;
                              First_Unacked, Last_Unacked : out Seq_Type);

   ------------------------------------------------------------------
   --  A window slot (in-flight for the sender, reorder for the receiver).
   ------------------------------------------------------------------
   type Slot is record
      Used    : Boolean := False;
      Seq     : Seq_Type := 0;
      Payload : Frame_Store;
   end record;
   type Slot_Array is array (Natural range <>) of Slot;

   ------------------------------------------------------------------
   --  Sender (mirrors submit / pending_heartbeat / recv_acknack / get_in_flight).
   ------------------------------------------------------------------
   type Sender_State is record
      Next_Seq   : Seq_Type := 0;
      Last_Hb_Ms : Long_Integer := -1;   -- sentinel: first HEARTBEAT always fires
      Slots      : Slot_Array (0 .. Window - 1);
   end record;

   --  Assign a seq, buffer the payload. Ok=False if the window is full or the
   --  payload exceeds Slot_Cap (Seq undefined then).
   procedure Submit (S : in out Sender_State; Payload : Frame_Store;
                     Seq : out Seq_Type; Ok : out Boolean);
   function In_Flight_Count (S : Sender_State) return Natural;

   --  Due HEARTBEAT: Has=True + First/Last when in-flight is non-empty and the
   --  period elapsed since Last_Hb_Ms (first call always fires). Advances
   --  Last_Hb_Ms. First/Last are the RFC-1982 window base/end (Seq_Lt, not min).
   procedure Pending_Heartbeat (S : in out Sender_State; Now_Ms : Long_Integer;
                                Has : out Boolean; First, Last : out Seq_Type);

   --  Process an ACKNACK: everything < First_Unacked (RFC-1982) is acked and
   --  removed; within [base, base+16) a clear bit is acked (removed), a set bit
   --  is still-missing (kept for retransmit).
   procedure Recv_Acknack (S : in out Sender_State; First_Unacked : Seq_Type;
                           Bitmap : Bitmap_Type);

   procedure Get_In_Flight (S : Sender_State; Seq : Seq_Type;
                            Payload : out Frame_Store; Ok : out Boolean);

   ------------------------------------------------------------------
   --  Receiver (mirrors recv_data / drain_in_order / pending_acknack / reset).
   ------------------------------------------------------------------
   type Receiver_State is record
      Expected : Seq_Type := 0;
      Slots    : Slot_Array (0 .. Recv_Buffer - 1);
   end record;

   --  Buffer an incoming sample. Ok=False only when the reorder buffer is full.
   --  Duplicates (seq < expected) and already-buffered seqs are silently ok.
   procedure Recv_Data (R : in out Receiver_State; Seq : Seq_Type;
                        Payload : Frame_Store; Ok : out Boolean);
   function Out_Of_Order_Count (R : Receiver_State) return Natural;

   --  Deliver the next in-order sample if present; advances Expected. Got=False
   --  when the expected seq is not yet buffered.
   procedure Drain_Next (R : in out Receiver_State; Seq : out Seq_Type;
                         Payload : out Frame_Store; Got : out Boolean);

   --  NACK bitmap of the missing slots in [Expected, Expected+16).
   function Pending_Acknack (R : Receiver_State) return Bitmap_Type;

   procedure Reset (R : in out Receiver_State);

end Zerodds_Ada83_Reliable;
