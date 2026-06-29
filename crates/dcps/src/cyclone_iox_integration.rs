// SPDX-License-Identifier: Apache-2.0
//! Wires the [`zerodds_iceoryx_cyclone`] bridge into the DCPS hot path so a
//! normal typed [`DataWriter`]/[`DataReader`], once `enable_cyclone_iox`-ed,
//! interoperates with Cyclone DDS over the iceoryx C++ (POSH) shared-memory
//! transport — cross-vendor same-host zero-copy.
//!
//! The sample is carried as Cyclone's serialized PSMX form: classic CDR / XCDR1
//! ([`DdsType::encode_xcdr1`]) in the user payload, a [`DdsPsmxMetadata`] header
//! (`SERIALIZED_DATA`, `cdr_identifier = CDR_LE`). This covers variable-length
//! and keyed types (Cyclone derives the instance from the deserialized key).
//!
//! Enabled handles are held in side-tables keyed by topic name, so the core
//! `write`/`take` paths gain only a feature-gated best-effort hook and the
//! `DataWriter`/`DataReader` structs are untouched.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use zerodds_iceoryx_cyclone::{
    CYCLONE_IOX_SERVICE, CycloneIoxReader, CycloneIoxWriter, DdsPsmxMetadata, iox_event_name,
};

use crate::dds_type::DdsType;
use crate::error::{DdsError, Result};
use crate::publisher::DataWriter;
use crate::subscriber::DataReader;

// The bridge handles are `Send` but not `Sync` (raw iceoryx C-binding handles),
// so each is wrapped in its own `Mutex` to serialize publish/take access. The
// writer cell also carries the writer's 16-byte RTPS GUID, stamped into every
// PSMX chunk so a Cyclone peer that discovered the writer over SEDP associates
// the shared-memory sample with it.
struct WriterEntry {
    writer: Mutex<CycloneIoxWriter>,
    guid: [u8; 16],
}
type WriterCell = Arc<WriterEntry>;
type ReaderCell = Arc<Mutex<CycloneIoxReader>>;

fn writers() -> &'static Mutex<HashMap<String, WriterCell>> {
    static M: OnceLock<Mutex<HashMap<String, WriterCell>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

fn readers() -> &'static Mutex<HashMap<String, ReaderCell>> {
    static M: OnceLock<Mutex<HashMap<String, ReaderCell>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A stable 16-byte pseudo-GUID for `topic` (FNV-1a, spread over 16 bytes) so a
/// Cyclone reader sees a consistent writer identity across samples.
fn guid_for(topic: &str) -> [u8; 16] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in topic.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let lo = h.to_le_bytes();
    let hi = h.rotate_left(32).to_le_bytes();
    let mut g = [0u8; 16];
    g[..8].copy_from_slice(&lo);
    g[8..].copy_from_slice(&hi);
    g
}

impl<T: DdsType> DataWriter<T> {
    /// Offers this writer's topic over the iceoryx C++ (POSH) transport so a
    /// Cyclone DDS reader (run with `ALLOW_NONDISCOVERED_WRITERS=true`) consumes
    /// its samples. Subsequent [`DataWriter::write`] calls additionally publish
    /// a Cyclone-PSMX serialized chunk. Linux + iceoryx POSH + `iox-roudi`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the side-table lock is poisoned.
    pub fn enable_cyclone_iox(&self) -> Result<()> {
        let topic = self.topic().name().to_string();
        let event = iox_event_name("", &topic);
        let w = CycloneIoxWriter::new("zerodds-dcps", CYCLONE_IOX_SERVICE, T::TYPE_NAME, &event);
        // Prefer the writer's real RTPS GUID (online participant) so a Cyclone
        // reader that discovered it over SEDP accepts the serialized PSMX
        // sample; fall back to a topic-derived pseudo-GUID offline.
        let guid = self.rtps_guid().unwrap_or_else(|| guid_for(&topic));
        writers()
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "cyclone-iox writer table poisoned",
            })?
            .insert(
                topic,
                Arc::new(WriterEntry {
                    writer: Mutex::new(w),
                    guid,
                }),
            );
        Ok(())
    }
}

impl<T: DdsType> DataReader<T> {
    /// Subscribes this reader's topic over the iceoryx C++ (POSH) transport so
    /// [`DataReader::take`] returns Cyclone-published samples. Linux + iceoryx
    /// POSH + `iox-roudi`.
    ///
    /// # Errors
    /// `PreconditionNotMet` if the side-table lock is poisoned.
    pub fn enable_cyclone_iox(&self) -> Result<()> {
        let topic = self.topic().name().to_string();
        let event = iox_event_name("", &topic);
        let r = CycloneIoxReader::new("zerodds-dcps", CYCLONE_IOX_SERVICE, T::TYPE_NAME, &event);
        readers()
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "cyclone-iox reader table poisoned",
            })?
            .insert(topic, Arc::new(Mutex::new(r)));
        Ok(())
    }
}

/// Publishes `sample` over iceoryx if `topic` was `enable_cyclone_iox`-ed.
/// Best-effort: a serialization or loan failure is dropped (the normal RTPS
/// path still ran). Classic CDR / XCDR1 payload + `SERIALIZED_DATA` metadata.
pub(crate) fn publish<T: DdsType>(topic: &str, sample: &T) {
    let entry = writers().lock().ok().and_then(|m| m.get(topic).cloned());
    let Some(entry) = entry else { return };
    let mut buf = Vec::new();
    if sample.encode_xcdr1(&mut buf).is_err() {
        return;
    }
    let meta = DdsPsmxMetadata::serialized(buf.len() as u32, entry.guid);
    if let Ok(w) = entry.writer.lock() {
        let _ = w.write_raw(&meta, &buf);
    }
}

/// Drains any Cyclone-published samples for `topic` (if `enable_cyclone_iox`-ed)
/// and decodes them. Returns an empty vec when nothing is queued or the topic is
/// not iox-enabled.
pub(crate) fn take<T: DdsType>(topic: &str) -> Vec<T> {
    let mut out = Vec::new();
    let r = readers().lock().ok().and_then(|m| m.get(topic).cloned());
    let Some(r) = r else { return out };
    let Ok(r) = r.lock() else { return out };
    while let Some((_meta, payload)) = r.take() {
        if let Ok(s) = T::decode_xcdr1(&payload) {
            out.push(s);
        }
    }
    out
}
