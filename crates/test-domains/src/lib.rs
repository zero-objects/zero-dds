//! Crate `zerodds-test-domains`. Safety classification: **SAFE**.
//!
//! Leases a DDS domain to one test process at a time.
//!
//! # Why a crate and not a helper in one test directory
//!
//! A DDS domain number *is* a UDP port number: RTPS §9.6.1.4.1 puts a
//! participant's SPDP multicast socket on `7400 + 250*domain`. So the
//! domain is a resource of the whole host, and every test binary in the
//! workspace competes for it — `zerodds-dcps`, `zerodds-c-api`,
//! `zerodds-durability-service`, `zerodds-routing-service`, and the
//! daemons those spawn. A helper private to one crate's `tests/`
//! directory can only arbitrate that crate.
//!
//! It was measured: two `zerodds-durability-svc --domain 81` processes
//! from a second checkout's run stayed up for forty minutes, and the
//! `crash_recovery` test of a third run counted 159 samples on
//! `CrashTopic` where its own writer had sent 20.
//!
//! # What a lease guarantees
//!
//! 1. **One process per domain**, across binaries and across checkouts:
//!    the lease is a file, where `serial_test::serial` is a mutex inside
//!    one process and `file_serial` a lock without a resource attached.
//!    With [`LEASE_DIR_VARIABLE`] pointing at a directory every CI job of
//!    a runner host mounts, also across the jobs' containers: see
//!    `owner_is_alive` for how a lease of another PID namespace is judged.
//! 2. **Ports the kernel will not take**: see `ephemeral_port_range`.
//! 3. **No foreign vendor on the same number**: see [`RESERVED_DOMAINS`]
//!    and [`FOREIGN_DOMAINS`].
//! 4. **No other host of the LAN on the same number**: the multicast of a
//!    domain reaches every host of the LAN, so each host leases from a
//!    share of the band of its own; see [`BAND_VARIABLE`].
//!
//! ```ignore
//! let domain = zerodds_test_domains::unique_domain();
//! let participant = factory.create_participant(domain, qos)?;
//! ```
//!
//! Use [`lease_domain`] where one test walks a list of cases and wants a
//! fresh domain per case; the guard gives the domain back when it drops.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

// ---------------------------------------------------------------------
// Domain leasing
// ---------------------------------------------------------------------
//
// A DDS domain is a host-wide resource, not a per-process one: the
// domain number *is* the UDP port number (RTPS §9.6.1.4.1), and two test
// binaries that pick the same number take the same ports. `serial_test`
// cannot arbitrate that — its mutex lives inside one process, and cargo
// runs test binaries in parallel. So the domain is leased through the
// filesystem, and the band it is leased from is chosen so that neither
// the kernel nor another vendor's stack is already sitting on those
// ports.

/// RTPS §9.6.1.4.1 port formula, mirrored from `crates/dcps/src/runtime.rs`.
///
/// `PB` — the port base every RTPS implementation starts from.
const PORT_BASE: u32 = 7400;
/// `DG` — how far apart two neighbouring domains are.
const DOMAIN_GAIN: u32 = 250;
/// `d1` — the SPDP unicast offset inside a domain.
const OFFSET_SPDP_UNICAST: u32 = 10;
/// `PG` — how far apart two participants of one domain are.
const PARTICIPANT_GAIN: u32 = 2;
/// `DcpsRuntime::start` walks participant indices `0..120` looking for a
/// free well-known SPDP unicast port, so one domain reaches that far up.
const MAX_PARTICIPANT_INDEX: u32 = 119;

/// The highest domain whose port window still fits in a `u16`.
///
/// OMG DDS §2.2.1 permits `232`, but `232` puts the last SPDP unicast
/// port at `65648`: `DcpsRuntime::start` rejects it with
/// `BadParameter { what: "domain_id too large for SPDP port mapping" }`.
/// A control run on 240–245 reported six failures that were this and not
/// the tests.
pub const MAX_DOMAIN: i32 = 231;

