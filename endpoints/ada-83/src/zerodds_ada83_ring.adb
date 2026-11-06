-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Zerodds_Ada83_Ring is

   type Slot_Ring is array (Integer range 0 .. Ring_Cap - 1) of Frame_Store;

   Slots  : Slot_Ring;
   Head   : Integer := 0;   -- the next slot the consumer reads
   Tail   : Integer := 0;   -- the next slot the producer writes
   Closed : Boolean := False;
   pragma SHARED (Head);
   pragma SHARED (Tail);
   pragma SHARED (Closed);

   procedure Enqueue (P : Frame_Store; Ok : out Boolean) is
      Next : constant Integer := (Tail + 1) mod Ring_Cap;
   begin
      if Next = Head then
         Ok := False;
      else
         Slots (Tail) := P;
         Tail := Next;
         Ok := True;
      end if;
   end Enqueue;

   procedure Dequeue (P : out Frame_Store; Got : out Boolean) is
   begin
      if Head = Tail then
         Got := False;
      else
         P := Slots (Head);
         Head := (Head + 1) mod Ring_Cap;
         Got := True;
      end if;
   end Dequeue;

   procedure Close is
   begin
      Closed := True;
   end Close;

   function Is_Closed return Boolean is
   begin
      return Closed;
   end Is_Closed;

   function Is_Empty return Boolean is
   begin
      return Head = Tail;
   end Is_Empty;

end Zerodds_Ada83_Ring;
