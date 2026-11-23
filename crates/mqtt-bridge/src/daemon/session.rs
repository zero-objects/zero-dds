// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The client side of an MQTT session — Spec §4.1, MQTT 5.0 §4.1.
//!
//! The Session State in the client is our QoS 1/2 PUBLISH packets the
//! broker has not completed, and the broker's QoS 2 PUBLISH packets we
//! have answered with PUBREC whose PUBREL has not come. A session outlives
//! its Network Connection under `clean_start: false` with a Session Expiry
//! Interval above 0 (MQTT 3.1.1: Clean Session 0); then the client MUST
//! store its state after the connection closed ([MQTT-3.1.2-23]), and on a
//! CONNACK with Session Present 1 it resends the PUBLISH packets with DUP
//! and the PUBREL packets that wait, with their identifiers, in their order
//! ([MQTT-4.4.0-1], [MQTT-4.6.0-1], [MQTT-4.6.0-4]).
//!
//! Such a session's state lives in a file of `mqtt.session_dir` per client
//! identifier, so that it outlives the process too: each change is appended
//! before the packet it concerns goes out, and the next start reads the
//! file back. The writes are not synced to the disk: a crash of the process
//! keeps them, a crash of the operating system can lose the last ones. A
//! record the disk takes only in part (a full disk) is cut off again, so
//! that the next one does not follow a broken record; a file that ends
//! inside its last record (the process ended while writing it) is read up
//! to that record. A lock file keeps a second process from the same state.
//!
//! The file: the header `ZDMQSES`, a format version (1) and the protocol
//! level of the packets (5 or 4); then records of a kind octet, the packet
//! identifier (2 octets, big endian), a length (4 octets, big endian) and
//! that many octets: `P` our PUBLISH packet as sent, `R` its PUBREC came
//! and the PUBREL went out, `C` the broker completed it (PUBACK, PUBCOMP,
//! or a refusal), `I` a QoS 2 PUBLISH of the broker answered with PUBREC,
//! `J` its PUBREL came.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::vec::Vec;

use super::config::DaemonConfig;
use crate::version::ProtocolVersion;

/// The first seven octets of a session file.
const MAGIC: &[u8; 7] = b"ZDMQSES";
/// The format version after the magic.
const FORMAT: u8 = 1;
/// Magic, format, protocol level.
const HEADER_LEN: usize = 9;
/// Records appended since the last rewrite before the file is rewritten
/// with the live entries alone (and at least eight per live entry).
const REWRITE_AFTER: usize = 4096;

const KIND_PUBLISHED: u8 = b'P';
const KIND_RELEASED: u8 = b'R';
const KIND_COMPLETED: u8 = b'C';
const KIND_RECEIVED: u8 = b'I';
const KIND_RECEIVED_RELEASED: u8 = b'J';

/// Where one of our QoS 1/2 PUBLISH packets stands (MQTT 5.0 §4.3.2,
/// §4.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutboundState {
    /// QoS 1 waits for PUBACK, QoS 2 for PUBREC.
    Published,
    /// QoS 2: PUBREL sent, waits for PUBCOMP.
    Released,
}

#[derive(Debug)]
struct Entry {
    /// The order of the entry's last change: a PUBLISH by when it was
    /// sent, a PUBREL by when its PUBREC came.
    seq: u64,
    state: OutboundState,
    /// The PUBLISH packet as sent, kept while it may have to be resent.
    frame: Option<Vec<u8>>,
}

/// What a session file held.
#[derive(Debug, Default)]
struct Replayed {
    outbound: BTreeMap<u16, Entry>,
    next_seq: u64,
    inbound: BTreeSet<u16>,
    /// The file ended inside a record (a crash while writing it).
    truncated: bool,
}

/// One packet that waits to be resent after a CONNACK with Session
/// Present 1, in its order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resend {
    /// A PUBLISH as it was sent; the DUP flag is set when it goes out.
    Publish(u16, Vec<u8>),
    /// A PUBREL.
    Release(u16),
}

/// The file of a lasting session and the lock on it.
#[derive(Debug)]
struct SessionFile {
    path: PathBuf,
    /// `None` until the session is the broker's ([`SessionState::settle`]).
    file: Option<File>,
    _lock: File,
    level: u8,
    /// Records appended since the last rewrite.
    appended: usize,
    /// A failed append left part of a record that could not be cut off
    /// ([`append_whole`]): the file is written again before the next
    /// record.
    torn: bool,
}