/// Domains that are never leased out.
///
/// * `0`, `1`, `2` — the DDS and ROS 2 defaults. The bench host runs
///   four vendor stacks; two `opendds_pub` processes were measured
///   sitting on `239.255.0.1:7400`, which is domain 0.
/// * `42` — the repository's own examples and the cross-vendor scripts.
/// * the rest of `0..=100` but `90` and `92` — the numbers a program in
///   `examples/` or a documentation comment may name. No test binds a
///   literal domain (`crates/dcps/tests/domain_hygiene.rs`), every one
///   leases; the list stays as it is so that the shares of the band
///   ([`BAND_VARIABLE`]) and the copies of this list in the Java, Python
///   and TypeScript test helpers stay what they are.
///
/// All of them are below `101`, so their RTPS ports end under `32768`
/// and stay clear of the ephemeral range on both platforms. Leases come
/// from what is left over plus the band above the ephemeral range.
///
/// `crates/dcps/tests/domain_hygiene.rs` keeps this list honest: it
/// fails on a literal domain of an example or a documentation comment
/// that is not in here, and on any entry here whose ports reach into the
/// ephemeral range.
pub const RESERVED_DOMAINS: &[i32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 93, 94, 95, 96, 97, 98, 99,
    100,
];

/// Domains a process outside this workspace uses on the LAN, never leased.
///
/// * `226` — `nebula-sim` of `zero/nebula` on the host `llvm` (board
///   status, 2026-11-01).
/// * `230` — `zero/cuas` on the host `llvm` (board status, 2026-11-01).
///
/// SPDP multicast reaches every host of the LAN, so a test on one of these
/// numbers sees that process's participants. All of them lie above the
/// ephemeral range, in the band itself.
pub const FOREIGN_DOMAINS: &[i32] = &[226, 230];

/// The environment variable that names this host's share of the band.
///
/// SPDP multicast crosses the LAN in both directions between the CI runner
/// host and the Linux test host, measured on 2026-11-01
/// (a datagram to `239.255.0.1` arrived 10 of 10 and 20 of 20 times), and
/// the runner's jobs share the host's network (`network_mode = "host"`).
/// Leases arbitrate one host only; two hosts on one domain read each other's
/// samples. So the hosts split the band:
///
/// * `ci` — the CI runner host's jobs: the domains in
///   [`CI_SHARE_LOW`]`..=`[`CI_SHARE_HIGH`]; on a platform whose band does
///   not reach them (Windows and macOS, ephemeral range from 49152), the
///   whole band, because that platform has one CI host and no other test host.
/// * `<low>-<high>` — the domains of the band in that closed range.
/// * unset — every other host: the band without the CI share.
///
/// `ci/merge-jobs.yml`, `ci/interop-omg.yml` and the root `.gitlab-ci.yml`
/// set it to `ci`.
pub const BAND_VARIABLE: &str = "ZERODDS_TEST_DOMAIN_BAND";

/// The lowest domain of the CI runner host's share; see [`BAND_VARIABLE`].
pub const CI_SHARE_LOW: i32 = 219;

/// The highest domain of the CI runner host's share; see [`BAND_VARIABLE`].
pub const CI_SHARE_HIGH: i32 = 231;

/// The closed port window a participant on `domain` can occupy.
pub fn domain_port_window(domain: i32) -> (u32, u32) {
    let low = PORT_BASE + DOMAIN_GAIN * (domain as u32);
    (
        low,
        low + OFFSET_SPDP_UNICAST + PARTICIPANT_GAIN * MAX_PARTICIPANT_INDEX,
    )
}

/// The port range the kernel hands out to sockets that ask for "any port".
///
/// This is the range a test domain must stay out of. A socket the kernel
/// picked from it carries neither `SO_REUSEADDR` nor `SO_REUSEPORT`, and
/// a later explicit bind to that port therefore fails — measured on the
/// Linux 6.x bench host: a plain `bind(0.0.0.0, 0)` on port 51417 made
/// the SPDP recipe (`SO_REUSEADDR` + `SO_REUSEPORT` + `bind(0.0.0.0)`)
/// fail with `EADDRINUSE`. That is where
/// `TransportError { label: "spdp multicast bind" }` comes from: not
/// another DDS participant, but any process holding an ephemeral UDP
/// port — including the user-data sockets of ZeroDDS' own participants
/// in the other test binaries of the same run.
///
/// Read from the kernel rather than assumed, because the platforms
/// disagree: Linux hands out `32768..60999`, macOS and Windows
/// `49152..65535`, and the safe band is the complement of whichever one is
/// in force.
fn ephemeral_port_range() -> (u32, u32) {
    static RANGE: OnceLock<(u32, u32)> = OnceLock::new();
    *RANGE.get_or_init(|| {
        if let Ok(text) = std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range") {
            let mut fields = text
                .split_whitespace()
                .filter_map(|f| f.parse::<u32>().ok());
            if let (Some(low), Some(high)) = (fields.next(), fields.next())
                && low <= high
            {
                return (low, high);
            }
        }
        if let Ok(out) = std::process::Command::new("sysctl")
            .args([
                "-n",
                "net.inet.ip.portrange.first",
                "net.inet.ip.portrange.last",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut fields = text
                .split_whitespace()
                .filter_map(|f| f.parse::<u32>().ok());
            if let (Some(low), Some(high)) = (fields.next(), fields.next())
                && low <= high
            {
                return (low, high);
            }
        }
        // Windows has neither file nor sysctl; `netsh` names its dynamic
        // range. Without it the union below left the Windows CI host two
        // domains, 90 and 92, the share of the Linux hosts (job 228668).
        if cfg!(windows)
            && let Ok(out) = std::process::Command::new("netsh")
                .args(["int", "ipv4", "show", "dynamicport", "udp"])
                .output()
            && let Some(range) = dynamic_port_range(&String::from_utf8_lossy(&out.stdout))
        {
            return range;
        }
        // No kernel answered. Take the union of what the platforms are
        // known to use, so an unreadable host loses band width instead of
        // handing out a port the kernel may take.
        (32768, 65535)
    })
}

