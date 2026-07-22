// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! File-per-sample cold adapter for the Durability-Service (ADR 0009).
//!
//! Crate `zerodds-durability-store-file`. Safety classification: **STANDARD**.
//!
//! Dependency-free `PERSISTENT` backend on a directory hierarchy:
//!
//! ```text
//! <root>/<hex(topic)>/<hex(instance_key)>/<sequence>.bin   # [8B created_at_nanos LE][payload]
//! <root>/<hex(topic)>/<hex(instance_key)>/.unregistered    # unregister wall-clock (unix nanos, ASCII)
//! ```
//!
//! The topic and instance components are hex-encoded so the mapping is
//! lossless and filesystem-safe (unlike a lossy sanitizer). The per-sample
//! header persists `created_at`, so the store survives a process restart with
//! full fidelity — that is exactly the `PERSISTENT` guarantee.
//!
//! Retention follows the topic [`Contract`] (set via
//! [`DurabilityStore::set_contract`]); contracts live in memory and are
//! re-registered by the daemon on startup.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zerodds_durability_store::{
    Contract, Cursor, DurabilitySample, DurabilityStore, Page, Result, Selector, StoreError,
    StoreStats,
};
use zerodds_qos::policies::history::HistoryKind;

// On-disk per-sample header: [8B created_at_nanos LE][1B representation]
// [1B big_endian] then the payload body.
// [8B created_at_nanos LE][1B representation][1B big_endian][16B source_guid]
// [8B source_sequence LE]. The source-identity tail (O2 P5) extended the header
// from 10 to 34 bytes; pre-P5 files are not read back (pre-1.0 format break).
const HEADER_LEN: usize = 34;
const DEFAULT_PAGE: usize = 1024;

/// File-per-sample durability store rooted at a directory.
pub struct FileStore {
    root: PathBuf,
    contracts: Mutex<BTreeMap<String, Contract>>,
    default_contract: Contract,
}

fn backend<E: core::fmt::Display>(ctx: &str, e: E) -> StoreError {
    StoreError::Backend(format!("file store: {ctx}: {e}"))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0xF), 16).unwrap_or('0'));
    }
    s
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

