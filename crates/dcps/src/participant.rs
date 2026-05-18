// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DomainParticipant — die "Wurzel"-Entity eines DDS-Programms.
//!
//! Spec-Referenz: OMG DDS 1.4 §2.2.2.2 `DomainParticipant`.
//!
//! Jedes DDS-Programm oeffnet typischerweise genau einen
//! `DomainParticipant` pro Domain-Id. Der Participant:
//!
//! - haelt die GUID-Prefix (12-Byte, leite ID fuer alle internen Endpoints),
//! - registriert sich via SPDP (Simple Participant Discovery Protocol),
//! - betreibt SEDP (Simple Endpoint Discovery Protocol) fuer
//!   Topic-/Writer-/Reader-Matching,
//! - ist Factory fuer Publisher, Subscriber, Topic.
//!
//! # Modi
//!
//! - **Live-Mode** (`new_with_runtime`, gerufen aus
//!   `DomainParticipantFactory::create_participant`): bindet UDP-
//!   Sockets, spawnt SPDP-/SEDP-/WLP-Threads, fuehrt das volle
//!   Discovery-Protokoll und die TypeLookup-Service-Endpoints
//!   (XTypes 1.3 §7.6.3.3.4).
//! - **Offline-Mode** (`new`, gerufen aus
//!   `DomainParticipantFactory::create_participant_offline`): keine
//!   Sockets, keine Threads. Topic-Registry, QoS-Negotiation und
//!   Loopback-Pfad fuer Unit-Tests sind verfuegbar.
//!
//! Topic-Registry: gleicher Name + gleicher Typ ergibt denselben
//! Topic-Handle (DDS 1.4 §2.2.2.2.1.10 `find_topic`).

extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::builtin_subscriber::BuiltinSubscriber;
use crate::builtin_topics::{ParticipantBuiltinTopicData, TopicBuiltinTopicData};
use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
use crate::instance_handle::InstanceHandle;
use crate::listener::ArcDomainParticipantListener;
use crate::publisher::Publisher;
use crate::qos::{DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos};
use crate::subscriber::Subscriber;
use crate::topic::{
    ContentFilteredTopic, Topic, TopicDescription, TopicDescriptionHandle, TopicInner,
};

#[cfg(feature = "std")]
use crate::runtime::{DcpsRuntime, RuntimeConfig};

/// Domain-Id-Typ (Spec: `DomainId_t` = long, also i32).
pub type DomainId = i32;

/// Shared Ignore-List-Filter eines `DomainParticipant`s. Wird vom
/// Participant gehalten **und** vom `DcpsRuntime`-Discovery-Hook
/// konsultiert (Klon des `Arc`). Spec-Referenz: DDS DCPS 1.4
/// §2.2.2.2.1.14-17 `ignore_participant/topic/publication/subscription`.
///
/// Per spec sind die Listen **monoton wachsend**: ein Handle kann
/// dazukommen, aber nie wieder entfernt werden. Daher reicht
/// `BTreeSet<InstanceHandle>` und keine Generation-Counter.
#[derive(Debug, Default)]
#[cfg(feature = "std")]
pub(crate) struct IgnoreFilterInner {
    pub(crate) participants: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) topics: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) publications: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) subscriptions: Mutex<BTreeSet<InstanceHandle>>,
}

/// Klonbarer Filter-Handle (Arc-bumps sind billig). Discovery-Hook
/// darf hier zwischendurch reinpoken, ohne lock-cycles auf den
/// gesamten ParticipantInner zu erzwingen.
#[derive(Clone, Debug, Default)]
#[cfg(feature = "std")]
pub struct IgnoreFilter {
    pub(crate) inner: Arc<IgnoreFilterInner>,
}

#[cfg(feature = "std")]
impl IgnoreFilter {
    /// Pruefe, ob ein Participant-Handle ignoriert ist.
    #[must_use]
    pub fn is_participant_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .participants
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Pruefe, ob ein Topic-Handle ignoriert ist.
    #[must_use]
    pub fn is_topic_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .topics
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Pruefe, ob ein Publication-Handle ignoriert ist.
    #[must_use]
    pub fn is_publication_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .publications
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Pruefe, ob ein Subscription-Handle ignoriert ist.
    #[must_use]
    pub fn is_subscription_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .subscriptions
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }
}

/// Zufaellig erzeugter 12-Byte-Participant-Prefix.
///
/// Schema (Spec `zerodds-zero-copy-1.0` §6 Welle 4):
/// - `bytes[0..4]`: Host-Id (FNV1a-Hash der `gethostname`-Ausgabe).
///   Zwei Participants auf derselben Maschine tragen denselben
///   Host-Id-Prefix → Discovery erkennt Same-Host-Match und kann
///   einen Zero-Copy-SHM-Pfad aktivieren.
/// - `bytes[4..8]`: Process-Id (LE).
/// - `bytes[8..12]`: Timestamp + Atomic-Counter, damit Re-Start des
///   gleichen Prozesses oder mehrere Participants im selben Prozess
///   unterschiedliche Prefixes bekommen.
///
/// Cross-Host-Hash-Kollision (4-Byte-FNV1a) ist theoretisch moeglich
/// aber praktisch vernachlaessigbar; ein false-positive Same-Host-
/// Match wuerde nur das SHM-Setup scheitern lassen und automatisch
/// auf den UDP-Pfad zurueckfallen.
#[cfg(feature = "std")]
fn random_guid_prefix() -> zerodds_rtps::wire_types::GuidPrefix {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let host_id = host_id_bytes();
    let pid = std::process::id();
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = [0u8; 12];
    bytes[0..4].copy_from_slice(&host_id);
    bytes[4..8].copy_from_slice(&pid.to_le_bytes());
    bytes[8..12].copy_from_slice(&(t as u32).to_le_bytes());
    bytes[11] = bytes[11].wrapping_add(c as u8);
    zerodds_rtps::wire_types::GuidPrefix::from_bytes(bytes)
}

/// Deterministischer 4-Byte-Host-Identifier auf Basis von
/// `gethostname`. Cached pro Prozess via `OnceLock`.
///
/// FNV1a-32 reicht: wir brauchen Identitaet (same-host yes/no), nicht
/// kryptographische Sicherheit. Falls `gethostname` fehlschlaegt
/// (CI-Container ohne Hostname), fallen wir auf einen prozesslokalen
/// Random-Wert zurueck — dann tritt kein false-positive Same-Host-
/// Match mit Peers auf derselben Maschine auf, was sicher ist (nur
/// die SHM-Optimierung wird verpasst).
#[cfg(feature = "std")]
fn host_id_bytes() -> [u8; 4] {
    use std::sync::OnceLock;
    static HOST_ID: OnceLock<[u8; 4]> = OnceLock::new();
    *HOST_ID.get_or_init(|| {
        let hostname = std::env::var("HOSTNAME")
            .ok()
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .or_else(read_etc_hostname);
        let h = match hostname {
            Some(s) if !s.is_empty() => fnv1a_32(s.as_bytes()),
            _ => {
                // Fallback: prozesslokaler Random-Wert. Dann hat dieser
                // Process einen einzigartigen "host" und macht keine
                // false-positive Same-Host-Optimierung.
                let pid = std::process::id();
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u32)
                    .unwrap_or(0);
                pid.wrapping_mul(0x9E37_79B1).wrapping_add(t)
            }
        };
        h.to_le_bytes()
    })
}

#[cfg(feature = "std")]
fn read_etc_hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
}

#[cfg(feature = "std")]
fn fnv1a_32(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Der Participant.
#[derive(Clone)]
pub struct DomainParticipant {
    inner: Arc<ParticipantInner>,
}

impl core::fmt::Debug for DomainParticipant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DomainParticipant")
            .field("domain_id", &self.inner.domain_id)
            .finish_non_exhaustive()
    }
}

