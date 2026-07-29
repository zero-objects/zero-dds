-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reliable Ada endpoint example — the async-decoupled writer in action.
--  The producer (main) only enqueues into the protected Send_Ring (wait-free);
--  a dedicated drain task owns the Sender_State + the UDP socket and does all
--  I/O: sends WRITE_DATA, emits HEARTBEAT, and on ACKNACK retransmits the
--  still-in-flight samples until the window drains. This is the many-core /
--  HPX pattern: the aggregator never enters the kernel.
--
--  Usage:  example_reliable <peer_port>     -- reliable sender, 12 samples
--          example_reliable bench           -- producer-latency micro-bench

with Ada.Command_Line;  use Ada.Command_Line;
with Ada.Text_IO;       use Ada.Text_IO;
with Ada.Real_Time;     use Ada.Real_Time;
with Ada.Streams;
with GNAT.Sockets;      use GNAT.Sockets;
with Interfaces;        use Interfaces;
with Interfaces.C;
with Deep_Reading;      use Deep_Reading;
with Reliable;          use Reliable;

procedure Example_Reliable is

   N_Samples : constant := 12;

   function To_SEA (FS : Frame_Store) return Ada.Streams.Stream_Element_Array is
      use Ada.Streams;
      SEA : Stream_Element_Array (1 .. Stream_Element_Offset (FS.Len));
   begin
      for I in 0 .. FS.Len - 1 loop
         SEA (Stream_Element_Offset (I + 1)) := Stream_Element (FS.Data (I));
      end loop;
      return SEA;
   end To_SEA;

   --  A telemetry sample carrying its index in Id (so gap-free delivery is
   --  observable on the peer side).
   function Sample (I : Natural) return Frame_Store is
      Rd : Reading;
   begin
      Rd.Id := Interfaces.C.unsigned_long (I);
      Rd.Value := Interfaces.C.C_float (Float (I) + 0.5);
      Set_Label (Rd, "reliable");
      return Marshal (Rd);
   end Sample;

   -----------------------------------------------------------------
   --  Latency micro-bench: producer enqueue->return (decoupled) vs a full
   --  inline frame+sendto (what the aggregator would pay without decoupling).
   -----------------------------------------------------------------
   procedure Bench is
      M    : constant := 20_000;
      Ring : Send_Ring;
      P    : constant Frame_Store := Sample (1);
      Ok   : Boolean;
      Sock : Socket_Type;
      Peer : Sock_Addr_Type;
      T0, T1, T2 : Time;
      Last : Ada.Streams.Stream_Element_Offset;
      Seq  : Reliable.Seq_Type := 0;
   begin
      --  decoupled: pure enqueue cost (drain never started; ring big enough)
      T0 := Clock;
      for I in 1 .. M loop
         Ring.Enqueue (P, Ok);
         if not Ok then
            declare G : Boolean; Q : Frame_Store; begin
               Ring.Dequeue (Q, G);  -- keep space; not counted meaningfully
            end;
            Ring.Enqueue (P, Ok);
         end if;
      end loop;
      T1 := Clock;

      --  inline: build the reliable frame + a real sendto per sample
      Create_Socket (Sock, Family_Inet, Socket_Datagram);
      Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), 9);  -- discard
      for I in 1 .. M loop
         Send_Socket (Sock, To_SEA (Write_Frame (Seq, P)), Last, Peer);
         Seq := Seq + 1;
      end loop;
      T2 := Clock;
      Close_Socket (Sock);

      Put_Line ("LATENCY decoupled_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (T1 - T0)) * 1.0e9 / Float (M))) &
        " inline_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (T2 - T1)) * 1.0e9 / Float (M))));
   end Bench;

   -----------------------------------------------------------------
   --  Reliable send against the peer on <Port>.
   -----------------------------------------------------------------
   procedure Run (Port : Natural) is
      Ring  : Send_Ring;
      Start : constant Time := Clock;

      function Now_Ms return Long_Integer is
      begin
         return Long_Integer (Float (To_Duration (Clock - Start)) * 1000.0);
      end Now_Ms;

      task Drain;
      task body Drain is
         Sock  : Socket_Type;
         Peer  : Sock_Addr_Type;
         S     : Sender_State;
         P, Fr : Frame_Store;
         Got, Have, Has, AOk : Boolean;
         Seq   : Reliable.Seq_Type;
         First, HF, HL : Reliable.Seq_Type;
         Bitmap : Unsigned_16;
         Last  : Ada.Streams.Stream_Element_Offset;

         procedure Send (F : Frame_Store) is
         begin
            Send_Socket (Sock, To_SEA (F), Last, Peer);
         end Send;

         procedure Try_Recv (F : out Frame_Store; OkR : out Boolean) is
            Buf  : Ada.Streams.Stream_Element_Array (1 .. 512);
            Lst  : Ada.Streams.Stream_Element_Offset;
            From : Sock_Addr_Type;
         begin
            F := (Data => [others => 0], Len => 0);
            OkR := False;
            Receive_Socket (Sock, Buf, Lst, From);
            F.Len := Natural (Lst);
            for I in 1 .. Lst loop
               F.Data (Natural (I) - 1) := Byte (Buf (I));
            end loop;
            OkR := Natural (Lst) > 0;
         exception
            when Socket_Error =>
               OkR := False;  -- receive timeout / would-block
         end Try_Recv;

      begin
         Create_Socket (Sock, Family_Inet, Socket_Datagram);
         Bind_Socket (Sock, (Family_Inet, Any_Inet_Addr, 0));
         Set_Socket_Option (Sock, Socket_Level, (Receive_Timeout, 0.02));
         Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), Port_Type (Port));

         loop
            --  1) window backpressure: only pull a new sample if room in-flight
            if In_Flight_Count (S) < Window then
               Ring.Dequeue (P, Got);
               if Got then
                  Submit (S, P, Seq, AOk);
                  if AOk then
                     Send (Write_Frame (Seq, P));
                  end if;
               end if;
            else
               Got := False;
            end if;

            --  2) process an ACKNACK, retransmit still-in-flight (missing)
            Try_Recv (Fr, Have);
            if Have then
               Parse_Acknack (Fr, AOk, First, Bitmap);
               if AOk then
                  Recv_Acknack (S, First, Bitmap);
                  for I in S.Slots'Range loop
                     if S.Slots (I).Used then
                        Send (Write_Frame (S.Slots (I).Seq, S.Slots (I).Payload));
                     end if;
                  end loop;
               end if;
            end if;

            --  3) periodic HEARTBEAT
            Pending_Heartbeat (S, Now_Ms, Has, HF, HL);
            if Has then
               Send (Heartbeat_Frame (HF, HL));
            end if;

            --  4) done when producer closed the ring and everything is acked
            exit when Ring.Is_Closed and then Ring.Pending = 0
              and then In_Flight_Count (S) = 0;

            if not Got and not Have and not Has then
               delay 0.001;
            end if;
         end loop;
         Close_Socket (Sock);
      end Drain;

      Ok : Boolean;
   begin
      --  producer: enqueue N samples wait-free, then close the ring
      for I in 0 .. N_Samples - 1 loop
         loop
            Ring.Enqueue (Sample (I), Ok);
            exit when Ok;
            delay 0.0001;  -- ring full: local backpressure
         end loop;
      end loop;
      Ring.Close;
      --  block end waits for Drain to terminate (window drained)
   end Run;

begin
   if Argument_Count >= 1 and then Argument (1) = "bench" then
      Bench;
   elsif Argument_Count >= 1 then
      Run (Natural'Value (Argument (1)));
      Put_Line ("ADA RELIABLE OK" & Natural'Image (N_Samples));
   else
      Put_Line ("usage: example_reliable <peer_port> | bench");
   end if;
end Example_Reliable;
