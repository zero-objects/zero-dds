// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `HistoryCache` — geordnete Sample-Ablage fuer Reliable Writer/Reader.
//!
//! DDSI-RTPS 2.5 §8.4.8. Beide Seiten (Writer + Reader) halten je eine
//! eigene Cache-Instanz:
//!
//! - **Writer-Cache**: per `write()` abgelegte `CacheChange`s, aus denen
//!   auf AckNack-Request hin re-gesendet wird. Entfernt Samples erst,
//!   wenn **alle** matched Reader sie via AckNack bestaetigt haben.
//! - **Reader-Cache**: empfangene `CacheChange`s in SN-Reihenfolge, fuer
//!   in-order Delivery an die Applikations-Schicht. Kann via
//!   `remove_up_to` nach Delivery geleert werden.
//!
//! **History-QoS (WP 1.4 T3-Follow-up):** der Cache wird ueber
//! [`HistoryKind`] konfiguriert — `KeepAll` (hart-begrenzt, Error bei
//! Overflow) vs. `KeepLast(depth)` (Ring-Buffer, aeltestes Sample faellt
//! bei Overflow heraus). KeepLast ist Spec-gerecht (§8.7.4) und
//! entkoppelt Writer-Cache-GC von Reader-ACKNACK-Progress — ein
//! stalled Reader verhindert damit nicht mehr, dass andere
//! Reader weitere Samples bekommen ("per-destination queue"-Modell).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

use crate::wire_types::SequenceNumber;

#[cfg(feature = "inspect")]
use alloc::borrow::ToOwned;

#[cfg(feature = "inspect")]
fn dispatch_rtps_tap(label: &str, sn: SequenceNumber, payload: Vec<u8>) {
    let ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    #[allow(clippy::cast_sign_loss)]
    let corr = sn.0 as u64;
    let frame = zerodds_inspect_endpoint::Frame::rtps(label.to_owned(), ts_ns, corr, payload);
    zerodds_inspect_endpoint::tap::dispatch(&frame);
}

/// Art eines Cache-Eintrags (DDSI-RTPS §8.2.1.2 / §8.7.2.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Gueltiges Sample, von DDS-DataReader-Filter akzeptiert.
    Alive,
    /// Gueltiges Sample, vom Reader-side Filter (TIME_BASED_FILTER /
    /// ContentFilteredTopic) verworfen — bleibt aber im NACK-Pfad
    /// "available", damit Reliable-Writer es nicht erneut sendet
    /// (filteredCount-Zaehler in `ChangeFromWriter`).
    AliveFiltered,
    /// `dispose`-Marker.
    NotAliveDisposed,
    /// `unregister`-Marker.
    NotAliveUnregistered,
    /// Kombinierter dispose+unregister.
    NotAliveDisposedUnregistered,
}

impl ChangeKind {
    /// Spec §8.4.10.5 — `is_relevant` true fuer alle Live-Kinds; nur
    /// `AliveFiltered` ist explizit *nicht* relevant fuer den DDS-
    /// User-API-Path (zaehlt aber im NACK-Pfad als "received").
    #[must_use]
    pub fn is_relevant(self) -> bool {
        !matches!(self, Self::AliveFiltered)
    }

    /// Spec §8.4.10.5 — `is_alive_kind` umfasst Alive + AliveFiltered.
    #[must_use]
    pub fn is_alive_kind(self) -> bool {
        matches!(self, Self::Alive | Self::AliveFiltered)
    }
}

/// Einzelner Cache-Eintrag.
///
/// `payload` wird als `Arc<[u8]>` gehalten — Cache, Writer-Build-
/// Datagram-Pfad und Reader-Delivery teilen sich eine einzige
/// Allocation. Das spart im Reliable-Writer-Tick den n-fachen
/// `Vec::clone()` pro Reader-Proxy (Perf-Audit F7/F8/F10, ~30-50 %
/// Throughput-Gewinn bei grossen Payloads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheChange {
    /// Sequence-Number (writer-lokal eindeutig).
    pub sequence_number: SequenceNumber,
    /// Nutzlast (serialisierter Sample), referenzgezaehlt.
    pub payload: Arc<[u8]>,
    /// Art des Events.
    pub kind: ChangeKind,
    /// Optionaler `PID_KEY_HASH` aus dem Inline-QoS (Spec §9.6.4.8).
    /// Reader-Side: bei keyed Topics + ALIVE-Samples mit Inline-Hash
    /// oder bei Lifecycle-Markern (Disposed/Unregistered) gefuellt
    /// vom Reader-Pfad. Writer-Side: gesetzt vom Writer wenn der Pfad
    /// den Hash entlang der Sample-Pipeline propagiert.
    pub key_hash: Option<[u8; 16]>,
}

impl CacheChange {
    /// Erstellt ein Alive-Change. Nimmt `Vec<u8>` entgegen und
    /// konvertiert einmalig in `Arc<[u8]>`. Fuer Call-Sites, die die
    /// Allocation bereits als Arc haben, gibt es [`Self::alive_arc`].
    #[must_use]
    pub fn alive(sn: SequenceNumber, payload: Vec<u8>) -> Self {
        Self::alive_arc(sn, Arc::from(payload))
    }

