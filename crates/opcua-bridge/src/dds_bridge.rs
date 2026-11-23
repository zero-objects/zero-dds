// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS side of the daemon: one [`DcpsRuntime`] per configured domain,
//! raw-byte reader/writer registration via `DcpsRuntime::register_user_
//! reader_kind`/`register_user_writer_kind` — the same primitive
//! `crates/xrce-agent`'s daemon and `crates/routing-service` use for a
//! topic with no compiled Rust type — and a [`SerializedTypeSupport`] per
//! topic, built by [`crate::idl`] from the operator's own `.idl` file.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_dcps::participant::bridge_guid_prefix;
use zerodds_dcps::runtime::{
    DcpsRuntime, DiscoveredEndpointInfo, RuntimeConfig, UserReaderConfig, UserSample,
    UserWriterConfig,
};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

/// A DDS-side operation failed.
#[derive(Debug, Clone)]
pub enum DdsBridgeError {
    /// `DcpsRuntime::start` failed.
    Start(String),
    /// Reader/writer registration failed.
    Register(String),
    /// A write against an unregistered writer.
    UnknownWriter,
}

impl core::fmt::Display for DdsBridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Start(m) => write!(f, "DcpsRuntime::start: {m}"),
            Self::Register(m) => write!(f, "registration failed: {m}"),
            Self::UnknownWriter => write!(f, "write against an unregistered writer"),
        }
    }
}

impl std::error::Error for DdsBridgeError {}

/// One buffered sample, with the wire coordinates
/// [`SerializedTypeSupport::read_body`] needs to decode it: the DDS
/// representation (`0` = XCDR1, `2` = XCDR2) and byte order a foreign
/// writer chose, taken off the encapsulation header before this daemon
/// ever sees the body — dropping them here would silently mis-decode any
/// peer that is not this runtime's own little-endian-XCDR2 default.
#[derive(Debug, Clone)]
pub struct BufferedSample {
    /// CDR body, without its encapsulation header.
    pub body: Vec<u8>,
    /// `0` = XCDR1, `2` = XCDR2.
    pub representation: u8,
    /// `true` = big-endian.
    pub big_endian: bool,
    /// The representation identifier of the header the sample came under
    /// (DDSI-RTPS 2.5 §10.5): which of CDR2, D_CDR2 or PL_CDR2 it is.
    pub representation_identifier: [u8; 2],
    /// Source timestamp from the writer's INFO_TS submessage
    /// (DDSI-RTPS §8.7.3), if any — `None` when the writer sent none, in
    /// which case the caller uses its own receipt-time clock instead of
    /// pretending every sample arrived at the same instant (Spec
    /// §9.3.4.4's historizing samples are ordered/keyed by this).
    pub source_timestamp: Option<zerodds_rtps::header_extension::HeTimestamp>,
}

/// A registered reader: its `EntityId` plus the buffer its pump thread
/// fills. Cloning shares the buffer (both directions of `poll` see the same
/// samples), matching `crates/xrce-agent`'s daemon.
#[derive(Clone)]
pub struct ReaderHandle {
    entity_id: EntityId,
    buffer: Arc<Mutex<Vec<BufferedSample>>>,
    type_support: Option<Arc<SerializedTypeSupport>>,
    replaced: Arc<AtomicU64>,
}

/// How many samples one reader's buffer holds between two [`ReaderHandle::poll`]
/// calls — the bound on what a side that stops draining can make the bridge
/// hold. At the bound the oldest sample gives way to the newest (the bridge
/// maps a topic's current value), and the handle counts it.
pub const READER_BUFFER_CAP: usize = 4096;

impl ReaderHandle {
    /// The registered `EntityId`.
    #[must_use]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    /// This reader's [`SerializedTypeSupport`], if the topic's type was
    /// resolved.
    #[must_use]
    pub fn type_support(&self) -> Option<&Arc<SerializedTypeSupport>> {
        self.type_support.as_ref()
    }

    /// How many samples gave way to a newer one because the buffer was at
    /// [`READER_BUFFER_CAP`] — each one a sample the bridge never mapped.
    #[must_use]
    pub fn replaced(&self) -> u64 {
        self.replaced.load(Ordering::Relaxed)
    }

    /// Drains every sample buffered since the last call. Never blocks.
    pub fn poll(&self) -> Vec<BufferedSample> {
        self.buffer
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default()
    }
}

