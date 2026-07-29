# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# `deps/build.jl` — the idiomatic native Julia package build step, run by
# `Pkg.build("RobotDemo")` (and automatically after `Pkg.add`/`Pkg.develop`
# for a package that declares a `deps/build.jl`). Mirrors `zerodds-build` /
# CMake's `zerodds_idlc_generate()` for the Julia ecosystem.
#
# Staleness check: only re-invokes zerodds-idlc when the .idl is newer
# than the last-generated .jl (or the output is missing) — `Pkg.build`
# itself has no incremental-rebuild concept (it always re-runs
# build.jl), so this script does its own mtime comparison, the same
# thing dub's preGenerateCommands (no native staleness either) is
# documented as lacking in ../../d-dub/README.md.

const project_root = normpath(joinpath(@__DIR__, ".."))
const idl_file = joinpath(project_root, "idl", "Robot.idl")
const out_dir = joinpath(project_root, "src", "gen")
const out_file = joinpath(out_dir, "Robot.jl")
const executable = get(ENV, "ZERODDS_IDLC", "zerodds-idlc")

mkpath(out_dir)

needs_regen = !isfile(out_file) || mtime(idl_file) > mtime(out_file)

if needs_regen
    println("zerodds-idlc: generating ", out_file, " from ", idl_file)
    cmd = `$executable generate $idl_file --julia -o $out_dir`
    run(cmd)
else
    println("zerodds-idlc: ", out_file, " up to date, skipping regeneration")
end