/// The range in the output of `netsh int ipv4 show dynamicport udp`: the
/// values of its first two `label : value` lines, the start port and the
/// number of ports. The labels are in the system's language, so they are
/// not read.
fn dynamic_port_range(text: &str) -> Option<(u32, u32)> {
    let mut values = text
        .lines()
        .filter_map(|line| line.rsplit_once(':'))
        .filter_map(|(_, value)| value.trim().parse::<u32>().ok());
    let start = values.next()?;
    let count = values.next()?;
    let last = start.checked_add(count)?.checked_sub(1)?;
    (count > 0 && last <= u32::from(u16::MAX)).then_some((start, last))
}

/// Every domain whose whole port window sits outside the ephemeral range.
///
/// Ordered from the top down, so the high band (above the ephemeral
/// range) is handed out first. That band is where no vendor default and
/// no ROS deployment lives — ROS 2 recommends `0..101` for exactly the
/// port reason above — so a test that lands there is also out of reach of
/// foreign traffic on the host, which no lock can achieve.
fn safe_domains() -> &'static [i32] {
    static BAND: OnceLock<Vec<i32>> = OnceLock::new();
    BAND.get_or_init(|| {
        let (busy_low, busy_high) = ephemeral_port_range();
        let band: Vec<i32> = (0..=MAX_DOMAIN)
            .rev()
            .filter(|domain| !RESERVED_DOMAINS.contains(domain))
            .filter(|domain| !FOREIGN_DOMAINS.contains(domain))
            .filter(|&domain| {
                let (low, high) = domain_port_window(domain);
                high <= u32::from(u16::MAX) && (high < busy_low || low > busy_high)
            })
            .collect();
        assert!(
            !band.is_empty(),
            "no DDS domain keeps its RTPS ports out of this host's ephemeral \
             port range {busy_low}..{busy_high}. Every test domain would bind \
             a port the kernel also hands out, so `spdp multicast bind` would \
             fail at random. Narrow the range \
             (`net.ipv4.ip_local_port_range`) before running the suite."
        );
        let named = std::env::var(BAND_VARIABLE).unwrap_or_default();
        let share = host_share(&band, named.trim());
        // zerodds-lint: allow no_panic_in_safe — a share without a domain
        // has no correct value to hand out.
        assert!(
            !share.is_empty(),
            "no DDS domain in this host's share ({BAND_VARIABLE}={named}) of \
             the band {band:?}"
        );
        share
    })
}

/// This host's share of `band`, by the value of [`BAND_VARIABLE`].
fn host_share(band: &[i32], named: &str) -> Vec<i32> {
    let ci: Vec<i32> = band
        .iter()
        .copied()
        .filter(|domain| (CI_SHARE_LOW..=CI_SHARE_HIGH).contains(domain))
        .collect();
    match named {
        "" => band.iter().copied().filter(|d| !ci.contains(d)).collect(),
        "ci" if ci.is_empty() => band.to_vec(),
        "ci" => ci,
        range => {
            let bounds: Option<(i32, i32)> = range.split_once('-').and_then(|(low, high)| {
                Some((low.trim().parse().ok()?, high.trim().parse().ok()?))
            });
            // zerodds-lint: allow no_panic_in_safe — a share that cannot be
            // read would hand out the band of another host.
            let (low, high) =
                bounds.unwrap_or_else(|| panic!("{BAND_VARIABLE}={range}: `ci` or `<low>-<high>`"));
            band.iter()
                .copied()
                .filter(|domain| (low..=high).contains(domain))
                .collect()
        }
    }
}

