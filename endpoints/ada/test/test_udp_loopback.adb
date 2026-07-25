-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Live UDP loopback E2E for the Ada Stage 1 endpoint (ADR 0013). Encodes the
--  fixed SensorReading, sends it over a real UDP socket on 127.0.0.1, receives
--  it back, decodes and verifies the round-trip -- proving the Ada bindings
--  drive real datagram transport, not just an in-memory buffer.

with Ada.Command_Line; use Ada.Command_Line;
with Ada.Text_IO;      use Ada.Text_IO;
with Ada.Streams;      use Ada.Streams;
with Interfaces.C;     use Interfaces.C;
with GNAT.Sockets;     use GNAT.Sockets;
with Zdw;     use Zdw;
with Sample_Sensor;    use Sample_Sensor;
with Sample_Fixtures;  use Sample_Fixtures;

procedure Test_Udp_Loopback is
   S    : constant Sensor_Reading := Sensor_Fixture;
   Buf  : aliased Stream_Element_Array (0 .. 255) := [others => 0];
   W    : aliased Zdw_Writer;
   Ok   : Boolean := True;
begin
   Writer_Init (W'Access, Buf'Address, Buf'Length, ZDW_LE);
   if Encode (W'Access, S) /= ZDW_OK then
      Put_Line ("encode error" & int'Image (W.Error));
      Set_Exit_Status (1);
      return;
   end if;

   declare
      Sock   : Socket_Type;
      Addr   : Sock_Addr_Type;
      Bound  : Sock_Addr_Type;
      From   : Sock_Addr_Type;
      Send_N : constant Stream_Element_Offset := Stream_Element_Offset (W.Len) - 1;
      Last   : Stream_Element_Offset;
      Recv   : Stream_Element_Array (0 .. 255) := [others => 0];
      R      : aliased Zdw_Reader;
      D      : Sensor_Reading;
   begin
      Create_Socket (Sock, Family_Inet, Socket_Datagram);
      Set_Socket_Option (Sock, Socket_Level, (Reuse_Address, True));
      Addr := (Family_Inet, Loopback_Inet_Addr, Any_Port);
      Bind_Socket (Sock, Addr);
      Bound := Get_Socket_Name (Sock);  -- the ephemeral port actually assigned

      Send_Socket (Sock, Buf (0 .. Send_N), Last,
                   To => (Family_Inet, Loopback_Inet_Addr, Bound.Port));
      Put_Line ("sent" & Stream_Element_Offset'Image (Last + 1)
                & " bytes to 127.0.0.1:" & Port_Type'Image (Bound.Port));

      Receive_Socket (Sock, Recv, Last, From);
      Put_Line ("received" & Stream_Element_Offset'Image (Last + 1) & " bytes");

      Reader_Init (R'Access, Recv'Address, size_t (Last + 1), ZDW_LE);
      if Decode (R'Access, D) /= ZDW_OK then
         Put_Line ("decode error" & int'Image (R.Error));
         Ok := False;
      elsif D.Id /= S.Id or else D.Kind /= S.Kind or else D.Flags /= S.Flags
        or else D.Value /= S.Value or else D.Stamp /= S.Stamp
        or else D.Raw_Len /= S.Raw_Len
        or else D.Raw (0) /= S.Raw (0) or else D.Raw (3) /= S.Raw (3)
      then
         Put_Line ("round-trip mismatch over UDP");
         Ok := False;
      else
         Put_Line ("UDP loopback round-trip ok");
      end if;

      Close_Socket (Sock);
   end;

   if Ok then
      Put_Line ("ALL OK");
      Set_Exit_Status (0);
   else
      Set_Exit_Status (1);
   end if;
end Test_Udp_Loopback;
