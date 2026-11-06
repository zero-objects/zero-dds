#!/usr/bin/env python3
"""Runs a command with DDS domains leased for it.

    python3 tests/integration/with_domains.py <count> -- <command>...

Leases `count` domains through the protocol of crates/test-domains (the
Python copy in tests/interop/omg/test_domains.py, held to the Rust constants
by crates/test-domains/tests/py_lease_agrees.rs), hands them to the command
as `ZERODDS_TEST_DOMAINS` (comma-separated, the variable the C++ smoke
program and the cargo-driven tests read), keeps them alive by the lease
heartbeat while the command runs, and gives them back after it.
"""
import os
from pathlib import Path
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "interop" / "omg"))
import test_domains  # noqa: E402


def lease(count):
    deadline = time.monotonic() + test_domains.PATIENCE_MS / 1000
    held = []
    while len(held) < count:
        # The host's share of the band, without the domains foreign
        # processes use on the LAN (`$ZERODDS_TEST_DOMAIN_BAND`).
        for domain in test_domains.safe_domains():
            if domain in held:
                continue
            if test_domains.claim(domain):
                held.append(domain)
                with test_domains._lock:
                    test_domains._held.add(domain)
                test_domains._start_heartbeat()
                if len(held) == count:
                    break
        if len(held) < count:
            if time.monotonic() > deadline:
                raise SystemExit(f"with_domains: {len(held)} of {count} DDS domains free "
                                 f"in {test_domains.lease_dir()}")
            # The band is held by other live processes; the leases of
            # owners that have gone are taken over on the next pass.
            time.sleep(0.5)
    return held


def main(argv):
    if len(argv) < 4 or argv[2] != "--":
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    domains = lease(int(argv[1]))
    env = dict(os.environ, ZERODDS_TEST_DOMAINS=",".join(str(d) for d in domains))
    print("with_domains: ZERODDS_TEST_DOMAINS=" + env["ZERODDS_TEST_DOMAINS"], flush=True)
    try:
        return subprocess.run(argv[3:], env=env).returncode
    finally:
        for domain in domains:
            test_domains.release(domain)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
