-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Interfaces;          use Interfaces;
with Zerodds_Native_Wire; use Zerodds_Native_Wire;
with Native_Reliable;     use Native_Reliable;

package body Native_Reliable_Sample is

   --  { uint32 id; float value; string label } with the index in the id (so
   --  gap-free delivery is observable on the peer side).
   function Sample (I : Natural) return Payload is
      W : Writer;
   begin
      Init (W, Little);
      Put_U32 (W, Unsigned_32 (I));
      Put_F32 (W, IEEE_Float_32 (Float (I) + 0.5));
      Put_String (W, "reliable-native");
      return Make_Payload (Bytes (W));
   end Sample;

end Native_Reliable_Sample;
