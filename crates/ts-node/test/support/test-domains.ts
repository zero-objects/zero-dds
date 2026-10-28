// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// test-domains.ts — leases a DDS domain to one test process at a time, by the
// protocol of `crates/test-domains` (the Rust tests) and of its Java copy
// `crates/java-omgdds/java/src/test/java/org/zerodds/testing/TestDomains.java`:
// one file `domain-<d>.lease` per domain in `$TMPDIR/zerodds-dcps-domains`
// (else `/tmp`), created exclusively, holding `pid <N> since <T>`; a lease whose
// owner is gone is taken over under `takeover.lock`. Leases come from the same
// band: the domains up to MAX_DOMAIN whose RTPS port window lies outside the
// ephemeral port range of the host and that are not in RESERVED_DOMAINS. On
// Linux (ephemeral 32768 to 60999) that is 215 to 231, 90 and 92.
//
// A domain is a host-wide resource: its number is the UDP port of its
// participants (DDSI-RTPS 2.5 §9.6.1.1). These tests used the fixed domains 0,
// 1, 7, 8, 9 and 55, which the Rust tests keep for themselves (55 is one of the
// `zerodds-c-api` tests'), and `100 + pid % 50`, whose ports from domain 101 on
// reach into the ephemeral range. `crates/test-domains/tests/ts_lease_agrees.rs`
// checks that the constants here are those of the Rust crate.

import { closeSync, existsSync, mkdirSync, openSync, readFileSync, statSync, unlinkSync, writeSync } from "node:fs";
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
/** How long a test waits for a domain of the band. */
const PATIENCE_MS = 180_000;

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
    if (RESERVED_DOMAINS.includes(domain)) continue;
    const low = PORT_BASE + DOMAIN_GAIN * domain;
    const high = low + OFFSET_SPDP_UNICAST + PARTICIPANT_GAIN * MAX_PARTICIPANT_INDEX;
    if (high <= 65535 && (high < busyLow || low > busyHigh)) band.push(domain);
  }
  if (band.length === 0) {
    throw new Error(`no DDS domain keeps its RTPS ports out of the ephemeral port range ${busyLow}..${busyHigh}`);
  }
  return band;
}

/** `$TMPDIR/zerodds-dcps-domains`, as Rust's `std::env::temp_dir()` reads it. */
function dir(): string {
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
    writeSync(fd, "pid " + process.pid + " since " + Math.floor(Date.now() / 1000) + "\n");
  } finally {
    closeSync(fd);
  }
  return true;
}

/** Whether `pid N since T` names a live process; an unreadable line counts as alive. */
function ownerIsAlive(owner: string): boolean {
  const pid = Number(owner.split(/\s+/)[1]);
  if (!Number.isInteger(pid) || pid <= 0) return true;
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
      if (ownerIsAlive(held)) return false;
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
