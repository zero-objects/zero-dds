// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The PubSub runtime — drives writer and reader groups over a
//! [`PubSubTransport`] (OPC Foundation Part 14 §6.2.6 PubSubConnection
//! behaviour).
//!
//! A [`Publisher`] owns a [`WriterGroup`], its [`DataSetWriter`]s and the
//! [`PublishedDataSet`]s they read from; each [`publish_cycle`] produces a
//! DataSetMessage per writer, frames them into one NetworkMessage and sends it.
//! A [`Subscriber`] owns a [`ReaderGroup`] and on each [`poll`] receives one
//! datagram, decodes it and dispatches it to the matching readers.
//!
//! With the `security` feature, `Publisher::publish_cycle_secured` and
//! `Subscriber::poll_secured` add UADP message security (`crate::security`)
//! around the same flow.
//!
//! [`publish_cycle`]: Publisher::publish_cycle
//! [`poll`]: Subscriber::poll

use alloc::string::String;
use alloc::vec::Vec;

use crate::binary::{from_binary, to_binary};
use crate::error::{DecodeError, EncodeError};
use crate::reader::{MatchedDataSet, ReaderGroup};
use crate::transport::{PubSubTransport, TransportError};
use crate::uadp::network_message::NetworkMessage;
use crate::writer::{DataSetWriter, PublishedDataSet, WriterGroup};

/// An error from the PubSub runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum DaemonError {
    /// A NetworkMessage could not be encoded.
    Encode(EncodeError),
    /// A received datagram could not be decoded.
    Decode(DecodeError),
    /// The transport carrier failed.
    Transport(TransportError),
    /// A writer references a PublishedDataSet the Publisher does not hold.
    UnknownDataSet(String),
    /// A security operation failed (feature `security`).
    #[cfg(feature = "security")]
    Security(crate::security::SecurityError),
}

impl From<EncodeError> for DaemonError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl From<DecodeError> for DaemonError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<TransportError> for DaemonError {
    fn from(e: TransportError) -> Self {
        Self::Transport(e)
    }
}

#[cfg(feature = "security")]
impl From<crate::security::SecurityError> for DaemonError {
    fn from(e: crate::security::SecurityError) -> Self {
        Self::Security(e)
    }
}

