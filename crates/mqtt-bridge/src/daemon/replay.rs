// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The daemon's replay buffer of the TRANSIENT and PERSISTENT topics —
//! Spec §6.
//!
//! For each `out` or `bidir` topic with the durability `transient` or
//! `persistent` the daemon keeps the PUBLISH of the newest sample of each
//! instance (an instance is a key hash; the samples without one are one
//! instance); a dispose or an unregister removes the instance. After each
//! CONNACK the buffer is published again, oldest first: a broker that
//! restarted without its retained messages, and the subscribers of a
//! daemon that restarted, get the topic's samples again. `transient` keeps
//! them in memory; `persistent` also in the file `<client id>.replay` of
//! `mqtt.session_dir` (the octets of the identifier outside
//! `A-Z a-z 0-9 . _ -` as `%XX`), which a restarted daemon reads back.
//!
//! A sample whose Message Expiry Interval has passed since it arrived is
//! not published again; the interval of one that is, is reduced by the time
//! since (MQTT 5.0 §3.3.2.3.3).
//!
//! The file: the header `ZDMQRPL` and a format version (1); then records of
//! a kind octet (`S` a sample, `F` an instance removed), the topic name
//! (2 octets of length, big endian, and its UTF-8), a key flag octet (1:
//! 16 octets of key hash follow), and for `S` the seconds since the Unix
//! epoch it arrived (8 octets), the encoded properties and the payload
//! (each 4 octets of length, big endian, and the octets). Each change is
//! appended; the file is rewritten with the live entries alone when it
//! holds four times as many records. The writes are not synced to the disk.
//! A record the disk takes only in part (a full disk) is cut off again, so
//! that the next one does not follow a broken record. A file that ends
//! inside its last record (the process ended while writing it) is read up
//! to that record, with the warning `replay_file_truncated`, and written
//! again; any other fault of the file prevents the start.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;

use super::log::{self, Level};
use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::vec::Vec;

use super::config::{DaemonConfig, TopicConfig};
use crate::packet::ControlPacketType;
use crate::property_set::{PropertyContext, PropertySet};

const MAGIC: &[u8; 7] = b"ZDMQRPL";
const FORMAT: u8 = 1;
const KIND_SAMPLE: u8 = b'S';
const KIND_FORGET: u8 = b'F';

/// The instance of a sample: its key hash, `None` for the samples without
/// one.
pub type Instance = Option<[u8; 16]>;

/// One kept PUBLISH.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    /// Seconds since the Unix epoch when it arrived.
    arrived: u64,
    /// The encoded properties of the PUBLISH.
    props: Vec<u8>,
    /// Its payload.
    message: Vec<u8>,
    /// The order of arrival.
    seq: u64,
}

/// A PUBLISH to send again: the DDS topic, the payload, the properties.
pub type Replayed = (String, Vec<u8>, PropertySet);

/// The replay file of `cfg`'s client identifier in `mqtt.session_dir`.
#[must_use]
pub fn replay_file(cfg: &DaemonConfig) -> PathBuf {
    Path::new(&cfg.session_dir).join(format!(
        "{}.replay",
        super::session::file_stem(&cfg.client_id)
    ))
}

