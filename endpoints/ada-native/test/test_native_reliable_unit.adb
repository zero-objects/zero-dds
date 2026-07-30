-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Unit + byte-golden tests for the pure-Ada reliable stream. Mirrors
--  crates/xrce/src/reliable.rs and endpoints/ada test_reliable_unit, and asserts
--  HEARTBEAT/ACKNACK byte identity against the reference goldens.
--  Usage: test_native_reliable_unit <golden_dir>

with Ada.Command_Line;      use Ada.Command_Line;
with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Streams.Stream_IO; use Ada.Streams.Stream_IO;
with Interfaces;            use Interfaces;
with Zerodds_Native_Wire;   use Zerodds_Native_Wire;
with Native_Reliable;       use Native_Reliable;

procedure Test_Native_Reliable_Unit is

   Failures : Natural := 0;

   procedure Check (Cond : Boolean; Name : String) is
   begin
      if not Cond then
         Put_Line ("FAIL: " & Name);
         Failures := Failures + 1;
      end if;
   end Check;

   --  A one-byte payload carrying value V.
   function P1 (V : Byte) return Payload is
   begin
      return Make_Payload ([0 => V]);
   end P1;

   function Frames_Equal (A, B : Byte_Array) return Boolean is
   begin
      if A'Length /= B'Length then
         return False;
      end if;
      for I in 0 .. A'Length - 1 loop
         if A (A'First + I) /= B (B'First + I) then
            return False;
         end if;
      end loop;
      return True;
   end Frames_Equal;

   function Read_File (Path : String) return Byte_Array is
      F   : Ada.Streams.Stream_IO.File_Type;
      Buf : Byte_Array (0 .. 63);
      N   : Natural := 0;
      B   : Byte;
   begin
      Open (F, In_File, Path);
      while not End_Of_File (F) loop
         Byte'Read (Stream (F), B);
         Buf (N) := B;
         N := N + 1;
      end loop;
      Close (F);
      return Buf (0 .. N - 1);
   end Read_File;

   Seq : Native_Reliable.Seq_Type;
   Ok  : Boolean;

