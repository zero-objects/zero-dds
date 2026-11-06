-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Zerodds_Ada83_Ring;

package body Zerodds_Ada83_Async_Writer is

   package Ring is new Zerodds_Ada83_Ring;

   Done : Boolean := False;
   pragma SHARED (Done);

   task Drain is
      entry Start;
      entry Finished;
   end Drain;

   task body Drain is
      S        : Sender_State;
      P, Fr, PL : Frame_Store;
      Got, Have, Has, Ok, AOk, GOk : Boolean;
      Seq, First, HF, HL, Target : Seq_Type;
      Bitmap   : Bitmap_Type;
      Deadline : Long_Integer := -1;
   begin
      select
         accept Start;
      or
         terminate;
      end select;

      loop
         --  1) a new sample into the window while it has room
         Got := False;
         if In_Flight_Count (S) < Window then
            Ring.Dequeue (P, Got);
            if Got then
               Submit (S, P, Seq, Ok);
               if Ok then
                  Send (Write_Frame (Seq, Object_Id, P));
               end if;
            end if;
         end if;

         --  2) an ACKNACK: prune the acknowledged, retransmit the missing
         Try_Recv (Fr, Have);
         if Have then
            Parse_Acknack (Fr, AOk, First, Bitmap);
            if AOk then
               Recv_Acknack (S, First, Bitmap);
               for B in 0 .. 15 loop
                  if Nack_Bit (Bitmap, B) then
                     Target := (First + B) mod 65536;
                     Get_In_Flight (S, Target, PL, GOk);
                     if GOk then
                        Send (Write_Frame (Target, Object_Id, PL));
                     end if;
                  end if;
               end loop;
            end if;
         end if;

         --  3) the periodic HEARTBEAT
         Pending_Heartbeat (S, Now_Ms, Has, HF, HL);
         if Has then
            Send (Heartbeat_Frame (HF, HL));
         end if;

         --  4) done when the producer closed, the ring is empty and the
         --  window drained; give up at the deadline
         if Ring.Is_Closed and then Ring.Is_Empty then
            if In_Flight_Count (S) = 0 then
               Done := True;
               exit;
            end if;
            if Deadline < 0 then
               Deadline := Now_Ms + Drain_Deadline_Ms;
            end if;
            exit when Now_Ms >= Deadline;
         end if;

         if not Got and not Have then
            delay 0.001;
         end if;
      end loop;

      select
         accept Finished;
      or
         terminate;
      end select;
   end Drain;

   procedure Start is
   begin
      Drain.Start;
   end Start;

   procedure Enqueue (P : Frame_Store; Ok : out Boolean) is
   begin
      if P.Len > Slot_Cap then
         raise CONSTRAINT_ERROR;
      end if;
      Ring.Enqueue (P, Ok);
   end Enqueue;

   procedure Close is
   begin
      Ring.Close;
   end Close;

   procedure Wait is
   begin
      Drain.Finished;
   end Wait;

   function Drained return Boolean is
   begin
      return Done;
   end Drained;

end Zerodds_Ada83_Async_Writer;
