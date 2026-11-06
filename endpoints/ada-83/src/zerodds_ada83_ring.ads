-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Single-producer single-consumer ring of Frame_Store, the queue of the Ada 83
--  async-decoupled writer (reliable-endpoint 1.0 §2): the producer enqueues
--  without a rendezvous and without a system call, the drain task dequeues.
--
--  Strict Ada 83 (-gnat83): no protected objects. The two indices and the
--  closed flag are shared variables (Ada 83 RM 9.11, pragma SHARED), so every
--  read and every update of them is a synchronization point between the two
--  tasks: a slot written before the producer advances Tail is complete when the
--  consumer sees the new Tail. One producer task and one consumer task.

with Zerodds_Ada83_Reliable; use Zerodds_Ada83_Reliable;

generic
package Zerodds_Ada83_Ring is

   --  One slot stays free to tell a full ring from an empty one.
   Ring_Cap : constant := 256;

   --  Ok=False when the ring is full: backpressure, the producer decides.
   procedure Enqueue (P : Frame_Store; Ok : out Boolean);
   --  Got=False when the ring is empty.
   procedure Dequeue (P : out Frame_Store; Got : out Boolean);
   --  The producer is done; what is queued stays to be dequeued.
   procedure Close;
   function Is_Closed return Boolean;
   function Is_Empty return Boolean;

end Zerodds_Ada83_Ring;
