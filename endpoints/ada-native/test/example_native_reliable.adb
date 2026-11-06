-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reliable pure-Ada endpoint example -- the async-decoupled writer in action.
--  The producer (main) only enqueues into the protected Send_Ring (wait-free);
--  a dedicated drain task owns the Sender_State + the UDP socket and does all
--  I/O: sends WRITE_DATA, emits HEARTBEAT, and on ACKNACK retransmits the
--  still-in-flight samples until the window drains. Byte-identical wire to
--  endpoints/c (zerodds_reliable_async.c) and crates/xrce.
--
--  Usage:  example_native_reliable <peer_port>   -- reliable sender, 12 samples
--          example_native_reliable bench         -- producer-latency micro-bench

with Ada.Command_Line;    use Ada.Command_Line;
with Ada.Text_IO;         use Ada.Text_IO;
with Ada.Real_Time;       use Ada.Real_Time;
with Ada.Streams;
with GNAT.Sockets;        use GNAT.Sockets;
with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;
with Native_Reliable;     use Native_Reliable;
with Native_Reliable_Sample; use Native_Reliable_Sample;

procedure Example_Native_Reliable is

   N_Samples : constant := 12;

   --  The DataWriter this example writes to (ObjectId = raw 16#001# plus
   --  OBJK_DATAWRITER, DDS-XRCE 1.0 7.2.1). It rides in every frame's
   --  BaseObjectRequest.
   Writer_Id : constant Interfaces.Unsigned_16 := 16#0015#;

   function To_SEA (F : Byte_Array) return Ada.Streams.Stream_Element_Array is
      use Ada.Streams;
      SEA : Stream_Element_Array (1 .. Stream_Element_Offset (F'Length));
   begin
      for I in 0 .. F'Length - 1 loop
         SEA (Stream_Element_Offset (I + 1)) := Stream_Element (F (F'First + I));
      end loop;
      return SEA;
   end To_SEA;

   -----------------------------------------------------------------
   --  Latency micro-bench (reliable-endpoint 1.0 §5 point 4): the producer's
   --  enqueue into the Send_Ring against the inline frame + sendto the
   --  producer pays without decoupling. Only the enqueues are timed: each
   --  round fills the ring once and empties it untimed, so no enqueue meets
   --  backpressure.
   -----------------------------------------------------------------
   procedure Bench is
      Rounds : constant := 80;
      M      : constant := Rounds * (Ring_Cap - 1);
      Ring   : Send_Ring;
      P      : constant Payload := Sample (1);
      Q      : Payload;
      Ok, Got : Boolean;
      Sock   : Socket_Type;
      Peer   : Sock_Addr_Type;
      Enqueued : Time_Span := Time_Span_Zero;
      T0, T1 : Time;
      Last   : Ada.Streams.Stream_Element_Offset;
      Seq    : Native_Reliable.Seq_Type := 0;
   begin
      for R in 1 .. Rounds loop
         T0 := Clock;
         for I in 1 .. Ring_Cap - 1 loop
            Ring.Enqueue (P, Ok);
            if not Ok then
               raise Program_Error with "enqueue met a full ring";
            end if;
         end loop;
         Enqueued := Enqueued + (Clock - T0);
         loop
            Ring.Dequeue (Q, Got);
            exit when not Got;
         end loop;
      end loop;

      Create_Socket (Sock, Family_Inet, Socket_Datagram);
      Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), 9);  --  discard
      T0 := Clock;
      for I in 1 .. M loop
         Send_Socket (Sock, To_SEA (Write_Frame (Seq, Writer_Id, P)), Last, Peer);
         Seq := Seq + 1;
      end loop;
      T1 := Clock;
      Close_Socket (Sock);

      Put_Line ("LATENCY decoupled_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (Enqueued)) * 1.0e9 / Float (M))) &
        " inline_ns=" &
        Long_Integer'Image (Long_Integer (Float (To_Duration (T1 - T0)) * 1.0e9 / Float (M))));
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
         P     : Payload;
         Fr    : Byte_Array (0 .. Max_Buffer - 1);
         FrLen : Natural;
         Got, Have, Has, AOk : Boolean;
         Seq   : Native_Reliable.Seq_Type;
         First, HF, HL : Native_Reliable.Seq_Type;
         Bitmap : Unsigned_16;
         Last  : Ada.Streams.Stream_Element_Offset;

         procedure Send (F : Byte_Array) is
         begin
            Send_Socket (Sock, To_SEA (F), Last, Peer);
         end Send;

         procedure Try_Recv (OkR : out Boolean) is
            Buf  : Ada.Streams.Stream_Element_Array (1 .. 512);
            Lst  : Ada.Streams.Stream_Element_Offset;
            From : Sock_Addr_Type;
         begin
            FrLen := 0;
            OkR := False;
            Receive_Socket (Sock, Buf, Lst, From);
            FrLen := Natural (Lst);
            for I in 1 .. Lst loop
               Fr (Natural (I) - 1) := Byte (Buf (I));
            end loop;
            OkR := Natural (Lst) > 0;
         exception
            when Socket_Error =>
               OkR := False;  --  receive timeout / would-block
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
                     Send (Write_Frame (Seq, Writer_Id, P));
                  end if;
               end if;
            else
               Got := False;
            end if;

            --  2) process an ACKNACK, retransmit still-in-flight (missing)
            Try_Recv (Have);
            if Have then
               Parse_Acknack (Fr (0 .. FrLen - 1), AOk, First, Bitmap);
               if AOk then
                  Recv_Acknack (S, First, Bitmap);
                  for I in S.Slots'Range loop
                     if S.Slots (I).Used then
                        Send (Write_Frame (S.Slots (I).Seq, Writer_Id, S.Slots (I).Data));
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
            delay 0.0001;  --  ring full: local backpressure
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
      Put_Line ("ADA NATIVE RELIABLE OK" & Natural'Image (N_Samples));
   else
      Put_Line ("usage: example_native_reliable <peer_port> | bench");
   end if;
end Example_Native_Reliable;