pub(crate) struct ParticipantInner {
    pub(crate) domain_id: DomainId,
    pub(crate) qos: Mutex<DomainParticipantQos>,
    /// Entity-Lifecycle (DCPS §2.2.2.1).
    pub(crate) entity_state: Arc<crate::entity::EntityState>,
    /// Topic-Registry (Name → TopicInner). Wiederholte
    /// `create_topic` mit gleichem Namen + Typ liefern denselben
    /// Handle; mit anderem Typ → `InconsistentPolicy`-Error.
    topics: Mutex<BTreeMap<String, Arc<TopicInner>>>,
    /// Runtime-Handle mit UDP-Sockets + Discovery-Threads. `None`
    /// wenn der Participant im offline-Modus erzeugt wurde (Tests
    /// die kein Netzwerk wollen).
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// Vorinstallierter Builtin-Subscriber (DDS 1.4 §2.2.2.2.1.7).
    /// Genau einer pro Participant. Die Sinks werden bei
    /// Konstruktion in den Runtime-Discovery-Hook eingehaengt.
    pub(crate) builtin_subscriber: Arc<BuiltinSubscriber>,
    /// Ignore-Filter (Spec §2.2.2.2.1.14-17). Klon liegt in der
    /// Runtime und wird vom Discovery-Hot-Path gegengeprueft, damit
    /// SPDP/SEDP-Samples nach `ignore_*` nicht mehr in die Builtin-
    /// Reader fallen.
    #[cfg(feature = "std")]
    pub(crate) ignore_filter: IgnoreFilter,
    /// Lokale Publisher-Registry (fuer `delete_contained_entities` +
    /// `contains_entity` per Spec §2.2.2.2.1.10). Wir tracken die
    /// `InstanceHandle` jedes mit `create_publisher` erzeugten
    /// Publishers; `delete_contained_entities` cleart die Liste.
    /// Echte Drop-Semantik der einzelnen Publisher passiert per
    /// `Arc`-Refcount, sobald der User-Handle fallengelassen wird
    ///.
    publishers: Mutex<Vec<InstanceHandle>>,
    /// Analog zu `publishers`.
    subscribers: Mutex<Vec<InstanceHandle>>,
    /// Aggregat aller DataWriter-Handles aller Publisher dieses
    /// Participants (Spec §2.2.2.2.1.10 contains_entity rekursiv).
    /// Pub/Sub registrieren neue Children via Weak-Backref.
    pub(crate) datawriters: Mutex<Vec<InstanceHandle>>,
    /// Aggregat aller DataReader-Handles aller Subscriber dieses
    /// Participants.
    pub(crate) datareaders: Mutex<Vec<InstanceHandle>>,
    /// optionaler [`ArcDomainParticipantListener`] +
    /// [`StatusMask`]. Bubble-Up-Target fuer alle Children, deren
    /// engerer Listener das Status-Bit nicht abdeckt.
    pub(crate) listener: Mutex<Option<(ArcDomainParticipantListener, StatusMask)>>,
    /// Built-in DynamicType-Registry. Wird in `new()`/
    /// `new_with_runtime()` automatisch mit den 4 Spec-§7.6.5-Built-in-
    /// Types befuellt (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`,
    /// `DDS::KeyedBytes`). Ueber [`DomainParticipant::find_builtin_type`]
    /// abrufbar.
    #[cfg(feature = "std")]
    pub(crate) type_registry: Mutex<BTreeMap<String, zerodds_types::dynamic::DynamicType>>,
    /// TypeLookup-Client-State pro Participant. Pending
    /// Get-Types-Requests werden hier gequeued; Backoff via
    /// `last_attempt_per_hash` damit unbekannte TypeIDs nicht jeden
    /// Tick re-queryt werden.
    #[cfg(feature = "std")]
    pub(crate) type_lookup: Mutex<TypeLookupState>,
}

/// TypeLookup-Client-State pro Participant. Tracked Pending-
/// Requests + Backoff-Timer + Retry-Count pro unbekanntem TypeID-Hash.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct TypeLookupState {
    /// Pro TypeID: (last_attempt_instant, retry_count).
    pub attempts: BTreeMap<zerodds_types::EquivalenceHash, (std::time::Instant, u32)>,
    /// Optional Sink fuer outgoing TypeLookup-Requests (Test-Hook).
    /// Production-Pfad waere ein Reliable-Writer auf dem
    /// `TL_SVC_REQ_WRITER`-Endpoint; bis dahin queued der Sink
    /// (Test-Mode) oder bleibt None (Live-Mode = no-op).
    pub outgoing: Vec<(zerodds_types::EquivalenceHash, u64)>,
}

#[cfg(feature = "std")]
impl TypeLookupState {
    /// Backoff-Periode (5s) zwischen Wiederholungen.
    pub const BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);
    /// Maximale Versuche pro unbekanntem TypeID.
    pub const MAX_ATTEMPTS: u32 = 3;
}

impl DomainParticipant {
    /// Offline-Konstruktor ohne Runtime — fuer Skeleton-Tests.
    /// Produktions-Code geht durch `DomainParticipantFactory::
    /// create_participant` das automatisch eine Runtime startet.
    pub(crate) fn new(domain_id: DomainId, qos: DomainParticipantQos) -> Self {
        let builtin = Arc::new(BuiltinSubscriber::new());
        let participant = Self {
            inner: Arc::new(ParticipantInner {
                domain_id,
                qos: Mutex::new(qos),
                entity_state: crate::entity::EntityState::new(),
                topics: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                runtime: None,
                builtin_subscriber: builtin,
                #[cfg(feature = "std")]
                ignore_filter: IgnoreFilter::default(),
                publishers: Mutex::new(Vec::new()),
                subscribers: Mutex::new(Vec::new()),
                datawriters: Mutex::new(Vec::new()),
                datareaders: Mutex::new(Vec::new()),
                listener: Mutex::new(None),
                #[cfg(feature = "std")]
                type_registry: Mutex::new(BTreeMap::new()),
                #[cfg(feature = "std")]
                type_lookup: Mutex::new(TypeLookupState::default()),
            }),
        };
        // Auto-Register der 4 Spec-§7.6.5-Built-in-Types.
        #[cfg(feature = "std")]
        participant.register_builtin_types();
        participant
    }

    /// Konstruktor mit live Runtime (UDP + Discovery). Gibt
    /// `TransportError` zurueck, wenn Socket-Bind scheitert.
    ///
    /// # Errors
    /// [`DdsError::TransportError`] bei Bind-Problemen.
    #[cfg(feature = "std")]
    pub(crate) fn new_with_runtime(
        domain_id: DomainId,
        qos: DomainParticipantQos,
        config: RuntimeConfig,
    ) -> Result<Self> {
        let runtime = DcpsRuntime::start(domain_id, random_guid_prefix(), config)?;
        let builtin = Arc::new(BuiltinSubscriber::new());
        // Discovery-Hook verdrahten: Runtime pusht ab jetzt SPDP/SEDP-
        // Events in die 4 Builtin-Reader.
        runtime.attach_builtin_sinks(builtin.sinks());
        //  shared Ignore-Filter mit der Runtime teilen, damit der
        // Discovery-Hot-Path (handle_spdp_datagram +
        // push_sedp_events_to_builtin_readers) die Listen konsultieren
        // kann.
        let ignore_filter = IgnoreFilter::default();
        runtime.attach_ignore_filter(ignore_filter.clone());
        let participant = Self {
            inner: Arc::new(ParticipantInner {
                domain_id,
                qos: Mutex::new(qos),
                entity_state: crate::entity::EntityState::new(),
                topics: Mutex::new(BTreeMap::new()),
                runtime: Some(runtime),
                builtin_subscriber: builtin,
                ignore_filter,
                publishers: Mutex::new(Vec::new()),
                subscribers: Mutex::new(Vec::new()),
                datawriters: Mutex::new(Vec::new()),
                datareaders: Mutex::new(Vec::new()),
                listener: Mutex::new(None),
                type_registry: Mutex::new(BTreeMap::new()),
                type_lookup: Mutex::new(TypeLookupState::default()),
            }),
        };
        // Auto-Register der 4 Spec-§7.6.5-Built-in-Types.
        participant.register_builtin_types();
        Ok(participant)
    }

