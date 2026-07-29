-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Round-trips a Pose through the *generated* Robot.Marshal/Unmarshal --
-- proof that the Makefile wrapper actually ran zerodds-idlc before
-- gprbuild compiled this unit (generated/robot.ads does not exist in the
-- source tree).
with Ada.Text_IO;             use Ada.Text_IO;
with Ada.Strings.Unbounded;   use Ada.Strings.Unbounded;
with Robot;                   use Robot;

procedure Main is
   Original : constant Robot.Pose :=
     (robot_id => To_Unbounded_String ("r2d2"),
      x        => 1.5,
      y        => -2.5,
      theta    => 0.75);

   Wire    : constant Byte_Array := Robot.Marshal (Original, Little);
   Decoded : constant Robot.Pose := Robot.Unmarshal (Wire, Little);
begin
   if Decoded.robot_id /= Original.robot_id
     or else Decoded.x /= Original.x
     or else Decoded.y /= Original.y
     or else Decoded.theta /= Original.theta
   then
      raise Program_Error with "round-trip mismatch";
   end if;

   Put_Line
     ("OK: Pose round-tripped through generated Marshal/Unmarshal (" &
      Wire'Length'Image & " wire bytes), robot_id=" &
      To_String (Decoded.robot_id));
end Main;