    /// Erstellt ein Alive-Change aus einer bereits existierenden
    /// `Arc<[u8]>`-Payload. Zero-Copy-Pfad fuer Caller, die die
    /// Allocation teilen (Writer ↔ Cache ↔ Datagram).
    ///
    /// **Crate-intern:** externe Nutzer sollen via [`Self::alive`] mit
    /// `Vec<u8>` eintreten — der Arc-Pfad ist reine Writer/Reader-
    /// interne Optimierung und soll nicht versehentlich Teil der
    /// ffentlichen API werden.
    #[must_use]
    pub(crate) fn alive_arc(sn: SequenceNumber, payload: Arc<[u8]>) -> Self {
        Self {
            sequence_number: sn,
            payload,
            kind: ChangeKind::Alive,
            key_hash: None,
        }
    }

    /// Erstellt einen Lifecycle-Marker (Spec §8.2.1.2). `payload` ist die
    /// Key-Only-Serialisierung der disposed/unregistered Instanz —
    /// genau das, was als `PID_KEY_HASH` in der Inline-QoS landet.
    #[must_use]
    pub fn lifecycle(sn: SequenceNumber, payload: Vec<u8>, kind: ChangeKind) -> Self {
        Self {
            sequence_number: sn,
            payload: Arc::from(payload),
            kind,
            key_hash: None,
        }
    }
}

/// History-QoS (DDSI-RTPS §8.7.4, Spec Table 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    /// Cache ist hart begrenzt; bei Overflow liefert `insert` einen
    /// `CapacityExceeded`-Fehler. Nuetzlich fuer No-Loss-Szenarien, in
    /// denen der Writer eher blockieren als Daten verwerfen will
    /// (Dateitransfer, Logging).
    KeepAll,
    /// Cache haelt maximal `depth` neueste Samples. Bei Overflow faellt
    /// automatisch das **aelteste** Sample raus — Writer-`insert`
    /// schlaegt nie wegen Kapazitaet fehl. Spec-Default fuer DDS.
    KeepLast {
        /// Maximalzahl Samples im Cache.
        depth: usize,
    },
}

/// Fehler-Varianten fuer Cache-Operationen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheError {
    /// `KeepAll`-Cache hat seine Kapazitaet erreicht.
    CapacityExceeded,
    /// Dieselbe SN wurde bereits eingefuegt.
    DuplicateSequenceNumber,
    /// `KeepLast` mit `depth == 0` — jedes Insert waere sofortiges
    /// Drop. Der Fall wird bei `insert` abgelehnt statt silent akzeptiert.
    ZeroDepth,
}

/// Sentinel-Wert fuer "kein Eintrag" in den `AtomicI64`-Stats. Wird
/// gewaehlt damit jeder gueltige `SequenceNumber.0` (>= 1 per Spec)
/// als positiver Wert eindeutig unterscheidbar ist.
const STATS_SENTINEL_NO_SN: i64 = i64::MIN;

/// Atomar-aktualisierte Snapshot-Statistik eines [`HistoryCache`].
///
/// **** das eigentliche `BTreeMap`-Storage des Caches
/// braucht weiter `&mut self` zum Mutieren (lese-/schreib-konkurrente
/// `BTreeMap`-Mutation gibt's in `std` nicht), aber **Statistik-Werte**
/// (Laenge, max/min SN, Eviction-Counter) werden parallel in einem
/// `Arc<HistoryCacheStats>` mitgefuehrt. Monitoring-Threads, SEDP-Tick-
/// Loops und Telemetrie koennen so zustimmungsfrei pollen, ohne den
/// Writer/Reader-Lock zu nehmen.
///
/// Konsistenz-Garantie: jede mutierende Methode des Caches updated die
/// Atomics **nach** der `BTreeMap`-Mutation, mit `Release`-Ordering.
/// Reader nutzen `Acquire`-Ordering — sie sehen einen konsistenten
/// Stand der **letzten** abgeschlossenen Cache-Operation, nie einen
/// halb-aktualisierten Zustand der einzelnen Atomics.
///
/// Was *nicht* garantiert ist: cross-field-Konsistenz. Wenn ein
/// Reader `len` und dann `max_sn` liest, koennen zwischen den
/// Loads weitere Inserts passiert sein. Das ist akzeptabel fuer
/// Monitoring; fuer harte Wire-Pfade (Heartbeat-Build) wird weiter
/// der Writer-Lock genommen.
#[derive(Debug)]
pub struct HistoryCacheStats {
    /// Anzahl Changes im Cache (entspricht `BTreeMap::len`).
    pub len: AtomicUsize,
    /// Anzahl per `KeepLast`-Eviction verworfener Samples seit Start.
    pub evicted: AtomicU64,
    /// Hoechste SN im Cache, oder [`STATS_SENTINEL_NO_SN`] wenn leer.
    pub max_sn: AtomicI64,
    /// Niedrigste SN im Cache, oder [`STATS_SENTINEL_NO_SN`] wenn leer.
    pub min_sn: AtomicI64,
}

impl Default for HistoryCacheStats {
    fn default() -> Self {
        Self {
            len: AtomicUsize::new(0),
            evicted: AtomicU64::new(0),
            max_sn: AtomicI64::new(STATS_SENTINEL_NO_SN),
            min_sn: AtomicI64::new(STATS_SENTINEL_NO_SN),
        }
    }
}

impl HistoryCacheStats {
    /// Snapshot der vier Atomics als Plain-Old-Data-Struct. Wird mit
    /// `Acquire`-Ordering geladen — synchronisiert mit der
    /// `Release`-Speicheroperation in [`HistoryCache::insert`] /
    /// [`HistoryCache::remove_up_to`].
    #[must_use]
    pub fn snapshot(&self) -> HistoryCacheSnapshot {
        HistoryCacheSnapshot {
            len: self.len.load(Ordering::Acquire),
            evicted: self.evicted.load(Ordering::Acquire),
            max_sn: decode_sn_atom(self.max_sn.load(Ordering::Acquire)),
            min_sn: decode_sn_atom(self.min_sn.load(Ordering::Acquire)),
        }
    }
}

