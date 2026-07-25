-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
--  A representative @final type and its fixed XCDR2 codec, mirroring
--  endpoints/c/test/sample_sensor.[hc] field-for-field. Stands in for what the
--  Ada `wire-fixed` codegen would emit per IDL type (ADR 0013, Stage 1).

with Interfaces.C; use Interfaces.C;
with Zdw;           use Zdw;

package Sample_Sensor is

   Label_Cap : constant := 64;
   Raw_Cap   : constant := 64;

   type Byte_Array is array (size_t range <>) of aliased unsigned_char;

   type Sensor_Reading is record
      Id      : unsigned_long := 0;                      -- uint32
      Kind    : unsigned      := 0;                      -- uint16
      Flags   : unsigned_char := 0;                      -- uint8
      Value   : C_float       := 0.0;                    -- float
      Stamp   : Zdw_U64;                                 -- uint64
      Label   : char_array (0 .. Label_Cap - 1) := [others => nul]; -- string
      Raw     : Byte_Array (0 .. Raw_Cap - 1)   := [others => 0];   -- seq<octet>
      Raw_Len : size_t        := 0;
   end record;

   --  Encode `S` into writer `W`; returns the writer's sticky error code.
   function Encode (W : access Zdw_Writer; S : Sensor_Reading) return int;

   --  Decode from reader `R` into `S`; returns the reader's sticky error code.
   function Decode (R : access Zdw_Reader; S : out Sensor_Reading) return int;

end Sample_Sensor;