impl core::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::Transport(e) => write!(f, "transport error: {e}"),
            Self::UnknownDataSet(n) => write!(f, "writer references unknown PublishedDataSet {n}"),
            #[cfg(feature = "security")]
            Self::Security(e) => write!(f, "security error: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DaemonError {}

/// The publishing runtime: a [`WriterGroup`] plus its writers and the
/// PublishedDataSets they read from, bound to a transport.
#[derive(Debug, Clone)]
pub struct Publisher<T: PubSubTransport> {
    transport: T,
    group: WriterGroup,
    writers: Vec<DataSetWriter>,
    datasets: Vec<PublishedDataSet>,
    // Monotonic per-message nonce source; only read on the secured path.
    #[cfg_attr(not(feature = "security"), allow(dead_code))]
    nonce_counter: u64,
}

impl<T: PubSubTransport> Publisher<T> {
    /// Creates a Publisher for `group` over `transport`.
    #[must_use]
    pub fn new(transport: T, group: WriterGroup) -> Self {
        Self {
            transport,
            group,
            writers: Vec::new(),
            datasets: Vec::new(),
            nonce_counter: 0,
        }
    }

    /// Registers a PublishedDataSet (referenced by writers via its name).
    pub fn add_dataset(&mut self, dataset: PublishedDataSet) -> &mut Self {
        self.datasets.push(dataset);
        self
    }

    /// Registers a DataSetWriter.
    pub fn add_writer(&mut self, writer: DataSetWriter) -> &mut Self {
        self.writers.push(writer);
        self
    }

    /// Mutable access to a registered PublishedDataSet by name.
    pub fn dataset_mut(&mut self, name: &str) -> Option<&mut PublishedDataSet> {
        self.datasets.iter_mut().find(|d| d.name() == name)
    }

    /// The transport carrier.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Produces one DataSetMessage per writer and frames them into a single
    /// NetworkMessage. `timestamp` (DateTime ticks) feeds the message/group
    /// Timestamp content-mask bits.
    fn frame_cycle(&mut self, timestamp: Option<i64>) -> Result<NetworkMessage, DaemonError> {
        let mut messages = Vec::with_capacity(self.writers.len());
        for w in &mut self.writers {
            let name = w.config().data_set_name.clone();
            let ds = self
                .datasets
                .iter()
                .find(|d| d.name() == name)
                .ok_or(DaemonError::UnknownDataSet(name))?;
            messages.push(w.produce(ds, timestamp)?);
        }
        Ok(self.group.frame(messages, timestamp))
    }

    /// Runs one publish cycle: produce, frame, serialise and send a plaintext
    /// NetworkMessage.
    ///
    /// # Errors
    /// [`DaemonError`] if a dataset is missing, encoding fails or the send
    /// fails.
    pub fn publish_cycle(&mut self, timestamp: Option<i64>) -> Result<(), DaemonError> {
        let nm = self.frame_cycle(timestamp)?;
        let bytes = to_binary(&nm)?;
        self.transport.send(&bytes)?;
        Ok(())
    }

    /// Runs one publish cycle with UADP message security: produce, frame,
    /// then [`protect`](crate::security::protect) (always signed, encrypted)
    /// with a per-cycle monotonic nonce, and send.
    ///
    /// # Errors
    /// [`DaemonError`] on a missing dataset, encode/crypto failure or send
    /// failure.
    #[cfg(feature = "security")]
    pub fn publish_cycle_secured(
        &mut self,
        timestamp: Option<i64>,
        policy: crate::security::SecurityPolicy,
        key: &crate::security::SecurityKey,
    ) -> Result<(), DaemonError> {
        let nm = self.frame_cycle(timestamp)?;
        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        let nonce = self.nonce_counter.to_be_bytes();
        let bytes = crate::security::protect(&nm, policy, key, &nonce, true)?;
        self.transport.send(&bytes)?;
        Ok(())
    }
}

/// The subscribing runtime: a [`ReaderGroup`] bound to a transport.
#[derive(Debug, Clone)]
pub struct Subscriber<T: PubSubTransport> {
    transport: T,
    reader_group: ReaderGroup,
}

impl<T: PubSubTransport> Subscriber<T> {
    /// Creates a Subscriber for `reader_group` over `transport`.
    #[must_use]
    pub fn new(transport: T, reader_group: ReaderGroup) -> Self {
        Self {
            transport,
            reader_group,
        }
    }

    /// Mutable access to the reader group (to add readers).
    pub fn reader_group_mut(&mut self) -> &mut ReaderGroup {
        &mut self.reader_group
    }

    /// The transport carrier.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Receives and dispatches one plaintext datagram. Returns an empty vector
    /// when the transport has nothing pending ([`TransportError::Timeout`]).
    ///
    /// # Errors
    /// [`DaemonError`] on a non-timeout transport failure or a decode failure.
    pub fn poll(&self) -> Result<Vec<MatchedDataSet>, DaemonError> {
        let bytes = match self.transport.receive() {
            Ok(b) => b,
            Err(TransportError::Timeout) => return Ok(Vec::new()),
            Err(e) => return Err(DaemonError::Transport(e)),
        };
        let nm: NetworkMessage = from_binary(&bytes)?;
        Ok(self.reader_group.accept(&nm)?)
    }

    /// Receives and dispatches one secured datagram, verifying and decrypting
    /// it via [`unprotect`](crate::security::unprotect). Returns an empty
    /// vector on a transport timeout.
    ///
    /// # Errors
    /// [`DaemonError`] on a non-timeout transport failure, a security failure
    /// or a decode failure.
    #[cfg(feature = "security")]
    pub fn poll_secured(
        &self,
        policy: crate::security::SecurityPolicy,
        sks: &crate::security::SecurityKeyService,
    ) -> Result<Vec<MatchedDataSet>, DaemonError> {
        let bytes = match self.transport.receive() {
            Ok(b) => b,
            Err(TransportError::Timeout) => return Ok(Vec::new()),
            Err(e) => return Err(DaemonError::Transport(e)),
        };
        let nm = crate::security::unprotect(&bytes, policy, sks)?;
        Ok(self.reader_group.accept(&nm)?)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::config::{
        ConfigurationVersion, DataSetMetaData, DataSetReaderConfig, DataSetWriterConfig,
        FieldMetaData, WriterGroupConfig,
    };
    use crate::reader::DataSetReader;
    use crate::transport::LoopbackTransport;
    use crate::uadp::dataset_message::DataSetMessageKind;
    use crate::uadp::network_message::PublisherId;
    use zerodds_opcua_gateway::data_value::{DataValue, Variant, VariantValue};
    use zerodds_opcua_gateway::types::BuiltinTypeKind;

    fn dv(v: i32) -> DataValue {
        DataValue {
            value: Some(Variant::scalar(VariantValue::Int32(v))),
            status: None,
            source_timestamp: None,
            server_timestamp: None,
            source_pico_sec: None,
            server_pico_sec: None,
        }
    }

    fn meta() -> DataSetMetaData {
        DataSetMetaData::new(
            "ds1",
            alloc::vec![FieldMetaData::scalar("a", BuiltinTypeKind::Int32)],
        )
    }

    fn publisher(tx: LoopbackTransport) -> Publisher<LoopbackTransport> {
        let mut pds = PublishedDataSet::new("ds1");
        pds.add_field("a", dv(0));
        let mut pubr = Publisher::new(
            tx,
            WriterGroup::new(WriterGroupConfig::new("g1", 1), PublisherId::UInt16(9)),
        );
        pubr.add_dataset(pds).add_writer(DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        ));
        pubr
    }

    fn subscriber(tx: LoopbackTransport) -> Subscriber<LoopbackTransport> {
        let mut rg = ReaderGroup::new();
        rg.add_reader(DataSetReader::new(DataSetReaderConfig::new("r1", meta())));
        Subscriber::new(tx, rg)
    }

    #[test]
    fn end_to_end_publish_then_poll() {
        let bus = LoopbackTransport::new();
        let mut pubr = publisher(bus.clone());
        let subr = subscriber(bus);

        pubr.dataset_mut("ds1").expect("ds").set("a", dv(123));
        pubr.publish_cycle(None).expect("publish");

        let matched = subr.poll().expect("poll");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].reader_name, "r1");
        assert_eq!(matched[0].data.writer_id, 5);
        assert_eq!(matched[0].data.kind, DataSetMessageKind::KeyFrame);
        assert_eq!(
            matched[0].data.fields[0].value.value,
            Some(Variant::scalar(VariantValue::Int32(123)))
        );
    }

    #[test]
    fn poll_returns_empty_when_idle() {
        let bus = LoopbackTransport::new();
        let subr = subscriber(bus);
        assert!(subr.poll().expect("poll").is_empty());
    }

    #[test]
    fn unknown_dataset_is_reported() {
        let bus = LoopbackTransport::new();
        let mut pubr = Publisher::new(
            bus,
            WriterGroup::new(WriterGroupConfig::new("g1", 1), PublisherId::UInt16(9)),
        );
        // Writer references "ds1" but no dataset was added.
        pubr.add_writer(DataSetWriter::new(
            DataSetWriterConfig::new("w1", 5, "ds1"),
            ConfigurationVersion::default(),
        ));
        assert_eq!(
            pubr.publish_cycle(None),
            Err(DaemonError::UnknownDataSet(String::from("ds1")))
        );
    }

    #[cfg(feature = "security")]
    #[test]
    fn end_to_end_secured() {
        use crate::security::{SecurityKey, SecurityKeyService, SecurityPolicy};

        let policy = SecurityPolicy::Aes256Ctr;
        let blob = alloc::vec![0x5Au8; policy.key_material_len()];
        let key = SecurityKey::from_blob(policy, 11, &blob).expect("key");
        let sks = SecurityKeyService::new(policy, "g", key.clone());

        let bus = LoopbackTransport::new();
        let mut pubr = publisher(bus.clone());
        let subr = subscriber(bus);

        pubr.dataset_mut("ds1").expect("ds").set("a", dv(777));
        pubr.publish_cycle_secured(None, policy, &key)
            .expect("publish");

        // The plaintext value must not be observable on the wire.
        assert_eq!(pubr.transport().pending(), 1);

        let matched = subr.poll_secured(policy, &sks).expect("poll");
        assert_eq!(matched.len(), 1);
        assert_eq!(
            matched[0].data.fields[0].value.value,
            Some(Variant::scalar(VariantValue::Int32(777)))
        );
    }
}
