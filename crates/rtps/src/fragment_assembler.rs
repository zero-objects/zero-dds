// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Fragment-Reassembly fuer DDSI-RTPS 2.5 §8.4.14 auf Reader-Seite.
//!
//! Fuehrt pro in-flight Sample-SN einen `FragmentBuffer`, in den DATA_FRAG-
//! Submessages eingespielt werden. Sobald alle Fragmente da sind, faellt
//! ein vollstaendiger Sample heraus, den der `ReliableReader` wie ein
//! regulaeres DATA behandelt.
//!
//! # DoS-Haltung
//!
//! Der Assembler muss Input von ungetrusteten Writers robust verarbeiten.
//! Drei Caps schuetzen gegen pathologische Inputs:
//!
//! - `max_pending_sns`: Hoechstzahl gleichzeitig in Arbeit befindlicher
//!   SNs. Ueberlauf verwirft die aelteste (kleinste) unvollstaendige SN.
//! - `max_sample_bytes`: Obergrenze fuer `sample_size`. DATA_FRAGs mit
//!   `sample_size > cap` werden verworfen **ohne** Allokation —
//!   Schutz gegen "ich behaupte 4 GB sample und hoffe, dass du allokierst".
//! - `max_fragment_size`: Obergrenze fuer `fragment_size`-Angaben vom
//!   Writer. Uebliche MTU ist < 1500; wir akzeptieren bis 65535.
//!
//! Verworfene Fragmente werden in `drop_count` gezaehlt (Diagnose).

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use crate::submessages::{DataFragSubmessage, FragmentNumberSet};
use crate::wire_types::{FragmentNumber, SequenceNumber};

/// Default-Cap fuer Anzahl gleichzeitig in-flight SNs.
pub const DEFAULT_MAX_PENDING_SNS: usize = 64;
/// Default-Cap fuer maximale Sample-Groesse (1 MiB). Groessere Samples
/// sind in Phase 1 kein Use-Case; DDS-Security/Fragmentation auf
/// grossen Images wartet auf Phase 2+.
pub const DEFAULT_MAX_SAMPLE_BYTES: usize = 1024 * 1024;
/// Default-Cap fuer `fragment_size` (u16-Maximum gemaess Spec).
pub const DEFAULT_MAX_FRAGMENT_SIZE: u16 = u16::MAX;

/// Ein vollstaendig reassemblierter Sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedSample {
    /// Writer-Sequence-Number.
    pub sequence_number: SequenceNumber,
    /// Reassemblierter Payload (Gesamt-Sample-Bytes in Originalreihenfolge).
    pub payload: Vec<u8>,
}

/// Konfiguration fuer den Assembler.
#[derive(Debug, Clone, Copy)]
pub struct AssemblerCaps {
    /// Max. Anzahl gleichzeitiger SNs.
    pub max_pending_sns: usize,
    /// Max. sample_size in Bytes.
    pub max_sample_bytes: usize,
    /// Max. fragment_size in Bytes.
    pub max_fragment_size: u16,
}

impl Default for AssemblerCaps {
    /// Konservative Defaults fuer typische DDS-Workloads (1 MiB Samples,
    /// 64 gleichzeitige in-flight SNs, u16-max Fragment-Size).
    fn default() -> Self {
        Self {
            max_pending_sns: DEFAULT_MAX_PENDING_SNS,
            max_sample_bytes: DEFAULT_MAX_SAMPLE_BYTES,
            max_fragment_size: DEFAULT_MAX_FRAGMENT_SIZE,
        }
    }
}

/// Pro-SN-Ringpuffer fuer reinkommende Fragmente.
#[derive(Debug, Clone)]
struct FragmentBuffer {
    sample_size: u32,
    fragment_size: u16,
    total_fragments: u32,
    received: BTreeSet<FragmentNumber>,
    data: Vec<u8>,
}

impl FragmentBuffer {
    fn new(sample_size: u32, fragment_size: u16) -> Self {
        let total = if fragment_size == 0 {
            0
        } else {
            sample_size.div_ceil(u32::from(fragment_size))
        };
        Self {
            sample_size,
            fragment_size,
            total_fragments: total,
            received: BTreeSet::new(),
            data: vec![0u8; sample_size as usize],
        }
    }

    fn is_complete(&self) -> bool {
        self.total_fragments > 0 && self.received.len() as u32 == self.total_fragments
    }