/// A registered writer.
#[derive(Clone, Copy)]
pub struct WriterHandle {
    entity_id: EntityId,
}

impl WriterHandle {
    /// The registered `EntityId`.
    #[must_use]
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }
}

/// One DDS domain's runtime plus every reader/writer this daemon has
/// registered on it.
pub struct DdsBridge {
    domain_id: i32,
    runtime: Arc<DcpsRuntime>,
    // Behind a lock: a topic whose type arrives through TypeLookup gets its
    // reader or writer while the daemon runs, through the bridge it shares.
    readers: Mutex<HashMap<String, ReaderHandle>>,
    writers: Mutex<HashMap<String, WriterEntry>>,
}

/// A registered writer with the support its bodies are written with.
type WriterEntry = (WriterHandle, Option<Arc<SerializedTypeSupport>>);

impl DdsBridge {
    /// Starts a [`DcpsRuntime`] for `domain_id`.
    ///
    /// # Errors
    /// [`DdsBridgeError::Start`].
    pub fn start(
        domain_id: i32,
        process_name: &str,
        config: RuntimeConfig,
    ) -> Result<Self, DdsBridgeError> {
        let prefix = bridge_guid_prefix(&std::format!("{process_name}-domain-{domain_id}"));
        let runtime = DcpsRuntime::start(domain_id, prefix, config)
            .map_err(|e| DdsBridgeError::Start(std::format!("{e:?}")))?;
        Ok(Self {
            domain_id,
            runtime,
            readers: Mutex::new(HashMap::new()),
            writers: Mutex::new(HashMap::new()),
        })
    }

    /// The DDS domain id this bridge runs on.
    #[must_use]
    pub fn domain_id(&self) -> i32 {
        self.domain_id
    }

    /// The underlying runtime, for callers that need a capability this
    /// wrapper does not expose yet (e.g. `type_lookup_server` for
    /// [`crate::dds_bridge::resolve_discovered`]).
    #[must_use]
    pub fn runtime(&self) -> &Arc<DcpsRuntime> {
        &self.runtime
    }

    /// Registers a reader on `topic_name`/`type_name`, buffering samples
    /// for [`ReaderHandle::poll`]. Idempotent: a second call for the same
    /// topic name returns the existing handle rather than a second reader.
    ///
    /// # Errors
    /// [`DdsBridgeError::Register`].
    pub fn reader(
        &mut self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        reliable: bool,
        type_support: Option<Arc<SerializedTypeSupport>>,
    ) -> Result<ReaderHandle, DdsBridgeError> {
        self.add_reader(topic_name, type_name, keyed, reliable, type_support)
    }

    /// [`Self::reader`] through a shared bridge, for a topic whose type
    /// arrived while the daemon runs.
    ///
    /// # Errors
    /// [`DdsBridgeError::Register`].
    pub fn add_reader(
        &self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        reliable: bool,
        type_support: Option<Arc<SerializedTypeSupport>>,
    ) -> Result<ReaderHandle, DdsBridgeError> {
        let mut readers = self
            .readers
            .lock()
            .map_err(|_| DdsBridgeError::Register("reader table poisoned".to_string()))?;
        if let Some(existing) = readers.get(topic_name) {
            return Ok(existing.clone());
        }
        let cfg = UserReaderConfig {
            topic_name: topic_name.to_string(),
            type_name: type_name.to_string(),
            reliable,
            type_identifier: self.announce_type(type_support.as_deref())?,
            ..Default::default()
        };
        let (entity_id, rx) = self
            .runtime
            .register_user_reader_kind(cfg, keyed)
            .map_err(|e| DdsBridgeError::Register(std::format!("{e:?}")))?;
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let replaced = Arc::new(AtomicU64::new(0));
        spawn_pump(Arc::clone(&buffer), Arc::clone(&replaced), rx);
        let handle = ReaderHandle {
            entity_id,
            buffer,
            type_support,
            replaced,
        };
        readers.insert(topic_name.to_string(), handle.clone());
        Ok(handle)
    }

