#!/usr/bin/env bash
# Does any ROS 2 type declare an extensibility?
#
# `rmw_zerodds_create_publisher` states FINAL for every topic, and the reason
# is a measurement rather than an assumption: the rmw entry point takes a type
# *name*, the introspection struct behind it carries no extensibility field,
# and so there is no value in that path to pass on. What settles it is what
# ROS types actually are.
#
# Measured 2026-09-05 against ROS 2 Humble: 224 generated `.idl` files, not one
# carrying `@appendable`, `@mutable` or `@final`, and `rosidl` emitting no
# extensibility at all. Every ROS type on that path is plain CDR, which is what
# FINAL frames.
#
# This script repeats that measurement. It exits non-zero the moment a ROS
# package ships a type with a declared extensibility, because that is the day
# the stated FINAL becomes wrong — a hand-written `.idl` in a ROS package is
# allowed, it has simply never happened in 224 files. Failing here is cheaper
# than an `@appendable` body framed as plain CDR, which OpenDDS reads with the
# DHEADER as its first member: delivered, with shifted values, refused by
# nothing.
#
# Usage:  ROS_PREFIX=/root/micromamba/envs/ros2 tools/ros-extensibility-scan.sh
#
# Exit status:
#   0  no ROS type declares an extensibility — the stated FINAL holds
#   1  at least one does; each is named
#   2  no ROS installation to measure (never a pass)
set -u

prefix="${ROS_PREFIX:-/root/micromamba/envs/ros2}"
if [ ! -d "$prefix/share" ]; then
    echo "ros-extensibility-scan: no ROS tree at $prefix/share" >&2
    echo "  set ROS_PREFIX. This is not a pass: nothing was measured." >&2
    exit 2
fi

total=$(find "$prefix/share" -name '*.idl' 2>/dev/null | wc -l | tr -d ' ')
if [ "$total" -eq 0 ]; then
    echo "ros-extensibility-scan: $prefix/share holds no .idl files" >&2
    echo "  nothing was measured, so nothing is confirmed." >&2
    exit 2
fi

# `@extensibility(...)` as well as the three shorthands: a type is declared
# either way, and only the shorthands would be found by a search for them.
declared=$(grep -rlE '@(appendable|mutable|final|extensibility)' \
    "$prefix/share" --include='*.idl' 2>/dev/null || true)

if [ -n "$declared" ]; then
    n=$(printf '%s\n' "$declared" | wc -l | tr -d ' ')
    echo "ros-extensibility-scan: $n of $total ROS .idl files declare an extensibility."
    echo
    printf '%s\n' "$declared" | while IFS= read -r f; do
        printf '  %s\n' "${f#"$prefix/share/"}"
        grep -nE '@(appendable|mutable|final|extensibility)' "$f" 2>/dev/null |
            head -3 | sed 's/^/      /'
    done
    echo
    echo "The shim states FINAL for every ROS topic — see the comment at"
    echo "\`rmw_zerodds_create_publisher\` in crates/rmw-zerodds-shim/src/lib.rs."
    echo "For a non-final type that is now wrong: its body would be framed as"
    echo "plain CDR, and a strict reader takes the DHEADER for the first member."
    exit 1
fi

echo "ros-extensibility-scan: 0 of $total ROS .idl files declare an extensibility."
echo "The stated FINAL holds for every type on this path."
