// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DCPS entities behind the ports (DDS4CCM 1.1 §7.4.1) and the use of
//! QoS profiles (§7.4.3).
//!
//! # Allocation (§7.4.1)
//!
//! A port has a DataReader or a DataWriter "entirely dedicated to" it.
//! The Subscriber, the Publisher and the DomainParticipant "may be
//! allocated to the port or to the component (meaning that they will be
//! shared by the ports of that component) or to the container (meaning
//! that they will be shared by the components running in that
//! container)"; "components and containers can be given DDS entities that
//! can be used by the infrastructure for servicing embedded ports if they
//! meet the port requirements".
//!
//! A [`Container`] and each [`Component`] of it are such levels. A level
//! has default entities per domain: those it was given
//! ([`Container::set_default_entities`]) or those it creates from its QoS
//! profile at the first port that asks.
//!
//! # Profiles (§7.4.3)
//!
//! The profile of the connector "should contain all values for
//! initializing DDS Entities that are required by the connector"; it is
//! what a port requests. A profile of a component defines the "component's
//! default DomainParticipant, Subscriber, and/or Publisher", and "any
//! topic_qos, datareader_qos, or datawriter_qos is simply ignored"; a
//! profile of a container defines the "container's defaults that should be
//! used in priority, if suitable". Default entities "should be used
//! preferably if their setting is compatible with the QoS requested in
//! the connector's profile. If they are not compatible, specific entities
//! dedicated to the 'non-compatible' port will be created."
//!
//! How this crate reads it:
//!
//! - A port needs a Publisher or a Subscriber, in a DomainParticipant.
//!   The order is the container's default with the container's
//!   DomainParticipant ("in priority"), then the component's default with
//!   the DomainParticipant it lives in, then a Publisher or Subscriber
//!   dedicated to the port in the first default DomainParticipant that is
//!   compatible, then a DomainParticipant dedicated to the port.
//! - A default entity is compatible when every policy the connector's
//!   profile sets for that kind of entity has the same value in the QoS of
//!   the default entity. A policy the connector's profile does not set is
//!   not requested. A DomainParticipant must be of the connector's domain.
//! - A container always has defaults: without a profile its
//!   DomainParticipant, Publisher and Subscriber have the default QoS of
//!   DDS. A component has a default DomainParticipant when its profile has
//!   a `domainparticipant_qos` (DDS-XML 1.0 writes `domain_participant_qos`;
//!   both are read) or it was given one, and then a Publisher and a
//!   Subscriber in it; a component whose profile has only a `publisher_qos` or a
//!   `subscriber_qos` has that default entity in the DomainParticipant of
//!   the container.
//! - A default Publisher or Subscriber serves a port only together with
//!   its DomainParticipant: both must be compatible. A port with a
//!   dedicated DomainParticipant has a dedicated Publisher or Subscriber.
//! - A `DDS_Update` port with `is_global_scope` has two entities, its
//!   DataWriter and the DataReader it looks the instances up on. Each is
//!   placed as above; a DomainParticipant dedicated to the port holds both,
//!   the DataReader in a Subscriber dedicated to the port.
//! - A topic is one per name and data type in a DomainParticipant. A topic
//!   that exists there, of another connector or of the application, keeps
//!   its QoS and serves a connector when it is compatible in the same
//!   sense: every policy the `topic_qos` of the connector's profile sets
//!   has the same value. Otherwise the port is refused; there is no
//!   dedicated topic.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use zerodds_dcps::error::return_code;
use zerodds_dcps::qos::HistoryKind;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DdsError, DomainId, DomainParticipant, DomainParticipantFactory,
    DomainParticipantQos, Entity, InstanceHandle, Publisher, PublisherQos, Subscriber,
    SubscriberQos, TopicQos,
};
use zerodds_xml::{
    CCM_LIBRARY, EntityQos, FilteredEntityQos, QosLibrary, ReadOptions, ResolvedQos, XmlError,
    default_qos, parse_dds_xml_with, resolve_profile, select_qos_for_topic,
};

use crate::ccm_dds::InternalError;
use crate::reader_core::lock;
use crate::threading::{ThreadPool, ThreadingPolicy};

/// Why a QoS profile could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// The value is neither an XML string nor a file URL, or the URL does
    /// not name a local file.
    Url(String),
    /// The file of the URL could not be read.
    File {
        /// The path of the URL.
        path: String,
        /// The error of the operating system.
        message: String,
    },
    /// The document is not a QoS profile document.
    Xml(XmlError),
    /// The document has no profile.
    NoProfile,
    /// The document has no profile of the name the URL asks for.
    UnknownProfile(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Url(why) => write!(f, "qos_profile: {why}"),
            Self::File { path, message } => write!(f, "qos_profile: cannot read {path}: {message}"),
            Self::Xml(error) => write!(f, "qos_profile: {error}"),
            Self::NoProfile => f.write_str("qos_profile: the document has no profile"),
            Self::UnknownProfile(name) => {
                write!(f, "qos_profile: the document has no profile `{name}`")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

/// A QoS profile (§7.4.2.3), resolved over its `base_name` chain.
///
/// The value of an attribute `qos_profile` is "a file URL or the XML
/// string itself" (§7.3.1):
///
/// - a value that starts with `<` is the XML string;
/// - a `file:` URL names the document (`file:///path`, `file:/path`,
///   `file://localhost/path`, with `%xx` escapes); on Windows a path with
///   a drive letter is written `file:///C:/path` (RFC 8089 Appendix E.2);
/// - the empty value is no profile: every entity has the default QoS of
///   DDS.
///
/// The document is read with the forms of DDS4CCM that DDS-XML 1.0 does
/// not have (profiles directly under `<dds>`, `domainparticipant_qos`).
/// It may hold several profiles, as `base_name` needs. The profile used is
/// the last one directly under `<dds>`, since a profile can only inherit
/// from those before it (§7.4.2.3.1). A document that has its profiles in
/// `<qos_library>` elements only (the form of DDS-XML 1.0) gives the last
/// profile of its last library that has one; where a document has both,
/// the profiles directly under `<dds>` come first, wherever the libraries
/// stand. The fragment of a URL names another profile:
/// `file:///path#Profile`, or `#Library::Profile` for a profile of a
/// `<qos_library>`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    resolved: Option<ResolvedQos>,
}

fn percent_decoded(text: &str) -> Result<String, ProfileError> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let escape = text
                .get(at + 1..at + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                .ok_or_else(|| ProfileError::Url(format!("bad % escape in `{text}`")))?;
            out.push(escape);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ProfileError::Url(format!("`{text}` is not UTF-8")))
}

/// The path of a `file:` URL that starts with a drive letter, without the
/// slash before the letter: `/C:/dir/file` is `C:/dir/file` (RFC 8089
/// Appendix E.2). `None` for a path without a drive letter.
fn drive_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix('/')?;
    let bytes = rest.as_bytes();
    let drive = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && matches!(bytes.get(2), None | Some(b'/'));
    drive.then_some(rest)
}

