-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Interfaces.C;
with Deep_Reading; use Deep_Reading;

package body Reliable_Sample is

   --  A telemetry sample carrying its index in Id (so gap-free delivery is
   --  observable on the peer side).
   function Sample (I : Natural) return Frame_Store is
      Rd : Reading;
   begin
      Rd.Id := Interfaces.C.unsigned_long (I);
      Rd.Value := Interfaces.C.C_float (Float (I) + 0.5);
      Set_Label (Rd, "reliable");
      return Marshal (Rd);
   end Sample;

end Reliable_Sample;