/// The replay buffer of one connection.
#[derive(Debug, Default)]
pub struct ReplayBuffer {
    /// The DDS topics kept in memory or in the file.
    kept: BTreeSet<String>,
    /// The DDS topics also kept in the file.
    persistent: BTreeSet<String>,
    entries: BTreeMap<(String, Instance), Entry>,
    file: Option<PathBuf>,
    /// Records in the file.
    records: usize,
    next_seq: u64,
    /// A failed append left part of a record that could not be cut off
    /// (`session::append_whole`): the file is written again before the
    /// next record.
    torn: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl ReplayBuffer {
    /// The buffer of the TRANSIENT and PERSISTENT topics of `cfg` with a
    /// DataReader; the entries of its persistent topics read back from the
    /// file, those of a topic no longer persistent left out.
    ///
    /// A file that ends inside its last record gives the records before
    /// it, with a warning.
    ///
    /// # Errors
    /// The directory cannot be created, the file cannot be read, holds no
    /// replay records, or a record other than the last one cannot be read.
    pub fn open(cfg: &DaemonConfig) -> Result<Self, String> {
        let readers: Vec<&TopicConfig> = cfg
            .topics
            .iter()
            .filter(|t| matches!(t.direction.as_str(), "out" | "bidir"))
            .collect();
        let kept: BTreeSet<String> = readers
            .iter()
            .filter(|t| super::qos_translation::replayed(t))
            .map(|t| t.dds_name.clone())
            .collect();
        let persistent: BTreeSet<String> = readers
            .iter()
            .filter(|t| t.durability == "persistent")
            .map(|t| t.dds_name.clone())
            .collect();
        let mut me = Self {
            kept,
            persistent,
            ..Self::default()
        };
        if me.persistent.is_empty() || cfg.client_id.is_empty() {
            return Ok(me);
        }
        let path = replay_file(cfg);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        if path.exists() {
            let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let truncated = me
                .load(&bytes)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if truncated {
                log::event(Level::Warn, "replay_file_truncated")
                    .tenant(&cfg.tenant)
                    .field("path", &path.display().to_string())
                    .field("reason", "the last record is cut off and was not read")
                    .emit();
            }
        }
        me.file = Some(path);
        // Only the live entries of the topics persistent now stay.
        me.rewrite()?;
        Ok(me)
    }

    /// Reads the records of `bytes` into the buffer. `Ok(true)` when the
    /// file ends inside its last record (the process ended while writing
    /// it): the records before it are read, it is not.
    fn load(&mut self, bytes: &[u8]) -> Result<bool, String> {
        if bytes.len() < 8 || &bytes[..7] != MAGIC || bytes[7] != FORMAT {
            return Err("not a replay file of format 1".into());
        }
        let mut at = 8;
        let mut truncated = false;
        while at < bytes.len() {
            match self.load_record(bytes, &mut at) {
                Ok(()) => {}
                Err(None) => {
                    truncated = true;
                    break;
                }
                Err(Some(e)) => return Err(e),
            }
        }
        // The topics no longer persistent are not read back.
        let persistent = self.persistent.clone();
        self.entries
            .retain(|(name, _), _| persistent.contains(name));
        Ok(truncated)
    }

    /// The record of `bytes` at `at`, read into the buffer; `at` moves past
    /// it. `Err(None)` when the file ends inside it, `Err(Some)` for a
    /// record that cannot be read.
    fn load_record(&mut self, bytes: &[u8], at: &mut usize) -> Result<(), Option<String>> {
        let take = |at: &mut usize, n: usize| -> Result<&[u8], Option<String>> {
            let end = at
                .checked_add(n)
                .filter(|e| *e <= bytes.len())
                .ok_or(None::<String>)?;
            let out = &bytes[*at..end];
            *at = end;
            Ok(out)
        };
        let mut pos = *at;
        let kind = take(&mut pos, 1)?[0];
        let len = take(&mut pos, 2)?;
        let name_len = usize::from(u16::from_be_bytes([len[0], len[1]]));
        let name = String::from_utf8(take(&mut pos, name_len)?.to_vec())
            .map_err(|_| Some("a topic name that is no UTF-8".to_string()))?;
        let instance = match take(&mut pos, 1)?[0] {
            0 => None,
            1 => {
                let mut k = [0u8; 16];
                k.copy_from_slice(take(&mut pos, 16)?);
                Some(k)
            }
            other => return Err(Some(format!("key flag {other}"))),
        };
        match kind {
            KIND_SAMPLE => {
                let mut arrived = [0u8; 8];
                arrived.copy_from_slice(take(&mut pos, 8)?);
                let mut parts = [Vec::new(), Vec::new()];
                for part in &mut parts {
                    let l = take(&mut pos, 4)?;
                    let n = u32::from_be_bytes([l[0], l[1], l[2], l[3]]) as usize;
                    *part = take(&mut pos, n)?.to_vec();
                }
                let [props, message] = parts;
                let seq = self.next_seq;
                self.next_seq += 1;
                self.entries.insert(
                    (name, instance),
                    Entry {
                        arrived: u64::from_be_bytes(arrived),
                        props,
                        message,
                        seq,
                    },
                );
            }
            KIND_FORGET => {
                self.entries.remove(&(name, instance));
            }
            other => return Err(Some(format!("record kind {other}"))),
        }
        *at = pos;
        Ok(())
    }

    fn record(kind: u8, name: &str, instance: Instance, entry: Option<&Entry>) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(kind);
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&name_len.to_be_bytes());
        out.extend_from_slice(&name.as_bytes()[..usize::from(name_len)]);
        match instance {
            None => out.push(0),
            Some(k) => {
                out.push(1);
                out.extend_from_slice(&k);
            }
        }
        if let Some(e) = entry {
            out.extend_from_slice(&e.arrived.to_be_bytes());
            for part in [&e.props, &e.message] {
                let n = u32::try_from(part.len()).unwrap_or(u32::MAX);
                out.extend_from_slice(&n.to_be_bytes());
                out.extend_from_slice(part);
            }
        }
        out
    }

    /// Appends `bytes` to the file, whole or not at all
    /// (`session::append_whole`); rewrites it when it holds four times the
    /// live records, or when a failed append left part of a record in it.
    fn append(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(path) = self.file.clone() else {
            return Ok(());
        };
        if self.torn {
            // The change is in the entries already; the file written again
            // holds it.
            return self.rewrite();
        }
        let mut f = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if let Err(failed) = super::session::append_whole(&mut f, bytes) {
            self.torn = failed.torn;
            return Err(format!("{}: {}", path.display(), failed.error));
        }
        self.records += 1;
        if self.records > 4 * self.entries.len() + 64 {
            self.rewrite()?;
        }
        Ok(())
    }

    /// Writes the live entries of the persistent topics to a new file and
    /// puts it in place of the old one.
    fn rewrite(&mut self) -> Result<(), String> {
        let Some(path) = self.file.clone() else {
            return Ok(());
        };
        let mut bytes = MAGIC.to_vec();
        bytes.push(FORMAT);
        let mut n = 0;
        // In the order of arrival, which a file read back keeps.
        let mut live: Vec<(&(String, Instance), &Entry)> = self.entries.iter().collect();
        live.sort_by_key(|(_, e)| e.seq);
        for ((name, instance), e) in live {
            if self.persistent.contains(name) {
                bytes.extend(Self::record(KIND_SAMPLE, name, *instance, Some(e)));
                n += 1;
            }
        }
        let tmp = path.with_extension("replay.tmp");
        let mut f = File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        f.write_all(&bytes)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.records = n;
        self.torn = false;
        Ok(())
    }

    /// Whether the samples of `dds_name` are kept.
    #[must_use]
    pub fn keeps(&self, dds_name: &str) -> bool {
        self.kept.contains(dds_name)
    }

    /// The PUBLISH of the newest sample of `instance` of `dds_name`.
    ///
    /// # Errors
    /// The record cannot be written to the file.
    pub fn keep(
        &mut self,
        dds_name: &str,
        instance: Instance,
        message: &[u8],
        props: &PropertySet,
    ) -> Result<(), String> {
        if !self.keeps(dds_name) {
            return Ok(());
        }
        let entry = Entry {
            arrived: now_secs(),
            props: props.encode().map_err(|()| "the properties".to_string())?,
            message: message.to_vec(),
            seq: self.next_seq,
        };
        self.next_seq += 1;
        let record = Self::record(KIND_SAMPLE, dds_name, instance, Some(&entry));
        self.entries.insert((dds_name.to_string(), instance), entry);
        if self.persistent.contains(dds_name) {
            self.append(&record)?;
        }
        Ok(())
    }

    /// `instance` of `dds_name` was disposed or unregistered.
    ///
    /// # Errors
    /// The record cannot be written to the file.
    pub fn forget(&mut self, dds_name: &str, instance: Instance) -> Result<(), String> {
        if self
            .entries
            .remove(&(dds_name.to_string(), instance))
            .is_some()
            && self.persistent.contains(dds_name)
        {
            let record = Self::record(KIND_FORGET, dds_name, instance, None);
            self.append(&record)?;
        }
        Ok(())
    }

    /// The PUBLISH packets to send again, oldest first: the Message Expiry
    /// Interval reduced by the time since arrival, the expired ones left
    /// out.
    #[must_use]
    pub fn replay(&self) -> Vec<Replayed> {
        let now = now_secs();
        let mut all: Vec<(&(String, Instance), &Entry)> = self.entries.iter().collect();
        all.sort_by_key(|(_, e)| e.seq);
        let mut out = Vec::new();
        for ((name, _), e) in all {
            let Ok(mut props) = PropertySet::parse(
                &e.props,
                PropertyContext::Packet(ControlPacketType::Publish),
            ) else {
                continue;
            };
            if let Some(expiry) = props.message_expiry_interval {
                let elapsed = now.saturating_sub(e.arrived);
                if elapsed >= u64::from(expiry) {
                    continue;
                }
                props.message_expiry_interval =
                    Some(expiry - u32::try_from(elapsed).unwrap_or(expiry));
            }
            out.push((name.clone(), e.message.clone(), props));
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn cfg(dir: &Path, durability: &str) -> DaemonConfig {
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.client_id = "replay/1".into();
        cfg.session_dir = dir.display().to_string();
        cfg.topics.push(TopicConfig {
            dds_name: "R".into(),
            dds_type: "R".into(),
            mqtt_topic: "r".into(),
            direction: "out".into(),
            durability: durability.into(),
            ..TopicConfig::default()
        });
        cfg.topics.push(TopicConfig {
            dds_name: "V".into(),
            dds_type: "V".into(),
            mqtt_topic: "v".into(),
            direction: "out".into(),
            durability: "volatile".into(),
            ..TopicConfig::default()
        });
        cfg
    }

    fn scratch(leaf: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("zerodds-mqtt-replay-{}", std::process::id()))
            .join(leaf);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn props(expiry: Option<u32>) -> PropertySet {
        PropertySet {
            message_expiry_interval: expiry,
            content_type: Some("application/x-dds-cdr2".into()),
            ..PropertySet::default()
        }
    }

    /// Spec §6 TRANSIENT: the newest sample of each instance is kept in
    /// memory, oldest first; a volatile topic keeps nothing; a dispose
    /// removes the instance; nothing is written to a file.
    #[test]
    fn mqttbridge_6_transient_keeps_the_newest_sample_of_each_instance() {
        let dir = scratch("transient");
        let mut b = ReplayBuffer::open(&cfg(&dir, "transient")).unwrap();
        let (k1, k2) = (Some([1u8; 16]), Some([2u8; 16]));
        b.keep("R", k1, b"a1", &props(None)).unwrap();
        b.keep("R", k2, b"b1", &props(None)).unwrap();
        b.keep("R", k1, b"a2", &props(None)).unwrap();
        b.keep("V", None, b"v", &props(None)).unwrap();
        let got: Vec<Vec<u8>> = b.replay().into_iter().map(|(_, m, _)| m).collect();
        assert_eq!(got, [b"b1".to_vec(), b"a2".to_vec()]);
        b.forget("R", k2).unwrap();
        let got: Vec<Vec<u8>> = b.replay().into_iter().map(|(_, m, _)| m).collect();
        assert_eq!(got, [b"a2".to_vec()]);
        assert!(!replay_file(&cfg(&dir, "transient")).exists());
    }

    /// Spec §6 PERSISTENT: the buffer outlives the process in
    /// `<client id>.replay`; a buffer opened again reads the newest sample
    /// of each instance back, without the removed one; a topic that is no
    /// longer persistent is not read back.
    #[test]
    fn mqttbridge_6_persistent_samples_are_read_back_after_a_restart() {
        let dir = scratch("persistent");
        let c = cfg(&dir, "persistent");
        {
            let mut b = ReplayBuffer::open(&c).unwrap();
            for i in 0..200u8 {
                b.keep("R", Some([i % 3; 16]), &[i], &props(Some(600)))
                    .unwrap();
            }
            b.forget("R", Some([0; 16])).unwrap();
        }
        let file = replay_file(&c);
        assert_eq!(file.file_name().unwrap(), "replay%2F1.replay");
        let b = ReplayBuffer::open(&c).unwrap();
        let got: Vec<(String, Vec<u8>, Option<u32>)> = b
            .replay()
            .into_iter()
            .map(|(n, m, p)| (n, m, p.message_expiry_interval))
            .collect();
        assert_eq!(
            got,
            [
                ("R".to_string(), vec![197], Some(600)),
                ("R".to_string(), vec![199], Some(600)),
            ]
        );
        // The file was rewritten on the way: far fewer than 200 records.
        assert!(fs::metadata(&file).unwrap().len() < 4096);
        let transient = ReplayBuffer::open(&cfg(&dir, "transient")).unwrap();
        assert!(transient.replay().is_empty());
        fs::write(&file, b"garbage").unwrap();
        assert!(ReplayBuffer::open(&c).is_err());
    }

    /// Spec §6: a kept sample whose Message Expiry Interval has passed is
    /// not replayed; the interval of the others shrinks by the time since
    /// arrival.
    #[test]
    fn mqttbridge_6_an_expired_sample_is_not_replayed() {
        let mut b = ReplayBuffer::open(&cfg(&scratch("expiry"), "transient")).unwrap();
        b.keep("R", Some([1; 16]), b"old", &props(Some(5))).unwrap();
        b.keep("R", Some([2; 16]), b"new", &props(Some(100)))
            .unwrap();
        for e in b.entries.values_mut() {
            e.arrived -= 10;
        }
        let got: Vec<(Vec<u8>, Option<u32>)> = b
            .replay()
            .into_iter()
            .map(|(_, m, p)| (m, p.message_expiry_interval))
            .collect();
        assert_eq!(got, [(b"new".to_vec(), Some(90))]);
    }

    /// Spec §6: a file that ends inside its last record (the process ended
    /// while writing it) gives the records before it and is written again
    /// without the cut-off one; a record before the last one that cannot
    /// be read still prevents the start.
    #[test]
    fn mqttbridge_6_a_cut_off_last_record_leaves_the_records_before_it() {
        let dir = scratch("cut");
        let c = cfg(&dir, "persistent");
        {
            let mut b = ReplayBuffer::open(&c).unwrap();
            b.keep("R", Some([1; 16]), b"first", &props(Some(600)))
                .unwrap();
            b.keep("R", Some([2; 16]), b"second", &props(Some(600)))
                .unwrap();
        }
        let file = replay_file(&c);
        let whole = fs::read(&file).unwrap();
        let second = ReplayBuffer::record(
            KIND_SAMPLE,
            "R",
            Some([2; 16]),
            Some(&Entry {
                arrived: 0,
                props: props(Some(600)).encode().unwrap(),
                message: b"second".to_vec(),
                seq: 0,
            }),
        )
        .len();
        let before = whole[..whole.len() - second].to_vec();
        // Inside the payload, its length, the properties, after the kind.
        for cut in [1, 8, 30, second - 1] {
            fs::write(&file, &whole[..whole.len() - cut]).unwrap();
            let b = ReplayBuffer::open(&c).unwrap();
            let got: Vec<Vec<u8>> = b.replay().into_iter().map(|(_, m, _)| m).collect();
            assert_eq!(got, [b"first".to_vec()], "cut {cut}");
            drop(b);
            assert_eq!(fs::read(&file).unwrap(), before, "written again, cut {cut}");
        }
        let mut broken = whole;
        broken[8] = b'X';
        fs::write(&file, &broken).unwrap();
        let e = ReplayBuffer::open(&c).unwrap_err();
        assert!(e.contains("record kind"), "{e}");
    }

    /// Spec §6: when a failed append left part of a record that could not
    /// be cut off, the next change writes the file again from the entries
    /// instead of appending behind the broken record.
    #[test]
    fn mqttbridge_6_a_torn_replay_file_is_written_again_before_the_next_record() {
        let dir = scratch("torn");
        let c = cfg(&dir, "persistent");
        let mut b = ReplayBuffer::open(&c).unwrap();
        b.keep("R", Some([1; 16]), b"first", &props(None)).unwrap();
        let file = replay_file(&c);
        let mut f = OpenOptions::new().append(true).open(&file).unwrap();
        f.write_all(&[KIND_SAMPLE, 0, 1]).unwrap();
        b.torn = true;
        b.keep("R", Some([2; 16]), b"second", &props(None)).unwrap();
        assert!(!b.torn);
        drop(b);
        let b = ReplayBuffer::open(&c).unwrap();
        let got: Vec<Vec<u8>> = b.replay().into_iter().map(|(_, m, _)| m).collect();
        assert_eq!(got, [b"first".to_vec(), b"second".to_vec()]);
    }
}
