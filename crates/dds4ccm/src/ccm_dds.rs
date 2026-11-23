// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The non-typed part of the module `CCM_DDS` (DDS4CCM 1.1 Annex A): the
//! constructs that do not depend on the data type.
//!
//! The IDL maps as the interface mapping of this repository does: an
//! `attribute` is a getter and, unless `readonly`, a `set_` method; a
//! `setraises` clause is the error of that setter; an `exception` is a
//! structure with its members; an `inout` or `out` parameter is `&mut`.

use std::fmt;

use zerodds_dcps::status::{
    InconsistentTopicStatus, OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus,
};
use zerodds_dcps::{AnyDataReader, DdsError, InstanceHandle, Time, TopicDescriptionHandle};

/// `DataNumber_t`: a count or an index of data.
pub type DataNumber = u32;

/// `DataNumberSeq`.
pub type DataNumberSeq = Vec<DataNumber>;

/// `UNLIMITED`: the value of `max_delivered_data` that sets no limit.
pub const UNLIMITED: DataNumber = 0;

/// `DDS::ReturnCode_t`, the type of [`InternalError::error_code`]; the
/// values are those of [`zerodds_dcps::error::return_code`].
pub type ReturnCode = i32;

/// `DDS::StatusKind`: one bit of [`zerodds_dcps::psm_constants::status`].
pub type StatusKind = u32;

/// `AccessStatus`: whether the component has seen the value before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccessStatus {
    /// The value was not communicated to the component before.
    #[default]
    FreshInfo,
    /// The value was read before.
    AlreadySeen,
}

/// `InstanceStatus`: the status of the instance at sample time, as the
/// component perceives it (§7.2.2.1.2, Figure 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InstanceStatus {
    /// The component sees the instance for the first time.
    #[default]
    InstanceCreated,
    /// An existing instance reenters the filter after having been filtered
    /// out: the first fresh sample of it that passes the query of the
    /// Reader of the port again.
    InstanceFilteredIn,
    /// An existing instance is modified and stays within the filter.
    InstanceUpdated,
    /// An existing instance just stopped passing the filter: the first
    /// fresh sample of it the query of the Reader of the port rejects. The
    /// datum beside it is that sample. A get or a listener delivers the
    /// status; a read of the Reader goes through the query and does not
    /// see the sample.
    InstanceFilteredOut,
    /// The instance stopped existing. The datum beside it holds the key
    /// only.
    InstanceDeleted,
}

/// `ReadInfo`: the simplified `DDS::SampleInfo` that accompanies each datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReadInfo {
    /// The DDS instance handle.
    pub instance_handle: InstanceHandle,
    /// The DDS source timestamp.
    pub source_timestamp: Time,
    /// Whether the component has seen the value.
    pub access_status: AccessStatus,
    /// The instance status at the time of the sample.
    pub instance_status: InstanceStatus,
}

/// `ReadInfoSeq`.
pub type ReadInfoSeq = Vec<ReadInfo>;

/// `QueryFilter`: a query expression and its parameters. An empty
/// expression means no query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryFilter {
    /// The expression, by the rules of DDS.
    pub expression: String,
    /// The values of `%0`, `%1`, … in the expression.
    pub parameters: Vec<String>,
}

impl QueryFilter {
    /// A filter of `expression` with `parameters`.
    #[must_use]
    pub fn new(expression: impl Into<String>, parameters: Vec<String>) -> Self {
        Self {
            expression: expression.into(),
            parameters,
        }
    }

    /// Whether the expression is empty, which means no query.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.expression.is_empty()
    }
}

/// `ListenerMode`: how a data listener is called (§7.2.2.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ListenerMode {
    /// No operation of the listener is called. The default: the component
    /// initialises itself, with the Reader of the port, before the first
    /// notification.
    #[default]
    NotEnabled,
    /// One sample per call: `on_one_data`, `on_one_update`.
    OneByOne,
    /// A group of samples per call: `on_many_data`, `on_many_updates`.
    ManyByMany,
}

/// Exception `AlreadyCreated`: the instances a create operation found
/// existing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlreadyCreated {
    /// The indexes of the erroneous instances in the submitted sequence; `0`
    /// for a `_one` operation.
    pub indexes: DataNumberSeq,
}

/// Exception `NonExistent`: the instances an update, delete or read
/// operation found missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonExistent {
    /// The indexes of the erroneous instances in the submitted sequence; `0`
    /// for a `_one` operation.
    pub indexes: DataNumberSeq,
}

