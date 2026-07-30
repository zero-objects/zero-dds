-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Reliable is

   use type Interfaces.Unsigned_16;

   HDR : constant := 8;  -- XRCE 8-byte header

   ------------------------------------------------------------------
   function Seq_Lt (A, B : Seq_Type) return Boolean is
      Diff : constant Seq_Type := B - A;  -- modular wrap
   begin
      return Diff /= 0 and then Diff < 16#8000#;
   end Seq_Lt;

   ------------------------------------------------------------------
   --  Frames
   ------------------------------------------------------------------
   function Write_Frame (Seq : Seq_Type; Body_FS : Frame_Store) return Frame_Store is
      FS : Frame_Store;
      N  : constant Natural := Body_FS.Len;
   begin
      FS.Data (0) := 16#80#;              -- session: no-key
      FS.Data (1) := Reliable_Stream;      -- stream: reliable (>=128)
      FS.Data (2) := Byte (Seq and 16#FF#);
      FS.Data (3) := Byte (Shift_Right (Seq, 8) and 16#FF#);
      FS.Data (4) := 16#07#;               -- WRITE_DATA
      FS.Data (5) := 16#03#;
      FS.Data (6) := Byte (Unsigned_16 (N) and 16#FF#);
      FS.Data (7) := Byte (Shift_Right (Unsigned_16 (N), 8) and 16#FF#);
      for I in 0 .. N - 1 loop
         FS.Data (HDR + I) := Body_FS.Data (I);
      end loop;
      FS.Len := HDR + N;
      return FS;
   end Write_Frame;

   procedure Read_Frame (F : Frame_Store; Seq : out Seq_Type; Body_FS : out Frame_Store) is
   begin
      Seq := 0;
      Body_FS := (Data => [others => 0], Len => 0);
      if F.Len >= HDR and then F.Data (4) = 16#07# then
         Seq := Seq_Type (F.Data (2)) + Shift_Left (Seq_Type (F.Data (3)), 8);
         for I in 0 .. F.Len - HDR - 1 loop
            Body_FS.Data (I) := F.Data (HDR + I);
         end loop;
         Body_FS.Len := F.Len - HDR;
      end if;
   end Read_Frame;

   function Control_Frame (Sm : Byte; B0, B1, B2, B3, Stream : Byte; Msg_Seq : Seq_Type)
                           return Frame_Store is
      FS : Frame_Store;
   begin
      FS.Data (0) := 16#80#;               -- session: no-key
      FS.Data (1) := 16#00#;               -- control stream (NONE); target in body
      FS.Data (2) := Byte (Msg_Seq and 16#FF#);
      FS.Data (3) := Byte (Shift_Right (Msg_Seq, 8) and 16#FF#);
      FS.Data (4) := Sm;
      FS.Data (5) := 16#01#;               -- E-flag little-endian only
      FS.Data (6) := 5;                    -- body length
      FS.Data (7) := 0;
      FS.Data (8) := B0;
      FS.Data (9) := B1;
      FS.Data (10) := B2;
      FS.Data (11) := B3;
      FS.Data (12) := Stream;
      FS.Len := 13;
      return FS;
   end Control_Frame;

   function Acknack_Frame (First_Unacked : Seq_Type; Bitmap : Interfaces.Unsigned_16;
                           Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                           return Frame_Store is
   begin
      return Control_Frame
        (16#0A#,
         Byte (First_Unacked and 16#FF#), Byte (Shift_Right (First_Unacked, 8) and 16#FF#),
         Byte (Bitmap and 16#FF#), Byte (Shift_Right (Bitmap, 8) and 16#FF#),
         Stream, Msg_Seq);
   end Acknack_Frame;

   procedure Parse_Acknack (F : Frame_Store; Ok : out Boolean;
                            First_Unacked : out Seq_Type; Bitmap : out Interfaces.Unsigned_16) is
   begin
      Ok := F.Len >= 13 and then F.Data (4) = 16#0A#;
      First_Unacked := 0;
      Bitmap := 0;
      if Ok then
         First_Unacked := Seq_Type (F.Data (8)) + Shift_Left (Seq_Type (F.Data (9)), 8);
         Bitmap := Unsigned_16 (F.Data (10)) + Shift_Left (Unsigned_16 (F.Data (11)), 8);
      end if;
   end Parse_Acknack;

   function Heartbeat_Frame (First_Unacked, Last_Unacked : Seq_Type;
                             Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1)
                             return Frame_Store is
   begin
      return Control_Frame
        (16#0B#,
         Byte (First_Unacked and 16#FF#), Byte (Shift_Right (First_Unacked, 8) and 16#FF#),
         Byte (Last_Unacked and 16#FF#), Byte (Shift_Right (Last_Unacked, 8) and 16#FF#),
         Stream, Msg_Seq);
   end Heartbeat_Frame;

   procedure Parse_Heartbeat (F : Frame_Store; Ok : out Boolean;
                              First_Unacked, Last_Unacked : out Seq_Type) is
   begin
      Ok := F.Len >= 13 and then F.Data (4) = 16#0B#;
      First_Unacked := 0;
      Last_Unacked := 0;
      if Ok then
         First_Unacked := Seq_Type (F.Data (8)) + Shift_Left (Seq_Type (F.Data (9)), 8);
         Last_Unacked := Seq_Type (F.Data (10)) + Shift_Left (Seq_Type (F.Data (11)), 8);
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
      if Payload.Len > Max_Payload then
         return;
      end if;
      for I in S.Slots'Range loop
         if not S.Slots (I).Used then
            S.Slots (I) := (Used => True, Seq => S.Next_Seq, Payload => Payload);
            Seq := S.Next_Seq;
            S.Next_Seq := S.Next_Seq + 1;
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
                           Bitmap : Interfaces.Unsigned_16) is
   begin
      --  1) acknowledge everything strictly before First_Unacked
      for I in S.Slots'Range loop
         if S.Slots (I).Used and then Seq_Lt (S.Slots (I).Seq, First_Unacked) then
            S.Slots (I).Used := False;
         end if;
      end loop;
      --  2) window [base, base+16): clear bit => acked => drop
      for K in 0 .. 15 loop
         if (Shift_Right (Bitmap, K) and 1) = 0 then
            declare
               Target : constant Seq_Type := First_Unacked + Seq_Type (K);
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
      Payload := (Data => [others => 0], Len => 0);
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
            R.Slots (I) := (Used => True, Seq => Seq, Payload => Payload);
            return;
         end if;
      end loop;
   end Recv_Data;

   procedure Drain_Next (R : in out Receiver_State; Seq : out Seq_Type;
                         Payload : out Frame_Store; Got : out Boolean) is
   begin
      Seq := R.Expected;
      Payload := (Data => [others => 0], Len => 0);
      Got := False;
      for I in R.Slots'Range loop
         if R.Slots (I).Used and then R.Slots (I).Seq = R.Expected then
            Payload := R.Slots (I).Payload;
            R.Slots (I).Used := False;
            R.Expected := R.Expected + 1;
            Got := True;
            return;
         end if;
      end loop;
   end Drain_Next;

   function Pending_Acknack (R : Receiver_State) return Interfaces.Unsigned_16 is
      Bitmap : Unsigned_16 := 0;
   begin
      for K in 0 .. 15 loop
         declare
            Target  : constant Seq_Type := R.Expected + Seq_Type (K);
            Present : Boolean := False;
         begin
            for I in R.Slots'Range loop
               if R.Slots (I).Used and then R.Slots (I).Seq = Target then
                  Present := True;
               end if;
            end loop;
            if not Present then
               Bitmap := Bitmap or Shift_Left (Unsigned_16 (1), K);
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

   ------------------------------------------------------------------
   --  Send_Ring (producer -> drain-task boundary)
   ------------------------------------------------------------------
   protected body Send_Ring is
      procedure Enqueue (P : Frame_Store; Ok : out Boolean) is
      begin
         if Count >= Ring_Cap then
            Ok := False;  -- backpressure
         else
            Q (Tail) := P;
            Tail := (Tail + 1) mod Ring_Cap;
            Count := Count + 1;
            Ok := True;
         end if;
      end Enqueue;

      procedure Dequeue (P : out Frame_Store; Got : out Boolean) is
      begin
         if Count = 0 then
            P := (Data => [others => 0], Len => 0);
            Got := False;
         else
            P := Q (Head);
            Head := (Head + 1) mod Ring_Cap;
            Count := Count - 1;
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

      function Pending return Natural is
      begin
         return Count;
      end Pending;
   end Send_Ring;

end Reliable;