/// The path and the fragment of a `file:` URL. On Windows the path of
/// `file:///C:/dir/file` is `C:/dir/file`; elsewhere `/C:/dir/file` is a
/// path like any other.
fn file_url(value: &str) -> Result<(String, Option<String>), ProfileError> {
    let scheme = value.get(..5).unwrap_or_default();
    if !scheme.eq_ignore_ascii_case("file:") {
        return Err(ProfileError::Url(format!(
            "`{value}` is neither an XML string nor a file URL"
        )));
    }
    let rest = &value[5..];
    let (rest, fragment) = match rest.split_once('#') {
        Some((rest, fragment)) => (rest, Some(percent_decoded(fragment)?)),
        None => (rest, None),
    };
    let path = match rest.strip_prefix("//") {
        Some(with_authority) => {
            let (authority, path) = match with_authority.find('/') {
                Some(slash) => with_authority.split_at(slash),
                None => (with_authority, ""),
            };
            if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
                return Err(ProfileError::Url(format!(
                    "`{value}` names the host `{authority}`, not a local file"
                )));
            }
            path
        }
        None => rest,
    };
    if !path.starts_with('/') {
        return Err(ProfileError::Url(format!("`{value}` has no absolute path")));
    }
    let path = percent_decoded(path)?;
    let local = drive_path(&path).filter(|_| cfg!(windows));
    let path = local.map_or_else(|| path.clone(), str::to_owned);
    Ok((path, fragment))
}

/// `Library::Profile` of the profile to use. The profiles directly under
/// `<dds>` are in the library the reader appends last, after every
/// `<qos_library>` of the document: without a name the last of them is
/// the profile, and only a document without any gives the last profile of
/// its last `<qos_library>`.
fn lookup_path(libraries: &[QosLibrary], selected: Option<&str>) -> Result<String, ProfileError> {
    let Some(name) = selected else {
        return libraries
            .iter()
            .rev()
            .find_map(|library| {
                let profile = library.profiles.last()?;
                Some(format!("{}::{}", library.name, profile.name))
            })
            .ok_or(ProfileError::NoProfile);
    };
    if name.contains("::") {
        return Ok(name.to_owned());
    }
    let top_level = libraries.iter().filter(|l| l.name == CCM_LIBRARY);
    let in_libraries = libraries.iter().filter(|l| l.name != CCM_LIBRARY);
    top_level
        .chain(in_libraries)
        .find(|library| library.profile(name).is_some())
        .map(|library| format!("{}::{name}", library.name))
        .ok_or_else(|| ProfileError::UnknownProfile(name.to_owned()))
}

fn or_default<P: Clone>(set: Option<&P>, default: P) -> P {
    set.cloned().unwrap_or(default)
}

fn participant_qos(qos: Option<&EntityQos>) -> DomainParticipantQos {
    let default = DomainParticipantQos::default();
    let Some(qos) = qos else {
        return default;
    };
    DomainParticipantQos {
        user_data: or_default(qos.user_data.as_ref(), default.user_data),
        entity_factory: or_default(qos.entity_factory.as_ref(), default.entity_factory),
        property: default.property,
    }
}

fn publisher_qos(qos: Option<&EntityQos>) -> PublisherQos {
    publisher_qos_over(PublisherQos::default(), qos)
}

/// `base` with the policies `qos` sets.
fn publisher_qos_over(base: PublisherQos, qos: Option<&EntityQos>) -> PublisherQos {
    let Some(qos) = qos else {
        return base;
    };
    PublisherQos {
        presentation: or_default(qos.presentation.as_ref(), base.presentation),
        partition: or_default(qos.partition.as_ref(), base.partition),
        group_data: or_default(qos.group_data.as_ref(), base.group_data),
        entity_factory: or_default(qos.entity_factory.as_ref(), base.entity_factory),
    }
}

fn subscriber_qos(qos: Option<&EntityQos>) -> SubscriberQos {
    subscriber_qos_over(SubscriberQos::default(), qos)
}

/// `base` with the policies `qos` sets.
fn subscriber_qos_over(base: SubscriberQos, qos: Option<&EntityQos>) -> SubscriberQos {
    let Some(qos) = qos else {
        return base;
    };
    SubscriberQos {
        presentation: or_default(qos.presentation.as_ref(), base.presentation),
        partition: or_default(qos.partition.as_ref(), base.partition),
        group_data: or_default(qos.group_data.as_ref(), base.group_data),
        entity_factory: or_default(qos.entity_factory.as_ref(), base.entity_factory),
    }
}

/// `base` with the policies a `datawriter_qos` sets, as
/// `EntityQos::into_writer_qos` reads them.
fn datawriter_qos_over(base: DataWriterQos, qos: Option<&EntityQos>) -> DataWriterQos {
    let Some(qos) = qos else {
        return base;
    };
    DataWriterQos {
        durability: or_default(qos.durability.as_ref(), base.durability),
        durability_service: or_default(qos.durability_service.as_ref(), base.durability_service),
        deadline: or_default(qos.deadline.as_ref(), base.deadline),
        latency_budget: or_default(qos.latency_budget.as_ref(), base.latency_budget),
        liveliness: or_default(qos.liveliness.as_ref(), base.liveliness),
        reliability: or_default(qos.reliability.as_ref(), base.reliability),
        destination_order: or_default(qos.destination_order.as_ref(), base.destination_order),
        history: or_default(qos.history.as_ref(), base.history),
        resource_limits: or_default(qos.resource_limits.as_ref(), base.resource_limits),
        transport_priority: or_default(qos.transport_priority.as_ref(), base.transport_priority),
        lifespan: or_default(qos.lifespan.as_ref(), base.lifespan),
        ownership: or_default(qos.ownership.as_ref(), base.ownership),
        ownership_strength: or_default(qos.ownership_strength.as_ref(), base.ownership_strength),
        presentation: or_default(qos.presentation.as_ref(), base.presentation),
        partition: or_default(qos.partition.as_ref(), base.partition),
        writer_data_lifecycle: or_default(
            qos.writer_data_lifecycle.as_ref(),
            base.writer_data_lifecycle,
        ),
        user_data: or_default(qos.user_data.as_ref(), base.user_data),
        topic_data: or_default(qos.topic_data.as_ref(), base.topic_data),
        group_data: or_default(qos.group_data.as_ref(), base.group_data),
        ..base
    }
}

/// `base` with the policies a `datareader_qos` sets, as
/// `EntityQos::into_reader_qos` reads them.
fn datareader_qos_over(base: DataReaderQos, qos: Option<&EntityQos>) -> DataReaderQos {
    let Some(qos) = qos else {
        return base;
    };
    DataReaderQos {
        durability: or_default(qos.durability.as_ref(), base.durability),
        deadline: or_default(qos.deadline.as_ref(), base.deadline),
        latency_budget: or_default(qos.latency_budget.as_ref(), base.latency_budget),
        liveliness: or_default(qos.liveliness.as_ref(), base.liveliness),
        reliability: or_default(qos.reliability.as_ref(), base.reliability),
        destination_order: or_default(qos.destination_order.as_ref(), base.destination_order),
        history: or_default(qos.history.as_ref(), base.history),
        resource_limits: or_default(qos.resource_limits.as_ref(), base.resource_limits),
        ownership: or_default(qos.ownership.as_ref(), base.ownership),
        time_based_filter: or_default(qos.time_based_filter.as_ref(), base.time_based_filter),
        presentation: or_default(qos.presentation.as_ref(), base.presentation),
        partition: or_default(qos.partition.as_ref(), base.partition),
        reader_data_lifecycle: or_default(
            qos.reader_data_lifecycle.as_ref(),
            base.reader_data_lifecycle,
        ),
        user_data: or_default(qos.user_data.as_ref(), base.user_data),
        topic_data: or_default(qos.topic_data.as_ref(), base.topic_data),
        group_data: or_default(qos.group_data.as_ref(), base.group_data),
        ..base
    }
}

