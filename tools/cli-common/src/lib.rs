// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-cli-common` — interne Helfer für die ZeroDDS-CLI-Tools.
//!
//! Crate `zerodds-cli-common`. Safety classification: **COMFORT**.
//! Reine Tooling-Helfer (CLI-frontend), keine Runtime-Pfade.
//!
//! Sammelt die wenigen pieces of boilerplate die alle 7 Tools
//! (`zerodds-record`, `-bench`, `-monitor`, `-spy`, `-snitch`,
//! `-pcap`, `-mq`) brauchen: SIGINT/SIGTERM-Hook, GUID-Prefix-
//! Generation, Duration-Parsing mit `s/m/h`-Suffixen.
//!
//! **Nicht für externe Konsumenten** — keine Stabilitäts-Garantie,
//! keine Crates.io-Publikation.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zerodds_dcps::runtime::UserReaderConfig;
use zerodds_qos::{DeadlineQosPolicy, DurabilityKind, LivelinessQosPolicy, OwnershipKind};
use zerodds_rtps::wire_types::GuidPrefix;

/// Erzeugt einen prozess-stabilen `GuidPrefix` aus PID + nanos +
/// Tool-Marker-Byte.
///
/// `marker` (z.B. `0xFE` für record, `0xFD` für bench) landet im
/// vorletzten Byte und macht die Prefixe pro Tool trennbar.
#[must_use]
pub fn stable_prefix(marker: u8) -> GuidPrefix {
    let mut bytes = [0u8; 12];
    let pid = std::process::id();
    bytes[0..4].copy_from_slice(&pid.to_le_bytes());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    bytes[4..8].copy_from_slice(&nanos.to_le_bytes());
    bytes[8] = marker;
    GuidPrefix::from_bytes(bytes)
}

/// Berechnet die Participant-GUID (16 Byte: 12 prefix + 4 EntityId
/// `00 00 00 C1` für `ENTITYID_PARTICIPANT`).
#[must_use]
pub fn participant_guid(prefix: GuidPrefix) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&prefix.0);
    g[12..15].copy_from_slice(&[0, 0, 0]);
    g[15] = 0xC1;
    g
}

/// Unix-Zeit in Nanosekunden (i64; -1 bei System-Clock-Failure).
#[must_use]
pub fn unix_ns_now() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total = dur
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(dur.subsec_nanos()));
    i64::try_from(total).unwrap_or(i64::MAX)
}

/// Fehler beim Parsen einer Duration-Spec wie `30s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationParseError {
    /// Eingabe die nicht parse-bar war.
    pub input: String,
}

impl std::fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid duration spec: {}", self.input)
    }
}

impl std::error::Error for DurationParseError {}

/// Parst `5`, `5s`, `2m`, `1h` zu einer `Duration`.
///
/// # Errors
/// [`DurationParseError`] bei nicht-numerischem Prefix oder unbekannter Einheit.
pub fn parse_duration(s: &str) -> Result<Duration, DurationParseError> {
    let bad = || DurationParseError {
        input: s.to_string(),
    };
    let (num, unit) = s
        .find(|c: char| c.is_alphabetic())
        .map_or((s, "s"), |idx| (&s[..idx], &s[idx..]));
    let n: u64 = num.parse().map_err(|_| bad())?;
    let secs = match unit {
        "s" | "" => n,
        "m" => n.checked_mul(60).ok_or_else(bad)?,
        "h" => n.checked_mul(3600).ok_or_else(bad)?,
        _ => return Err(bad()),
    };
    Ok(Duration::from_secs(secs))
}

/// Installiert einen SIGINT/SIGTERM-Handler der bei Receive das
/// `stop`-Flag auf `true` setzt. Auf Windows ist das eine no-op
/// (User stoppt mit Task-Kill oder `--duration`).
pub fn install_signal_handler(stop: Arc<AtomicBool>) {
    install_inner(stop);
}

#[cfg(unix)]
fn install_inner(stop: Arc<AtomicBool>) {
    use std::sync::Mutex;
    static HOOK: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
    if let Ok(mut g) = HOOK.lock() {
        *g = Some(stop);
    }
    extern "C" fn handler(_: i32) {
        if let Ok(g) = HOOK.lock() {
            if let Some(s) = g.as_ref() {
                s.store(true, Ordering::Relaxed);
            }
        }
    }
    // SAFETY: libc::signal nimmt einen C-ABI-Funktionspointer; `handler`
    // ist `extern "C"` und passt auf die libc-Signatur.
    unsafe {
        libc::signal(libc::SIGINT, handler as usize);
        libc::signal(libc::SIGTERM, handler as usize);
    }
}

#[cfg(not(unix))]
fn install_inner(_stop: Arc<AtomicBool>) {}

/// Default `UserReaderConfig` für untyped/`zerodds::RawBytes`-Topics.
#[must_use]
pub fn raw_reader_config(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: "zerodds::RawBytes".to_string(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("5").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("3m").unwrap(), Duration::from_secs(180));
    }

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
    }

    #[test]
    fn parse_duration_rejects_garbage() {
        assert!(parse_duration("3x").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn stable_prefix_carries_marker() {
        let p = stable_prefix(0xAB);
        assert_eq!(p.0[8], 0xAB);
    }

    #[test]
    fn participant_guid_has_participant_eid() {
        let prefix = stable_prefix(0x42);
        let g = participant_guid(prefix);
        assert_eq!(&g[..12], &prefix.0[..]);
        assert_eq!(&g[12..], &[0, 0, 0, 0xC1]);
    }

    #[test]
    fn unix_ns_now_is_positive() {
        assert!(unix_ns_now() > 0);
    }
}
