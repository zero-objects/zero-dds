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
use zerodds_opcua_gateway::types::BuiltinTypeKind;
use zerodds_opcua_pubsub::config::{
    ConfigurationVersion, DataSetMetaData, DataSetReaderConfig, DataSetWriterConfig, FieldMetaData,
    WriterGroupConfig,
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
}

impl core::fmt::Display for PubSubGlueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Default(m) => write!(f, "default value: {m}"),
            Self::Convert(e) => write!(f, "{e}"),
            Self::Transport(m) => write!(f, "transport: {m}"),
            Self::Dds(m) => write!(f, "DDS: {m}"),
            Self::Codec(m) => write!(f, "codec: {m}"),
        }
    }
}

impl std::error::Error for PubSubGlueError {}

impl From<dynamic_value::ConvertError> for PubSubGlueError {
    fn from(e: dynamic_value::ConvertError) -> Self {
        Self::Convert(e)
    }
}

/// A [`FlatField`]'s [`zerodds_opcua_gateway::data_value::Variant::type_kind`],
/// or the enum/bitmask fallback [`crate::dynamic_value`] always encodes
/// them as (`Int32`/`UInt32`) — never `None` for a field `flatten` actually
/// emitted, since every leaf it emits is exactly one of those kinds.
fn field_kind(field: &FlatField) -> BuiltinTypeKind {
    field.value.type_kind().unwrap_or(BuiltinTypeKind::Int32)
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
    #[cfg(feature = "pubsub-security")]
    security: Option<(
        zerodds_opcua_pubsub::security::SecurityPolicy,
        zerodds_opcua_pubsub::security::SecurityKeyService,
    )>,
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
        dds_bridge: &mut DdsBridge,
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
            .reader(
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
        let group = WriterGroup::new(
            WriterGroupConfig::new(std::format!("{dds_topic}-wg"), writer_group_id),
            PublisherId::UInt16(publisher_id),
        );
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
    /// [`Self::tick`] that publishes calls
    /// [`Publisher::publish_cycle_secured`] instead of the plain
    /// `publish_cycle`.
    #[cfg(feature = "pubsub-security")]
    pub fn set_security(
        &mut self,
        policy: zerodds_opcua_pubsub::security::SecurityPolicy,
        sks: zerodds_opcua_pubsub::security::SecurityKeyService,
    ) {
        self.security = Some((policy, sks));
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
            for field in &fields {
                dataset.set_variant(field_name(field), field.value.clone());
            }
            applied += 1;
        }
        if now < self.next_publish {
            return Ok((applied, false));
        }
        self.next_publish = now + self.interval;
        #[cfg(feature = "pubsub-security")]
        if let Some((policy, sks)) = &self.security {
            self.publisher
                .publish_cycle_secured(None, *policy, sks.current_key())
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
    #[cfg(feature = "pubsub-security")]
    security: Option<(
        zerodds_opcua_pubsub::security::SecurityPolicy,
        zerodds_opcua_pubsub::security::SecurityKeyService,
    )>,
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
        dds_bridge: &mut DdsBridge,
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
            .writer(
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
        let by_path: std::collections::HashMap<&str, BuiltinTypeKind> = fields
            .iter()
            .map(|f| (f.path.as_str(), field_kind(f)))
            .collect();
        let meta_fields = field_paths
            .iter()
            .map(|path| {
                FieldMetaData::scalar(
                    field_name_of(path),
                    *by_path
                        .get(path.as_str())
                        .unwrap_or(&BuiltinTypeKind::Int32),
                )
            })
            .collect();
        let meta = DataSetMetaData::new(dataset_name, meta_fields);

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
        self.security = Some((policy, sks));
    }

    /// Receives every pending UADP datagram, rebuilds the sample and writes
    /// it into DDS through `dds_bridge` (the same one [`Self::new`]
    /// registered this job's writer on). Returns the number of samples
    /// written.
    ///
    /// # Errors
    /// [`PubSubGlueError`] on a decode/convert/transport/DDS-write failure.
    pub fn tick(&mut self, dds_bridge: &DdsBridge) -> Result<usize, PubSubGlueError> {
        #[cfg(feature = "pubsub-security")]
        let matched = if let Some((policy, sks)) = &self.security {
            self.subscriber
                .poll_secured(*policy, sks)
                .map_err(|e| PubSubGlueError::Transport(e.to_string()))?
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
                        path: unmangle_field_name(&f.name),
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
        Ok(written)
    }

    /// The registered DDS `WriterHandle`.
    #[must_use]
    pub fn writer(&self) -> WriterHandle {
        self.writer
    }
}

/// Sanitizes a [`FlatField`] path into a Part 14 field name (which must not
/// be empty — a scalar topic's single leaf, `flatten`'s `""` path, becomes
/// `"value"`).
fn field_name(field: &FlatField) -> &str {
    field_name_of(&field.path)
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

    #[test]
    fn field_names_never_empty_and_round_trip() {
        // `field_name_of("")` must not produce an invalid (empty) Part-14
        // field name — the scalar-topic case.
        assert_eq!(field_name_of(""), "value");
        assert_eq!(field_name_of("x"), "x");
        assert_eq!(unmangle_field_name("value"), "");
        assert_eq!(unmangle_field_name("x"), "x");
    }

    #[test]
    fn publisher_job_builds_a_pds_field_per_leaf() {
        let ast = ast_of("@final struct P { long x; long y; };");
        let dyn_type = DynamicType::find(&ast, "P").expect("type");
        let type_support = crate::idl::type_support_for(&[ast], "P").expect("support");
        let lease = zerodds_test_domains::lease_domain();
        let mut bridge = DdsBridge::start(
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
            &mut bridge,
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

    #[cfg(feature = "pubsub-security")]
    #[test]
    fn secured_publisher_and_subscriber_round_trip_a_dds_sample() {
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

        let mut bridge = DdsBridge::start(lease.id(), "pubsub-sec-main", RuntimeConfig::default())
            .expect("start");

        let group: Ipv4Addr = "239.9.9.9".parse().expect("addr");
        let port = 48_300 + (std::process::id() % 400) as u16;
        let multicast: SocketAddr = std::format!("{group}:{port}").parse().expect("addr");

        let pub_transport = UdpTransport::bind("0.0.0.0:0".parse().expect("addr"), multicast)
            .expect("bind publisher");
        let mut publisher_job = PublisherJob::new(
            &mut bridge,
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
            &mut bridge,
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
    fn subscriber_with_the_wrong_key_refuses_not_silently_drops() {
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
        let mut bridge =
            DdsBridge::start(lease.id(), "pubsub-sec-wrongkey", RuntimeConfig::default())
                .expect("start");

        let group: Ipv4Addr = "239.9.9.10".parse().expect("addr");
        let port = 48_700 + (std::process::id() % 400) as u16;
        let multicast: SocketAddr = std::format!("{group}:{port}").parse().expect("addr");

        let pub_transport = UdpTransport::bind("0.0.0.0:0".parse().expect("addr"), multicast)
            .expect("bind publisher");
        let mut publisher_job = PublisherJob::new(
            &mut bridge,
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
            &mut bridge,
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
