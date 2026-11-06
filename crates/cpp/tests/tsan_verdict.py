#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Runs the smoke program built with `-fsanitize=thread` and fails on a data
race in the C++ code (DDS-PSM-Cxx 1.0 §7.3: the operations are reentrant).

    python3 tsan_verdict.py <smoke_dds_psm_tsan> [mode]

libzerodds is Rust code the sanitizer does not instrument: its locks,
channels and atomics are invisible to it, and memory handed from one thread to
another through them is reported as a race. A report one of whose two accesses
has a frame in libzerodds is no verdict on the C++ headers and is counted, not
failed; a report located anywhere else, in the headers or in the test, fails.
The program's own verdict is its exit status and its `OK` line; the sanitizer
runs with `exitcode=0` so that it does not replace that status.

GCC's and clang's ThreadSanitizer stop with "unexpected memory mapping" on a
kernel whose address space randomization is wider than their shadow memory
assumes; `setarch -R` starts the program without randomization, where the
host allows it (a container's seccomp profile may refuse the personality).
"""
import os
import platform
import subprocess
import sys


def setarch_works():
    try:
        return subprocess.run(["setarch", platform.machine(), "-R", "true"],
                              capture_output=True).returncode == 0
    except OSError:
        return False


def main(argv):
    if len(argv) < 2:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    command = argv[1:]
    if setarch_works():
        command = ["setarch", platform.machine(), "-R"] + command
    env = dict(os.environ, TSAN_OPTIONS="halt_on_error=0 exitcode=0 second_deadlock_stack=1")
    result = subprocess.run(command, env=env, capture_output=True, text=True, errors="replace")
    sys.stdout.write(result.stdout)
    blocks = [block for block in result.stderr.split("==================")
              if "WARNING: ThreadSanitizer: data race" in block]

    def accesses(block):
        ends = [block.find(marker) for marker in ("Location is", "Thread T", "Mutex M", "SUMMARY:")
                if block.find(marker) >= 0]
        return block[:min(ends)] if ends else block

    own = [block for block in blocks if "libzerodds" not in accesses(block)]
    other = [line for line in result.stderr.splitlines()
             if line.strip() and not line.startswith("[smoke]")]
    print(f"ThreadSanitizer: {len(blocks)} reports, {len(blocks) - len(own)} in libzerodds, "
          f"{len(own)} elsewhere")
    if result.returncode != 0 or "OK" not in result.stdout:
        print(f"the program failed (exit {result.returncode}):", file=sys.stderr)
        print("\n".join(other[:60]), file=sys.stderr)
        return 1
    if own:
        print("data races in the C++ code:", file=sys.stderr)
        for block in own:
            print("\n".join(block.splitlines()[:24]), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
