// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-cli-common` — internal helpers for the ZeroDDS CLI tools.
//!
//! Crate `zerodds-cli-common`. Safety classification: **COMFORT**.
//! Pure tooling helpers (CLI frontend), no runtime paths.
//!
//! Collects the few pieces of boilerplate that the CLI tools
//! (`zerodds-admin`, `-bench`, `-monitor`, `-mq`, `-pcap`,
//! `-record`, `-snitch`, `-spy`) need: SIGINT/SIGTERM hook,
//! GUID-prefix generation, duration parsing with `s/m/h` suffixes.
//!
//! **Not a general-purpose library.** The crate is published on
//! crates.io (`publish = true`) only because the dependent CLI crates
//! need their dependencies to resolve there.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::GuidPrefix;

/// Generates a process-stable `GuidPrefix` in the layout every ZeroDDS
/// participant uses (`GuidPrefix::zerodds`): the vendor id in bytes 0..2
/// (DDSI-RTPS 2.5 §9.3.1.5), the host identifier
/// ([`zerodds_dcps::participant::host_id_bytes`]) in 2..6, the PID in
/// 6..10, the tool marker in byte 10 and the low byte of the sub-second
/// nanos in 11.
///
/// `marker` (e.g. `0xFE` for record, `0xFD` for bench) makes the prefixes
/// separable per tool. The host identifier is what makes a tool a
/// same-host peer of the participants it talks to
/// (`GuidPrefix::is_same_host`): with the PID there instead, every other
/// participant took a recorder for a remote host and sent it everything
/// over UDP.
#[must_use]
pub fn stable_prefix(marker: u8) -> GuidPrefix {
    let pid = std::process::id().to_le_bytes();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    GuidPrefix::zerodds(
        zerodds_dcps::participant::host_id_bytes(),
        [
            pid[0],
            pid[1],
            pid[2],
            pid[3],
            marker,
            nanos.to_le_bytes()[0],
        ],
    )
}

/// Computes the Participant GUID (16 bytes: 12 prefix + 4 EntityId
/// `00 00 00 C1` for `ENTITYID_PARTICIPANT`).
#[must_use]
pub fn participant_guid(prefix: GuidPrefix) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&prefix.0);
    g[12..15].copy_from_slice(&[0, 0, 0]);
    g[15] = 0xC1;
    g
}

/// Unix time in nanoseconds (i64; -1 on system clock failure).
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

/// Error while parsing a duration spec like `30s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationParseError {
    /// Input that was not parseable.
    pub input: String,
}

impl std::fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid duration spec: {}", self.input)
    }
}

impl std::error::Error for DurationParseError {}

/// Parses `5`, `5s`, `2m`, `1h` into a `Duration`.
///
/// # Errors
/// [`DurationParseError`] on non-numeric prefix or unknown unit.
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

/// Installs a SIGINT/SIGTERM handler that on receive sets the
/// `stop` flag to `true`. On Windows this is a no-op
/// (user stops with task-kill or `--duration`).
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
    // SAFETY: libc::signal takes a C-ABI function pointer; `handler`
    // is `extern "C"` and matches the libc signature.
    unsafe {
        libc::signal(libc::SIGINT, handler as usize);
        libc::signal(libc::SIGTERM, handler as usize);
    }
}

#[cfg(not(unix))]
fn install_inner(_stop: Arc<AtomicBool>) {}

/// Default `UserReaderConfig` for untyped/`zerodds::RawBytes` topics.
#[must_use]
pub fn raw_reader_config(topic: &str) -> UserReaderConfig {
    raw_reader_config_typed(topic, RAW_BYTES_TYPE_NAME)
}

/// The opaque "raw bytes" type name a monitoring reader announces when it has
/// no concrete type to follow.
pub const RAW_BYTES_TYPE_NAME: &str = "zerodds::RawBytes";

/// Like [`raw_reader_config`] but announces a caller-supplied `type_name`.
///
/// This is the building block for **type-following** monitoring tools
/// (`zerodds-spy`, `zerodds-record`): DDS matches reader↔writer by type-name
/// equality on BOTH ends, so a generic monitor that wants to see a typed topic
/// (e.g. `cuas::Track`) must announce that writer's *actual* `type_name`, not a
/// generic `RawBytes`. The payload is still consumed opaquely (the reader never
/// decodes it), so one config shape works for every followed type.
///
/// `type_identifier` stays `None` → the match falls back to pure `type_name`
/// comparison (DDS 1.4 §2.2.3 default path), which is exactly what we want: no
/// TypeObject is required to attach.
#[must_use]
pub fn raw_reader_config_typed(topic: &str, type_name: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        ..Default::default()
    }
}

/// The `UserWriterConfig` the bridging and measuring tools (`zerodds-bench`,
/// `zerodds-mq`) write their raw samples with: RELIABLE and VOLATILE, every
/// other policy at its default, no TypeObject announced.
#[must_use]
pub fn reliable_raw_writer_config(topic: &str, type_name: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        ..Default::default()
    }
}

