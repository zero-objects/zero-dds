-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Round-trips a Robot::Pose through the *generated* zd_enc_Robot_Pose /
-- zd_dec_Robot_Pose -- proof that the Makefile wrapper ran zerodds-idlc
-- before gprbuild compiled this unit (generated/zd_types.ads is not in the
-- source tree). Zd_Wire is the runtime in runtime/ada.
with Ada.Text_IO;             use Ada.Text_IO;
with Ada.Strings.Unbounded;   use Ada.Strings.Unbounded;
with Interfaces;              use type Interfaces.IEEE_Float_64;
with Zd_Wire;                 use type Zd_Wire.Status;
with Zd_Types;

procedure Main is
   Framing  : constant Zd_Wire.Pl_Cdr1_Framing := (others => <>);
   Original : constant Zd_Types.Robot_Pose :=
     (Robot_Id => To_Unbounded_String ("r2d2"),
      X        => 1.5,
      Y        => -2.5,
      Theta    => 0.75);
   W : Zd_Wire.Writer :=
     Zd_Wire.New_Writer (Zd_Types.Zd_Endianness, 4, Framing, False);
begin
   Zd_Types.zd_enc_Robot_Pose (W, Original);
   declare
      Wire    : constant Zd_Wire.Byte_Array_Access := Zd_Wire.Finish (W);
      R       : Zd_Wire.Reader :=
        Zd_Wire.New_Reader (Wire, Zd_Types.Zd_Endianness, 4, Framing);
      Decoded : constant Zd_Types.Robot_Pose :=
        Zd_Types.zd_dec_Robot_Pose (R, Zd_Wire.Tc_Discard);
   begin
      if Zd_Wire.Reader_Status (R) /= Zd_Wire.Status_Ok
        or else Decoded.Robot_Id /= Original.Robot_Id
        or else Decoded.X /= Original.X
        or else Decoded.Y /= Original.Y
        or else Decoded.Theta /= Original.Theta
      then
         raise Program_Error with "round-trip mismatch";
      end if;

      Put_Line
        ("OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose ("
         & Wire'Length'Image & " wire bytes), robot_id="
         & To_String (Decoded.Robot_Id));
   end;
end Main;