fn topic_qos(qos: Option<&EntityQos>) -> TopicQos {
    let default = TopicQos::default();
    let Some(qos) = qos else {
        return default;
    };
    TopicQos {
        durability: or_default(qos.durability.as_ref(), default.durability),
        durability_service: or_default(qos.durability_service.as_ref(), default.durability_service),
        deadline: or_default(qos.deadline.as_ref(), default.deadline),
        latency_budget: or_default(qos.latency_budget.as_ref(), default.latency_budget),
        liveliness: or_default(qos.liveliness.as_ref(), default.liveliness),
        reliability: or_default(qos.reliability.as_ref(), default.reliability),
        destination_order: or_default(qos.destination_order.as_ref(), default.destination_order),
        history: or_default(qos.history.as_ref(), default.history),
        resource_limits: or_default(qos.resource_limits.as_ref(), default.resource_limits),
        transport_priority: or_default(qos.transport_priority.as_ref(), default.transport_priority),
        lifespan: or_default(qos.lifespan.as_ref(), default.lifespan),
        ownership: or_default(qos.ownership.as_ref(), default.ownership),
        topic_data: or_default(qos.topic_data.as_ref(), default.topic_data),
        data_representation: default.data_representation,
    }
}

fn same<P: PartialEq>(requested: Option<&P>, has: &P) -> bool {
    requested.is_none_or(|requested| requested == has)
}

/// §7.4.3: a default DomainParticipant is compatible "with the QoS
/// requested in the connector's profile".
fn participant_meets(has: &DomainParticipantQos, requested: Option<&EntityQos>) -> bool {
    requested.is_none_or(|r| {
        same(r.user_data.as_ref(), &has.user_data)
            && same(r.entity_factory.as_ref(), &has.entity_factory)
    })
}

fn publisher_meets(has: &PublisherQos, requested: Option<&EntityQos>) -> bool {
    requested.is_none_or(|r| {
        same(r.presentation.as_ref(), &has.presentation)
            && same(r.partition.as_ref(), &has.partition)
            && same(r.group_data.as_ref(), &has.group_data)
            && same(r.entity_factory.as_ref(), &has.entity_factory)
    })
}

fn subscriber_meets(has: &SubscriberQos, requested: Option<&EntityQos>) -> bool {
    requested.is_none_or(|r| {
        same(r.presentation.as_ref(), &has.presentation)
            && same(r.partition.as_ref(), &has.partition)
            && same(r.group_data.as_ref(), &has.group_data)
            && same(r.entity_factory.as_ref(), &has.entity_factory)
    })
}

impl Profile {
    /// Reads the value of an attribute `qos_profile`.
    ///
    /// # Errors
    /// [`ProfileError`]: the value is neither form, the file or the
    /// document cannot be read, or the profile is not in it.
    pub fn read(qos_profile: &str) -> Result<Self, ProfileError> {
        let value = qos_profile.trim();
        if value.is_empty() {
            return Ok(Self::default());
        }
        let (document, selected) = if value.starts_with('<') {
            (value.to_owned(), None)
        } else {
            let (path, fragment) = file_url(value)?;
            let document = std::fs::read_to_string(&path).map_err(|e| ProfileError::File {
                path,
                message: e.to_string(),
            })?;
            (document, fragment)
        };
        let libraries = parse_dds_xml_with(&document, &ReadOptions::accept_vendor_forms())
            .map_err(ProfileError::Xml)?
            .value
            .qos_libraries;
        let path = lookup_path(&libraries, selected.as_deref())?;
        let resolved = resolve_profile(&libraries, &path).map_err(|error| match error {
            XmlError::UnresolvedReference(_) if selected.is_some() => {
                ProfileError::UnknownProfile(path.clone())
            }
            other => ProfileError::Xml(other),
        })?;
        Ok(Self {
            resolved: Some(resolved),
        })
    }