    /// The `TypeIdentifier` an endpoint of `type_support`'s type announces,
    /// with the type's `TypeObject`s in the runtime's TypeLookup registry
    /// before the announcement goes out, so that a peer that asks for the
    /// type gets it (XTypes 1.3 §7.6.3.2, §7.6.3.3). Without a support the
    /// endpoint announces no type.
    fn announce_type(
        &self,
        type_support: Option<&SerializedTypeSupport>,
    ) -> Result<zerodds_types::TypeIdentifier, DdsBridgeError> {
        let Some(support) = type_support else {
            return Ok(zerodds_types::TypeIdentifier::None);
        };
        for object in support.type_objects() {
            self.runtime
                .register_type_object(object.clone())
                .map_err(|e| DdsBridgeError::Register(std::format!("type object: {e:?}")))?;
        }
        Ok(support.type_identifier().clone())
    }

    /// Registers a writer on `topic_name`/`type_name`. Idempotent like
    /// [`Self::reader`].
    ///
    /// # Errors
    /// [`DdsBridgeError::Register`].
    pub fn writer(
        &mut self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        reliable: bool,
        type_support: Option<Arc<SerializedTypeSupport>>,
    ) -> Result<WriterHandle, DdsBridgeError> {
        self.add_writer(topic_name, type_name, keyed, reliable, type_support)
    }

    /// [`Self::writer`] through a shared bridge, for a topic whose type
    /// arrived while the daemon runs.
    ///
    /// # Errors
    /// [`DdsBridgeError::Register`].
    pub fn add_writer(
        &self,
        topic_name: &str,
        type_name: &str,
        keyed: bool,
        reliable: bool,
        type_support: Option<Arc<SerializedTypeSupport>>,
    ) -> Result<WriterHandle, DdsBridgeError> {
        let mut writers = self
            .writers
            .lock()
            .map_err(|_| DdsBridgeError::Register("writer table poisoned".to_string()))?;
        if let Some((existing, _)) = writers.get(topic_name) {
            return Ok(*existing);
        }
        let cfg = UserWriterConfig {
            topic_name: topic_name.to_string(),
            type_name: type_name.to_string(),
            reliable,
            durability: Default::default(),
            ownership: Default::default(),
            type_identifier: self.announce_type(type_support.as_deref())?,
            ..Default::default()
        };
        let entity_id = self
            .runtime
            .register_user_writer_kind(cfg, keyed)
            .map_err(|e| DdsBridgeError::Register(std::format!("{e:?}")))?;
        if let Some(ts) = &type_support {
            // The bodies `write` takes are XCDR2 of the topic type: the
            // writer offers XCDR2 and frames them under the header of the
            // type's extensibility (DDS-XTypes 1.3 §7.6.3.1.2, Table 60:
            // CDR2, D_CDR2 or PL_CDR2), not under the one of a @final type.
            use zerodds_dcps::dds_type::Extensibility;
            use zerodds_types::qos::ExtensibilityForRepr as Ext;
            let map = |e| DdsBridgeError::Register(std::format!("{e:?}"));
            self.runtime
                .set_user_writer_data_rep_override(
                    entity_id,
                    Some(vec![
                        zerodds_rtps::publication_data::data_representation::XCDR2,
                    ]),
                )
                .map_err(map)?;
            let ext = match ts.extensibility() {
                Extensibility::Final => Ext::Final,
                Extensibility::Appendable => Ext::Appendable,
                Extensibility::Mutable => Ext::Mutable,
            };
            self.runtime
                .set_user_writer_wire_extensibility(entity_id, ext)
                .map_err(map)?;
        }
        let handle = WriterHandle { entity_id };
        writers.insert(topic_name.to_string(), (handle, type_support));
        Ok(handle)
    }

    /// Writes one already-encoded CDR body, in this daemon's own canonical
    /// wire form (XCDR2, little-endian — representation `2`/`big_endian
    /// false`, exactly what [`SerializedTypeSupport::from_dynamic_data`]
    /// plus [`SerializedTypeSupport::write_body(_, 2, false)`] produce; this
    /// method never re-encodes, only re-reads that same body to recompute
    /// its `KeyHash`). Uses the writer's [`SerializedTypeSupport`] (if
    /// registered with one) to compute a real `KeyHash`; without one the
    /// sample is written unkeyed, opaque.
    ///
    /// # Errors
    /// [`DdsBridgeError::UnknownWriter`] / [`DdsBridgeError::Register`] (the
    /// runtime rejected the write).
    pub fn write(&self, topic_name: &str, body: &[u8]) -> Result<(), DdsBridgeError> {
        let (handle, type_support) = self
            .writers
            .lock()
            .map_err(|_| DdsBridgeError::Register("writer table poisoned".to_string()))?
            .get(topic_name)
            .cloned()
            .ok_or(DdsBridgeError::UnknownWriter)?;
        let key_hash = type_support
            .as_ref()
            .and_then(|ts| ts.read_body(body, 2, false).ok())
            .and_then(|sample| ts_key(&type_support, &sample));
        match key_hash {
            Some(hash) => self
                .runtime
                .write_user_sample_keyed(handle.entity_id, body, hash, None)
                .map_err(|e| {
                    DdsBridgeError::Register(std::format!("write_user_sample_keyed: {e:?}"))
                }),
            None => self
                .runtime
                .write_user_sample_borrowed(handle.entity_id, body)
                .map_err(|e| DdsBridgeError::Register(std::format!("write_user_sample: {e:?}"))),
        }
    }

