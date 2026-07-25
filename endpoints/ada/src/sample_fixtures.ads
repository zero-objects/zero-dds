-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  The fixed SensorReading test vector, shared by the byte-identity and the
--  UDP-loopback tests. MUST match endpoints/golden-gen/src/main.rs and
--  endpoints/c/test/test_byte_identity.c (`fill_sample`).

with Sample_Sensor; use Sample_Sensor;

package Sample_Fixtures is

   function Sensor_Fixture return Sensor_Reading;

end Sample_Fixtures;