    /// `Library::Profile` of the profile; `None` for no profile. The
    /// profiles directly under `<dds>` are in the library `(dds)`.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.resolved.as_ref().map(|r| r.lookup_path.as_str())
    }

    fn container(
        &self,
        pick: fn(&ResolvedQos) -> &Vec<FilteredEntityQos>,
        topic_name: Option<&str>,
    ) -> Option<&EntityQos> {
        let slots = pick(self.resolved.as_ref()?);
        match topic_name {
            // §7.4.3: "the topic_filter feature of the QoS Profile may be
            // used to properly allocate values to entities".
            Some(topic_name) => select_qos_for_topic(slots, topic_name),
            None => default_qos(slots),
        }
    }

    /// The `domain_participant_qos` the profile has for `topic_name`: what
    /// a connector requests of a DomainParticipant.
    pub(crate) fn participant(&self, topic_name: Option<&str>) -> Option<&EntityQos> {
        self.container(|r| &r.domainparticipant_qos, topic_name)
    }

    pub(crate) fn publisher(&self, topic_name: Option<&str>) -> Option<&EntityQos> {
        self.container(|r| &r.publisher_qos, topic_name)
    }

    pub(crate) fn subscriber(&self, topic_name: Option<&str>) -> Option<&EntityQos> {
        self.container(|r| &r.subscriber_qos, topic_name)
    }

    /// The QoS of the topic `topic_name`: the policies of the `topic_qos`
    /// the profile has for it, the default of DDS for the others.
    #[must_use]
    pub fn topic_qos(&self, topic_name: &str) -> TopicQos {
        topic_qos(self.container(|r| &r.topic_qos, Some(topic_name)))
    }

    /// The policies the `topic_qos` of the profile sets for `topic_name`
    /// that have another value in `has`, the QoS of a topic that exists,
    /// by their XML names. A policy the profile does not set is not
    /// requested (§7.4.3).
    pub(crate) fn topic_qos_differences(
        &self,
        topic_name: &str,
        has: &TopicQos,
    ) -> Vec<&'static str> {
        let Some(r) = self.container(|r| &r.topic_qos, Some(topic_name)) else {
            return Vec::new();
        };
        let policies = [
            ("durability", same(r.durability.as_ref(), &has.durability)),
            (
                "durability_service",
                same(r.durability_service.as_ref(), &has.durability_service),
            ),
            ("deadline", same(r.deadline.as_ref(), &has.deadline)),
            (
                "latency_budget",
                same(r.latency_budget.as_ref(), &has.latency_budget),
            ),
            ("liveliness", same(r.liveliness.as_ref(), &has.liveliness)),
            (
                "reliability",
                same(r.reliability.as_ref(), &has.reliability),
            ),
            (
                "destination_order",
                same(r.destination_order.as_ref(), &has.destination_order),
            ),
            ("history", same(r.history.as_ref(), &has.history)),
            (
                "resource_limits",
                same(r.resource_limits.as_ref(), &has.resource_limits),
            ),
            (
                "transport_priority",
                same(r.transport_priority.as_ref(), &has.transport_priority),
            ),
            ("lifespan", same(r.lifespan.as_ref(), &has.lifespan)),
            ("ownership", same(r.ownership.as_ref(), &has.ownership)),
            ("topic_data", same(r.topic_data.as_ref(), &has.topic_data)),
        ];
        let differing = policies.iter().filter(|(_, same)| !same);
        differing.map(|(name, _)| *name).collect()
    }

    /// The QoS of a DataWriter of the topic `topic_name`.
    #[must_use]
    pub fn datawriter_qos(&self, topic_name: &str) -> DataWriterQos {
        self.container(|r| &r.datawriter_qos, Some(topic_name))
            .map_or_else(DataWriterQos::default, |qos| qos.into_writer_qos().into())
    }

    /// The QoS of a DataReader of the topic `topic_name`.
    #[must_use]
    pub fn datareader_qos(&self, topic_name: &str) -> DataReaderQos {
        self.container(|r| &r.datareader_qos, Some(topic_name))
            .map_or_else(DataReaderQos::default, |qos| qos.into_reader_qos().into())
    }

    /// [`Self::datawriter_qos`] for a connector whose pattern has the
    /// HISTORY kind `history` (§7.3.2, §7.3.3): the kind of the pattern
    /// where the profile does not set HISTORY.
    pub(crate) fn datawriter_qos_of_pattern(
        &self,
        topic_name: &str,
        history: HistoryKind,
    ) -> DataWriterQos {
        let mut qos = self.datawriter_qos(topic_name);
        let set = self.container(|r| &r.datawriter_qos, Some(topic_name));
        if set.is_none_or(|qos| qos.history.is_none()) {
            qos.history.kind = history;
        }
        qos
    }

    /// [`Self::datareader_qos`] for a connector whose pattern has the
    /// HISTORY kind `history`; see [`Self::datawriter_qos_of_pattern`].
    pub(crate) fn datareader_qos_of_pattern(
        &self,
        topic_name: &str,
        history: HistoryKind,
    ) -> DataReaderQos {
        let mut qos = self.datareader_qos(topic_name);
        let set = self.container(|r| &r.datareader_qos, Some(topic_name));
        if set.is_none_or(|qos| qos.history.is_none()) {
            qos.history.kind = history;
        }
        qos
    }

    /// The container of the kind `pick` for an entity that serves all
    /// `topic_names` together: the Publisher, the Subscriber and the
    /// DomainParticipant of a DLRL port (§8.4.2). It is the container
    /// `topic_filter` selects for each of the topics, as for the entity
    /// of one topic (§7.4.3); `None` when no container is selected for
    /// any of them.
    ///
    /// # Errors
    /// Two of the topics, in the order of `topic_names`, whose selected
    /// containers differ (one selected and the other not, or two with
    /// other policies): the one entity cannot take both, and the profile
    /// does not say which one applies.
    fn shared_container(
        &self,
        pick: fn(&ResolvedQos) -> &Vec<FilteredEntityQos>,
        topic_names: &[String],
    ) -> Result<Option<&EntityQos>, [String; 2]> {
        let mut first: Option<(&String, Option<&EntityQos>)> = None;
        for topic_name in topic_names {
            let selected = self.container(pick, Some(topic_name));
            match first {
                None => first = Some((topic_name, selected)),
                Some((of_first, held)) if held != selected => {
                    return Err([of_first.clone(), topic_name.clone()]);
                }
                Some(_) => {}
            }
        }
        Ok(first.and_then(|(_, selected)| selected))
    }

    /// The `domainparticipant_qos` for a DomainParticipant that serves
    /// all `topic_names`; see [`Self::shared_container`].
    pub(crate) fn participant_of_topics(
        &self,
        topic_names: &[String],
    ) -> Result<Option<&EntityQos>, [String; 2]> {
        self.shared_container(|r| &r.domainparticipant_qos, topic_names)
    }

    /// `base`, the QoS of a Publisher that exists and serves all
    /// `topic_names`, with the policies the `publisher_qos` selected for
    /// them sets; see [`Self::shared_container`].
    pub(crate) fn publisher_qos_over(
        &self,
        topic_names: &[String],
        base: PublisherQos,
    ) -> Result<PublisherQos, [String; 2]> {
        let selected = self.shared_container(|r| &r.publisher_qos, topic_names)?;
        Ok(publisher_qos_over(base, selected))
    }

    /// `base`, the QoS of a Subscriber that exists and serves all
    /// `topic_names`, with the policies the `subscriber_qos` selected for
    /// them sets; see [`Self::shared_container`].
    pub(crate) fn subscriber_qos_over(
        &self,
        topic_names: &[String],
        base: SubscriberQos,
    ) -> Result<SubscriberQos, [String; 2]> {
        let selected = self.shared_container(|r| &r.subscriber_qos, topic_names)?;
        Ok(subscriber_qos_over(base, selected))
    }

    /// `base`, the QoS of a DataWriter of the topic `topic_name` that
    /// exists, with the policies the `datawriter_qos` of the profile sets
    /// for that topic.
    pub(crate) fn datawriter_qos_over(
        &self,
        topic_name: &str,
        base: DataWriterQos,
    ) -> DataWriterQos {
        datawriter_qos_over(
            base,
            self.container(|r| &r.datawriter_qos, Some(topic_name)),
        )
    }

    /// `base`, the QoS of a DataReader of the topic `topic_name` that
    /// exists, with the policies the `datareader_qos` of the profile sets
    /// for that topic.
    pub(crate) fn datareader_qos_over(
        &self,
        topic_name: &str,
        base: DataReaderQos,
    ) -> DataReaderQos {
        datareader_qos_over(
            base,
            self.container(|r| &r.datareader_qos, Some(topic_name)),
        )
    }
}

/// A Publisher or a Subscriber: the entity a port's DataWriter or
/// DataReader is created in.
pub(crate) trait Group: Sized {
    /// The container of this kind the profile has for `topic_name`.
    fn requested<'a>(profile: &'a Profile, topic_name: Option<&str>) -> Option<&'a EntityQos>;
    /// Creates one in `participant` with the policies of `qos`.
    fn create(participant: &DomainParticipant, qos: Option<&EntityQos>) -> Result<Self, DdsError>;
    /// §7.4.3: its setting "is compatible with the QoS requested in the
    /// connector's profile".
    fn meets(&self, requested: Option<&EntityQos>) -> bool;
    /// Deletes it; its DataWriters or DataReaders are deleted before.
    fn delete(&self) -> Result<(), DdsError>;
    /// Where a level keeps its default of this kind.
    #[doc(hidden)]
    fn slot(defaults: &mut DomainDefaults) -> &mut Option<(InstanceHandle, Held<Arc<Self>>)>;
}

impl Group for Publisher {
    fn requested<'a>(profile: &'a Profile, topic_name: Option<&str>) -> Option<&'a EntityQos> {
        profile.publisher(topic_name)
    }

    fn create(participant: &DomainParticipant, qos: Option<&EntityQos>) -> Result<Self, DdsError> {
        participant.try_create_publisher(publisher_qos(qos))
    }

    fn meets(&self, requested: Option<&EntityQos>) -> bool {
        publisher_meets(&self.get_qos(), requested)
    }

    fn delete(&self) -> Result<(), DdsError> {
        // A Publisher whose DomainParticipant is gone went with it.
        match self.get_participant() {
            Some(participant) => participant.delete_publisher(self),
            None => Ok(()),
        }
    }

    fn slot(defaults: &mut DomainDefaults) -> &mut Option<(InstanceHandle, Held<Arc<Self>>)> {
        &mut defaults.publisher
    }
}

impl Group for Subscriber {
    fn requested<'a>(profile: &'a Profile, topic_name: Option<&str>) -> Option<&'a EntityQos> {
        profile.subscriber(topic_name)
    }

    fn create(participant: &DomainParticipant, qos: Option<&EntityQos>) -> Result<Self, DdsError> {
        participant.try_create_subscriber(subscriber_qos(qos))
    }

    fn meets(&self, requested: Option<&EntityQos>) -> bool {
        subscriber_meets(&self.get_qos(), requested)
    }

    fn delete(&self) -> Result<(), DdsError> {
        match self.get_participant() {
            Some(participant) => participant.delete_subscriber(self),
            None => Ok(()),
        }
    }

    fn slot(defaults: &mut DomainDefaults) -> &mut Option<(InstanceHandle, Held<Arc<Self>>)> {
        &mut defaults.subscriber
    }
}