impl Clone for HistoryCacheStats {
    fn clone(&self) -> Self {
        Self {
            len: AtomicUsize::new(self.len.load(Ordering::Acquire)),
            evicted: AtomicU64::new(self.evicted.load(Ordering::Acquire)),
            max_sn: AtomicI64::new(self.max_sn.load(Ordering::Acquire)),
            min_sn: AtomicI64::new(self.min_sn.load(Ordering::Acquire)),
        }
    }
}

fn decode_sn_atom(v: i64) -> Option<SequenceNumber> {
    if v == STATS_SENTINEL_NO_SN {
        None
    } else {
        Some(SequenceNumber(v))
    }
}

fn encode_sn_atom(sn: Option<SequenceNumber>) -> i64 {
    sn.map_or(STATS_SENTINEL_NO_SN, |s| s.0)
}

/// Plain-Old-Data-Snapshot der `HistoryCache`-Statistiken zu einem
/// einzelnen Zeitpunkt. Wird von [`HistoryCacheStats::snapshot`]
/// erzeugt; jede Komponente ist mit `Acquire`-Ordering geladen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCacheSnapshot {
    /// Anzahl Changes.
    pub len: usize,
    /// Anzahl per `KeepLast`-Eviction verworfener Samples.
    pub evicted: u64,
    /// Hoechste SN, falls vorhanden.
    pub max_sn: Option<SequenceNumber>,
    /// Niedrigste SN, falls vorhanden.
    pub min_sn: Option<SequenceNumber>,
}

/// Geordnete Sample-Ablage.
///
/// Interner Storage: `BTreeMap` fuer O(log n) Insert/Lookup und effizienten
/// Range-Iterator.
///
/// **** die schreibenden Methoden brauchen weiter
/// `&mut self` (BTreeMap ist nicht concurrent-safe), aber Stats sind
/// in einem `Arc<HistoryCacheStats>` parallel verfuegbar — siehe
/// [`stats`](Self::stats).
#[derive(Debug)]
pub struct HistoryCache {
    changes: BTreeMap<SequenceNumber, CacheChange>,
    kind: HistoryKind,
    max_samples: usize,
    evicted_count: u64,
    stats: Arc<HistoryCacheStats>,
    /// Optionaler Label fuer Inspect-Endpoint-Tap (R-026). Nur mit
    /// Cargo-Feature `inspect` gesetzt; sonst phantom.
    #[cfg(feature = "inspect")]
    inspect_label: Option<alloc::string::String>,
}

impl Clone for HistoryCache {
    fn clone(&self) -> Self {
        // Jeder Klon bekommt ein **eigenes** `Arc<HistoryCacheStats>`,
        // damit Mutationen am Klon nicht die Stats des Originals
        // verfaelschen. Der initiale Wert wird aus dem Original
        // uebernommen, sodass ein Cache.clone() einen aequivalenten
        // Snapshot zeigt.
        Self {
            changes: self.changes.clone(),
            kind: self.kind,
            max_samples: self.max_samples,
            evicted_count: self.evicted_count,
            stats: Arc::new((*self.stats).clone()),
            #[cfg(feature = "inspect")]
            inspect_label: self.inspect_label.clone(),
        }
    }
}

impl HistoryCache {
    /// Erzeugt einen neuen Cache. `max_samples` ist die obere Grenze:
    /// bei `KeepAll` fuehrt Ueberschreitung zu `CapacityExceeded`, bei
    /// `KeepLast` zu LRU-Eviction des aeltesten Samples.
    #[must_use]
    pub fn new_with_kind(kind: HistoryKind, max_samples: usize) -> Self {
        Self {
            changes: BTreeMap::new(),
            kind,
            max_samples,
            evicted_count: 0,
            stats: Arc::new(HistoryCacheStats::default()),
            #[cfg(feature = "inspect")]
            inspect_label: None,
        }
    }

    /// Setzt das Inspect-Endpoint-Label fuer den RTPS-Layer-Tap.
    /// No-op wenn Feature `inspect` aus ist.
    #[cfg(feature = "inspect")]
    pub fn set_inspect_label(&mut self, label: alloc::string::String) {
        self.inspect_label = Some(label);
    }

    /// Geteilter Stats-Handle fuer **lock-freies** Monitoring.
    ///
    /// Konsumenten halten einen `Arc<HistoryCacheStats>` und pollen die
    /// Atomics mit `Acquire`-Ordering. Werte spiegeln immer eine
    /// abgeschlossene Cache-Mutation; cross-field-Konsistenz zwischen
    /// `len` und `max_sn` ist nicht garantiert (Tear-Risiko).
    #[must_use]
    pub fn stats(&self) -> Arc<HistoryCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Synchronisiert die Atomics mit dem aktuellen `BTreeMap`-Zustand.
    /// Wird von allen mutierenden Methoden am Ende aufgerufen.
    fn refresh_stats(&self) {
        self.stats.len.store(self.changes.len(), Ordering::Release);
        self.stats
            .evicted
            .store(self.evicted_count, Ordering::Release);
        let max = self.changes.keys().next_back().copied();
        let min = self.changes.keys().next().copied();
        self.stats
            .max_sn
            .store(encode_sn_atom(max), Ordering::Release);
        self.stats
            .min_sn
            .store(encode_sn_atom(min), Ordering::Release);
    }