    /// Interner Zugriff auf die Runtime — von Publisher/Subscriber
    /// verwendet, um DataWriter/Reader anzulegen. `None` wenn der
    /// Participant im offline-Modus ist.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn runtime(&self) -> Option<&Arc<DcpsRuntime>> {
        self.inner.runtime.as_ref()
    }

    /// Domain-Id.
    #[must_use]
    pub fn domain_id(&self) -> DomainId {
        self.inner.domain_id
    }

    /// Liefert eine Kopie der DomainParticipantQos (Spec §2.2.2.2.1.4
    /// `get_qos`).
    #[must_use]
    pub fn qos(&self) -> DomainParticipantQos {
        self.inner.qos.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Setzt die DomainParticipantQos (Spec §2.2.2.2.1.3 `set_qos`).
    ///
    /// # Errors
    /// Aktuell keine — die Methode liefert `Ok(())` immer. Spec laesst
    /// `IMMUTABLE_POLICY` zu, was wir aber nicht aktiv produzieren
    /// (alle Policies sind im RC1 mutable).
    pub fn set_qos(&self, qos: DomainParticipantQos) -> Result<()> {
        if let Ok(mut g) = self.inner.qos.lock() {
            *g = qos;
        }
        Ok(())
    }

    /// Registriert die 4 Spec-§7.6.5-Built-in-Types
    /// (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`, `DDS::KeyedBytes`)
    /// im lokalen TypeRegistry. Idempotent — doppelter Aufruf ueber-
    /// schreibt die Eintraege deterministisch.
    ///
    /// Wird automatisch aus `new()`/`new_with_runtime()` aufgerufen,
    /// kann aber auch nach einem `unregister_builtin_types()`-Disable
    /// erneut aufgerufen werden.
    #[cfg(feature = "std")]
    pub fn register_builtin_types(&self) {
        if let Ok(types) = zerodds_types::dynamic::all_builtin_types() {
            if let Ok(mut reg) = self.inner.type_registry.lock() {
                for (name, t) in types {
                    reg.insert(name, t);
                }
            }
        }
    }

    /// Loescht alle registrierten Built-in-Types. Wird heute
    /// nicht von Default-Pfaden gerufen — Test-Hilfsfunktion fuer
    /// Disable-Flag-Tests.
    #[cfg(feature = "std")]
    pub fn unregister_builtin_types(&self) {
        if let Ok(mut reg) = self.inner.type_registry.lock() {
            reg.retain(|name, _| !zerodds_types::dynamic::is_builtin_type_name(name));
        }
    }

    /// Lookup eines Built-in-Types via Spec-Name (Spec §7.6.5).
    /// Gibt `Some(DynamicType)` zurueck wenn der Name bekannt ist
    /// (registriert via `register_builtin_types`).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn find_builtin_type(&self, name: &str) -> Option<zerodds_types::dynamic::DynamicType> {
        self.inner
            .type_registry
            .lock()
            .ok()
            .and_then(|reg| reg.get(name).cloned())
    }

    /// Anzahl registrierter Built-in-Types. Nach `new()` == 4.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn registered_type_count(&self) -> usize {
        self.inner
            .type_registry
            .lock()
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Versucht einen TypeLookup-Request fuer einen unbekannten
    /// `EquivalenceHash` zu queuen. Beachtet Backoff (5s zwischen
    /// Versuchen) und maximal 3 Wiederholungen pro Hash.
    ///
    /// Returns: `true` wenn der Request gequeued wurde, `false` bei
    /// Backoff-Suppression oder Max-Attempts.
    #[cfg(feature = "std")]
    pub fn enqueue_type_lookup(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        let mut state = match self.inner.type_lookup.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let now = std::time::Instant::now();
        if let Some((last, retries)) = state.attempts.get(&hash).copied() {
            if retries >= TypeLookupState::MAX_ATTEMPTS {
                return false;
            }
            if now.duration_since(last) < TypeLookupState::BACKOFF {
                return false;
            }
            state
                .attempts
                .insert(hash, (now, retries.saturating_add(1)));
        } else {
            state.attempts.insert(hash, (now, 1));
        }
        // Naechste Sequence-Number fuer den Request.
        let seq = state.outgoing.len() as u64 + 1;
        state.outgoing.push((hash, seq));
        true
    }

    /// Drainet die queued TypeLookup-Requests. Liefert
    /// `Vec<(hash, seq)>`. In Production-Umgebung wuerde der Caller
    /// die Hashes via TypeLookupClient + Reliable-Writer auf den
    /// `TL_SVC_REQ_WRITER`-Endpoint senden.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn drain_type_lookup_requests(&self) -> Vec<(zerodds_types::EquivalenceHash, u64)> {
        self.inner
            .type_lookup
            .lock()
            .map(|mut s| core::mem::take(&mut s.outgoing))
            .unwrap_or_default()
    }

    /// Empfaengt ein TypeLookup-Reply (TypeObjects pro Hash).
    /// Registriert die TypeObjects in einem internen TypeRegistry-
    /// Spiegel — danach kann ein gestoppter QoS-Match retried werden.
    ///
    /// Anzahl erfolgreich registrierter Typen wird zurueckgegeben.
    #[cfg(feature = "std")]
    pub fn ingest_type_lookup_reply(
        &self,
        types: Vec<(
            zerodds_types::EquivalenceHash,
            zerodds_types::MinimalTypeObject,
        )>,
    ) -> usize {
        let mut count = 0;
        if let Ok(mut state) = self.inner.type_lookup.lock() {
            for (hash, _t) in &types {
                state.attempts.remove(hash);
                count += 1;
            }
        }
        // Clippy-bait avoidance: types-Vec wird hier konsumiert, der
        // eigentliche TypeRegistry-Insert kann der Caller machen
        // (z.B. via shared TypeLookupServer.registry).
        let _ = types;
        count
    }

    /// SEDP-Discovery-Hook: prueft eine eingehende
    /// `PublicationBuiltinTopicData` auf Type-Hashes, die lokal nicht
    /// aufloesbar sind. Bei Bedarf wird ein TypeLookup-Request via
    /// `enqueue_type_lookup` gequeued.
    ///
    /// Der RPC-Pfad ist via `DcpsRuntime::send_type_lookup_request`
    /// auf den TL_SVC_REQ_*-Endpoints (XTypes 1.3 §7.6.3.3.4) live;
    /// diese Methode entscheidet pro Hash, ob ein Re-Request lohnt
    /// (lokale Registry-Lookup + Backoff-Tracking).
    ///
    /// Returns: Anzahl gequeued unbekannter Hashes (max 2 — minimal +
    /// complete).
    #[cfg(feature = "std")]
    pub fn on_remote_publication_discovered(&self, type_information_blob: Option<&[u8]>) -> usize {
        self.on_remote_type_information(type_information_blob)
    }

    /// SEDP-Discovery-Hook fuer
    /// `SubscriptionBuiltinTopicData`. Symmetrisch zu
    /// `on_remote_publication_discovered`.
    #[cfg(feature = "std")]
    pub fn on_remote_subscription_discovered(&self, type_information_blob: Option<&[u8]>) -> usize {
        self.on_remote_type_information(type_information_blob)
    }

    #[cfg(feature = "std")]
    fn on_remote_type_information(&self, blob: Option<&[u8]>) -> usize {
        let Some(bytes) = blob else {
            return 0;
        };
        let Ok(ti) = zerodds_types::type_information::TypeInformation::from_bytes_le(bytes) else {
            return 0;
        };
        let mut queued = 0;
        // Minimal-Hash pruefen.
        if let Some(hash) = extract_equivalence_hash(&ti.minimal.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        // Complete-Hash pruefen (falls vorhanden).
        if let Some(hash) = extract_equivalence_hash(&ti.complete.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        queued
    }

    /// Internal helper — true wenn der Hash bereits im lokalen
    /// `TypeLookupServer.registry` aufloesbar ist (entweder via
    /// `register_type_object` lokal eingespeist oder via vorherigen
    /// `getTypes`-Reply-Ingest gefuellt). Verhindert dass wir fuer
    /// Hashes, die wir bereits kennen, redundante Lookup-Requests
    /// rausgeben.
    #[cfg(feature = "std")]
    fn has_type_for_hash(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return false;
        };
        let Ok(server) = rt.type_lookup_server.lock() else {
            return false;
        };
        server.registry.get_minimal(&hash).is_some()
            || server.registry.get_complete(&hash).is_some()
    }

    /// True wenn fuer den Hash bereits MAX_ATTEMPTS erreicht.
    /// Wird vom Match-Re-Try-Pfad konsultiert: spaeter aufgeben statt
    /// endlos zu pollen.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn type_lookup_exhausted(&self, hash: zerodds_types::EquivalenceHash) -> bool {
        self.inner
            .type_lookup
            .lock()
            .ok()
            .and_then(|s| s.attempts.get(&hash).map(|(_, n)| *n))
            .unwrap_or(0)
            >= TypeLookupState::MAX_ATTEMPTS
    }

    /// Erzeugt einen typed Topic-Handle. Wiederholte Aufrufe mit
    /// gleichem Namen + Typ liefern denselben Handle (Ref-geteilt).
    ///
    /// # Errors
    /// - `InconsistentPolicy` wenn ein Topic mit diesem Namen
    ///   bereits unter anderem Typ registriert ist.
    /// - `BadParameter` bei leerem Namen.
    pub fn create_topic<T: DdsType>(&self, name: &str, qos: TopicQos) -> Result<Topic<T>> {
        if name.is_empty() {
            return Err(DdsError::BadParameter { what: "topic name" });
        }
        let mut topics = self
            .inner
            .topics
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "topic registry poisoned",
            })?;
        if let Some(existing) = topics.get(name) {
            if existing.type_name != T::TYPE_NAME {
                // Inconsistent-Topic-Detection. Bumpt den
                // Counter auf dem existierenden Topic — beim
                // naechsten `inconsistent_topic_status()`-Read wird
                // der Listener via Bubble-Up gefeuert.
                #[cfg(feature = "std")]
                existing
                    .inconsistent_topic_count
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                return Err(DdsError::InconsistentPolicy {
                    what: "topic name reused with different type",
                });
            }
            // Gleicher Typ → shared handle.
            return Ok(reconstruct_topic::<T>(existing.clone(), self.clone()));
        }
        let topic = Topic::<T>::new(name.into(), qos, self.clone());
        topics.insert(name.into(), topic_inner(&topic));
        Ok(topic)
    }

    /// Sofortiger lokaler Lookup eines Topics nach Name — gibt `None`
    /// zurueck, wenn kein lokales `create_topic` mit diesem Namen
    /// erfolgt ist. **Macht keinen Discovery-Wait** (das ist
    /// `find_topic`). Spec-Referenz: OMG DDS 1.4 §2.2.2.2.1.12
    /// "lookup_topicdescription".
    #[must_use]
    pub fn lookup_topicdescription(&self, name: &str) -> Option<TopicDescriptionHandle> {
        let topics = self.inner.topics.lock().ok()?;
        let inner = topics.get(name)?;
        Some(TopicDescriptionHandle::new(
            inner.name.clone(),
            String::from(inner.type_name),
            self.clone(),
        ))
    }

    /// Wartet bis ein Topic mit dem gegebenen Namen via Discovery
    /// (SEDP-Publication oder -Subscription) sichtbar ist — oder bis
    /// `timeout` abgelaufen ist. Spec-Referenz: OMG DDS 1.4
    /// §2.2.2.2.1.11 `find_topic`.
    ///
    /// Returns:
    /// - `Ok(handle)` mit Name + Type-Name + Participant, falls
    ///   waehrend `timeout` ein passendes SEDP-Endpoint sichtbar
    ///   wurde. Lokale Topics zaehlen ebenfalls (keine Pflicht zu
    ///   warten, wenn `create_topic` schon lief).
    /// - `Err(Timeout)` wenn `timeout` abgelaufen ist.
    ///
    /// # Errors
    /// - `DdsError::Timeout` wenn `timeout` ohne Discovery-Match
    ///   abgelaufen ist.
    /// - `DdsError::BadParameter` bei leerem Namen.
    #[cfg(feature = "std")]
    pub fn find_topic(
        &self,
        name: &str,
        timeout: core::time::Duration,
    ) -> Result<TopicDescriptionHandle> {
        if name.is_empty() {
            return Err(DdsError::BadParameter { what: "topic name" });
        }
        let deadline = std::time::Instant::now() + timeout;
        // Sofort lokal pruefen — vermeidet busy-wait wenn das Topic
        // bereits via create_topic lokal angelegt ist.
        if let Some(h) = self.lookup_topicdescription(name) {
            return Ok(h);
        }
        // Poll-Loop ueber den SEDP-Cache. Spec laesst die Strategie
        // offen; Cyclone-DDS pollt ebenfalls.
        let poll = core::time::Duration::from_millis(20);
        loop {
            if let Some(handle) = self.find_topic_in_sedp(name) {
                return Ok(handle);
            }
            if std::time::Instant::now() >= deadline {
                return Err(DdsError::Timeout);
            }
            std::thread::sleep(poll);
        }
    }

    /// Helper: schaut im SEDP-Cache nach, ob ein remote Endpoint
    /// (Publication oder Subscription) ein Topic mit dem Namen
    /// announciert hat. Liefert das erste Match (Name + Type-Name).
    #[cfg(feature = "std")]
    fn find_topic_in_sedp(&self, name: &str) -> Option<TopicDescriptionHandle> {
        let rt = self.inner.runtime.as_ref()?;
        let sedp = rt.sedp.lock().ok()?;
        // Publications zuerst pruefen.
        for p in sedp.cache().publications() {
            if p.data.topic_name == name {
                return Some(TopicDescriptionHandle::new(
                    p.data.topic_name.clone(),
                    p.data.type_name.clone(),
                    self.clone(),
                ));
            }
        }
        for s in sedp.cache().subscriptions() {
            if s.data.topic_name == name {
                return Some(TopicDescriptionHandle::new(
                    s.data.topic_name.clone(),
                    s.data.type_name.clone(),
                    self.clone(),
                ));
            }
        }
        None
    }

    /// Erzeugt ein `ContentFilteredTopic` als Subset eines bereits
    /// vorhandenen `Topic<T>`. Spec-Referenz: OMG DDS 1.4
    /// §2.2.2.2.1.13 `create_contentfilteredtopic`.
    ///
    /// Die `filter_expression` ist ein SQL-Subset (siehe Annex B).
    /// `filter_parameters` sind Strings, die `%0`, `%1`, ... in der
    /// Expression ersetzen.
    ///
    /// # Errors
    /// - `BadParameter` bei leerem Namen oder leerer Expression.
    /// - `BadParameter` wenn die Filter-Expression nicht parst.
    /// - `BadParameter` wenn ein referenzierter `%N`-Parameter nicht
    ///   im `filter_parameters`-Vec geliefert wird.
    pub fn create_contentfilteredtopic<T: DdsType>(
        &self,
        name: &str,
        related_topic: &Topic<T>,
        filter_expression: &str,
        filter_parameters: alloc::vec::Vec<String>,
    ) -> Result<ContentFilteredTopic<T>> {
        if name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "content-filtered-topic name",
            });
        }
        if filter_expression.is_empty() {
            return Err(DdsError::BadParameter {
                what: "filter expression",
            });
        }
        ContentFilteredTopic::<T>::new(
            name.into(),
            related_topic.clone(),
            filter_expression.into(),
            filter_parameters,
            self.clone(),
        )
    }

    /// Erzeugt eine `MultiTopic` als kombinierende TopicDescription
    /// ueber 1+ Underlying-Topics mit SQL-Subscription-Expression.
    /// Spec-Referenz: OMG DDS 1.4 §2.2.2.2.1.15 `create_multitopic`
    /// (optionales Spec-Feature).
    ///
    /// # Errors
    /// - `BadParameter` bei leerem Namen oder Type-Namen.
    /// - `BadParameter` wenn `related_topic_names` leer ist.
    /// - `BadParameter` wenn die Subscription-Expression nicht parst.
    /// - `BadParameter` wenn ein referenzierter `%N`-Parameter nicht
    ///   im `expression_parameters`-Vec geliefert wird.
    pub fn create_multitopic<T: DdsType>(
        &self,
        name: &str,
        type_name: &str,
        related_topic_names: alloc::vec::Vec<String>,
        subscription_expression: &str,
        expression_parameters: alloc::vec::Vec<String>,
    ) -> Result<crate::topic::MultiTopic<T>> {
        if name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic name",
            });
        }
        if type_name.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic type_name",
            });
        }
        if subscription_expression.is_empty() {
            return Err(DdsError::BadParameter {
                what: "multitopic subscription expression",
            });
        }
        crate::topic::MultiTopic::<T>::new(
            name.into(),
            type_name.into(),
            related_topic_names,
            subscription_expression.into(),
            expression_parameters,
            self.clone(),
        )
    }

    /// Loescht eine `MultiTopic`. Spec §2.2.2.2.1.16
    /// `delete_multitopic`. v1.2 ist es ein no-op-shim mit Participant-
    /// Match-Check.
    ///
    /// # Errors
    /// `BadParameter` wenn die MultiTopic zu einem anderen Participant
    /// gehoert.
    pub fn delete_multitopic<T: DdsType>(&self, mt: &crate::topic::MultiTopic<T>) -> Result<()> {
        if mt.get_participant().inner_ptr() != self.inner_ptr() {
            return Err(DdsError::BadParameter {
                what: "multitopic belongs to different participant",
            });
        }
        Ok(())
    }

    /// Loescht ein `ContentFilteredTopic`. Spec-Referenz:
    /// §2.2.2.2.1.14 `delete_contentfilteredtopic`.
    ///
    /// In Rust ist das Lifetime-Handle des CFT bereits durch `Drop`
    /// abgedeckt — die zugrundeliegenden Ressourcen werden frei, sobald
    /// der `ContentFilteredTopic<T>` aus dem Scope geht. Diese Methode
    /// existiert fuer Spec-Compliance der C++-API und validiert den
    /// `Participant`-Match (Spec verlangt `BadParameter`, wenn das CFT
    /// zu einem anderen Participant gehoert).
    ///
    /// # Errors
    /// - `BadParameter` wenn das CFT zu einem anderen Participant
    ///   gehoert.
    pub fn delete_contentfilteredtopic<T: DdsType>(
        &self,
        cft: &ContentFilteredTopic<T>,
    ) -> Result<()> {
        if cft.get_participant().inner_ptr() != self.inner_ptr() {
            return Err(DdsError::BadParameter {
                what: "cft belongs to different participant",
            });
        }
        Ok(())
    }

    /// Interner Identity-Pointer fuer Participant-Vergleich
    /// (verwendet bei `delete_contentfilteredtopic`-Validierung).
    pub(crate) fn inner_ptr(&self) -> *const ParticipantInner {
        Arc::as_ptr(&self.inner)
    }

    /// Erzeugt einen Publisher mit gegebener QoS (Default reicht fuer
    /// v1.2).
    pub fn create_publisher(&self, qos: PublisherQos) -> Publisher {
        #[cfg(feature = "std")]
        let p = {
            let p = Publisher::new(qos, self.inner.runtime.clone());
            // Bubble-Up-Back-Pointer (weak) verdrahten, damit
            // Writer-Events bis zum DomainParticipantListener kommen.
            p.attach_participant(Arc::downgrade(&self.inner));
            p
        };
        #[cfg(not(feature = "std"))]
        let p = Publisher::new(qos);
        // Handle fuer contains_entity / delete_contained_entities tracken.
        if let Ok(mut list) = self.inner.publishers.lock() {
            list.push(p.inner.entity_state.instance_handle());
        }
        p
    }

    /// Erzeugt einen Subscriber.
    pub fn create_subscriber(&self, qos: SubscriberQos) -> Subscriber {
        #[cfg(feature = "std")]
        let s = {
            let s = Subscriber::new(qos, self.inner.runtime.clone());
            // Bubble-Up-Back-Pointer (weak) verdrahten.
            s.attach_participant(Arc::downgrade(&self.inner));
            s
        };
        #[cfg(not(feature = "std"))]
        let s = Subscriber::new(qos);
        if let Ok(mut list) = self.inner.subscribers.lock() {
            list.push(s.inner.entity_state.instance_handle());
        }
        s
    }

    /// Anzahl aktuell registrierter Topics. Diagnose-API.
    #[must_use]
    pub fn topics_len(&self) -> usize {
        self.inner.topics.lock().map(|t| t.len()).unwrap_or(0)
    }

    /// Anzahl aktuell entdeckter Remote-Participants ueber SPDP.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.7 `get_discovered_participants`.
    /// 0 im offline-Modus.
    #[must_use]
    pub fn discovered_participants_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_participants().len();
        }
        0
    }

    /// Anzahl aktuell im SEDP-Cache bekannter Remote-Publications.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.9 `get_discovered_topics` (~analog).
    #[must_use]
    pub fn discovered_publications_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_publications_count();
        }
        0
    }

    /// Anzahl aktuell im SEDP-Cache bekannter Remote-Subscriptions.
    #[must_use]
    pub fn discovered_subscriptions_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_subscriptions_count();
        }
        0
    }

    // ============================================================
    // ignore_* (DDS 1.4 §2.2.2.2.1.14-17)
    // ============================================================

    /// Markiert einen entdeckten remote `DomainParticipant` als
    /// "ignoriert" — alle weiteren SPDP-Beacons mit diesem Handle
    /// fallen aus dem Builtin-Reader-Stream raus, und gleichzeitig
    /// werden alle SEDP-Endpoints, die zum gleichen Participant-
    /// Prefix gehoeren, ebenfalls verworfen (Spec §2.2.2.2.1.14).
    ///
    /// Per Spec ist die Aktion **monoton** — ein einmal ignorierter
    /// Participant bleibt es fuer den Lebenszyklus dieses
    /// Participants.
    ///
    /// # Errors
    /// Aktuell keine — die Methode liefert `Ok(())` immer. Spec laesst
    /// `OUT_OF_RESOURCES` zu, was wir aber nicht aktiv produzieren.
    pub fn ignore_participant(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.participants.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Markiert ein entdecktes remote Topic als "ignoriert".
    /// Spec §2.2.2.2.1.15.
    ///
    /// # Errors
    /// Wie [`Self::ignore_participant`].
    pub fn ignore_topic(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.topics.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Markiert eine entdeckte remote Publication als "ignoriert".
    /// Spec §2.2.2.2.1.16.
    ///
    /// # Errors
    /// Wie [`Self::ignore_participant`].
    pub fn ignore_publication(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.publications.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Markiert eine entdeckte remote Subscription als "ignoriert".
    /// Spec §2.2.2.2.1.17.
    ///
    /// # Errors
    /// Wie [`Self::ignore_participant`].
    pub fn ignore_subscription(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.subscriptions.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// `true` wenn `handle` per `ignore_participant` markiert wurde.
    #[must_use]
    pub fn is_participant_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_participant_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` wenn `handle` per `ignore_topic` markiert wurde.
    #[must_use]
    pub fn is_topic_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_topic_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` wenn `handle` per `ignore_publication` markiert wurde.
    #[must_use]
    pub fn is_publication_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_publication_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// `true` wenn `handle` per `ignore_subscription` markiert wurde.
    #[must_use]
    pub fn is_subscription_ignored(&self, handle: InstanceHandle) -> bool {
        #[cfg(feature = "std")]
        return self.inner.ignore_filter.is_subscription_ignored(handle);
        #[cfg(not(feature = "std"))]
        {
            let _ = handle;
            false
        }
    }

    /// Interner Zugriff auf den shared Ignore-Filter — von
    /// Tests + Runtime-Discovery-Hook genutzt.
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn ignore_filter(&self) -> IgnoreFilter {
        self.inner.ignore_filter.clone()
    }

    // ============================================================
    // delete_contained_entities (DDS 1.4 §2.2.2.2.1.18)
    // ============================================================

    /// Loescht **alle** vom Participant gehaltenen Children
    /// (Publishers, Subscribers, Topics, Builtin-Reader-Inboxes).
    /// Spec §2.2.2.2.1.18 — analoger Pendant existiert in
    /// Publisher/Subscriber/DataReader, der hier rekursiv mit
    /// abgedeckt wird.
    ///
    /// Offline-Verhalten:
    /// - Topic-Registry geleert (lokale Topics).
    /// - Publisher-/Subscriber-Tracker geleert.
    /// - Builtin-Topic-Reader-Inboxes geleert (so dass
    ///   `take()` nach `delete_contained_entities` ein leeres
    ///   Vec liefert).
    /// - **Kein** SEDP-Unannounce — das Live-Verhalten
    ///   uebernimmt das, sobald die Runtime ein
    ///   `Drop`/`shutdown`-Handle bekommt. Aktueller Stand: der
    ///   Runtime-Thread laeuft bis zum Process-Exit.
    ///
    /// # Errors
    /// `PreconditionNotMet` wenn ein interner Mutex vergiftet ist.
    pub fn delete_contained_entities(&self) -> Result<()> {
        // Topic-Registry leeren.
        {
            let mut topics =
                self.inner
                    .topics
                    .lock()
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "topic registry poisoned",
                    })?;
            topics.clear();
        }
        // Publisher-/Subscriber-Marker leeren.
        if let Ok(mut p) = self.inner.publishers.lock() {
            p.clear();
        }
        if let Ok(mut s) = self.inner.subscribers.lock() {
            s.clear();
        }
        // Builtin-Reader-Inboxes leeren — User soll nach
        // delete_contained_entities() einen sauberen Builtin-
        // Subscriber sehen, der erst neue (post-delete) Discovery-
        // Events ausliefert.
        let sinks = self.inner.builtin_subscriber.sinks();
        if let Ok(mut g) = sinks.participant.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.topic.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.publication.lock() {
            g.clear();
        }
        if let Ok(mut g) = sinks.subscription.lock() {
            g.clear();
        }
        Ok(())
    }

    /// Anzahl der per `create_publisher` getrackten Publisher.
    /// Diagnose-API fuer Tests.
    #[must_use]
    pub fn publishers_len(&self) -> usize {
        self.inner.publishers.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Anzahl der per `create_subscriber` getrackten Subscriber.
    #[must_use]
    pub fn subscribers_len(&self) -> usize {
        self.inner.subscribers.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Liefert den `InstanceHandle` dieses Participants. Identifiziert
    /// die Entity gegenueber DCPS-API-Konsumenten (Spec §2.2.2.1.1
    /// `get_instance_handle`).
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.entity_state.instance_handle()
    }

    /// Spec §2.2.2.2.1.10 `contains_entity` — `true` wenn `handle` zu
    /// diesem Participant oder einer seiner direkt **oder rekursiv**
    /// enthaltenen Entities gehoert.
    ///
    /// **Eingeschlossene Entity-Typen:**
    /// - der Participant selbst
    /// - alle per `create_topic` registrierten Topics
    /// - alle per `create_publisher` / `create_subscriber` erzeugten
    ///   Publisher/Subscriber
    /// - **rekursiv**: alle per `Publisher::create_datawriter` /
    ///   `Subscriber::create_datareader` erzeugten DataWriter/DataReader.
    #[must_use]
    pub fn contains_entity(&self, handle: InstanceHandle) -> bool {
        if self.instance_handle() == handle {
            return true;
        }
        if let Ok(topics) = self.inner.topics.lock() {
            for t in topics.values() {
                if t.entity_state.instance_handle() == handle {
                    return true;
                }
            }
        }
        if let Ok(pubs) = self.inner.publishers.lock() {
            if pubs.contains(&handle) {
                return true;
            }
        }
        if let Ok(subs) = self.inner.subscribers.lock() {
            if subs.contains(&handle) {
                return true;
            }
        }
        if let Ok(dws) = self.inner.datawriters.lock() {
            if dws.contains(&handle) {
                return true;
            }
        }
        if let Ok(drs) = self.inner.datareaders.lock() {
            if drs.contains(&handle) {
                return true;
            }
        }
        false
    }

    // ============================================================
    // get_discovered_* (DDS 1.4 §2.2.2.2.1.27-30)
    // ============================================================

    /// Liefert die `InstanceHandle`s aller aktuell entdeckten
    /// remote Participants (Spec §2.2.2.2.1.27). Im offline-Modus
    /// leer. Ignorierte Participants tauchen **nicht** auf.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_discovered_participants(&self) -> Vec<InstanceHandle> {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for d in rt.discovered_participants() {
            let h = InstanceHandle::from_guid(d.data.guid);
            if self.is_participant_ignored(h) {
                continue;
            }
            out.push(h);
        }
        out
    }

    /// Offline-Variante (kein std → keine Runtime).
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_participants(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// Liefert die `ParticipantBuiltinTopicData` zu einem Handle aus
    /// `get_discovered_participants` (Spec §2.2.2.2.1.28).
    ///
    /// # Errors
    /// `BadParameter` wenn `handle` keinen entdeckten Participant
    /// referenziert (oder wenn er ignoriert wurde).
    #[cfg(feature = "std")]
    pub fn get_discovered_participant_data(
        &self,
        handle: InstanceHandle,
    ) -> Result<ParticipantBuiltinTopicData> {
        if self.is_participant_ignored(handle) {
            return Err(DdsError::BadParameter {
                what: "participant handle is ignored",
            });
        }
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Err(DdsError::BadParameter {
                what: "no runtime — offline participant",
            });
        };
        for d in rt.discovered_participants() {
            if InstanceHandle::from_guid(d.data.guid) == handle {
                return Ok(ParticipantBuiltinTopicData::from_wire(&d.data));
            }
        }
        Err(DdsError::BadParameter {
            what: "unknown participant handle",
        })
    }

    /// Offline-Variante.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_participant_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<ParticipantBuiltinTopicData> {
        Err(DdsError::BadParameter {
            what: "no runtime — offline participant",
        })
    }

    /// Liefert die `InstanceHandle`s aller aktuell entdeckten
    /// remote Topics. Spec §2.2.2.2.1.29.
    ///
    /// Topics werden via SEDP-Pub/Sub-Announcements indirekt
    /// entdeckt — pro `(topic_name, type_name)` synthetisieren wir
    /// einen stabilen Schluessel via `TopicBuiltinTopicData::
    /// synthesize_key`. Ignorierte Topics tauchen nicht auf.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_discovered_topics(&self) -> Vec<InstanceHandle> {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Vec::new();
        };
        let Ok(sedp) = rt.sedp.lock() else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        for p in sedp.cache().publications() {
            let key = TopicBuiltinTopicData::synthesize_key(&p.data.topic_name, &p.data.type_name);
            let h = InstanceHandle::from_guid(key);
            if self.is_topic_ignored(h) {
                continue;
            }
            seen.insert(h);
        }
        for s in sedp.cache().subscriptions() {
            let key = TopicBuiltinTopicData::synthesize_key(&s.data.topic_name, &s.data.type_name);
            let h = InstanceHandle::from_guid(key);
            if self.is_topic_ignored(h) {
                continue;
            }
            seen.insert(h);
        }
        seen.into_iter().collect()
    }

    /// Offline-Variante.
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_topics(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// Liefert die `TopicBuiltinTopicData` zu einem Handle aus
    /// `get_discovered_topics`. Spec §2.2.2.2.1.30.
    ///
    /// # Errors
    /// `BadParameter` wenn `handle` keinem entdeckten Topic
    /// entspricht (oder ignoriert wurde).
    #[cfg(feature = "std")]
    pub fn get_discovered_topic_data(
        &self,
        handle: InstanceHandle,
    ) -> Result<TopicBuiltinTopicData> {
        if self.is_topic_ignored(handle) {
            return Err(DdsError::BadParameter {
                what: "topic handle is ignored",
            });
        }
        let Some(rt) = self.inner.runtime.as_ref() else {
            return Err(DdsError::BadParameter {
                what: "no runtime — offline participant",
            });
        };
        let Ok(sedp) = rt.sedp.lock() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "sedp poisoned",
            });
        };
        // Erste Match auf Pub-Seite.
        for p in sedp.cache().publications() {
            let topic = TopicBuiltinTopicData::from_publication(&p.data);
            if InstanceHandle::from_guid(topic.key) == handle {
                return Ok(topic);
            }
        }
        for s in sedp.cache().subscriptions() {
            let topic = TopicBuiltinTopicData::from_subscription(&s.data);
            if InstanceHandle::from_guid(topic.key) == handle {
                return Ok(topic);
            }
        }
        Err(DdsError::BadParameter {
            what: "unknown topic handle",
        })
    }

    /// Offline-Variante.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_topic_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<TopicBuiltinTopicData> {
        Err(DdsError::BadParameter {
            what: "no runtime — offline participant",
        })
    }

    /// Builtin-Subscriber des Participants (DDS 1.4 §2.2.2.2.1.7).
    ///
    /// Liefert immer denselben Subscriber-Handle (genau ein
    /// Builtin-Subscriber pro Participant). Er enthaelt 4
    /// vor-erzeugte Reader fuer die Builtin-Topics:
    ///
    /// - `DCPSParticipant` → `ParticipantBuiltinTopicData`
    /// - `DCPSTopic` → `TopicBuiltinTopicData`
    /// - `DCPSPublication` → `PublicationBuiltinTopicData`
    /// - `DCPSSubscription` → `SubscriptionBuiltinTopicData`
    ///
    /// SPDP-/SEDP-Receive triggert intern einen Sample-Insert, der
    /// per `take()`/`read()` abgeholt werden kann (DDS 1.4 §2.2.5).
    ///
    /// # Example
    /// ```
    /// use zerodds_dcps::*;
    /// let participant = DomainParticipantFactory::instance()
    ///     .create_participant_offline(0, DomainParticipantQos::default());
    /// let bs = participant.get_builtin_subscriber();
    /// let r = bs
    ///     .lookup_datareader::<DcpsParticipantBuiltinTopicData>("DCPSParticipant")
    ///     .expect("builtin reader");
    /// // Anfangs leer (offline-Mode → keine SPDP-Empfange).
    /// assert!(r.take().expect("take").is_empty());
    /// ```
    #[must_use]
    pub fn get_builtin_subscriber(&self) -> Arc<BuiltinSubscriber> {
        Arc::clone(&self.inner.builtin_subscriber)
    }

    // ============================================================
    // Listener-Slot (DDS 1.4 §2.2.2.2.3)
    // ============================================================

    /// Setzt den `DomainParticipantListener`. `listener=None` loescht
    /// den Slot. `mask` ist die [`StatusMask`], die festlegt, welche
    /// Status-Bits dieser Listener konsumiert (Spec §2.2.4.2.3 Bubble-Up).
    pub fn set_listener(&self, listener: Option<ArcDomainParticipantListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        // Spiegele die Mask ins EntityState — fuer get_listener_mask().
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// Liefert den aktuell installierten Listener-Klon, falls vorhanden.
    /// Spec §2.2.2.2.3.x get_listener.
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDomainParticipantListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot des Listener-Slots (Listener + Mask) — fuer den
    /// Dispatch-Pfad. Klont den Arc unter dem Mutex und gibt das
    /// Lock direkt frei (Lock-Discipline: Callbacks aussen ausfuehren).
    #[must_use]
    #[allow(dead_code)] // benutzt via Topic::listener_chain (cfg(std))
    pub(crate) fn snapshot_listener(&self) -> Option<(ArcDomainParticipantListener, StatusMask)> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)))
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

