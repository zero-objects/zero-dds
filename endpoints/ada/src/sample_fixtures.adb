-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

with Interfaces.C; use Interfaces.C;

package body Sample_Fixtures is

   function Sensor_Fixture return Sensor_Reading is
      S   : Sensor_Reading;
      Src : constant char_array := To_C ("bay-12");
   begin
      S.Id    := 16#A1B2C3D4#;
      S.Kind  := 16#1234#;
      S.Flags := 16#5A#;
      S.Value := 3.5;
      S.Stamp := (Hi => 16#01020304#, Lo => 16#05060708#);
      S.Label (Src'Range) := Src;
      S.Raw (0) := 16#DE#;
      S.Raw (1) := 16#AD#;
      S.Raw (2) := 16#BE#;
      S.Raw (3) := 16#EF#;
      S.Raw_Len := 4;
      return S;
   end Sensor_Fixture;

end Sample_Fixtures;
