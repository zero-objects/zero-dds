// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Part 14 UADP wiring: a [`Publisher`] per configured DDS-topic-to-DataSet
//! mapping (DDS → PubSub) and a [`Subscriber`] per DataSet-to-DDS-topic
//! mapping (PubSub → DDS), both driven by the same
//! [`crate::dynamic_value::flatten`]/[`crate::dynamic_value::unflatten`]
//! pair the AddressSpace side uses.

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_opcua_gateway::data_value::DataValue;
use zerodds_opcua_pubsub::config::{
    ConfigurationVersion, DataSetMetaData, DataSetReaderConfig, DataSetWriterConfig, FieldMetaData,
    NetworkMessageContentMask, WriterGroupConfig,
};
use zerodds_opcua_pubsub::daemon::{Publisher, Subscriber};
use zerodds_opcua_pubsub::reader::{DataSetReader, ReaderGroup};
use zerodds_opcua_pubsub::transport::UdpTransport;
use zerodds_opcua_pubsub::uadp::network_message::PublisherId;
use zerodds_opcua_pubsub::writer::{DataSetWriter, PublishedDataSet, WriterGroup};
use zerodds_types::dynamic::DynamicType;

use crate::dds_bridge::{DdsBridge, ReaderHandle, WriterHandle};
use crate::dynamic_value::{self, FlatField};

/// A PubSub-side operation failed.
#[derive(Debug)]
pub enum PubSubGlueError {
    /// The type's Table 9 default could not be built ([`dynamic_value::default_data`]).
    Default(String),
    /// [`dynamic_value::flatten`]/[`dynamic_value::unflatten`] refused the
    /// shape.
    Convert(dynamic_value::ConvertError),
    /// The UDP transport could not be bound/joined.
    Transport(String),
    /// The DDS side ([`crate::dds_bridge::DdsBridgeError`]).
    Dds(String),
    /// The type could not be decoded/encoded ([`SerializedTypeSupport`]).
    Codec(String),
    /// A DataSet field names no part of the topic's type.
    Field(String),
}

impl core::fmt::Display for PubSubGlueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Default(m) => write!(f, "default value: {m}"),
            Self::Convert(e) => write!(f, "{e}"),
            Self::Transport(m) => write!(f, "transport: {m}"),
            Self::Dds(m) => write!(f, "DDS: {m}"),
            Self::Codec(m) => write!(f, "codec: {m}"),
            Self::Field(path) => write!(f, "DataSet field `{path}` names no part of the type"),
        }
    }
}

impl std::error::Error for PubSubGlueError {}

impl From<dynamic_value::ConvertError> for PubSubGlueError {
    fn from(e: dynamic_value::ConvertError) -> Self {
        Self::Convert(e)
    }
}

/// Publishes DDS topic `dds_topic`'s samples as a UADP DataSet.
pub struct PublisherJob {
    dds_topic: String,
    dyn_type: DynamicType,
    reader: ReaderHandle,
    type_support: Arc<SerializedTypeSupport>,
    publisher: Publisher<UdpTransport>,
    dataset_name: String,
    interval: Duration,
    next_publish: Instant,
    /// The `MinorVersion` the DataSetWriter advertises since the last
    /// field was appended; 0 until then.
    minor_version: u32,
    #[cfg(feature = "pubsub-security")]
    security: Option<SharedKeys>,
}

/// The most fields a PublishedDataSet grows to while the daemon runs; the
/// fields past the type's own come from the samples of remote writers.
pub const MAX_DATASET_FIELDS: usize = 4096;

/// `VersionTime` (Part 14 §6.2.3.2.5, Part 4 §7.38): seconds since
/// 2000-01-01T00:00:00Z.
fn version_time_now() -> u32 {
    const Y2000: u64 = 946_684_800;
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u32::try_from(since.as_secs().saturating_sub(Y2000)).unwrap_or(u32::MAX)
        })
}

