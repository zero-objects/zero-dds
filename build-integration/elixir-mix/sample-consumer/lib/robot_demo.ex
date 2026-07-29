# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Robot.Pose through the *generated* marshal_xcdr/2 and
# unmarshal/2 — proof that the :zerodds_idl compiler actually ran before
# :elixir compiled this module's own dependency on Robot.Pose.
defmodule RobotDemo do
  def main do
    pose = %Robot.Pose{robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75}

    wire = Robot.Pose.marshal_xcdr(pose, :little)
    decoded = Robot.Pose.unmarshal(wire, :little)

    if decoded.robot_id != pose.robot_id or decoded.x != pose.x or decoded.y != pose.y or
         decoded.theta != pose.theta do
      raise "round-trip mismatch: #{inspect(decoded)}"
    end

    IO.puts(
      "OK: Robot.Pose round-tripped through generated marshal_xcdr/unmarshal (#{byte_size(wire)} wire bytes), robot_id=#{decoded.robot_id}"
    )
  end
end