/// Where a port is placed (§7.4.1): its Publisher or Subscriber and the
/// DomainParticipant of that, each shared with the component or the
/// container or dedicated to the port.
pub(crate) struct Placement<G> {
    pub(crate) participant: DomainParticipant,
    /// The connector created `participant` for this port.
    pub(crate) dedicated_participant: bool,
    pub(crate) group: Arc<G>,
    /// The connector created `group` for this port.
    pub(crate) dedicated_group: bool,
}

/// A default entity of a level, and whether the level created it.
pub(crate) struct Held<E> {
    entity: E,
    created: bool,
}

/// The default entities of a level in one domain.
#[derive(Default)]
pub(crate) struct DomainDefaults {
    participant: Option<Held<DomainParticipant>>,
    /// With the DomainParticipant it lives in.
    publisher: Option<(InstanceHandle, Held<Arc<Publisher>>)>,
    subscriber: Option<(InstanceHandle, Held<Arc<Subscriber>>)>,
}

#[derive(Default)]
struct LevelState {
    profile: Profile,
    /// A port asked this level for an entity: its profile is fixed.
    used: bool,
    domains: BTreeMap<DomainId, DomainDefaults>,
    threading: Option<ThreadingPolicy>,
    pool: Option<Arc<ThreadPool>>,
}

/// A container or a component as a holder of default entities and of a
/// thread pool.
struct Level {
    name: String,
    state: Mutex<LevelState>,
}

/// Why a container or a component refused a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelError {
    /// A port uses the default entities: they and the profile are fixed.
    NonChangeable,
    /// The QoS profile could not be read.
    QosProfile(ProfileError),
    /// The entities given do not belong together: the Publisher or the
    /// Subscriber is not of the DomainParticipant.
    ForeignEntity,
}

impl std::fmt::Display for LevelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonChangeable => f.write_str("the default entities are in use"),
            Self::QosProfile(error) => error.fmt(f),
            Self::ForeignEntity => {
                f.write_str("the Publisher or Subscriber is not of the DomainParticipant")
            }
        }
    }
}

impl std::error::Error for LevelError {}

impl Level {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            state: Mutex::new(LevelState::default()),
        }
    }

    fn set_qos_profile(&self, qos_profile: &str) -> Result<(), LevelError> {
        let profile = Profile::read(qos_profile).map_err(LevelError::QosProfile)?;
        let mut state = lock(&self.state);
        if state.used {
            return Err(LevelError::NonChangeable);
        }
        state.profile = profile;
        Ok(())
    }

    fn set_default_entities(
        &self,
        participant: DomainParticipant,
        publisher: Option<Arc<Publisher>>,
        subscriber: Option<Arc<Subscriber>>,
    ) -> Result<(), LevelError> {
        let home = participant.instance_handle();
        let of_participant = |owner: Option<DomainParticipant>| {
            owner.is_some_and(|owner| owner.instance_handle() == home)
        };
        if publisher
            .as_ref()
            .is_some_and(|p| !of_participant(p.get_participant()))
            || subscriber
                .as_ref()
                .is_some_and(|s| !of_participant(s.get_participant()))
        {
            return Err(LevelError::ForeignEntity);
        }
        let mut state = lock(&self.state);
        if state.used {
            return Err(LevelError::NonChangeable);
        }
        fn given<E>(entity: E) -> Held<E> {
            Held {
                entity,
                created: false,
            }
        }
        state.domains.insert(
            participant.domain_id(),
            DomainDefaults {
                participant: Some(given(participant)),
                publisher: publisher.map(|p| (home, given(p))),
                subscriber: subscriber.map(|s| (home, given(s))),
            },
        );
        Ok(())
    }

    /// The default DomainParticipant of the level in `domain`, which the
    /// level creates at the first call if it has one: always (`always`),
    /// or when its profile has a `domain_participant_qos`.
    fn participant(
        &self,
        domain: DomainId,
        always: bool,
        container: &ContainerInner,
    ) -> Result<Option<DomainParticipant>, DdsError> {
        let mut state = lock(&self.state);
        state.used = true;
        let qos = state.profile.participant(None).cloned();
        let defaults = state.domains.entry(domain).or_default();
        if let Some(held) = &defaults.participant {
            return Ok(Some(held.entity.clone()));
        }
        if !always && qos.is_none() {
            return Ok(None);
        }
        let participant = container.create_participant(domain, participant_qos(qos.as_ref()))?;
        defaults.participant = Some(Held {
            entity: participant.clone(),
            created: true,
        });
        Ok(Some(participant))
    }

    /// The default Publisher or Subscriber of the level in `participant`,
    /// the DomainParticipant the level lives in. The level has one when
    /// `participant` is its own default DomainParticipant, or when its
    /// profile has a container of that kind; it creates it at the first
    /// call.
    fn group<G: Group>(&self, participant: &DomainParticipant) -> Result<Option<Arc<G>>, DdsError> {
        let handle = participant.instance_handle();
        let mut state = lock(&self.state);
        state.used = true;
        let qos = G::requested(&state.profile, None).cloned();
        let defaults = state.domains.entry(participant.domain_id()).or_default();
        let owns = defaults
            .participant
            .as_ref()
            .is_some_and(|held| held.entity.instance_handle() == handle);
        let slot = G::slot(defaults);
        if let Some((home, held)) = slot {
            return Ok((*home == handle).then(|| Arc::clone(&held.entity)));
        }
        if !owns && qos.is_none() {
            return Ok(None);
        }
        let group = Arc::new(G::create(participant, qos.as_ref())?);
        *slot = Some((
            handle,
            Held {
                entity: Arc::clone(&group),
                created: true,
            },
        ));
        Ok(Some(group))
    }

    /// The thread pool of the level's threading policy; `None` without a
    /// policy. A policy set after the pool was started gives a new pool to
    /// the facets activated from then on.
    fn pool(&self, name: &str) -> std::io::Result<Option<Arc<ThreadPool>>> {
        let mut state = lock(&self.state);
        let Some(policy) = state.threading else {
            return Ok(None);
        };
        if let Some(pool) = state.pool.as_ref().filter(|pool| pool.policy() == policy) {
            return Ok(Some(Arc::clone(pool)));
        }
        let pool = ThreadPool::new(name, policy)?;
        state.pool = Some(Arc::clone(&pool));
        Ok(Some(pool))
    }

    /// Notes in `survey` what the level holds as defaults: the check
    /// before a removal.
    fn survey(&self, survey: &mut Survey) {
        let state = lock(&self.state);
        for defaults in state.domains.values() {
            if let Some((home, held)) = &defaults.publisher {
                survey.publishers.push(*home);
                survey.in_use |= held.created && held.entity.has_datawriters();
            }
            if let Some((home, held)) = &defaults.subscriber {
                survey.subscribers.push(*home);
                survey.in_use |= held.created && held.entity.has_datareaders();
            }
            if let Some(held) = defaults.participant.as_ref().filter(|held| held.created) {
                survey.created.push(held.entity.clone());
            }
        }
    }

    /// Deletes the default entities the level created and forgets those
    /// it was given. What DDS does not delete stays a default of the
    /// level, with the other defaults of its domain; a DomainParticipant
    /// is deleted without what it contains, so one that still has an
    /// entity stays. Returns the first error; the others are still tried.
    fn remove(&self) -> Result<(), DdsError> {
        fn delete<G: Group>(
            slot: &mut Option<(InstanceHandle, Held<Arc<G>>)>,
        ) -> Result<(), DdsError> {
            let Some((home, held)) = slot.take_if(|(_, held)| held.created) else {
                return Ok(());
            };
            held.entity.delete().inspect_err(|_| {
                *slot = Some((home, held));
            })
        }
        let mut state = lock(&self.state);
        let mut first = Ok(());
        let mut note = |result: Result<(), DdsError>| {
            let failed = result.is_err();
            if first.is_ok() {
                first = result;
            }
            failed
        };
        for (domain, mut defaults) in std::mem::take(&mut state.domains) {
            let mut kept = note(delete(&mut defaults.publisher));
            kept |= note(delete(&mut defaults.subscriber));
            if !kept && let Some(held) = defaults.participant.take_if(|held| held.created) {
                let factory = DomainParticipantFactory::instance();
                if note(factory.delete_participant(&held.entity)) {
                    defaults.participant = Some(held);
                    kept = true;
                }
            }
            if kept {
                state.domains.insert(domain, defaults);
            }
        }
        if state.domains.is_empty() {
            state.used = false;
            state.pool = None;
        }
        first
    }
}

