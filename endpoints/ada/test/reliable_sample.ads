-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The sample example_reliable sends as number I. This body marshals a
--  telemetry `Reading` carrying I in its Id; the live E2E
--  (crates/endpoint-e2e/tests/ada_reliable.rs) builds the example with a body
--  of its own that encodes the `Ping` of the codec the Ada generator writes
--  from the IDL.

with Deep_Reading;

package Reliable_Sample is

   function Sample (I : Natural) return Deep_Reading.Frame_Store;

end Reliable_Sample;