impl crate::entity::Entity for DomainParticipant {
    type Qos = DomainParticipantQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    fn set_qos(&self, qos: Self::Qos) -> Result<()> {
        // DomainParticipantQos: USER_DATA + ENTITY_FACTORY sind alle
        // Changeable=YES per Spec §2.2.3 — kein Immutable-Check nötig.
        if let Ok(mut current) = self.inner.qos.lock() {
            *current = qos;
        }
        Ok(())
    }

    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable();
        Ok(())
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

// ---- interne Helfer ----

fn topic_inner<T: DdsType>(t: &Topic<T>) -> Arc<TopicInner> {
    t.inner()
}

/// Extrahiert den `EquivalenceHash` aus einem
/// `TypeIdentifier`, sofern es einer der Hash-Varianten ist.
#[cfg(feature = "std")]
fn extract_equivalence_hash(
    ti: &zerodds_types::TypeIdentifier,
) -> Option<zerodds_types::EquivalenceHash> {
    use zerodds_types::TypeIdentifier;
    match ti {
        TypeIdentifier::EquivalenceHashMinimal(h) | TypeIdentifier::EquivalenceHashComplete(h) => {
            Some(*h)
        }
        _ => None,
    }
}

fn reconstruct_topic<T: DdsType>(
    inner: Arc<TopicInner>,
    participant: DomainParticipant,
) -> Topic<T> {
    // Der TopicInner selbst ist generisch-agnostisch (nur Name +
    // type-name-String); wir setzen einen neuen Topic-Handle mit
    // demselben Inner auf. `Topic::new` wuerde einen neuen Inner
    // anlegen — wir wollen aber den shared inner teilen.
    Topic::<T>::from_inner(inner, participant)
}

// Topic braucht dafuer einen `from_inner`-Konstruktor.
impl<T: DdsType> Topic<T> {
    pub(crate) fn from_inner(inner: Arc<TopicInner>, participant: DomainParticipant) -> Self {
        Self::_from_inner_impl(inner, participant)
    }
}

// Da `Topic<T>` seinen Inner privat haelt, brauchen wir einen
// `_from_inner_impl`-Shortcut ebenfalls im topic-Modul. Der ist
// gleich neben dem Konstruktor.

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dds_type::RawBytes;