impl PublisherJob {
    /// Builds a publisher: registers the DDS reader on `dds_bridge`, builds
    /// the initial `PublishedDataSet` from the type's Table 9 default, and
    /// binds `transport`.
    ///
    /// # Errors
    /// [`PubSubGlueError`] — see its variants.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dds_bridge: &DdsBridge,
        dds_topic: String,
        type_name: &str,
        dyn_type: DynamicType,
        type_support: Arc<SerializedTypeSupport>,
        keyed: bool,
        transport: UdpTransport,
        publisher_id: u16,
        writer_group_id: u16,
        dataset_writer_id: u16,
        dataset_name: String,
        publish_interval: Duration,
    ) -> Result<Self, PubSubGlueError> {
        let reader = dds_bridge
            .add_reader(
                &dds_topic,
                type_name,
                keyed,
                false,
                Some(Arc::clone(&type_support)),
            )
            .map_err(|e| PubSubGlueError::Dds(e.to_string()))?;

        let default = dynamic_value::default_data(&dyn_type)
            .map_err(|e| PubSubGlueError::Default(e.to_string()))?;
        let (fields, _skipped) = dynamic_value::flatten(&dyn_type, &default)?;

        let mut pds = PublishedDataSet::new(dataset_name.clone());
        for field in &fields {
            pds.add_field(
                field_name(field),
                DataValue::new_value(field.value.clone(), 0, 0),
            );
        }
        // OPC 10000-14 §7.2.4.4.2 Table 154: the PublisherId goes out only
        // when the NetworkMessageContentMask names it. The connection's
        // `publisher_id` is there to be sent, so the mask names it; without
        // it a Subscriber that filters by PublisherId (§6.2.9.1) never
        // matched this publisher.
        let mut group_config =
            WriterGroupConfig::new(std::format!("{dds_topic}-wg"), writer_group_id);
        group_config.network_message_content_mask = NetworkMessageContentMask::from_bits(
            group_config.network_message_content_mask.bits()
                | NetworkMessageContentMask::PUBLISHER_ID,
        );
        let group = WriterGroup::new(group_config, PublisherId::UInt16(publisher_id));
        let mut publisher = Publisher::new(transport, group);
        publisher.add_dataset(pds);
        publisher.add_writer(DataSetWriter::new(
            DataSetWriterConfig::new(
                std::format!("{dds_topic}-w"),
                dataset_writer_id,
                dataset_name.clone(),
            ),
            ConfigurationVersion::default(),
        ));

        Ok(Self {
            dds_topic,
            dyn_type,
            reader,
            type_support,
            publisher,
            dataset_name,
            interval: publish_interval,
            next_publish: Instant::now(),
            minor_version: 0,
            #[cfg(feature = "pubsub-security")]
            security: None,
        })
    }

    /// The fields of the PublishedDataSet, in their order, and the
    /// `MinorVersion` its DataSetWriter advertises.
    #[must_use]
    pub fn dataset_fields(&mut self) -> (Vec<String>, u32) {
        let fields = self
            .publisher
            .dataset_mut(&self.dataset_name)
            .map(|dataset| dataset.field_names().to_vec())
            .unwrap_or_default();
        let minor_version = self
            .publisher
            .writers()
            .first()
            .map_or(0, |writer| writer.config_version().minor_version);
        (fields, minor_version)
    }

    /// This job's DDS topic name.
    #[must_use]
    pub fn dds_topic(&self) -> &str {
        &self.dds_topic
    }

    /// Installs Part 14 §8 UADP message security: every subsequent
    /// [`Self::tick`] that publishes calls
    /// [`Publisher::publish_cycle_secured`] instead of the plain
    /// `publish_cycle`.
    #[cfg(feature = "pubsub-security")]
    pub fn set_security(
        &mut self,
        policy: zerodds_opcua_pubsub::security::SecurityPolicy,
        sks: zerodds_opcua_pubsub::security::SecurityKeyService,
    ) {
        self.security = Some(KeyState::fixed(policy, sks));
    }

    /// Installs Part 14 §8 UADP message security with keys that a key
    /// source fills and rotates (spec §4.3): while there are none, nothing
    /// goes through.
    #[cfg(feature = "pubsub-security")]
    pub fn set_keys(&mut self, keys: SharedKeys) {
        self.security = Some(keys);
    }

    /// Drains every DDS sample buffered since the last call, applies the
    /// most recent to the PublishedDataSet, and — if `interval` elapsed —
    /// publishes one UADP NetworkMessage. Returns
    /// `(samples applied, published)`.
    ///
    /// # Errors
    /// [`PubSubGlueError`] on a decode/convert/transport failure.
    pub fn tick(
        &mut self,
        now: Instant,
        on_skip: &mut dyn FnMut(&str, &dynamic_value::ConvertError),
    ) -> Result<(usize, bool), PubSubGlueError> {
        let samples = self.reader.poll();
        let mut applied = 0;
        for sample in &samples {
            let decoded = self
                .type_support
                .read_body(&sample.body, sample.representation, sample.big_endian)
                .map_err(PubSubGlueError::Codec)?;
            let (fields, skipped) = dynamic_value::flatten(&self.dyn_type, decoded.data())?;
            for skip in &skipped {
                on_skip(&self.dds_topic, skip);
            }
            let Some(dataset) = self.publisher.dataset_mut(&self.dataset_name) else {
                return Err(PubSubGlueError::Dds(std::format!(
                    "PublishedDataSet `{}` missing",
                    self.dataset_name
                )));
            };
            // A field the DataSet does not have is a path the type's default
            // sample did not have: a new map key, a member of another union
            // branch, an optional member that is now there. It is appended
            // (spec §5.3; Part 14 §6.2.3.2.5: a new MinorVersion).
            let mut appended = Vec::new();
            for field in &fields {
                if !dataset.set_variant(field_name(field), field.value.clone()) {
                    appended.push(field);
                }
            }
            for field in appended {
                let known = self
                    .publisher
                    .dataset_mut(&self.dataset_name)
                    .map_or(0, |dataset| dataset.len());
                if known >= MAX_DATASET_FIELDS {
                    on_skip(
                        &self.dds_topic,
                        &dynamic_value::ConvertError::Unsupported {
                            path: field.path.clone(),
                            reason: "the PublishedDataSet has its most fields; the value is not published",
                        },
                    );
                    continue;
                }
                self.minor_version = version_time_now().max(self.minor_version.saturating_add(1));
                self.publisher.append_field(
                    &self.dataset_name,
                    field_name(field),
                    DataValue::new_value(field.value.clone(), 0, 0),
                    self.minor_version,
                );
            }
            applied += 1;
        }
        if now < self.next_publish {
            return Ok((applied, false));
        }
        self.next_publish = now + self.interval;
        #[cfg(feature = "pubsub-security")]
        if let Some(keys) = &self.security {
            let state = keys
                .lock()
                .map_err(|_| PubSubGlueError::Transport("key state poisoned".to_string()))?;
            // Part 14 §5.4.5.3: without keys the WriterGroup stays
            // PreOperational; nothing is published.
            let Some(service) = state.service.as_ref() else {
                return Ok((applied, false));
            };
            self.publisher
                .publish_cycle_secured(None, state.policy, service.current_key())
                .map_err(|e| PubSubGlueError::Transport(e.to_string()))?;
            return Ok((applied, true));
        }
        self.publisher
            .publish_cycle(None)
            .map_err(|e| PubSubGlueError::Transport(e.to_string()))?;
        Ok((applied, true))
    }
}

