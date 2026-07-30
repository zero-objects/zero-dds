// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const mod = b.addModule("zerodds", .{ .root_source_file = b.path("src/zerodds.zig") });

    // Unit tests (byte-identity + sync + async). Run with the goldens in build/.
    const tests = b.addTest(.{
        .root_source_file = b.path("src/zerodds.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);

    // Runnable example.
    const exe = b.addExecutable(.{
        .name = "example",
        .root_source_file = b.path("example/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    exe.root_module.addImport("zerodds", mod);
    b.installArtifact(exe);
    const run_exe = b.addRunArtifact(exe);
    const run_step = b.step("run", "Run the example");
    run_step.dependOn(&run_exe.step);

    // Deeper sync + async examples (typed telemetry, full field decode).
    inline for (.{ "example_sync", "example_async" }) |name| {
        const ex = b.addExecutable(.{
            .name = name,
            .root_source_file = b.path(name ++ ".zig"),
            .target = target,
            .optimize = optimize,
        });
        ex.root_module.addImport("zerodds", mod);
        const run_ex = b.addRunArtifact(ex);
        const step = b.step("run-" ++ name, "Run the " ++ name);
        step.dependOn(&run_ex.step);
    }
}
