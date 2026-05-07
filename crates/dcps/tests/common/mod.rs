//! Shared Test-Helper.
//!
//! Integration-Tests in diesem Verzeichnis binden via
//! `#[path = "common/mod.rs"] mod common;` in der jeweiligen
//! Test-Datei ein.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, Ordering};

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
