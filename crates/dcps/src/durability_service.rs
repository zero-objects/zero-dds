// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Durability-service storage backend (Spec §2.2.3.5 + §2.2.3.4
//! TRANSIENT/PERSISTENT path).
//!
//! In DDS the durability levels are characterized as follows (Spec
//! §2.2.3.4 Tab. 16):
//! * `VOLATILE`: no history for late joiners.
//! * `TRANSIENT_LOCAL`: writer history until disconnect.
//! * `TRANSIENT`: a separate storage service holds the history beyond
//!   the writer lifetime, but does NOT survive a service crash.
//! * `PERSISTENT`: like TRANSIENT, but disk-persistent.
//!
//! This module implements the backend abstraction for TRANSIENT +
//! PERSISTENT. The RTPS path feeds it in the writer data path
//! (`runtime.rs::handle_user_publish`) and consumes it on late-joiner
//! match (in addition to the writer's own TRANSIENT_LOCAL history).
//!
//! Architecture:
//!
//! 1. [`DurabilityBackend`] — trait with `store`, `replay_for_topic`,
//!    `cleanup_after_delay`.
//! 2. [`InMemoryDurabilityBackend`] — default for the TRANSIENT kind.
//! 3. [`OnDiskDurabilityBackend`] — for the PERSISTENT kind, persists
//!    on a directory hierarchy (one file per `(topic, instance,
//!    sequence)`).
//!
//! Both backends respect `DurabilityServiceQosPolicy`:
//! `service_cleanup_delay` (wait time after `unregister` before the
//! instance is removed), `history_kind`/`history_depth` (cap per
//! instance), `max_samples`/`max_instances`/`max_samples_per_instance`.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration as CoreDuration;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use zerodds_qos::DurabilityKind;
use zerodds_qos::policies::durability_service::DurabilityServiceQosPolicy;
use zerodds_qos::policies::history::HistoryKind;
use zerodds_qos::policies::resource_limits::LENGTH_UNLIMITED;

use crate::error::{DdsError, Result};

/// Stable sample slot in the durability history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurabilitySample {
    /// Topic name.
    pub topic: String,
    /// Instance KeyHash.
    pub instance_key: [u8; 16],
    /// Sequence number (writer-assigned, monotonic).
    pub sequence: u64,
    /// Encoded payload (XCDR2 body).
    pub payload: Vec<u8>,
    /// Creation timestamp (wall clock).
    pub created_at: SystemTime,
}

/// Trait for durability storage backends. Both implementations
/// (in-memory + on-disk) satisfy the same interface so the runtime
/// path can swap them.
pub trait DurabilityBackend: Send + Sync {
    /// Persists a sample.
    ///
    /// # Errors
    /// `OutOfResources` when the `max_samples` caps are exceeded;
    /// I/O error for on-disk backends.
    fn store(&self, sample: DurabilitySample) -> Result<()>;

    /// Returns all stored samples for a topic in order of their
    /// sequence number (sorted per instance).
    ///
    /// # Errors
    /// I/O error for on-disk backends.
    fn replay_for_topic(&self, topic: &str) -> Result<Vec<DurabilitySample>>;

    /// Marks an instance as `unregister` so that its associated
    /// samples are removed after `service_cleanup_delay`. `now` is the
    /// current wall clock; the actual deletion happens later via
    /// [`Self::cleanup`].
    ///
    /// # Errors
    /// I/O error for on-disk backends.
    fn unregister_instance(
        &self,
        topic: &str,
        instance_key: [u8; 16],
        now: SystemTime,
    ) -> Result<()>;

    /// Deletes all instances whose `unregister` timestamp + cleanup
    /// delay is before `now`. Returns the number of removed instances.
    ///
    /// # Errors
    /// I/O error for on-disk backends.
    fn cleanup(&self, now: SystemTime) -> Result<usize>;
}

