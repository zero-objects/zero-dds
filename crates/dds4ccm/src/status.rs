// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Status access (DDS4CCM 1.1 §7.2.2.1.5): the DDS listeners a connector
//! fragment sets on the entities of its port, and where each status goes.
//!
//! | DDS status | goes to |
//! |---|---|
//! | `REQUESTED_DEADLINE_MISSED`, `SAMPLE_LOST` | `PortStatusListener` of the port |
//! | `INCONSISTENT_TOPIC`, `REQUESTED_INCOMPATIBLE_QOS`, `SAMPLE_REJECTED`, `OFFERED_DEADLINE_MISSED`, `OFFERED_INCOMPATIBLE_QOS` | `ConnectorStatusListener` |
//! | `DATA_AVAILABLE` | the fragment: the data listener of the port |
//! | `LIVELINESS_CHANGED`, `SUBSCRIPTION_MATCHED`, `LIVELINESS_LOST`, `PUBLICATION_MATCHED` | `ConnectorStatusListener::on_unexpected_status` |
//!
//! The first two rows are the operations of the two interfaces. The
//! statuses "needed for functioning (e.g., data on readers)" are "kept for
//! internal implementation of connectors fragments and therefore not
//! reported". `on_unexpected_status` is "added on ConnectorStatusListener
//! to report unexpected statuses" with "the reporting DDS Entity and the
//! DDS status kind": the statuses of a DataReader and a DataWriter that no
//! other operation names.
//!
//! The DataReader a `DDS_Update` port with `is_global_scope` looks the
//! instances up on belongs to a port without a `PortStatusListener`: its
//! statuses of the first row are reported with `on_unexpected_status` as
//! well, the others as in the table. So are the statuses of the
//! DataReaders of a DLRL port (§8.4.1), through the listener of its
//! Subscriber; its DataWriters have the listener of a publishing port.
//!
//! The operations of `PortStatusListener`, a facet of the port, are called
//! where the threading policy of the port puts its facets (§7.4.4,
//! [`crate::threading`]). `ConnectorStatusListener` is no facet of a port:
//! it is the receptacle `error_listener` of the connector and is called on
//! the thread DDS reports the status on.

use std::sync::{Arc, Mutex, Weak};

use zerodds_dcps::listener::{
    DataReaderListener, DataWriterListener, SubscriberListener, TopicListener,
};
use zerodds_dcps::psm_constants::status as status_kind;
use zerodds_dcps::publisher::AnyDataWriter;
use zerodds_dcps::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};
use zerodds_dcps::{AnyDataReader, DdsType, StatusMask, TopicDescriptionHandle};

use crate::ccm_dds::{ConnectorStatusListener, DdsEntity, PortStatusListener};
use crate::listener::Push;
use crate::reader_core::lock;
use crate::receptacle::Receptacle;
use crate::threading::Facets;

/// `uses ConnectorStatusListener error_listener` of a connector, shared
/// with the ports of the connector.
pub type ErrorListener = Receptacle<dyn ConnectorStatusListener>;

/// The statuses the listener of a subscribing port takes, without the data.
pub(crate) const READER_STATUSES: StatusMask = status_kind::REQUESTED_DEADLINE_MISSED
    | status_kind::SAMPLE_LOST
    | status_kind::REQUESTED_INCOMPATIBLE_QOS
    | status_kind::SAMPLE_REJECTED
    | status_kind::LIVELINESS_CHANGED
    | status_kind::SUBSCRIPTION_MATCHED;

/// The statuses the listener of a publishing port takes.
pub(crate) const WRITER_STATUSES: StatusMask = status_kind::OFFERED_DEADLINE_MISSED
    | status_kind::OFFERED_INCOMPATIBLE_QOS
    | status_kind::LIVELINESS_LOST
    | status_kind::PUBLICATION_MATCHED;

/// The `DDS::DataReaderListener` of a subscribing port, and of the
/// DataReader a `DDS_Update` port keeps for itself.
pub struct ReaderStatusListener<T: DdsType> {
    /// `status` of the port; `None` for a DataReader of a port that has no
    /// `PortStatusListener`.
    port: Option<Arc<Receptacle<dyn PortStatusListener>>>,
    connector: Arc<ErrorListener>,
    /// The push side of the port, if it has one. Weak: the reader holds
    /// this listener and the push side holds the reader.
    push: Option<Weak<Push<T>>>,
    /// Where `status` of the port is called.
    facets: Facets,
}

impl<T: DdsType> ReaderStatusListener<T> {
    /// The listener of a port whose `status` is `port`, in a connector
    /// whose `error_listener` is `connector`.
    #[must_use]
    pub fn new(
        port: Arc<Receptacle<dyn PortStatusListener>>,
        connector: Arc<ErrorListener>,
    ) -> Self {
        Self {
            port: Some(port),
            connector,
            push: None,
            facets: Facets::default(),
        }
    }