/// Subscribes a UADP DataSet and writes received samples into a DDS topic.
pub struct SubscriberJob {
    dds_topic: String,
    dyn_type: DynamicType,
    writer: WriterHandle,
    type_support: Arc<SerializedTypeSupport>,
    subscriber: Subscriber<UdpTransport>,
    last: zerodds_types::dynamic::DynamicData,
    /// The topic's type is a scalar: its one field is `value`, the empty
    /// path.
    scalar_root: bool,
    #[cfg(feature = "pubsub-security")]
    security: Option<SharedKeys>,
}

impl SubscriberJob {
    /// Builds a subscriber: registers the DDS writer on `dds_bridge` and the
    /// [`DataSetReader`] for `field_paths` (in [`crate::dynamic_value::flatten`]
    /// order — the same order the remote Publisher's own `PublishedDataSet`
    /// fields were added in).
    ///
    /// # Errors
    /// [`PubSubGlueError`] — see its variants.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dds_bridge: &DdsBridge,
        dds_topic: String,
        type_name: &str,
        dyn_type: DynamicType,
        type_support: Arc<SerializedTypeSupport>,
        keyed: bool,
        transport: UdpTransport,
        dataset_name: String,
        field_paths: &[String],
    ) -> Result<Self, PubSubGlueError> {
        let writer = dds_bridge
            .add_writer(
                &dds_topic,
                type_name,
                keyed,
                false,
                Some(Arc::clone(&type_support)),
            )
            .map_err(|e| PubSubGlueError::Dds(e.to_string()))?;

        let default = dynamic_value::default_data(&dyn_type)
            .map_err(|e| PubSubGlueError::Default(e.to_string()))?;
        let (fields, _) = dynamic_value::flatten(&dyn_type, &default)?;
        let scalar_root = fields.len() == 1 && fields[0].path.is_empty();
        let ddstype = crate::dynamic_type::to_dds_type(&dyn_type)
            .map_err(|e| PubSubGlueError::Default(e.to_string()))?;
        let meta = dataset_metadata(&dataset_name, &ddstype, field_paths)?;

        let mut group = ReaderGroup::new();
        group.add_reader(DataSetReader::new(DataSetReaderConfig::new(
            std::format!("{dds_topic}-r"),
            meta,
        )));
        let subscriber = Subscriber::new(transport, group);

        Ok(Self {
            dds_topic,
            dyn_type,
            writer,
            type_support,
            subscriber,
            last: default,
            scalar_root,
            #[cfg(feature = "pubsub-security")]
            security: None,
        })
    }

    /// This job's DDS topic name.
    #[must_use]
    pub fn dds_topic(&self) -> &str {
        &self.dds_topic
    }

    /// Installs Part 14 §8 UADP message security: every subsequent
    /// [`Self::tick`] calls [`Subscriber::poll_secured`] instead of the
    /// plain `poll`, refusing (not silently accepting as cleartext) any
    /// datagram that fails to verify/decrypt.
    #[cfg(feature = "pubsub-security")]
    pub fn set_security(
        &mut self,
        policy: zerodds_opcua_pubsub::security::SecurityPolicy,
        sks: zerodds_opcua_pubsub::security::SecurityKeyService,
    ) {
        self.security = Some(KeyState::fixed(policy, sks));
    }

    /// Installs Part 14 §8 UADP message security with keys that a key
    /// source fills and rotates (spec §4.3): while there are none, nothing
    /// goes through.
    #[cfg(feature = "pubsub-security")]
    pub fn set_keys(&mut self, keys: SharedKeys) {
        self.security = Some(keys);
    }

    /// Receives every pending UADP datagram, rebuilds the sample and writes
    /// it into DDS through `dds_bridge` (the same one [`Self::new`]
    /// registered this job's writer on). Returns the number of samples
    /// written.
    ///
    /// # Errors
    /// [`PubSubGlueError`] on a decode/convert/transport/DDS-write failure.
    pub fn tick(&mut self, dds_bridge: &DdsBridge) -> Result<usize, PubSubGlueError> {
        // Every DataSet that is there, not one a tick: a publisher that
        // sends faster than the daemon ticks left the socket filling and
        // its values ever further behind.
        let mut written = 0;
        for _ in 0..MAX_DATASETS_PER_TICK {
            let (count, matched) = self.receive_once(dds_bridge)?;
            written += count;
            if matched == 0 {
                break;
            }
        }
        Ok(written)
    }

    /// One receive: the DataSets of at most one NetworkMessage, written
    /// into DDS; the number written and the number matched. Nothing
    /// matched is the end of what is there (a read that timed out, or a
    /// message for no reader of this job).
    fn receive_once(&mut self, dds_bridge: &DdsBridge) -> Result<(usize, usize), PubSubGlueError> {
        #[cfg(feature = "pubsub-security")]
        let matched = if let Some(keys) = &self.security {
            let mut state = keys
                .lock()
                .map_err(|_| PubSubGlueError::Transport("key state poisoned".to_string()))?;
            // Without keys the DataSetReader stays PreOperational.
            let Some(service) = state.service.as_ref() else {
                return Ok((0, 0));
            };
            match self.subscriber.poll_secured(state.policy, service) {
                Ok(matched) => matched,
                // Part 14 §8.3.2: "If the CurrentTokenId in the message is
                // not recognized the receiver shall call this Method again
                // to get new keys."
                Err(zerodds_opcua_pubsub::daemon::DaemonError::Security(
                    zerodds_opcua_pubsub::security::SecurityError::UnknownToken(token),
                )) => {
                    state.unknown_token = Some(token);
                    return Err(PubSubGlueError::Transport(std::format!(
                        "no key for SecurityTokenId {token}"
                    )));
                }
                Err(e) => return Err(PubSubGlueError::Transport(e.to_string())),
            }
        } else {
            self.subscriber
                .poll()
                .map_err(|e| PubSubGlueError::Transport(e.to_string()))?
        };
        #[cfg(not(feature = "pubsub-security"))]
        let matched = self
            .subscriber
            .poll()
            .map_err(|e| PubSubGlueError::Transport(e.to_string()))?;
        let mut written = 0;
        for m in &matched {
            let fields: Vec<FlatField> = m
                .data
                .fields
                .iter()
                .filter_map(|f| {
                    f.value.value.clone().map(|v| FlatField {
                        // Only a scalar topic's one field is named `value`
                        // for the empty path; a struct's member `value` is
                        // the path `value`.
                        path: if self.scalar_root {
                            unmangle_field_name(&f.name)
                        } else {
                            f.name.clone()
                        },
                        value: v,
                    })
                })
                .collect();
            let data = dynamic_value::unflatten(&self.dyn_type, &fields, &self.last)?;
            let sample = self
                .type_support
                .from_dynamic_data(data.clone())
                .map_err(PubSubGlueError::Codec)?;
            let body = self
                .type_support
                .write_body(&sample, 2, false)
                .map_err(PubSubGlueError::Codec)?;
            dds_bridge
                .write(&self.dds_topic, &body)
                .map_err(|e| PubSubGlueError::Dds(e.to_string()))?;
            self.last = data;
            written += 1;
        }
        Ok((written, matched.len()))
    }

    /// The registered DDS `WriterHandle`.
    #[must_use]
    pub fn writer(&self) -> WriterHandle {
        self.writer
    }
}