    /// Legacy-Konstruktor — erzeugt einen `KeepAll`-Cache mit harter
    /// Kapazitaets-Grenze. Fuer neue Nutzer [`new_with_kind`] bevorzugen.
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self::new_with_kind(HistoryKind::KeepAll, max_samples)
    }

    /// History-Kind dieses Caches.
    #[must_use]
    pub fn kind(&self) -> HistoryKind {
        self.kind
    }

    /// Anzahl per `KeepLast`-Eviction verworfener Samples seit
    /// Start.
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.evicted_count
    }

    /// Fuegt ein Change ein.
    ///
    /// # Errors
    /// - `CapacityExceeded`: nur bei `KeepAll`, Cache voll.
    /// - `DuplicateSequenceNumber`: SN bereits vorhanden.
    /// - `ZeroDepth`: `KeepLast { depth: 0 }`.
    pub fn insert(&mut self, change: CacheChange) -> Result<(), CacheError> {
        if self.changes.contains_key(&change.sequence_number) {
            return Err(CacheError::DuplicateSequenceNumber);
        }
        let cap = self.effective_max_samples()?;
        if self.changes.len() >= cap {
            match self.kind {
                HistoryKind::KeepAll => return Err(CacheError::CapacityExceeded),
                HistoryKind::KeepLast { .. } => {
                    // Aeltestes Sample raus (LRU nach SN-Reihenfolge).
                    if let Some((&oldest, _)) = self.changes.iter().next() {
                        self.changes.remove(&oldest);
                        self.evicted_count = self.evicted_count.saturating_add(1);
                    }
                }
            }
        }
        #[cfg(feature = "inspect")]
        let tap_view = self.inspect_label.as_ref().map(|label| {
            (
                label.clone(),
                change.sequence_number,
                change.payload.to_vec(),
            )
        });
        self.changes.insert(change.sequence_number, change);
        self.refresh_stats();
        #[cfg(feature = "inspect")]
        if let Some((label, sn, payload)) = tap_view {
            dispatch_rtps_tap(&label, sn, payload);
        }
        Ok(())
    }

    /// Effective max-samples = min(max_samples, depth).
    fn effective_max_samples(&self) -> Result<usize, CacheError> {
        match self.kind {
            HistoryKind::KeepAll => Ok(self.max_samples),
            HistoryKind::KeepLast { depth } => {
                if depth == 0 {
                    return Err(CacheError::ZeroDepth);
                }
                Ok(core::cmp::min(depth, self.max_samples))
            }
        }
    }

    /// Holt ein Change per SN.
    #[must_use]
    pub fn get(&self, sn: SequenceNumber) -> Option<&CacheChange> {
        self.changes.get(&sn)
    }

    /// Entfernt alle Changes mit SN ≤ `sn`.
    /// Liefert die Anzahl entfernter Eintraege.
    pub fn remove_up_to(&mut self, sn: SequenceNumber) -> usize {
        let keep = self.changes.split_off(&SequenceNumber(sn.0 + 1));
        let removed = self.changes.len();
        self.changes = keep;
        self.refresh_stats();
        removed
    }

    /// Iteriert in SN-Reihenfolge ueber Changes im Bereich `[lo, hi]`
    /// (beide inklusiv).
    pub fn iter_range(
        &self,
        lo: SequenceNumber,
        hi: SequenceNumber,
    ) -> impl Iterator<Item = &CacheChange> + '_ {
        self.changes.range(lo..=hi).map(|(_, v)| v)
    }

    /// Kleinste SN im Cache.
    #[must_use]
    pub fn min_sn(&self) -> Option<SequenceNumber> {
        self.changes.keys().next().copied()
    }

    /// Groesste SN im Cache.
    #[must_use]
    pub fn max_sn(&self) -> Option<SequenceNumber> {
        self.changes.keys().next_back().copied()
    }

    /// Anzahl Changes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// True wenn keine Changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Maximale Kapazitaet.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_samples
    }
}

// ============================================================================
// LockFreeReadHistoryCache — // ============================================================================

/// `HistoryCache`-Variante mit lock-free Lese-Pfad via RCU/Copy-on-Write.
///
/// **** Reader (z.B. SEDP-Tick, Heartbeat-Bau,
/// Resend-Iteration) sehen einen `Arc`-stabilen Snapshot des
/// `BTreeMap`-Storage; sie greifen ohne weiteren Lock-Touch darauf zu.
/// Writer serialisieren ueber einen internen [`RcuCell`]-Mutex und
/// publizieren copy-on-write.
///
/// # Trade-Offs
///
/// * **Read-Pfad**: 1× Mutex-Acquire fuer Refcount-Inc + 0 weitere
///   Locks. Concurrent Reader sehen denselben Arc; Read-Iteration ist
///   im Wesentlichen Lock-Frei.
/// * **Write-Pfad**: O(n) pro Insert/Remove, weil der `BTreeMap`-
///   Inhalt geklont werden muss. Akzeptabel fuer kleine Caches
///   (<= 1000 Samples). Fuer write-heavy Pfade weiter [`HistoryCache`]
///   nutzen.
/// * **Memory**: aktive Snapshots verbrauchen Speicher bis sie freigegeben
///   werden (Reader-Lebensdauer). Cache-Mutationen invalidieren keine
///   bestehenden Reader-Snapshots.
///
/// # Wann verwenden
///
/// * Discovery-Caches, die haeufig von SEDP-Tick + Match-Loops gelesen,
///   aber nur bei `announce_publication`/`subscription` mutiert werden.
/// * Monitoring-/Tooling-Pfade, die ohne Lock-Touch ueber den Cache
///   iterieren wollen.
///
/// # Wann NICHT verwenden
///
/// * Reliable-Writer-Cache mit hoher Insert-Rate (jede Insert klont
///   den ganzen BTreeMap). Da bleibt [`HistoryCache`] besser.
///
/// Persistente Datenstrukturen (`im::OrdMap`) wuerden den
/// Write-Cost-Aufwand auf O(log n) druecken — das ist die
/// natuerliche Folge-Optimierung wenn diese Variante in Production
/// landet.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct LockFreeReadHistoryCache {
    inner: zerodds_foundation::rcu::RcuCell<LockFreeInner>,
    stats: Arc<HistoryCacheStats>,
}