/// What the levels of a container hold as default entities.
#[derive(Default)]
struct Survey {
    /// A default Publisher or Subscriber a level created has a DataWriter
    /// or a DataReader.
    in_use: bool,
    /// The DomainParticipant of each default Publisher.
    publishers: Vec<InstanceHandle>,
    /// The DomainParticipant of each default Subscriber.
    subscribers: Vec<InstanceHandle>,
    /// The default DomainParticipants a level created.
    created: Vec<DomainParticipant>,
}

impl Survey {
    /// Whether a port still has its DataWriter or DataReader in a default
    /// entity a level created: in a default Publisher or Subscriber, or in
    /// a Publisher or Subscriber dedicated to it in a default
    /// DomainParticipant.
    fn serves_a_port(&self) -> bool {
        let of = |homes: &[InstanceHandle], participant: InstanceHandle| {
            homes.iter().filter(|home| **home == participant).count()
        };
        self.in_use
            || self.created.iter().any(|participant| {
                let handle = participant.instance_handle();
                participant.publishers_len() > of(&self.publishers, handle)
                    || participant.subscribers_len() > of(&self.subscribers, handle)
            })
    }
}

/// Deletes a DomainParticipant with what it still contains.
pub(crate) fn delete_participant(participant: &DomainParticipant) -> Result<(), DdsError> {
    participant.delete_contained_entities()?;
    DomainParticipantFactory::instance().delete_participant(participant)
}

struct ContainerInner {
    level: Level,
    /// The runtime configuration of the DomainParticipants the container
    /// creates; `None` reads the standard configuration.
    runtime: Option<RuntimeConfig>,
    components: Mutex<Vec<Arc<Level>>>,
}

impl ContainerInner {
    fn create_participant(
        &self,
        domain: DomainId,
        qos: DomainParticipantQos,
    ) -> Result<DomainParticipant, DdsError> {
        let factory = DomainParticipantFactory::instance();
        match &self.runtime {
            Some(config) => factory.create_participant_with_config(domain, qos, config.clone()),
            None => factory.create_participant(domain, qos),
        }
    }
}

/// A DDS-capable container: "a container hosting DDS-capable components"
/// (§7.4.3). It holds the default entities its components share and the
/// container level of the threading policy (§7.4.4).
#[derive(Clone)]
pub struct Container {
    inner: Arc<ContainerInner>,
}

/// A DDS-capable component: "a component that has at least one DDS port"
/// (§7.4.3). It holds the default entities its ports share and the
/// component level of the threading policy (§7.4.4).
#[derive(Clone)]
pub struct Component {
    container: Arc<ContainerInner>,
    level: Arc<Level>,
}

/// The configuration a container and a component share.
macro_rules! level_configuration {
    ($holder:ident, $what:literal) => {
        impl $holder {
            #[doc = concat!("The name of the ", $what, ".")]
            #[must_use]
            pub fn name(&self) -> &str {
                &self.level().name
            }

            #[doc = concat!("Attaches a QoS profile to the ", $what, " (§7.4.3): a file URL or the XML string, as [`Profile`] reads it. It defines the default DomainParticipant, Publisher and Subscriber of the ", $what, "; its `topic_qos`, `datareader_qos` and `datawriter_qos` are ignored.")]
            ///
            /// # Errors
            /// [`LevelError::QosProfile`] for a profile that cannot be
            /// read, [`LevelError::NonChangeable`] once a port asked for
            /// the default entities.
            pub fn set_qos_profile(&self, qos_profile: &str) -> Result<(), LevelError> {
                self.level().set_qos_profile(qos_profile)
            }

            #[doc = concat!("Gives the ", $what, " its default entities of the domain of `participant` (§7.4.1). They serve a port when they meet what the connector's profile requests. The ", $what, " does not delete them. Without a Publisher or a Subscriber the ", $what, " creates that one in `participant` from its profile.")]
            ///
            /// # Errors
            /// [`LevelError::ForeignEntity`] if `publisher` or
            /// `subscriber` is not of `participant`,
            /// [`LevelError::NonChangeable`] once a port asked for the
            /// default entities.
            pub fn set_default_entities(
                &self,
                participant: DomainParticipant,
                publisher: Option<Arc<Publisher>>,
                subscriber: Option<Arc<Subscriber>>,
            ) -> Result<(), LevelError> {
                self.level()
                    .set_default_entities(participant, publisher, subscriber)
            }

            #[doc = concat!("The threading policy of the ", $what, " (§7.4.4).")]
            #[must_use]
            pub fn threading_policy(&self) -> Option<ThreadingPolicy> {
                lock(&self.level().state).threading
            }

            #[doc = concat!("Sets the threading policy of the ", $what, " (§7.4.4). It holds for the facets activated after the call; `None` removes it.")]
            pub fn set_threading_policy(&self, policy: Option<ThreadingPolicy>) {
                lock(&self.level().state).threading = policy;
            }
        }
    };
}

level_configuration!(Container, "container");
level_configuration!(Component, "component");

impl Container {
    /// A container whose DomainParticipants read the standard runtime
    /// configuration. Its thread pool, if it has a threading policy, is
    /// named `name`.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self::build(name, None)
    }

    /// A container whose DomainParticipants run with `runtime`.
    #[must_use]
    pub fn with_runtime_config(name: &str, runtime: RuntimeConfig) -> Self {
        Self::build(name, Some(runtime))
    }

    fn build(name: &str, runtime: Option<RuntimeConfig>) -> Self {
        Self {
            inner: Arc::new(ContainerInner {
                level: Level::new(name),
                runtime,
                components: Mutex::new(Vec::new()),
            }),
        }
    }

    fn level(&self) -> &Level {
        &self.inner.level
    }

    /// A component running in this container. Its thread pool, if it has
    /// a threading policy, is named `<container>.<name>`.
    #[must_use]
    pub fn component(&self, name: &str) -> Component {
        let level = Arc::new(Level::new(name));
        lock(&self.inner.components).push(Arc::clone(&level));
        Component {
            container: Arc::clone(&self.inner),
            level,
        }
    }

    /// Removes the container: deletes the default entities it and its
    /// components created. The connectors of its components are removed
    /// before, the entities it was given stay.
    ///
    /// # Errors
    /// `PRECONDITION_NOT_MET`, with nothing deleted and the defaults as
    /// they were, while a port still has its DataReader or DataWriter in
    /// a default Subscriber or Publisher the container or a component
    /// created, or in a Subscriber or Publisher dedicated to it in a
    /// default DomainParticipant.
    ///
    /// Otherwise `InternalError` with the code of the first DDS error. A
    /// default DomainParticipant that still has a topic is such an error:
    /// it stays the default of its domain, and a later call deletes it.
    pub fn remove(&self) -> Result<(), InternalError> {
        let components = lock(&self.inner.components).clone();
        let levels = || {
            let of_components = components.iter().map(|level| &**level);
            of_components.chain(std::iter::once(&self.inner.level))
        };
        let mut survey = Survey::default();
        for level in levels() {
            level.survey(&mut survey);
        }
        if survey.serves_a_port() {
            return Err(InternalError::code(return_code::PRECONDITION_NOT_MET));
        }
        // The components first: a default of a component can live in the
        // DomainParticipant of the container.
        let mut first = Ok(());
        for level in levels() {
            let removed = level.remove();
            if first.is_ok() {
                first = removed;
            }
        }
        Ok(first?)
    }
}

