//! Shared test helpers.
//!
//! Integration tests in this directory include this via
//! `#[path = "common/mod.rs"] mod common;` in the respective
//! test file.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::net::Ipv4Addr;
use std::time::Duration;

use zerodds_dcps::runtime::RuntimeConfig;

/// Returns a `RuntimeConfig` with an SPDP multicast group that is
/// **unique per call**.
///
/// Background: by default all DCPS integration tests in the same binary
/// share the spec multicast group `239.255.0.1:7400`. When N tests run
/// in parallel, N participants compete for SPDP multicast reception —
/// packets get delivered to the "wrong" reader, causing match timeouts.
///
/// This function generates an admin-scoped multicast group (`239.X.Y.Z`,
/// RFC 2365 organization-local) from PID + call counter. Two
/// `isolated_cfg()` calls return **different** groups — anyone who wants
/// the same bus for multiple participants **must clone the config**.
///
/// ```ignore
/// let cfg = isolated_cfg();
/// let a = factory.create_participant_with_config(domain, qos, cfg.clone()).unwrap();
/// let b = factory.create_participant_with_config(domain, qos, cfg).unwrap();
/// // a and b find each other via multicast 239.X.Y.Z.
/// ```
///
/// Fast discovery period for tests (100 ms instead of the 5 s spec default).
pub fn isolated_cfg() -> RuntimeConfig {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let raw = pid.wrapping_mul(2654435761) ^ slot.wrapping_mul(0x9E3779B9);
    // 239.255.X.Y — same-subnet scope (RFC 2365 local scope, /16 suffix of
    // the spec default 239.255.0.1). GitHub Actions runners have restricted
    // multicast routing tables; only local-scope addresses (239.255.0.0/16)
    // are reliably reachable. Avoid 239.255.0.1 (spec default) →
    // y_lo starts at 2.
    let x = 1 + ((raw >> 8) & 0x7F) as u8; // [1, 128]
    let y = 2 + (raw & 0x7F) as u8; // [2, 129]
    let group = Ipv4Addr::new(239, 255, x, y);
    RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        spdp_multicast_group: group,
        ..RuntimeConfig::default()
    }
}

/// Returns a domain ID that is **unique within the process**.
///
/// Background: the CI has multiple runner instances (host-network,
/// Docker). When two parallel jobs run the same test binary
/// and both use a hard-coded domain ID, the UDP/multicast ports
/// on the host collide → SPDP packets from the "wrong" runner
/// arrive, and match timeouts hit. This function spreads by
/// PID + per-test slot so that two parallel runs see different
/// domains with high probability.
///
/// DDS domain IDs lie in `[0, 232]` (OMG DDS §2.2.1); we stay
/// in the range `[100, 229]` so that we hit neither production defaults
/// (0, 42) nor the ShapesDemo standard (0).
pub fn unique_domain(family: u8) -> i32 {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let raw = (pid.wrapping_mul(2654435761) ^ slot.wrapping_mul(0x9E3779B9)) as u64;
    // `family` reserves coarse classes (Lifespan=5, Deadline=6, ...)
    // so that test failures can be mapped more easily to the correct
    // path with `strace`. Range [100, 229] (DDS spec §2.2.1:
    // domain_id <= 232).
    let offset = i32::from(((raw >> 8) & 0x7F) as u8); // 0..127
    100 + (i32::from(family) % 13) * 10 + (offset % 10)
}

/// Discovery match timeout, adaptive for CI vs. a local dev run.
///
/// Background: `wait_for_matched_*(1, 5s)` is plenty on an unloaded
/// laptop; on a GitHub Actions runner (CI), under
/// `cargo llvm-cov` (instrumentation), or with parallel test
/// threads in the same process, 5s is too tight. Detection: env vars
/// LLVM_PROFILE_FILE (set by llvm-cov), CARGO_LLVM_COV
/// (sometimes), CI (GitHub/GitLab/etc.).
/// RAII wrapper around a DomainParticipant that is cleanly removed
/// from the factory singleton on drop.
///
/// Background: `DomainParticipantFactory::instance()` is, per
/// OMG DDS 1.4 §2.2.2.2, a singleton that holds all created
/// participants in a map (`lookup_participant` spec).
/// Tests that create + drop a participant let it accumulate in the
/// singleton — the Arc refcount stays > 0, runtime threads keep
/// running, and the next tests see 4+ participants in the
/// same process.
///
/// On going out of scope, this guard calls `factory.delete_participant`,
/// which releases the factory reference → Arc count drops → the
/// actual DomainParticipant drop runs → Runtime.shutdown.
pub struct ParticipantGuard {
    inner: Option<zerodds_dcps::DomainParticipant>,
}

impl ParticipantGuard {
    #[must_use]
    pub fn new(p: zerodds_dcps::DomainParticipant) -> Self {
        Self { inner: Some(p) }
    }
}

impl core::ops::Deref for ParticipantGuard {
    type Target = zerodds_dcps::DomainParticipant;
    fn deref(&self) -> &Self::Target {
        // We only release the inner `Some(...)` after drop; before that
        // it is always safe to unwrap.
        self.inner
            .as_ref()
            .expect("ParticipantGuard accessed after drop")
    }
}

impl Drop for ParticipantGuard {
    fn drop(&mut self) {
        if let Some(p) = self.inner.take() {
            let factory = zerodds_dcps::DomainParticipantFactory::instance();
            let _ = factory.delete_participant(&p);
            // p (DomainParticipant Arc) is dropped here; if the
            // factory reference was the last one, this now leads to
            // ParticipantInner drop → Runtime::shutdown.
        }
    }
}

pub fn match_timeout() -> std::time::Duration {
    let cov = std::env::var_os("LLVM_PROFILE_FILE").is_some()
        || std::env::var_os("CARGO_LLVM_COV").is_some();
    let ci = std::env::var_os("CI").is_some();
    // Coverage instrumentation is ~3-5x slower; plain CI runners
    // (cargo test without llvm-cov) are 1.5-2x slower than a dev laptop.
    if cov {
        std::time::Duration::from_secs(60)
    } else if ci {
        std::time::Duration::from_secs(20)
    } else {
        std::time::Duration::from_secs(5)
    }
}
