//! Shared Test-Helper.
//!
//! Integration-Tests in diesem Verzeichnis binden via
//! `#[path = "common/mod.rs"] mod common;` in der jeweiligen
//! Test-Datei ein.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::net::Ipv4Addr;
use std::time::Duration;

use zerodds_dcps::runtime::RuntimeConfig;

/// Liefert einen `RuntimeConfig` mit einer **pro Aufruf eindeutigen**
/// SPDP-Multicast-Gruppe.
///
/// Hintergrund: alle DCPS-Integrationstests im selben Binary teilen sich
/// per Default die Spec-Multicast-Group `239.255.0.1:7400`. Wenn N Tests
/// parallel laufen, konkurrieren N Participants um SPDP-Multicast-Empfang
/// — Pakete werden dem "falschen" Reader zugestellt, match-Timeouts.
///
/// Diese Funktion erzeugt eine Admin-scoped Multicast-Gruppe (`239.X.Y.Z`,
/// RFC 2365 organization-local) anhand PID + Aufruf-Counter. Zwei
/// `isolated_cfg()`-Aufrufe liefern **verschiedene** Gruppen — wer den
/// gleichen Bus für mehrere Participants will, **muss das Config cloned**.
///
/// ```ignore
/// let cfg = isolated_cfg();
/// let a = factory.create_participant_with_config(domain, qos, cfg.clone()).unwrap();
/// let b = factory.create_participant_with_config(domain, qos, cfg).unwrap();
/// // a und b finden sich gegenseitig via Multicast 239.X.Y.Z.
/// ```
///
/// Schnelle Discovery-Periode für Tests (100 ms statt 5 s Spec-Default).
pub fn isolated_cfg() -> RuntimeConfig {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let raw = pid.wrapping_mul(2654435761) ^ slot.wrapping_mul(0x9E3779B9);
    // 239.255.X.Y — same-subnet-scope (RFC 2365 Local-scope, /16-suffix der
    // Spec-Default 239.255.0.1). GitHub-Actions-runner haben eingeschränkte
    // Multicast-Routing-Tabellen; nur Local-Scope-Adressen (239.255.0.0/16)
    // sind verlässlich erreichbar. Vermeide 239.255.0.1 (Spec-Default) →
    // y_lo startet bei 2.
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

/// Liefert eine **im Prozess eindeutige** Domain-ID.
///
/// Hintergrund: Die CI hat mehrere Runner-Instanzen (host-network,
/// Docker). Wenn zwei parallele Jobs dasselbe Test-Binary laufen
/// und beide eine hart-codierte Domain-ID nutzen, kollidieren die
/// UDP-/Multicast-Ports auf dem Host → SPDP-Pakete vom "falschen"
/// Runner kommen an, match-Timeouts schlagen zu. Diese Funktion
/// streut per PID + pro-Test-Slot, damit zwei parallele Runs mit
/// hoher Wahrscheinlichkeit unterschiedliche Domains sehen.
///
/// DDS-Domain-IDs liegen in `[0, 232]` (OMG DDS §2.2.1); wir bleiben
/// im Bereich `[100, 229]`, damit wir weder Produktions-Defaults
/// (0, 42) noch den ShapesDemo-Standard (0) treffen.
pub fn unique_domain(family: u8) -> i32 {
    static SLOT: AtomicU32 = AtomicU32::new(0);
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let raw = (pid.wrapping_mul(2654435761) ^ slot.wrapping_mul(0x9E3779B9)) as u64;
    // `family` reserviert grobe Klassen (Lifespan=5, Deadline=6, ...)
    // damit Test-Failures einfacher mit `strace` auf den korrekten
    // Pfad gemappt werden koennen. Bereich [100, 229] (DDS-Spec §2.2.1:
    // domain_id <= 232).
    let offset = i32::from(((raw >> 8) & 0x7F) as u8); // 0..127
    100 + (i32::from(family) % 13) * 10 + (offset % 10)
}

/// Discovery-Match-Timeout, adaptiv fuer CI vs lokalen Dev-Lauf.
///
/// Hintergrund: `wait_for_matched_*(1, 5s)` ist auf einem ungeladenen
/// Laptop reichlich; auf GitHub-Actions-Runner (CI), unter
/// `cargo llvm-cov` (Instrumentation), oder bei parallelen Test-
/// Threads im selben Prozess sind 5s zu eng. Detection: env-vars
/// LLVM_PROFILE_FILE (von llvm-cov gesetzt), CARGO_LLVM_COV
/// (manchmal), CI (GitHub/GitLab/etc.).
/// RAII-Wrapper um einen DomainParticipant der bei Drop sauber
/// aus dem Factory-Singleton entfernt wird.
///
/// Hintergrund: `DomainParticipantFactory::instance()` ist per
/// OMG DDS 1.4 §2.2.2.2 ein Singleton, der alle erzeugten
/// Participants in einer Map haelt (`lookup_participant`-Spec).
/// Tests die einen Participant erzeugen + droppen lassen den im
/// Singleton akkumulieren — der Arc-Refcount bleibt > 0, Runtime-
/// Threads laufen weiter, naechste Tests sehen 4+ Participants im
/// selben Process.
///
/// Dieser Guard ruft beim Out-of-Scope `factory.delete_participant`
/// auf, was die Factory-Referenz freigibt → Arc-Count faellt → der
/// echte DomainParticipant-Drop laeuft → Runtime.shutdown.
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
        // Wir geben das innere `Some(...)` nur nach Drop frei, vorher
        // immer sicher unwrappable.
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
            // p (DomainParticipant Arc) wird hier gedroppt; wenn die
            // Factory-Referenz die letzte war, fuehrt das jetzt zu
            // ParticipantInner-Drop → Runtime::shutdown.
        }
    }
}

pub fn match_timeout() -> std::time::Duration {
    let cov = std::env::var_os("LLVM_PROFILE_FILE").is_some()
        || std::env::var_os("CARGO_LLVM_COV").is_some();
    let ci = std::env::var_os("CI").is_some();
    // Coverage-Instrumentation ist ~3-5x langsamer; reine CI-Runner
    // (cargo test ohne llvm-cov) sind 1.5-2x langsamer als Dev-Laptop.
    if cov {
        std::time::Duration::from_secs(60)
    } else if ci {
        std::time::Duration::from_secs(20)
    } else {
        std::time::Duration::from_secs(5)
    }
}
