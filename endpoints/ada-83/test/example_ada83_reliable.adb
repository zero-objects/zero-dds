-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Reliable-stream UDP sender app for the live E2E (crates/endpoint-e2e). Plays
--  the reliable SENDER against the shared Rust ReliablePeer: submit N samples on
--  stream 0x80, WRITE_DATA each, then loop { HEARTBEAT; on ACKNACK recv_acknack
--  + retransmit the still-missing set bits } until the send window drains.
--
--  This is the driver-app half of the Ada-83 split (mirrors endpoints/c's
--  reliable_udp_app.c): the strict-Ada-83 reliability CORE
--  (Zerodds_Ada83_Reliable) carries no I/O, so this app owns the transport. Ada
--  83 has no sockets, so -- exactly like the C app is built with the default gcc
--  std rather than the strict-C89 Makefile -- this main is compiled with a
--  relaxed Ada standard (GNAT.Sockets, Ada.Command_Line, Ada.Real_Time are not
--  Ada 83) while the reliability core it calls stays -gnat83.
--
--  usage: example_ada83_reliable <port> [count]

with Ada.Command_Line;        use Ada.Command_Line;
with Ada.Text_IO;             use Ada.Text_IO;
with Ada.Real_Time;           use Ada.Real_Time;
with Ada.Streams;
with GNAT.Sockets;            use GNAT.Sockets;
with Zerodds_Ada83_Wire;      use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Reliable;  use Zerodds_Ada83_Reliable;

procedure Example_Ada83_Reliable is

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

   --  Sample body = sample index as a u32 little-endian (so the peer observes
   --  gap-free, in-order delivery). Byte-for-byte what the C app sends.
   function Sample (I : Natural) return Frame_Store is
      W  : Writer;
      FS : Frame_Store;
   begin
      Init (W, Little);
      Put_U32 (W, Long_Integer (I));
      FS.Len := Length (W);
      for K in 0 .. Length (W) - 1 loop
         FS.Data (K) := W.Buf (K);
      end loop;
      return FS;
   end Sample;

   Sock   : Socket_Type;
   Peer   : Sock_Addr_Type;
   S      : Sender_State;
   Port   : Natural;
   Count  : Natural := 12;
   Seq    : Seq_Type;
   Ok     : Boolean;
   Last   : Ada.Streams.Stream_Element_Offset;

   procedure Send (F : Frame_Store) is
   begin
      Send_Socket (Sock, To_SEA (F), Last, Peer);
   end Send;

   --  Non-blocking (timeout-bounded) receive of one datagram into F.
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

begin
   if Argument_Count < 1 then
      Put_Line (Standard_Error, "usage: example_ada83_reliable <port> [count]");
      Set_Exit_Status (2);
      return;
   end if;
   Port := Natural'Value (Argument (1));
   if Argument_Count >= 2 then
      Count := Natural'Value (Argument (2));
   end if;
   if Count < 1 or else Count > Window then
      Count := 12;
   end if;

   Create_Socket (Sock, Family_Inet, Socket_Datagram);
   Bind_Socket (Sock, (Family_Inet, Any_Inet_Addr, 0));
   Set_Socket_Option (Sock, Socket_Level, (Receive_Timeout, 0.05));
   Peer := (Family_Inet, Inet_Addr ("127.0.0.1"), Port_Type (Port));

   --  submit + transmit N samples
   for I in 0 .. Count - 1 loop
      declare
         Body_FS : constant Frame_Store := Sample (I);
      begin
         Submit (S, Body_FS, Seq, Ok);
         if not Ok then
            Put_Line (Standard_Error, "submit" & Integer'Image (I) & " failed");
            Set_Exit_Status (1);
            return;
         end if;
         Send (Write_Frame (Seq, Body_FS));
      end;
   end loop;

   --  recover: HEARTBEAT + ACKNACK-driven retransmit until the window drains.
   declare
      Deadline : constant Long_Integer := Now_Ms + 15_000;
      Fr       : Frame_Store;
      Have, Has, AOk, GOk : Boolean;
      First, HF, HL : Seq_Type;
      Bitmap   : Bitmap_Type;
      PL       : Frame_Store;
   begin
      while In_Flight_Count (S) > 0 and then Now_Ms < Deadline loop
         Pending_Heartbeat (S, Now_Ms, Has, HF, HL);
         if Has then
            Send (Heartbeat_Frame (HF, HL));
         end if;

         Try_Recv (Fr, Have);
         if Have then
            Parse_Acknack (Fr, AOk, First, Bitmap);
            if AOk then
               Recv_Acknack (S, First, Bitmap);
               for B in 0 .. 15 loop
                  if Nack_Bit (Bitmap, B) then
                     declare
                        Target : constant Seq_Type := (First + B) mod 65536;
                     begin
                        Get_In_Flight (S, Target, PL, GOk);
                        if GOk then
                           Send (Write_Frame (Target, PL));
                        end if;
                     end;
                  end if;
               end loop;
            end if;
         end if;
      end loop;
   end;

   if In_Flight_Count (S) = 0 then
      Put_Line ("reliable sender: all" & Integer'Image (Count)
                & " samples acknowledged");
      Close_Socket (Sock);
   else
      Put_Line (Standard_Error, "reliable sender: window not drained ("
                & Integer'Image (In_Flight_Count (S)) & " left)");
      Close_Socket (Sock);
      Set_Exit_Status (1);
   end if;
end Example_Ada83_Reliable;