impl Component {
    fn level(&self) -> &Level {
        &self.level
    }

    /// The Publisher or the Subscriber, and its DomainParticipant, for a
    /// port of a connector in `domain` with the profile `profile` and the
    /// topic `topic_name`: the container's defaults if they have what the
    /// profile sets, else the component's, else a Publisher or Subscriber
    /// dedicated to the port in the default DomainParticipant that suits,
    /// else in a DomainParticipant dedicated to the port.
    pub(crate) fn place<G: Group>(
        &self,
        domain: DomainId,
        profile: &Profile,
        topic_name: &str,
    ) -> Result<Placement<G>, DdsError> {
        let wanted_participant = profile.participant(Some(topic_name));
        let wanted_group = G::requested(profile, Some(topic_name));
        let suits = |participant: &DomainParticipant| {
            participant_meets(&participant.qos(), wanted_participant)
        };
        let shared = |participant, group| Placement {
            participant,
            dedicated_participant: false,
            group,
            dedicated_group: false,
        };

        let container = &self.container.level;
        let Some(of_container) = container.participant(domain, true, &self.container)? else {
            return Err(DdsError::PreconditionNotMet {
                reason: "the container has no default DomainParticipant",
            });
        };
        let container_suits = suits(&of_container);
        if container_suits
            && let Some(group) = container.group::<G>(&of_container)?
            && group.meets(wanted_group)
        {
            return Ok(shared(of_container, group));
        }

        // The component lives in its own default DomainParticipant, or
        // else in the container's.
        let own = self.level.participant(domain, false, &self.container)?;
        let home = own.clone().unwrap_or_else(|| of_container.clone());
        let home_suits = suits(&home);
        if home_suits
            && let Some(group) = self.level.group::<G>(&home)?
            && group.meets(wanted_group)
        {
            return Ok(shared(home, group));
        }

        let default_participant = if container_suits {
            Some(of_container)
        } else {
            own.filter(|_| home_suits)
        };
        let (participant, dedicated_participant) = match default_participant {
            Some(participant) => (participant, false),
            None => {
                let qos = participant_qos(wanted_participant);
                (self.container.create_participant(domain, qos)?, true)
            }
        };
        match G::create(&participant, wanted_group) {
            Ok(group) => Ok(Placement {
                participant,
                dedicated_participant,
                group: Arc::new(group),
                dedicated_group: true,
            }),
            Err(error) => {
                if dedicated_participant {
                    // The error of the Publisher or Subscriber is the one
                    // to report.
                    let _ = delete_participant(&participant);
                }
                Err(error)
            }
        }
    }

    /// The DomainParticipant for a port of a connector in `domain` that
    /// creates its Publisher and Subscriber itself, a DLRL port (§8.4.1),
    /// whose connector's profile requests `wanted` of a DomainParticipant:
    /// the container's default if it has what `wanted` sets, else the
    /// component's, else one dedicated to the port. The second value says
    /// whether it is dedicated to the port.
    pub(crate) fn place_participant(
        &self,
        domain: DomainId,
        wanted: Option<&EntityQos>,
    ) -> Result<(DomainParticipant, bool), DdsError> {
        let suits = |participant: &DomainParticipant| participant_meets(&participant.qos(), wanted);
        let container = &self.container.level;
        let Some(of_container) = container.participant(domain, true, &self.container)? else {
            return Err(DdsError::PreconditionNotMet {
                reason: "the container has no default DomainParticipant",
            });
        };
        if suits(&of_container) {
            return Ok((of_container, false));
        }
        if let Some(own) = self.level.participant(domain, false, &self.container)?
            && suits(&own)
        {
            return Ok((own, false));
        }
        let qos = participant_qos(wanted);
        Ok((self.container.create_participant(domain, qos)?, true))
    }