/// The environment variable that names the lease directory.
///
/// Set by the CI jobs of a runner host whose jobs run in containers of
/// their own but share the host's network (`network_mode = "host"`): the
/// directory is a volume every job of that host mounts, so the jobs
/// arbitrate the host's domains among themselves the way the test
/// processes of one job do. Without it the leases live in
/// `$TMPDIR/zerodds-dcps-domains`, which is host-wide on a machine
/// without containers and private to a container inside one.
pub const LEASE_DIR_VARIABLE: &str = "ZERODDS_TEST_DOMAIN_DIR";

/// Where the leases live: one file per domain, host-wide.
///
/// Host-wide and not under `target/`, because the resource is host-wide:
/// two checkouts running the suite at the same time take the same ports.
/// The same place `serial_test`'s `file_serial` keeps its locks, unless
/// [`LEASE_DIR_VARIABLE`] names another.
fn lease_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| match std::env::var_os(LEASE_DIR_VARIABLE) {
        Some(named) if !named.is_empty() => {
            let dir = PathBuf::from(named);
            // Not created here: a variable naming a directory that is not
            // there means the shared volume is not mounted, and a
            // directory created inside the container would arbitrate this
            // job alone while looking host-wide.
            // zerodds-lint: allow no_panic_in_safe — see above; a lease
            // from a private directory puts two jobs on one set of ports.
            assert!(
                dir.is_dir(),
                "{LEASE_DIR_VARIABLE}={} is not a directory: the lease \
                 directory the jobs of this host share is not mounted",
                dir.display()
            );
            dir
        }
        _ => {
            let dir = std::env::temp_dir().join("zerodds-dcps-domains");
            // zerodds-lint: allow no_panic_in_safe — a helper that cannot get a
            // domain has no correct value to return, and a returned default
            // puts two tests on one set of UDP ports.
            std::fs::create_dir_all(&dir).unwrap_or_else(|why| panic!("{}: {why}", dir.display()));
            dir
        }
    })
}

/// How often a holder refreshes the modification time of its lease files,
/// in milliseconds. The Java and TypeScript copies use the same value.
pub const HEARTBEAT_MILLIS: u64 = 2_000;

/// How old the modification time of a lease of another PID namespace may
/// be before its owner counts as gone, in milliseconds: ten missed
/// heartbeats. The Java and TypeScript copies use the same value.
pub const HEARTBEAT_STALE_MILLIS: u64 = 20_000;

const HEARTBEAT: Duration = Duration::from_millis(HEARTBEAT_MILLIS);
const HEARTBEAT_STALE: Duration = Duration::from_millis(HEARTBEAT_STALE_MILLIS);

/// The PID namespace of this process, `pid:[<inode>]`, where the kernel
/// tells it (`/proc/self/ns/pid`, Linux); `None` elsewhere.
///
/// Two CI jobs in containers of their own have different PID namespaces:
/// PID 57 in one names nothing, or another process, in the other. A lease
/// carries its owner's namespace so that a reader in another one does not
/// look its PID up in the wrong process table.
fn pid_namespace() -> Option<&'static str> {
    static NS: OnceLock<Option<String>> = OnceLock::new();
    NS.get_or_init(|| {
        std::fs::read_link("/proc/self/ns/pid")
            .ok()
            .map(|link| link.to_string_lossy().into_owned())
            .filter(|ns| !ns.is_empty() && !ns.contains(char::is_whitespace))
    })
    .as_deref()
}

/// Keeps the modification time of every lease this process holds fresh.
///
/// One thread per process, started with the first lease. A reader in
/// another PID namespace cannot ask the kernel whether the owner lives, so
/// it reads this heartbeat instead (`owner_is_alive`). The thread outlives
/// every lease and dies with the process; a process killed with `SIGKILL`
/// stops refreshing, and its leases go stale after [`HEARTBEAT_STALE`].
fn start_heartbeat() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("zerodds-domain-heartbeat".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(HEARTBEAT);
                    let held: Vec<i32> = LEASED_HERE
                        .lock()
                        .map(|mine| mine.iter().copied().collect())
                        .unwrap_or_default();
                    for domain in held {
                        touch(&lease_dir().join(format!("domain-{domain}.lease")));
                    }
                }
            });
        // zerodds-lint: allow no_panic_in_safe — without the heartbeat a
        // job in another container takes this process's domains after
        // twenty seconds.
        spawned.unwrap_or_else(|why| panic!("lease heartbeat thread: {why}"));
    });
}