/// Helper: service cleanup delay from QoS into `core::time::Duration`.
///
/// QoS Duration stores `fraction` as `2^-32 s`, not as nanos.
fn cleanup_delay(qos: &DurabilityServiceQosPolicy) -> CoreDuration {
    let secs = u64::try_from(qos.service_cleanup_delay.seconds.max(0)).unwrap_or(0);
    // fraction (2^-32 s) → nanoseconds
    let frac = u64::from(qos.service_cleanup_delay.fraction);
    let nanos = (frac.saturating_mul(1_000_000_000) >> 32) as u32;
    CoreDuration::new(secs, nanos)
}

/// Per-instance slot with ringbuffer-like history.
#[derive(Debug, Default, Clone)]
struct InstanceSlot {
    samples: Vec<DurabilitySample>,
    /// `unregister` timestamp; when `Some(t)`, the instance is removed
    /// from the backend after `t + service_cleanup_delay`.
    unregistered_at: Option<SystemTime>,
}

impl InstanceSlot {
    fn push(
        &mut self,
        s: DurabilitySample,
        history_kind: HistoryKind,
        history_depth: i32,
        max_samples_per_instance: i32,
    ) -> Result<()> {
        // History-kind cap.
        match history_kind {
            HistoryKind::KeepLast => {
                let depth_unsigned = if history_depth <= 0 {
                    1
                } else {
                    history_depth as usize
                };
                while self.samples.len() >= depth_unsigned {
                    self.samples.remove(0);
                }
            }
            HistoryKind::KeepAll => {
                if max_samples_per_instance != LENGTH_UNLIMITED
                    && self.samples.len() >= max_samples_per_instance as usize
                {
                    return Err(DdsError::OutOfResources {
                        what: "durability backend: max_samples_per_instance reached",
                    });
                }
            }
        }
        self.samples.push(s);
        Ok(())
    }
}

/// `(topic, instance_key)` lookup key.
type Key = (String, [u8; 16]);

#[derive(Debug, Default)]
struct InMemoryState {
    by_key: BTreeMap<Key, InstanceSlot>,
    total_samples: usize,
}

/// In-memory durability backend (default for DurabilityKind::Transient).
pub struct InMemoryDurabilityBackend {
    qos: DurabilityServiceQosPolicy,
    state: Mutex<InMemoryState>,
}

impl InMemoryDurabilityBackend {
    /// Constructor.
    #[must_use]
    pub fn new(qos: DurabilityServiceQosPolicy) -> Self {
        Self {
            qos,
            state: Mutex::new(InMemoryState::default()),
        }
    }

    /// Number of stored samples (diagnostics / tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.state.lock().map(|s| s.total_samples).unwrap_or(0)
    }

    /// True if no samples are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DurabilityBackend for InMemoryDurabilityBackend {
    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let mut g = self
            .state
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "in-memory durability backend poisoned",
            })?;
        // Cap on max_samples / max_instances.
        if self.qos.max_samples != LENGTH_UNLIMITED
            && g.total_samples >= self.qos.max_samples as usize
        {
            return Err(DdsError::OutOfResources {
                what: "durability backend: max_samples reached",
            });
        }
        let key = (sample.topic.clone(), sample.instance_key);
        let new_instance = !g.by_key.contains_key(&key);
        if new_instance
            && self.qos.max_instances != LENGTH_UNLIMITED
            && g.by_key.len() >= self.qos.max_instances as usize
        {
            return Err(DdsError::OutOfResources {
                what: "durability backend: max_instances reached",
            });
        }
        let slot = g.by_key.entry(key).or_default();
        let before = slot.samples.len();
        slot.push(
            sample,
            self.qos.history_kind,
            self.qos.history_depth,
            self.qos.max_samples_per_instance,
        )?;
        let delta = slot.samples.len() as isize - before as isize;
        g.total_samples = (g.total_samples as isize + delta).max(0) as usize;
        Ok(())
    }

    fn replay_for_topic(&self, topic: &str) -> Result<Vec<DurabilitySample>> {
        let g = self
            .state
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "in-memory durability backend poisoned",
            })?;
        let mut out = Vec::new();
        for ((t, _), slot) in g.by_key.iter() {
            if t == topic {
                out.extend(slot.samples.iter().cloned());
            }
        }
        out.sort_by_key(|s| (s.instance_key, s.sequence));
        Ok(out)
    }

    fn unregister_instance(
        &self,
        topic: &str,
        instance_key: [u8; 16],
        now: SystemTime,
    ) -> Result<()> {
        let mut g = self
            .state
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "in-memory durability backend poisoned",
            })?;
        if let Some(slot) = g.by_key.get_mut(&(topic.to_string(), instance_key)) {
            slot.unregistered_at = Some(now);
        }
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let delay = cleanup_delay(&self.qos);
        let mut g = self
            .state
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "in-memory durability backend poisoned",
            })?;
        let to_remove: Vec<Key> = g
            .by_key
            .iter()
            .filter_map(|(k, slot)| {
                slot.unregistered_at.and_then(|ts| {
                    let due = ts.checked_add(delay)?;
                    if now >= due { Some(k.clone()) } else { None }
                })
            })
            .collect();
        let removed = to_remove.len();
        for k in to_remove {
            if let Some(slot) = g.by_key.remove(&k) {
                g.total_samples = g.total_samples.saturating_sub(slot.samples.len());
            }
        }
        Ok(removed)
    }
}

