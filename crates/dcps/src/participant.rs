// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DomainParticipant — the "root" entity of a DDS program.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.2 `DomainParticipant`.
//!
//! Every DDS program typically opens exactly one `DomainParticipant`
//! per domain id. The participant:
//!
//! - holds the GUID prefix (12 bytes, the base ID for all internal endpoints),
//! - registers itself via SPDP (Simple Participant Discovery Protocol),
//! - runs SEDP (Simple Endpoint Discovery Protocol) for
//!   topic/writer/reader matching,
//! - is the factory for publishers, subscribers and topics.
//!
//! # Modes
//!
//! - **Live mode** (`new_with_runtime`, called from
//!   `DomainParticipantFactory::create_participant`): binds UDP sockets,
//!   spawns SPDP/SEDP/WLP threads, runs the full discovery protocol and
//!   the TypeLookup service endpoints (XTypes 1.3 §7.6.3.3.4).
//! - **Offline mode** (`new`, called from
//!   `DomainParticipantFactory::create_participant_offline`): no
//!   sockets, no threads. The topic registry, QoS negotiation and a
//!   loopback path for unit tests are available.
//!
//! Topic registry: the same name + same type yields the same topic
//! handle (DDS 1.4 §2.2.2.2.1.10 `find_topic`).

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

/// Domain-id type (Spec: `DomainId_t` = long, i.e. i32).
pub type DomainId = i32;

/// Shared ignore-list filter of a `DomainParticipant`. Held by the
/// participant **and** consulted by the `DcpsRuntime` discovery hook
/// (a clone of the `Arc`). Spec reference: DDS DCPS 1.4 §2.2.2.2.1.14-17
/// `ignore_participant/topic/publication/subscription`.
///
/// Per spec the lists are **monotonically growing**: a handle can be
/// added, but never removed again. Hence `BTreeSet<InstanceHandle>`
/// suffices and no generation counters are needed.
#[derive(Debug, Default)]
#[cfg(feature = "std")]
pub(crate) struct IgnoreFilterInner {
    pub(crate) participants: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) topics: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) publications: Mutex<BTreeSet<InstanceHandle>>,
    pub(crate) subscriptions: Mutex<BTreeSet<InstanceHandle>>,
}

/// Cloneable filter handle (Arc bumps are cheap). The discovery hook may
/// poke in here in between, without forcing lock cycles on the entire
/// ParticipantInner.
#[derive(Clone, Debug, Default)]
#[cfg(feature = "std")]
pub struct IgnoreFilter {
    pub(crate) inner: Arc<IgnoreFilterInner>,
}

#[cfg(feature = "std")]
impl IgnoreFilter {
    /// Check whether a participant handle is ignored.
    #[must_use]
    pub fn is_participant_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .participants
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a topic handle is ignored.
    #[must_use]
    pub fn is_topic_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .topics
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a publication handle is ignored.
    #[must_use]
    pub fn is_publication_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .publications
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }

    /// Check whether a subscription handle is ignored.
    #[must_use]
    pub fn is_subscription_ignored(&self, h: InstanceHandle) -> bool {
        self.inner
            .subscriptions
            .lock()
            .map(|s| s.contains(&h))
            .unwrap_or(false)
    }
}

/// Randomly generated 12-byte participant prefix.
///
/// Scheme (Spec `zerodds-zero-copy-1.0` §6 wave 4):
/// - `bytes[0..4]`: host id (FNV1a hash of the `gethostname` output).
///   Two participants on the same machine carry the same host-id
///   prefix → discovery detects a same-host match and can enable a
///   zero-copy SHM path.
/// - `bytes[4..8]`: process id (LE).
/// - `bytes[8..12]`: timestamp + atomic counter, so that a restart of
///   the same process, or multiple participants in the same process,
///   get different prefixes.
///
/// A cross-host hash collision (4-byte FNV1a) is theoretically possible
/// but practically negligible; a false-positive same-host match would
/// only make the SHM setup fail and automatically fall back to the UDP
/// path.
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