    /// §7.4.4 for a port without a policy: "the component's threadpool is
    /// used", "if there is no component's one, the container's threadpool
    /// is used"; `None` is the default.
    pub(crate) fn thread_pool(&self) -> Result<Option<Arc<ThreadPool>>, InternalError> {
        let container = &self.container.level;
        let name = format!("{}.{}", container.name, self.level.name);
        let pool = match self.level.pool(&name) {
            Ok(Some(pool)) => Ok(Some(pool)),
            Ok(None) => container.pool(&container.name),
            Err(error) => Err(error),
        };
        pool.map_err(|_| InternalError::code(return_code::OUT_OF_RESOURCES))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::qos::ReliabilityKind;

    const DOCUMENT: &str = r#"<dds>
      <qos_profile name="Base">
        <datawriter_qos><reliability><kind>BEST_EFFORT_RELIABILITY_QOS</kind></reliability></datawriter_qos>
        <publisher_qos><partition><name><element>base</element></name></partition></publisher_qos>
      </qos_profile>
      <qos_profile name="Derived" base_name="Base">
        <datawriter_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>
        <datawriter_qos topic_filter="Alarm*">
          <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>7</depth></history>
        </datawriter_qos>
      </qos_profile>
    </dds>"#;

    #[test]
    fn dds4ccm_7_3_1_an_xml_string_is_read_and_its_last_profile_is_the_profile() {
        let profile = Profile::read(DOCUMENT).unwrap();
        assert_eq!(profile.name(), Some("(dds)::Derived"));
        // Own value, and the value of the base (§7.4.2.3.1).
        let writer = profile.datawriter_qos("Track");
        assert_eq!(writer.history.kind, HistoryKind::KeepAll);
        assert_eq!(writer.reliability.kind, ReliabilityKind::BestEffort);
        let partition = profile.publisher(Some("Track")).unwrap().partition.clone();
        assert_eq!(partition.unwrap().names, ["base"]);
        // What the profile does not name is the default of DDS.
        assert_eq!(profile.datareader_qos("Track"), DataReaderQos::default());
        assert_eq!(profile.topic_qos("Track"), TopicQos::default());
        assert!(profile.participant(Some("Track")).is_none());
    }

    #[test]
    fn dds4ccm_7_4_3_the_topic_filter_allocates_the_values_to_the_entities_of_a_topic() {
        let profile = Profile::read(DOCUMENT).unwrap();
        let alarm = profile.datawriter_qos("AlarmHigh");
        assert_eq!(
            (alarm.history.kind, alarm.history.depth),
            (HistoryKind::KeepLast, 7)
        );
        assert_eq!(
            profile.datawriter_qos("Track").history.kind,
            HistoryKind::KeepAll
        );
    }

    #[test]
    fn dds4ccm_7_3_1_the_empty_value_is_no_profile_and_every_qos_is_the_default() {
        for empty in ["", "  \n"] {
            let profile = Profile::read(empty).unwrap();
            assert_eq!(profile, Profile::default());
            assert_eq!(profile.name(), None);
            assert_eq!(profile.datawriter_qos("T"), DataWriterQos::default());
            assert_eq!(profile.datareader_qos("T"), DataReaderQos::default());
            assert_eq!(profile.topic_qos("T"), TopicQos::default());
            assert!(profile.publisher(Some("T")).is_none());
            assert!(profile.subscriber(None).is_none());
        }
    }

    #[test]
    fn dds4ccm_7_3_1_a_container_without_policies_is_the_default_qos() {
        // A container that is named and sets nothing requests nothing new.
        let profile = Profile::read(
            "<dds><qos_profile name=\"P\"><datawriter_qos/><datareader_qos/><topic_qos/>\
             <publisher_qos/><subscriber_qos/></qos_profile></dds>",
        )
        .unwrap();
        assert_eq!(profile.datawriter_qos("T"), DataWriterQos::default());
        assert_eq!(profile.datareader_qos("T"), DataReaderQos::default());
        assert_eq!(profile.topic_qos("T"), TopicQos::default());
        assert_eq!(
            publisher_qos(profile.publisher(None)),
            PublisherQos::default()
        );
        assert_eq!(
            subscriber_qos(profile.subscriber(None)),
            SubscriberQos::default()
        );
        assert!(publisher_meets(
            &PublisherQos::default(),
            profile.publisher(None)
        ));
    }

    #[test]
    fn dds4ccm_7_3_1_a_file_url_is_parsed_into_path_and_profile() {
        let url = |value| file_url(value).unwrap();
        assert_eq!(url("file:///etc/qos.xml"), ("/etc/qos.xml".into(), None));
        assert_eq!(url("file:/etc/qos.xml"), ("/etc/qos.xml".into(), None));
        assert_eq!(
            url("FILE://localhost/etc/my%20qos.xml#Lib::P%31"),
            ("/etc/my qos.xml".into(), Some("Lib::P1".into()))
        );
        // RFC 8089 Appendix E.2: a drive letter after the slash. On
        // Windows the path starts at the letter.
        assert_eq!(drive_path("/C:/dir/qos.xml"), Some("C:/dir/qos.xml"));
        assert_eq!(drive_path("/c:"), Some("c:"));
        for no_drive in [
            "/etc/qos.xml",
            "/C:qos.xml",
            "/CD:/qos.xml",
            "/1:/qos.xml",
            "C:/x",
        ] {
            assert_eq!(drive_path(no_drive), None, "{no_drive}");
        }
        let with_drive = if cfg!(windows) {
            "C:/dir/my qos.xml"
        } else {
            "/C:/dir/my qos.xml"
        };
        for form in [
            "file:///C:/dir/my%20qos.xml#P",
            "file:/C:/dir/my%20qos.xml#P",
            "file://localhost/C%3A/dir/my%20qos.xml#P",
        ] {
            assert_eq!(url(form), (with_drive.into(), Some("P".into())), "{form}");
        }
        for bad in [
            "/etc/qos.xml",
            "qos.xml",
            "http://example.org/qos.xml",
            "file://elsewhere/etc/qos.xml",
            "file:qos.xml",
            "file:///etc/%zz.xml",
        ] {
            let error = Profile::read(bad).unwrap_err();
            assert!(matches!(error, ProfileError::Url(_)), "{bad}: {error:?}");
        }
    }

    /// Without a name in the URL: the last profile directly under `<dds>`;
    /// in a document without such a profile the last one of the last
    /// `<qos_library>`.
    #[test]
    fn dds4ccm_7_3_1_the_profile_of_a_document_with_libraries() {
        let named =
            |name: &str| format!("<qos_profile name=\"{name}\"><datawriter_qos/></qos_profile>");
        let library = |name: &str, profiles: &[&str]| {
            let profiles: String = profiles.iter().map(|profile| named(profile)).collect();
            format!("<qos_library name=\"{name}\">{profiles}</qos_library>")
        };
        let used = |document: String| {
            let profile = Profile::read(&format!("<dds>{document}</dds>")).unwrap();
            profile.name().map(str::to_owned)
        };
        // Libraries only: the last profile of the last library.
        let libraries = library("A", &["A1", "A2"]) + &library("B", &["B1", "B2"]);
        assert_eq!(used(libraries), Some("B::B2".into()));
        // Profiles directly under `<dds>` and a library after them: the
        // last one directly under `<dds>`, not the last of the document.
        let library_last = named("Top1") + &named("Top2") + &library("L", &["L1"]);
        assert_eq!(used(library_last), Some("(dds)::Top2".into()));
        // The same with the library first, and with profiles on both sides
        // of it.
        let library_first = library("L", &["L1"]) + &named("Top1");
        assert_eq!(used(library_first), Some("(dds)::Top1".into()));
        let around = named("Top1") + &library("L", &["L1"]) + &named("Top2");
        assert_eq!(used(around), Some("(dds)::Top2".into()));
    }

    #[test]
    fn dds4ccm_7_3_1_what_is_no_profile_document_is_an_error() {
        assert!(matches!(
            Profile::read("<qos/>").unwrap_err(),
            ProfileError::Xml(_)
        ));
        assert_eq!(
            Profile::read("<dds/>").unwrap_err(),
            ProfileError::NoProfile
        );
        assert!(matches!(
            Profile::read("file:///nonexistent/dds4ccm/qos.xml").unwrap_err(),
            ProfileError::File { .. }
        ));
    }

    /// A topic that exists is held to the policies the profile sets for
    /// it, and to no other.
    #[test]
    fn dds4ccm_7_3_1_a_topic_differs_in_the_policies_the_profile_sets_to_another_value() {
        let profile = Profile::read(
            "<dds><qos_profile name=\"P\">\
             <topic_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>\
             <history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>3</depth></history></topic_qos>\
             <topic_qos topic_filter=\"Alarm*\"><lifespan><duration><sec>5</sec>\
             <nanosec>0</nanosec></duration></lifespan></topic_qos>\
             </qos_profile></dds>",
        )
        .unwrap();
        // The topic the profile describes has no difference.
        let described = profile.topic_qos("Track");
        assert!(
            profile
                .topic_qos_differences("Track", &described)
                .is_empty()
        );
        // The default of DDS differs where the profile sets another value:
        // the reliability of a topic is BEST_EFFORT, its history depth 1.
        assert_eq!(
            profile.topic_qos_differences("Track", &TopicQos::default()),
            ["reliability", "history"]
        );
        // A policy the profile does not set is not compared.
        let mut elsewhere = described.clone();
        elsewhere.topic_data.value = vec![1];
        elsewhere.durability.kind = zerodds_dcps::qos::DurabilityKind::TransientLocal;
        assert!(
            profile
                .topic_qos_differences("Track", &elsewhere)
                .is_empty()
        );
        // `topic_filter` selects what is requested of the topic.
        assert_eq!(
            profile.topic_qos_differences("AlarmHigh", &TopicQos::default()),
            ["lifespan"]
        );
        // No profile requests nothing.
        assert!(
            Profile::default()
                .topic_qos_differences("Track", &elsewhere)
                .is_empty()
        );
    }

    #[test]
    fn dds4ccm_7_4_3_a_default_entity_is_compatible_when_it_has_what_the_profile_sets() {
        let profile = Profile::read(DOCUMENT).unwrap();
        let requested = profile.publisher(Some("Track"));
        // The default Publisher has the empty partition, the profile asks
        // for `base`.
        assert!(!publisher_meets(&PublisherQos::default(), requested));
        assert!(publisher_meets(&publisher_qos(requested), requested));
        // A profile that sets nothing for a kind requests nothing of it.
        assert!(subscriber_meets(
            &SubscriberQos::default(),
            profile.subscriber(Some("Track"))
        ));
        assert!(participant_meets(
            &DomainParticipantQos::default(),
            profile.participant(Some("Track"))
        ));
    }
}