// --------------------- On-Disk-Backend (PERSISTENT) ---------------------

/// On-disk durability backend (DurabilityKind::Persistent).
///
/// Layout: `<root>/<topic>/<hex(instance_key)>/<sequence>.bin` holds
/// the encoded payload; `<root>/<topic>/<hex(instance_key)>/.unregistered`
/// marks the instance cleanup timestamp (Unix nanos as ASCII).
pub struct OnDiskDurabilityBackend {
    qos: DurabilityServiceQosPolicy,
    root: PathBuf,
}

impl OnDiskDurabilityBackend {
    /// Constructor — creates the root path if it does not exist.
    ///
    /// # Errors
    /// Filesystem error while creating the root.
    pub fn new<P: Into<PathBuf>>(root: P, qos: DurabilityServiceQosPolicy) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| DdsError::PreconditionNotMet {
            reason: io_static_msg(&e, "durability backend: cannot create root"),
        })?;
        Ok(Self { qos, root })
    }

    fn instance_dir(&self, topic: &str, key: &[u8; 16]) -> PathBuf {
        let mut p = self.root.join(sanitize_topic(topic));
        p.push(hex16(key));
        p
    }

    fn unregister_marker(&self, topic: &str, key: &[u8; 16]) -> PathBuf {
        self.instance_dir(topic, key).join(".unregistered")
    }

    fn count_total_samples(&self) -> Result<usize> {
        let mut total = 0usize;
        let topics = match std::fs::read_dir(&self.root) {
            Ok(d) => d,
            Err(_) => return Ok(0),
        };
        for topic_dir in topics.flatten() {
            if !topic_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let instances = match std::fs::read_dir(topic_dir.path()) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for inst_dir in instances.flatten() {
                if !inst_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                if let Ok(samples) = std::fs::read_dir(inst_dir.path()) {
                    for s in samples.flatten() {
                        if s.file_name() != ".unregistered" {
                            total += 1;
                        }
                    }
                }
            }
        }
        Ok(total)
    }

    fn count_instances_for_topic(&self, topic: &str) -> usize {
        let topic_dir = self.root.join(sanitize_topic(topic));
        match std::fs::read_dir(&topic_dir) {
            Ok(d) => d
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .count(),
            Err(_) => 0,
        }
    }
}

