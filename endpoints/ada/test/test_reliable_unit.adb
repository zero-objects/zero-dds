-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Unit + byte-golden tests for the Ada reliable stream. Mirrors
--  crates/xrce/src/reliable.rs tests and asserts HEARTBEAT/ACKNACK byte
--  identity against the reference goldens. Usage: test_reliable_unit <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams.Stream_IO; use Ada.Streams.Stream_IO;
with Interfaces;            use Interfaces;
with Deep_Reading;          use Deep_Reading;
with Reliable;              use Reliable;

procedure Test_Reliable_Unit is

   Failures : Natural := 0;

   procedure Check (Cond : Boolean; Name : String) is
   begin
      if not Cond then
         Put_Line ("FAIL: " & Name);
         Failures := Failures + 1;
      end if;
   end Check;

   --  A one-byte payload carrying value V.
   function Payload (V : Byte) return Frame_Store is
      FS : Frame_Store;
   begin
      FS.Data (0) := V;
      FS.Len := 1;
      return FS;
   end Payload;

   function Frames_Equal (A, B : Frame_Store) return Boolean is
   begin
      if A.Len /= B.Len then
         return False;
      end if;
      for I in 0 .. A.Len - 1 loop
         if A.Data (I) /= B.Data (I) then
            return False;
         end if;
      end loop;
      return True;
   end Frames_Equal;

   function Read_File (Path : String) return Frame_Store is
      F  : Ada.Streams.Stream_IO.File_Type;
      FS : Frame_Store;
      B  : Byte;
   begin
      Open (F, In_File, Path);
      while not End_Of_File (F) loop
         Byte'Read (Stream (F), B);
         FS.Data (FS.Len) := B;
         FS.Len := FS.Len + 1;
      end loop;
      Close (F);
      return FS;
   end Read_File;

   Seq : Reliable.Seq_Type;
   Ok  : Boolean;