/// Sets the modification time of an existing lease file to now. A file
/// given back meanwhile stays given back: no `create`.
fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}

/// Domains this process already holds, so it does not lease one twice.
///
/// The lease file alone cannot answer this: two threads of one process
/// both see a file they themselves created.
static LEASED_HERE: Mutex<BTreeSet<i32>> = Mutex::new(BTreeSet::new());

/// Take `domain` for this process, or report that someone else has it.
///
/// The claim is the existence of the file, created with `create_new` so
/// that two processes racing here cannot both win — the same shape
/// `crates/wire-probe/tests/three_column_gate.rs` uses for its own
/// exclusive claim. A lease whose owner is gone is taken over rather
/// than obeyed: a run killed with `SIGKILL` leaves its files behind, and
/// a band that shrank with every crash would be its own trap.
fn claim_domain(domain: i32) -> bool {
    claim_in(lease_dir(), domain)
}

/// [`claim_domain`] in `dir`.
fn claim_in(dir: &Path, domain: i32) -> bool {
    let path = dir.join(format!("domain-{domain}.lease"));
    match write_claim(&path) {
        Ok(()) => true,
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
            // Somebody holds it — or held it and is gone. Deciding that
            // means read, remove, create: three syscalls, so it has to
            // happen under the take-over lock or two processes both
            // conclude the dead owner's lease is theirs and the second
            // one's remove deletes the first one's fresh claim. That is
            // not a rare window: every run starts with the previous
            // run's leases stale and every binary scanning them at once.
            with_takeover_lock(dir, || {
                let held = std::fs::read_to_string(&path).unwrap_or_default();
                if owner_is_alive(held.trim(), &path) {
                    return false;
                }
                println!(
                    "domain {domain}: taking over a lease whose owner is gone ({})",
                    held.trim()
                );
                let _ = std::fs::remove_file(&path);
                write_claim(&path).is_ok()
            }) == Some(true)
        }
        Err(why) => panic!("{}: {why}", path.display()),
    }
}

/// Creates the lease file, or fails because it is already there.
///
/// `create_new` is the atomic half: one syscall, so two processes racing
/// here cannot both win — the same shape
/// `crates/wire-probe/tests/three_column_gate.rs` uses for its own
/// exclusive claim.
fn write_claim(path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    // The claim is the file. The line names the owner for the liveness
    // check of other processes, `pid <N> since <T>`, followed by
    // ` ns <namespace>` where the kernel names the PID namespace.
    let _ = match pid_namespace() {
        Some(ns) => writeln!(file, "pid {} since {stamp} ns {ns}", std::process::id()),
        None => writeln!(file, "pid {} since {stamp}", std::process::id()),
    };
    Ok(())
}

/// Runs `body` with every other take-over on this host held off.
///
/// `None` if the lock could not be had — the caller then leaves the
/// lease alone and looks at the next domain, which is the safe answer.
fn with_takeover_lock<T>(dir: &Path, body: impl FnOnce() -> T) -> Option<T> {
    let path = dir.join("takeover.lock");
    for _ in 0..400 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => {
                let out = body();
                let _ = std::fs::remove_file(&path);
                return Some(out);
            }
            Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
                // A holder killed mid-take-over would block every run
                // after it. The three syscalls under this lock take
                // microseconds, so anything older than 30 s is a corpse.
                let stale = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|when| when.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(30));
                if stale {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    }
    None
}

/// Whether the `pid N since T [ns NS]` line of the lease at `path` names
/// an owner that still exists.
///
/// A line this cannot parse counts as alive, because that is the safe
/// answer when the question cannot be settled: a wrong "it is gone"
/// hands a running test's ports to a second one, where a wrong "it is
/// alive" costs one domain out of the band.
///
/// An owner in another PID namespace (another container on the same
/// host, the lease directory shared through [`LEASE_DIR_VARIABLE`]) is
/// judged by its heartbeat: alive while the file's modification time is
/// younger than [`HEARTBEAT_STALE`]. Its PID would be looked up in the
/// wrong process table.
fn owner_is_alive(owner: &str, path: &Path) -> bool {
    owner_is_alive_from(owner, path, pid_namespace())
}