/// The most NetworkMessages a subscriber job reads in one tick; the rest
/// waits for the next one.
pub const MAX_DATASETS_PER_TICK: usize = 256;

/// The keys of a secured PubSub job, shared with what supplies them: a
/// fixed key, a SecurityGroup of this daemon, a Security Key Service the
/// keys are pulled from, or one that pushes them (spec §4.3).
#[cfg(feature = "pubsub-security")]
#[derive(Debug)]
pub struct KeyState {
    /// The SecurityPolicy of the keys.
    pub policy: zerodds_opcua_pubsub::security::SecurityPolicy,
    /// The keys; `None` until the first ones arrive.
    pub service: Option<zerodds_opcua_pubsub::security::SecurityKeyService>,
    /// When the next key becomes the current one; `None` for keys that do
    /// not rotate.
    pub next_rotation: Option<Instant>,
    /// The lifetime of a key.
    pub key_lifetime: Duration,
    /// The last SecurityTokenId a subscriber got a message of and had no
    /// key for; what makes a puller fetch again.
    pub unknown_token: Option<u32>,
}

/// [`KeyState`] behind a lock.
#[cfg(feature = "pubsub-security")]
pub type SharedKeys = Arc<std::sync::Mutex<KeyState>>;

#[cfg(feature = "pubsub-security")]
impl KeyState {
    /// One key that does not rotate.
    #[must_use]
    pub fn fixed(
        policy: zerodds_opcua_pubsub::security::SecurityPolicy,
        service: zerodds_opcua_pubsub::security::SecurityKeyService,
    ) -> SharedKeys {
        Arc::new(std::sync::Mutex::new(Self {
            policy,
            service: Some(service),
            next_rotation: None,
            key_lifetime: Duration::ZERO,
            unknown_token: None,
        }))
    }

    /// No keys yet: they come from a Security Key Service.
    #[must_use]
    pub fn waiting(policy: zerodds_opcua_pubsub::security::SecurityPolicy) -> SharedKeys {
        Arc::new(std::sync::Mutex::new(Self {
            policy,
            service: None,
            next_rotation: None,
            key_lifetime: Duration::ZERO,
            unknown_token: None,
        }))
    }

    /// Takes keys a Security Key Service gave (Part 14 §8.3.2,
    /// §9.1.3.3): `keys` start at `first_token_id`, the first is the
    /// current one; the next becomes current after `time_to_next_key`,
    /// each one after that a `key_lifetime` later.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        group: &str,
        policy: zerodds_opcua_pubsub::security::SecurityPolicy,
        first_token_id: u32,
        keys: Vec<zerodds_opcua_pubsub::security::SecurityKey>,
        time_to_next_key: Duration,
        key_lifetime: Duration,
        now: Instant,
    ) {
        if keys.is_empty() {
            return;
        }
        let same_policy = policy == self.policy;
        self.policy = policy;
        match self.service.as_mut().filter(|_| same_policy) {
            Some(service) => service.apply_fetched(first_token_id, keys),
            None => {
                let mut keys = keys.into_iter();
                if let Some(current) = keys.next() {
                    let mut service = zerodds_opcua_pubsub::security::SecurityKeyService::new(
                        policy, group, current,
                    );
                    for key in keys {
                        service.push_future_key(key);
                    }
                    self.service = Some(service);
                }
            }
        }
        // `TimeToNextKey` is the time of the key `first_token_id`; a
        // current key further on (this side rotated before the SKS
        // answered) ends that many lifetimes later.
        let mut ahead = 0u32;
        if let Some(service) = self.service.as_mut() {
            service.set_key_lifetime_ms(key_lifetime.as_secs_f64() * 1000.0);
            let current = service.current_key().token_id;
            let mut token = first_token_id;
            while token != current && ahead <= 1 + service.future_keys().len() as u32 {
                token = zerodds_opcua_pubsub::security::next_token_id(token);
                ahead += 1;
            }
            if token != current {
                ahead = 0;
            }
        }
        self.key_lifetime = key_lifetime;
        self.next_rotation = Some(now + time_to_next_key + key_lifetime * ahead);
        self.unknown_token = None;
    }

    /// Makes the next key current when its time has come (Part 14
    /// §8.3.2: "The TimeToNextKey defines the time when to switch from the
    /// current key to the next key and the KeyLifetime defines when to
    /// switch from one future key to the next future key"). `true` when a
    /// key was rotated.
    pub fn rotate_due(&mut self, now: Instant) -> bool {
        let (Some(at), Some(service)) = (self.next_rotation, self.service.as_mut()) else {
            return false;
        };
        if now < at || self.key_lifetime.is_zero() {
            return false;
        }
        // Every key whose time is over, on the schedule and not from the
        // moment this runs: a late call does not shift the keys after it.
        let mut at = at;
        let mut rotated = false;
        while now >= at {
            rotated |= service.rotate();
            at += self.key_lifetime;
        }
        self.next_rotation = Some(at);
        rotated
    }

    /// The time until the next key becomes current.
    #[must_use]
    pub fn time_to_next_key(&self, now: Instant) -> Duration {
        self.next_rotation
            .map_or(self.key_lifetime, |at| at.saturating_duration_since(now))
    }
}