/// Innerer Zustand des [`LockFreeReadHistoryCache`]. Wird von
/// [`LockFreeReadHistoryCache::snapshot`] als `Arc<LockFreeInner>`
/// nach aussen gegeben — Reader iterieren ueber `changes` direkt.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct LockFreeInner {
    /// Sample-Map keyed by SequenceNumber.
    pub changes: BTreeMap<SequenceNumber, CacheChange>,
    /// History-QoS-Kind.
    pub kind: HistoryKind,
    /// Cap aus QoS.
    pub max_samples: usize,
    /// Eviction-Counter.
    pub evicted_count: u64,
}

#[cfg(feature = "std")]
impl LockFreeInner {
    fn effective_max_samples(&self) -> Result<usize, CacheError> {
        match self.kind {
            HistoryKind::KeepAll => Ok(self.max_samples),
            HistoryKind::KeepLast { depth } => {
                if depth == 0 {
                    return Err(CacheError::ZeroDepth);
                }
                Ok(core::cmp::min(depth, self.max_samples))
            }
        }
    }
}

#[cfg(feature = "std")]
impl LockFreeReadHistoryCache {
    /// Erzeugt eine neue Lock-Free-Read-Cache.
    #[must_use]
    pub fn new_with_kind(kind: HistoryKind, max_samples: usize) -> Self {
        Self {
            inner: zerodds_foundation::rcu::RcuCell::new(LockFreeInner {
                changes: BTreeMap::new(),
                kind,
                max_samples,
                evicted_count: 0,
            }),
            stats: Arc::new(HistoryCacheStats::default()),
        }
    }

