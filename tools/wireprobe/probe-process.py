#!/usr/bin/env python3
"""Bound one probe phase and drain its process group. Exit 125 is infrastructure.

Nested supervisors receive TERM before KILL, allowing their own isolated groups
to be drained as well. Linux additionally adopts and reaps orphan descendants.
Elsewhere cleanup waits the direct child and terminates owned process groups,
not arbitrary daemonized descendants. Never signals unrelated processes.
"""
import argparse
import ctypes
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

INFRASTRUCTURE = 125


def limit_open_files():
    # Avoid allocating BEAM descriptor tables for Docker's inherited billion-
    # descriptor limit. Only lower limits of this supervisor and its children.
    import resource
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    bounded = lambda value: 65536 if value == resource.RLIM_INFINITY else min(value, 65536)
    resource.setrlimit(resource.RLIMIT_NOFILE, (bounded(soft), bounded(hard)))


def adopt_orphans():
    # Linux CI: orphaned grandchildren become ours, not init's zombies. This
    # also lets cleanup reach an orphaned nested supervisor's separate group.
    if sys.platform == "linux":
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), "PR_SET_CHILD_SUBREAPER")


def adopted_children():
    if sys.platform != "linux":
        return []
    return [int(pid) for pid in Path(f"/proc/self/task/{os.getpid()}/children").read_text().split()]


def reap_finished_children():
    """Call only after Popen has waited the direct child.

    A subreaper inherits zombies as well as running children. Erlang's
    inet_gethost/erl_child_setup can already be dead when the VM exits.
    Reaping them is our responsibility, not evidence of a live leak.
    """
    while True:
        try:
            pid, _ = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            break
        if pid == 0:
            break


def drain_adopted():
    # Only our current children, after the original direct child was waited.
    # Kill/reap in rounds because terminating a parent can orphan its children.
    for _ in range(100):
        children = adopted_children()
        if not children:
            return
        for pid in children:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        reap_finished_children()
        time.sleep(0.01)
    raise RuntimeError("owned descendants did not terminate")


def positive_seconds(value):
    try:
        number = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be an integer") from error
    if not 1 <= number <= 86400:
        raise argparse.ArgumentTypeError("timeout must be in 1..86400 seconds")
    return number


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def stop_group(process, grace):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    deadline = time.monotonic() + grace
    while group_exists(process.pid) and time.monotonic() < deadline:
        process.poll()
        time.sleep(0.05)
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()
    drain_adopted()


def run(command, timeout, grace):
    interrupted = []
    saved = {}
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        saved[sig] = signal.signal(sig, lambda signum, _frame: interrupted.append(signum))
    process = None
    result = INFRASTRUCTURE
    try:
        limit_open_files()
        adopt_orphans()
        process = subprocess.Popen(command, start_new_session=True)
        deadline = time.monotonic() + timeout
        while process.poll() is None:
            if interrupted or time.monotonic() >= deadline:
                cause = f"signal {interrupted[0]}" if interrupted else f"timeout {timeout}s"
                print(f"probe-process: infrastructure failure: {cause}", file=sys.stderr)
                break
            time.sleep(0.05)
        code = process.poll()
        # poll() has reaped the direct child before waitpid(-1) is safe.
        if code is not None:
            reap_finished_children()
        if code is None or interrupted or code < 0 or code >= 124:
            print(f"probe-process: infrastructure failure: child status {code}", file=sys.stderr)
        elif group_exists(process.pid) or adopted_children():
            print("probe-process: infrastructure failure: child left descendants", file=sys.stderr)
        else:
            result = code
    except OSError as error:
        print(f"probe-process: infrastructure failure: {error}", file=sys.stderr)
    finally:
        if process is not None:
            try:
                stop_group(process, grace)
            except (OSError, RuntimeError) as error:
                print(f"probe-process: infrastructure cleanup failure: {error}", file=sys.stderr)
                result = INFRASTRUCTURE
        for sig, handler in saved.items():
            signal.signal(sig, handler)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=positive_seconds, required=True)
    parser.add_argument("--grace", type=positive_seconds, default=3)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("command required")
    return run(command, args.timeout, args.grace)


if __name__ == "__main__":
    sys.exit(main())