fn nanos_of(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn time_of(nanos: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_nanos(nanos)
}

impl FileStore {
    /// Opens (creating if absent) a file store at `root`, with `default_contract`
    /// for topics that have no explicit contract.
    ///
    /// # Errors
    /// Filesystem error creating the root directory.
    pub fn open<P: Into<PathBuf>>(root: P, default_contract: Contract) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| backend("create root", e))?;
        Ok(Self {
            root,
            contracts: Mutex::new(BTreeMap::new()),
            default_contract,
        })
    }

    fn contract_for(&self, topic: &str) -> Result<Contract> {
        Ok(self
            .contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("file store contracts"))?
            .get(topic)
            .copied()
            .unwrap_or(self.default_contract))
    }

    fn topic_dir(&self, topic: &str) -> PathBuf {
        self.root.join(hex(topic.as_bytes()))
    }

    fn instance_dir(&self, topic: &str, key: &[u8; 16]) -> PathBuf {
        self.topic_dir(topic).join(hex(key))
    }

    /// Lists `(sequence, path)` of sample files in an instance dir, sorted by
    /// sequence.
    fn list_samples(dir: &Path) -> Vec<(u64, PathBuf)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if let Some(stem) = name.strip_suffix(".bin") {
                    if let Ok(seq) = stem.parse::<u64>() {
                        out.push((seq, e.path()));
                    }
                }
            }
        }
        out.sort_by_key(|(s, _)| *s);
        out
    }

    fn list_instances(&self, topic: &str) -> Vec<[u8; 16]> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.topic_dir(topic)) {
            for e in rd.flatten() {
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(bytes) = unhex(&e.file_name().to_string_lossy()) {
                        if let Ok(k) = <[u8; 16]>::try_from(bytes.as_slice()) {
                            out.push(k);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    fn read_sample(topic: &str, key: [u8; 16], seq: u64, path: &Path) -> Option<DurabilitySample> {
        let raw = std::fs::read(path).ok()?;
        if raw.len() < HEADER_LEN {
            return None;
        }
        let nanos = u64::from_le_bytes(raw[..8].try_into().ok()?);
        let mut source_guid = [0u8; 16];
        source_guid.copy_from_slice(&raw[10..26]);
        let source_sequence = i64::from_le_bytes(raw[26..34].try_into().ok()?);
        Some(DurabilitySample {
            topic: topic.to_string(),
            instance_key: key,
            sequence: seq,
            payload: raw[HEADER_LEN..].to_vec(),
            representation: raw[8],
            big_endian: raw[9] != 0,
            created_at: time_of(nanos),
            source_guid,
            source_sequence,
        })
    }

    fn topic_sample_count(&self, topic: &str) -> usize {
        self.list_instances(topic)
            .iter()
            .map(|k| Self::list_samples(&self.instance_dir(topic, k)).len())
            .sum()
    }
}

impl DurabilityStore for FileStore {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.contracts
            .lock()
            .map_err(|_| StoreError::Poisoned("file store contracts"))?
            .insert(topic.to_string(), contract);
        Ok(())
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let contract = self.contract_for(&sample.topic)?;
        let inst_dir = self.instance_dir(&sample.topic, &sample.instance_key);
        let new_instance = !inst_dir.exists();

        // A re-send of an existing sequence overwrites its `<seq>.bin` — an
        // idempotent replace that grows nothing, so no cap may reject it.
        let is_resend = inst_dir.join(format!("{}.bin", sample.sequence)).exists();

        if !is_resend
            && contract.samples_bounded()
            && matches!(contract.history_kind, HistoryKind::KeepAll)
            && self.topic_sample_count(&sample.topic) >= contract.max_samples as usize
        {
            return Err(StoreError::OutOfResources("max_samples"));
        }
        if new_instance
            && contract.instances_bounded()
            && self.list_instances(&sample.topic).len() >= contract.max_instances as usize
        {
            return Err(StoreError::OutOfResources("max_instances"));
        }
        std::fs::create_dir_all(&inst_dir).map_err(|e| backend("mkdir instance", e))?;

        match contract.history_kind {
            HistoryKind::KeepLast => {
                let depth = contract.effective_depth();
                let existing = Self::list_samples(&inst_dir);
                // After adding one, keep only the newest `depth`.
                let mut to_remove = existing.len() as isize + 1 - depth as isize;
                for (_, p) in existing {
                    if to_remove <= 0 {
                        break;
                    }
                    let _ = std::fs::remove_file(&p);
                    to_remove -= 1;
                }
            }
            HistoryKind::KeepAll => {
                if !is_resend
                    && contract.per_instance_bounded()
                    && Self::list_samples(&inst_dir).len()
                        >= contract.max_samples_per_instance as usize
                {
                    return Err(StoreError::OutOfResources("max_samples_per_instance"));
                }
            }
        }

        let mut buf = Vec::with_capacity(HEADER_LEN + sample.payload.len());
        buf.extend_from_slice(&nanos_of(sample.created_at).to_le_bytes());
        buf.push(sample.representation);
        buf.push(u8::from(sample.big_endian));
        buf.extend_from_slice(&sample.source_guid);
        buf.extend_from_slice(&sample.source_sequence.to_le_bytes());
        buf.extend_from_slice(&sample.payload);
        let path = inst_dir.join(format!("{}.bin", sample.sequence));
        std::fs::write(&path, &buf).map_err(|e| backend("write sample", e))?;
        Ok(())
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        let mut matched: Vec<DurabilitySample> = Vec::new();
        let instances = match selector.instance_key {
            Some(k) => vec![k],
            None => self.list_instances(topic),
        };
        for key in instances {
            let dir = self.instance_dir(topic, &key);
            for (seq, path) in Self::list_samples(&dir) {
                if let Some(s) = Self::read_sample(topic, key, seq, &path) {
                    if selector.matches(&s) {
                        matched.push(s);
                    }
                }
            }
        }
        matched.sort_by_key(|s| (s.instance_key, s.sequence));
        let limit = selector.limit.unwrap_or(DEFAULT_PAGE);
        let exhausted = matched.len() <= limit;
        matched.truncate(limit);
        let next: Option<Cursor> = if exhausted {
            None
        } else {
            matched.last().map(|s| (s.instance_key, s.sequence))
        };
        Ok(Page {
            samples: matched,
            next,
        })
    }

    fn unregister(&self, topic: &str, instance_key: &[u8; 16], now: SystemTime) -> Result<()> {
        let dir = self.instance_dir(topic, instance_key);
        if dir.exists() {
            let marker = dir.join(".unregistered");
            std::fs::write(&marker, nanos_of(now).to_string())
                .map_err(|e| backend("write unregister marker", e))?;
        }
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let mut removed = 0usize;
        let topic_dirs = match std::fs::read_dir(&self.root) {
            Ok(d) => d,
            Err(_) => return Ok(0),
        };
        for topic_entry in topic_dirs.flatten() {
            let Some(topic_bytes) = unhex(&topic_entry.file_name().to_string_lossy()) else {
                continue;
            };
            let topic = String::from_utf8_lossy(&topic_bytes).into_owned();
            let delay = self.contract_for(&topic)?.cleanup_delay;
            let Ok(inst_dirs) = std::fs::read_dir(topic_entry.path()) else {
                continue;
            };
            for inst_entry in inst_dirs.flatten() {
                let marker = inst_entry.path().join(".unregistered");
                let Ok(content) = std::fs::read_to_string(&marker) else {
                    continue;
                };
                let Ok(nanos) = content.trim().parse::<u64>() else {
                    continue;
                };
                if let Some(deadline) = time_of(nanos).checked_add(delay) {
                    if now >= deadline {
                        let _ = std::fs::remove_dir_all(inst_entry.path());
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }

    fn stats(&self, topic: &str) -> Result<StoreStats> {
        let mut stats = StoreStats::default();
        for key in self.list_instances(topic) {
            stats.instances += 1;
            for (seq, path) in Self::list_samples(&self.instance_dir(topic, &key)) {
                if let Some(s) = Self::read_sample(topic, key, seq, &path) {
                    stats.samples += 1;
                    stats.bytes += s.payload.len() as u64;
                }
            }
        }
        Ok(stats)
    }
}