/// The Session State of one client.
#[derive(Debug, Default)]
pub(crate) struct SessionState {
    outbound: BTreeMap<u16, Entry>,
    next_seq: u64,
    inbound: BTreeSet<u16>,
    /// The file of a session that outlives its connection; `None` for one
    /// that ends with it.
    store: Option<SessionFile>,
    /// Whether the client has Session State of an earlier connection: its
    /// file existed at the start, or the session was settled since.
    known: bool,
}

/// The file name of `client_id`: the octets outside `A-Z a-z 0-9 . _ -`
/// as `%XX`.
pub(crate) fn file_stem(client_id: &str) -> String {
    let mut out = String::with_capacity(client_id.len());
    for b in client_id.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-') {
            out.push(char::from(b));
        } else {
            out.push_str(&std::format!("%{b:02X}"));
        }
    }
    out
}

/// The session file of `cfg`'s client identifier in `mqtt.session_dir`.
#[must_use]
pub fn session_file(cfg: &DaemonConfig) -> PathBuf {
    Path::new(&cfg.session_dir).join(std::format!("{}.session", file_stem(&cfg.client_id)))
}

/// Whether `cfg` asks for a session that outlives its connection: Clean
/// Start 0 with a Session Expiry Interval above 0 (MQTT 3.1.1: Clean
/// Session 0), under a client identifier of its own (an empty one gets a
/// new one from the broker at each CONNECT, MQTT 5.0 §3.1.3.1).
#[must_use]
pub fn lasting(cfg: &DaemonConfig) -> bool {
    !cfg.clean_start
        && !cfg.client_id.is_empty()
        && (cfg.protocol_version == ProtocolVersion::V311 || cfg.session_expiry_interval_secs > 0)
}

