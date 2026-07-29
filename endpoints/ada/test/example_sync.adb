-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
--  five typed `Reading { Id, Value, Label }` samples and delivers them; the
--  subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.

with Ada.Command_Line; use Ada.Command_Line;
with Ada.Text_IO;      use Ada.Text_IO;
with Interfaces;
with Interfaces.C;
with Deep_Reading;     use Deep_Reading;

procedure Example_Sync is
   Total     : constant := 5;
   Transport : Mailbox;
   Got_Count : Natural := 0;
begin
   for I in 0 .. Total - 1 loop
      declare
         R : Reading;
      begin
         R.Id    := Interfaces.C.unsigned_long (16#1000# + I);
         R.Value := Interfaces.C.C_float (20.0 + Float (I) * 0.5);
         Set_Label (R, "bay-" & Digit2 (I));
         Transport.Deliver (Frame (Interfaces.Unsigned_16 (I + 1), Marshal (R)));
      end;
   end loop;

   while Got_Count < Total loop
      declare
         FS      : Frame_Store;
         Got     : Boolean;
         Body_FS : Frame_Store;
         R       : Reading;
      begin
         Transport.Try_Receive (FS, Got);
         exit when not Got;
         Body_FS := Deframe (FS);
         R := Unmarshal (Body_FS.Data (0 .. Body_FS.Len - 1));
         Put_Line ("sync reading" & Natural'Image (Got_Count)
                   & ": id=0x" & Hex (R.Id)
                   & " value=" & F1 (R.Value)
                   & " label=""" & Label_Str (R) & """");
         Got_Count := Got_Count + 1;
      end;
   end loop;

   if Got_Count /= Total then
      Put_Line ("incomplete");
      Set_Exit_Status (1);
      return;
   end if;
   Put_Line ("ALL OK");
   Set_Exit_Status (0);
end Example_Sync;