    fn missing(&self) -> FragmentNumberSet {
        if self.total_fragments == 0 {
            return FragmentNumberSet::from_missing(FragmentNumber(1), &[]);
        }
        let mut missing_nums = Vec::new();
        for f in 1..=self.total_fragments {
            let fnum = FragmentNumber(f);
            if !self.received.contains(&fnum) {
                missing_nums.push(fnum);
            }
        }
        let base = missing_nums
            .first()
            .copied()
            .unwrap_or(FragmentNumber(self.total_fragments.saturating_add(1)));
        FragmentNumberSet::from_missing(base, &missing_nums)
    }
}

/// Zurueckgewiesenes-Fragment-Kategorie — nur fuer Diagnostik.
///
/// **Neue Varianten ergaenzen**: pflegeaendernd auch die
/// [`DropReason::as_str`]-Methode anpassen (exhaustive match), sonst
/// bricht der Build. Das ist Absicht — so wird verhindert, dass neue
/// Failure-Modes still in Logging-/Metrics-Pfaden verloren gehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropReason {
    /// `sample_size` ueber Cap.
    SampleTooLarge,
    /// `fragment_size` ueber Cap oder == 0.
    FragmentSizeInvalid,
    /// `fragment_starting_num == 0` (1-basiert erwartet).
    FragmentIndexZero,
    /// Fragment-Index jenseits von `total_fragments`.
    FragmentIndexOutOfRange,
    /// Payload-Laenge passt nicht zu Fragment-Position.
    PayloadSizeMismatch,
    /// Spaeterer DataFrag widerspricht einem schon gespeicherten
    /// (anderes `sample_size` oder `fragment_size`).
    InconsistentWithBuffered,
    /// `fragments_in_submessage == 0` oder inkonsistent.
    FragmentsInSubmessageInvalid,
    /// Anzahl gleichzeitig verwalteter SNs wuerde `max_pending_sns`
    /// uebersteigen — aelteste unvollstaendige SN wurde verworfen.
    PendingSnsCapExceeded,
    /// `max_pending_sns == 0` — Assembler akzeptiert keine Eintraege.
    AssemblerDisabled,
}

impl DropReason {
    /// Stabile String-Repraesentation fuer Logging/Metrics. Exhaustives
    /// Match — neue Varianten brechen hier absichtlich den Build.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SampleTooLarge => "sample_too_large",
            Self::FragmentSizeInvalid => "fragment_size_invalid",
            Self::FragmentIndexZero => "fragment_index_zero",
            Self::FragmentIndexOutOfRange => "fragment_index_out_of_range",
            Self::PayloadSizeMismatch => "payload_size_mismatch",
            Self::InconsistentWithBuffered => "inconsistent_with_buffered",
            Self::FragmentsInSubmessageInvalid => "fragments_in_submessage_invalid",
            Self::PendingSnsCapExceeded => "pending_sns_cap_exceeded",
            Self::AssemblerDisabled => "assembler_disabled",
        }
    }
}

impl core::fmt::Display for DropReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// State eines Reassemblers.
///
/// `FragmentAssembler::default()` liefert einen Assembler mit
/// [`AssemblerCaps::default`] — dem einzigen Defaults-Weg.
#[derive(Debug, Clone, Default)]
pub struct FragmentAssembler {
    buffers: BTreeMap<SequenceNumber, FragmentBuffer>,
    caps: AssemblerCaps,
    drop_count: u64,
    last_drop_reason: Option<DropReason>,
}

impl FragmentAssembler {
    /// Erzeugt einen Assembler mit den gegebenen Caps.
    #[must_use]
    pub fn new(caps: AssemblerCaps) -> Self {
        Self {
            buffers: BTreeMap::new(),
            caps,
            drop_count: 0,
            last_drop_reason: None,
        }
    }

    /// Anzahl aktiver SNs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Ist der Assembler leer?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Anzahl verworfener Fragmente seit Start (oder seit
    /// [`reset_diagnostics`](Self::reset_diagnostics)).
    #[must_use]
    pub fn drop_count(&self) -> u64 {
        self.drop_count
    }