    /// Every remote publication SEDP has discovered on this domain, merged
    /// with every remote subscription — the "discovered DDS topics" this
    /// daemon can offer a browsable AddressSpace container for even before
    /// (or without) a matching config entry resolves its type.
    #[must_use]
    pub fn discovered_topics(&self) -> Vec<DiscoveredEndpointInfo> {
        let mut out = self.runtime.discovered_publication_endpoints();
        out.extend(self.runtime.discovered_subscription_endpoints());
        out
    }
}

fn ts_key(
    type_support: &Option<Arc<SerializedTypeSupport>>,
    sample: &zerodds_dcps::serialized_type::SerializedSample,
) -> Option<[u8; 16]> {
    type_support
        .as_ref()
        .and_then(|ts| ts.key_material(sample))
        .map(|(hash, _)| hash)
}

fn spawn_pump(
    buffer: Arc<Mutex<Vec<BufferedSample>>>,
    replaced: Arc<AtomicU64>,
    rx: Receiver<UserSample>,
) {
    std::thread::spawn(move || {
        while let Ok(sample) = rx.recv() {
            let UserSample::Alive {
                payload,
                representation,
                big_endian,
                representation_identifier,
                source_timestamp,
                ..
            } = sample
            else {
                continue; // lifecycle/coherent-set markers: no live "current value" update for a dispose.
            };
            let Ok(mut guard) = buffer.lock() else { break };
            // An AddressSpace/PubSub side that stops draining must not let
            // this grow without bound: at the cap the oldest sample gives
            // way and is counted — the newest value is the one to map.
            if guard.len() >= READER_BUFFER_CAP {
                guard.remove(0);
                replaced.fetch_add(1, Ordering::Relaxed);
            }
            guard.push(BufferedSample {
                body: payload.to_vec(),
                // `UserSample` tags XCDR2 as 1; this buffer, like
                // `SerializedTypeSupport::read_body`, names it 2.
                representation: zerodds_dcps::serialized_type::read_body_representation(
                    representation,
                ),
                big_endian,
                representation_identifier,
                source_timestamp,
            });
        }
    });
}

/// Resolves a [`SerializedTypeSupport`] for a discovered endpoint's
/// complete `TypeIdentifier` ([`DiscoveredEndpointInfo::
/// complete_type_identifier`]) through the runtime's own TypeLookup
/// registry (`DcpsRuntime::type_lookup_server`), filled by
/// `DcpsRuntime::register_type_object` and by the `getTypes` replies the
/// runtime receives, among them the ones [`TypeQuery`] asks for. `None`
/// until the registry holds the type and every type it depends on.
#[must_use]
pub fn resolve_discovered(
    bridge: &DdsBridge,
    type_identifier: &zerodds_types::TypeIdentifier,
    type_name: &str,
) -> Option<Arc<SerializedTypeSupport>> {
    let zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash) = type_identifier else {
        return None;
    };
    let server = bridge.runtime().type_lookup_server.lock().ok()?;
    let root = server.registry.get_complete(hash)?.clone();
    let dependency_hashes = server
        .registry
        .transitive_dependencies(hash, zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES);
    let mut dependencies = Vec::with_capacity(dependency_hashes.len());
    for dep_hash in &dependency_hashes {
        dependencies.push(server.registry.get_complete(dep_hash)?.clone());
    }
    drop(server);
    SerializedTypeSupport::from_complete_type_objects(type_name, root, dependencies).ok()
}

