// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// test-domains.ts — leases a DDS domain to one test process at a time, by the
// protocol of `crates/test-domains` (the Rust tests) and of its Java copy
// `crates/java-omgdds/java/src/test/java/org/zerodds/testing/TestDomains.java`:
// one file `domain-<d>.lease` per domain in the directory `$ZERODDS_TEST_DOMAIN_DIR`
// names (the volume the CI jobs of a runner host share; it must exist), else
// `$TMPDIR/zerodds-dcps-domains` (else `/tmp`), created exclusively, holding
// `pid <N> since <T> ns <pid namespace>`; a lease whose owner is gone is taken
// over under `takeover.lock`. An owner in this PID namespace is alive while its
// PID is; an owner in another one (another container of the host) while it
// refreshes the file's modification time, which every holder does every
// HEARTBEAT_MS: a lease older than HEARTBEAT_STALE_MS counts as given up. Leases come from the same
// band: the domains up to MAX_DOMAIN whose RTPS port window lies outside the
// ephemeral port range of the host and that are not in RESERVED_DOMAINS or
// FOREIGN_DOMAINS. On Linux (ephemeral 32768 to 60999) that is 215 to 231
// without 226 and 230, 90 and 92. Of those a host leases its share (`share`): the CI
// runner host's jobs set `$ZERODDS_TEST_DOMAIN_BAND` to `ci` and lease from
// CI_SHARE_LOW to CI_SHARE_HIGH, every other host from the rest, because SPDP
// multicast reaches every host of the LAN.
//
// A domain is a host-wide resource: its number is the UDP port of its
// participants (DDSI-RTPS 2.5 §9.6.1.1). These tests used the fixed domains 0,
// 1, 7, 8, 9 and 55, which the Rust tests keep for themselves (55 is one of the
// `zerodds-c-api` tests'), and `100 + pid % 50`, whose ports from domain 101 on
// reach into the ephemeral range. `crates/test-domains/tests/ts_lease_agrees.rs`
// checks that the constants here are those of the Rust crate.

import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readlinkSync, statSync, unlinkSync, utimesSync, writeSync } from "node:fs";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

/** RTPS §9.6.1.1: the port base. */
const PORT_BASE = 7400;
/** The distance of two neighbouring domains. */
const DOMAIN_GAIN = 250;
/** The SPDP unicast offset inside a domain. */
const OFFSET_SPDP_UNICAST = 10;
/** The distance of two participants of one domain. */
const PARTICIPANT_GAIN = 2;
/** The highest participant index a participant of a domain takes. */
const MAX_PARTICIPANT_INDEX = 119;
/** The highest domain whose port window fits in a u16. */
const MAX_DOMAIN = 231;
/** The domains that stay hard-coded in the Rust tests and are never leased out. */
const RESERVED_DOMAINS: readonly number[] = [
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
  26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
  50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
  74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 93, 94, 95, 96, 97, 98, 99,
  100,
];
/** Domains processes outside this workspace use on the LAN: 226 and 230. */
const FOREIGN_DOMAINS: readonly number[] = [226, 230];
/** The lowest domain of the CI runner host's share. */
const CI_SHARE_LOW = 219;
/** The highest domain of the CI runner host's share. */
const CI_SHARE_HIGH = 231;
/** The environment variable that names this host's share of the band. */
const BAND_VARIABLE = "ZERODDS_TEST_DOMAIN_BAND";
/** How long a test waits for a domain of the band. */
const PATIENCE_MS = 180_000;
/** How often a holder refreshes the modification time of its lease files. */
const HEARTBEAT_MS = 2_000;
/** How old the lease of another PID namespace may be before its owner counts as gone. */
const HEARTBEAT_STALE_MS = 20_000;
/** The environment variable that names the lease directory the jobs of a host share. */
const LEASE_DIR_VARIABLE = "ZERODDS_TEST_DOMAIN_DIR";
let heartbeat: ReturnType<typeof setInterval> | undefined;

/** Domains this process holds. */
const leasedHere = new Set<number>();
let releaseOnExit = false;

