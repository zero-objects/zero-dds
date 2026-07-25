-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors

package body Sample_Sensor is

   function Encode (W : access Zdw_Writer; S : Sensor_Reading) return int is
      Ignore : int;
   begin
      Ignore := Put_U32 (W, S.Id);
      Ignore := Put_U16 (W, S.Kind);
      Ignore := Put_U8 (W, S.Flags);
      Ignore := Put_F32 (W, S.Value);
      Ignore := Put_U64 (W, S.Stamp);
      Ignore := Put_String (W, S.Label'Address);
      Ignore := Put_Seq_U8 (W, S.Raw'Address, S.Raw_Len);
      return W.Error;
   end Encode;

   function Decode (R : access Zdw_Reader; S : out Sensor_Reading) return int is
      Ignore  : int;
      V_Id    : aliased unsigned_long;
      V_Kind  : aliased unsigned;
      V_Flags : aliased unsigned_char;
      V_Val   : aliased C_float;
      V_Stamp : aliased Zdw_U64;
      V_N     : aliased size_t;
   begin
      S := (others => <>);
      Ignore := Get_U32 (R, V_Id'Access);
      S.Id := V_Id;
      Ignore := Get_U16 (R, V_Kind'Access);
      S.Kind := V_Kind;
      Ignore := Get_U8 (R, V_Flags'Access);
      S.Flags := V_Flags;
      Ignore := Get_F32 (R, V_Val'Access);
      S.Value := V_Val;
      Ignore := Get_U64 (R, V_Stamp'Access);
      S.Stamp := V_Stamp;
      Ignore := Get_String (R, S.Label'Address, Label_Cap);
      Ignore := Get_Seq_U8 (R, S.Raw'Address, Raw_Cap, V_N'Access);
      S.Raw_Len := V_N;
      return R.Error;
   end Decode;

end Sample_Sensor;