    #[test]
    fn participant_created_with_domain_id() {
        let p = DomainParticipant::new(42, DomainParticipantQos::default());
        assert_eq!(p.domain_id(), 42);
        assert_eq!(p.topics_len(), 0);
    }

    /// Welle 4a (Spec `zerodds-zero-copy-1.0` §6): zwei GuidPrefixe im
    /// selben Prozess teilen den Host-Id-Prefix → `is_same_host = true`.
    /// PID-Bytes muessen mit `process::id()` korrespondieren.
    #[test]
    fn random_guid_prefixes_share_host_id_within_process() {
        let p1 = random_guid_prefix();
        let p2 = random_guid_prefix();
        assert_eq!(p1.host_id(), p2.host_id(), "same-host within process");
        assert!(p1.is_same_host(p2));

        let pid_le = std::process::id().to_le_bytes();
        let bytes = p1.to_bytes();
        assert_eq!(&bytes[4..8], &pid_le, "PID-Bytes in prefix[4..8]");

        // Counter+Time-Bytes muessen die beiden Prefixes unterscheidbar
        // machen.
        assert_ne!(p1, p2, "two prefixes must be distinct");
    }

    #[test]
    fn host_id_bytes_deterministic_within_process() {
        let a = host_id_bytes();
        let b = host_id_bytes();
        assert_eq!(a, b, "OnceLock-cached host-id muss stabil sein");
    }

