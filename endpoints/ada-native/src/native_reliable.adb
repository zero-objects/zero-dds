-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Native_Reliable is

   use type Interfaces.Unsigned_16;

   HDR : constant := 8;  --  XRCE 8-byte header

   ------------------------------------------------------------------
   function Make_Payload (Data : Byte_Array) return Payload is
      P : Payload;
   begin
      P.Len := Data'Length;
      for I in 0 .. Data'Length - 1 loop
         P.Data (I) := Data (Data'First + I);
      end loop;
      return P;
   end Make_Payload;

   ------------------------------------------------------------------
   function Seq_Lt (A, B : Seq_Type) return Boolean is
      Diff : constant Seq_Type := B - A;  --  modular wrap
   begin
      return Diff /= 0 and then Diff < 16#8000#;
   end Seq_Lt;

   ------------------------------------------------------------------
   --  Frames
   ------------------------------------------------------------------
   function Write_Frame (Seq : Seq_Type; Body_P : Payload) return Byte_Array is
      N  : constant Natural := Body_P.Len;
      FS : Byte_Array (0 .. HDR + N - 1);
   begin
      FS (0) := Session_Nokey;                --  session: no-key
      FS (1) := Reliable_Stream;               --  stream: reliable (>=128)
      FS (2) := Byte (Seq and 16#FF#);
      FS (3) := Byte (Shift_Right (Seq, 8) and 16#FF#);
      FS (4) := 16#07#;                        --  WRITE_DATA
      FS (5) := 16#03#;                        --  flags: E-flag LE + data-present
      FS (6) := Byte (Unsigned_16 (N) and 16#FF#);
      FS (7) := Byte (Shift_Right (Unsigned_16 (N), 8) and 16#FF#);
      for I in 0 .. N - 1 loop
         FS (HDR + I) := Body_P.Data (I);
      end loop;
      return FS;
   end Write_Frame;

   procedure Read_Frame
     (F : Byte_Array; Seq : out Seq_Type; Body_P : out Payload; Valid : out Boolean)
   is
      Sm_Len : Natural;
   begin
      Seq := 0;
      Body_P := (Data => [others => 0], Len => 0);
      Valid := False;
      if F'Length >= HDR and then F (F'First + 4) = 16#07# then
         Sm_Len := Natural (F (F'First + 6)) + Natural (F (F'First + 7)) * 256;
         if HDR + Sm_Len <= F'Length and then Sm_Len <= Payload_Cap then
            Seq := Seq_Type (F (F'First + 2)) + Shift_Left (Seq_Type (F (F'First + 3)), 8);
            for I in 0 .. Sm_Len - 1 loop
               Body_P.Data (I) := F (F'First + HDR + I);
            end loop;
            Body_P.Len := Sm_Len;
            Valid := True;
         end if;
      end if;
   end Read_Frame;

   --  A 13-byte XRCE control frame (ACKNACK / HEARTBEAT), golden layout: header
   --  stream NONE + msg seq, body(5) = b0·b1·b2·b3·stream.
   function Control_Frame
     (Sm : Byte; B0, B1, B2, B3, Stream : Byte; Msg_Seq : Seq_Type)
      return Byte_Array
   is
      FS : Byte_Array (0 .. 12);
   begin
      FS (0) := Session_Nokey;                 --  session: no-key
      FS (1) := 16#00#;                        --  control stream (NONE); target in body
      FS (2) := Byte (Msg_Seq and 16#FF#);
      FS (3) := Byte (Shift_Right (Msg_Seq, 8) and 16#FF#);
      FS (4) := Sm;
      FS (5) := 16#01#;                        --  E-flag little-endian only
      FS (6) := 5;                             --  body length
      FS (7) := 0;
      FS (8) := B0;
      FS (9) := B1;
      FS (10) := B2;
      FS (11) := B3;
      FS (12) := Stream;
      return FS;
   end Control_Frame;

   function Acknack_Frame
     (First_Unacked : Seq_Type; Bitmap : Interfaces.Unsigned_16;
      Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1) return Byte_Array is
   begin
      return Control_Frame
        (16#0A#,
         Byte (First_Unacked and 16#FF#), Byte (Shift_Right (First_Unacked, 8) and 16#FF#),
         Byte (Bitmap and 16#FF#), Byte (Shift_Right (Bitmap, 8) and 16#FF#),
         Stream, Msg_Seq);
   end Acknack_Frame;

   procedure Parse_Acknack
     (F : Byte_Array; Ok : out Boolean;
      First_Unacked : out Seq_Type; Bitmap : out Interfaces.Unsigned_16) is
   begin
      Ok := F'Length >= 13 and then F (F'First + 4) = 16#0A#;
      First_Unacked := 0;
      Bitmap := 0;
      if Ok then
         First_Unacked := Seq_Type (F (F'First + 8)) + Shift_Left (Seq_Type (F (F'First + 9)), 8);
         Bitmap := Unsigned_16 (F (F'First + 10)) + Shift_Left (Unsigned_16 (F (F'First + 11)), 8);
      end if;
   end Parse_Acknack;

   function Heartbeat_Frame
     (First_Unacked, Last_Unacked : Seq_Type;
      Stream : Byte := Reliable_Stream; Msg_Seq : Seq_Type := 1) return Byte_Array is
   begin
      return Control_Frame
        (16#0B#,
         Byte (First_Unacked and 16#FF#), Byte (Shift_Right (First_Unacked, 8) and 16#FF#),
         Byte (Last_Unacked and 16#FF#), Byte (Shift_Right (Last_Unacked, 8) and 16#FF#),
         Stream, Msg_Seq);
   end Heartbeat_Frame;

   procedure Parse_Heartbeat
     (F : Byte_Array; Ok : out Boolean;
      First_Unacked, Last_Unacked : out Seq_Type) is
   begin
      Ok := F'Length >= 13 and then F (F'First + 4) = 16#0B#;
      First_Unacked := 0;
      Last_Unacked := 0;
      if Ok then
         First_Unacked := Seq_Type (F (F'First + 8)) + Shift_Left (Seq_Type (F (F'First + 9)), 8);
         Last_Unacked := Seq_Type (F (F'First + 10)) + Shift_Left (Seq_Type (F (F'First + 11)), 8);
      end if;
   end Parse_Heartbeat;

   ------------------------------------------------------------------
   --  Sender
   ------------------------------------------------------------------
   procedure Submit
     (S : in out Sender_State; Data : Payload; Seq : out Seq_Type; Ok : out Boolean) is
   begin
      Seq := 0;
      Ok := False;
      if Data.Len > Max_Payload then
         return;
      end if;
      for I in S.Slots'Range loop
         if not S.Slots (I).Used then
            S.Slots (I) := (Used => True, Seq => S.Next_Seq, Data => Data);
            Seq := S.Next_Seq;
            S.Next_Seq := S.Next_Seq + 1;
            Ok := True;
            return;
         end if;
      end loop;  --  window full
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

   procedure Pending_Heartbeat
     (S : in out Sender_State; Now_Ms : Long_Integer;
      Has : out Boolean; First, Last : out Seq_Type)
   is
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

   procedure Recv_Acknack
     (S : in out Sender_State; First_Unacked : Seq_Type;
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

   procedure Get_In_Flight
     (S : Sender_State; Seq : Seq_Type; Data : out Payload; Ok : out Boolean) is
   begin
      Data := (Data => [others => 0], Len => 0);
      Ok := False;
      for I in S.Slots'Range loop
         if S.Slots (I).Used and then S.Slots (I).Seq = Seq then
            Data := S.Slots (I).Data;
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

   procedure Recv_Data
     (R : in out Receiver_State; Seq : Seq_Type; Data : Payload; Ok : out Boolean) is
   begin
      Ok := True;
      if Seq_Lt (Seq, R.Expected) then
         return;  --  duplicate, already delivered
      end if;
      for I in R.Slots'Range loop
         if R.Slots (I).Used and then R.Slots (I).Seq = Seq then
            return;  --  already buffered
         end if;
      end loop;
      if Out_Of_Order_Count (R) >= Recv_Buffer then
         Ok := False;  --  reorder buffer full
         return;
      end if;
      for I in R.Slots'Range loop
         if not R.Slots (I).Used then
            R.Slots (I) := (Used => True, Seq => Seq, Data => Data);
            return;
         end if;
      end loop;
   end Recv_Data;

   procedure Drain_Next
     (R : in out Receiver_State; Seq : out Seq_Type; Data : out Payload; Got : out Boolean) is
   begin
      Seq := R.Expected;
      Data := (Data => [others => 0], Len => 0);
      Got := False;
      for I in R.Slots'Range loop
         if R.Slots (I).Used and then R.Slots (I).Seq = R.Expected then
            Data := R.Slots (I).Data;
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
      procedure Enqueue (P : Payload; Ok : out Boolean) is
      begin
         if Count >= Ring_Cap then
            Ok := False;  --  backpressure
         else
            Q (Tail) := P;
            Tail := (Tail + 1) mod Ring_Cap;
            Count := Count + 1;
            Ok := True;
         end if;
      end Enqueue;

      procedure Dequeue (P : out Payload; Got : out Boolean) is
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

end Native_Reliable;
