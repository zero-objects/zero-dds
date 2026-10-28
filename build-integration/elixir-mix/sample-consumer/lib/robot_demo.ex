# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Robot::Pose through the *generated* ZdCodec — proof that
# the :zerodds_idl compiler ran before :elixir compiled this module's use
# of Robot.Pose. The wire runtime is runtime/elixir (:zerodds_wire).
defmodule RobotDemo do
  def main do
    pose = %Robot.Pose{robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75}

    w = ZeroddsWire.Writer.new(ZdEndian.value(), 4)
    w = ZdCodec.zd_enc_Robot_Pose(w, pose)
    wire = ZeroddsWire.Writer.finish(w)

    r = ZeroddsWire.Reader.new(wire, ZdEndian.value(), 4)
    {decoded, r} = ZdCodec.zd_dec_Robot_Pose(r, :discard)

    if ZeroddsWire.Reader.reader_status(r) != :ok or decoded.robot_id != pose.robot_id or
         decoded.x != pose.x or decoded.y != pose.y or decoded.theta != pose.theta do
      raise "round-trip mismatch: #{inspect(decoded)}"
    end

    IO.puts(
      "OK: Robot::Pose round-tripped through generated ZdCodec (#{byte_size(wire)} wire bytes), robot_id=#{decoded.robot_id}"
    )
  end
end