/// How long a [`TypeQuery`] waits for an answer before it asks the same
/// peer for the same type again: a request or a reply may be lost
/// (the TypeLookup endpoints are RELIABLE, the peer may have gone).
pub const TYPE_QUERY_RETRY: Duration = Duration::from_secs(5);

/// Learns the type of a topic from the remote endpoints of the topic
/// (spec §5.1): each endpoint that announces a complete `TypeIdentifier`
/// is asked for the type and its dependencies over TypeLookup (XTypes 1.3
/// §7.6.3.3.4, `getTypes` and `getTypeDependencies`), and the type is
/// there once the runtime's registry holds it all.
#[derive(Debug)]
pub struct TypeQuery {
    topic: String,
    /// The peers asked, with the type asked for and when.
    asked: Vec<(GuidPrefix, zerodds_types::TypeIdentifier, Instant)>,
}

/// What [`TypeQuery::poll`] found.
#[derive(Debug, Clone)]
pub enum TypeQueryState {
    /// The type: its support, named as the endpoint that had it names it.
    Resolved(Arc<SerializedTypeSupport>),
    /// Not yet: how many remote endpoints of the topic there are, and how
    /// many of them announce no complete type to ask for.
    Waiting {
        /// Remote endpoints of the topic.
        endpoints: usize,
        /// Of those, the ones without a complete `TypeIdentifier`.
        without_type: usize,
    },
}

impl TypeQuery {
    /// A query for the type of `topic`.
    #[must_use]
    pub fn new(topic: &str) -> Self {
        Self {
            topic: topic.to_string(),
            asked: Vec::new(),
        }
    }