/** Blocks the thread for `ms` (the lease is taken before a test's participant exists). */
function pause(ms: number): void {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/** The port range the kernel hands out for "any port", read from the kernel. */
function ephemeralPortRange(): [number, number] {
  try {
    const [low, high] = readFileSync("/proc/sys/net/ipv4/ip_local_port_range", "utf8")
      .trim().split(/\s+/).map(Number);
    if (Number.isInteger(low) && Number.isInteger(high) && low <= high) return [low, high];
  } catch {
    // not Linux: sysctl below
  }
  try {
    const [low, high] = execFileSync("sysctl", ["-n", "net.inet.ip.portrange.first", "net.inet.ip.portrange.last"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim().split(/\s+/).map(Number);
    if (Number.isInteger(low) && Number.isInteger(high) && low <= high) return [low, high];
  } catch {
    // the union below
  }
  // Neither kernel answered: the union of what both platforms use.
  return [32768, 65535];
}

/** The domains of the band, the high ones first. */
function safeDomains(): number[] {
  const [busyLow, busyHigh] = ephemeralPortRange();
  const band: number[] = [];
  for (let domain = MAX_DOMAIN; domain >= 0; domain--) {
    if (RESERVED_DOMAINS.includes(domain) || FOREIGN_DOMAINS.includes(domain)) continue;
    const low = PORT_BASE + DOMAIN_GAIN * domain;
    const high = low + OFFSET_SPDP_UNICAST + PARTICIPANT_GAIN * MAX_PARTICIPANT_INDEX;
    if (high <= 65535 && (high < busyLow || low > busyHigh)) band.push(domain);
  }
  if (band.length === 0) {
    throw new Error(`no DDS domain keeps its RTPS ports out of the ephemeral port range ${busyLow}..${busyHigh}`);
  }
  const named = (process.env[BAND_VARIABLE] ?? "").trim();
  const mine = share(band, named);
  if (mine.length === 0) {
    throw new Error(`no DDS domain in this host's share (${BAND_VARIABLE}=${named}) of the band ${band.join(" ")}`);
  }
  return mine;
}

/**
 * This host's share of the band: `ci` the domains from CI_SHARE_LOW to
 * CI_SHARE_HIGH (the whole band where none of them is in it: Windows and
 * macOS), `<low>-<high>` those in that range, empty the domains outside the CI
 * share.
 */
function share(band: number[], named: string): number[] {
  const ci = band.filter((domain) => domain >= CI_SHARE_LOW && domain <= CI_SHARE_HIGH);
  if (named === "") return band.filter((domain) => !ci.includes(domain));
  if (named === "ci") return ci.length > 0 ? ci : band;
  const bounds = named.split("-").map((word) => Number.parseInt(word.trim(), 10));
  if (bounds.length !== 2 || bounds.some(Number.isNaN)) {
    throw new Error(`${BAND_VARIABLE}=${named}: \`ci\` or \`<low>-<high>\``);
  }
  return band.filter((domain) => domain >= bounds[0] && domain <= bounds[1]);
}

/** The PID namespace of this process, `pid:[<inode>]`, where Linux names it. */
function pidNamespace(): string | undefined {
  try {
    const ns = readlinkSync("/proc/self/ns/pid");
    return ns !== "" && !/\s/.test(ns) ? ns : undefined;
  } catch {
    return undefined;
  }
}

/** Refreshes the modification time of every held lease, for readers in another PID namespace. */
function startHeartbeat(): void {
  if (heartbeat !== undefined) return;
  heartbeat = setInterval(() => {
    const now = new Date();
    for (const domain of leasedHere) {
      try { utimesSync(file(domain), now, now); } catch { /* given back meanwhile */ }
    }
  }, HEARTBEAT_MS);
  heartbeat.unref();
}

/**
 * The directory `$ZERODDS_TEST_DOMAIN_DIR` names, which must exist (a variable
 * naming a directory that is not there means the shared volume is not mounted);
 * else `$TMPDIR/zerodds-dcps-domains`, as Rust's `std::env::temp_dir()` reads it.
 */
function dir(): string {
  const shared = process.env[LEASE_DIR_VARIABLE];
  if (shared !== undefined && shared !== "") {
    if (!existsSync(shared) || !statSync(shared).isDirectory()) {
      throw new Error(`${LEASE_DIR_VARIABLE}=${shared} is not a directory: the lease directory the jobs of this host share is not mounted`);
    }
    return shared;
  }
  const tmp = process.env.TMPDIR;
  const path = join(tmp !== undefined && tmp !== "" ? tmp : "/tmp", "zerodds-dcps-domains");
  mkdirSync(path, { recursive: true });
  return path;
}

function file(domain: number): string {
  return join(dir(), "domain-" + domain + ".lease");
}

/** Creates `path` exclusively with the claim line; false when it exists. */
function writeClaim(path: string): boolean {
  let fd: number;
  try {
    fd = openSync(path, "wx");
  } catch (why) {
    if ((why as NodeJS.ErrnoException).code === "EEXIST") return false;
    throw why;
  }
  try {
    const ns = pidNamespace();
    writeSync(fd, "pid " + process.pid + " since " + Math.floor(Date.now() / 1000) + (ns !== undefined ? " ns " + ns : "") + "\n");
  } finally {
    closeSync(fd);
  }
  return true;
}

/**
 * Whether `pid N since T [ns NS]` names a live owner; an unreadable line counts
 * as alive. An owner in another PID namespace is alive while its heartbeat is fresh.
 */
function ownerIsAlive(owner: string, path: string): boolean {
  const words = owner.split(/\s+/);
  const pid = Number(words[1]);
  if (!Number.isInteger(pid) || pid <= 0) return true;
  const at = words.indexOf("ns");
  const theirs = at >= 0 ? words[at + 1] : undefined;
  const ours = pidNamespace();
  if (theirs !== undefined && ours !== undefined && theirs !== ours) {
    try {
      return Date.now() - statSync(path).mtimeMs < HEARTBEAT_STALE_MS;
    } catch (why) {
      return (why as NodeJS.ErrnoException).code !== "ENOENT";
    }
  }
  if (existsSync("/proc/" + pid)) return true;
  if (existsSync("/proc/self")) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (why) {
    return (why as NodeJS.ErrnoException).code === "EPERM";
  }
}

function claim(domain: number): boolean {
  const path = file(domain);
  if (writeClaim(path)) return true;
  const lock = join(dir(), "takeover.lock");
  for (let attempt = 0; attempt < 400; attempt++) {
    let fd: number;
    try {
      fd = openSync(lock, "wx");
    } catch (why) {
      if ((why as NodeJS.ErrnoException).code !== "EEXIST") return false;
      let age = 0;
      try {
        age = Date.now() - statSync(lock).mtimeMs;
      } catch {
        continue; // removed meanwhile
      }
      if (age > 30_000) {
        try { unlinkSync(lock); } catch { /* another process took it */ }
        continue;
      }
      pause(5);
      continue;
    }
    closeSync(fd);
    try {
      let held = "";
      try { held = readFileSync(path, "utf8").trim(); } catch { /* given back meanwhile */ }
      // An empty line is a claim still being written: alive, as in the Rust crate.
      if (ownerIsAlive(held, path)) return false;
      try { unlinkSync(path); } catch { /* given back meanwhile */ }
      return writeClaim(path);
    } finally {
      try { unlinkSync(lock); } catch { /* already gone */ }
    }
  }
  return false;
}

function release(domain: number): void {
  leasedHere.delete(domain);
  try { unlinkSync(file(domain)); } catch { /* the next process that finds the owner gone takes it over */ }
}

/**
 * A domain no other process of the host uses, held until this process exits.
 * Waits up to three minutes for one of the band.
 */
export function uniqueDomain(): number {
  const end = Date.now() + PATIENCE_MS;
  for (;;) {
    for (const domain of safeDomains()) {
      if (leasedHere.has(domain)) continue;
      if (claim(domain)) {
        leasedHere.add(domain);
        startHeartbeat();
        if (!releaseOnExit) {
          releaseOnExit = true;
          process.on("exit", () => { for (const held of [...leasedHere]) release(held); });
        }
        return domain;
      }
    }
    if (Date.now() >= end) {
      throw new Error(`no DDS domain left after ${PATIENCE_MS / 1000} s: every domain of the band is leased by a live process (${dir()})`);
    }
    pause(500);
  }
}
