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
//! 2. **Ports the kernel will not take**: see `ephemeral_port_range`.
//! 3. **No foreign vendor on the same number**: see [`RESERVED_DOMAINS`].
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

/// Domains that stay hard-coded and must never be leased out.
///
/// * `0`, `1`, `2` — the DDS and ROS 2 defaults. The bench host runs
///   four vendor stacks; two `opendds_pub` processes were measured
///   sitting on `239.255.0.1:7400`, which is domain 0.
/// * `42` — the repository's own examples and the cross-vendor scripts.
/// * `96`, `97`, `98` — the FastDDS live tests, where the domain is
///   agreed with a peer this process starts by command line.
/// * `61`, `62`, `91`, `93`, `95` — integration tests that stay on a
///   fixed domain because nothing else in the workspace uses theirs.
/// * `12`, `14`–`16`, `23`–`29`, `32`–`39`, `50`, `51`–`59`, `68`–`89`,
///   `94`, `99` — the `zerodds-c-api` tests, whose
///   `zerodds_dpf_create_participant` and `zerodds_runtime_create` both
///   go through the online factory and bind real sockets.
/// * the rest — domains hard-coded by the `DcpsRuntime::start` unit
///   tests in `crates/dcps/src/runtime.rs`, which bind real sockets in
///   the library test binary.
///
/// All of them are below `101`, so their RTPS ports end under `32768`
/// and stay clear of the ephemeral range on both platforms. Leases come
/// from what is left over plus the band above the ephemeral range.
///
/// `crates/dcps/tests/domain_hygiene.rs` keeps this list honest: it
/// fails on any literal domain in the workspace that is not in here, and
/// on any entry here whose ports reach into the ephemeral range.
pub const RESERVED_DOMAINS: &[i32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 93, 94, 95, 96, 97, 98, 99,
    100,
];

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
/// Read from the kernel rather than assumed, because the two platforms
/// disagree: Linux hands out `32768..60999`, macOS `49152..65535`, and
/// the safe band is the complement of whichever one is in force.
fn ephemeral_port_range() -> (u32, u32) {
    static RANGE: OnceLock<(u32, u32)> = OnceLock::new();
    *RANGE.get_or_init(|| {
        if let Ok(text) = std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range") {
            let mut fields = text
                .split_whitespace()
                .filter_map(|f| f.parse::<u32>().ok());
            if let (Some(low), Some(high)) = (fields.next(), fields.next()) {
                if low <= high {
                    return (low, high);
                }
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
            if let (Some(low), Some(high)) = (fields.next(), fields.next()) {
                if low <= high {
                    return (low, high);
                }
            }
        }
        // Neither kernel answered. Take the union of what both platforms
        // are known to use, so an unreadable host loses band width
        // instead of handing out a port the kernel may take.
        (32768, 65535)
    })
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
        band
    })
}

/// Where the leases live: one file per domain, host-wide.
///
/// Host-wide and not under `target/`, because the resource is host-wide:
/// two checkouts running the suite at the same time take the same ports.
/// The same place `serial_test`'s `file_serial` keeps its locks.
fn lease_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("zerodds-dcps-domains");
        // zerodds-lint: allow no_panic_in_safe — a helper that cannot get a
        // domain has no correct value to return, and a returned default
        // puts two tests on one set of UDP ports.
        std::fs::create_dir_all(&dir).unwrap_or_else(|why| panic!("{}: {why}", dir.display()));
        dir
    })
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
    let path = lease_dir().join(format!("domain-{domain}.lease"));
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
            with_takeover_lock(|| {
                let held = std::fs::read_to_string(&path).unwrap_or_default();
                if owner_is_alive(held.trim()) {
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
    // Best effort: the claim is the file, the line is for whoever has to
    // read the directory later.
    let _ = writeln!(file, "pid {} since {stamp}", std::process::id());
    Ok(())
}

/// Runs `body` with every other take-over on this host held off.
///
/// `None` if the lock could not be had — the caller then leaves the
/// lease alone and looks at the next domain, which is the safe answer.
fn with_takeover_lock<T>(body: impl FnOnce() -> T) -> Option<T> {
    let path = lease_dir().join("takeover.lock");
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

/// Whether the `pid N since T` line names a process that still exists.
///
/// A line this cannot parse counts as alive, because that is the safe
/// answer when the question cannot be settled: a wrong "it is gone"
/// hands a running test's ports to a second one, where a wrong "it is
/// alive" costs one domain out of the band.
fn owner_is_alive(owner: &str) -> bool {
    let Some(pid) = owner
        .split_whitespace()
        .nth(1)
        .and_then(|word| word.parse::<u32>().ok())
    else {
        return true;
    };
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
