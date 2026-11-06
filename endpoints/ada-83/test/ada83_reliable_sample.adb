-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Zerodds_Ada83_Wire;     use Zerodds_Ada83_Wire;
with Zerodds_Ada83_Reliable; use Zerodds_Ada83_Reliable;

package body Ada83_Reliable_Sample is

   function Sample (I : Natural) return Frame_Store is
      W  : Writer;
      FS : Frame_Store;
   begin
      Init (W, Little);
      Put_U32 (W, Long_Integer (I));
      FS.Len := Length (W);
      for K in 0 .. Length (W) - 1 loop
         FS.Data (K) := W.Buf (K);
      end loop;
      return FS;
   end Sample;

end Ada83_Reliable_Sample;
