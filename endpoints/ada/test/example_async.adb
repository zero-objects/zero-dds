-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  Deep example (async): the same sensor-telemetry flow, but the subscriber
--  does not own the run-loop. A `Reader_Task` pulls frames from the transport
--  and forwards decoded bodies into an Inbox protected object; the main task
--  blocks on `Inbox.Receive` -- the idiomatic Ada task + protected-object
--  concurrency model. Every field is decoded.

with Ada.Text_IO; use Ada.Text_IO;
with Interfaces;
with Interfaces.C;
with Deep_Reading; use Deep_Reading;

procedure Example_Async is
   Total     : constant := 5;
   Transport : aliased Mailbox;
   Inbox     : aliased Mailbox;
begin
   for I in 0 .. Total - 1 loop
      declare
         R : Reading;
      begin
         R.Id    := Interfaces.C.unsigned_long (16#2000# + I);
         R.Value := Interfaces.C.C_float (100.0 - Float (I));
         Set_Label (R, "sensor-" & Digit2 (I));
         Transport.Deliver (Frame (Interfaces.Unsigned_16 (I + 1), Marshal (R)));
      end;
   end loop;

   declare
      RT : Reader_Task (Transport'Access, Inbox'Access, Total);
      pragma Unreferenced (RT);
   begin
      for Got in 0 .. Total - 1 loop
         declare
            Body_FS : Frame_Store;
            R       : Reading;
         begin
            Inbox.Receive (Body_FS);
            R := Unmarshal (Body_FS.Data (0 .. Body_FS.Len - 1));
            Put_Line ("async reading" & Natural'Image (Got)
                      & ": id=0x" & Hex (R.Id)
                      & " value=" & F1 (R.Value)
                      & " label=""" & Label_Str (R) & """");
         end;
      end loop;
      --  RT terminates on its own after forwarding Total samples.
   end;

   Put_Line ("ALL OK");
end Example_Async;