    /// Legacy-Konstruktor — `KeepAll`.
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self::new_with_kind(HistoryKind::KeepAll, max_samples)
    }

    /// Lock-free Read-Snapshot von Stats.
    #[must_use]
    pub fn stats(&self) -> Arc<HistoryCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Liefert einen `Arc`-Snapshot des aktuellen Cache-Stand fuer
    /// lock-free-Iteration.
    #[must_use]
    pub fn snapshot(&self) -> Arc<LockFreeInner> {
        self.inner.read()
    }

    /// History-Kind.
    #[must_use]
    pub fn kind(&self) -> HistoryKind {
        self.inner.read().kind
    }

    /// Anzahl per `KeepLast`-Eviction verworfener Samples.
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.stats.evicted.load(Ordering::Acquire)
    }

    /// Anzahl Changes (Acquire-Load des Atomic).
    #[must_use]
    pub fn len(&self) -> usize {
        self.stats.len.load(Ordering::Acquire)
    }

    /// True wenn keine Changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Kleinste SN aus Atom — lock-frei.
    #[must_use]
    pub fn min_sn(&self) -> Option<SequenceNumber> {
        decode_sn_atom(self.stats.min_sn.load(Ordering::Acquire))
    }

    /// Groesste SN aus Atom — lock-frei.
    #[must_use]
    pub fn max_sn(&self) -> Option<SequenceNumber> {
        decode_sn_atom(self.stats.max_sn.load(Ordering::Acquire))
    }

    /// Maximale Kapazitaet.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.read().max_samples
    }

    /// Holt ein Change per SN — geklont (CacheChange ist Arc-payload-
    /// gewrapped, also ein Refcount-Inc).
    #[must_use]
    pub fn get(&self, sn: SequenceNumber) -> Option<CacheChange> {
        self.inner.read().changes.get(&sn).cloned()
    }

    /// Sample-Snapshot im SN-Bereich `[lo, hi]`. Liefert Vec
    /// — wir koennen keine Iter `<'a>` ueber einen Arc-Snapshot
    /// zurueckgeben, wenn der Snapshot nicht referenziert wird.
    #[must_use]
    pub fn iter_range_snapshot(&self, lo: SequenceNumber, hi: SequenceNumber) -> Vec<CacheChange> {
        let snap = self.inner.read();
        snap.changes
            .range(lo..=hi)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Fuegt ein Change ein. Copy-on-Write des BTreeMap.
    ///
    /// # Errors
    /// Wie [`HistoryCache::insert`].
    pub fn insert(&self, change: CacheChange) -> Result<(), CacheError> {
        // Pre-Check: BTreeMap-Klon vermeiden, wenn wir schon wissen dass
        // der Insert fehlschlaegt (Duplicate, ZeroDepth, KeepAll-Full).
        let dup_or_full: Result<(), CacheError> = {
            let snap = self.inner.read();
            if snap.changes.contains_key(&change.sequence_number) {
                Err(CacheError::DuplicateSequenceNumber)
            } else {
                let cap = snap.effective_max_samples()?;
                if snap.changes.len() >= cap {
                    if matches!(snap.kind, HistoryKind::KeepAll) {
                        Err(CacheError::CapacityExceeded)
                    } else {
                        Ok(()) // KeepLast: evict im write-with
                    }
                } else {
                    Ok(())
                }
            }
        };
        dup_or_full?;
        self.inner.modify(|inner| {
            // Eviction-Logik: KeepLast laesst aelteste fallen.
            let cap = match inner.effective_max_samples() {
                Ok(c) => c,
                Err(_) => return,
            };
            if inner.changes.len() >= cap {
                if let HistoryKind::KeepLast { .. } = inner.kind {
                    if let Some((&oldest, _)) = inner.changes.iter().next() {
                        inner.changes.remove(&oldest);
                        inner.evicted_count = inner.evicted_count.saturating_add(1);
                    }
                }
            }
            inner.changes.insert(change.sequence_number, change.clone());
        });
        self.refresh_stats();
        Ok(())
    }

    /// Entfernt alle Changes mit SN ≤ `sn`. Liefert Anzahl entfernter.
    pub fn remove_up_to(&self, sn: SequenceNumber) -> usize {
        let mut removed = 0;
        self.inner.modify(|inner| {
            let keep = inner.changes.split_off(&SequenceNumber(sn.0 + 1));
            removed = inner.changes.len();
            inner.changes = keep;
        });
        self.refresh_stats();
        removed
    }

    fn refresh_stats(&self) {
        let snap = self.inner.read();
        self.stats.len.store(snap.changes.len(), Ordering::Release);
        self.stats
            .evicted
            .store(snap.evicted_count, Ordering::Release);
        let max = snap.changes.keys().next_back().copied();
        let min = snap.changes.keys().next().copied();
        self.stats
            .max_sn
            .store(encode_sn_atom(max), Ordering::Release);
        self.stats
            .min_sn
            .store(encode_sn_atom(min), Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sn(n: i64) -> SequenceNumber {
        SequenceNumber(n)
    }

    fn alive(n: i64) -> CacheChange {
        CacheChange::alive(sn(n), alloc::vec![n as u8])
    }

    #[test]
    fn new_cache_is_empty() {
        let c = HistoryCache::new(10);
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
        assert_eq!(c.min_sn(), None);
        assert_eq!(c.max_sn(), None);
    }

    #[test]
    fn insert_and_get() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).expect("insert");
        c.insert(alive(2)).expect("insert");
        assert_eq!(
            c.get(sn(1)).map(|ch| ch.payload.as_ref().to_vec()),
            Some(alloc::vec![1])
        );
        assert_eq!(c.get(sn(3)), None);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn insert_duplicate_is_err() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).expect("insert");
        assert_eq!(c.insert(alive(1)), Err(CacheError::DuplicateSequenceNumber));
    }

    #[test]
    fn insert_at_capacity_is_err() {
        let mut c = HistoryCache::new(2);
        c.insert(alive(1)).expect("insert");
        c.insert(alive(2)).expect("insert");
        assert_eq!(c.insert(alive(3)), Err(CacheError::CapacityExceeded));
    }

    #[test]
    fn min_max_sn_reflect_content() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(5)).unwrap();
        c.insert(alive(3)).unwrap();
        c.insert(alive(7)).unwrap();
        assert_eq!(c.min_sn(), Some(sn(3)));
        assert_eq!(c.max_sn(), Some(sn(7)));
    }

    #[test]
    fn remove_up_to_inclusive() {
        let mut c = HistoryCache::new(10);
        for i in 1..=5 {
            c.insert(alive(i)).unwrap();
        }
        let removed = c.remove_up_to(sn(3));
        assert_eq!(removed, 3);
        assert_eq!(c.len(), 2);
        assert_eq!(c.min_sn(), Some(sn(4)));
    }

    #[test]
    fn remove_up_to_with_no_matches_is_noop() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(10)).unwrap();
        assert_eq!(c.remove_up_to(sn(5)), 0);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn iter_range_is_ordered() {
        let mut c = HistoryCache::new(10);
        for i in [5, 1, 3, 8, 2] {
            c.insert(alive(i)).unwrap();
        }
        let collected: alloc::vec::Vec<i64> = c
            .iter_range(sn(2), sn(5))
            .map(|ch| ch.sequence_number.0)
            .collect();
        assert_eq!(collected, alloc::vec![2, 3, 5]);
    }

    #[test]
    fn iter_range_empty_when_no_overlap() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).unwrap();
        c.insert(alive(2)).unwrap();
        assert_eq!(c.iter_range(sn(10), sn(20)).count(), 0);
    }

    #[test]
    fn capacity_accessor() {
        let c = HistoryCache::new(42);
        assert_eq!(c.capacity(), 42);
    }

    #[test]
    fn cache_change_alive_constructor() {
        let ch = CacheChange::alive(sn(1), alloc::vec![1, 2, 3]);
        assert_eq!(ch.kind, ChangeKind::Alive);
        assert_eq!(ch.sequence_number, sn(1));
        assert_eq!(ch.payload.as_ref(), &[1, 2, 3][..]);
    }

    // ---- §8.2.1.2 ChangeKind_t alle 4 Spec-Varianten + AliveFiltered ----

    #[test]
    fn change_kind_alive_is_relevant_and_alive() {
        assert!(ChangeKind::Alive.is_relevant());
        assert!(ChangeKind::Alive.is_alive_kind());
    }

    #[test]
    fn change_kind_alive_filtered_is_alive_but_not_relevant() {
        // Spec §8.4.10.5: AliveFiltered ist alive_kind aber !is_relevant.
        assert!(ChangeKind::AliveFiltered.is_alive_kind());
        assert!(!ChangeKind::AliveFiltered.is_relevant());
    }

    #[test]
    fn change_kind_not_alive_kinds_are_not_alive() {
        for k in [
            ChangeKind::NotAliveDisposed,
            ChangeKind::NotAliveUnregistered,
            ChangeKind::NotAliveDisposedUnregistered,
        ] {
            assert!(!k.is_alive_kind(), "{k:?}");
            assert!(k.is_relevant(), "{k:?}");
        }
    }

    #[test]
    fn change_kind_distinct_variants() {
        // Identity-Sanity — alle 5 Varianten sind verschieden.
        let v = [
            ChangeKind::Alive,
            ChangeKind::AliveFiltered,
            ChangeKind::NotAliveDisposed,
            ChangeKind::NotAliveUnregistered,
            ChangeKind::NotAliveDisposedUnregistered,
        ];
        for (i, a) in v.iter().enumerate() {
            for (j, b) in v.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // ========================================================================
    // D.4 Phase A — Atomic Stats / Lock-Free Snapshot Tests
    // ========================================================================

    #[test]
    fn stats_default_is_empty_with_no_sn() {
        let c = HistoryCache::new(10);
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 0);
        assert_eq!(snap.evicted, 0);
        assert_eq!(snap.max_sn, None);
        assert_eq!(snap.min_sn, None);
    }

    #[test]
    fn stats_track_insert_and_remove() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(3)).unwrap();
        c.insert(alive(5)).unwrap();
        c.insert(alive(7)).unwrap();
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 3);
        assert_eq!(snap.min_sn, Some(sn(3)));
        assert_eq!(snap.max_sn, Some(sn(7)));
        assert_eq!(snap.evicted, 0);

        c.remove_up_to(sn(5));
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 1);
        assert_eq!(snap.min_sn, Some(sn(7)));
        assert_eq!(snap.max_sn, Some(sn(7)));
    }

    #[test]
    fn stats_track_keeplast_eviction() {
        let mut c = HistoryCache::new_with_kind(HistoryKind::KeepLast { depth: 2 }, 100);
        c.insert(alive(1)).unwrap();
        c.insert(alive(2)).unwrap();
        c.insert(alive(3)).unwrap(); // evicts alive(1)
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 2);
        assert_eq!(snap.evicted, 1);
        assert_eq!(snap.min_sn, Some(sn(2)));
        assert_eq!(snap.max_sn, Some(sn(3)));
    }

    #[test]
    fn stats_arc_is_shared_across_clones_of_handle() {
        // Mehrere `cache.stats()`-Aufrufe liefern denselben Arc — sodass
        // ein Reader, der den Handle einmalig zieht, alle nachfolgenden
        // Cache-Mutationen sieht.
        let mut c = HistoryCache::new(10);
        let s1 = c.stats();
        let s2 = c.stats();
        assert!(Arc::ptr_eq(&s1, &s2));
        c.insert(alive(1)).unwrap();
        assert_eq!(s1.snapshot().len, 1);
        assert_eq!(s2.snapshot().len, 1);
    }

    #[test]
    fn stats_reader_thread_sees_inserts_concurrently() {
        // Lock-Free-Read aus einem zweiten Thread waehrend der Writer
        // mutiert. Korrektheits-Test fuer die Acquire/Release-Ordering.
        use std::sync::Arc as StdArc;
        use std::sync::Mutex as StdMutex;
        use std::thread;
        use std::time::Duration;

        let cache = StdArc::new(StdMutex::new(HistoryCache::new(2_000)));
        let stats = cache.lock().expect("init lock").stats();

        let writer_cache = StdArc::clone(&cache);
        let writer = thread::spawn(move || {
            for i in 1..=1_000 {
                let mut c = writer_cache.lock().expect("write lock");
                c.insert(alive(i)).expect("insert");
            }
        });

        let reader_stats = StdArc::clone(&stats);
        let reader = thread::spawn(move || {
            // Lese 100x Stats, ohne den Writer-Lock zu nehmen.
            for _ in 0..100 {
                let snap = reader_stats.snapshot();
                // len darf nur monoton wachsen waehrend Writer laeuft.
                assert!(snap.len <= 1_000);
                if let Some(max) = snap.max_sn {
                    assert!(max.0 >= 1 && max.0 <= 1_000);
                }
                thread::sleep(Duration::from_micros(50));
            }
        });

        writer.join().expect("writer joined");
        reader.join().expect("reader joined");

        let final_snap = stats.snapshot();
        assert_eq!(final_snap.len, 1_000);
        assert_eq!(final_snap.max_sn, Some(sn(1_000)));
        assert_eq!(final_snap.min_sn, Some(sn(1)));
    }

    #[test]
    fn clone_creates_independent_stats_handles() {
        // Cache.clone() darf nicht den Stats-Arc des Originals teilen,
        // sonst wuerden Mutationen am Klon das Original verfaelschen.
        let mut a = HistoryCache::new(10);
        a.insert(alive(1)).unwrap();
        let b = a.clone();
        assert!(!Arc::ptr_eq(&a.stats(), &b.stats()));
        assert_eq!(a.stats().snapshot().len, 1);
        assert_eq!(b.stats().snapshot().len, 1);

        let mut a_mut = a;
        a_mut.insert(alive(2)).unwrap();
        assert_eq!(a_mut.stats().snapshot().len, 2);
        assert_eq!(b.stats().snapshot().len, 1, "clone unaffected");
    }

    // ========================================================================
    // D.4 Phase C — LockFreeReadHistoryCache Tests
    // ========================================================================

    #[cfg(feature = "std")]
    mod lock_free_tests {
        use super::*;

        #[test]
        fn lock_free_new_is_empty() {
            let c = LockFreeReadHistoryCache::new(10);
            assert_eq!(c.len(), 0);
            assert!(c.is_empty());
            assert_eq!(c.min_sn(), None);
            assert_eq!(c.max_sn(), None);
        }

        #[test]
        fn lock_free_insert_and_get() {
            let c = LockFreeReadHistoryCache::new(10);
            // Insert ohne &mut self — reine Interior-Mutability.
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            assert_eq!(
                c.get(sn(1)).map(|ch| ch.payload.as_ref().to_vec()),
                Some(alloc::vec![1])
            );
            assert_eq!(c.get(sn(3)), None);
            assert_eq!(c.len(), 2);
        }

        #[test]
        fn lock_free_min_max_lock_free_loads() {
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(5)).unwrap();
            c.insert(alive(3)).unwrap();
            c.insert(alive(7)).unwrap();
            assert_eq!(c.min_sn(), Some(sn(3)));
            assert_eq!(c.max_sn(), Some(sn(7)));
        }

        #[test]
        fn lock_free_keeplast_evicts_oldest() {
            let c =
                LockFreeReadHistoryCache::new_with_kind(HistoryKind::KeepLast { depth: 2 }, 100);
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            c.insert(alive(3)).unwrap(); // evicts 1
            assert_eq!(c.len(), 2);
            assert_eq!(c.min_sn(), Some(sn(2)));
            assert_eq!(c.max_sn(), Some(sn(3)));
            assert_eq!(c.evicted_count(), 1);
        }

        #[test]
        fn lock_free_keepall_full_rejects() {
            let c = LockFreeReadHistoryCache::new(2);
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            assert_eq!(c.insert(alive(3)), Err(CacheError::CapacityExceeded));
        }

        #[test]
        fn lock_free_duplicate_sn_rejected() {
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(1)).unwrap();
            assert_eq!(c.insert(alive(1)), Err(CacheError::DuplicateSequenceNumber));
        }

        #[test]
        fn lock_free_remove_up_to() {
            let c = LockFreeReadHistoryCache::new(10);
            for i in 1..=5 {
                c.insert(alive(i)).unwrap();
            }
            let removed = c.remove_up_to(sn(3));
            assert_eq!(removed, 3);
            assert_eq!(c.len(), 2);
            assert_eq!(c.min_sn(), Some(sn(4)));
        }

        #[test]
        fn lock_free_iter_range_snapshot() {
            let c = LockFreeReadHistoryCache::new(10);
            for i in 1..=5 {
                c.insert(alive(i)).unwrap();
            }
            let mid: alloc::vec::Vec<_> = c
                .iter_range_snapshot(sn(2), sn(4))
                .iter()
                .map(|ch| ch.sequence_number)
                .collect();
            assert_eq!(mid, alloc::vec![sn(2), sn(3), sn(4)]);
        }

        #[test]
        fn lock_free_snapshot_outlives_writes() {
            // Snapshot-API-Garantie: ein Reader-Arc bleibt unveraendert,
            // auch wenn der Writer den Cache spaeter mutiert.
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(1)).unwrap();
            let snap = c.snapshot();
            assert_eq!(snap.changes.len(), 1);

            c.insert(alive(2)).unwrap();
            c.insert(alive(3)).unwrap();
            // Original-Snapshot: immer noch nur SN 1.
            assert_eq!(snap.changes.len(), 1);
            assert!(snap.changes.contains_key(&sn(1)));
            // Live-Cache: 3 Eintraege.
            assert_eq!(c.len(), 3);
        }

        #[test]
        fn lock_free_concurrent_readers_writers_smoke() {
            use std::sync::Arc as StdArc;
            use std::thread;

            let cache: StdArc<LockFreeReadHistoryCache> =
                StdArc::new(LockFreeReadHistoryCache::new(2_000));
            let cache_w = StdArc::clone(&cache);
            let writer = thread::spawn(move || {
                for i in 1..=500 {
                    cache_w.insert(alive(i)).expect("insert");
                }
            });

            let cache_r = StdArc::clone(&cache);
            let reader = thread::spawn(move || {
                for _ in 0..200 {
                    let snap = cache_r.snapshot();
                    // Snapshot ist intern konsistent: changes.len matches
                    // den Range zwischen min und max.
                    if let (Some(min), Some(max)) = (
                        snap.changes.keys().next().copied(),
                        snap.changes.keys().next_back().copied(),
                    ) {
                        let inferred_count = (max.0 - min.0 + 1) as usize;
                        assert!(
                            snap.changes.len() <= inferred_count,
                            "snapshot inkonsistent"
                        );
                    }
                }
            });

            writer.join().expect("writer joined");
            reader.join().expect("reader joined");

            assert_eq!(cache.len(), 500);
            assert_eq!(cache.max_sn(), Some(sn(500)));
        }
    }
}
