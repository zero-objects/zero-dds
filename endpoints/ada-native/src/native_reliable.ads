-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reliable XRCE stream (stream_id >= 128, DDS-XRCE spec §8.4.10/§8.4.11) for
--  the **pure-Ada** endpoint SDK (ADR 0013 Stage 2, no C / no FFI). The wire
--  frames and the state machine are byte-identical to `endpoints/c`
--  (`zerodds_reliable_async.c` + `zdw_xrce_*_frame`) and to `crates/xrce`
--  (`crates/xrce/src/reliable.rs`). This mirrors the procedural `endpoints/ada`
--  `Reliable` package, re-expressed over the native `Byte_Array` wire-core.
--
--  The RFC-1982 16-bit sequence comparison `Seq_Lt` is the base of the window:
--  HEARTBEAT first/last and ACKNACK pruning are computed by modular comparison,
--  not by numeric ordering, so the window is correct across the 0xFFFF wrap.
--
--  Async-decoupled writer: the producer enqueues into the protected `Send_Ring`
--  wait-free (no syscall) and a dedicated drain task owns the `Sender_State`,
--  frames WRITE_DATA, emits HEARTBEAT, and retransmits on ACKNACK.

with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;

package Native_Reliable is

   Window          : constant := 16;      --  in-flight cap == 16-bit ACKNACK bitmap
   Recv_Buffer     : constant := 64;       --  out-of-order reorder cap (DoS bound)
   Payload_Cap     : constant := 256;      --  per-sample store (== ZDW_RING_SLOTCAP)
   Max_Payload     : constant := Payload_Cap;
   Heartbeat_Ms    : constant := 500;      --  spec 100ms; 500ms w/o Tx pacing
   Session_Nokey   : constant Byte := 16#80#;
   Reliable_Stream : constant Byte := 16#80#;

   subtype Seq_Type is Interfaces.Unsigned_16;

   --  A bounded sample body (no heap; an endpoint serializes into fixed space).
   type Payload is record
      Data : Byte_Array (0 .. Payload_Cap - 1) := [others => 0];
      Len  : Natural := 0;
   end record;

   function Make_Payload (Data : Byte_Array) return Payload
     with Pre => Data'Length <= Payload_Cap;

   --  RFC-1982 16-bit sequence comparison: True iff A < B (modular window).
   function Seq_Lt (A, B : Seq_Type) return Boolean;

   ------------------------------------------------------------------
   --  Wire frames (little-endian, byte-identical to the reference)
   ------------------------------------------------------------------
   --  WRITE_DATA id=0x07 flags=0x03; the stream-header seq carries the sample's
   --  RFC-1982 sequence number.
   function Write_Frame (Seq : Seq_Type; Body_P : Payload) return Byte_Array;
   --  Deframe a reliable WRITE_DATA -> (Seq, body). Valid=False if not 0x07.
   procedure Read_Frame
     (F : Byte_Array; Seq : out Seq_Type; Body_P : out Payload; Valid : out Boolean);

   --  ACKNACK id=0x0A flags=0x01, body(5)= first(i16 LE)·nack[0]·nack[1]·stream.
   --  The header uses the XRCE control convention (stream NONE, msg seq) so the
   --  bytes match the reference golden; the target stream is in the body.
   function Acknack_Frame
     (First_Unacked : Seq_Type; Bitmap : Interfaces.Unsigned_16;
      Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1) return Byte_Array;
   procedure Parse_Acknack
     (F : Byte_Array; Ok : out Boolean;
      First_Unacked : out Seq_Type; Bitmap : out Interfaces.Unsigned_16);

   --  HEARTBEAT id=0x0B flags=0x01, body(5)= first(i16 LE)·last(i16 LE)·stream.
   function Heartbeat_Frame
     (First_Unacked, Last_Unacked : Seq_Type;
      Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1) return Byte_Array;
   procedure Parse_Heartbeat
     (F : Byte_Array; Ok : out Boolean;
      First_Unacked, Last_Unacked : out Seq_Type);

   ------------------------------------------------------------------
   --  A window slot (in-flight for the sender, reorder for the receiver).
   ------------------------------------------------------------------
   type Slot is record
      Used    : Boolean := False;
      Seq     : Seq_Type := 0;
      Data    : Payload;
   end record;
   type Slot_Array is array (Natural range <>) of Slot;

   ------------------------------------------------------------------
   --  Sender  (mirrors submit / pending_heartbeat / recv_acknack / get_in_flight)
   ------------------------------------------------------------------
   type Sender_State is record
      Next_Seq   : Seq_Type := 0;
      Last_Hb_Ms : Long_Integer := -1;
      Slots      : Slot_Array (0 .. Window - 1);
   end record;

   --  Assign a seq, buffer the payload. Ok=False if the window is full or the
   --  payload exceeds Max_Payload (Seq undefined then).
   procedure Submit
     (S : in out Sender_State; Data : Payload; Seq : out Seq_Type; Ok : out Boolean);
   function In_Flight_Count (S : Sender_State) return Natural;

   --  Due HEARTBEAT: Has=True + First/Last when in-flight non-empty and the
   --  period elapsed since Last_Hb_Ms (first call always fires). First/Last are
   --  the RFC-1982 window ends (via Seq_Lt). Advances Last_Hb_Ms.
   procedure Pending_Heartbeat
     (S : in out Sender_State; Now_Ms : Long_Integer;
      Has : out Boolean; First, Last : out Seq_Type);

   --  Process an ACKNACK: everything < First_Unacked (RFC-1982) is acked and
   --  removed; within [base, base+16) a clear bit is acked (removed), a set bit
   --  is missing (kept for retransmit).
   procedure Recv_Acknack
     (S : in out Sender_State; First_Unacked : Seq_Type;
      Bitmap : Interfaces.Unsigned_16);

   procedure Get_In_Flight
     (S : Sender_State; Seq : Seq_Type; Data : out Payload; Ok : out Boolean);

   ------------------------------------------------------------------
   --  Receiver  (mirrors recv_data / drain_in_order / pending_acknack / reset)
   ------------------------------------------------------------------
   type Receiver_State is record
      Expected : Seq_Type := 0;
      Slots    : Slot_Array (0 .. Recv_Buffer - 1);
   end record;

   --  Buffer an incoming sample. Ok=False only when the reorder buffer is full.
   --  Duplicates (seq < expected) and already-buffered seqs are silently ok.
   procedure Recv_Data
     (R : in out Receiver_State; Seq : Seq_Type; Data : Payload; Ok : out Boolean);
   function Out_Of_Order_Count (R : Receiver_State) return Natural;

   --  Deliver the next in-order sample if present; advances Expected. Got=False
   --  when the expected seq is not yet buffered.
   procedure Drain_Next
     (R : in out Receiver_State; Seq : out Seq_Type; Data : out Payload; Got : out Boolean);

   --  Bitmap of the missing slots in [Expected, Expected+16).
   function Pending_Acknack (R : Receiver_State) return Interfaces.Unsigned_16;

   procedure Reset (R : in out Receiver_State);

   ------------------------------------------------------------------
   --  Async-decoupled writer: the producer enqueues (wait-free while not full),
   --  a dedicated drain task dequeues. Backpressure = ring full.
   ------------------------------------------------------------------
   Ring_Cap : constant := 256;
   type Ring_Array is array (0 .. Ring_Cap - 1) of Payload;

   protected type Send_Ring is
      --  Wait-free while not full: Ok=False signals backpressure.
      procedure Enqueue (P : Payload; Ok : out Boolean);
      procedure Dequeue (P : out Payload; Got : out Boolean);
      procedure Close;
      function Is_Closed return Boolean;
      function Pending return Natural;
   private
      Q      : Ring_Array;
      Head   : Natural := 0;
      Tail   : Natural := 0;
      Count  : Natural := 0;
      Closed : Boolean := False;
   end Send_Ring;

end Native_Reliable;
