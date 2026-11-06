-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The sample example_native_reliable sends as number I. This body writes a
--  telemetry sample carrying I in its first u32; the live E2E
--  (crates/endpoint-e2e/tests/ada_native_reliable.rs) builds the example with
--  a body of its own that encodes the `Ping` of the codec the Ada generator
--  writes from the IDL.

with Native_Reliable;

package Native_Reliable_Sample is

   function Sample (I : Natural) return Native_Reliable.Payload;

end Native_Reliable_Sample;