/// Exception `InternalError`: an error DDS returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalError {
    /// The DDS return code.
    pub error_code: ReturnCode,
    /// The index of the erroneous datum in the submitted sequence; `0` for
    /// an operation on one datum.
    pub index: DataNumber,
}

impl InternalError {
    /// The exception for the DDS error `error` at `index` of the submitted
    /// sequence.
    #[must_use]
    pub fn from_dds(error: &DdsError, index: DataNumber) -> Self {
        Self {
            error_code: error.as_return_code(),
            index,
        }
    }

    /// The exception for the DDS return code `error_code` of an operation on
    /// one datum.
    #[must_use]
    pub fn code(error_code: ReturnCode) -> Self {
        Self {
            error_code,
            index: 0,
        }
    }
}

impl From<DdsError> for InternalError {
    fn from(error: DdsError) -> Self {
        Self::from_dds(&error, 0)
    }
}

/// Exception `NonChangeable`: an attribute was set after the configuration
/// was complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonChangeable;

impl fmt::Display for AlreadyCreated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AlreadyCreated: indexes {:?}", self.indexes)
    }
}

impl fmt::Display for NonExistent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NonExistent: indexes {:?}", self.indexes)
    }
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InternalError: DDS return code {} at index {}",
            self.error_code, self.index
        )
    }
}

impl fmt::Display for NonChangeable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NonChangeable")
    }
}

impl std::error::Error for AlreadyCreated {}
impl std::error::Error for NonExistent {}
impl std::error::Error for InternalError {}
impl std::error::Error for NonChangeable {}

/// The exceptions of the interface `Updater`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdaterError {
    /// `AlreadyCreated`, of `create_one` and `create_many`.
    AlreadyCreated(AlreadyCreated),
    /// `NonExistent`, of the update and delete operations.
    NonExistent(NonExistent),
    /// `InternalError`.
    InternalError(InternalError),
}

/// The exceptions of the interface `Reader`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderError {
    /// `NonExistent`, of `read_one_last` and `read_one_all`.
    NonExistent(NonExistent),
    /// `InternalError`.
    InternalError(InternalError),
}

impl From<InternalError> for UpdaterError {
    fn from(e: InternalError) -> Self {
        Self::InternalError(e)
    }
}

impl From<AlreadyCreated> for UpdaterError {
    fn from(e: AlreadyCreated) -> Self {
        Self::AlreadyCreated(e)
    }
}

impl From<NonExistent> for UpdaterError {
    fn from(e: NonExistent) -> Self {
        Self::NonExistent(e)
    }
}

impl From<NonExistent> for ReaderError {
    fn from(e: NonExistent) -> Self {
        Self::NonExistent(e)
    }
}

impl From<InternalError> for ReaderError {
    fn from(e: InternalError) -> Self {
        Self::InternalError(e)
    }
}

