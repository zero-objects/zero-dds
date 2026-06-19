// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! In-process discovery fastpath.
//!
//! Two `DomainParticipant`s in the same process and the same domain
//! discover each other through this process registry **deterministically
//! and synchronously** — instead of relying on lossy UDP multicast
//! SPDP/SEDP, whose reliable retransmit can stall under scheduler load
//! (discovery flake `volatile_writer_does_not_deliver_history_…`).
//!
//! The wire discovery path is left untouched: cross-process and
//! cross-vendor discovery still run over SPDP/SEDP. The fastpath is
//! purely **additive** — the SEDP cache inserts and the endpoint
//! matching are idempotent (GUID-keyed and `add_*_proxy`-idempotent,
//! respectively), so an announcement that later additionally arrives
//! over UDP is a no-op.

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::net::Ipv4Addr;
use std::sync::Mutex;

use crate::runtime::DcpsRuntime;

/// A registered in-process participant.
struct Entry {
    domain: i32,
    group: Ipv4Addr,
    rt: Weak<DcpsRuntime>,
}

/// Process-global registry of all live runtimes.
static REGISTRY: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// Lock-free hint for the number of registered runtimes. Updated on
/// `register`; the reader hot path reads it without a lock and skips
/// the `peers()` vec allocation when no second peer exists (the common
/// cross-process bench case).
static REGISTRY_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Registers a runtime. Call directly after the `Arc` wrap.
///
/// Dead entries (a `Weak` without a strong ref) are swept on every
/// registration — an explicit unregister on runtime drop is therefore
/// not needed.
pub(crate) fn register(rt: &Arc<DcpsRuntime>, domain: i32, group: Ipv4Addr) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.retain(|e| e.rt.strong_count() > 0);
        reg.push(Entry {
            domain,
            group,
            rt: Arc::downgrade(rt),
        });
        REGISTRY_COUNT.store(reg.len(), Ordering::Relaxed);
    }
}

/// Lock-free hint: number of registered runtimes (including self).
/// `<= 1` ⇒ the caller may skip the `peers()` lookup path. If `2+`,
/// the caller MUST call `peers()` because there may be a real collision
/// (e.g. multi-DomainParticipant in-process tests).
#[must_use]
pub(crate) fn registry_count_hint() -> usize {
    REGISTRY_COUNT.load(Ordering::Relaxed)
}

/// All live runtimes on `domain` + `group`. The result may contain the
/// caller itself — the caller filters by `GuidPrefix`.
#[must_use]
pub(crate) fn peers(domain: i32, group: Ipv4Addr) -> Vec<Arc<DcpsRuntime>> {
    let Ok(reg) = REGISTRY.lock() else {
        return Vec::new();
    };
    reg.iter()
        .filter(|e| e.domain == domain && e.group == group)
        .filter_map(|e| e.rt.upgrade())
        .collect()
}
