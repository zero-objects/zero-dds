// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE FRAGMENT-Reassembler (Spec §8.4.13).
//!
//! Spec §8.3.5.14 erlaubt FRAGMENT-Submessages nur in reliable Streams.
//! Jedes Fragment traegt einen Body-Slice der zerlegten Original-
//! Submessage und ein `LAST`-Flag (siehe `crate::submessages::fragment`).
//!
//! Der Reassembler ist tick-frei: Fragmente werden via `add_fragment`
//! eingespielt; sobald `LAST` empfangen ist und alle Bytes-Bereiche luecken-
//! frei sind, wird `Some(Vec<u8>)` zurueckgeliefert. Bis dahin behaelt der
//! Reassembler eine einzige Pre-Allokation pro `(stream, base_seq)`.
//!
//! ## DoS-Caps
//! - `MAX_FRAGMENTS_PER_STREAM = 256`: maximale Anzahl Fragmente pro
//!   in-flight Reassembly. 256 deckt 256·1500 ≈ 384 KiB ab — mehr als
//!   genug fuer den realistischen Use-Case.
//! - `MAX_TOTAL_PAYLOAD = 1 MiB`: harter Cap fuer den reassemblierten
//!   Payload. Wird auch zum Pre-Allocation-Limit genutzt (kein
//!   "behauptete 4 GiB" → OOM).
//! - `MAX_PENDING_STREAMS = 32`: gleichzeitig zulaessig laufende
//!   Reassemblies. Schuetzt vor "1000 Streams a 1 Fragment".
//! - GC: jedes Fragment hat einen `arrival`-Tick; alte Reassemblies
//!   werden via `gc(now)` nach `gc_ttl` verworfen.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;

use crate::error::XrceError;

/// DoS-Cap: maximale Fragmente pro in-flight Reassembly.
pub const MAX_FRAGMENTS_PER_STREAM: usize = 256;
/// DoS-Cap: maximale Gesamtgroesse einer reassemblierten Submessage.
pub const MAX_TOTAL_PAYLOAD: usize = 1 << 20; // 1 MiB
/// DoS-Cap: maximale Anzahl gleichzeitig laufender Reassemblies.
pub const MAX_PENDING_STREAMS: usize = 32;
/// Default-TTL fuer abgelaufene Reassemblies (10 s).
pub const DEFAULT_GC_TTL: Duration = Duration::from_secs(10);

/// Schluessel einer in-flight Reassembly: `(stream_id, base_seqnr)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssemblerKey {
    /// Reliable Stream-Id.
    pub stream_id: u8,
    /// Base-Sequence-Number der ersten Fragment-Submessage.
    pub base_seq: u16,
}

#[derive(Debug, Clone)]
struct PendingAssembly {
    /// Geordnete Map `offset → bytes`. Sobald `last_offset_end`
    /// gesetzt und alle Slots zusammenhaengend sind, ist die
    /// Reassembly fertig.
    fragments: BTreeMap<u32, Vec<u8>>,
    /// Wenn `LAST` empfangen ist, das End-Offset (offset + len) des
    /// letzten Fragments. Markiert die Gesamtlaenge.
    final_size: Option<u32>,
    /// Tick des letzten Inputs — fuer GC.
    last_arrival: Duration,
}

impl PendingAssembly {
    fn new(now: Duration) -> Self {
        Self {
            fragments: BTreeMap::new(),
            final_size: None,
            last_arrival: now,
        }
    }

    fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    fn current_total(&self) -> usize {
        self.fragments.values().map(Vec::len).sum()
    }

    /// Ist die Reassembly luecken-frei und hat das letzte Fragment?
    fn is_complete(&self) -> bool {
        let Some(target) = self.final_size else {
            return false;
        };
        let mut cursor: u32 = 0;
        for (&offset, bytes) in &self.fragments {
            if offset != cursor {
                return false;
            }
            cursor = cursor.saturating_add(bytes.len() as u32);
        }
        cursor == target
    }

    fn assemble(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.final_size.unwrap_or(0) as usize);
        for (_, frag) in self.fragments {
            out.extend_from_slice(&frag);
        }
        out
    }
}

/// FRAGMENT-Reassembler ueber alle reliable Streams.
#[derive(Debug, Clone, Default)]
pub struct FragmentAssembler {
    pending: BTreeMap<AssemblerKey, PendingAssembly>,
    gc_ttl: Duration,
    drop_count: u64,
}