/// Tracks which `(topic, type_name)` publications a monitoring tool has already
/// attached to, so it can attach a type-following raw reader to each *newly*
/// discovered writer type (including late joiners) exactly once.
///
/// The dedup core ([`TypeFollower::newly_seen`]) is pure — feed it the
/// discovered pairs and it returns only the ones not seen before — so it is
/// unit-testable without a live runtime. [`TypeFollower::poll`] is the
/// convenience that reads `DcpsRuntime::discovered_publication_topics()`.
#[derive(Debug, Default)]
pub struct TypeFollower {
    /// Topic allow-list; empty ⇒ follow every topic.
    topics: std::collections::HashSet<String>,
    /// `(topic, type_name)` pairs already reported, so each attaches once.
    seen: std::collections::HashSet<(String, String)>,
}

impl TypeFollower {
    /// Follow only the given topics. An empty iterator follows *all* topics.
    #[must_use]
    pub fn new<I, S>(topics: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            topics: topics.into_iter().map(Into::into).collect(),
            seen: std::collections::HashSet::new(),
        }
    }

    /// True if this follower accepts samples for `topic`.
    #[must_use]
    pub fn wants_topic(&self, topic: &str) -> bool {
        self.topics.is_empty() || self.topics.contains(topic)
    }

    /// Pure dedup core: given the currently discovered `(topic, type_name)`
    /// pairs, return those matching the topic filter that have not been seen
    /// before, marking them seen. Order-preserving; duplicates within the input
    /// collapse to one.
    pub fn newly_seen<I, A, B>(&mut self, discovered: I) -> Vec<(String, String)>
    where
        I: IntoIterator<Item = (A, B)>,
        A: Into<String>,
        B: Into<String>,
    {
        let mut fresh = Vec::new();
        for (t, ty) in discovered {
            let pair = (t.into(), ty.into());
            if !self.wants_topic(&pair.0) {
                continue;
            }
            if self.seen.insert(pair.clone()) {
                fresh.push(pair);
            }
        }
        fresh
    }

    /// Poll a runtime's discovered publications and return the newly-appeared
    /// `(topic, type_name)` pairs to attach a follower reader to.
    pub fn poll(&mut self, runtime: &zerodds_dcps::runtime::DcpsRuntime) -> Vec<(String, String)> {
        self.newly_seen(runtime.discovered_publication_topics())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn raw_reader_config_defaults_to_rawbytes_type() {
        let c = raw_reader_config("Track");
        assert_eq!(c.topic_name, "Track");
        assert_eq!(c.type_name, RAW_BYTES_TYPE_NAME);
    }

    #[test]
    fn raw_reader_config_typed_carries_the_supplied_type_name() {
        let c = raw_reader_config_typed("Track", "cuas::Track");
        assert_eq!(c.topic_name, "Track");
        assert_eq!(c.type_name, "cuas::Track");
        // Same opaque shape as the RawBytes config (reliable, volatile, no TID).
        assert!(c.reliable);
        assert!(matches!(
            c.type_identifier,
            zerodds_types::TypeIdentifier::None
        ));
    }

    #[test]
    fn type_follower_reports_each_pair_once() {
        let mut tf = TypeFollower::new(Vec::<String>::new()); // all topics
        let fresh = tf.newly_seen([("Track", "cuas::Track"), ("Track", "cuas::Track")]);
        assert_eq!(
            fresh,
            vec![("Track".to_string(), "cuas::Track".to_string())]
        );
        // Re-polling the same discovery yields nothing new.
        assert!(tf.newly_seen([("Track", "cuas::Track")]).is_empty());
    }

    #[test]
    fn type_follower_picks_up_late_joiner_types() {
        let mut tf = TypeFollower::new(Vec::<String>::new());
        assert_eq!(tf.newly_seen([("Track", "cuas::Track")]).len(), 1);
        // A second writer with a DIFFERENT type on the same topic is new.
        let fresh = tf.newly_seen([("Track", "cuas::Track"), ("Track", "cuas::TrackV2")]);
        assert_eq!(
            fresh,
            vec![("Track".to_string(), "cuas::TrackV2".to_string())]
        );
    }

    #[test]
    fn type_follower_filters_by_topic_allowlist() {
        let mut tf = TypeFollower::new(["Track"]);
        assert!(tf.wants_topic("Track"));
        assert!(!tf.wants_topic("Other"));
        let fresh = tf.newly_seen([("Track", "cuas::Track"), ("Other", "x::Y")]);
        assert_eq!(
            fresh,
            vec![("Track".to_string(), "cuas::Track".to_string())]
        );
    }

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
        assert_eq!(
            p.0[0..2],
            [0x01, 0xF0],
            "DDSI-RTPS 2.5 §9.3.1.5: the vendor id first"
        );
        assert_eq!(p.0[10], 0xAB);
    }

    /// A tool is a same-host peer of the participants on its host: its
    /// prefix carries the host identifier where `is_same_host` looks, and
    /// the PID where the runtime tells processes apart.
    #[test]
    fn stable_prefix_is_a_same_host_peer_of_this_hosts_participants() {
        let p = stable_prefix(0xFE);
        assert_eq!(p.host_id(), zerodds_dcps::participant::host_id_bytes());
        assert_eq!(p.0[6..10], std::process::id().to_le_bytes());
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
