-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The async-decoupled reliable writer of the Ada 83 endpoint
--  (reliable-endpoint 1.0 §2): the producer only enqueues into a
--  Zerodds_Ada83_Ring; a drain task owns the Sender_State and does all I/O --
--  WRITE_DATA for each dequeued sample, HEARTBEAT on the period, and on ACKNACK
--  the retransmit of the samples still marked missing -- until the producer has
--  closed the ring and the send window has drained.
--
--  Strict Ada 83 (-gnat83), tasking without protected objects. Ada 83 has no
--  sockets, so the transport is the instantiating application's: Send puts one
--  datagram out, Try_Recv returns one received datagram or Have=False once the
--  transport's poll timeout passed, Now_Ms is a monotonic clock.

with Zerodds_Ada83_Reliable; use Zerodds_Ada83_Reliable;

generic
   with procedure Send (F : Frame_Store);
   with procedure Try_Recv (F : out Frame_Store; Have : out Boolean);
   with function Now_Ms return Long_Integer;
   --  The DataWriter of every frame's BaseObjectRequest (DDS-XRCE 1.0 7.7.8).
   Object_Id : in Integer;
   --  How long the drain keeps retransmitting after the producer closed.
   Drain_Deadline_Ms : in Long_Integer := 15_000;
package Zerodds_Ada83_Async_Writer is

   --  The drain task begins; before this call it sends nothing.
   procedure Start;
   --  The producer's write: a copy into the ring, no rendezvous, no system call.
   --  Ok=False when the ring is full. A sample longer than Slot_Cap raises
   --  CONSTRAINT_ERROR rather than being dropped later by the drain.
   procedure Enqueue (P : Frame_Store; Ok : out Boolean);
   --  No more samples.
   procedure Close;
   --  Blocks until the drain task has ended.
   procedure Wait;
   --  True once every sample was acknowledged; False after the drain deadline.
   function Drained return Boolean;

end Zerodds_Ada83_Async_Writer;