    #[test]
    fn create_topic_stores_in_registry() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t1 = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        let t2 = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        assert_eq!(t1.name(), t2.name());
        assert_eq!(p.topics_len(), 1);
    }

    #[test]
    fn create_topic_rejects_type_conflict() {
        // Zweiter DdsType fuer Test.
        #[derive(Debug)]
        struct DummyU32(u32);
        impl DdsType for DummyU32 {
            const TYPE_NAME: &'static str = "test::DummyU32";
            fn encode(
                &self,
                out: &mut alloc::vec::Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                if bytes.len() != 4 {
                    return Err(crate::dds_type::DecodeError::Invalid { what: "u32 len" });
                }
                let mut a = [0u8; 4];
                a.copy_from_slice(bytes);
                Ok(Self(u32::from_le_bytes(a)))
            }
        }

        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _ = p
            .create_topic::<RawBytes>("X", TopicQos::default())
            .unwrap();
        let err = p
            .create_topic::<DummyU32>("X", TopicQos::default())
            .unwrap_err();
        assert!(matches!(err, DdsError::InconsistentPolicy { .. }));
    }

    #[test]
    fn create_topic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .create_topic::<RawBytes>("", TopicQos::default())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn lookup_topicdescription_returns_local_topics() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _t = p
            .create_topic::<RawBytes>("Hello", TopicQos::default())
            .unwrap();
        let h = p.lookup_topicdescription("Hello").expect("local lookup");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), "Hello");
        assert_eq!(h.get_type_name(), RawBytes::TYPE_NAME);
        assert_eq!(h.get_participant().domain_id(), 0);
    }

    #[test]
    fn lookup_topicdescription_none_for_unknown() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.lookup_topicdescription("Unknown").is_none());
    }

    // ---- §2.2.2.2.1.10 contains_entity ----

    #[test]
    fn contains_entity_returns_true_for_self_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = p.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_true_for_local_topic() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Hi", TopicQos::default())
            .unwrap();
        let topic_handle = t.inner().entity_state.instance_handle();
        assert!(p.contains_entity(topic_handle));
    }

    #[test]
    fn contains_entity_returns_true_for_local_publisher() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let pub_ = p.create_publisher(PublisherQos::default());
        let h = pub_.inner.entity_state.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_true_for_local_subscriber() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let s = p.create_subscriber(SubscriberQos::default());
        let h = s.inner.entity_state.instance_handle();
        assert!(p.contains_entity(h));
    }

    #[test]
    fn contains_entity_returns_false_for_unknown_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // Ein anderer Participant hat einen anderen Handle.
        let other = DomainParticipant::new(0, DomainParticipantQos::default());
        let other_h = other.instance_handle();
        assert!(!p.contains_entity(other_h));
    }

    #[test]
    fn contains_entity_returns_false_for_topic_after_delete() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Tmp", TopicQos::default())
            .unwrap();
        let topic_handle = t.inner().entity_state.instance_handle();
        assert!(p.contains_entity(topic_handle));
        p.delete_contained_entities().unwrap();
        assert!(!p.contains_entity(topic_handle));
    }

    #[test]
    fn contains_entity_recursive_finds_local_datawriter() {
        // §2.2.2.2.1.10 — contains_entity MUSS auch DataWriter-Handles
        // erkennen, die ueber Publisher::create_datawriter erzeugt wurden.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Hello", TopicQos::default())
            .unwrap();
        let pub_ = p.create_publisher(PublisherQos::default());
        let dw = pub_
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let dw_handle = dw.instance_handle();
        assert!(p.contains_entity(dw_handle));
        // Plus: Publisher selbst exposes contains_writer(handle).
        assert!(pub_.contains_writer(dw_handle));
    }

    #[test]
    fn contains_entity_recursive_finds_local_datareader() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Hello2", TopicQos::default())
            .unwrap();
        let sub = p.create_subscriber(SubscriberQos::default());
        let dr = sub
            .create_datareader(&topic, crate::qos::DataReaderQos::default())
            .unwrap();
        let dr_handle = dr.subscription_handle();
        assert!(p.contains_entity(dr_handle));
        assert!(sub.contains_reader(dr_handle));
    }

    #[test]
    fn contains_entity_recursive_does_not_find_foreign_datawriter() {
        // Negativ: DW, der ueber einen anderen Participant erzeugt wurde,
        // ist NICHT contained.
        let p1 = DomainParticipant::new(0, DomainParticipantQos::default());
        let p2 = DomainParticipant::new(1, DomainParticipantQos::default());
        let topic = p2
            .create_topic::<RawBytes>("Foreign", TopicQos::default())
            .unwrap();
        let pub2 = p2.create_publisher(PublisherQos::default());
        let dw2 = pub2
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        assert!(!p1.contains_entity(dw2.instance_handle()));
        assert!(p2.contains_entity(dw2.instance_handle()));
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_returns_immediately_for_local() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _t = p
            .create_topic::<RawBytes>("Local", TopicQos::default())
            .unwrap();
        let started = std::time::Instant::now();
        let h = p
            .find_topic("Local", core::time::Duration::from_secs(5))
            .expect("local find");
        // Sollte deutlich unter dem Timeout liegen — lokal ist
        // sofortiger Return.
        assert!(started.elapsed() < core::time::Duration::from_millis(50));
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), "Local");
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_times_out_when_unknown() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .find_topic("NotExists", core::time::Duration::from_millis(80))
            .unwrap_err();
        assert!(matches!(err, DdsError::Timeout));
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .find_topic("", core::time::Duration::from_millis(10))
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn create_contentfilteredtopic_rejects_empty_name() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn create_contentfilteredtopic_rejects_empty_expression() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("CF", &topic, "", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn delete_contentfilteredtopic_accepts_own() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap();
        p.delete_contentfilteredtopic(&cft).unwrap();
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_resolves_via_sedp_subscription() {
        // Variante des Discovery-Hooks: dieses Mal injizieren wir
        // eine Subscription (Reader-Side-Discovery), nicht eine
        // Publication. find_topic muss beides finden.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{DurabilityKind, ReliabilityKind, ReliabilityQos};
        use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                43,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("runtime start");

        let target_topic = "DiscoveredViaSubSedp";
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0xCD; 12]);
                let subdata = SubscriptionBuiltinTopicData {
                    key: Guid::new(prefix, EntityId::user_reader_with_key([4, 5, 6])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: target_topic.into(),
                    type_name: "test::SubT".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    data_representation: alloc::vec::Vec::new(),
                    content_filter: None,
                    security_info: None,
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                };
                sedp.cache_mut().insert_subscription(subdata, CoreDur::ZERO);
            }
        }

        let h = p
            .find_topic(target_topic, CoreDur::from_millis(200))
            .expect("find via subscription");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), target_topic);
        assert_eq!(h.get_type_name(), "test::SubT");
    }

    #[cfg(feature = "std")]
    #[test]
    fn find_topic_resolves_after_sedp_publication() {
        // Spec §2.2.2.2.1.11: find_topic muss zurueckkehren, sobald
        // ein Topic via Discovery sichtbar ist. Wir starten einen
        // Live-Participant (mit echter Runtime) und injizieren eine
        // Publication direkt in den SEDP-Cache, um den
        // Discovery-Hook zu verifizieren ohne abhaengig zu sein vom
        // UDP-Roundtrip.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                42,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("runtime start");

        let target_topic = "DiscoveredViaSedp";
        let target_type = "test::Discovered";

        // Spawn ein Worker, der nach kurzem Delay eine Publication
        // in den SEDP-Cache injiziert.
        let p_inject = p.clone();
        let topic_name = String::from(target_topic);
        let type_name = String::from(target_type);
        let join = std::thread::spawn(move || {
            std::thread::sleep(CoreDur::from_millis(50));
            if let Some(rt) = p_inject.runtime() {
                if let Ok(mut sedp) = rt.sedp.lock() {
                    let prefix = GuidPrefix::from_bytes([0xAB; 12]);
                    let pubdata = PublicationBuiltinTopicData {
                        key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                        participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                        topic_name,
                        type_name,
                        durability: DurabilityKind::Volatile,
                        reliability: ReliabilityQos {
                            kind: ReliabilityKind::Reliable,
                            max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(
                                1,
                            ),
                        },
                        ownership: zerodds_qos::OwnershipKind::Shared,
                        ownership_strength: 0,
                        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                        deadline: zerodds_qos::DeadlineQosPolicy::default(),
                        lifespan: zerodds_qos::LifespanQosPolicy::default(),
                        partition: alloc::vec::Vec::new(),
                        user_data: alloc::vec::Vec::new(),
                        topic_data: alloc::vec::Vec::new(),
                        group_data: alloc::vec::Vec::new(),
                        type_information: None,
                        data_representation: alloc::vec::Vec::new(),
                        security_info: None,
                        service_instance_name: None,
                        related_entity_guid: None,
                        topic_aliases: None,
                        type_identifier: zerodds_types::TypeIdentifier::None,
                    };
                    sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
                }
            }
        });

        let result = p.find_topic(target_topic, CoreDur::from_secs(2));
        join.join().expect("inject thread");
        let h = result.expect("find_topic should resolve via SEDP");
        use crate::topic::TopicDescription as _;
        assert_eq!(h.get_name(), target_topic);
        assert_eq!(h.get_type_name(), target_type);
    }

    // ============================================================
    // ignore_* / delete_contained_entities / get_discovered_*
    // ============================================================

    #[test]
    fn ignore_participant_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0xAA);
        assert!(!p.is_participant_ignored(h));
        p.ignore_participant(h).unwrap();
        assert!(p.is_participant_ignored(h));
    }

    #[test]
    fn ignore_topic_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0xBB);
        assert!(!p.is_topic_ignored(h));
        p.ignore_topic(h).unwrap();
        assert!(p.is_topic_ignored(h));
    }

    #[test]
    fn ignore_publication_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0xCC);
        assert!(!p.is_publication_ignored(h));
        p.ignore_publication(h).unwrap();
        assert!(p.is_publication_ignored(h));
    }

    #[test]
    fn ignore_subscription_records_handle() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0xDD);
        assert!(!p.is_subscription_ignored(h));
        p.ignore_subscription(h).unwrap();
        assert!(p.is_subscription_ignored(h));
    }

    #[test]
    fn ignore_lists_are_independent() {
        // Spec §2.2.2.2.1.14-17: jede ignore_*-Liste lebt fuer sich,
        // ein Handle in der Topic-Liste taucht nicht in der
        // Participant-Liste auf.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0xEE);
        p.ignore_topic(h).unwrap();
        assert!(p.is_topic_ignored(h));
        assert!(!p.is_participant_ignored(h));
        assert!(!p.is_publication_ignored(h));
        assert!(!p.is_subscription_ignored(h));
    }

    #[test]
    fn ignore_is_monotonic_and_idempotent() {
        // Doppeltes ignore_participant darf nicht in einen Fehler
        // umschlagen, und der Filter-State darf sich nicht "umkehren".
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let h = InstanceHandle::from_raw(0x42);
        p.ignore_participant(h).unwrap();
        p.ignore_participant(h).unwrap();
        assert!(p.is_participant_ignored(h));
    }

    #[test]
    fn delete_contained_entities_clears_topics_and_groups() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let _t = p
            .create_topic::<RawBytes>("ToBeRemoved", TopicQos::default())
            .unwrap();
        let _pub_ = p.create_publisher(PublisherQos::default());
        let _sub_ = p.create_subscriber(SubscriberQos::default());
        assert_eq!(p.topics_len(), 1);
        assert_eq!(p.publishers_len(), 1);
        assert_eq!(p.subscribers_len(), 1);
        p.delete_contained_entities().unwrap();
        assert_eq!(p.topics_len(), 0);
        assert_eq!(p.publishers_len(), 0);
        assert_eq!(p.subscribers_len(), 0);
    }

    #[test]
    fn delete_contained_entities_clears_builtin_reader_inboxes() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        // Per Hand einen Builtin-Sample injizieren, damit wir nach
        // dem clear gegen 0 vergleichen koennen.
        use crate::builtin_topics::ParticipantBuiltinTopicData as DcpsP;
        use zerodds_rtps::wire_types::Guid;
        let bs = p.get_builtin_subscriber();
        bs.sinks()
            .push_participant(&DcpsP {
                key: Guid::from_bytes([7u8; 16]),
                user_data: alloc::vec::Vec::new(),
            })
            .unwrap();
        let r = bs.participant_reader();
        assert_eq!(r.read().unwrap().len(), 1);
        p.delete_contained_entities().unwrap();
        assert_eq!(r.read().unwrap().len(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participants_offline_is_empty() {
        // Ohne Runtime liefert get_discovered_participants ein leeres
        // Vec — Spec §2.2.2.2.1.27 erlaubt das.
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.get_discovered_participants().is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participant_data_offline_errors() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .get_discovered_participant_data(InstanceHandle::from_raw(1))
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topics_offline_is_empty() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        assert!(p.get_discovered_topics().is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topic_data_offline_errors() {
        let p = DomainParticipant::new(0, DomainParticipantQos::default());
        let err = p
            .get_discovered_topic_data(InstanceHandle::from_raw(1))
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_participants_lists_after_spdp_inject() {
        // End-to-End: live Participant + ein synth. SPDP-Beacon eines
        // remote-Participants → get_discovered_participants liefert
        // genau ein Handle, get_discovered_participant_data findet die
        // Wire-Daten dazu.
        use crate::factory::DomainParticipantFactory;
        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                30,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");

        // Direkt in den Discovered-Cache injizieren ueber den
        // handle_spdp_datagram-Pfad. Wir bauen ein synthetisches
        // Beacon mit dem gleichen Helper wie die Runtime-Tests.
        use zerodds_rtps::participant_data::ParticipantBuiltinTopicData as WirePart;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, ProtocolVersion, VendorId};
        let remote = GuidPrefix::from_bytes([0xCA; 12]);
        let wire = WirePart {
            guid: Guid::new(remote, EntityId::PARTICIPANT),
            protocol_version: ProtocolVersion::V2_5,
            vendor_id: VendorId::ZERODDS,
            default_unicast_locator: None,
            default_multicast_locator: None,
            metatraffic_unicast_locator: None,
            metatraffic_multicast_locator: None,
            domain_id: Some(30),
            builtin_endpoint_set: 0,
            lease_duration: zerodds_rtps::participant_data::Duration::from_secs(100),
            user_data: alloc::vec::Vec::new(),
            properties: Default::default(),
            identity_token: None,
            permissions_token: None,
            identity_status_token: None,
            sig_algo_info: None,
            kx_algo_info: None,
            sym_cipher_algo_info: None,
        };
        let beacon = zerodds_discovery::spdp::SpdpBeacon::new(wire.clone())
            .serialize()
            .expect("serialize");
        if let Some(rt) = p.runtime() {
            crate::runtime::handle_spdp_datagram_for_test(rt, &beacon);
        }

        let handles = p.get_discovered_participants();
        assert_eq!(handles.len(), 1);
        let data = p
            .get_discovered_participant_data(handles[0])
            .expect("data lookup");
        assert_eq!(data.key, wire.guid);
        // Ignorieren → leere Liste.
        p.ignore_participant(handles[0]).unwrap();
        assert!(p.get_discovered_participants().is_empty());
        let err = p.get_discovered_participant_data(handles[0]).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topics_lists_unique_handles_for_pub_and_sub() {
        // Pub + Sub auf demselben (topic, type) → ein Topic-Handle.
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::subscription_data::SubscriptionBuiltinTopicData;
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                21,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0x77; 12]);
                let pubdata = PublicationBuiltinTopicData {
                    key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "SharedTopic".into(),
                    type_name: "SharedType".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    ownership_strength: 0,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    lifespan: zerodds_qos::LifespanQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    data_representation: alloc::vec::Vec::new(),
                    security_info: None,
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                };
                let subdata = SubscriptionBuiltinTopicData {
                    key: Guid::new(prefix, EntityId::user_reader_with_key([4, 5, 6])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "SharedTopic".into(),
                    type_name: "SharedType".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    data_representation: alloc::vec::Vec::new(),
                    content_filter: None,
                    security_info: None,
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
                sedp.cache_mut().insert_subscription(subdata, CoreDur::ZERO);
            }
        }
        let topics = p.get_discovered_topics();
        assert_eq!(topics.len(), 1, "Pub+Sub auf gleichem Topic → 1 Handle");
        let data = p.get_discovered_topic_data(topics[0]).expect("topic data");
        assert_eq!(data.name, "SharedTopic");
        assert_eq!(data.type_name, "SharedType");
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topic_data_filters_ignored() {
        use crate::factory::DomainParticipantFactory;
        use core::time::Duration as CoreDur;
        use zerodds_rtps::publication_data::{
            DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
        };
        use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                22,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");
        if let Some(rt) = p.runtime() {
            if let Ok(mut sedp) = rt.sedp.lock() {
                let prefix = GuidPrefix::from_bytes([0x55; 12]);
                let pubdata = PublicationBuiltinTopicData {
                    key: Guid::new(prefix, EntityId::user_writer_with_key([1, 2, 3])),
                    participant_key: Guid::new(prefix, EntityId::PARTICIPANT),
                    topic_name: "ToIgnore".into(),
                    type_name: "T".into(),
                    durability: DurabilityKind::Volatile,
                    reliability: ReliabilityQos {
                        kind: ReliabilityKind::Reliable,
                        max_blocking_time: zerodds_rtps::participant_data::Duration::from_secs(1),
                    },
                    ownership: zerodds_qos::OwnershipKind::Shared,
                    ownership_strength: 0,
                    liveliness: zerodds_qos::LivelinessQosPolicy::default(),
                    deadline: zerodds_qos::DeadlineQosPolicy::default(),
                    lifespan: zerodds_qos::LifespanQosPolicy::default(),
                    partition: alloc::vec::Vec::new(),
                    user_data: alloc::vec::Vec::new(),
                    topic_data: alloc::vec::Vec::new(),
                    group_data: alloc::vec::Vec::new(),
                    type_information: None,
                    data_representation: alloc::vec::Vec::new(),
                    security_info: None,
                    service_instance_name: None,
                    related_entity_guid: None,
                    topic_aliases: None,
                    type_identifier: zerodds_types::TypeIdentifier::None,
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
            }
        }
        let topics_before = p.get_discovered_topics();
        assert_eq!(topics_before.len(), 1);
        // Jetzt das Topic ignorieren — get_discovered_topics darf es
        // nicht mehr listen, get_discovered_topic_data muss
        // BadParameter liefern.
        p.ignore_topic(topics_before[0]).unwrap();
        assert!(p.get_discovered_topics().is_empty());
        let err = p.get_discovered_topic_data(topics_before[0]).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn delete_contentfilteredtopic_rejects_foreign() {
        let p1 = DomainParticipant::new(0, DomainParticipantQos::default());
        let p2 = DomainParticipant::new(1, DomainParticipantQos::default());
        let topic = p1
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p1
            .create_contentfilteredtopic("CF", &topic, "x > 0", alloc::vec::Vec::new())
            .unwrap();
        let err = p2.delete_contentfilteredtopic(&cft).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }
}