    /// Looks at the remote endpoints of the topic: the first whose type the
    /// registry holds gives it; the others are asked for theirs, each at
    /// most once per [`TYPE_QUERY_RETRY`].
    pub fn poll(&mut self, bridge: &DdsBridge, now: Instant) -> TypeQueryState {
        let own = bridge.runtime().guid_prefix;
        let mut endpoints = 0;
        let mut without_type = 0;
        for endpoint in bridge.discovered_topics() {
            let mut prefix = [0u8; 12];
            prefix.copy_from_slice(&endpoint.endpoint_guid[..12]);
            let peer = GuidPrefix::from_bytes(prefix);
            if endpoint.topic_name != self.topic || peer == own {
                continue;
            }
            endpoints += 1;
            let Some(id) = endpoint.complete_type_identifier.as_ref() else {
                without_type += 1;
                continue;
            };
            if let Some(support) = resolve_discovered(bridge, id, &endpoint.type_name) {
                return TypeQueryState::Resolved(support);
            }
            let recent = self.asked.iter().any(|(asked_peer, asked_id, at)| {
                *asked_peer == peer
                    && asked_id == id
                    && now.saturating_duration_since(*at) < TYPE_QUERY_RETRY
            });
            if recent {
                continue;
            }
            // A request that does not go out (no locator for the peer yet)
            // is asked again with the next poll.
            let runtime = bridge.runtime();
            let sent = runtime
                .send_type_lookup_request(peer, core::slice::from_ref(id))
                .ok()
                .flatten()
                .is_some();
            let _ = runtime.send_type_dependencies_request(peer, core::slice::from_ref(id));
            if sent {
                self.asked
                    .retain(|(asked_peer, asked_id, _)| !(*asked_peer == peer && asked_id == id));
                self.asked.push((peer, id.clone(), now));
            }
        }
        TypeQueryState::Waiting {
            endpoints,
            without_type,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {

    fn alive(index: u8, representation: u8) -> UserSample {
        UserSample::Alive {
            key_hash: None,
            payload: zerodds_dcps::sample_bytes::SampleBytes::from_vec(vec![index]),
            writer_guid: [0; 16],
            writer_strength: 0,
            representation,
            big_endian: false,
            representation_identifier: [0x00, if representation == 0 { 0x01 } else { 0x07 }],
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: -1,
            coherent_set_sn: None,
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        }
    }

    fn wait_for(buffer: &Arc<Mutex<Vec<BufferedSample>>>, n: usize) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while buffer.lock().unwrap().len() < n && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// `UserSample` tags XCDR2 as 1, `SerializedTypeSupport::read_body`
    /// takes 2: the buffer carries the latter, or every XCDR2 sample fails
    /// to decode in the bridge.
    #[test]
    fn opcuabridge_6_1_the_pump_names_xcdr2_the_way_read_body_takes_it() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let replaced = Arc::new(AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_pump(Arc::clone(&buffer), Arc::clone(&replaced), rx);
        tx.send(alive(0, 0)).unwrap();
        tx.send(alive(1, 1)).unwrap();
        wait_for(&buffer, 2);
        let reps: Vec<u8> = buffer
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.representation)
            .collect();
        assert_eq!(reps, vec![0, 2]);
    }

    /// At the bound the oldest sample gives way to the newest and is
    /// counted, where it used to be the newest that vanished silently.
    #[test]
    fn at_the_bound_the_oldest_sample_gives_way_and_is_counted() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let replaced = Arc::new(AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_pump(Arc::clone(&buffer), Arc::clone(&replaced), rx);
        for i in 0..=READER_BUFFER_CAP {
            tx.send(alive((i % 251) as u8, 0)).unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while replaced.load(Ordering::Relaxed) < 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let guard = buffer.lock().unwrap();
        assert_eq!(guard.len(), READER_BUFFER_CAP);
        assert_eq!(replaced.load(Ordering::Relaxed), 1);
        assert_eq!(guard[0].body, vec![1], "the first sample gave way");
        assert_eq!(
            guard[READER_BUFFER_CAP - 1].body,
            vec![(READER_BUFFER_CAP % 251) as u8],
            "the newest sample is kept"
        );
    }

    use super::*;

    /// DDS-XTypes 1.3 §7.6.3.1.2, Table 60, with DDSI-RTPS 2.5 §10.5: a
    /// sample the bridge writes goes out under the header of its type's
    /// extensibility, CDR2_LE (0x0007) for `@final`, D_CDR2_LE (0x0009)
    /// for `@appendable`, PL_CDR2_LE (0x000b) for `@mutable`.
    #[test]
    fn xtypes_7_6_3_1_2_a_bridge_sample_goes_out_under_the_header_of_its_type() {
        use zerodds_types::dynamic::DynamicData;
        let lease = zerodds_test_domains::lease_domain();
        let ast = Arc::new(
            zerodds_wire_probe::encode::model_from_idl(
                "@final struct F { long x; };\n@appendable struct A { long x; };\n\
                 @mutable struct M { long x; };\n",
                "header_test.idl",
            )
            .expect("idl parses"),
        );
        let mut writer =
            DdsBridge::start(lease.id(), "opcua-header-writer", RuntimeConfig::default())
                .expect("start");
        let mut reader =
            DdsBridge::start(lease.id(), "opcua-header-reader", RuntimeConfig::default())
                .expect("start");
        for (name, identifier) in [
            ("F", [0x00, 0x07]),
            ("A", [0x00, 0x09]),
            ("M", [0x00, 0x0b]),
        ] {
            let ts = crate::idl::type_support_for(&[Arc::clone(&ast)], name).expect("support");
            let w = writer
                .writer(name, name, false, true, Some(Arc::clone(&ts)))
                .expect("writer");
            let r = reader
                .reader(name, name, false, true, Some(Arc::clone(&ts)))
                .expect("reader");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while writer.runtime.user_writer_matched_count(w.entity_id) < 1 {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                assert!(!left.is_zero(), "{name}: the reader matched");
                writer.runtime.wait_match_event(left);
            }
            let sample = ts
                .from_dynamic_data(DynamicData::Aggregate(vec![(
                    "x".to_string(),
                    DynamicData::Integer {
                        kind: Some(zerodds_ast::PrimitiveKind::Int32),
                        value: 5,
                    },
                )]))
                .expect("sample");
            writer
                .write(name, &ts.write_body(&sample, 2, false).expect("body"))
                .expect("write");
            let got = loop {
                let got = r.poll();
                if !got.is_empty() {
                    break got;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "{name}: the sample arrived"
                );
                // The reader's pump fills a buffer the bridge drains by
                // polling; this is the interval of that poll.
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            assert_eq!(got[0].representation_identifier, identifier, "{name}");
        }
    }

    #[test]
    fn opcuabridge_6_2_write_against_unknown_topic_is_reported() {
        let lease = zerodds_test_domains::lease_domain();
        let bridge = DdsBridge::start(
            lease.id(),
            "zerodds-opcua-bridged-test",
            RuntimeConfig::default(),
        )
        .expect("start");
        assert!(matches!(
            bridge.write("Nope", &[]),
            Err(DdsBridgeError::UnknownWriter)
        ));
    }
}