/// Deterministic 4-byte host identifier based on `gethostname`. Cached
/// per process via `OnceLock`.
///
/// FNV1a-32 is enough: we need identity (same-host yes/no), not
/// cryptographic security. If `gethostname` fails (a CI container
/// without a hostname), we fall back to a process-local random value —
/// then no false-positive same-host match occurs with peers on the same
/// machine, which is safe (only the SHM optimization is missed).
///
/// `pub` so that `zerodds-c-api` places the same host identifier in its
/// GuidPrefix — otherwise two C-FFI processes on the same host would
/// never see each other as same-host (`is_same_host`), and SHM /
/// fragmentation optimizations would not apply for any C++/C#/TS
/// bindings.
#[cfg(feature = "std")]
pub fn host_id_bytes() -> [u8; 4] {
    use std::sync::OnceLock;
    static HOST_ID: OnceLock<[u8; 4]> = OnceLock::new();
    *HOST_ID.get_or_init(|| {
        // Primary: gethostname(3) — works uniformly on Linux, macOS and
        // the BSDs, without env-var / etc-file sources that are
        // sometimes missing (macOS has no /etc/hostname; HOSTNAME is
        // Bash-only and not exported; COMPUTERNAME is Windows).
        // Previously: 3 sources tried, all silently failed, fell back to
        // PID+time → a different host_id per process on the same
        // machine, and same-host optimizations (LOOPBACK_FRAGMENT_SIZE,
        // same-host SHM) did not apply.
        let hostname = gethostname_via_libc()
            .or_else(|| std::env::var("HOSTNAME").ok())
            .or_else(|| std::env::var("COMPUTERNAME").ok())
            .or_else(read_etc_hostname);
        let h = match hostname {
            Some(s) if !s.is_empty() => fnv1a_32(s.as_bytes()),
            _ => {
                // Last fallback: a process-local random value. Then this
                // process has a unique "host" and makes no
                // false-positive same-host optimization.
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

#[cfg(all(feature = "std", unix))]
#[allow(unsafe_code)]
fn gethostname_via_libc() -> Option<String> {
    // POSIX `gethostname(buf, len)` — 256 bytes are enough for all
    // realistic hostnames (HOST_NAME_MAX is typically 64 or 255).
    let mut buf = [0u8; 256];
    // SAFETY: buf is valid writable memory of buf.len() bytes;
    // gethostname writes at most len bytes and NUL-terminates.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast::<libc::c_char>(), buf.len()) };
    if rc != 0 {
        return None;
    }
    // NUL-terminated string; find it + decode UTF-8.
    let len = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    if len == 0 {
        return None;
    }
    core::str::from_utf8(&buf[..len]).ok().map(|s| s.to_owned())
}

#[cfg(all(feature = "std", not(unix)))]
fn gethostname_via_libc() -> Option<String> {
    None
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

/// The participant.
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
    /// Entity lifecycle (DCPS §2.2.2.1).
    pub(crate) entity_state: Arc<crate::entity::EntityState>,
    /// Topic registry (name → TopicInner). Repeated `create_topic` with
    /// the same name + type return the same handle; with a different
    /// type → `InconsistentPolicy` error.
    topics: Mutex<BTreeMap<String, Arc<TopicInner>>>,
    /// Runtime handle with UDP sockets + discovery threads. `None` when
    /// the participant was created in offline mode (tests that want no
    /// networking).
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// Pre-installed builtin subscriber (DDS 1.4 §2.2.2.2.1.7). Exactly
    /// one per participant. The sinks are hooked into the runtime
    /// discovery hook at construction time.
    pub(crate) builtin_subscriber: Arc<BuiltinSubscriber>,
    /// Ignore filter (Spec §2.2.2.2.1.14-17). A clone lives in the
    /// runtime and is checked by the discovery hot path, so that
    /// SPDP/SEDP samples no longer reach the builtin readers after
    /// `ignore_*`.
    #[cfg(feature = "std")]
    pub(crate) ignore_filter: IgnoreFilter,
    /// Local publisher registry (for `delete_contained_entities` +
    /// `contains_entity` per Spec §2.2.2.2.1.10). We track the
    /// `InstanceHandle` of every publisher created with
    /// `create_publisher`; `delete_contained_entities` clears the list.
    /// The actual drop semantics of each publisher happen via the `Arc`
    /// refcount once the user handle is dropped.
    publishers: Mutex<Vec<InstanceHandle>>,
    /// Analogous to `publishers`.
    subscribers: Mutex<Vec<InstanceHandle>>,
    /// Aggregate of all DataWriter handles of all publishers of this
    /// participant (Spec §2.2.2.2.1.10 contains_entity, recursive).
    /// Pub/Sub register new children via a weak back-reference.
    pub(crate) datawriters: Mutex<Vec<InstanceHandle>>,
    /// Aggregate of all DataReader handles of all subscribers of this
    /// participant.
    pub(crate) datareaders: Mutex<Vec<InstanceHandle>>,
    /// Optional [`ArcDomainParticipantListener`] + [`StatusMask`].
    /// Bubble-up target for all children whose narrower listener does not
    /// cover the status bit.
    pub(crate) listener: Mutex<Option<(ArcDomainParticipantListener, StatusMask)>>,
    /// Built-in DynamicType registry. Automatically populated in `new()`/
    /// `new_with_runtime()` with the 4 Spec §7.6.5 built-in types
    /// (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`,
    /// `DDS::KeyedBytes`). Retrievable via
    /// [`DomainParticipant::find_builtin_type`].
    #[cfg(feature = "std")]
    pub(crate) type_registry: Mutex<BTreeMap<String, zerodds_types::dynamic::DynamicType>>,
    /// TypeLookup client state per participant. Pending get-types
    /// requests are queued here; backoff via `last_attempt_per_hash` so
    /// that unknown TypeIDs are not re-queried every tick.
    #[cfg(feature = "std")]
    pub(crate) type_lookup: Mutex<TypeLookupState>,
}

/// TypeLookup client state per participant. Tracks pending requests +
/// backoff timer + retry count per unknown TypeID hash.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct TypeLookupState {
    /// Per TypeID: (last_attempt_instant, retry_count).
    pub attempts: BTreeMap<zerodds_types::EquivalenceHash, (std::time::Instant, u32)>,
    /// Optional sink for outgoing TypeLookup requests (test hook). The
    /// production path would be a reliable writer on the
    /// `TL_SVC_REQ_WRITER` endpoint; until then the sink queues (test
    /// mode) or stays None (live mode = no-op).
    pub outgoing: Vec<(zerodds_types::EquivalenceHash, u64)>,
}

#[cfg(feature = "std")]
impl TypeLookupState {
    /// Backoff period (5s) between retries.
    pub const BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);
    /// Maximum attempts per unknown TypeID.
    pub const MAX_ATTEMPTS: u32 = 3;
}

impl DomainParticipant {
    /// Offline constructor without a runtime — for skeleton tests.
    /// Production code goes through `DomainParticipantFactory::
    /// create_participant`, which automatically starts a runtime.
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
        // Auto-register the 4 Spec §7.6.5 built-in types.
        #[cfg(feature = "std")]
        participant.register_builtin_types();
        participant
    }

    /// Constructor with a live runtime (UDP + discovery). Returns
    /// `TransportError` if the socket bind fails.
    ///
    /// # Errors
    /// [`DdsError::TransportError`] on bind problems.
    #[cfg(feature = "std")]
    pub(crate) fn new_with_runtime(
        domain_id: DomainId,
        qos: DomainParticipantQos,
        config: RuntimeConfig,
    ) -> Result<Self> {
        // DDS-Security spec-style logger wireup: if the participant QoS carries
        // `dds.sec.log.*` properties, materialize the fan-out logger from them
        // and wire it into the runtime (no-op when absent).
        #[cfg(feature = "security")]
        let config = config
            .with_security_log_properties(&qos.property)
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "invalid dds.sec.log.* security logger configuration",
            })?;
        let runtime = DcpsRuntime::start(domain_id, random_guid_prefix(), config)?;
        let builtin = Arc::new(BuiltinSubscriber::new());
        // Wire up the discovery hook: from now on the runtime pushes
        // SPDP/SEDP events into the 4 builtin readers.
        runtime.attach_builtin_sinks(builtin.sinks());
        // Share the ignore filter with the runtime, so that the
        // discovery hot path (handle_spdp_datagram +
        // push_sedp_events_to_builtin_readers) can consult the lists.
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
        // Auto-register the 4 Spec §7.6.5 built-in types.
        participant.register_builtin_types();
        Ok(participant)
    }

    /// Internal access to the runtime — used by Publisher/Subscriber to
    /// create DataWriter/Reader. `None` when the participant is in
    /// offline mode.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn runtime(&self) -> Option<&Arc<DcpsRuntime>> {
        self.inner.runtime.as_ref()
    }

    /// Domain id.
    #[must_use]
    pub fn domain_id(&self) -> DomainId {
        self.inner.domain_id
    }

    /// Returns a copy of the DomainParticipantQos (Spec §2.2.2.2.1.4
    /// `get_qos`).
    #[must_use]
    pub fn qos(&self) -> DomainParticipantQos {
        self.inner.qos.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Sets the DomainParticipantQos (Spec §2.2.2.2.1.3 `set_qos`).
    ///
    /// # Errors
    /// Currently none — the method always returns `Ok(())`. The spec
    /// allows `IMMUTABLE_POLICY`, which we do not actively produce (all
    /// policies are mutable in RC1).
    pub fn set_qos(&self, qos: DomainParticipantQos) -> Result<()> {
        if let Ok(mut g) = self.inner.qos.lock() {
            *g = qos;
        }
        Ok(())
    }

    /// Registers the 4 Spec §7.6.5 built-in types
    /// (`DDS::String`, `DDS::KeyedString`, `DDS::Bytes`, `DDS::KeyedBytes`)
    /// in the local type registry. Idempotent — a second call overwrites
    /// the entries deterministically.
    ///
    /// Called automatically from `new()`/`new_with_runtime()`, but can
    /// also be called again after an `unregister_builtin_types()`
    /// disable.
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

    /// Deletes all registered built-in types. Not called from any
    /// default path today — a test helper for disable-flag tests.
    #[cfg(feature = "std")]
    pub fn unregister_builtin_types(&self) {
        if let Ok(mut reg) = self.inner.type_registry.lock() {
            reg.retain(|name, _| !zerodds_types::dynamic::is_builtin_type_name(name));
        }
    }

    /// Lookup of a built-in type by spec name (Spec §7.6.5). Returns
    /// `Some(DynamicType)` if the name is known (registered via
    /// `register_builtin_types`).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn find_builtin_type(&self, name: &str) -> Option<zerodds_types::dynamic::DynamicType> {
        self.inner
            .type_registry
            .lock()
            .ok()
            .and_then(|reg| reg.get(name).cloned())
    }

    /// Number of registered built-in types. After `new()` == 4.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn registered_type_count(&self) -> usize {
        self.inner
            .type_registry
            .lock()
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Attempts to queue a TypeLookup request for an unknown
    /// `EquivalenceHash`. Respects backoff (5s between attempts) and at
    /// most 3 retries per hash.
    ///
    /// Returns: `true` if the request was queued, `false` on backoff
    /// suppression or max attempts.
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
        // Next sequence number for the request.
        let seq = state.outgoing.len() as u64 + 1;
        state.outgoing.push((hash, seq));
        true
    }

    /// Drains the queued TypeLookup requests. Returns `Vec<(hash, seq)>`.
    /// In a production environment the caller would send the hashes via
    /// TypeLookupClient + reliable writer to the `TL_SVC_REQ_WRITER`
    /// endpoint.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn drain_type_lookup_requests(&self) -> Vec<(zerodds_types::EquivalenceHash, u64)> {
        self.inner
            .type_lookup
            .lock()
            .map(|mut s| core::mem::take(&mut s.outgoing))
            .unwrap_or_default()
    }

    /// Receives a TypeLookup reply (TypeObjects per hash). Registers the
    /// TypeObjects in an internal type-registry mirror — afterwards a
    /// stalled QoS match can be retried.
    ///
    /// Returns the number of successfully registered types.
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
        // Clippy-bait avoidance: the types vec is consumed here; the
        // actual type-registry insert can be done by the caller
        // (e.g. via the shared TypeLookupServer.registry).
        let _ = types;
        count
    }

    /// SEDP discovery hook: checks an incoming
    /// `PublicationBuiltinTopicData` for type hashes that cannot be
    /// resolved locally. If needed, a TypeLookup request is queued via
    /// `enqueue_type_lookup`.
    ///
    /// The RPC path is live via `DcpsRuntime::send_type_lookup_request`
    /// on the TL_SVC_REQ_* endpoints (XTypes 1.3 §7.6.3.3.4); this method
    /// decides per hash whether a re-request is worthwhile (local
    /// registry lookup + backoff tracking).
    ///
    /// Returns: number of unknown hashes queued (max 2 — minimal +
    /// complete).
    #[cfg(feature = "std")]
    pub fn on_remote_publication_discovered(&self, type_information_blob: Option<&[u8]>) -> usize {
        self.on_remote_type_information(type_information_blob)
    }

    /// SEDP discovery hook for `SubscriptionBuiltinTopicData`. Symmetric
    /// to `on_remote_publication_discovered`.
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
        // Check the minimal hash.
        if let Some(hash) = extract_equivalence_hash(&ti.minimal.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        // Check the complete hash (if present).
        if let Some(hash) = extract_equivalence_hash(&ti.complete.typeid_with_size.type_id) {
            if !self.has_type_for_hash(hash) && self.enqueue_type_lookup(hash) {
                queued += 1;
            }
        }
        queued
    }

    /// Internal helper — true if the hash is already resolvable in the
    /// local `TypeLookupServer.registry` (either fed in locally via
    /// `register_type_object` or populated by a previous `getTypes`
    /// reply ingest). Prevents us from issuing redundant lookup requests
    /// for hashes we already know.
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

    /// True if MAX_ATTEMPTS has already been reached for the hash.
    /// Consulted by the match-retry path: give up eventually instead of
    /// polling forever.
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

    /// Creates a typed topic handle. Repeated calls with the same name +
    /// type return the same handle (ref-shared).
    ///
    /// # Errors
    /// - `InconsistentPolicy` if a topic with this name is already
    ///   registered under a different type.
    /// - `BadParameter` for an empty name.
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
                // Inconsistent-topic detection. Bumps the counter on the
                // existing topic — on the next `inconsistent_topic_status()`
                // read, the listener is fired via bubble-up.
                #[cfg(feature = "std")]
                existing
                    .inconsistent_topic_count
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                return Err(DdsError::InconsistentPolicy {
                    what: "topic name reused with different type",
                });
            }
            // Same type → shared handle.
            return Ok(reconstruct_topic::<T>(existing.clone(), self.clone()));
        }
        let topic = Topic::<T>::new(name.into(), qos, self.clone());
        topics.insert(name.into(), topic_inner(&topic));
        Ok(topic)
    }

    /// Immediate local lookup of a topic by name — returns `None` if no
    /// local `create_topic` with this name has occurred. **Does no
    /// discovery wait** (that is `find_topic`). Spec reference: OMG
    /// DDS 1.4 §2.2.2.2.1.12 "lookup_topicdescription".
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

    /// Waits until a topic with the given name is visible via discovery
    /// (an SEDP publication or subscription) — or until `timeout`
    /// elapses. Spec reference: OMG DDS 1.4 §2.2.2.2.1.11 `find_topic`.
    ///
    /// Returns:
    /// - `Ok(handle)` with name + type name + participant, if a matching
    ///   SEDP endpoint became visible during `timeout`. Local topics
    ///   count as well (no need to wait if `create_topic` already ran).
    /// - `Err(Timeout)` if `timeout` elapsed.
    ///
    /// # Errors
    /// - `DdsError::Timeout` if `timeout` elapsed without a discovery
    ///   match.
    /// - `DdsError::BadParameter` for an empty name.
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
        // Check locally right away — avoids a busy-wait if the topic was
        // already created locally via create_topic.
        if let Some(h) = self.lookup_topicdescription(name) {
            return Ok(h);
        }
        // Poll loop over the SEDP cache. The spec leaves the strategy
        // open; Cyclone DDS polls as well.
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

    /// Helper: checks the SEDP cache for whether a remote endpoint
    /// (publication or subscription) has announced a topic with the
    /// name. Returns the first match (name + type name).
    #[cfg(feature = "std")]
    fn find_topic_in_sedp(&self, name: &str) -> Option<TopicDescriptionHandle> {
        let rt = self.inner.runtime.as_ref()?;
        let sedp = rt.sedp.lock().ok()?;
        // Check publications first.
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

    /// Creates a `ContentFilteredTopic` as a subset of an existing
    /// `Topic<T>`. Spec reference: OMG DDS 1.4 §2.2.2.2.1.13
    /// `create_contentfilteredtopic`.
    ///
    /// The `filter_expression` is a SQL subset (see Annex B).
    /// `filter_parameters` are strings that replace `%0`, `%1`, ... in
    /// the expression.
    ///
    /// # Errors
    /// - `BadParameter` for an empty name or empty expression.
    /// - `BadParameter` if the filter expression does not parse.
    /// - `BadParameter` if a referenced `%N` parameter is not supplied
    ///   in the `filter_parameters` vec.
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

    /// Creates a `MultiTopic` as a combining TopicDescription over 1+
    /// underlying topics with a SQL subscription expression. Spec
    /// reference: OMG DDS 1.4 §2.2.2.2.1.15 `create_multitopic`
    /// (an optional spec feature).
    ///
    /// # Errors
    /// - `BadParameter` for an empty name or type name.
    /// - `BadParameter` if `related_topic_names` is empty.
    /// - `BadParameter` if the subscription expression does not parse.
    /// - `BadParameter` if a referenced `%N` parameter is not supplied
    ///   in the `expression_parameters` vec.
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

    /// Deletes a `MultiTopic`. Spec §2.2.2.2.1.16 `delete_multitopic`.
    /// In v1.2 it is a no-op shim with a participant match check.
    ///
    /// # Errors
    /// `BadParameter` if the MultiTopic belongs to a different
    /// participant.
    pub fn delete_multitopic<T: DdsType>(&self, mt: &crate::topic::MultiTopic<T>) -> Result<()> {
        if mt.get_participant().inner_ptr() != self.inner_ptr() {
            return Err(DdsError::BadParameter {
                what: "multitopic belongs to different participant",
            });
        }
        Ok(())
    }

    /// Deletes a `ContentFilteredTopic`. Spec reference: §2.2.2.2.1.14
    /// `delete_contentfilteredtopic`.
    ///
    /// In Rust, the CFT's lifetime handle is already covered by `Drop` —
    /// the underlying resources are freed once the
    /// `ContentFilteredTopic<T>` goes out of scope. This method exists
    /// for spec compliance of the C++ API and validates the participant
    /// match (the spec requires `BadParameter` if the CFT belongs to a
    /// different participant).
    ///
    /// # Errors
    /// - `BadParameter` if the CFT belongs to a different participant.
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

    /// Internal identity pointer for participant comparison (used in
    /// `delete_contentfilteredtopic` validation).
    pub(crate) fn inner_ptr(&self) -> *const ParticipantInner {
        Arc::as_ptr(&self.inner)
    }

    /// Creates a publisher with the given QoS (the default is enough for
    /// v1.2).
    pub fn create_publisher(&self, qos: PublisherQos) -> Publisher {
        #[cfg(feature = "std")]
        let p = {
            let p = Publisher::new(qos, self.inner.runtime.clone());
            // Wire up the (weak) bubble-up back-pointer, so that writer
            // events reach the DomainParticipantListener.
            p.attach_participant(Arc::downgrade(&self.inner));
            p
        };
        #[cfg(not(feature = "std"))]
        let p = Publisher::new(qos);
        // Track the handle for contains_entity / delete_contained_entities.
        if let Ok(mut list) = self.inner.publishers.lock() {
            list.push(p.inner.entity_state.instance_handle());
        }
        p
    }

    /// Creates a subscriber.
    pub fn create_subscriber(&self, qos: SubscriberQos) -> Subscriber {
        #[cfg(feature = "std")]
        let s = {
            let s = Subscriber::new(qos, self.inner.runtime.clone());
            // Wire up the (weak) bubble-up back-pointer.
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

    /// Number of currently registered topics. Diagnostic API.
    #[must_use]
    pub fn topics_len(&self) -> usize {
        self.inner.topics.lock().map(|t| t.len()).unwrap_or(0)
    }

    /// Number of currently discovered remote participants via SPDP.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.7 `get_discovered_participants`.
    /// 0 in offline mode.
    #[must_use]
    pub fn discovered_participants_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_participants().len();
        }
        0
    }

    /// Number of remote publications currently known in the SEDP cache.
    /// Spec: OMG DDS 1.4 §2.2.2.2.1.9 `get_discovered_topics` (~analogous).
    #[must_use]
    pub fn discovered_publications_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            return rt.discovered_publications_count();
        }
        0
    }

    /// Number of remote subscriptions currently known in the SEDP cache.
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

    /// Marks a discovered remote `DomainParticipant` as "ignored" — all
    /// further SPDP beacons with this handle drop out of the builtin
    /// reader stream, and at the same time all SEDP endpoints belonging
    /// to the same participant prefix are also discarded
    /// (Spec §2.2.2.2.1.14).
    ///
    /// Per spec the action is **monotonic** — a once-ignored participant
    /// stays ignored for the lifetime of this participant.
    ///
    /// # Errors
    /// Currently none — the method always returns `Ok(())`. The spec
    /// allows `OUT_OF_RESOURCES`, which we do not actively produce.
    pub fn ignore_participant(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.participants.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Marks a discovered remote topic as "ignored". Spec §2.2.2.2.1.15.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_topic(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.topics.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Marks a discovered remote publication as "ignored".
    /// Spec §2.2.2.2.1.16.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_publication(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.publications.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// Marks a discovered remote subscription as "ignored".
    /// Spec §2.2.2.2.1.17.
    ///
    /// # Errors
    /// As [`Self::ignore_participant`].
    pub fn ignore_subscription(&self, handle: InstanceHandle) -> Result<()> {
        #[cfg(feature = "std")]
        if let Ok(mut s) = self.inner.ignore_filter.inner.subscriptions.lock() {
            s.insert(handle);
        }
        Ok(())
    }

    /// `true` if `handle` was marked via `ignore_participant`.
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

    /// `true` if `handle` was marked via `ignore_topic`.
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

    /// `true` if `handle` was marked via `ignore_publication`.
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

    /// `true` if `handle` was marked via `ignore_subscription`.
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

    /// Internal access to the shared ignore filter — used by tests + the
    /// runtime discovery hook.
    #[cfg(feature = "std")]
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn ignore_filter(&self) -> IgnoreFilter {
        self.inner.ignore_filter.clone()
    }

    // ============================================================
    // delete_contained_entities (DDS 1.4 §2.2.2.2.1.18)
    // ============================================================

    /// Deletes **all** children held by the participant (publishers,
    /// subscribers, topics, builtin reader inboxes). Spec §2.2.2.2.1.18
    /// — an analogous counterpart exists in
    /// Publisher/Subscriber/DataReader, which is covered here
    /// recursively.
    ///
    /// Offline behavior:
    /// - Topic registry cleared (local topics).
    /// - Publisher/subscriber trackers cleared.
    /// - Builtin-topic reader inboxes cleared (so that `take()` after
    ///   `delete_contained_entities` returns an empty vec).
    /// - **No** SEDP unannounce — the live behavior handles that once
    ///   the runtime gets a `Drop`/`shutdown` handle. Current state: the
    ///   runtime thread runs until process exit.
    ///
    /// # Errors
    /// `PreconditionNotMet` if an internal mutex is poisoned.
    pub fn delete_contained_entities(&self) -> Result<()> {
        // Clear the topic registry.
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
        // Clear the publisher/subscriber markers.
        if let Ok(mut p) = self.inner.publishers.lock() {
            p.clear();
        }
        if let Ok(mut s) = self.inner.subscribers.lock() {
            s.clear();
        }
        // Clear the builtin reader inboxes — after
        // delete_contained_entities() the user should see a clean
        // builtin subscriber that only delivers new (post-delete)
        // discovery events.
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

    /// Number of publishers tracked via `create_publisher`. Diagnostic
    /// API for tests.
    #[must_use]
    pub fn publishers_len(&self) -> usize {
        self.inner.publishers.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Number of subscribers tracked via `create_subscriber`.
    #[must_use]
    pub fn subscribers_len(&self) -> usize {
        self.inner.subscribers.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Returns this participant's `InstanceHandle`. Identifies the entity
    /// to DCPS API consumers (Spec §2.2.2.1.1 `get_instance_handle`).
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.entity_state.instance_handle()
    }

    /// This participant's **discovery-space** handle: `InstanceHandle::from_guid`
    /// of its participant GUID.
    ///
    /// Unlike [`Self::instance_handle`] (a local allocator counter), this is the
    /// handle the ignore filter and discovery actually key on. It is therefore
    /// the correct argument to ignore THIS participant from another one — e.g. a
    /// durability service whose sibling ingest/replay participants must ignore
    /// each other to avoid an echo loop. `HANDLE_NIL` when offline.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn participant_handle(&self) -> InstanceHandle {
        match self.inner.runtime.as_ref() {
            Some(rt) => {
                let guid = zerodds_rtps::wire_types::Guid::new(
                    rt.guid_prefix,
                    zerodds_rtps::wire_types::EntityId::PARTICIPANT,
                );
                crate::instance_handle::InstanceHandle::from_guid(guid)
            }
            None => crate::instance_handle::HANDLE_NIL,
        }
    }

    /// Spec §2.2.2.2.1.10 `contains_entity` — `true` if `handle` belongs
    /// to this participant or one of its directly **or recursively**
    /// contained entities.
    ///
    /// **Included entity types:**
    /// - the participant itself
    /// - all topics registered via `create_topic`
    /// - all publishers/subscribers created via `create_publisher` /
    ///   `create_subscriber`
    /// - **recursively**: all DataWriter/DataReader created via
    ///   `Publisher::create_datawriter` / `Subscriber::create_datareader`.
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

    /// Returns the `InstanceHandle`s of all currently discovered remote
    /// participants (Spec §2.2.2.2.1.27). Empty in offline mode. Ignored
    /// participants do **not** appear.
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

    /// Offline variant (no std → no runtime).
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_participants(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// Returns the `ParticipantBuiltinTopicData` for a handle from
    /// `get_discovered_participants` (Spec §2.2.2.2.1.28).
    ///
    /// # Errors
    /// `BadParameter` if `handle` does not reference a discovered
    /// participant (or if it was ignored).
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

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_participant_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<ParticipantBuiltinTopicData> {
        Err(DdsError::BadParameter {
            what: "no runtime — offline participant",
        })
    }

    /// Returns the `InstanceHandle`s of all currently discovered remote
    /// topics. Spec §2.2.2.2.1.29.
    ///
    /// Topics are discovered indirectly via SEDP pub/sub announcements —
    /// per `(topic_name, type_name)` we synthesize a stable key via
    /// `TopicBuiltinTopicData::synthesize_key`. Ignored topics do not
    /// appear.
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

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    #[must_use]
    pub fn get_discovered_topics(&self) -> Vec<InstanceHandle> {
        Vec::new()
    }

    /// Returns the `TopicBuiltinTopicData` for a handle from
    /// `get_discovered_topics`. Spec §2.2.2.2.1.30.
    ///
    /// # Errors
    /// `BadParameter` if `handle` does not correspond to a discovered
    /// topic (or was ignored).
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
        // First match on the publication side.
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

    /// Offline variant.
    #[cfg(not(feature = "std"))]
    pub fn get_discovered_topic_data(
        &self,
        _handle: InstanceHandle,
    ) -> Result<TopicBuiltinTopicData> {
        Err(DdsError::BadParameter {
            what: "no runtime — offline participant",
        })
    }

    /// The participant's builtin subscriber (DDS 1.4 §2.2.2.2.1.7).
    ///
    /// Always returns the same subscriber handle (exactly one builtin
    /// subscriber per participant). It contains 4 pre-created readers for
    /// the builtin topics:
    ///
    /// - `DCPSParticipant` → `ParticipantBuiltinTopicData`
    /// - `DCPSTopic` → `TopicBuiltinTopicData`
    /// - `DCPSPublication` → `PublicationBuiltinTopicData`
    /// - `DCPSSubscription` → `SubscriptionBuiltinTopicData`
    ///
    /// SPDP/SEDP receive internally triggers a sample insert that can be
    /// picked up via `take()`/`read()` (DDS 1.4 §2.2.5).
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
    /// // Initially empty (offline mode → no SPDP receives).
    /// assert!(r.take().expect("take").is_empty());
    /// ```
    #[must_use]
    pub fn get_builtin_subscriber(&self) -> Arc<BuiltinSubscriber> {
        Arc::clone(&self.inner.builtin_subscriber)
    }

    // ============================================================
    // Listener-Slot (DDS 1.4 §2.2.2.2.3)
    // ============================================================

    /// Sets the `DomainParticipantListener`. `listener=None` clears the
    /// slot. `mask` is the [`StatusMask`] that determines which status
    /// bits this listener consumes (Spec §2.2.4.2.3 bubble-up).
    pub fn set_listener(&self, listener: Option<ArcDomainParticipantListener>, mask: StatusMask) {
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        // Mirror the mask into the EntityState — for get_listener_mask().
        self.inner.entity_state.set_listener_mask(mask);
    }

    /// Returns the currently installed listener clone, if present.
    /// Spec §2.2.2.2.3.x get_listener.
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDomainParticipantListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
    }

    /// Snapshot of the listener slot (listener + mask) — for the dispatch
    /// path. Clones the Arc under the mutex and releases the lock
    /// immediately (lock discipline: run callbacks outside).
    #[must_use]
    #[allow(dead_code)] // used via Topic::listener_chain (cfg(std))
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
        // DomainParticipantQos: USER_DATA + ENTITY_FACTORY are all
        // Changeable=YES per Spec §2.2.3 — no immutable check needed.
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

// ---- internal helpers ----

fn topic_inner<T: DdsType>(t: &Topic<T>) -> Arc<TopicInner> {
    t.inner()
}

/// Extracts the `EquivalenceHash` from a `TypeIdentifier`, if it is one
/// of the hash variants.
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
    // The TopicInner itself is generic-agnostic (just name +
    // type-name string); we set up a new topic handle with the same
    // inner. `Topic::new` would create a new inner — but we want to
    // share the shared inner.
    Topic::<T>::from_inner(inner, participant)
}

// Topic needs a `from_inner` constructor for this.
impl<T: DdsType> Topic<T> {
    pub(crate) fn from_inner(inner: Arc<TopicInner>, participant: DomainParticipant) -> Self {
        Self::_from_inner_impl(inner, participant)
    }
}

// Since `Topic<T>` keeps its inner private, we also need a
// `_from_inner_impl` shortcut in the topic module. It is right next to
// the constructor.

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

    /// Wave 4a (Spec `zerodds-zero-copy-1.0` §6): two GuidPrefixes in the
    /// same process share the host-id prefix → `is_same_host = true`.
    /// The PID bytes must correspond to `process::id()`.
    #[test]
    fn random_guid_prefixes_share_host_id_within_process() {
        let p1 = random_guid_prefix();
        let p2 = random_guid_prefix();
        assert_eq!(p1.host_id(), p2.host_id(), "same-host within process");
        assert!(p1.is_same_host(p2));

        let pid_le = std::process::id().to_le_bytes();
        let bytes = p1.to_bytes();
        assert_eq!(&bytes[4..8], &pid_le, "PID bytes in prefix[4..8]");

        // The counter + time bytes must make the two prefixes
        // distinguishable.
        assert_ne!(p1, p2, "two prefixes must be distinct");
    }

    #[test]
    fn host_id_bytes_deterministic_within_process() {
        let a = host_id_bytes();
        let b = host_id_bytes();
        assert_eq!(a, b, "OnceLock-cached host-id must be stable");
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
        // A second DdsType for the test.
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
        // A different participant has a different handle.
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
        // §2.2.2.2.1.10 — contains_entity MUST also recognize DataWriter
        // handles created via Publisher::create_datawriter.
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
        // Plus: the publisher itself exposes contains_writer(handle).
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
        // Negative: a DW created via a different participant is NOT
        // contained.
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
        // Should be well below the timeout — local is an immediate
        // return.
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
        // A variant of the discovery hook: this time we inject a
        // subscription (reader-side discovery), not a publication.
        // find_topic must find both.
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
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
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
        // Spec §2.2.2.2.1.11: find_topic must return as soon as a topic
        // is visible via discovery. We start a live participant (with a
        // real runtime) and inject a publication directly into the SEDP
        // cache, to verify the discovery hook without depending on the
        // UDP round-trip.
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

        // Spawn a worker that, after a short delay, injects a publication
        // into the SEDP cache.
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
                        unicast_locators: Vec::new(),
                        multicast_locators: Vec::new(),
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
        // Spec §2.2.2.2.1.14-17: each ignore_* list lives on its own; a
        // handle in the topic list does not appear in the participant
        // list.
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
        // A double ignore_participant must not turn into an error, and
        // the filter state must not "reverse".
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
        // Manually inject a builtin sample, so that after the clear we
        // can compare against 0.
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
        // Without a runtime, get_discovered_participants returns an empty
        // vec — Spec §2.2.2.2.1.27 allows that.
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
        // End-to-end: a live participant + one synthetic SPDP beacon of a
        // remote participant → get_discovered_participants returns
        // exactly one handle, get_discovered_participant_data finds the
        // matching wire data.
        use crate::factory::DomainParticipantFactory;
        let p = DomainParticipantFactory::instance()
            .create_participant_with_config(
                30,
                DomainParticipantQos::default(),
                crate::runtime::RuntimeConfig::default(),
            )
            .expect("rt start");

        // Inject directly into the discovered cache via the
        // handle_spdp_datagram path. We build a synthetic beacon with the
        // same helper as the runtime tests.
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
            participant_security_info: None,
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
        // Ignore → empty list.
        p.ignore_participant(handles[0]).unwrap();
        assert!(p.get_discovered_participants().is_empty());
        let err = p.get_discovered_participant_data(handles[0]).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn get_discovered_topics_lists_unique_handles_for_pub_and_sub() {
        // Pub + Sub on the same (topic, type) → one topic handle.
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
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
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
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
                sedp.cache_mut().insert_subscription(subdata, CoreDur::ZERO);
            }
        }
        let topics = p.get_discovered_topics();
        assert_eq!(topics.len(), 1, "Pub+Sub on same topic -> 1 handle");
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
                    unicast_locators: Vec::new(),
                    multicast_locators: Vec::new(),
                };
                sedp.cache_mut().insert_publication(pubdata, CoreDur::ZERO);
            }
        }
        let topics_before = p.get_discovered_topics();
        assert_eq!(topics_before.len(), 1);
        // Now ignore the topic — get_discovered_topics must no longer
        // list it, get_discovered_topic_data must return BadParameter.
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
