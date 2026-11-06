-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The sample example_ada83_reliable sends as number I. This body writes I as
--  a u32 little-endian, byte for byte what the C app sends; the live E2E
--  (crates/endpoint-e2e/tests/ada83_reliable.rs) builds the example with a
--  body of its own that encodes the `Ping` of the codec the Ada 83 generator
--  writes from the IDL. Strict Ada 83.

with Zerodds_Ada83_Reliable;

package Ada83_Reliable_Sample is

   function Sample (I : Natural) return Zerodds_Ada83_Reliable.Frame_Store;

end Ada83_Reliable_Sample;