/// Sanitizes a [`FlatField`] path into a Part 14 field name (which must not
/// be empty — a scalar topic's single leaf, `flatten`'s `""` path, becomes
/// `"value"`).
fn field_name(field: &FlatField) -> &str {
    field_name_of(&field.path)
}

/// The `DataSetMetaData` of a DataSet of the fields `field_paths` of a
/// sample of `ty` (OPC 10000-14 §6.2.3.2): each field with the built-in
/// type it is transferred as, its DataType, ValueRank and ArrayDimensions
/// (Gateway 1.0 §9.2, [`crate::ua_types`]) and a `DataSetFieldId` of its
/// own; the namespaces the DataTypes are in and the descriptions of the
/// DataTypes that are no built-in types (§6.2.3.2.2).
///
/// # Errors
/// [`PubSubGlueError::Field`] for a path that names no part of the type.
pub fn dataset_metadata(
    dataset_name: &str,
    ty: &zerodds_opcua_gateway::dds_to_ua::walker::DdsType,
    field_paths: &[String],
) -> Result<DataSetMetaData, PubSubGlueError> {
    let ns = crate::daemon::server::NS;
    let fields = field_paths
        .iter()
        .map(|path| {
            let member = crate::ua_types::type_at_path(ty, path)
                .ok_or_else(|| PubSubGlueError::Field(path.clone()))?;
            let shape = crate::ua_types::shape_of(ns, member);
            let name = field_name_of(path);
            let mut field = FieldMetaData::scalar(name, crate::ua_types::builtin_of(member));
            field.data_type = shape.data_type;
            field.value_rank = shape.value_rank;
            field.array_dimensions = if shape.value_rank > 0 {
                shape.array_dimensions
            } else {
                Vec::new()
            };
            field.data_set_field_id = field_id(dataset_name, name);
            Ok(field)
        })
        .collect::<Result<Vec<_>, PubSubGlueError>>()?;
    let mut meta = DataSetMetaData::new(dataset_name, fields);
    // §6.2.3.2.2: entry 0 is namespace 1; the daemon's DataTypes are in
    // `NS`.
    meta.namespaces = vec![
        zerodds_opcua_server::server::APPLICATION_URI.to_string(),
        crate::ua_types::NAMESPACE_URI.to_string(),
    ];
    let (structures, enums) = crate::ua_types::type_descriptions(ns, ty);
    meta.structure_data_types = structures;
    meta.enum_data_types = enums;
    Ok(meta)
}

/// A `DataSetFieldId` (§6.2.3.2.3: "The unique ID for the field in the
/// DataSet"): the same for the same field of the same DataSet, whenever
/// the daemon starts. Two 64-bit FNV-1a hashes of the DataSet's and the
/// field's names.
fn field_id(dataset_name: &str, field_name: &str) -> zerodds_opcua_gateway::types::Guid {
    let fnv = |seed: u64| {
        let mut hash = seed;
        for byte in dataset_name
            .bytes()
            .chain(core::iter::once(0))
            .chain(field_name.bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash
    };
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&fnv(0xcbf2_9ce4_8422_2325).to_le_bytes());
    bytes[8..].copy_from_slice(&fnv(0x6c62_272e_07bb_0142).to_le_bytes());
    zerodds_opcua_gateway::types::Guid::from_bytes(bytes)
}

fn field_name_of(path: &str) -> &str {
    if path.is_empty() { "value" } else { path }
}