fn record(kind: u8, pid: u16, body: &[u8]) -> Result<Vec<u8>, String> {
    let len = u32::try_from(body.len()).map_err(|_| "a packet above 4 GiB".to_string())?;
    let mut out = Vec::with_capacity(7 + body.len());
    out.push(kind);
    out.extend_from_slice(&pid.to_be_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

/// A file records are appended to: a [`File`] opened for appending, a
/// double in the tests.
pub(crate) trait RecordFile: Write {
    /// Its length in octets.
    ///
    /// # Errors
    /// The length cannot be read.
    fn size(&self) -> std::io::Result<u64>;

    /// Cuts it to `size` octets.
    ///
    /// # Errors
    /// The file cannot be cut.
    fn cut(&mut self, size: u64) -> std::io::Result<()>;
}

impl RecordFile for File {
    fn size(&self) -> std::io::Result<u64> {
        Ok(self.metadata()?.len())
    }

    fn cut(&mut self, size: u64) -> std::io::Result<()> {
        self.set_len(size)
    }
}

/// A record that did not go into its file.
#[derive(Debug)]
pub(crate) struct AppendFailed {
    /// Why the write failed.
    pub(crate) error: std::io::Error,
    /// The part of the record that reached the file could not be cut off
    /// again: the file ends inside a record, and it is written again from
    /// the entries before the next record goes in.
    pub(crate) torn: bool,
}

/// Appends `record` to `file` whole or not at all: when the write fails (a
/// full disk), the part of the record that reached the file is cut off
/// again, so that the next record does not follow a broken one.
///
/// # Errors
/// The write failed; [`AppendFailed::torn`] when the cut failed too.
pub(crate) fn append_whole(file: &mut impl RecordFile, record: &[u8]) -> Result<(), AppendFailed> {
    let size = file
        .size()
        .map_err(|error| AppendFailed { error, torn: false })?;
    file.write_all(record).map_err(|error| AppendFailed {
        torn: file.cut(size).is_err(),
        error,
    })
}

fn header(level: u8) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[..7].copy_from_slice(MAGIC);
    h[7] = FORMAT;
    h[8] = level;
    h
}

/// The entries the records of `bytes` leave. `None` when the header is not
/// one of format 1 with protocol level `level`.
fn replay(bytes: &[u8], level: u8) -> Option<Replayed> {
    if bytes.len() < HEADER_LEN || bytes[..HEADER_LEN] != header(level) {
        return None;
    }
    let mut r = Replayed::default();
    let mut at = HEADER_LEN;
    while at < bytes.len() {
        let Some(head) = bytes.get(at..at + 7) else {
            r.truncated = true;
            break;
        };
        let kind = head[0];
        let pid = u16::from_be_bytes([head[1], head[2]]);
        let len = u32::from_be_bytes([head[3], head[4], head[5], head[6]]) as usize;
        let Some(body) = bytes.get(at + 7..at + 7 + len) else {
            r.truncated = true;
            break;
        };
        at += 7 + len;
        let seq = r.next_seq;
        match kind {
            KIND_PUBLISHED => {
                r.next_seq += 1;
                r.outbound.insert(
                    pid,
                    Entry {
                        seq,
                        state: OutboundState::Published,
                        frame: Some(body.to_vec()),
                    },
                );
            }
            KIND_RELEASED => {
                r.next_seq += 1;
                r.outbound.insert(
                    pid,
                    Entry {
                        seq,
                        state: OutboundState::Released,
                        frame: None,
                    },
                );
            }
            KIND_COMPLETED => {
                r.outbound.remove(&pid);
            }
            KIND_RECEIVED => {
                r.inbound.insert(pid);
            }
            KIND_RECEIVED_RELEASED => {
                r.inbound.remove(&pid);
            }
            _ => {
                r.truncated = true;
                break;
            }
        }
    }
    Some(r)
}

impl SessionState {
    /// The state of a session that ends with its connection.
    #[must_use]
    pub(crate) fn in_memory() -> Self {
        Self::default()
    }

    /// The state for the CONNECT of `cfg`. A lasting session ([`lasting`])
    /// takes the state its file holds; without a file, or with a file of
    /// another protocol level, the client has no Session State. With Clean
    /// Start 1 the file of an earlier session is removed: the session is
    /// discarded ([MQTT-3.1.2-4]).
    ///
    /// # Errors
    /// `mqtt.session_dir` cannot be created or written, another process
    /// holds the lock of the client identifier, the file cannot be read.
    pub(crate) fn for_connect(cfg: &DaemonConfig) -> Result<Self, String> {
        let path = session_file(cfg);
        if !lasting(cfg) {
            if cfg.clean_start && !cfg.client_id.is_empty() && path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| std::format!("session state {}: {e}", path.display()))?;
            }
            return Ok(Self::in_memory());
        }
        let dir = Path::new(&cfg.session_dir);
        fs::create_dir_all(dir)
            .map_err(|e| std::format!("mqtt.session_dir {}: {e}", dir.display()))?;
        let lock_path = path.with_extension("lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| std::format!("session lock {}: {e}", lock_path.display()))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(std::format!(
                    "another process holds the session state of client id `{}` ({})",
                    cfg.client_id,
                    lock_path.display()
                ));
            }
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(std::format!("session lock {}: {e}", lock_path.display()));
            }
        }
        let level = cfg.protocol_version.level();
        let mut me = Self {
            store: Some(SessionFile {
                path: path.clone(),
                file: None,
                _lock: lock,
                level,
                appended: 0,
                torn: false,
            }),
            ..Self::default()
        };
        let bytes = match fs::read(&path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(std::format!("session state {}: {e}", path.display())),
        };
        if let Some(bytes) = bytes {
            match replay(&bytes, level) {
                Some(r) => {
                    if r.truncated {
                        super::log::event(super::log::Level::Warn, "session_file_truncated")
                            .tenant(&cfg.tenant)
                            .field("path", &path.display().to_string())
                            .field("reason", "the last record is cut off and was not read")
                            .emit();
                    }
                    me.outbound = r.outbound;
                    me.next_seq = r.next_seq;
                    me.inbound = r.inbound;
                    me.known = true;
                    me.rewrite()?;
                }
                None => super::log::event(super::log::Level::Warn, "session_file_foreign")
                    .tenant(&cfg.tenant)
                    .field("path", &path.display().to_string())
                    .number("protocol_level", u64::from(level))
                    .field(
                        "reason",
                        "not a session file of this MQTT protocol level; the client has no session state",
                    )
                    .emit(),
            }
        }
        Ok(me)
    }

    /// Whether the client has Session State of an earlier connection
    /// (MQTT 5.0 §3.2.2.1.1).
    #[must_use]
    pub(crate) fn known(&self) -> bool {
        self.known
    }

    /// Whether this session outlives its connection.
    #[must_use]
    pub(crate) fn is_lasting(&self) -> bool {
        self.store.is_some()
    }

    /// After a CONNACK: the state is now the one of the session the broker
    /// holds. With Session Present 0 what it held is discarded
    /// ([MQTT-3.2.2-5]); the number of discarded entries is returned.
    ///
    /// # Errors
    /// The file cannot be written.
    pub(crate) fn settle(&mut self, session_present: bool) -> Result<usize, String> {
        let mut discarded = 0;
        if !session_present {
            discarded = self.outbound.len() + self.inbound.len();
            self.outbound.clear();
            self.inbound.clear();
        }
        if self.store.is_some() {
            self.known = true;
            let fresh = self.store.as_ref().is_some_and(|s| s.file.is_none());
            if fresh || discarded > 0 {
                self.rewrite()?;
            }
        }
        Ok(discarded)
    }

    /// Writes the file again with the live entries alone, in their order.
    fn rewrite(&mut self) -> Result<(), String> {
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        let mut bytes = header(store.level).to_vec();
        let mut order: Vec<(&u16, &Entry)> = self.outbound.iter().collect();
        order.sort_by_key(|(_, e)| e.seq);
        for (pid, e) in order {
            let rec = match (e.state, &e.frame) {
                (OutboundState::Published, Some(frame)) => record(KIND_PUBLISHED, *pid, frame)?,
                _ => record(KIND_RELEASED, *pid, &[])?,
            };
            bytes.extend_from_slice(&rec);
        }
        for pid in &self.inbound {
            bytes.extend_from_slice(&record(KIND_RECEIVED, *pid, &[])?);
        }
        let tmp = store.path.with_extension("session.tmp");
        let fail = |e: std::io::Error| std::format!("session state {}: {e}", store.path.display());
        fs::write(&tmp, &bytes).map_err(fail)?;
        fs::rename(&tmp, &store.path).map_err(fail)?;
        store.file = Some(
            OpenOptions::new()
                .append(true)
                .open(&store.path)
                .map_err(fail)?,
        );
        store.appended = 0;
        store.torn = false;
        Ok(())
    }

    /// Appends one record, whole or not at all ([`append_whole`]); rewrites
    /// the file when it holds many more records than live entries, or
    /// when a failed append left part of a record in it.
    fn append(&mut self, kind: u8, pid: u16, body: &[u8]) -> Result<(), String> {
        let live = self.outbound.len() + self.inbound.len();
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        if store.file.is_none() {
            return Ok(());
        }
        if store.torn {
            // The change is in the entries already; the file written again
            // holds it.
            return self.rewrite();
        }
        let rec = record(kind, pid, body)?;
        let Some(file) = store.file.as_mut() else {
            return Ok(());
        };
        if let Err(failed) = append_whole(file, &rec) {
            store.torn = failed.torn;
            return Err(std::format!(
                "session state {}: {}",
                store.path.display(),
                failed.error
            ));
        }
        store.appended += 1;
        if store.appended >= REWRITE_AFTER && store.appended >= 8 * live {
            self.rewrite()?;
        }
        Ok(())
    }

    /// Our PUBLISH `frame` with identifier `pid` is about to go out.
    ///
    /// # Errors
    /// The file cannot be written; the PUBLISH must not go out then.
    pub(crate) fn published(&mut self, pid: u16, frame: &[u8]) -> Result<(), String> {
        let seq = self.next_seq;
        self.next_seq += 1;
        let keep = self.store.is_some();
        self.outbound.insert(
            pid,
            Entry {
                seq,
                state: OutboundState::Published,
                frame: keep.then(|| frame.to_vec()),
            },
        );
        self.append(KIND_PUBLISHED, pid, frame)
    }

    /// The PUBREC of `pid` came; its PUBREL is about to go out.
    ///
    /// # Errors
    /// The file cannot be written.
    pub(crate) fn released(&mut self, pid: u16) -> Result<(), String> {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.outbound.insert(
            pid,
            Entry {
                seq,
                state: OutboundState::Released,
                frame: None,
            },
        );
        self.append(KIND_RELEASED, pid, &[])
    }

    /// The broker completed `pid` (PUBACK, PUBCOMP, or a refusal).
    ///
    /// # Errors
    /// The file cannot be written.
    pub(crate) fn completed(&mut self, pid: u16) -> Result<(), String> {
        if self.outbound.remove(&pid).is_none() {
            return Ok(());
        }
        self.append(KIND_COMPLETED, pid, &[])
    }

    /// A QoS 2 PUBLISH of the broker with `pid` is about to be answered
    /// with PUBREC. `Ok(true)` the first time, `Ok(false)` for a resend of
    /// one that was delivered ([MQTT-4.3.3-10]).
    ///
    /// # Errors
    /// The file cannot be written.
    pub(crate) fn received(&mut self, pid: u16) -> Result<bool, String> {
        if !self.inbound.insert(pid) {
            return Ok(false);
        }
        self.append(KIND_RECEIVED, pid, &[])?;
        Ok(true)
    }

    /// The broker's PUBREL of `pid` came; its PUBCOMP is about to go out.
    ///
    /// # Errors
    /// The file cannot be written.
    pub(crate) fn received_released(&mut self, pid: u16) -> Result<(), String> {
        if !self.inbound.remove(&pid) {
            return Ok(());
        }
        self.append(KIND_RECEIVED_RELEASED, pid, &[])
    }

    /// Our QoS 1/2 PUBLISH packets the broker has not completed.
    #[must_use]
    pub(crate) fn in_flight(&self) -> usize {
        self.outbound.len()
    }

    /// Whether `pid` is one of them.
    #[must_use]
    pub(crate) fn has(&self, pid: u16) -> bool {
        self.outbound.contains_key(&pid)
    }

    /// Where `pid` stands.
    #[must_use]
    pub(crate) fn state_of(&self, pid: u16) -> Option<OutboundState> {
        self.outbound.get(&pid).map(|e| e.state)
    }

    /// The PUBLISH packet of `pid` as it was sent, while it waits for its
    /// acknowledgement in a lasting session.
    #[must_use]
    pub(crate) fn frame_of(&self, pid: u16) -> Option<&[u8]> {
        self.outbound.get(&pid).and_then(|e| e.frame.as_deref())
    }

    /// Whether the broker's QoS 2 PUBLISH `pid` waits for its PUBREL.
    #[must_use]
    pub(crate) fn has_received(&self, pid: u16) -> bool {
        self.inbound.contains(&pid)
    }

    /// The broker's QoS 2 PUBLISH packets that wait for their PUBREL.
    #[must_use]
    pub(crate) fn received_len(&self) -> usize {
        self.inbound.len()
    }

    /// What waits to be resent after a CONNACK with Session Present 1, in
    /// the order of [MQTT-4.6.0-1] and [MQTT-4.6.0-4]. A PUBLISH whose
    /// packet was not kept (a session that ends with its connection) cannot
    /// be resent; it is not named.
    #[must_use]
    pub(crate) fn resend_order(&self) -> Vec<Resend> {
        let mut order: Vec<(&u16, &Entry)> = self.outbound.iter().collect();
        order.sort_by_key(|(_, e)| e.seq);
        order
            .into_iter()
            .filter_map(|(pid, e)| match (e.state, &e.frame) {
                (OutboundState::Published, Some(f)) => Some(Resend::Publish(*pid, f.clone())),
                (OutboundState::Published, None) => None,
                (OutboundState::Released, _) => Some(Resend::Release(*pid)),
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn cfg(dir: &Path, client_id: &str) -> DaemonConfig {
        let mut c = DaemonConfig::default_for_dev();
        c.client_id = client_id.to_string();
        c.clean_start = false;
        c.session_expiry_interval_secs = 300;
        c.session_dir = dir.display().to_string();
        c
    }

    fn scratch(leaf: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(std::format!("zerodds-mqtt-session-{}", std::process::id()))
            .join(leaf);
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Spec §4.1, [MQTT-3.1.2-23]: the state of a lasting session outlives
    /// the client: a QoS 1 PUBLISH not acknowledged, a QoS 2 one released,
    /// a completed one, a QoS 2 PUBLISH of the broker before its PUBREL.
    /// The next client reads them back and resends in the order of
    /// [MQTT-4.6.0-1] and [MQTT-4.6.0-4]: the PUBREL of 7 comes after the
    /// PUBLISH of 9, since its PUBREC came after 9 was sent.
    #[test]
    fn mqttbridge_4_1_the_session_state_outlives_the_client() {
        let dir = scratch("outlives");
        let c = cfg(&dir, "outlives/cli");
        {
            let mut s = SessionState::for_connect(&c).unwrap();
            assert!(!s.known(), "no file yet");
            s.settle(false).unwrap();
            s.published(7, b"seven").unwrap();
            s.published(8, b"eight").unwrap();
            s.published(9, b"nine").unwrap();
            s.completed(8).unwrap();
            s.released(7).unwrap();
            assert!(s.received(40).unwrap());
            assert!(!s.received(40).unwrap(), "a resend");
            assert!(s.received(41).unwrap());
            s.received_released(41).unwrap();
        }
        assert!(session_file(&c).ends_with("outlives%2Fcli.session"));
        let s = SessionState::for_connect(&c).unwrap();
        assert!(s.known());
        assert_eq!(
            s.resend_order(),
            [Resend::Publish(9, b"nine".to_vec()), Resend::Release(7)]
        );
        assert!(s.has_received(40) && !s.has_received(41));
        assert_eq!(s.in_flight(), 2);
    }

    /// [MQTT-3.2.2-5]: with Session Present 0 the client discards its state,
    /// also in the file; [MQTT-3.1.2-4]: Clean Start 1 removes the file.
    #[test]
    fn mqttbridge_4_1_session_present_0_and_clean_start_discard_the_state() {
        let dir = scratch("discard");
        let mut c = cfg(&dir, "discard-cli");
        {
            let mut s = SessionState::for_connect(&c).unwrap();
            s.settle(false).unwrap();
            s.published(1, b"one").unwrap();
            assert!(s.received(2).unwrap());
        }
        {
            let mut s = SessionState::for_connect(&c).unwrap();
            assert_eq!(s.in_flight(), 1);
            assert_eq!(s.settle(false).unwrap(), 2);
            assert!(s.resend_order().is_empty());
        }
        let s = SessionState::for_connect(&c).unwrap();
        assert!(s.known() && s.resend_order().is_empty() && s.received_len() == 0);
        drop(s);
        c.clean_start = true;
        let s = SessionState::for_connect(&c).unwrap();
        assert!(!s.is_lasting());
        assert!(!session_file(&c).exists(), "Clean Start 1 removes it");
    }

    /// A file cut off inside its last record keeps the records before; a
    /// file of the other protocol level is no Session State; a session of
    /// expiry 0 or with an empty client identifier keeps no file.
    #[test]
    fn mqttbridge_4_1_a_cut_off_or_foreign_session_file() {
        let dir = scratch("cut");
        let c = cfg(&dir, "cut-cli");
        {
            let mut s = SessionState::for_connect(&c).unwrap();
            s.settle(false).unwrap();
            s.published(3, b"three").unwrap();
            s.published(4, b"four").unwrap();
        }
        let path = session_file(&c);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 2]).unwrap();
        let s = SessionState::for_connect(&c).unwrap();
        assert_eq!(s.resend_order(), [Resend::Publish(3, b"three".to_vec())]);
        drop(s);
        let mut v311 = c.clone();
        v311.protocol_version = ProtocolVersion::V311;
        let s = SessionState::for_connect(&v311).unwrap();
        assert!(!s.known(), "packets of protocol level 5");
        drop(s);
        let mut zero = c.clone();
        zero.session_expiry_interval_secs = 0;
        assert!(!lasting(&zero));
        let mut empty = c.clone();
        empty.client_id = String::new();
        assert!(!lasting(&empty));
        assert!(SessionState::for_connect(&empty).unwrap().store.is_none());
        let mut clean_v311 = v311.clone();
        clean_v311.session_expiry_interval_secs = 0;
        assert!(lasting(&clean_v311), "MQTT 3.1.1: Clean Session 0 lasts");
    }

    /// A file that takes `room` octets more, then fails as a full disk
    /// does; its cut fails when `cut_fails`.
    struct Disk {
        data: Vec<u8>,
        room: usize,
        cut_fails: bool,
    }

    impl Write for Disk {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if self.room == 0 {
                return Err(std::io::Error::other("no space left on device"));
            }
            let n = b.len().min(self.room);
            self.data.extend_from_slice(&b[..n]);
            self.room -= n;
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl RecordFile for Disk {
        fn size(&self) -> std::io::Result<u64> {
            Ok(self.data.len() as u64)
        }

        fn cut(&mut self, size: u64) -> std::io::Result<()> {
            if self.cut_fails {
                return Err(std::io::Error::other("cut refused"));
            }
            self.data.truncate(usize::try_from(size).unwrap());
            Ok(())
        }
    }

    /// Spec §4.1: a record the disk takes only in part (a full disk) is
    /// cut off again, so that the next record follows a whole one and the
    /// file reads back without a fault; when the cut fails too, the append
    /// says so, and the next change writes the file again from the
    /// entries instead of appending behind the broken record.
    #[test]
    fn mqttbridge_4_1_a_record_the_disk_takes_in_part_is_cut_off_again() {
        let one = record(KIND_PUBLISHED, 1, b"one").unwrap();
        let two = record(KIND_PUBLISHED, 2, b"two").unwrap();
        let done = record(KIND_COMPLETED, 1, &[]).unwrap();
        let mut disk = Disk {
            data: header(5).to_vec(),
            room: one.len() + 4,
            cut_fails: false,
        };
        append_whole(&mut disk, &one).unwrap();
        let failed = append_whole(&mut disk, &two).unwrap_err();
        assert!(!failed.torn);
        assert_eq!(disk.data.len(), HEADER_LEN + one.len(), "two cut off");
        disk.room = usize::MAX;
        append_whole(&mut disk, &done).unwrap();
        let r = replay(&disk.data, 5).unwrap();
        assert!(!r.truncated, "every record whole");
        assert!(r.outbound.is_empty(), "one completed, two never written");
        disk.room = 3;
        disk.cut_fails = true;
        assert!(append_whole(&mut disk, &two).unwrap_err().torn);

        let dir = scratch("torn");
        let c = cfg(&dir, "torn-cli");
        let mut s = SessionState::for_connect(&c).unwrap();
        s.settle(false).unwrap();
        s.published(1, b"one").unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(session_file(&c))
            .unwrap();
        f.write_all(&[KIND_PUBLISHED, 0, 9]).unwrap();
        s.store.as_mut().unwrap().torn = true;
        s.published(2, b"two").unwrap();
        assert!(!s.store.as_ref().unwrap().torn);
        drop(s);
        let s = SessionState::for_connect(&c).unwrap();
        assert_eq!(
            s.resend_order(),
            [
                Resend::Publish(1, b"one".to_vec()),
                Resend::Publish(2, b"two".to_vec())
            ]
        );
    }

    /// A second holder of the same client identifier's state is refused;
    /// a directory that cannot be made is an error naming it.
    #[test]
    fn mqttbridge_4_1_one_process_holds_a_session_state() {
        let dir = scratch("lock");
        let c = cfg(&dir, "lock-cli");
        let first = SessionState::for_connect(&c).unwrap();
        let e = SessionState::for_connect(&c).unwrap_err();
        assert!(e.contains("another process"), "{e}");
        drop(first);
        assert!(SessionState::for_connect(&c).is_ok());
        let file = scratch("not-a-dir");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"x").unwrap();
        let e = SessionState::for_connect(&cfg(&file.join("sub"), "x")).unwrap_err();
        assert!(e.contains("mqtt.session_dir"), "{e}");
    }

    /// Many completed records make the file rewrite itself with the live
    /// entries alone.
    #[test]
    fn mqttbridge_4_1_the_session_file_is_rewritten_with_the_live_entries() {
        let dir = scratch("rewrite");
        let c = cfg(&dir, "rewrite-cli");
        let mut s = SessionState::for_connect(&c).unwrap();
        s.settle(false).unwrap();
        for i in 0..5000u32 {
            let pid = u16::try_from(i % 60_000 + 1).unwrap();
            s.published(pid, &[0u8; 32]).unwrap();
            s.completed(pid).unwrap();
        }
        s.published(9, b"live").unwrap();
        let len = fs::metadata(session_file(&c)).unwrap().len();
        assert!(len < 4096 * 48, "{len}");
        drop(s);
        let s = SessionState::for_connect(&c).unwrap();
        assert_eq!(s.resend_order(), [Resend::Publish(9, b"live".to_vec())]);
    }
}
