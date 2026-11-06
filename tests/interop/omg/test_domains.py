# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Leases DDS domains to one process at a time, by the protocol of
`crates/test-domains` (the Rust tests) and of its Java and TypeScript copies.

One file `domain-<d>.lease` per domain in the directory `$ZERODDS_TEST_DOMAIN_DIR`
names (the volume the CI jobs of a runner host share; it must exist), else
`$TMPDIR/zerodds-dcps-domains` (else `/tmp`), created exclusively, holding
`pid <N> since <T> ns <pid namespace>`; a lease whose owner is gone is taken over
under `takeover.lock`. An owner in this PID namespace is alive while its PID is;
an owner in another one (another container of the host) while it refreshes the
file's modification time, which every holder does every HEARTBEAT_MS: a lease
older than HEARTBEAT_STALE_MS counts as given up. Leases come from the same
band: the domains up to MAX_DOMAIN whose RTPS port window lies outside the
ephemeral port range of the host and that are not in RESERVED_DOMAINS or
FOREIGN_DOMAINS, from the top down, and of those the host's share
(`in_share`): the CI runner host's jobs set `$ZERODDS_TEST_DOMAIN_BAND` to `ci`
and lease from CI_SHARE, every other host leases from the rest, so that two
hosts of the LAN never meet on a domain. `crates/test-domains/tests/py_lease_agrees.rs`
checks that the constants here are those of the Rust crate.