    /// The listener of a DataReader of a port that has no
    /// `PortStatusListener`, in a connector whose `error_listener` is
    /// `connector`: the DataReader a `DDS_Update` port with
    /// `is_global_scope` looks the instances up on. Every status goes to
    /// `connector`; `REQUESTED_DEADLINE_MISSED` and `SAMPLE_LOST`, which
    /// `ConnectorStatusListener` has no operation for, with
    /// `on_unexpected_status`.
    #[must_use]
    pub fn of_connector(connector: Arc<ErrorListener>) -> Self {
        Self {
            port: None,
            connector,
            push: None,
            facets: Facets::default(),
        }
    }

    pub(crate) fn with_facets(mut self, facets: Facets) -> Self {
        self.facets = facets;
        self
    }

    pub(crate) fn with_push(mut self, push: &Arc<Push<T>>) -> Self {
        self.push = Some(Arc::downgrade(push));
        self
    }

    /// The status mask to set this listener with.
    #[must_use]
    pub fn mask(&self) -> StatusMask {
        if self.push.is_some() {
            READER_STATUSES | status_kind::DATA_AVAILABLE
        } else {
            READER_STATUSES
        }
    }

    fn unexpected(&self, reader: &AnyDataReader, kind: StatusMask) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_unexpected_status(&DdsEntity::DataReader(reader.clone()), kind);
        }
    }
}

impl<T: DdsType + Send + 'static> DataReaderListener for ReaderStatusListener<T> {
    fn on_data_available_entity(&self, _reader: &AnyDataReader) {
        if let Some(push) = self.push.as_ref().and_then(Weak::upgrade) {
            push.deliver();
        }
    }

    fn on_requested_deadline_missed_entity(
        &self,
        reader: &AnyDataReader,
        status: RequestedDeadlineMissedStatus,
    ) {
        let Some(port) = &self.port else {
            return self.unexpected(reader, status_kind::REQUESTED_DEADLINE_MISSED);
        };
        let (port, reader) = (Arc::clone(port), reader.clone());
        self.facets.call(move || {
            if let Some(listener) = port.get_connection() {
                listener.on_requested_deadline_missed(&reader, status);
            }
        });
    }

    fn on_sample_lost_entity(&self, reader: &AnyDataReader, status: SampleLostStatus) {
        let Some(port) = &self.port else {
            return self.unexpected(reader, status_kind::SAMPLE_LOST);
        };
        let (port, reader) = (Arc::clone(port), reader.clone());
        self.facets.call(move || {
            if let Some(listener) = port.get_connection() {
                listener.on_sample_lost(&reader, status);
            }
        });
    }

    fn on_requested_incompatible_qos_entity(
        &self,
        reader: &AnyDataReader,
        status: RequestedIncompatibleQosStatus,
    ) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_requested_incompatible_qos(reader, status);
        }
    }

    fn on_sample_rejected_entity(&self, reader: &AnyDataReader, status: SampleRejectedStatus) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_sample_rejected(reader, status);
        }
    }

    fn on_liveliness_changed_entity(
        &self,
        reader: &AnyDataReader,
        _status: LivelinessChangedStatus,
    ) {
        self.unexpected(reader, status_kind::LIVELINESS_CHANGED);
    }

    fn on_subscription_matched_entity(
        &self,
        reader: &AnyDataReader,
        _status: SubscriptionMatchedStatus,
    ) {
        self.unexpected(reader, status_kind::SUBSCRIPTION_MATCHED);
    }
}

/// The `DDS::SubscriberListener` of the Subscriber of a DLRL port
/// (§8.4.1, [`crate::dlrl`]).
///
/// The DataReaders of a DLRL port are those of its Cache, whose own
/// listener takes `DATA_AVAILABLE`, the status the Cache needs to apply
/// the updates. Their other statuses go up to the Subscriber (DDS 1.4
/// §2.2.4.2.3), where this listener reports them to the connector as for
/// a DataReader of a port without a `PortStatusListener`
/// ([`ReaderStatusListener::of_connector`]): `REQUESTED_INCOMPATIBLE_QOS`
/// and `SAMPLE_REJECTED` with their operations, `REQUESTED_DEADLINE_MISSED`,
/// `SAMPLE_LOST`, `LIVELINESS_CHANGED` and `SUBSCRIPTION_MATCHED` with
/// `on_unexpected_status`. Its mask has neither `DATA_ON_READERS` nor
/// `DATA_AVAILABLE`: the data stay with the Cache.
pub(crate) struct SubscriberStatusListener {
    connector: Arc<ErrorListener>,
}

impl SubscriberStatusListener {
    pub(crate) fn new(connector: Arc<ErrorListener>) -> Self {
        Self { connector }
    }

    pub(crate) fn mask(&self) -> StatusMask {
        READER_STATUSES
    }

    fn unexpected(&self, reader: &AnyDataReader, kind: StatusMask) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_unexpected_status(&DdsEntity::DataReader(reader.clone()), kind);
        }
    }
}

impl SubscriberListener for SubscriberStatusListener {
    fn on_requested_deadline_missed_entity(
        &self,
        reader: &AnyDataReader,
        _status: RequestedDeadlineMissedStatus,
    ) {
        self.unexpected(reader, status_kind::REQUESTED_DEADLINE_MISSED);
    }

