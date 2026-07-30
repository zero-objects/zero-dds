# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Pose through the *generated* marshal_xcdr/unmarshal_xcdr_Pose
# — proof that deps/build.jl actually ran zerodds-idlc before this module
# was loaded (src/gen/Robot.jl does not exist in the source tree).
module RobotDemo

include(joinpath(@__DIR__, "gen", "Robot.jl"))

function main()
    pose = Pose("r2d2", 1.5, -2.5, 0.75)

    wire = marshal_xcdr(pose, LE)
    decoded = unmarshal_xcdr_Pose(wire, LE)

    @assert decoded.robot_id == pose.robot_id
    @assert decoded.x == pose.x
    @assert decoded.y == pose.y
    @assert decoded.theta == pose.theta

    println("OK: Pose round-tripped through generated marshal_xcdr/unmarshal_xcdr_Pose (",
            length(wire), " wire bytes), robot_id=", decoded.robot_id)
end

end # module RobotDemo

RobotDemo.main()