impl fmt::Display for UpdaterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyCreated(e) => e.fmt(f),
            Self::NonExistent(e) => e.fmt(f),
            Self::InternalError(e) => e.fmt(f),
        }
    }
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonExistent(e) => e.fmt(f),
            Self::InternalError(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for UpdaterError {}
impl std::error::Error for ReaderError {}

/// Interface `DataListenerControl`: controls the data listener of the port
/// (§7.2.2.1.3).
pub trait DataListenerControl: Send + Sync {
    /// Attribute `mode`; [`ListenerMode::NotEnabled`] by default.
    fn mode(&self) -> ListenerMode;
    /// Sets `mode`. A mode other than `NotEnabled` delivers the samples the
    /// port holds and the component has not seen, on the calling thread,
    /// before it returns.
    fn set_mode(&self, mode: ListenerMode);
    /// Attribute `max_delivered_data`; [`UNLIMITED`] by default.
    fn max_delivered_data(&self) -> DataNumber;
    /// Sets `max_delivered_data`, the most samples of one `on_many_data` or
    /// `on_many_updates`.
    fn set_max_delivered_data(&self, value: DataNumber);
}

/// Interface `StateListenerControl`: a [`DataListenerControl`] for a
/// `StateListener`.
pub trait StateListenerControl: DataListenerControl {
    /// Attribute `is_filter_interpreted`; `false` by default.
    fn is_filter_interpreted(&self) -> bool;
    /// Sets `is_filter_interpreted`: whether an instance that enters the
    /// query of the Reader of the port counts as created (`on_creation`)
    /// and one that leaves it as deleted (`on_deletion`), or both as
    /// updates.
    fn set_is_filter_interpreted(&self, value: bool);
}

/// Interface `ContentFilterSetting`: changes the parameters of the content
/// filter of the port (§7.2.2.1.4).
pub trait ContentFilterSetting: Send + Sync {
    /// Sets the parameters of the filter expression.
    ///
    /// # Errors
    /// `InternalError` with the code DDS returned; `PRECONDITION_NOT_MET`
    /// when the port has no content filter.
    fn set_filter_parameters(&self, parameters: Vec<String>) -> Result<(), InternalError>;
}

/// Interface `PortStatusListener`: the statuses that are relevant to the
/// component (§7.2.2.1.5). The operations are those of the
/// `DDS::DataReaderListener`.
pub trait PortStatusListener: Send + Sync {
    /// The reader did not receive a sample within its DEADLINE.
    fn on_requested_deadline_missed(
        &self,
        the_reader: &AnyDataReader,
        status: RequestedDeadlineMissedStatus,
    );
    /// A sample was lost.
    fn on_sample_lost(&self, the_reader: &AnyDataReader, status: SampleLostStatus);
}

/// `DDS::Entity`, the reporting entity of
/// [`ConnectorStatusListener::on_unexpected_status`].
#[derive(Clone)]
pub enum DdsEntity {
    /// A topic.
    Topic(TopicDescriptionHandle),
    /// A DataReader.
    DataReader(AnyDataReader),
    /// A DataWriter.
    DataWriter(zerodds_dcps::publisher::AnyDataWriter),
}

impl fmt::Debug for DdsEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topic(t) => f.debug_tuple("Topic").field(t).finish(),
            Self::DataReader(r) => f.debug_tuple("DataReader").field(r).finish(),
            Self::DataWriter(w) => f.debug_tuple("DataWriter").field(w).finish(),
        }
    }
}

/// Interface `ConnectorStatusListener`: the statuses that are relevant
/// system-wide (§7.2.2.1.5). The operations are those of the DDS listeners;
/// `on_unexpected_status` reports every other status.
pub trait ConnectorStatusListener: Send + Sync {
    /// Another participant has the topic with another type.
    fn on_inconsistent_topic(
        &self,
        the_topic: &TopicDescriptionHandle,
        status: InconsistentTopicStatus,
    );
    /// A writer offers a QoS the reader's request is incompatible with.
    fn on_requested_incompatible_qos(
        &self,
        the_reader: &AnyDataReader,
        status: RequestedIncompatibleQosStatus,
    );
    /// The reader rejected a sample.
    fn on_sample_rejected(&self, the_reader: &AnyDataReader, status: SampleRejectedStatus);
    /// The writer missed its DEADLINE.
    fn on_offered_deadline_missed(
        &self,
        the_writer: &zerodds_dcps::publisher::AnyDataWriter,
        status: OfferedDeadlineMissedStatus,
    );
    /// A reader requests a QoS the writer's offer is incompatible with.
    fn on_offered_incompatible_qos(
        &self,
        the_writer: &zerodds_dcps::publisher::AnyDataWriter,
        status: OfferedIncompatibleQosStatus,
    );
    /// A status none of the other operations reports.
    fn on_unexpected_status(&self, the_entity: &DdsEntity, status_kind: StatusKind);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dds4ccm_annex_a_unlimited_is_zero_and_the_defaults_are_those_of_the_idl() {
        assert_eq!(UNLIMITED, 0);
        assert_eq!(ListenerMode::default(), ListenerMode::NotEnabled);
        assert!(QueryFilter::default().is_empty());
        assert!(!QueryFilter::new("x > %0", vec!["1".into()]).is_empty());
    }

    #[test]
    fn dds4ccm_annex_a_internal_error_carries_the_dds_return_code_and_the_index() {
        let e = InternalError::from_dds(&DdsError::BadParameter { what: "x" }, 3);
        assert_eq!(
            e.error_code,
            zerodds_dcps::error::return_code::BAD_PARAMETER
        );
        assert_eq!(e.index, 3);
        let e = InternalError::from(DdsError::PreconditionNotMet { reason: "x" });
        assert_eq!(
            e.error_code,
            zerodds_dcps::error::return_code::PRECONDITION_NOT_MET
        );
        assert_eq!(e.index, 0);
        assert_eq!(
            UpdaterError::from(e),
            UpdaterError::InternalError(e),
            "an InternalError is one of the exceptions of the Updater"
        );
    }
}