/// [`owner_is_alive`], read from the PID namespace `ours`.
fn owner_is_alive_from(owner: &str, path: &Path, ours: Option<&str>) -> bool {
    let Some(pid) = owner
        .split_whitespace()
        .nth(1)
        .and_then(|word| word.parse::<u32>().ok())
    else {
        return true;
    };
    let foreign = match (owner_namespace(owner), ours) {
        (Some(theirs), Some(ours)) => theirs != ours,
        _ => false,
    };
    if foreign {
        return heartbeat_is_fresh(path);
    }
    let proc_entry = PathBuf::from(format!("/proc/{pid}"));
    if proc_entry.exists() {
        return true;
    }
    if cfg!(target_os = "linux") {
        // /proc is authoritative there, and it said no.
        return false;
    }
    // `kill -0` without taking a `libc` dependency for one call.
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The `ns <namespace>` word of a lease line, if it has one.
fn owner_namespace(owner: &str) -> Option<&str> {
    let mut words = owner.split_whitespace();
    while let Some(word) = words.next() {
        if word == "ns" {
            return words.next();
        }
    }
    None
}

/// Whether the lease at `path` was refreshed within [`HEARTBEAT_STALE`].
///
/// A modification time that cannot be read counts as fresh, for the
/// reason `owner_is_alive` gives; a file that is gone counts as stale.
fn heartbeat_is_fresh(path: &Path) -> bool {
    match std::fs::metadata(path).and_then(|meta| meta.modified()) {
        // A modification time in the future (a clock step) is fresh.
        Ok(when) => !matches!(when.elapsed(), Ok(age) if age >= HEARTBEAT_STALE),
        Err(why) => why.kind() != std::io::ErrorKind::NotFound,
    }
}

/// A domain held for as long as this value lives.
///
/// Dropping it puts the domain back into the band. Use it where one test
/// walks a list of cases and wants a fresh domain per case — the
/// alternative, leasing per case and never releasing, empties the band
/// in a single test.
pub struct DomainLease {
    domain: i32,
}

impl DomainLease {
    /// The domain number to hand to `create_participant_with_config` or
    /// `DcpsRuntime::start`.
    #[must_use]
    pub fn id(&self) -> i32 {
        self.domain
    }
}

impl Drop for DomainLease {
    fn drop(&mut self) {
        if let Ok(mut mine) = LEASED_HERE.lock() {
            mine.remove(&self.domain);
        }
        let path = lease_dir().join(format!("domain-{}.lease", self.domain));
        // Best effort: a lease file left behind is taken over by the
        // next process that finds its owner gone.
        let _ = std::fs::remove_file(path);
    }
}

/// Leases from `safe_domains` and hands back a guard that releases it.
///
/// See [`unique_domain`] for why the domain has to be leased at all.
///
/// # Panics
/// When every safe domain is held by a live process.
#[must_use]
pub fn lease_domain() -> DomainLease {
    lease_domain_at_most(MAX_DOMAIN)
}

/// [`lease_domain`], from the safe domains up to `highest` only: for a
/// test whose peer takes fewer domains than the port mapping allows.
/// OpenSplice 6.9 refuses `231` ("config: root/Domain/Id/#text: 231: out
/// of range", measured) and takes `230`.
///
/// # Panics
/// When every safe domain up to `highest` is held by a live process, or
/// there is none.
#[must_use]
pub fn lease_domain_at_most(highest: i32) -> DomainLease {
    /// How long a test waits for a domain before it gives up. Long
    /// enough for the binaries cargo runs beside this one to finish and
    /// have their leases reclaimed, short enough that a genuinely stuck
    /// band is reported inside one test's lifetime rather than looking
    /// like a hang.
    const PATIENCE: Duration = Duration::from_secs(180);

    let began = std::time::Instant::now();
    let mut announced = false;
    loop {
        if let Some(lease) = try_lease(highest) {
            return lease;
        }
        if began.elapsed() >= PATIENCE {
            break;
        }
        if !announced {
            announced = true;
            // Exclusive means waiting sometimes, and a wait nobody can
            // see is indistinguishable from a hang.
            println!(
                "waiting for a DDS domain: all {} in the safe band are leased \
                 by a live process ({})",
                safe_domains().len(),
                lease_dir().display(),
            );
        }
        // Each pass also reclaims the leases of owners that have since
        // exited, so waiting makes progress on its own.
        std::thread::sleep(Duration::from_millis(500));
    }
    let (busy_low, busy_high) = ephemeral_port_range();
    // zerodds-lint: allow no_panic_in_safe — a helper that cannot get a
    // domain has no correct value to return, and a returned default
    // puts two tests on one set of UDP ports.
    panic!(
        "no DDS domain left after {}s. All {} domains that keep their RTPS \
         ports out of this host's ephemeral port range {busy_low}..{busy_high} \
         are leased by a process that is still alive. The leases are in {}, \
         one file per domain, each naming its owner. Nothing is removed while \
         an owner lives, so either another run of this suite is still going or \
         a lease names a PID that is still up.",
        PATIENCE.as_secs(),
        safe_domains().len(),
        lease_dir().display(),
    );
}

/// One pass over the band up to `highest`: the first domain nobody
/// holds, or nothing.
fn try_lease(highest: i32) -> Option<DomainLease> {
    let mut mine = LEASED_HERE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for &domain in safe_domains() {
        if domain > highest || mine.contains(&domain) {
            continue;
        }
        if claim_domain(domain) {
            mine.insert(domain);
            start_heartbeat();
            return Some(DomainLease { domain });
        }
    }
    None
}

/// Leases held until this process exits, for the callers of
/// [`unique_domain`].
static HELD_FOR_PROCESS: Mutex<Vec<DomainLease>> = Mutex::new(Vec::new());

/// A DDS domain no other process on this host is using.
///
/// Three properties, all of which the previous PID-hash version lacked:
///
/// 1. **Exclusive across binaries.** The lease is a file, so it holds
///    across the process boundary `serial_test::serial` cannot cross.
///    It matters because two participants on one domain share a port,
///    and a socket bound to `0.0.0.0:port` receives *every* multicast
///    group on that port — measured: a socket joined only to
///    `239.255.9.9` received a datagram sent to `239.255.7.7`. A unique
///    multicast group therefore does not separate two runs; only a
///    unique domain does.
/// 2. **Off the ephemeral port range.** See `ephemeral_port_range`.
/// 3. **Off the foreign stacks.** See `safe_domains` and
///    [`RESERVED_DOMAINS`].
///
/// The lease is held for the life of the process. Participants outlive
/// the test function that made them often enough (the factory singleton
/// keeps them until `ParticipantGuard` drops), and a lease released too
/// early is worse than a band that is one domain narrower.
///
/// Both participants of a test that expects them to meet must be given
/// the **same** value — call this once per test and reuse it, the way
/// `isolated_cfg` is cloned.
pub fn unique_domain() -> i32 {
    let lease = lease_domain();
    let domain = lease.id();
    HELD_FOR_PROCESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(lease);
    domain
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    /// `netsh` answers in the system's language and with CRLF; the range is
    /// read from the values, not the labels.
    #[test]
    fn the_windows_dynamic_port_range_is_read_from_netsh_in_any_language() {
        let english = "\r\nProtocol udp Dynamic Port Range\r\n\
                       ---------------------------------\r\n\
                       Start Port      : 49152\r\nNumber of Ports : 16384\r\n\r\n";
        let german = "\r\nProtokoll udp Dynamischer Portbereich\r\n\
                      ---------------------------------\r\n\
                      Startport       : 49152\r\nAnzahl der Ports: 16384\r\n";
        assert_eq!(dynamic_port_range(english), Some((49152, 65535)));
        assert_eq!(dynamic_port_range(german), Some((49152, 65535)));
        assert_eq!(
            dynamic_port_range("Start Port : 1025\nNumber of Ports : 64511\n"),
            Some((1025, 65535))
        );
        // Past the last port, no ports, no answer: not a range.
        assert_eq!(
            dynamic_port_range("Start Port : 60000\nNumber of Ports : 16384\n"),
            None
        );
        assert_eq!(
            dynamic_port_range("Start Port : 49152\nNumber of Ports : 0\n"),
            None
        );
        assert_eq!(dynamic_port_range(""), None);
    }

    /// A lease directory of this test alone, so that the claims below do
    /// not touch the band the other test binaries lease from.
    fn private_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zerodds-test-domains-unit-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn lease_file(dir: &Path, line: &str, age: Duration) -> PathBuf {
        let path = dir.join("domain-215.lease");
        std::fs::write(&path, line).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
        path
    }

    /// A PID no process has: Linux caps PIDs at 2^22, macOS at 99 999.
    const NO_SUCH_PID: u32 = 4_000_000_000;

    #[test]
    fn a_lease_of_another_pid_namespace_lives_while_its_heartbeat_is_fresh() {
        let dir = private_dir("fresh");
        let line = format!("pid {NO_SUCH_PID} since 0 ns pid:[1]");
        let path = lease_file(&dir, &line, Duration::from_secs(1));
        // The PID names nothing here, and it must not matter: it belongs
        // to another process table.
        assert!(owner_is_alive_from(&line, &path, Some("pid:[2]")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_lease_of_another_pid_namespace_without_heartbeat_is_taken_over() {
        let dir = private_dir("stale");
        let line = format!("pid {} since 0 ns pid:[1]", std::process::id());
        let path = lease_file(&dir, &line, HEARTBEAT_STALE + Duration::from_secs(5));
        // The PID names this very process here, and it must not matter.
        assert!(!owner_is_alive_from(&line, &path, Some("pid:[2]")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_lease_of_this_pid_namespace_is_judged_by_its_pid() {
        let dir = private_dir("same");
        let mine = format!("pid {} since 0 ns pid:[1]", std::process::id());
        let path = lease_file(&dir, &mine, HEARTBEAT_STALE * 3);
        assert!(owner_is_alive_from(&mine, &path, Some("pid:[1]")));
        let gone = format!("pid {NO_SUCH_PID} since 0 ns pid:[1]");
        let path = lease_file(&dir, &gone, Duration::ZERO);
        assert!(!owner_is_alive_from(&gone, &path, Some("pid:[1]")));
        // A line without namespace (an older writer) is judged by its PID.
        let old = format!("pid {NO_SUCH_PID} since 0");
        assert!(!owner_is_alive_from(&old, &path, Some("pid:[1]")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_lease_line_counts_as_alive() {
        let dir = private_dir("unreadable");
        let path = lease_file(&dir, "", HEARTBEAT_STALE * 3);
        assert!(owner_is_alive_from("", &path, Some("pid:[1]")));
        assert!(owner_is_alive_from(
            "pid ? since 0 ns pid:[9]",
            &path,
            Some("pid:[1]")
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The Linux band of a host with the ephemeral range 32768..60999.
    const LINUX_BAND: &[i32] = &[
        231, 229, 228, 227, 225, 224, 223, 222, 221, 220, 219, 218, 217, 216, 215, 92, 90,
    ];

    #[test]
    fn the_ci_host_and_every_other_host_take_disjoint_shares() {
        let ci = host_share(LINUX_BAND, "ci");
        let other = host_share(LINUX_BAND, "");
        assert_eq!(ci, [231, 229, 228, 227, 225, 224, 223, 222, 221, 220, 219]);
        assert_eq!(other, [218, 217, 216, 215, 92, 90]);
        assert!(ci.iter().all(|domain| !other.contains(domain)));
    }

    #[test]
    fn a_band_without_the_ci_share_is_the_ci_hosts_whole_band() {
        // Windows and macOS: ephemeral range from 49152, band below 167.
        let band = [166, 165, 164, 101];
        assert_eq!(host_share(&band, "ci"), band);
        assert_eq!(host_share(&band, ""), band);
    }

    #[test]
    fn a_named_range_is_cut_from_the_band() {
        assert_eq!(host_share(LINUX_BAND, "215-218"), [218, 217, 216, 215]);
        assert_eq!(host_share(LINUX_BAND, " 90 - 92 "), [92, 90]);
    }

    #[test]
    fn foreign_domains_are_not_in_the_band() {
        assert!(
            FOREIGN_DOMAINS
                .iter()
                .all(|domain| !safe_domains().contains(domain))
        );
    }

    #[test]
    fn a_claim_takes_over_a_dead_owner_and_leaves_a_live_one() {
        let dir = private_dir("claim");
        lease_file(&dir, &format!("pid {NO_SUCH_PID} since 0"), Duration::ZERO);
        assert!(claim_in(&dir, 215), "the owner is gone");
        let held = std::fs::read_to_string(dir.join("domain-215.lease")).unwrap();
        assert!(held.starts_with(&format!("pid {} since ", std::process::id())));
        assert_eq!(
            owner_namespace(&held),
            pid_namespace(),
            "the claim names this process's namespace where the kernel tells it"
        );
        assert!(!claim_in(&dir, 215), "this process holds it now");
        let _ = std::fs::remove_dir_all(dir);
    }
}
