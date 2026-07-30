-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Unit + byte-golden tests for the pre-Object Ada 83 reliable stream. Mirrors
--  crates/xrce/src/reliable.rs and endpoints/ada's test_reliable_unit. Strict
--  Ada 83: Text_IO only, no Command_Line, no Interfaces -- the HEARTBEAT/ACKNACK
--  reference goldens are embedded inline (the same bytes the Rust
--  zerodds-endpoint-golden emits and crates/endpoint-e2e asserts), so the test
--  is self-contained and needs no golden files.

with Text_IO;
with Zerodds_Ada83_Wire;      use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Reliable;  use Zerodds_Ada83_Reliable;

procedure Test_Ada83_Reliable is

   Failures : Natural := 0;

   procedure Check (Cond : Boolean; Name : String) is
   begin
      if not Cond then
         Text_IO.Put_Line ("FAIL: " & Name);
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

   --  Frame equals the reference Golden bytes (0 .. Golden'Last).
   function Equals (F : Frame_Store; Golden : Byte_Array) return Boolean is
   begin
      if F.Len /= Golden'Length then
         return False;
      end if;
      for I in Golden'Range loop
         if F.Data (I - Golden'First) /= Golden (I) then
            return False;
         end if;
      end loop;
      return True;
   end Equals;

   --  Reference goldens (byte-for-byte the zerodds-endpoint-golden output,
   --  verified against crates/xrce + endpoints/c; identical to the constants the
   --  Rust ada83_reliable e2e test carries).
   Golden_Heartbeat : constant Byte_Array (0 .. 12) :=
     (16#80#, 16#00#, 16#01#, 16#00#, 16#0B#, 16#01#, 16#05#, 16#00#,
      16#01#, 16#00#, 16#03#, 16#00#, 16#80#);
   Golden_Acknack : constant Byte_Array (0 .. 12) :=
     (16#80#, 16#00#, 16#01#, 16#00#, 16#0A#, 16#01#, 16#05#, 16#00#,
      16#01#, 16#00#, 16#00#, 16#00#, 16#80#);

   Seq : Seq_Type;
   Ok  : Boolean;

begin
   --  1) submit assigns monotonic seqnrs
   declare
      S : Sender_State;
      S0, S1 : Seq_Type;
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

   --  3) heartbeat first / silence / after period
   declare
      S : Sender_State;
      Has : Boolean;
      F, L : Seq_Type;
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
      --  base=2, bit0 set => seq2 missing, seq0+1 acked
      Recv_Acknack (S, 2, 1);
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
      DSeq : Seq_Type;
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
      DSeq : Seq_Type;
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
         Recv_Data (R, I, Payload (1), Ok);
         Check (Ok, "recv_buffer_fill");
      end loop;
      Recv_Data (R, Recv_Buffer + 1, Payload (1), Ok);
      Check (not Ok, "recv_buffer_full_rejects");
   end;

   --  8) pending_acknack marks missing slots (expected 0, have 1 + 3)
   declare
      R : Receiver_State;
      Bitmap : Bitmap_Type;
   begin
      Recv_Data (R, 1, Payload (1), Ok);
      Recv_Data (R, 3, Payload (3), Ok);
      Bitmap := Pending_Acknack (R);
      Check (Nack_Bit (Bitmap, 0), "acknack_bit0_missing");        -- seq0
      Check (Nack_Bit (Bitmap, 2), "acknack_bit2_missing");        -- seq2
      Check (not Nack_Bit (Bitmap, 1), "acknack_bit1_present");    -- seq1
      Check (not Nack_Bit (Bitmap, 3), "acknack_bit3_present");    -- seq3
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
   --      16-bit wrap. Window 0xFFFE,0xFFFF,0,1 -> the correct base/end is
   --      0xFFFE/0x0001, not numeric 0x0000/0xFFFF (Seq_Lt, not a numeric min).
   declare
      S : Sender_State;
      R : Receiver_State;
      Q0, Q1, Q2, Q3, DSeq : Seq_Type;
      Has, Got : Boolean;
      F, L : Seq_Type;
      P : Frame_Store;
      Bitmap : Bitmap_Type;
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
      Check (Nack_Bit (Bitmap, 0), "wrap_ffff_nacked");
      Check (not Nack_Bit (Bitmap, 1), "wrap_0000_present");
      Check (not Nack_Bit (Bitmap, 2), "wrap_0001_present");

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
   Check (Equals (Heartbeat_Frame (1, 3), Golden_Heartbeat),
          "golden_heartbeat_byte_identical");
   Check (Equals (Acknack_Frame (1, 0), Golden_Acknack),
          "golden_acknack_byte_identical");

   --  11) write-frame round-trips through the reliable deframer
   declare
      Body_FS, Out_FS : Frame_Store;
      RSeq : Seq_Type;
   begin
      Body_FS.Data (0) := 16#DE#;
      Body_FS.Data (1) := 16#AD#;
      Body_FS.Len := 2;
      Read_Frame (Write_Frame (7, Body_FS), RSeq, Out_FS);
      Check (RSeq = 7 and Out_FS.Len = 2
             and Out_FS.Data (0) = 16#DE# and Out_FS.Data (1) = 16#AD#,
             "write_frame_roundtrip");
   end;

   if Failures = 0 then
      Text_IO.Put_Line ("ALL OK");
   else
      Text_IO.Put_Line ("FAILURES:" & Natural'Image (Failures));
   end if;
end Test_Ada83_Reliable;