    /// Der Grund des zuletzt verworfenen Fragments, falls ueberhaupt
    /// eines verworfen wurde. Fuer Debugging/Metrics — nicht fuer
    /// Control-Flow-Entscheidungen.
    #[must_use]
    pub fn last_drop_reason(&self) -> Option<DropReason> {
        self.last_drop_reason
    }

    /// Setzt Diagnose-Zaehler auf 0 zurueck. `buffers` bleiben
    /// unveraendert — das ist reine Metric-Hygiene (Long-Running-Reader
    /// wollen ihre Delta-Snapshots).
    pub fn reset_diagnostics(&mut self) {
        self.drop_count = 0;
        self.last_drop_reason = None;
    }

    /// True, wenn fuer mind. eine SN Fragmente fehlen.
    #[must_use]
    pub fn has_gaps(&self) -> bool {
        self.buffers.values().any(|b| !b.is_complete())
    }

    /// Iteriert SNs, fuer die aktuell Fragment-Luecken existieren.
    pub fn incomplete_sns(&self) -> impl Iterator<Item = SequenceNumber> + '_ {
        self.buffers
            .iter()
            .filter(|(_, b)| !b.is_complete())
            .map(|(sn, _)| *sn)
    }

    /// Fehlende Fragmente fuer eine SN. Liefert leeren Set, wenn SN
    /// unbekannt oder bereits komplett.
    #[must_use]
    pub fn missing_fragments(&self, sn: SequenceNumber) -> FragmentNumberSet {
        match self.buffers.get(&sn) {
            Some(b) => b.missing(),
            None => FragmentNumberSet::from_missing(FragmentNumber(1), &[]),
        }
    }

    /// Entfernt den Buffer fuer diese SN (z.B. bei GAP-Signal oder nach
    /// Completion). Gibt den Buffer zurueck, falls vorhanden.
    pub fn discard(&mut self, sn: SequenceNumber) -> bool {
        self.buffers.remove(&sn).is_some()
    }

    /// Spielt ein DATA_FRAG ein. Liefert bei Vervollstaendigung den
    /// reassemblierten Sample.
    ///
    /// Inkonsistente oder pathologische Fragmente werden verworfen und
    /// in `drop_count` gezaehlt — sie koennen nicht die interne Map
    /// ueber die Caps hinaus wachsen lassen.
    pub fn insert(&mut self, df: &DataFragSubmessage) -> Option<CompletedSample> {
        // --- Eingangsvalidierung (no-alloc gate) ----------------------
        if df.fragment_size == 0 || df.fragment_size > self.caps.max_fragment_size {
            self.record_drop(DropReason::FragmentSizeInvalid);
            return None;
        }
        if df.fragments_in_submessage == 0 {
            self.record_drop(DropReason::FragmentsInSubmessageInvalid);
            return None;
        }
        if df.sample_size as usize > self.caps.max_sample_bytes {
            self.record_drop(DropReason::SampleTooLarge);
            return None;
        }
        if df.fragment_starting_num.0 == 0 {
            self.record_drop(DropReason::FragmentIndexZero);
            return None;
        }

        // Vorab-Berechnungen
        let total_fragments = df.sample_size.div_ceil(u32::from(df.fragment_size));
        let last_frag = df
            .fragment_starting_num
            .0
            .checked_add(u32::from(df.fragments_in_submessage) - 1)
            .unwrap_or(u32::MAX);
        if last_frag > total_fragments {
            self.record_drop(DropReason::FragmentIndexOutOfRange);
            return None;
        }

        // Cap: Anzahl gleichzeitig in-flight SNs.
        if !self.buffers.contains_key(&df.writer_sn)
            && self.buffers.len() >= self.caps.max_pending_sns
        {
            // Aelteste SN verwerfen — DoS-Schutz. Der betroffene Sample
            // ist weg; der Reader muss das wie einen GAP-Zustand behandeln.
            let Some(&oldest) = self.buffers.keys().next() else {
                // Cap == 0: niemand darf rein.
                self.record_drop(DropReason::AssemblerDisabled);
                return None;
            };
            self.buffers.remove(&oldest);
            self.record_drop(DropReason::PendingSnsCapExceeded);
        }

        // Buffer anlegen oder konsistent erweitern.
        let buffer = match self.buffers.get_mut(&df.writer_sn) {
            Some(existing) => {
                if existing.sample_size != df.sample_size
                    || existing.fragment_size != df.fragment_size
                {
                    self.record_drop(DropReason::InconsistentWithBuffered);
                    return None;
                }
                existing
            }
            None => {
                self.buffers.insert(
                    df.writer_sn,
                    FragmentBuffer::new(df.sample_size, df.fragment_size),
                );
                self.buffers.get_mut(&df.writer_sn)?
            }
        };

        // Fragment-Bytes an die richtige Position schreiben.
        let frag_size_usize = buffer.fragment_size as usize;
        let frag_count = df.fragments_in_submessage as usize;
        let first_idx = (df.fragment_starting_num.0 - 1) as usize;
        let byte_start = first_idx * frag_size_usize;
        let expected_last_frag = core::cmp::min(last_frag, buffer.total_fragments);
        // Erwartete Payload-Laenge: frag_count-1 volle Fragmente + ggf.
        // verkuerztes letztes Fragment (wenn last_frag == total_fragments).
        let full_portion = (frag_count - 1) * frag_size_usize;
        let tail_size = if expected_last_frag == buffer.total_fragments {
            // Letztes Fragment des Samples darf kuerzer sein.
            buffer.sample_size as usize - ((buffer.total_fragments - 1) as usize) * frag_size_usize
        } else {
            frag_size_usize
        };
        let expected_len = full_portion + tail_size;
        if df.serialized_payload.len() != expected_len {
            self.record_drop(DropReason::PayloadSizeMismatch);
            return None;
        }

        // Schreiben
        let data_end = byte_start + df.serialized_payload.len();
        if data_end > buffer.data.len() {
            self.record_drop(DropReason::PayloadSizeMismatch);
            return None;
        }
        buffer.data[byte_start..data_end].copy_from_slice(&df.serialized_payload);
        for f in 0..df.fragments_in_submessage as u32 {
            buffer
                .received
                .insert(FragmentNumber(df.fragment_starting_num.0 + f));
        }

        if buffer.is_complete() {
            // Buffer entnehmen und CompletedSample zurueckgeben.
            let buf = self.buffers.remove(&df.writer_sn)?;
            return Some(CompletedSample {
                sequence_number: df.writer_sn,
                payload: buf.data,
            });
        }
        None
    }

    fn record_drop(&mut self, reason: DropReason) {
        self.drop_count = self.drop_count.saturating_add(1);
        self.last_drop_reason = Some(reason);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::wire_types::EntityId;

    fn wid() -> EntityId {
        EntityId::user_writer_with_key([0x10, 0x20, 0x30])
    }
    fn rid() -> EntityId {
        EntityId::user_reader_with_key([0x40, 0x50, 0x60])
    }

    fn df(
        sn: i64,
        starting: u32,
        count: u16,
        frag_size: u16,
        sample_size: u32,
        payload: Vec<u8>,
    ) -> DataFragSubmessage {
        DataFragSubmessage {
            extra_flags: 0,
            reader_id: rid(),
            writer_id: wid(),
            writer_sn: SequenceNumber(sn),
            fragment_starting_num: FragmentNumber(starting),
            fragments_in_submessage: count,
            fragment_size: frag_size,
            sample_size,
            serialized_payload: alloc::sync::Arc::from(payload),
            inline_qos_flag: false,
            hash_key_flag: false,
            key_flag: false,
            non_standard_flag: false,
        }
    }

    #[test]
    fn single_fragment_sample_completes_immediately() {
        let mut a = FragmentAssembler::default();
        // sample_size=4, frag_size=4 → 1 Fragment
        let res = a.insert(&df(1, 1, 1, 4, 4, vec![1, 2, 3, 4]));
        assert!(res.is_some());
        let s = res.unwrap();
        assert_eq!(s.sequence_number, SequenceNumber(1));
        assert_eq!(s.payload, vec![1, 2, 3, 4]);
        assert_eq!(a.len(), 0);
    }

    #[test]
    fn two_fragments_complete_in_order() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        let res = a.insert(&df(1, 2, 1, 4, 8, vec![5, 6, 7, 8])).unwrap();
        assert_eq!(res.payload, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn fragments_complete_out_of_order() {
        let mut a = FragmentAssembler::default();
        // 2 zuerst, dann 1, dann 3
        assert!(a.insert(&df(1, 2, 1, 4, 10, vec![5, 6, 7, 8])).is_none());
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        let res = a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).unwrap();
        assert_eq!(res.payload, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn last_fragment_shorter_than_fragment_size() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 2, 1, 4, 10, vec![5, 6, 7, 8])).is_none());
        let res = a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).unwrap();
        assert_eq!(res.payload.len(), 10);
    }

    #[test]
    fn duplicate_fragment_is_idempotent() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.missing_fragments(SequenceNumber(1)).num_bits, 1);
    }

    #[test]
    fn missing_fragments_enumerates_gaps() {
        let mut a = FragmentAssembler::default();
        // Fragment 2 fehlt
        assert!(a.insert(&df(1, 1, 1, 4, 10, vec![1, 2, 3, 4])).is_none());
        assert!(a.insert(&df(1, 3, 1, 4, 10, vec![9, 10])).is_none());
        let ms = a.missing_fragments(SequenceNumber(1));
        let collected: Vec<_> = ms.iter_set().collect();
        assert_eq!(collected, vec![FragmentNumber(2)]);
    }

    #[test]
    fn inconsistent_sample_size_drops_fragment() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4])).is_none());
        // Zweiter Fragment meldet sample_size=12 statt 8 → verworfen
        let res = a.insert(&df(1, 2, 1, 4, 12, vec![5, 6, 7, 8]));
        assert!(res.is_none());
        assert_eq!(a.drop_count(), 1);
        // sn=1 ist weiter in-flight mit dem alten sample_size=8
        assert_eq!(a.missing_fragments(SequenceNumber(1)).num_bits, 1);
    }

    #[test]
    fn sample_too_large_drops_without_alloc() {
        let caps = AssemblerCaps {
            max_sample_bytes: 16,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        // sample_size=100 > cap=16 → verworfen
        assert!(a.insert(&df(1, 1, 1, 4, 100, vec![1, 2, 3, 4])).is_none());
        assert!(a.is_empty());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_size_zero_dropped() {
        let mut a = FragmentAssembler::default();
        // frag_size=0 → div by 0 vermeiden
        assert!(a.insert(&df(1, 1, 1, 0, 4, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_index_zero_dropped() {
        let mut a = FragmentAssembler::default();
        assert!(a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn fragment_index_out_of_range_dropped() {
        let mut a = FragmentAssembler::default();
        // sample_size=4, frag_size=4 → total=1, aber Index 2 angefragt
        assert!(a.insert(&df(1, 2, 1, 4, 4, vec![0])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn payload_size_mismatch_dropped() {
        let mut a = FragmentAssembler::default();
        // frag_size=4 aber payload ist nur 2 Byte → mismatch
        assert!(a.insert(&df(1, 1, 1, 4, 8, vec![1, 2])).is_none());
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn max_pending_sns_evicts_oldest() {
        let caps = AssemblerCaps {
            max_pending_sns: 2,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        // SN 1, 2 offen (nur je Fragment 1 von 2 erhalten)
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.len(), 2);
        // SN 3 drueckt SN 1 raus
        a.insert(&df(3, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.len(), 2);
        assert!(a.buffers.contains_key(&SequenceNumber(2)));
        assert!(a.buffers.contains_key(&SequenceNumber(3)));
        assert_eq!(a.drop_count(), 1);
    }

    #[test]
    fn has_gaps_flips_to_false_after_completion() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert!(a.has_gaps());
        a.insert(&df(1, 2, 1, 4, 8, vec![5, 6, 7, 8]));
        assert!(!a.has_gaps());
    }

    #[test]
    fn incomplete_sns_enumerates_in_order() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(5, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        let sns: Vec<_> = a.incomplete_sns().collect();
        assert_eq!(sns, vec![SequenceNumber(2), SequenceNumber(5)]);
    }

    #[test]
    fn discard_removes_buffer() {
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert!(a.discard(SequenceNumber(1)));
        assert!(a.is_empty());
        assert!(!a.discard(SequenceNumber(1)));
    }

    #[test]
    fn missing_for_unknown_sn_is_empty() {
        let a = FragmentAssembler::default();
        assert_eq!(a.missing_fragments(SequenceNumber(42)).num_bits, 0);
    }

    // ---- fragments_in_submessage > 1 (Bundle-Decode) ----

    #[test]
    fn bundled_fragments_all_full() {
        // 3 Fragmente in einem Submessage, alle voll (kein tail).
        // sample_size=18, frag_size=4, total=5. Wir bundeln Fragmente 1-3.
        let mut a = FragmentAssembler::default();
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let res = a.insert(&df(1, 1, 3, 4, 18, payload.clone()));
        assert!(res.is_none(), "not yet complete");
        // Fragmente 4, 5 fehlen noch
        let ms: Vec<_> = a.missing_fragments(SequenceNumber(1)).iter_set().collect();
        assert_eq!(ms, vec![FragmentNumber(4), FragmentNumber(5)]);
    }

    #[test]
    fn bundled_fragments_including_last_with_tail() {
        // 2 Fragmente in einem Submessage, inkl. letztem (Tail verkuerzt).
        // sample_size=10, frag_size=4, total=3. Bundle: Fragmente 2-3.
        let mut a = FragmentAssembler::default();
        // Erst Fragment 1 vorlegen
        assert!(
            a.insert(&df(1, 1, 1, 4, 10, vec![0xA, 0xB, 0xC, 0xD]))
                .is_none()
        );
        // Jetzt Bundle 2+3 (4 + 2 Byte = 6)
        let bundle = vec![5, 6, 7, 8, 9, 10];
        let res = a.insert(&df(1, 2, 2, 4, 10, bundle));
        assert!(res.is_some());
        let s = res.unwrap();
        assert_eq!(s.payload, vec![0xA, 0xB, 0xC, 0xD, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn bundled_fragments_payload_size_mismatch_rejected() {
        // Bundle mit behaupteten 3 Fragmenten à 4 Byte = 12, aber
        // nur 10 Byte geliefert.
        let mut a = FragmentAssembler::default();
        let payload = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert!(a.insert(&df(1, 1, 3, 4, 20, payload)).is_none());
        assert_eq!(a.drop_count(), 1);
        assert_eq!(a.last_drop_reason(), Some(DropReason::PayloadSizeMismatch));
    }

    // ---- last_drop_reason Diagnose ----

    #[test]
    fn last_drop_reason_tracks_most_recent() {
        let mut a = FragmentAssembler::default();
        assert_eq!(a.last_drop_reason(), None);
        a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::FragmentIndexZero));
        a.insert(&df(1, 1, 1, 0, 4, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::FragmentSizeInvalid));
    }

    #[test]
    fn pending_sns_cap_exceeded_uses_dedicated_reason() {
        let caps = AssemblerCaps {
            max_pending_sns: 1,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(
            a.last_drop_reason(),
            Some(DropReason::PendingSnsCapExceeded)
        );
    }

    #[test]
    fn default_assembler_uses_default_caps() {
        // B2-Regression: Default-Trait muss einen funktionierenden
        // Assembler liefern (nicht nur Zero-State, sondern korrekte Caps).
        let mut a = FragmentAssembler::default();
        assert!(a.is_empty());
        // Typischer Fall: 1-Fragment-Sample complete
        let res = a.insert(&df(1, 1, 1, 4, 4, vec![1, 2, 3, 4]));
        assert!(res.is_some());
    }

    #[test]
    fn reset_diagnostics_clears_counters_but_keeps_buffers() {
        // B8-Regression: reset_diagnostics soll nur Metrik-State
        // nullieren, in-flight Buffer bleiben erhalten.
        let mut a = FragmentAssembler::default();
        a.insert(&df(1, 0, 1, 4, 4, vec![1, 2, 3, 4])); // FragmentIndexZero → drop
        a.insert(&df(2, 1, 1, 4, 8, vec![1, 2, 3, 4])); // partial buffer
        assert_eq!(a.drop_count(), 1);
        assert_eq!(a.len(), 1);
        a.reset_diagnostics();
        assert_eq!(a.drop_count(), 0);
        assert!(a.last_drop_reason().is_none());
        assert_eq!(a.len(), 1, "buffers must stay intact");
    }

    #[test]
    fn max_pending_sns_zero_rejects_with_assembler_disabled() {
        let caps = AssemblerCaps {
            max_pending_sns: 0,
            ..AssemblerCaps::default()
        };
        let mut a = FragmentAssembler::new(caps);
        a.insert(&df(1, 1, 1, 4, 8, vec![1, 2, 3, 4]));
        assert_eq!(a.last_drop_reason(), Some(DropReason::AssemblerDisabled));
        assert!(a.is_empty());
    }
}
