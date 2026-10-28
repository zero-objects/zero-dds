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

   Checks_Failed : exception;

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

   --  Static storage: 70000 octets is past a default stack.
   Huge_Sample : constant Byte_Array (0 .. 69_999) := (others => 0);

   type Id_Store is array (0 .. N - 1) of Long_Integer;
   Got_Ids : Id_Store;
   Got     : Integer := 0;
   Ok_All  : Boolean := True;

   --  The DataWriter every frame in this test addresses (7.2.1: raw 16#001#
   --  plus OBJK_DATAWRITER).
   Writer_Id : constant Long_Integer :=
     Xrce_Object_Id (16#001#, Objk_DataWriter);

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
           (Frame_W, 16#80#, 16#01#, Long_Integer (I), Writer_Id,
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

   --  A frame the writer cannot hold. `Put_Bytes` sets `W.Overflow` and
   --  returns, so an oversize sample used to leave the eight-octet header
   --  behind -- a correct length field, no body -- and `Length (W)` reported
   --  8. The producer loop above never read the flag, and neither did any
   --  other caller in the tree.
   declare
      Big_W : Writer;
      Big   : constant Byte_Array (0 .. 4095) := (others => 0);
   begin
      Init (Big_W, Little);
      Xrce_Write_Frame (Big_W, 16#80#, 16#01#, 1, Writer_Id, Big);
      Text_IO.Put_Line ("a frame past Max_Buffer was not refused");
      Ok_All := False;
   exception
      when Frame_Does_Not_Fit =>
         Text_IO.Put_Line ("negative: a frame past Max_Buffer refused");
   end;

   --  The other bound, one the writer's buffer can never reach on its own:
   --  the 16-bit submessage_length field. `Len mod 256` and `(Len / 256) mod
   --  256` described 65536 as zero and `Put_Bytes` appended the sample
   --  regardless, so the frame claimed a length it did not carry. This was
   --  the only one of the fifteen bindings with no bound at all.
   declare
      Huge_W : Writer;
   begin
      Init (Huge_W, Little);
      Xrce_Write_Frame (Huge_W, 16#80#, 16#01#, 1, Writer_Id, Huge_Sample);
      Text_IO.Put_Line ("a sample above 65535 was not refused");
      Ok_All := False;
   exception
      when Sample_Too_Large =>
         Text_IO.Put_Line ("negative: sample above 65535 refused");
   end;

   if Ok_All then
      Text_IO.Put_Line ("Ada 83 poll-based async:" & Integer'Image (N)
                        & " samples framed + polled + decoded in order");
      Text_IO.Put_Line ("ALL OK");
   else
      Text_IO.Put_Line ("FAILED");
      --  See test_ada83_identity: printing a failure and exiting 0 makes
      --  the check invisible to `make test`.
      raise Checks_Failed;
   end if;
end Test_Ada83_Async;