begin
   --  1) submit assigns monotonic seqnrs
   declare
      S : Sender_State;
      S0, S1 : Native_Reliable.Seq_Type;
   begin
      Submit (S, P1 (1), S0, Ok);  Check (Ok, "submit0");
      Submit (S, P1 (2), S1, Ok);  Check (Ok, "submit1");
      Check (S0 = 0 and S1 = 1, "monotonic_seq");
      Check (In_Flight_Count (S) = 2, "in_flight_count_2");
   end;

   --  2) window full at 16
   declare
      S : Sender_State;
   begin
      for I in 1 .. Window loop
         Submit (S, P1 (0), Seq, Ok);
         Check (Ok, "window_fill");
      end loop;
      Submit (S, P1 (0), Seq, Ok);
      Check (not Ok, "window_full_rejects");
   end;

   --  3) heartbeat first / silence / empty
   declare
      S : Sender_State;
      Has : Boolean;
      F, L : Native_Reliable.Seq_Type;
   begin
      Pending_Heartbeat (S, 0, Has, F, L);
      Check (not Has, "hb_empty_none");
      Submit (S, P1 (1), Seq, Ok);
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
      P : Payload;
   begin
      Submit (S, P1 (16#A0#), Seq, Ok);
      Submit (S, P1 (16#A1#), Seq, Ok);
      Submit (S, P1 (16#A2#), Seq, Ok);
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
         Submit (S, P1 (0), Seq, Ok);
      end loop;
      Recv_Acknack (S, 5, 0);
      Check (In_Flight_Count (S) = 0, "acknack_full_clear");
   end;

   --  5) reorder: recv 2 then 0, drain gives 0; then 1, drain gives 1,2
   declare
      R : Receiver_State;
      DSeq : Native_Reliable.Seq_Type;
      P : Payload;
      Got : Boolean;
   begin
      Recv_Data (R, 2, P1 (22), Ok);  Check (Ok, "recv2");
      Recv_Data (R, 0, P1 (20), Ok);  Check (Ok, "recv0");
      Drain_Next (R, DSeq, P, Got);
      Check (Got and DSeq = 0, "drain_0");
      Drain_Next (R, DSeq, P, Got);
      Check (not Got, "drain_blocks_on_gap");
      Recv_Data (R, 1, P1 (21), Ok);  Check (Ok, "recv1");
      Drain_Next (R, DSeq, P, Got);  Check (Got and DSeq = 1, "drain_1");
      Drain_Next (R, DSeq, P, Got);  Check (Got and DSeq = 2, "drain_2");
   end;

   --  6) duplicate drop
   declare
      R : Receiver_State;
      DSeq : Native_Reliable.Seq_Type;
      P : Payload;
      Got : Boolean;
   begin
      Recv_Data (R, 0, P1 (1), Ok);
      Drain_Next (R, DSeq, P, Got);
      Recv_Data (R, 0, P1 (99), Ok);
      Check (Out_Of_Order_Count (R) = 0, "duplicate_dropped");
   end;

   --  7) receiver buffer full
   declare
      R : Receiver_State;
   begin
      for I in 1 .. Recv_Buffer loop
         Recv_Data (R, Native_Reliable.Seq_Type (I), P1 (1), Ok);
         Check (Ok, "recv_buffer_fill");
      end loop;
      Recv_Data (R, Native_Reliable.Seq_Type (Recv_Buffer + 1), P1 (1), Ok);
      Check (not Ok, "recv_buffer_full_rejects");
   end;

   --  8) pending_acknack marks missing slots (expected 0, have 1 + 3)
   declare
      R : Receiver_State;
      Bitmap : Unsigned_16;
   begin
      Recv_Data (R, 1, P1 (1), Ok);
      Recv_Data (R, 3, P1 (3), Ok);
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
      Submit (S, P1 (1), Seq, Ok);
      Recv_Data (R, 0, P1 (3), Ok);
      Reset (R);
      Check (Out_Of_Order_Count (R) = 0 and R.Expected = 0, "reset_clears");
   end;

   --  9b) RFC-1982 regression: HEARTBEAT window + loss recovery across the
   --      16-bit wrap. Window 0xFFFE,0xFFFF,0,1 -> the correct base/end is
   --      0xFFFE/0x0001, not numeric 0x0000/0xFFFF. This guards Seq_Lt.
   declare
      S : Sender_State;
      R : Receiver_State;
      Q0, Q1, Q2, Q3, DSeq : Native_Reliable.Seq_Type;
      Has, Got : Boolean;
      F, L : Native_Reliable.Seq_Type;
      P : Payload;
      Bitmap : Unsigned_16;
      N : Natural := 0;
   begin
      S.Next_Seq := 16#FFFE#;  --  seed just below the wrap
      Submit (S, P1 (10), Q0, Ok);  --  0xFFFE
      Submit (S, P1 (11), Q1, Ok);  --  0xFFFF (lost)
      Submit (S, P1 (12), Q2, Ok);  --  0x0000
      Submit (S, P1 (13), Q3, Ok);  --  0x0001
      Check (Q0 = 16#FFFE# and Q1 = 16#FFFF# and Q2 = 0 and Q3 = 1, "wrap_seqs");

      Pending_Heartbeat (S, 0, Has, F, L);
      Check (Has and F = 16#FFFE# and L = 16#0001#, "wrap_heartbeat_rfc1982_window");

      R.Expected := 16#FFFE#;  --  seed receiver just below the wrap
      Recv_Data (R, Q0, P1 (10), Ok);  --  0xFFFF lost
      Recv_Data (R, Q2, P1 (12), Ok);
      Recv_Data (R, Q3, P1 (13), Ok);
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

   --  10) frame round-trips: Write_Frame -> Read_Frame preserves seq + body
   declare
      P     : constant Payload := Make_Payload ([16#DE#, 16#AD#, 16#BE#, 16#EF#]);
      Frame : constant Byte_Array := Write_Frame (7, P);
      RSeq  : Native_Reliable.Seq_Type;
      RBody : Payload;
      Valid : Boolean;
   begin
      Check (Frame (4) = 16#07#, "write_frame_id_0x07");
      Check (Frame (1) = 16#80#, "write_frame_reliable_stream");
      Read_Frame (Frame, RSeq, RBody, Valid);
      Check (Valid and RSeq = 7 and RBody.Len = 4
             and RBody.Data (0) = 16#DE# and RBody.Data (3) = 16#EF#,
             "write_read_frame_roundtrip");
   end;

   --  11) control-frame parse round-trips
   declare
      HB : constant Byte_Array := Heartbeat_Frame (5, 9);
      AN : constant Byte_Array := Acknack_Frame (5, 2#0000_0000_0000_0011#);
      Okp : Boolean;
      A, B : Native_Reliable.Seq_Type;
      Bm  : Unsigned_16;
   begin
      Parse_Heartbeat (HB, Okp, A, B);
      Check (Okp and A = 5 and B = 9, "parse_heartbeat_roundtrip");
      Parse_Acknack (AN, Okp, A, Bm);
      Check (Okp and A = 5 and Bm = 2#0000_0000_0000_0011#, "parse_acknack_roundtrip");
   end;

   --  12) byte-golden: HEARTBEAT + ACKNACK identical to the reference
   if Argument_Count >= 1 then
      declare
         Dir     : constant String := Argument (1);
         HB_Gold : constant Byte_Array := Read_File (Dir & "/golden_heartbeat_le.bin");
         AN_Gold : constant Byte_Array := Read_File (Dir & "/golden_acknack_le.bin");
         HB_Mine : constant Byte_Array := Heartbeat_Frame (1, 3);
         AN_Mine : constant Byte_Array := Acknack_Frame (1, 0);
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
end Test_Native_Reliable_Unit;
