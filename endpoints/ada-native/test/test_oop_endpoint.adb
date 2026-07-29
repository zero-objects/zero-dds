-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Object-Ada async endpoint test: a concrete FIFO Transport and a Collector
--  Sample_Handler implement the interfaces; an Async_Writer fires N samples,
--  the Async_Reader drains them via dispatching Receive + On_Sample. Proves the
--  OOP reactor dispatches every sample through the tagged-type machinery.

with Ada.Text_IO;             use Ada.Text_IO;
with Ada.Command_Line;        use Ada.Command_Line;
with Interfaces;              use Interfaces;
with Zerodds_Native_Wire;     use Zerodds_Native_Wire;
with Zerodds_Native_Endpoint; use Zerodds_Native_Endpoint;

procedure Test_Oop_Endpoint is

   N : constant := 5;

   --  Concrete interface implementations live in a nested package so their
   --  tagged types + overriding primitives are declared and completed cleanly.
   package Impl is
      Slots     : constant := 16;
      Frame_Cap : constant := 256;

      type Frame_Store is array (0 .. Slots - 1) of Byte_Array (0 .. Frame_Cap - 1);
      type Len_Store is array (0 .. Slots - 1) of Natural;

      type Fifo_Transport is limited new Transport with record
         Store : Frame_Store;
         Len   : Len_Store := [others => 0];
         Head  : Natural := 0;
         Tail  : Natural := 0;
         Count : Natural := 0;
      end record;
      overriding procedure Deliver
        (T : in out Fifo_Transport; Frame : Byte_Array; Ok : out Boolean);
      overriding procedure Receive
        (T      : in out Fifo_Transport;
         Buf    : out Byte_Array;
         Last   : out Natural;
         Status : out Recv_Status);

      type Id_Store is array (0 .. N - 1) of Unsigned_32;
      type Collector is limited new Sample_Handler with record
         Ids : Id_Store := [others => 0];
         Got : Natural := 0;
      end record;
      overriding procedure On_Sample
        (H : in out Collector; Sample_Body : Byte_Array);
   end Impl;

   package body Impl is
      overriding procedure Deliver
        (T : in out Fifo_Transport; Frame : Byte_Array; Ok : out Boolean) is
      begin
         if T.Count = Slots or else Frame'Length > Frame_Cap then
            Ok := False;
            return;
         end if;
         T.Store (T.Tail) (0 .. Frame'Length - 1) := Frame;
         T.Len (T.Tail) := Frame'Length;
         T.Tail := (T.Tail + 1) mod Slots;
         T.Count := T.Count + 1;
         Ok := True;
      end Deliver;

      overriding procedure Receive
        (T      : in out Fifo_Transport;
         Buf    : out Byte_Array;
         Last   : out Natural;
         Status : out Recv_Status) is
      begin
         Buf := [others => 0];
         Last := 0;
         if T.Count = 0 then
            Status := Recv_Again;
            return;
         end if;
         declare
            L : constant Natural := T.Len (T.Head);
         begin
            Buf (0 .. L - 1) := T.Store (T.Head) (0 .. L - 1);
            Last := L - 1;
         end;
         T.Head := (T.Head + 1) mod Slots;
         T.Count := T.Count - 1;
         Status := Recv_Ok;
      end Receive;

      overriding procedure On_Sample
        (H : in out Collector; Sample_Body : Byte_Array)
      is
         R  : Reader;
         Id : Unsigned_32;
      begin
         Init (R, Sample_Body, Little);
         Get_U32 (R, Id);
         if H.Got < N then
            H.Ids (H.Got) := Id;
            H.Got := H.Got + 1;
         end if;
      end On_Sample;
   end Impl;

   function Sample_Of (Id : Unsigned_32) return Byte_Array is
      W : Writer;
   begin
      Init (W, Little);
      Put_U32 (W, Id);
      Put_U16 (W, 0);
      Put_U8 (W, 0);
      return Bytes (W);
   end Sample_Of;

   Fifo     : aliased Impl.Fifo_Transport;
   Handler  : aliased Impl.Collector;
   Reader_O : Async_Reader (Fifo'Access, Handler'Access);
   Writer_O : Async_Writer (Fifo'Access);
   Ok       : Boolean;
   Count    : Natural;

begin
   for I in 0 .. N - 1 loop
      Write (Writer_O, Sample_Of (16#1000# + Unsigned_32 (I)), Ok);
      if not Ok then
         Put_Line ("write" & Integer'Image (I) & " failed");
         Set_Exit_Status (1);
         return;
      end if;
   end loop;

   Run (Reader_O, Count);

   if Count /= N or else Handler.Got /= N then
      Put_Line ("dispatched" & Natural'Image (Count) & " got"
                & Natural'Image (Handler.Got) & ", expected" & Integer'Image (N));
      Set_Exit_Status (1);
      return;
   end if;
   for I in 0 .. N - 1 loop
      if Handler.Ids (I) /= 16#1000# + Unsigned_32 (I) then
         Put_Line ("sample" & Integer'Image (I) & " out of order");
         Set_Exit_Status (1);
         return;
      end if;
   end loop;

   Put_Line ("Object-Ada async endpoint:" & Natural'Image (N)
             & " samples dispatched + decoded in order");
   Put_Line ("ALL OK");
   Set_Exit_Status (0);
end Test_Oop_Endpoint;
