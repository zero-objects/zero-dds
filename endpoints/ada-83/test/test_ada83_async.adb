-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  pre-Object Ada 83 async endpoint test: POLL-based (no callbacks). N samples
--  are framed into an in-memory FIFO; the receive loop dequeues each frame,
--  extracts the XRCE body and decodes it. Proves the Ada-83 endpoint drives an
--  event loop the way legacy Ada 83 code would -- by polling.

with Text_IO;
with Zerodds_Ada83_Wire;     use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Endpoint; use Zerodds_Ada83_Endpoint;

procedure Test_Ada83_Async is

   N     : constant Integer := 5;
   Slots : constant Integer := 16;
   Cap   : constant Integer := 256;

   type Frame_Rec is record
      Data : Byte_Array (0 .. Cap - 1);
      Len  : Integer := 0;
   end record;
   type Frame_Store is array (0 .. Slots - 1) of Frame_Rec;

   Fifo  : Frame_Store;
   Head  : Integer := 0;
   Tail  : Integer := 0;
   Count : Integer := 0;

   type Id_Store is array (0 .. N - 1) of Long_Integer;
   Got_Ids : Id_Store;
   Got     : Integer := 0;
   Ok_All  : Boolean := True;

   procedure Enqueue (Frame : Byte_Array) is
   begin
      Fifo (Tail).Data (0 .. Frame'Length - 1) := Frame;
      Fifo (Tail).Len := Frame'Length;
      Tail := (Tail + 1) mod Slots;
      Count := Count + 1;
   end Enqueue;

begin
   --  Producer: frame N samples (each body = id + tail).
   for I in 0 .. N - 1 loop
      declare
         Body_W  : Writer;
         Frame_W : Writer;
      begin
         Init (Body_W, Little);
         Put_U32 (Body_W, 16#1000# + Long_Integer (I));
         Put_U16 (Body_W, 0);
         Put_U8 (Body_W, 0);
         Init (Frame_W, Little);
         Xrce_Write_Frame
           (Frame_W, 16#80#, 16#01#, Long_Integer (I),
            Body_W.Buf (0 .. Length (Body_W) - 1));
         Enqueue (Frame_W.Buf (0 .. Length (Frame_W) - 1));
      end;
   end loop;

   --  Consumer: poll the FIFO, decode each body's id.
   while Count > 0 loop
      declare
         Fr    : constant Frame_Rec := Fifo (Head);
         BF, BL : Integer;
         Valid : Boolean;
         R     : Reader;
         Id    : Long_Integer;
      begin
         Head := (Head + 1) mod Slots;
         Count := Count - 1;
         Xrce_Read_Frame (Fr.Data (0 .. Fr.Len - 1), BF, BL, Valid);
         if Valid then
            Init_Reader (R, Fr.Data (BF .. BL), Little);
            Get_U32 (R, Id);
            if Got < N then
               Got_Ids (Got) := Id;
               Got := Got + 1;
            end if;
         end if;
      end;
   end loop;

   if Got /= N then
      Text_IO.Put_Line ("received" & Integer'Image (Got) & ", expected"
                        & Integer'Image (N));
      Ok_All := False;
   end if;
   for I in 0 .. N - 1 loop
      if Ok_All and then Got_Ids (I) /= 16#1000# + Long_Integer (I) then
         Text_IO.Put_Line ("sample" & Integer'Image (I) & " out of order");
         Ok_All := False;
      end if;
   end loop;

   if Ok_All then
      Text_IO.Put_Line ("Ada 83 poll-based async:" & Integer'Image (N)
                        & " samples framed + polled + decoded in order");
      Text_IO.Put_Line ("ALL OK");
   else
      Text_IO.Put_Line ("FAILED");
   end if;
end Test_Ada83_Async;