fn io_static_msg(_e: &std::io::Error, msg: &'static str) -> &'static str {
    msg
}

fn sanitize_topic(topic: &str) -> String {
    // Restrict to [A-Za-z0-9_.-], replacing the rest with '_'.
    topic
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn hex16(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for x in b {
        let hi = (x >> 4) & 0xF;
        let lo = x & 0xF;
        s.push(core::char::from_digit(u32::from(hi), 16).unwrap_or('0'));
        s.push(core::char::from_digit(u32::from(lo), 16).unwrap_or('0'));
    }
    s
}

impl DurabilityBackend for OnDiskDurabilityBackend {
    fn store(&self, sample: DurabilitySample) -> Result<()> {
        // Pre-check caps.
        if self.qos.max_samples != LENGTH_UNLIMITED {
            let total = self.count_total_samples()?;
            if total >= self.qos.max_samples as usize {
                return Err(DdsError::OutOfResources {
                    what: "durability backend: max_samples reached",
                });
            }
        }
        let inst_dir = self.instance_dir(&sample.topic, &sample.instance_key);
        let new_instance = !inst_dir.exists();
        if new_instance && self.qos.max_instances != LENGTH_UNLIMITED {
            let count = self.count_instances_for_topic(&sample.topic);
            if count >= self.qos.max_instances as usize {
                return Err(DdsError::OutOfResources {
                    what: "durability backend: max_instances reached",
                });
            }
        }
        std::fs::create_dir_all(&inst_dir).map_err(|e| DdsError::PreconditionNotMet {
            reason: io_static_msg(&e, "durability backend: mkdir failed"),
        })?;
        // History-kind cap.
        match self.qos.history_kind {
            HistoryKind::KeepLast => {
                let depth = if self.qos.history_depth <= 0 {
                    1
                } else {
                    self.qos.history_depth as usize
                };
                let mut existing: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(&inst_dir)
                    .map_err(|e| DdsError::PreconditionNotMet {
                        reason: io_static_msg(&e, "durability backend: readdir failed"),
                    })?
                    .flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name == ".unregistered" {
                            return None;
                        }
                        let stem = name.strip_suffix(".bin")?;
                        let seq = stem.parse::<u64>().ok()?;
                        Some((seq, e.path()))
                    })
                    .collect();
                existing.sort_by_key(|(seq, _)| *seq);
                while existing.len() >= depth {
                    let (_, p) = existing.remove(0);
                    let _ = std::fs::remove_file(&p);
                }
            }
            HistoryKind::KeepAll => {
                if self.qos.max_samples_per_instance != LENGTH_UNLIMITED {
                    let count = std::fs::read_dir(&inst_dir)
                        .map_err(|e| DdsError::PreconditionNotMet {
                            reason: io_static_msg(&e, "durability backend: readdir failed"),
                        })?
                        .flatten()
                        .filter(|e| e.file_name() != ".unregistered")
                        .count();
                    if count >= self.qos.max_samples_per_instance as usize {
                        return Err(DdsError::OutOfResources {
                            what: "durability backend: max_samples_per_instance reached",
                        });
                    }
                }
            }
        }
        let path = inst_dir.join(alloc::format!("{}.bin", sample.sequence));
        std::fs::write(&path, &sample.payload).map_err(|e| DdsError::PreconditionNotMet {
            reason: io_static_msg(&e, "durability backend: write failed"),
        })?;
        Ok(())
    }

    fn replay_for_topic(&self, topic: &str) -> Result<Vec<DurabilitySample>> {
        let topic_dir = self.root.join(sanitize_topic(topic));
        let mut out = Vec::new();
        let dirs = match std::fs::read_dir(&topic_dir) {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };
        for inst_entry in dirs.flatten() {
            let inst_path = inst_entry.path();
            let key = match parse_hex16(&inst_entry.file_name().to_string_lossy()) {
                Some(k) => k,
                None => continue,
            };
            let samples = match std::fs::read_dir(&inst_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for entry in samples.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == ".unregistered" {
                    continue;
                }
                let Some(stem) = name.strip_suffix(".bin") else {
                    continue;
                };
                let Ok(seq) = stem.parse::<u64>() else {
                    continue;
                };
                let payload =
                    std::fs::read(entry.path()).map_err(|e| DdsError::PreconditionNotMet {
                        reason: io_static_msg(&e, "durability backend: read failed"),
                    })?;
                let created_at = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                out.push(DurabilitySample {
                    topic: topic.to_string(),
                    instance_key: key,
                    sequence: seq,
                    payload,
                    created_at,
                });
            }
        }
        out.sort_by_key(|s| (s.instance_key, s.sequence));
        Ok(out)
    }

    fn unregister_instance(
        &self,
        topic: &str,
        instance_key: [u8; 16],
        now: SystemTime,
    ) -> Result<()> {
        let dir = self.instance_dir(topic, &instance_key);
        if !dir.exists() {
            return Ok(());
        }
        let nanos = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let marker = self.unregister_marker(topic, &instance_key);
        std::fs::write(&marker, nanos.to_string().as_bytes()).map_err(|e| {
            DdsError::PreconditionNotMet {
                reason: io_static_msg(&e, "durability backend: marker write failed"),
            }
        })?;
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let delay = cleanup_delay(&self.qos);
        let mut removed = 0usize;
        let topics = match std::fs::read_dir(&self.root) {
            Ok(d) => d,
            Err(_) => return Ok(0),
        };
        for topic_dir in topics.flatten() {
            if !topic_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let instances = match std::fs::read_dir(topic_dir.path()) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for inst_dir in instances.flatten() {
                if !inst_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let marker = inst_dir.path().join(".unregistered");
                let Ok(content) = std::fs::read_to_string(&marker) else {
                    continue;
                };
                let Ok(nanos) = content.trim().parse::<u128>() else {
                    continue;
                };
                let unreg = SystemTime::UNIX_EPOCH + CoreDuration::from_nanos(nanos as u64);
                let due = unreg.checked_add(delay).unwrap_or(SystemTime::UNIX_EPOCH);
                if now >= due && std::fs::remove_dir_all(inst_dir.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

fn parse_hex16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = (chunk[0] as char).to_digit(16)?;
        let lo = (chunk[1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

/// Factory: creates the appropriate backend for the given durability
/// level. `root` is only needed for Persistent.
///
/// # Errors
/// Filesystem error during on-disk backend initialization;
/// `BadParameter` if `kind == Volatile/TransientLocal` (no durability
/// service needed) or `Persistent` without a `root`.
pub fn make_backend(
    kind: DurabilityKind,
    qos: DurabilityServiceQosPolicy,
    root: Option<PathBuf>,
) -> Result<alloc::boxed::Box<dyn DurabilityBackend>> {
    match kind {
        DurabilityKind::Volatile | DurabilityKind::TransientLocal => Err(DdsError::BadParameter {
            what: "durability backend: kind does not need a service",
        }),
        DurabilityKind::Transient => {
            Ok(alloc::boxed::Box::new(InMemoryDurabilityBackend::new(qos)))
        }
        DurabilityKind::Persistent => {
            let root = root.ok_or(DdsError::BadParameter {
                what: "durability backend: Persistent kind requires root path",
            })?;
            Ok(alloc::boxed::Box::new(OnDiskDurabilityBackend::new(
                root, qos,
            )?))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

    fn sample(topic: &str, key_byte: u8, seq: u64, payload: &[u8]) -> DurabilitySample {
        DurabilitySample {
            topic: topic.to_string(),
            instance_key: [key_byte; 16],
            sequence: seq,
            payload: payload.to_vec(),
            created_at: SystemTime::now(),
        }
    }

    fn keep_all_qos() -> DurabilityServiceQosPolicy {
        DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            ..DurabilityServiceQosPolicy::default()
        }
    }

    #[test]
    fn in_memory_store_and_replay_returns_sorted_samples() {
        let b = InMemoryDurabilityBackend::new(keep_all_qos());
        b.store(sample("T", 1, 2, b"b")).unwrap();
        b.store(sample("T", 1, 1, b"a")).unwrap();
        b.store(sample("T", 2, 1, b"c")).unwrap();
        let out = b.replay_for_topic("T").unwrap();
        assert_eq!(out.len(), 3);
        // sort by (instance, seq)
        assert_eq!(out[0].sequence, 1);
        assert_eq!(out[0].instance_key[0], 1);
        assert_eq!(out[1].sequence, 2);
        assert_eq!(out[1].instance_key[0], 1);
        assert_eq!(out[2].instance_key[0], 2);
    }

    #[test]
    fn in_memory_keeplast_caps_history_at_depth() {
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepLast,
            history_depth: 2,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = InMemoryDurabilityBackend::new(qos);
        for i in 1u64..=5 {
            b.store(sample("T", 1, i, &i.to_le_bytes())).unwrap();
        }
        let out = b.replay_for_topic("T").unwrap();
        // Depth=2 → only the last 2 sequences
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sequence, 4);
        assert_eq!(out[1].sequence, 5);
    }

    #[test]
    fn in_memory_keepall_max_samples_per_instance_returns_oor() {
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            max_samples_per_instance: 2,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = InMemoryDurabilityBackend::new(qos);
        b.store(sample("T", 1, 1, b"a")).unwrap();
        b.store(sample("T", 1, 2, b"b")).unwrap();
        let r = b.store(sample("T", 1, 3, b"c"));
        assert!(matches!(r, Err(DdsError::OutOfResources { .. })));
    }

    #[test]
    fn in_memory_max_samples_globally_returns_oor() {
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            max_samples: 2,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = InMemoryDurabilityBackend::new(qos);
        b.store(sample("T", 1, 1, b"a")).unwrap();
        b.store(sample("T", 2, 1, b"b")).unwrap();
        let r = b.store(sample("T", 3, 1, b"c"));
        assert!(matches!(r, Err(DdsError::OutOfResources { .. })));
    }

    #[test]
    fn in_memory_max_instances_returns_oor() {
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            max_instances: 1,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = InMemoryDurabilityBackend::new(qos);
        b.store(sample("T", 1, 1, b"a")).unwrap();
        let r = b.store(sample("T", 2, 1, b"b"));
        assert!(matches!(r, Err(DdsError::OutOfResources { .. })));
    }

    #[test]
    fn in_memory_unregister_then_cleanup_removes_after_delay() {
        let qos = DurabilityServiceQosPolicy {
            service_cleanup_delay: zerodds_qos::Duration::from_millis(100),
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = InMemoryDurabilityBackend::new(qos);
        let t0 = SystemTime::now();
        b.store(sample("T", 1, 1, b"a")).unwrap();
        b.unregister_instance("T", [1u8; 16], t0).unwrap();
        // Before the delay: cleanup does nothing.
        assert_eq!(b.cleanup(t0 + StdDuration::from_millis(50)).unwrap(), 0);
        assert_eq!(b.replay_for_topic("T").unwrap().len(), 1);
        // After the delay: cleanup removes.
        assert_eq!(b.cleanup(t0 + StdDuration::from_millis(150)).unwrap(), 1);
        assert!(b.replay_for_topic("T").unwrap().is_empty());
    }

    #[test]
    fn in_memory_replay_filters_by_topic() {
        let b = InMemoryDurabilityBackend::new(keep_all_qos());
        b.store(sample("A", 1, 1, b"a1")).unwrap();
        b.store(sample("B", 1, 1, b"b1")).unwrap();
        let a = b.replay_for_topic("A").unwrap();
        let bb = b.replay_for_topic("B").unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(bb.len(), 1);
        assert_eq!(a[0].topic, "A");
        assert_eq!(bb[0].topic, "B");
    }

    #[test]
    fn in_memory_unknown_topic_returns_empty() {
        let b = InMemoryDurabilityBackend::new(keep_all_qos());
        assert!(b.replay_for_topic("nope").unwrap().is_empty());
    }

    #[test]
    fn make_backend_rejects_volatile_and_transient_local() {
        let r1 = make_backend(
            DurabilityKind::Volatile,
            DurabilityServiceQosPolicy::default(),
            None,
        );
        let r2 = make_backend(
            DurabilityKind::TransientLocal,
            DurabilityServiceQosPolicy::default(),
            None,
        );
        assert!(matches!(r1, Err(DdsError::BadParameter { .. })));
        assert!(matches!(r2, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn make_backend_persistent_requires_root() {
        let r = make_backend(
            DurabilityKind::Persistent,
            DurabilityServiceQosPolicy::default(),
            None,
        );
        assert!(matches!(r, Err(DdsError::BadParameter { .. })));
    }

    #[test]
    fn make_backend_transient_returns_in_memory() {
        let b = make_backend(
            DurabilityKind::Transient,
            DurabilityServiceQosPolicy::default(),
            None,
        )
        .unwrap();
        b.store(sample("T", 1, 1, b"a")).unwrap();
        assert_eq!(b.replay_for_topic("T").unwrap().len(), 1);
    }

    fn tmp_dir(prefix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(alloc::format!("zerodds-dur-{prefix}-{nanos}"));
        p
    }

    #[test]
    fn on_disk_store_and_replay_roundtrip() {
        let root = tmp_dir("rt");
        let b = OnDiskDurabilityBackend::new(&root, keep_all_qos()).unwrap();
        b.store(sample("PersTopic", 7, 1, b"hello")).unwrap();
        b.store(sample("PersTopic", 7, 2, b"world")).unwrap();
        let out = b.replay_for_topic("PersTopic").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sequence, 1);
        assert_eq!(out[0].payload, b"hello");
        assert_eq!(out[1].sequence, 2);
        assert_eq!(out[1].payload, b"world");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn on_disk_keeplast_replaces_old_files() {
        let root = tmp_dir("kl");
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepLast,
            history_depth: 2,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = OnDiskDurabilityBackend::new(&root, qos).unwrap();
        for i in 1u64..=5 {
            b.store(sample("T", 1, i, &i.to_le_bytes())).unwrap();
        }
        let out = b.replay_for_topic("T").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].sequence, 4);
        assert_eq!(out[1].sequence, 5);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn on_disk_persistent_survives_backend_drop() {
        let root = tmp_dir("survive");
        {
            let b = OnDiskDurabilityBackend::new(&root, keep_all_qos()).unwrap();
            b.store(sample("Pers", 9, 42, b"alive")).unwrap();
        } // drop
        // A new backend with the same root path sees the sample.
        let b2 = OnDiskDurabilityBackend::new(&root, keep_all_qos()).unwrap();
        let out = b2.replay_for_topic("Pers").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].payload, b"alive");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn on_disk_unregister_and_cleanup_removes_directory() {
        let root = tmp_dir("cleanup");
        let qos = DurabilityServiceQosPolicy {
            service_cleanup_delay: zerodds_qos::Duration::from_millis(50),
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = OnDiskDurabilityBackend::new(&root, qos).unwrap();
        let t0 = SystemTime::now();
        b.store(sample("CT", 5, 1, b"v")).unwrap();
        b.unregister_instance("CT", [5u8; 16], t0).unwrap();
        assert_eq!(b.cleanup(t0 + StdDuration::from_millis(10)).unwrap(), 0);
        assert_eq!(b.cleanup(t0 + StdDuration::from_millis(100)).unwrap(), 1);
        assert!(b.replay_for_topic("CT").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn on_disk_max_samples_per_instance_returns_oor() {
        let root = tmp_dir("oor");
        let qos = DurabilityServiceQosPolicy {
            history_kind: HistoryKind::KeepAll,
            history_depth: -1,
            max_samples_per_instance: 2,
            ..DurabilityServiceQosPolicy::default()
        };
        let b = OnDiskDurabilityBackend::new(&root, qos).unwrap();
        b.store(sample("T", 1, 1, b"a")).unwrap();
        b.store(sample("T", 1, 2, b"b")).unwrap();
        let r = b.store(sample("T", 1, 3, b"c"));
        assert!(matches!(r, Err(DdsError::OutOfResources { .. })));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn hex16_roundtrip() {
        let key = [0xAB, 0xCD, 0xEF, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let h = hex16(&key);
        assert_eq!(h.len(), 32);
        assert_eq!(parse_hex16(&h).unwrap(), key);
    }

    #[test]
    fn parse_hex16_rejects_wrong_length_and_invalid_chars() {
        assert!(parse_hex16("abc").is_none());
        assert!(parse_hex16(&"x".repeat(32)).is_none());
    }

    #[test]
    fn sanitize_topic_replaces_path_chars() {
        assert_eq!(sanitize_topic("Topic/With:Path"), "Topic_With_Path");
        assert_eq!(sanitize_topic("ok-name.v1"), "ok-name.v1");
    }
}
