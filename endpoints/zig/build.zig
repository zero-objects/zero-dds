// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const mod = b.addModule("zerodds", .{ .root_source_file = b.path("src/zerodds.zig") });

    // Zig 0.16 took `root_source_file`/`target`/`optimize` out of
    // `addTest` and `addExecutable`: both now take a `root_module` that
    // `b.createModule` builds from exactly those three fields, and
    // imports are declared on the module rather than on the artifact.

    // Unit tests (byte-identity + sync + async). Run with the goldens in build/.
    const tests = b.addTest(.{
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/zerodds.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);

    // Runnable example.
    const exe = b.addExecutable(.{
        .name = "example",
        .root_module = b.createModule(.{
            .root_source_file = b.path("example/main.zig"),
            .target = target,
            .optimize = optimize,
            .imports = &.{.{ .name = "zerodds", .module = mod }},
        }),
    });
    b.installArtifact(exe);
    const run_exe = b.addRunArtifact(exe);
    const run_step = b.step("run", "Run the example");
    run_step.dependOn(&run_exe.step);

    // Deeper sync + async examples (typed telemetry, full field decode).
    inline for (.{ "example_sync", "example_async" }) |name| {
        const ex = b.addExecutable(.{
            .name = name,
            .root_module = b.createModule(.{
                .root_source_file = b.path(name ++ ".zig"),
                .target = target,
                .optimize = optimize,
                .imports = &.{.{ .name = "zerodds", .module = mod }},
            }),
        });
        const run_ex = b.addRunArtifact(ex);
        const step = b.step("run-" ++ name, "Run the " ++ name);
        step.dependOn(&run_ex.step);
    }

    // The reliable-stream demo. It imports `src/reliable.zig` by relative
    // path rather than through the module, so no import is added here; the
    // step existed in the gate before it existed in this file, which is the
    // suite-nothing-runs defect by another route.
    const ex_rel = b.addExecutable(.{
        .name = "example_reliable",
        .root_module = b.createModule(.{
            .root_source_file = b.path("example_reliable.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    const run_ex_rel = b.addRunArtifact(ex_rel);
    const rel_step = b.step("run-example_reliable", "Run the example_reliable");
    rel_step.dependOn(&run_ex_rel.step);
}