    fn on_sample_lost_entity(&self, reader: &AnyDataReader, _status: SampleLostStatus) {
        self.unexpected(reader, status_kind::SAMPLE_LOST);
    }

    fn on_requested_incompatible_qos_entity(
        &self,
        reader: &AnyDataReader,
        status: RequestedIncompatibleQosStatus,
    ) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_requested_incompatible_qos(reader, status);
        }
    }

    fn on_sample_rejected_entity(&self, reader: &AnyDataReader, status: SampleRejectedStatus) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_sample_rejected(reader, status);
        }
    }

    fn on_liveliness_changed_entity(
        &self,
        reader: &AnyDataReader,
        _status: LivelinessChangedStatus,
    ) {
        self.unexpected(reader, status_kind::LIVELINESS_CHANGED);
    }

    fn on_subscription_matched_entity(
        &self,
        reader: &AnyDataReader,
        _status: SubscriptionMatchedStatus,
    ) {
        self.unexpected(reader, status_kind::SUBSCRIPTION_MATCHED);
    }
}

/// The `DDS::DataWriterListener` of a publishing port.
pub struct WriterStatusListener {
    connector: Arc<ErrorListener>,
}

impl WriterStatusListener {
    /// The listener of a port in a connector whose `error_listener` is
    /// `connector`.
    #[must_use]
    pub fn new(connector: Arc<ErrorListener>) -> Self {
        Self { connector }
    }

    /// The status mask to set this listener with.
    #[must_use]
    pub fn mask(&self) -> StatusMask {
        WRITER_STATUSES
    }

    fn unexpected(&self, writer: &AnyDataWriter, kind: StatusMask) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_unexpected_status(&DdsEntity::DataWriter(writer.clone()), kind);
        }
    }
}

impl DataWriterListener for WriterStatusListener {
    fn on_offered_deadline_missed_entity(
        &self,
        writer: &AnyDataWriter,
        status: OfferedDeadlineMissedStatus,
    ) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_offered_deadline_missed(writer, status);
        }
    }

    fn on_offered_incompatible_qos_entity(
        &self,
        writer: &AnyDataWriter,
        status: OfferedIncompatibleQosStatus,
    ) {
        if let Some(listener) = self.connector.get_connection() {
            listener.on_offered_incompatible_qos(writer, status);
        }
    }

    fn on_liveliness_lost_entity(&self, writer: &AnyDataWriter, _status: LivelinessLostStatus) {
        self.unexpected(writer, status_kind::LIVELINESS_LOST);
    }

    fn on_publication_matched_entity(
        &self,
        writer: &AnyDataWriter,
        _status: PublicationMatchedStatus,
    ) {
        self.unexpected(writer, status_kind::PUBLICATION_MATCHED);
    }
}

/// The `DDS::TopicListener` of the topic of a connector.
///
/// The connector sets it on the topic it creates (§7.3.1,
/// [`crate::connector`]): the topic belongs to the connector and may serve
/// several of its ports. Connectors that use one topic in one
/// DomainParticipant share the listener; each `error_listener` is called.
/// A port completed by hand is given its topic; the caller sets this
/// listener there.
pub struct TopicStatusListener {
    connectors: Mutex<Vec<Arc<ErrorListener>>>,
}

impl TopicStatusListener {
    /// The listener of the topic of a connector whose `error_listener` is
    /// `connector`.
    #[must_use]
    pub fn new(connector: Arc<ErrorListener>) -> Self {
        Self {
            connectors: Mutex::new(vec![connector]),
        }
    }

    /// The status mask to set this listener with.
    #[must_use]
    pub fn mask(&self) -> StatusMask {
        status_kind::INCONSISTENT_TOPIC
    }

    /// Adds the `error_listener` of another connector of the topic.
    pub(crate) fn add(&self, connector: Arc<ErrorListener>) {
        lock(&self.connectors).push(connector);
    }

    /// Takes the `error_listener` of a connector out; `true` when none is
    /// left.
    pub(crate) fn remove(&self, connector: &Arc<ErrorListener>) -> bool {
        let mut connectors = lock(&self.connectors);
        if let Some(at) = connectors.iter().position(|c| Arc::ptr_eq(c, connector)) {
            connectors.remove(at);
        }
        connectors.is_empty()
    }
}

impl TopicListener for TopicStatusListener {
    fn on_inconsistent_topic_entity(
        &self,
        topic: &TopicDescriptionHandle,
        status: InconsistentTopicStatus,
    ) {
        // Not under the lock: a listener may remove its connector.
        let connectors = lock(&self.connectors).clone();
        for (at, connector) in connectors.iter().enumerate() {
            // A connector with two ports in the DomainParticipant (two
            // DLRL ports) holds two shares: its listener is called once.
            if connectors[..at].iter().any(|c| Arc::ptr_eq(c, connector)) {
                continue;
            }
            if let Some(listener) = connector.get_connection() {
                listener.on_inconsistent_topic(topic, status);
            }
        }
    }
}