impl FragmentAssembler {
    /// Konstruktor mit Default-TTL.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            gc_ttl: DEFAULT_GC_TTL,
            drop_count: 0,
        }
    }

    /// Mit explizit gesetzter GC-TTL.
    #[must_use]
    pub fn with_gc_ttl(ttl: Duration) -> Self {
        Self {
            pending: BTreeMap::new(),
            gc_ttl: ttl,
            drop_count: 0,
        }
    }

    /// Anzahl gleichzeitig laufender Reassemblies.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Anzahl insgesamt verworfener Fragmente (DoS-Diagnose).
    #[must_use]
    pub fn drop_count(&self) -> u64 {
        self.drop_count
    }

    /// Spielt ein Fragment ein. `offset` ist der Byte-Offset des
    /// Fragments innerhalb der zerlegten Original-Submessage.
    /// Liefert `Some(payload)`, wenn die Reassembly mit diesem Aufruf
    /// vollstaendig wird; `None` sonst.
    ///
    /// # Errors
    /// - `PayloadTooLarge`, wenn die Reassembly `MAX_TOTAL_PAYLOAD`
    ///   ueberschreiten wuerde.
    /// - `ValueOutOfRange`, wenn ein Cap erreicht ist
    ///   (`MAX_FRAGMENTS_PER_STREAM`, `MAX_PENDING_STREAMS`) oder ein
    ///   Fragment-Overlap entsteht.
    pub fn add_fragment(
        &mut self,
        key: AssemblerKey,
        offset: u32,
        last_flag: bool,
        bytes: Vec<u8>,
        now: Duration,
    ) -> Result<Option<Vec<u8>>, XrceError> {
        let new_end = offset
            .checked_add(bytes.len() as u32)
            .ok_or(XrceError::ValueOutOfRange {
                message: "fragment offset+len overflow",
            })?;
        if (new_end as usize) > MAX_TOTAL_PAYLOAD {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_TOTAL_PAYLOAD,
                actual: new_end as usize,
            });
        }

        // Neuen Pending-Bucket anlegen ggf. mit Stream-Cap-Check.
        let entry = match self.pending.get_mut(&key) {
            Some(e) => e,
            None => {
                if self.pending.len() >= MAX_PENDING_STREAMS {
                    self.drop_count = self.drop_count.saturating_add(1);
                    return Err(XrceError::ValueOutOfRange {
                        message: "fragment assembler max-pending-streams reached",
                    });
                }
                self.pending
                    .entry(key)
                    .or_insert_with(|| PendingAssembly::new(now))
            }
        };
        entry.last_arrival = now;

        if entry.fragment_count() >= MAX_FRAGMENTS_PER_STREAM {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::ValueOutOfRange {
                message: "fragment assembler max-fragments-per-stream reached",
            });
        }

        // Ueberlapp-Check (kein gleicher offset, kein Overlap).
        if entry.fragments.contains_key(&offset) {
            // duplicates dropped
            self.drop_count = self.drop_count.saturating_add(1);
            return Ok(None);
        }
        // Pruefen ob `[offset, new_end)` mit existierenden Ranges
        // ueberlappt: hole groessten key <= offset und kleinsten key > offset.
        let prev = entry.fragments.range(..=offset).next_back();
        if let Some((&po, pb)) = prev {
            let pe = po + pb.len() as u32;
            if pe > offset {
                self.drop_count = self.drop_count.saturating_add(1);
                return Err(XrceError::ValueOutOfRange {
                    message: "overlapping fragment",
                });
            }
        }
        let next = entry.fragments.range(offset..).next();
        if let Some((&no, _)) = next {
            if new_end > no {
                self.drop_count = self.drop_count.saturating_add(1);
                return Err(XrceError::ValueOutOfRange {
                    message: "overlapping fragment",
                });
            }
        }

        // Total-Cap nach Insert?
        if entry.current_total() + bytes.len() > MAX_TOTAL_PAYLOAD {
            self.drop_count = self.drop_count.saturating_add(1);
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_TOTAL_PAYLOAD,
                actual: entry.current_total() + bytes.len(),
            });
        }

        entry.fragments.insert(offset, bytes);
        if last_flag {
            entry.final_size = Some(new_end);
        }

        let complete = entry.is_complete();
        if complete {
            if let Some(done) = self.pending.remove(&key) {
                return Ok(Some(done.assemble()));
            }
        }
        Ok(None)
    }

    /// GC: verwirft alle Reassemblies, deren letztes Fragment laenger als
    /// `gc_ttl` her ist. Liefert die Anzahl verworfener Buckets.
    pub fn gc(&mut self, now: Duration) -> usize {
        let cutoff = now.saturating_sub(self.gc_ttl);
        let to_drop: Vec<AssemblerKey> = self
            .pending
            .iter()
            .filter(|(_, p)| p.last_arrival < cutoff)
            .map(|(k, _)| *k)
            .collect();
        let n = to_drop.len();
        for k in to_drop {
            self.pending.remove(&k);
            self.drop_count = self.drop_count.saturating_add(1);
        }
        n
    }

    /// Setzt alle Pending-Buckets zurueck (z.B. bei `RESET`).
    pub fn reset(&mut self) {
        let n = self.pending.len() as u64;
        self.pending.clear();
        self.drop_count = self.drop_count.saturating_add(n);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn k() -> AssemblerKey {
        AssemblerKey {
            stream_id: 0x80,
            base_seq: 0,
        }
    }

    #[test]
    fn happy_path_two_fragments() {
        let mut a = FragmentAssembler::new();
        let r = a
            .add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        let r = a
            .add_fragment(k(), 4, true, vec![5, 6, 7, 8], Duration::ZERO)
            .unwrap();
        assert_eq!(r.unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(a.pending_count(), 0);
    }

    #[test]
    fn out_of_order_reassembly() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 4, true, vec![5, 6, 7, 8], Duration::ZERO)
            .unwrap();
        let r = a
            .add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        assert_eq!(r.unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn missing_middle_fragment_does_not_complete() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        let r = a
            .add_fragment(k(), 4, true, vec![5, 6], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        assert_eq!(a.pending_count(), 1);
    }

    #[test]
    fn duplicate_offset_dropped() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        let before = a.drop_count();
        let r = a
            .add_fragment(k(), 0, false, vec![9, 9], Duration::ZERO)
            .unwrap();
        assert!(r.is_none());
        assert_eq!(a.drop_count(), before + 1);
    }

    #[test]
    fn overlapping_fragments_rejected() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1, 2, 3, 4], Duration::ZERO)
            .unwrap();
        let res = a.add_fragment(k(), 2, false, vec![9, 9], Duration::ZERO);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn dos_cap_max_total_payload() {
        let mut a = FragmentAssembler::new();
        let res = a.add_fragment(
            k(),
            0,
            true,
            vec![0u8; MAX_TOTAL_PAYLOAD + 1],
            Duration::ZERO,
        );
        assert!(matches!(res, Err(XrceError::PayloadTooLarge { .. })));
    }

    #[test]
    fn dos_cap_max_pending_streams() {
        let mut a = FragmentAssembler::new();
        for i in 0..MAX_PENDING_STREAMS as u16 {
            let key = AssemblerKey {
                stream_id: 0x80,
                base_seq: i,
            };
            a.add_fragment(key, 0, false, vec![0u8; 4], Duration::ZERO)
                .unwrap();
        }
        let key = AssemblerKey {
            stream_id: 0x80,
            base_seq: 999,
        };
        let res = a.add_fragment(key, 0, false, vec![0u8; 4], Duration::ZERO);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn dos_cap_max_fragments_per_stream() {
        let mut a = FragmentAssembler::new();
        for i in 0..MAX_FRAGMENTS_PER_STREAM as u32 {
            a.add_fragment(k(), i * 4, false, vec![0u8; 4], Duration::ZERO)
                .unwrap();
        }
        let res = a.add_fragment(
            k(),
            (MAX_FRAGMENTS_PER_STREAM as u32) * 4,
            true,
            vec![0u8; 4],
            Duration::ZERO,
        );
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn gc_drops_stale_assemblies() {
        let mut a = FragmentAssembler::with_gc_ttl(Duration::from_secs(5));
        a.add_fragment(k(), 0, false, vec![1, 2], Duration::ZERO)
            .unwrap();
        assert_eq!(a.pending_count(), 1);
        // 4s spaeter → noch nicht abgelaufen
        assert_eq!(a.gc(Duration::from_secs(4)), 0);
        assert_eq!(a.pending_count(), 1);
        // 11s spaeter → abgelaufen
        assert_eq!(a.gc(Duration::from_secs(11)), 1);
        assert_eq!(a.pending_count(), 0);
    }

    #[test]
    fn reset_clears_all() {
        let mut a = FragmentAssembler::new();
        a.add_fragment(k(), 0, false, vec![1], Duration::ZERO)
            .unwrap();
        a.reset();
        assert_eq!(a.pending_count(), 0);
        assert!(a.drop_count() >= 1);
    }
}