An OMG suite worker takes as many neighbouring domains as its cases name
(`lease_run`): the dds-rtps suite names domains 0 and 1 itself, shifted onto
the run by the driver.
"""
import atexit
import os
import tempfile
import threading
import time
from pathlib import Path

PORT_BASE = 7400
DOMAIN_GAIN = 250
OFFSET_SPDP_UNICAST = 10
PARTICIPANT_GAIN = 2
MAX_PARTICIPANT_INDEX = 119
MAX_DOMAIN = 231
RESERVED_DOMAINS = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 93, 94, 95, 96, 97, 98, 99,
    100,
]
FOREIGN_DOMAINS = [226, 230]
CI_SHARE_LOW = 219
CI_SHARE_HIGH = 231
BAND_VARIABLE = "ZERODDS_TEST_DOMAIN_BAND"
PATIENCE_MS = 180_000
HEARTBEAT_MS = 2_000
HEARTBEAT_STALE_MS = 20_000
LEASE_DIR_VARIABLE = "ZERODDS_TEST_DOMAIN_DIR"

_held = set()
_lock = threading.Lock()
_heartbeat = None


def lease_dir():
    named = os.environ.get(LEASE_DIR_VARIABLE)
    if named:
        path = Path(named)
        # Not created: a variable naming a missing directory means the shared
        # volume is not mounted, and a directory made here would arbitrate this
        # job alone while looking host-wide.
        if not path.is_dir():
            raise SystemExit(f"{LEASE_DIR_VARIABLE}={named} is not a directory: the lease "
                             "directory the jobs of this host share is not mounted")
        return path
    path = Path(os.environ.get("TMPDIR") or tempfile.gettempdir() or "/tmp") / "zerodds-dcps-domains"
    path.mkdir(parents=True, exist_ok=True)
    return path


def file(domain):
    return lease_dir() / ("domain-" + str(domain) + ".lease")


def ephemeral_port_range():
    try:
        low, high = (int(f) for f in
                     Path("/proc/sys/net/ipv4/ip_local_port_range").read_text().split()[:2])
        if low <= high:
            return low, high
    except (OSError, ValueError):
        pass
    return 32768, 65535


def domain_port_window(domain):
    low = PORT_BASE + DOMAIN_GAIN * domain
    return low, low + OFFSET_SPDP_UNICAST + PARTICIPANT_GAIN * MAX_PARTICIPANT_INDEX


def host_band():
    """Every domain whose port window is outside the ephemeral range and that
    neither this workspace nor a foreign process of the LAN holds fixed."""
    busy_low, busy_high = ephemeral_port_range()
    band = []
    for domain in range(MAX_DOMAIN, -1, -1):
        if domain in RESERVED_DOMAINS or domain in FOREIGN_DOMAINS:
            continue
        low, high = domain_port_window(domain)
        if high <= 65535 and (high < busy_low or low > busy_high):
            band.append(domain)
    return band


def in_share(band):
    """This host's share of the band, by `$ZERODDS_TEST_DOMAIN_BAND`: `ci` the
    domains in CI_SHARE (all of the band where none of them is in it: Windows
    and macOS, whose ephemeral range starts at 49152), `<low>-<high>` those in
    that range, unset the domains outside CI_SHARE."""
    named = (os.environ.get(BAND_VARIABLE) or "").strip()
    ci = [d for d in band if CI_SHARE_LOW <= d <= CI_SHARE_HIGH]
    if named == "ci":
        return ci or list(band)
    if named:
        try:
            low, high = (int(word) for word in named.split("-"))
        except ValueError:
            raise SystemExit(f"{BAND_VARIABLE}={named}: `ci` or `<low>-<high>`")
        return [d for d in band if low <= d <= high]
    return [d for d in band if d not in ci]


def safe_domains():
    band = in_share(host_band())
    if not band:
        raise SystemExit(f"no DDS domain in this host's share ({BAND_VARIABLE}="
                         f"{os.environ.get(BAND_VARIABLE, '')}) of the band {host_band()}")
    return band


def pid_namespace():
    try:
        ns = os.readlink("/proc/self/ns/pid")
    except OSError:
        return None
    return ns if ns and not any(c.isspace() for c in ns) else None


def write_claim(path):
    handle = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        line = "pid " + str(os.getpid()) + " since " + str(int(time.time()))
        ns = pid_namespace()
        if ns:
            line += " ns " + ns
        os.write(handle, (line + "\n").encode())
    finally:
        os.close(handle)


def heartbeat_is_fresh(path):
    try:
        age = time.time() - path.stat().st_mtime
    except FileNotFoundError:
        return False
    except OSError:
        return True
    return age < HEARTBEAT_STALE_MS / 1000


def owner_is_alive(owner, path):
    words = owner.split()
    try:
        pid = int(words[1])
    except (IndexError, ValueError):
        return True
    theirs = words[words.index("ns") + 1] if "ns" in words[:-1] else None
    ours = pid_namespace()
    if theirs and ours and theirs != ours:
        return heartbeat_is_fresh(path)
    return Path("/proc/" + str(pid)).exists()


def with_takeover_lock(body):
    path = lease_dir() / "takeover.lock"
    for _ in range(400):
        try:
            os.close(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644))
        except FileExistsError:
            try:
                stale = time.time() - path.stat().st_mtime > 30
            except OSError:
                stale = False
            if stale:
                path.unlink(missing_ok=True)
                continue
            time.sleep(0.005)
            continue
        try:
            return body()
        finally:
            path.unlink(missing_ok=True)
    return None


def claim(domain):
    path = file(domain)
    try:
        write_claim(path)
        return True
    except FileExistsError:
        def take_over():
            try:
                held = path.read_text().strip()
            except OSError:
                held = ""
            if owner_is_alive(held, path):
                return False
            path.unlink(missing_ok=True)
            try:
                write_claim(path)
            except FileExistsError:
                return False
            return True
        return with_takeover_lock(take_over) is True


def release(domain):
    with _lock:
        _held.discard(domain)
    file(domain).unlink(missing_ok=True)


def _beat():
    while True:
        time.sleep(HEARTBEAT_MS / 1000)
        with _lock:
            held = list(_held)
        for domain in held:
            try:
                os.utime(file(domain))
            except OSError:
                pass


def _start_heartbeat():
    global _heartbeat
    if _heartbeat is None:
        _heartbeat = threading.Thread(target=_beat, name="zerodds-domain-heartbeat", daemon=True)
        _heartbeat.start()
        atexit.register(lambda: [release(d) for d in list(_held)])


def lease_run(count):
    """`count` neighbouring domains of the band, `base` to `base + count - 1`,
    held until the process ends (or `release`); returns `base`."""
    band = safe_domains()
    bases = [d for d in sorted(band, reverse=True)
             if all(d + i in band for i in range(count))]
    deadline = time.monotonic() + PATIENCE_MS / 1000
    while True:
        for base in bases:
            with _lock:
                mine = set(_held)
            if any(base + i in mine for i in range(count)):
                continue
            taken = []
            for domain in range(base, base + count):
                if not claim(domain):
                    break
                taken.append(domain)
            if len(taken) == count:
                with _lock:
                    _held.update(taken)
                _start_heartbeat()
                return base
            for domain in taken:
                file(domain).unlink(missing_ok=True)
        if time.monotonic() > deadline:
            raise SystemExit(f"no {count} neighbouring DDS domains left in {lease_dir()} "
                             f"after {PATIENCE_MS // 1000} s")
        time.sleep(0.5)


def lease_pair():
    """Two neighbouring domains, `(base, base + 1)`: `lease_run(2)`."""
    return lease_run(2)