fn unmangle_field_name(name: &str) -> String {
    if name == "value" {
        String::new()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[cfg(feature = "pubsub-security")]
    use std::net::{Ipv4Addr, SocketAddr};
    use zerodds_dcps::runtime::RuntimeConfig;

    fn ast_of(idl: &str) -> std::sync::Arc<zerodds_ast::Ast> {
        std::sync::Arc::new(
            zerodds_wire_probe::encode::model_from_idl(idl, "pubsub_glue_test.idl").expect("idl"),
        )
    }

    /// OPC 10000-14 §6.2.3.2: each field with its built-in type, DataType,
    /// ValueRank and ArrayDimensions, an id of its own, the namespaces and
    /// the descriptions of the DataTypes that are no built-in types.
    #[test]
    fn uapubsub_6_2_3_2_the_metadata_describes_each_field_and_the_datatypes() {
        use zerodds_opcua_gateway::types::BuiltinTypeKind;
        let ast = ast_of(
            "struct Point { long x; long y; }; enum ModeKind { OFF, ON }; \
             @bit_bound(8) bitmask FlagSet { A, B }; \
             struct Probe { sequence<double> samples; ModeKind mode; FlagSet flags; \
                            Point at; map<string, long> tags; long grid[2][3]; };",
        );
        let ty = DynamicType::find(&ast, "Probe").expect("type");
        let ddstype = crate::dynamic_type::to_dds_type(&ty).expect("dds type");
        let paths: Vec<String> = ["samples", "mode", "flags", "at.x", "tags.k", "grid"]
            .iter()
            .map(|p| (*p).to_string())
            .collect();
        let meta = dataset_metadata("probe", &ddstype, &paths).expect("metadata");
        let ns = crate::daemon::server::NS;
        let field = |name: &str| meta.fields.iter().find(|f| f.name == name).expect(name);
        // An empty sequence of doubles is still a Double array.
        assert_eq!(field("samples").builtin_type, BuiltinTypeKind::Double);
        assert_eq!(field("samples").value_rank, 1);
        assert_eq!(field("samples").array_dimensions, vec![0]);
        assert_eq!(field("mode").builtin_type, BuiltinTypeKind::Int32);
        assert_eq!(
            field("mode").data_type,
            crate::ua_types::data_type_id(ns, "ModeKind")
        );
        assert_eq!(
            field("flags").builtin_type,
            BuiltinTypeKind::ExtensionObject
        );
        assert_eq!(
            field("at.x").data_type,
            zerodds_opcua_gateway::node_id::NodeId::numeric(0, 6)
        );
        assert_eq!(field("at.x").value_rank, -1);
        assert!(field("at.x").array_dimensions.is_empty());
        assert_eq!(field("tags.k").builtin_type, BuiltinTypeKind::Int32);
        assert_eq!(field("grid").value_rank, 2);
        assert_eq!(field("grid").array_dimensions, vec![2, 3]);
        let mut ids: Vec<_> = meta.fields.iter().map(|f| f.data_set_field_id).collect();
        ids.sort_by_key(|id| id.to_bytes());
        ids.dedup();
        assert_eq!(ids.len(), meta.fields.len(), "the field ids are not unique");
        assert_eq!(
            meta.namespaces,
            vec![
                zerodds_opcua_server::server::APPLICATION_URI.to_string(),
                crate::ua_types::NAMESPACE_URI.to_string()
            ]
        );
        let structures: Vec<&str> = meta
            .structure_data_types
            .iter()
            .map(|d| d.name.name.as_str())
            .collect();
        assert!(structures.contains(&"ProbeDataType"));
        assert!(structures.contains(&"PointDataType"));
        assert!(structures.contains(&"KeyValuePair"));
        let enums: Vec<(&str, u8)> = meta
            .enum_data_types
            .iter()
            .map(|d| (d.name.name.as_str(), d.builtin_type))
            .collect();
        assert!(enums.contains(&("ModeKindDataType", 6)));
        assert!(enums.contains(&("FlagSetDataType", 22)));
        // A path the type does not have is refused, not given Int32.
        assert!(matches!(
            dataset_metadata("probe", &ddstype, &["nothing".to_string()]),
            Err(PubSubGlueError::Field(_))
        ));
    }

    #[test]
    fn opcuabridge_4_3_a_scalar_topic_is_the_field_value() {
        // `field_name_of("")` must not produce an invalid (empty) Part-14
        // field name — the scalar-topic case.
        assert_eq!(field_name_of(""), "value");
        assert_eq!(field_name_of("x"), "x");
        assert_eq!(unmangle_field_name("value"), "");
        assert_eq!(unmangle_field_name("x"), "x");
    }

    #[test]
    fn opcuabridge_4_3_publisher_job_builds_a_pds_field_per_leaf() {
        let ast = ast_of("@final struct P { long x; long y; };");
        let dyn_type = DynamicType::find(&ast, "P").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "P").expect("support");
        let lease = zerodds_test_domains::lease_domain();
        let bridge = DdsBridge::start(
            lease.id(),
            "opcua-bridge-pubsub-test",
            RuntimeConfig::default(),
        )
        .expect("start");
        let transport = UdpTransport::bind(
            "127.0.0.1:0".parse().expect("addr"),
            "127.0.0.1:1".parse().expect("addr"),
        )
        .expect("bind");
        let job = PublisherJob::new(
            &bridge,
            "P".to_string(),
            "P",
            dyn_type,
            type_support,
            false,
            transport,
            1,
            1,
            1,
            "ds".to_string(),
            std::time::Duration::from_millis(50),
        )
        .expect("publisher job");
        assert_eq!(job.dds_topic(), "P");
    }

    /// Spec §5.3: a map key a sample brings and the DataSet has no field
    /// for is a field appended to it, and the DataSetWriter advertises a
    /// new MinorVersion (Part 14 §6.2.3.2.5). The same key again appends
    /// nothing.
    #[test]
    fn opcuabridge_5_3_a_new_map_key_is_a_field_appended_to_the_published_dataset() {
        use zerodds_opcua_gateway::data_value::{Variant, VariantValue};
        let ast = ast_of("@final struct M { long id; map<string, long> readings; };");
        let dyn_type = DynamicType::find(&ast, "M").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "M").expect("support");
        let lease = zerodds_test_domains::lease_domain();
        let bridge = DdsBridge::start(lease.id(), "opcua-bridge-map-pub", RuntimeConfig::default())
            .expect("start");
        let transport = UdpTransport::bind(
            "127.0.0.1:0".parse().expect("addr"),
            "127.0.0.1:1".parse().expect("addr"),
        )
        .expect("bind");
        let mut job = PublisherJob::new(
            &bridge,
            "M".to_string(),
            "M",
            dyn_type.clone(),
            type_support.clone(),
            false,
            transport,
            1,
            1,
            1,
            "ds".to_string(),
            std::time::Duration::from_millis(50),
        )
        .expect("publisher job");
        assert_eq!(job.dataset_fields(), (vec!["id".to_string()], 0));

        let mut writer_bridge = DdsBridge::start(
            lease.id(),
            "opcua-bridge-map-writer",
            RuntimeConfig::default(),
        )
        .expect("writer bridge");
        writer_bridge
            .writer("M", "M", false, false, Some(type_support.clone()))
            .expect("writer");
        let default = dynamic_value::default_data(&dyn_type).expect("default");
        let field = |path: &str, value: i32| FlatField {
            path: path.to_string(),
            value: Variant::scalar(VariantValue::Int32(value)),
        };
        let data = dynamic_value::unflatten(
            &dyn_type,
            &[field("id", 1), field("readings.a", 7)],
            &default,
        )
        .expect("unflatten");
        let sample = type_support.from_dynamic_data(data).expect("sample");
        let body = type_support.write_body(&sample, 2, false).expect("encode");

        let deadline = Instant::now() + Duration::from_secs(10);
        while job.dataset_fields().0.len() < 2 {
            assert!(Instant::now() < deadline, "the key never became a field");
            writer_bridge.write("M", &body).expect("dds write");
            std::thread::sleep(Duration::from_millis(100));
            job.tick(Instant::now(), &mut |_, _| {}).expect("tick");
        }
        let (fields, minor_version) = job.dataset_fields();
        assert_eq!(fields, ["id", "readings.a"]);
        assert!(minor_version > 0, "appended fields are a new MinorVersion");

        writer_bridge.write("M", &body).expect("dds write");
        std::thread::sleep(Duration::from_millis(300));
        job.tick(Instant::now(), &mut |_, _| {}).expect("tick");
        assert_eq!(job.dataset_fields(), (fields, minor_version));
    }

    #[test]
    fn opcuabridge_4_3_the_networkmessage_carries_the_connections_publisher_id() {
        let ast = ast_of("@final struct P { long x; };");
        let dyn_type = DynamicType::find(&ast, "P").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "P").expect("support");
        let lease = zerodds_test_domains::lease_domain();
        let bridge = DdsBridge::start(
            lease.id(),
            "opcua-bridge-publisher-id",
            RuntimeConfig::default(),
        )
        .expect("start");
        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
        receiver
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let transport = UdpTransport::bind(
            "127.0.0.1:0".parse().expect("addr"),
            receiver.local_addr().expect("addr"),
        )
        .expect("bind");
        let mut job = PublisherJob::new(
            &bridge,
            "P".to_string(),
            "P",
            dyn_type,
            type_support,
            false,
            transport,
            0x0102,
            1,
            1,
            "ds".to_string(),
            std::time::Duration::from_millis(50),
        )
        .expect("publisher job");
        let (_, published) = job.tick(Instant::now(), &mut |_, _| {}).expect("tick");
        assert!(published);
        let mut buf = [0u8; 2048];
        let n = receiver.recv(&mut buf).expect("one NetworkMessage");
        let message = &buf[..n];
        // Table 154: UADPFlags bit 4 (PublisherId), ExtendedFlags1 bits 0-2
        // 001 (UInt16), the PublisherId after the flags.
        assert_ne!(message[0] & 0x10, 0, "PublisherId enabled");
        assert_eq!(message[1] & 0x07, 0b001, "PublisherId of DataType UInt16");
        let at = if message[1] & 0x80 != 0 { 3 } else { 2 };
        assert_eq!(u16::from_le_bytes([message[at], message[at + 1]]), 0x0102);
    }

    #[cfg(feature = "pubsub-security")]
    #[test]
    fn opcuabridge_4_3_a_secured_publisher_job_delivers_a_dds_sample_to_a_subscriber_job() {
        use zerodds_opcua_pubsub::security::{SecurityKey, SecurityKeyService, SecurityPolicy};

        let policy = SecurityPolicy::Aes256Ctr;
        let blob = vec![0x5Au8; policy.key_material_len()];
        let key = SecurityKey::from_blob(policy, 11, &blob).expect("key");

        let ast = ast_of("@final struct Sec { long x; };");
        let dyn_type_pub = DynamicType::find(&ast, "Sec").expect("type");
        let dyn_type_sub = DynamicType::find(&ast, "Sec").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "Sec").expect("support");
        let lease = zerodds_test_domains::lease_domain();

        // The feeding DCPS writer — a third, independent participant,
        // exactly like a real remote publisher on the DDS side.
        let mut feed_bridge =
            DdsBridge::start(lease.id(), "pubsub-sec-feed", RuntimeConfig::default())
                .expect("start");
        feed_bridge
            .writer("Sec", "Sec", false, false, Some(type_support.clone()))
            .expect("feed writer");

        let bridge = DdsBridge::start(lease.id(), "pubsub-sec-main", RuntimeConfig::default())
            .expect("start");

        let group: Ipv4Addr = "239.9.9.9".parse().expect("addr");
        let port = 48_300 + (std::process::id() % 400) as u16;
        let multicast: SocketAddr = std::format!("{group}:{port}").parse().expect("addr");

        let pub_transport = UdpTransport::bind("0.0.0.0:0".parse().expect("addr"), multicast)
            .expect("bind publisher");
        let mut publisher_job = PublisherJob::new(
            &bridge,
            "Sec".to_string(),
            "Sec",
            dyn_type_pub,
            type_support.clone(),
            false,
            pub_transport,
            1,
            1,
            1,
            "ds".to_string(),
            std::time::Duration::from_millis(10),
        )
        .expect("publisher job");
        publisher_job.set_security(policy, SecurityKeyService::new(policy, "g1", key.clone()));

        // Bound to the *same port* the publisher sends to (with the
        // publisher's own local ephemeral port left at `0`) — the local
        // OS-level UDP demux match multicast delivery needs; joining the
        // group alone, on a mismatched ephemeral port, would receive
        // nothing.
        let sub_local: SocketAddr = std::format!("0.0.0.0:{port}").parse().expect("addr");
        let sub_transport = UdpTransport::bind(sub_local, multicast).expect("bind subscriber");
        sub_transport
            .join_multicast_v4(group, Ipv4Addr::UNSPECIFIED)
            .expect("join multicast");
        sub_transport
            .set_read_timeout(Some(std::time::Duration::from_millis(200)))
            .expect("read timeout");
        let mut subscriber_job = SubscriberJob::new(
            &bridge,
            "Sec".to_string(),
            "Sec",
            dyn_type_sub,
            type_support.clone(),
            false,
            sub_transport,
            "ds".to_string(),
            &["x".to_string()],
        )
        .expect("subscriber job");
        subscriber_job.set_security(policy, SecurityKeyService::new(policy, "g1", key));

        // Feed one DDS sample, so the publisher's own reader has
        // something to compose into the secured NetworkMessage.
        let data = zerodds_types::dynamic::DynamicData::Aggregate(vec![(
            "x".to_string(),
            zerodds_types::dynamic::DynamicData::Integer {
                kind: Some(zerodds_ast::PrimitiveKind::Int32),
                value: 55,
            },
        )]);
        let sample = type_support.from_dynamic_data(data).expect("build sample");
        let body = type_support.write_body(&sample, 2, false).expect("encode");
        std::thread::sleep(std::time::Duration::from_millis(300));
        feed_bridge.write("Sec", &body).expect("dds write");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut written = 0;
        while written == 0 && std::time::Instant::now() < deadline {
            publisher_job
                .tick(std::time::Instant::now(), &mut |_, _| {})
                .expect("publisher tick");
            written = subscriber_job.tick(&bridge).expect("subscriber tick");
            if written == 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        assert_eq!(
            written, 1,
            "the secured round trip must deliver exactly the one DDS sample fed in"
        );
    }

    #[cfg(feature = "pubsub-security")]
    #[test]
    fn opcuabridge_4_3_subscriber_with_the_wrong_key_refuses_not_silently_drops() {
        use zerodds_opcua_pubsub::security::{SecurityKey, SecurityKeyService, SecurityPolicy};

        let policy = SecurityPolicy::Aes256Ctr;
        let right = SecurityKey::from_blob(policy, 11, &vec![0x5Au8; policy.key_material_len()])
            .expect("key");
        let wrong = SecurityKey::from_blob(policy, 11, &vec![0xA5u8; policy.key_material_len()])
            .expect("key");

        let ast = ast_of("@final struct SecWrong { long x; };");
        let dyn_type_pub = DynamicType::find(&ast, "SecWrong").expect("type");
        let dyn_type_sub = DynamicType::find(&ast, "SecWrong").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "SecWrong").expect("support");
        let lease = zerodds_test_domains::lease_domain();
        let bridge = DdsBridge::start(lease.id(), "pubsub-sec-wrongkey", RuntimeConfig::default())
            .expect("start");

        let group: Ipv4Addr = "239.9.9.10".parse().expect("addr");
        let port = 48_700 + (std::process::id() % 400) as u16;
        let multicast: SocketAddr = std::format!("{group}:{port}").parse().expect("addr");

        let pub_transport = UdpTransport::bind("0.0.0.0:0".parse().expect("addr"), multicast)
            .expect("bind publisher");
        let mut publisher_job = PublisherJob::new(
            &bridge,
            "SecWrong".to_string(),
            "SecWrong",
            dyn_type_pub,
            type_support.clone(),
            false,
            pub_transport,
            1,
            1,
            1,
            "ds".to_string(),
            std::time::Duration::from_millis(10),
        )
        .expect("publisher job");
        publisher_job.set_security(policy, SecurityKeyService::new(policy, "g1", right));

        let sub_local: SocketAddr = std::format!("0.0.0.0:{port}").parse().expect("addr");
        let sub_transport = UdpTransport::bind(sub_local, multicast).expect("bind subscriber");
        sub_transport
            .join_multicast_v4(group, Ipv4Addr::UNSPECIFIED)
            .expect("join multicast");
        sub_transport
            .set_read_timeout(Some(std::time::Duration::from_millis(200)))
            .expect("read timeout");
        let mut subscriber_job = SubscriberJob::new(
            &bridge,
            "SecWrong".to_string(),
            "SecWrong",
            dyn_type_sub,
            type_support,
            false,
            sub_transport,
            "ds".to_string(),
            &["x".to_string()],
        )
        .expect("subscriber job");
        // The wrong key, same SecurityGroup/token id: a plausible
        // misconfiguration (stale/rotated key on one side), not a
        // deliberately absurd input.
        subscriber_job.set_security(policy, SecurityKeyService::new(policy, "g1", wrong));

        publisher_job
            .tick(std::time::Instant::now(), &mut |_, _| {})
            .expect("publisher tick");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut saw_error = false;
        while std::time::Instant::now() < deadline {
            match subscriber_job.tick(&bridge) {
                Err(_) => {
                    saw_error = true;
                    break;
                }
                Ok(0) => std::thread::sleep(std::time::Duration::from_millis(50)),
                Ok(n) => {
                    assert_eq!(
                        n, 0,
                        "a wrong-key datagram must never decode as if it were valid"
                    );
                }
            }
        }
        assert!(
            saw_error,
            "a wrong-key secured datagram must be refused, not silently ignored or misdecoded"
        );
    }
}
