# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Robot::Pose through the *generated* zd_enc_Robot_Pose/
# zd_dec_Robot_Pose — proof that deps/build.jl ran zerodds-idlc before this
# module was loaded (src/gen/Robot.jl is not in the source tree). The
# generated file names `.ZeroddsWire`, the wire runtime in runtime/julia.
module RobotDemo

include(joinpath(@__DIR__, "..", "..", "..", "..", "runtime", "julia", "zerodds_wire.jl"))
include(joinpath(@__DIR__, "gen", "Robot.jl"))

function main()
    pose = Robot_Pose("r2d2", 1.5, -2.5, 0.75)

    w = ZdWriter(ZD_ENDIANNESS, 4)
    zd_enc_Robot_Pose(w, pose)
    wire = zd_writer_bytes(w)

    r = ZdReader(wire, ZD_ENDIANNESS, 4)
    decoded = zd_dec_Robot_Pose(r, ZD_TRY_CONSTRUCT_DISCARD)

    @assert reader_status(r) == ZD_STATUS_OK
    @assert decoded.robot_id == pose.robot_id
    @assert decoded.x == pose.x
    @assert decoded.y == pose.y
    @assert decoded.theta == pose.theta

    println("OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (",
            length(wire), " wire bytes), robot_id=", decoded.robot_id)
end

end # module RobotDemo

# Run as a script, not while `Pkg.build()` precompiles the package.
if abspath(PROGRAM_FILE) == @__FILE__
    RobotDemo.main()
end