begin
   --  1) submit assigns monotonic seqnrs
   declare
      S : Sender_State;
      S0, S1 : Reliable.Seq_Type;
   begin
      Submit (S, Payload (1), S0, Ok);  Check (Ok, "submit0");
      Submit (S, Payload (2), S1, Ok);  Check (Ok, "submit1");
      Check (S0 = 0 and S1 = 1, "monotonic_seq");
      Check (In_Flight_Count (S) = 2, "in_flight_count_2");
   end;

   --  2) window full at 16
   declare
      S : Sender_State;
   begin
      for I in 1 .. Window loop
         Submit (S, Payload (0), Seq, Ok);
         Check (Ok, "window_fill");
      end loop;
      Submit (S, Payload (0), Seq, Ok);
      Check (not Ok, "window_full_rejects");
   end;

   --  3) heartbeat first / silence / empty
   declare
      S : Sender_State;
      Has : Boolean;
      F, L : Reliable.Seq_Type;
   begin
      Pending_Heartbeat (S, 0, Has, F, L);
      Check (not Has, "hb_empty_none");
      Submit (S, Payload (1), Seq, Ok);
      Pending_Heartbeat (S, 0, Has, F, L);
      Check (Has and F = 0 and L = 0, "hb_first_fires");
      Pending_Heartbeat (S, 100, Has, F, L);
      Check (not Has, "hb_silenced_before_period");
      Pending_Heartbeat (S, 600, Has, F, L);
      Check (Has, "hb_after_period");
   end;

   --  4) recv_acknack clears acked, keeps set-bit missing
   declare
      S : Sender_State;
      P : Frame_Store;
   begin
      Submit (S, Payload (16#A0#), Seq, Ok);
      Submit (S, Payload (16#A1#), Seq, Ok);
      Submit (S, Payload (16#A2#), Seq, Ok);
      --  base=2, bitmap bit0 set => seq2 missing, 0+1 acked
      Recv_Acknack (S, 2, 2#0000_0000_0000_0001#);
      Check (In_Flight_Count (S) = 1, "acknack_clears_acked");
      Get_In_Flight (S, 2, P, Ok);
      Check (Ok, "acknack_keeps_missing");
   end;

   --  4b) full clear when no bits set
   declare
      S : Sender_State;
   begin
      for I in 1 .. 5 loop
         Submit (S, Payload (0), Seq, Ok);
      end loop;
      Recv_Acknack (S, 5, 0);
      Check (In_Flight_Count (S) = 0, "acknack_full_clear");
   end;

   --  5) reorder: recv 2 then 0, drain gives 0; then 1, drain gives 1,2
   declare
      R : Receiver_State;
      DSeq : Reliable.Seq_Type;
      P : Frame_Store;
      Got : Boolean;
   begin
      Recv_Data (R, 2, Payload (22), Ok);  Check (Ok, "recv2");
      Recv_Data (R, 0, Payload (20), Ok);  Check (Ok, "recv0");
      Drain_Next (R, DSeq, P, Got);
      Check (Got and DSeq = 0, "drain_0");
      Drain_Next (R, DSeq, P, Got);
      Check (not Got, "drain_blocks_on_gap");
      Recv_Data (R, 1, Payload (21), Ok);  Check (Ok, "recv1");
      Drain_Next (R, DSeq, P, Got);  Check (Got and DSeq = 1, "drain_1");
      Drain_Next (R, DSeq, P, Got);  Check (Got and DSeq = 2, "drain_2");
   end;

   --  6) duplicate drop
   declare
      R : Receiver_State;
      DSeq : Reliable.Seq_Type;
      P : Frame_Store;
      Got : Boolean;
   begin
      Recv_Data (R, 0, Payload (1), Ok);
      Drain_Next (R, DSeq, P, Got);
      Recv_Data (R, 0, Payload (99), Ok);
      Check (Out_Of_Order_Count (R) = 0, "duplicate_dropped");
   end;

   --  7) receiver buffer full
   declare
      R : Receiver_State;
   begin
      for I in 1 .. Recv_Buffer loop
         Recv_Data (R, Reliable.Seq_Type (I), Payload (1), Ok);
         Check (Ok, "recv_buffer_fill");
      end loop;
      Recv_Data (R, Reliable.Seq_Type (Recv_Buffer + 1), Payload (1), Ok);
      Check (not Ok, "recv_buffer_full_rejects");
   end;

   --  8) pending_acknack marks missing slots (expected 0, have 1 + 3)
   declare
      R : Receiver_State;
      Bitmap : Unsigned_16;
   begin
      Recv_Data (R, 1, Payload (1), Ok);
      Recv_Data (R, 3, Payload (3), Ok);
      Bitmap := Pending_Acknack (R);
      Check ((Bitmap and 2#0001#) /= 0, "acknack_bit0_missing");   -- seq0
      Check ((Bitmap and 2#0100#) /= 0, "acknack_bit2_missing");   -- seq2
      Check ((Bitmap and 2#0010#) = 0, "acknack_bit1_present");    -- seq1
      Check ((Bitmap and 2#1000#) = 0, "acknack_bit3_present");    -- seq3
   end;

   --  9) reset clears everything
   declare
      S : Sender_State;
      R : Receiver_State;
   begin
      Submit (S, Payload (1), Seq, Ok);
      Recv_Data (R, 0, Payload (3), Ok);
      Reset (R);
      Check (Out_Of_Order_Count (R) = 0 and R.Expected = 0, "reset_clears");
   end;

   --  9b) RFC-1982 regression: HEARTBEAT window + loss recovery across the
   --      16-bit wrap (mirrors crates/xrce). Window 0xFFFE,0xFFFF,0,1 -> the
   --      correct base/end is 0xFFFE/0x0001, not numeric 0x0000/0xFFFF. The Ada
   --      Pending_Heartbeat already uses Seq_Lt, so this guards that behaviour.
   declare
      S : Sender_State;
      R : Receiver_State;
      Q0, Q1, Q2, Q3, DSeq : Reliable.Seq_Type;
      Has, Got : Boolean;
      F, L : Reliable.Seq_Type;
      P : Frame_Store;
      Bitmap : Unsigned_16;
      N : Natural := 0;
   begin
      S.Next_Seq := 16#FFFE#;  --  seed just below the wrap
      Submit (S, Payload (10), Q0, Ok);  --  0xFFFE
      Submit (S, Payload (11), Q1, Ok);  --  0xFFFF (lost)
      Submit (S, Payload (12), Q2, Ok);  --  0x0000
      Submit (S, Payload (13), Q3, Ok);  --  0x0001
      Check (Q0 = 16#FFFE# and Q1 = 16#FFFF# and Q2 = 0 and Q3 = 1, "wrap_seqs");

      Pending_Heartbeat (S, 0, Has, F, L);
      Check (Has and F = 16#FFFE# and L = 16#0001#, "wrap_heartbeat_rfc1982_window");

      R.Expected := 16#FFFE#;  --  seed receiver just below the wrap
      Recv_Data (R, Q0, Payload (10), Ok);  --  0xFFFF lost
      Recv_Data (R, Q2, Payload (12), Ok);
      Recv_Data (R, Q3, Payload (13), Ok);
      Drain_Next (R, DSeq, P, Got);
      Check (Got and DSeq = 16#FFFE#, "wrap_only_first_delivered");
      Drain_Next (R, DSeq, P, Got);
      Check (not Got and R.Expected = 16#FFFF#, "wrap_blocked_at_ffff");

      Bitmap := Pending_Acknack (R);
      Check ((Bitmap and 2#0001#) /= 0, "wrap_ffff_nacked");
      Check ((Bitmap and 2#0010#) = 0, "wrap_0000_present");
      Check ((Bitmap and 2#0100#) = 0, "wrap_0001_present");

      Recv_Acknack (S, 16#FFFF#, Bitmap);
      Get_In_Flight (S, Q1, P, Ok);
      Check (Ok, "wrap_ffff_retransmittable");
      Get_In_Flight (S, Q0, P, Ok);
      Check (not Ok, "wrap_fffe_acked");
      Check (In_Flight_Count (S) = 1, "wrap_one_in_flight");

      Get_In_Flight (S, Q1, P, Ok);
      Recv_Data (R, Q1, P, Ok);
      loop
         Drain_Next (R, DSeq, P, Got);
         exit when not Got;
         N := N + 1;
      end loop;
      Check (N = 3, "wrap_deliver_three_in_order");
   end;

   --  10) byte-golden: HEARTBEAT + ACKNACK identical to the reference
   if Argument_Count >= 1 then
      declare
         Dir : constant String := Argument (1);
         HB_Gold : constant Frame_Store := Read_File (Dir & "/golden_heartbeat_le.bin");
         AN_Gold : constant Frame_Store := Read_File (Dir & "/golden_acknack_le.bin");
         HB_Mine : constant Frame_Store := Heartbeat_Frame (1, 3);
         AN_Mine : constant Frame_Store := Acknack_Frame (1, 0);
      begin
         Check (Frames_Equal (HB_Mine, HB_Gold), "golden_heartbeat_byte_identical");
         Check (Frames_Equal (AN_Mine, AN_Gold), "golden_acknack_byte_identical");
      end;
   else
      Put_Line ("SKIP golden: no golden_dir argument");
   end if;

   if Failures = 0 then
      Put_Line ("ALL OK");
   else
      Put_Line ("FAILURES:" & Natural'Image (Failures));
      Set_Exit_Status (Failure);
   end if;
end Test_Reliable_Unit;
