-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reliable-stream UDP sender app for the live E2E (crates/endpoint-e2e). Plays
--  the reliable SENDER against the shared Rust ReliablePeer through the Ada 83
--  async-decoupled writer (Zerodds_Ada83_Async_Writer, reliable-endpoint 1.0
--  §2): the producer only enqueues N samples into the writer's ring; the drain
--  task sends WRITE_DATA on stream 0x80, HEARTBEAT on the period, and on
--  ACKNACK retransmits the still-missing set bits until the window drains.
--
--  This is the driver-app half of the Ada-83 split: the strict-Ada-83 core and
--  the writer carry no sockets, so this app owns the transport. Ada 83 has no
--  sockets, so this main is compiled with a relaxed Ada standard (GNAT.Sockets,
--  Ada.Command_Line, Ada.Real_Time are not Ada 83) while the core and the
--  writer it instantiates stay -gnat83.
--
--  usage: example_ada83_reliable <port> [count]
--         example_ada83_reliable bench    -- producer-latency micro-bench

with Ada.Command_Line;            use Ada.Command_Line;
with Ada.Text_IO;                 use Ada.Text_IO;
with Ada.Real_Time;               use Ada.Real_Time;
with Ada.Streams;
with GNAT.Sockets;                use GNAT.Sockets;
with Zerodds_Ada83_Wire;          use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Reliable;      use Zerodds_Ada83_Reliable;
with Zerodds_Ada83_Ring;
with Ada83_Reliable_Sample;     use Ada83_Reliable_Sample;
with Zerodds_Ada83_Async_Writer;

procedure Example_Ada83_Reliable is

   --  The DataWriter this example writes to (ObjectId = raw 16#001# plus
   --  OBJK_DATAWRITER, DDS-XRCE 1.0 7.2.1). It rides in every frame's
   --  BaseObjectRequest.
   Writer_Id : constant Integer := 16#0015#;

   Start : constant Time := Clock;

   function Now_Ms return Long_Integer is
   begin
      return Long_Integer (Float (To_Duration (Clock - Start)) * 1000.0);
   end Now_Ms;

   function To_SEA (FS : Frame_Store) return Ada.Streams.Stream_Element_Array is
      use Ada.Streams;
      SEA : Stream_Element_Array (1 .. Stream_Element_Offset (FS.Len));
   begin
      for I in 0 .. FS.Len - 1 loop
         SEA (Stream_Element_Offset (I + 1)) := Stream_Element (FS.Data (I));
      end loop;
      return SEA;
   end To_SEA;

   Sock : Socket_Type;
   Peer : Sock_Addr_Type;
   Last : Ada.Streams.Stream_Element_Offset;

   procedure Send (F : Frame_Store) is
   begin
      Send_Socket (Sock, To_SEA (F), Last, Peer);
   end Send;

   --  One datagram into F, or Have=False after the socket's receive timeout.
   procedure Try_Recv (F : out Frame_Store; Have : out Boolean) is
      Buf  : Ada.Streams.Stream_Element_Array (1 .. 512);
      Lst  : Ada.Streams.Stream_Element_Offset;
      From : Sock_Addr_Type;
   begin
      F.Len := 0;
      Have := False;
      Receive_Socket (Sock, Buf, Lst, From);
      F.Len := Natural (Lst);
      for I in 1 .. Lst loop
         F.Data (Natural (I) - 1) := Byte (Buf (I));
      end loop;
      Have := Natural (Lst) > 0;
   exception
      when Socket_Error =>
         Have := False;  -- receive timeout / would-block
   end Try_Recv;

   -----------------------------------------------------------------
   --  Latency micro-bench (reliable-endpoint 1.0 §5 point 4): the producer's
   --  enqueue into the writer's ring against the inline frame + sendto the
   --  producer pays without decoupling. Only the enqueues are timed: each
   --  round fills the ring once and empties it untimed, so no enqueue meets
   --  backpressure.
   -----------------------------------------------------------------
   procedure Bench is
      package Bench_Ring is new Zerodds_Ada83_Ring;
      Rounds   : constant := 80;
      M        : constant := Rounds * (Bench_Ring.Ring_Cap - 1);
      P        : constant Frame_Store := Sample (1);
      Q        : Frame_Store;
      Ok, Got  : Boolean;
      Enqueued : Time_Span := Time_Span_Zero;
      T0, T1   : Time;
      Seq      : Seq_Type := 0;
   begin
      for R in 1 .. Rounds loop
         T0 := Clock;
         for I in 1 .. Bench_Ring.Ring_Cap - 1 loop
            Bench_Ring.Enqueue (P, Ok);
            if not Ok then
               raise Program_Error with "enqueue met a full ring";
            end if;
         end loop;
         Enqueued := Enqueued + (Clock - T0);
         loop
            Bench_Ring.Dequeue (Q, Got);
            exit when not Got;
         end loop;
      end loop;

      Create_Socket (Sock, Family_Inet, Socket_Datagram);
      Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), 9);  -- discard
      T0 := Clock;
      for I in 1 .. M loop
         Send (Write_Frame (Seq, Writer_Id, P));
         Seq := (Seq + 1) mod 65536;
      end loop;
      T1 := Clock;
      Close_Socket (Sock);

      Put_Line ("LATENCY decoupled_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (Enqueued)) * 1.0e9 / Float (M))) &
        " inline_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (T1 - T0)) * 1.0e9 / Float (M))));
   end Bench;

   -----------------------------------------------------------------
   --  Reliable send of Count samples against the peer on Port.
   -----------------------------------------------------------------
   procedure Run (Port : Natural; Count : Natural; Drained : out Boolean) is
      package Async is new Zerodds_Ada83_Async_Writer
        (Send => Send, Try_Recv => Try_Recv, Now_Ms => Now_Ms, Object_Id => Writer_Id);
      Ok : Boolean;
   begin
      Create_Socket (Sock, Family_Inet, Socket_Datagram);
      Bind_Socket (Sock, (Family_Inet, Any_Inet_Addr, 0));
      Set_Socket_Option (Sock, Socket_Level, (Receive_Timeout, 0.02));
      Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), Port_Type (Port));

      Async.Start;
      --  producer: enqueue the samples, spin locally while the ring is full
      for I in 0 .. Count - 1 loop
         loop
            Async.Enqueue (Sample (I), Ok);
            exit when Ok;
            delay 0.0001;
         end loop;
      end loop;
      Async.Close;
      Async.Wait;
      Drained := Async.Drained;
      Close_Socket (Sock);
   end Run;

   Count   : Natural := 12;
   Drained : Boolean;

begin
   if Argument_Count < 1 then
      Put_Line (Standard_Error, "usage: example_ada83_reliable <port> [count] | bench");
      Set_Exit_Status (2);
      return;
   end if;
   if Argument (1) = "bench" then
      Bench;
      return;
   end if;
   if Argument_Count >= 2 then
      Count := Natural'Value (Argument (2));
   end if;

   Run (Natural'Value (Argument (1)), Count, Drained);
   if Drained then
      Put_Line ("reliable sender: all" & Integer'Image (Count)
                & " samples acknowledged");
   else
      Put_Line (Standard_Error, "reliable sender: window not drained");
      Set_Exit_Status (1);
   end if;
end Example_Ada83_Reliable;
