-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Zerodds_Ada83_Reliable is

   Wrap : constant := 65536;  -- 2**16, the RFC-1982 modulus

   ------------------------------------------------------------------
   --  Small integer helpers (no modular type, no bitwise operators).
   ------------------------------------------------------------------
   function Pow2 (K : Integer) return Integer is
   begin
      return 2 ** K;
   end Pow2;

   function Seq_Lt (A, B : Seq_Type) return Boolean is
      --  Ada `mod` with a positive right operand yields a non-negative result,
      --  so (B - A) mod Wrap is the unsigned 16-bit difference even when B < A.
      Diff : constant Integer := (B - A) mod Wrap;
   begin
      return Diff /= 0 and then Diff < 16#8000#;
   end Seq_Lt;

   function Nack_Bit (Bitmap : Bitmap_Type; K : Integer) return Boolean is
   begin
      return (Bitmap / Pow2 (K)) mod 2 = 1;
   end Nack_Bit;

   function Lo (V : Integer) return Byte is
   begin
      return Byte (V mod 256);
   end Lo;

   function Hi (V : Integer) return Byte is
   begin
      return Byte ((V / 256) mod 256);
   end Hi;

   ------------------------------------------------------------------
   --  Frames
   ------------------------------------------------------------------
   function Write_Frame (Seq : Seq_Type; Body_FS : Frame_Store) return Frame_Store is
      FS : Frame_Store;
      N  : constant Integer := Body_FS.Len;
   begin
      FS.Data (0) := 16#80#;             -- session: no-key
      FS.Data (1) := Reliable_Stream;     -- stream: reliable (>=128)
      FS.Data (2) := Lo (Seq);
      FS.Data (3) := Hi (Seq);
      FS.Data (4) := 16#07#;              -- WRITE_DATA
      FS.Data (5) := 16#03#;              -- flags: data-present + E little-endian
      FS.Data (6) := Lo (N);
      FS.Data (7) := Hi (N);
      for I in 0 .. N - 1 loop
         FS.Data (8 + I) := Body_FS.Data (I);
      end loop;
      FS.Len := 8 + N;
      return FS;
   end Write_Frame;

   procedure Read_Frame (F : Frame_Store; Seq : out Seq_Type; Body_FS : out Frame_Store) is
   begin
      Seq := 0;
      Body_FS := (Data => (others => 0), Len => 0);
      if F.Len >= 8 and then F.Data (4) = 16#07# then
         Seq := Integer (F.Data (2)) + Integer (F.Data (3)) * 256;
         for I in 0 .. F.Len - 8 - 1 loop
            Body_FS.Data (I) := F.Data (8 + I);
         end loop;
         Body_FS.Len := F.Len - 8;
      end if;
   end Read_Frame;

   --  Shared 13-byte control frame layout (only Sm/body/stream differ).
   function Control_Frame (Sm : Byte; B0, B1, B2, B3, Stream : Byte; Msg_Seq : Seq_Type)
                           return Frame_Store is
      FS : Frame_Store;
   begin
      FS.Data (0) := 16#80#;              -- session: no-key
      FS.Data (1) := 16#00#;              -- control stream (NONE); target in body
      FS.Data (2) := Lo (Msg_Seq);
      FS.Data (3) := Hi (Msg_Seq);
      FS.Data (4) := Sm;
      FS.Data (5) := 16#01#;              -- E little-endian only
      FS.Data (6) := 5;                   -- body length
      FS.Data (7) := 0;
      FS.Data (8) := B0;
      FS.Data (9) := B1;
      FS.Data (10) := B2;
      FS.Data (11) := B3;
      FS.Data (12) := Stream;
      FS.Len := 13;
      return FS;
   end Control_Frame;

   function Acknack_Frame (First_Unacked : Seq_Type; Bitmap : Bitmap_Type;
                           Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                           return Frame_Store is
   begin
      return Control_Frame
        (16#0A#, Lo (First_Unacked), Hi (First_Unacked), Lo (Bitmap), Hi (Bitmap),
         Stream, Msg_Seq);
   end Acknack_Frame;

   procedure Parse_Acknack (F : Frame_Store; Ok : out Boolean;
                            First_Unacked : out Seq_Type; Bitmap : out Bitmap_Type) is
   begin
      Ok := F.Len >= 13 and then F.Data (4) = 16#0A#;
      First_Unacked := 0;
      Bitmap := 0;
      if Ok then
         First_Unacked := Integer (F.Data (8)) + Integer (F.Data (9)) * 256;
         Bitmap := Integer (F.Data (10)) + Integer (F.Data (11)) * 256;
      end if;
   end Parse_Acknack;

   function Heartbeat_Frame (First_Unacked, Last_Unacked : Seq_Type;
                             Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                             return Frame_Store is
   begin
      return Control_Frame
        (16#0B#, Lo (First_Unacked), Hi (First_Unacked), Lo (Last_Unacked), Hi (Last_Unacked),
         Stream, Msg_Seq);
   end Heartbeat_Frame;

   procedure Parse_Heartbeat (F : Frame_Store; Ok : out Boolean;
                              First_Unacked, Last_Unacked : out Seq_Type) is
   begin
      Ok := F.Len >= 13 and then F.Data (4) = 16#0B#;
      First_Unacked := 0;
      Last_Unacked := 0;
      if Ok then
         First_Unacked := Integer (F.Data (8)) + Integer (F.Data (9)) * 256;
         Last_Unacked := Integer (F.Data (10)) + Integer (F.Data (11)) * 256;
      end if;
   end Parse_Heartbeat;

   ------------------------------------------------------------------
   --  Sender
   ------------------------------------------------------------------
   procedure Submit (S : in out Sender_State; Payload : Frame_Store;
                     Seq : out Seq_Type; Ok : out Boolean) is
   begin
      Seq := 0;
      Ok := False;
      if Payload.Len > Slot_Cap then
         return;
      end if;
      for I in S.Slots'Range loop
         if not S.Slots (I).Used then
            S.Slots (I).Used := True;
            S.Slots (I).Seq := S.Next_Seq;
            S.Slots (I).Payload := Payload;
            Seq := S.Next_Seq;
            S.Next_Seq := (S.Next_Seq + 1) mod Wrap;
            Ok := True;
            return;
         end if;
      end loop;  -- window full
   end Submit;

   function In_Flight_Count (S : Sender_State) return Natural is
      N : Natural := 0;
   begin
      for I in S.Slots'Range loop
         if S.Slots (I).Used then
            N := N + 1;
         end if;
      end loop;
      return N;
   end In_Flight_Count;

   procedure Pending_Heartbeat (S : in out Sender_State; Now_Ms : Long_Integer;
                                Has : out Boolean; First, Last : out Seq_Type) is
      Seen : Boolean := False;
   begin
      Has := False;
      First := 0;
      Last := 0;
      if In_Flight_Count (S) = 0 then
         return;
      end if;
      if S.Last_Hb_Ms >= 0 and then Now_Ms - S.Last_Hb_Ms < Heartbeat_Ms then
         return;
      end if;
      S.Last_Hb_Ms := Now_Ms;
      for I in S.Slots'Range loop
         if S.Slots (I).Used then
            if not Seen then
               First := S.Slots (I).Seq;
               Last := S.Slots (I).Seq;
               Seen := True;
            else
               if Seq_Lt (S.Slots (I).Seq, First) then
                  First := S.Slots (I).Seq;
               end if;
               if Seq_Lt (Last, S.Slots (I).Seq) then
                  Last := S.Slots (I).Seq;
               end if;
            end if;
         end if;
      end loop;
      Has := True;
   end Pending_Heartbeat;

   procedure Recv_Acknack (S : in out Sender_State; First_Unacked : Seq_Type;
                           Bitmap : Bitmap_Type) is
   begin
      --  1) acknowledge everything strictly before First_Unacked
      for I in S.Slots'Range loop
         if S.Slots (I).Used and then Seq_Lt (S.Slots (I).Seq, First_Unacked) then
            S.Slots (I).Used := False;
         end if;
      end loop;
      --  2) window [base, base+16): a clear bit => acked => drop
      for K in 0 .. 15 loop
         if not Nack_Bit (Bitmap, K) then
            declare
               Target : constant Seq_Type := (First_Unacked + K) mod Wrap;
            begin
               for I in S.Slots'Range loop
                  if S.Slots (I).Used and then S.Slots (I).Seq = Target then
                     S.Slots (I).Used := False;
                  end if;
               end loop;
            end;
         end if;
      end loop;
   end Recv_Acknack;

   procedure Get_In_Flight (S : Sender_State; Seq : Seq_Type;
                            Payload : out Frame_Store; Ok : out Boolean) is
   begin
      Payload := (Data => (others => 0), Len => 0);
      Ok := False;
      for I in S.Slots'Range loop
         if S.Slots (I).Used and then S.Slots (I).Seq = Seq then
            Payload := S.Slots (I).Payload;
            Ok := True;
            return;
         end if;
      end loop;
   end Get_In_Flight;

   ------------------------------------------------------------------
   --  Receiver
   ------------------------------------------------------------------
   function Out_Of_Order_Count (R : Receiver_State) return Natural is
      N : Natural := 0;
   begin
      for I in R.Slots'Range loop
         if R.Slots (I).Used then
            N := N + 1;
         end if;
      end loop;
      return N;
   end Out_Of_Order_Count;

   procedure Recv_Data (R : in out Receiver_State; Seq : Seq_Type;
                        Payload : Frame_Store; Ok : out Boolean) is
   begin
      Ok := True;
      if Seq_Lt (Seq, R.Expected) then
         return;  -- duplicate, already delivered
      end if;
      for I in R.Slots'Range loop
         if R.Slots (I).Used and then R.Slots (I).Seq = Seq then
            return;  -- already buffered
         end if;
      end loop;
      if Out_Of_Order_Count (R) >= Recv_Buffer then
         Ok := False;  -- reorder buffer full
         return;
      end if;
      for I in R.Slots'Range loop
         if not R.Slots (I).Used then
            R.Slots (I).Used := True;
            R.Slots (I).Seq := Seq;
            R.Slots (I).Payload := Payload;
            return;
         end if;
      end loop;
   end Recv_Data;

   procedure Drain_Next (R : in out Receiver_State; Seq : out Seq_Type;
                         Payload : out Frame_Store; Got : out Boolean) is
   begin
      Seq := R.Expected;
      Payload := (Data => (others => 0), Len => 0);
      Got := False;
      for I in R.Slots'Range loop
         if R.Slots (I).Used and then R.Slots (I).Seq = R.Expected then
            Payload := R.Slots (I).Payload;
            R.Slots (I).Used := False;
            R.Expected := (R.Expected + 1) mod Wrap;
            Got := True;
            return;
         end if;
      end loop;
   end Drain_Next;

   function Pending_Acknack (R : Receiver_State) return Bitmap_Type is
      Bitmap : Bitmap_Type := 0;
   begin
      for K in 0 .. 15 loop
         declare
            Target  : constant Seq_Type := (R.Expected + K) mod Wrap;
            Present : Boolean := False;
         begin
            for I in R.Slots'Range loop
               if R.Slots (I).Used and then R.Slots (I).Seq = Target then
                  Present := True;
               end if;
            end loop;
            if not Present then
               Bitmap := Bitmap + Pow2 (K);
            end if;
         end;
      end loop;
      return Bitmap;
   end Pending_Acknack;

   procedure Reset (R : in out Receiver_State) is
   begin
      R.Expected := 0;
      for I in R.Slots'Range loop
         R.Slots (I).Used := False;
      end loop;
   end Reset;

end Zerodds_Ada83_Reliable;
