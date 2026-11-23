// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The DDS-DCPS connectors (DDS4CCM 1.1 §7.3): `DDS_Base`,
//! `DDS_TopicBase`, `DDS_State` (pattern State Transfer) and `DDS_Event`
//! (pattern Event Transfer).
//!
//! A connector gathers "the connector fragments for all possible roles in
//! a given DDS use pattern". Its attributes say what the fragments need:
//! the domain, the QoS profile, the name of the topic and its key. "Any
//! attempt to change those attributes once the configuration is complete
//! will raise a NonChangeable exception."
//!
//! # Life of a connector
//!
//! 1. The attributes are set, `error_listener` is connected.
//! 2. `configuration_complete` reads the QoS profile, checks the
//!    attributes and fixes them.
//! 3. A component connects a port to a mirror port of the connector: the
//!    method of the connector that has the name of the mirror port
//!    (`observable`, `passive_observer`, `supplier`, …) takes the
//!    [`Component`] and makes the fragment of that role for it. The
//!    fragment creates "under the scene the DDS entities that are needed"
//!    (§7.4.1): the topic with the connector's `TopicListener`, the
//!    DataWriter or DataReader dedicated to the port, in the Publisher or
//!    Subscriber and the DomainParticipant [`crate::entities`] allocates.
//!    A publishing role returns its port; a subscribing role is given the
//!    port of the component, whose attribute `filter` and threading policy
//!    are set, and completes its configuration.
//! 4. `remove` deletes what the fragments created.
//!
//! # Attribute `key_fields`
//!
//! "The list of fields making up the key for that Topic." The key of a
//! topic of ZeroDDS is that of its data type. The attribute is checked
//! against the type when the configuration is completed: with the names
//! of the key members where the type carries its complete `TypeObject`
//! and has no base type, otherwise by whether the type has a key at all.
//! A list that contradicts the type fails the configuration.

use std::any::{Any, TypeId};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use zerodds_dcps::error::return_code;
use zerodds_dcps::listener::ArcTopicListener;
use zerodds_dcps::qos::HistoryKind;
use zerodds_dcps::{
    DataWriter, DdsError, DdsType, DomainId, DomainParticipant, InstanceHandle, Publisher,
    Subscriber, Topic,
};
use zerodds_dlrl::metamodel::DlrlException;
use zerodds_types::TypeIdentifier;
use zerodds_types::type_object::flags::StructMemberFlag;
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

use crate::ccm_dds::{ConnectorStatusListener, InternalError, NonChangeable};
use crate::entities::{Component, Group, Profile, ProfileError, delete_participant};
use crate::ports::{DdsGet, DdsListen, DdsRead, DdsStateListen, DdsUpdate, DdsWrite, Subscribing};
use crate::reader_core::lock;
use crate::receptacle::Receptacle;
use crate::status::{ErrorListener, TopicStatusListener};

/// Why a connector refused its configuration or a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    /// A port was connected before `configuration_complete`.
    NotConfigured,
    /// `configuration_complete` was called on a configured connector.
    AlreadyConfigured,
    /// Attribute `qos_profile` could not be read.
    QosProfile(ProfileError),
    /// Attribute `topic_name` is empty.
    TopicName,
    /// Attribute `key_fields` contradicts the key of the data type.
    KeyFields {
        /// The attribute.
        key_fields: Vec<String>,
        /// The key members of the data type; `None` where the type does
        /// not name them.
        key_of_type: Option<Vec<String>>,
    },
    /// The topic exists in the DomainParticipant of the port, and a policy
    /// the `topic_qos` of the connector's profile sets has another value
    /// there.
    TopicQos {
        /// Attribute `topic_name`.
        topic_name: String,
        /// The policies that differ, by their XML names.
        policies: Vec<&'static str>,
    },
    /// DDS refused an entity of a fragment.
    Dds(InternalError),
    /// A DLRL connector has no mirror port of that name (§8.3).
    UnknownMirrorPort(String),
    /// The DLRL entities of a DLRL port refused the fragment (§8.2.2): a
    /// home, a Cache, the registration of the DCPS entities or their
    /// activation.
    Dlrl(DlrlException),
    /// An entity of a DLRL port that serves all its topics (the
    /// DomainParticipant, the Publisher, the Subscriber) would take the
    /// container `topic_filter` selects for each topic, and the profile
    /// selects different containers of that kind for two of the topics
    /// (§8.4.2).
    TopicFilter {
        /// The kind of container: `domainparticipant_qos`,
        /// `publisher_qos` or `subscriber_qos`.
        container: &'static str,
        /// Two topics of the port whose selected containers differ, in
        /// the order of the port's topics.
        topic_names: [String; 2],
    },
}

impl std::fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => {
                f.write_str("the configuration of the connector is not complete")
            }
            Self::AlreadyConfigured => {
                f.write_str("the configuration of the connector is complete")
            }
            Self::QosProfile(error) => error.fmt(f),
            Self::TopicName => f.write_str("topic_name is empty"),
            Self::KeyFields {
                key_fields,
                key_of_type: Some(key_of_type),
            } => write!(
                f,
                "key_fields {key_fields:?} are not the key members {key_of_type:?} of the data type"
            ),
            Self::KeyFields {
                key_fields,
                key_of_type: None,
            } if key_fields.is_empty() => {
                f.write_str("key_fields is empty and the data type has a key")
            }
            Self::KeyFields { key_fields, .. } => {
                write!(f, "key_fields {key_fields:?} and the data type has no key")
            }
            Self::TopicQos {
                topic_name,
                policies,
            } => write!(
                f,
                "the topic `{topic_name}` exists with other values than the topic_qos of \
                 qos_profile sets: {}",
                policies.join(", ")
            ),
            Self::Dds(error) => error.fmt(f),
            Self::UnknownMirrorPort(name) => {
                write!(f, "the connector has no mirror port `{name}`")
            }
            Self::Dlrl(error) => write!(f, "DLRL: {error}"),
            Self::TopicFilter {
                container,
                topic_names: [first, second],
            } => write!(
                f,
                "qos_profile selects one {container} for the topic `{first}` and another for \
                 `{second}`, and one entity of the port serves both"
            ),
        }
    }
}

impl std::error::Error for ConnectorError {}

impl From<DdsError> for ConnectorError {
    fn from(error: DdsError) -> Self {
        Self::Dds(error.into())
    }
}

impl From<InternalError> for ConnectorError {
    fn from(error: InternalError) -> Self {
        Self::Dds(error)
    }
}

impl From<DlrlException> for ConnectorError {
    fn from(error: DlrlException) -> Self {
        Self::Dlrl(error)
    }
}

struct BaseAttributes {
    domain_id: DomainId,
    qos_profile: String,
}

/// Connector `DDS_Base` (§7.3.1): `uses ConnectorStatusListener
/// error_listener`, attributes `domain_id` and `qos_profile`. "All DDS
/// connectors should inherit from that base."
pub struct DdsBase {
    error_listener: Arc<ErrorListener>,
    attributes: Mutex<BaseAttributes>,
    /// The profile `qos_profile` names, read when the configuration was
    /// completed.
    profile: OnceLock<Profile>,
}

impl Default for DdsBase {
    fn default() -> Self {
        Self::new()
    }
}

impl DdsBase {
    /// A connector with `domain_id` 0 and no QoS profile.
    #[must_use]
    pub fn new() -> Self {
        Self {
            error_listener: Arc::new(Receptacle::new()),
            attributes: Mutex::new(BaseAttributes {
                domain_id: 0,
                qos_profile: String::new(),
            }),
            profile: OnceLock::new(),
        }
    }

    /// Connects `uses ConnectorStatusListener error_listener` and returns
    /// the listener it replaces.
    pub fn connect_error_listener(
        &self,
        listener: Arc<dyn ConnectorStatusListener>,
    ) -> Option<Arc<dyn ConnectorStatusListener>> {
        self.error_listener.connect(listener)
    }

    /// Disconnects `error_listener`.
    pub fn disconnect_error_listener(&self) -> Option<Arc<dyn ConnectorStatusListener>> {
        self.error_listener.disconnect()
    }

    /// The receptacle `error_listener`, as the fragments share it.
    #[must_use]
    pub fn error_listener(&self) -> Arc<ErrorListener> {
        Arc::clone(&self.error_listener)
    }

    /// Attribute `domain_id`.
    #[must_use]
    pub fn domain_id(&self) -> DomainId {
        lock(&self.attributes).domain_id
    }

    /// Sets attribute `domain_id`.
    ///
    /// # Errors
    /// `NonChangeable` once the configuration is complete.
    pub fn set_domain_id(&self, domain_id: DomainId) -> Result<(), NonChangeable> {
        let mut attributes = lock(&self.attributes);
        self.changeable()?;
        attributes.domain_id = domain_id;
        Ok(())
    }

    /// Attribute `qos_profile`: "File URL or XML string".
    #[must_use]
    pub fn qos_profile(&self) -> String {
        lock(&self.attributes).qos_profile.clone()
    }

    /// Sets attribute `qos_profile`: a file URL or the XML string, as
    /// [`Profile`] reads it. The value is read when the configuration is
    /// completed.
    ///
    /// # Errors
    /// `NonChangeable` once the configuration is complete.
    pub fn set_qos_profile(&self, qos_profile: impl Into<String>) -> Result<(), NonChangeable> {
        let mut attributes = lock(&self.attributes);
        self.changeable()?;
        attributes.qos_profile = qos_profile.into();
        Ok(())
    }

    /// Whether the configuration is complete.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.profile.get().is_some()
    }

    fn changeable(&self) -> Result<(), NonChangeable> {
        if self.is_configured() {
            Err(NonChangeable)
        } else {
            Ok(())
        }
    }

    /// Completes the configuration: reads the profile of `qos_profile`.
    /// From then on setting an attribute raises `NonChangeable`.
    ///
    /// # Errors
    /// [`ConnectorError::QosProfile`] for a profile that cannot be read;
    /// the attributes stay changeable. [`ConnectorError::AlreadyConfigured`]
    /// for the second call.
    pub fn configuration_complete(&self) -> Result<(), ConnectorError> {
        // Held to the end: a setter waits and then finds the connector
        // configured.
        let attributes = lock(&self.attributes);
        if self.is_configured() {
            return Err(ConnectorError::AlreadyConfigured);
        }
        let profile = Profile::read(&attributes.qos_profile).map_err(ConnectorError::QosProfile)?;
        // Not reached twice: the lock is held since the check.
        let _ = self.profile.set(profile);
        Ok(())
    }

    /// The profile of the configured connector.
    pub(crate) fn profile(&self) -> Result<&Profile, ConnectorError> {
        self.profile.get().ok_or(ConnectorError::NotConfigured)
    }
}

struct TopicAttributes {
    topic_name: String,
    key_fields: Vec<String>,
}

/// Connector `DDS_TopicBase` (§7.3.1): `DDS_Base` with the attributes
/// `topic_name` and `key_fields`, "the base for all mono-topic
/// connectors".
pub struct DdsTopicBase {
    base: DdsBase,
    attributes: Mutex<TopicAttributes>,
    /// `topic_name` and `key_fields` were checked for the data type: the
    /// configuration was completed here and not on the base alone.
    checked: AtomicBool,
}

impl Deref for DdsTopicBase {
    type Target = DdsBase;

    fn deref(&self) -> &DdsBase {
        &self.base
    }
}

impl Default for DdsTopicBase {
    fn default() -> Self {
        Self::new()
    }
}

/// The key members of `T`, where its complete `TypeObject` names them: a
/// structure without a base type.
fn key_members_of<T: DdsType>() -> Option<Vec<String>> {
    let Some(TypeObject::Complete(CompleteTypeObject::Struct(structure))) = T::type_object() else {
        return None;
    };
    if structure.header.base_type != TypeIdentifier::None {
        return None;
    }
    let keys = structure
        .member_seq
        .iter()
        .filter(|member| member.common.member_flags.has(StructMemberFlag::IS_KEY))
        .map(|member| member.detail.name.clone())
        .collect();
    Some(keys)
}

impl DdsTopicBase {
    /// A connector with `domain_id` 0, no QoS profile, an empty
    /// `topic_name` and no `key_fields`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: DdsBase::new(),
            attributes: Mutex::new(TopicAttributes {
                topic_name: String::new(),
                key_fields: Vec::new(),
            }),
            checked: AtomicBool::new(false),
        }
    }

    /// Attribute `topic_name`.
    #[must_use]
    pub fn topic_name(&self) -> String {
        lock(&self.attributes).topic_name.clone()
    }

    /// Sets attribute `topic_name`.
    ///
    /// # Errors
    /// `NonChangeable` once the configuration is complete.
    pub fn set_topic_name(&self, topic_name: impl Into<String>) -> Result<(), NonChangeable> {
        let mut attributes = lock(&self.attributes);
        self.base.changeable()?;
        attributes.topic_name = topic_name.into();
        Ok(())
    }

    /// Attribute `key_fields`.
    #[must_use]
    pub fn key_fields(&self) -> Vec<String> {
        lock(&self.attributes).key_fields.clone()
    }

    /// Sets attribute `key_fields`: the names of the members that make up
    /// the key of the topic; see the module documentation.
    ///
    /// # Errors
    /// `NonChangeable` once the configuration is complete.
    pub fn set_key_fields(&self, key_fields: Vec<String>) -> Result<(), NonChangeable> {
        let mut attributes = lock(&self.attributes);
        self.base.changeable()?;
        attributes.key_fields = key_fields;
        Ok(())
    }

    /// Completes the configuration of a connector for the data type `T`:
    /// checks `topic_name` and `key_fields` and reads the QoS profile.
    /// From then on setting an attribute raises `NonChangeable`.
    ///
    /// # Errors
    /// [`ConnectorError::TopicName`], [`ConnectorError::KeyFields`],
    /// [`ConnectorError::QosProfile`]: the attributes stay changeable.
    /// [`ConnectorError::AlreadyConfigured`] for the second call.
    pub fn configuration_complete<T: DdsType>(&self) -> Result<(), ConnectorError> {
        // Held to the end: a setter of this connector waits and then
        // finds it configured.
        let attributes = lock(&self.attributes);
        if self.base.is_configured() {
            return Err(ConnectorError::AlreadyConfigured);
        }
        if attributes.topic_name.is_empty() {
            return Err(ConnectorError::TopicName);
        }
        let key_of_type = key_members_of::<T>();
        let agrees = match &key_of_type {
            Some(key_of_type) => {
                let mut named = attributes.key_fields.clone();
                let mut of_type = key_of_type.clone();
                named.sort();
                of_type.sort();
                named == of_type
            }
            None => attributes.key_fields.is_empty() != T::HAS_KEY,
        };
        if !agrees {
            return Err(ConnectorError::KeyFields {
                key_fields: attributes.key_fields.clone(),
                key_of_type,
            });
        }
        self.base.configuration_complete()?;
        self.checked.store(true, Ordering::Release);
        Ok(())
    }

    /// The profile and the topic name of the connector whose configuration
    /// was completed for its data type. A connector completed on its
    /// `DDS_Base` alone has unchecked topic attributes and makes no
    /// fragment.
    fn configured(&self) -> Result<(&Profile, String), ConnectorError> {
        if !self.checked.load(Ordering::Acquire) {
            return Err(ConnectorError::NotConfigured);
        }
        Ok((self.base.profile()?, self.topic_name()))
    }
}

/// Where a fragment is placed: its Publisher or Subscriber, the
/// DomainParticipant of that and the topic of the connector in it, with
/// the profile and the topic name of the connector.
struct Placed<'a, G, T: DdsType> {
    group: Arc<G>,
    participant: DomainParticipant,
    /// `participant` is dedicated to the port.
    dedicated_participant: bool,
    topic: Topic<T>,
    profile: &'a Profile,
    topic_name: String,
}

/// A topic the connectors of this process created, with its listener.
struct SharedTopic {
    listener: Arc<TopicStatusListener>,
    /// The `Topic<T>`.
    topic: Box<dyn Any + Send + Sync>,
}

/// The DomainParticipant and the data type of a topic, its name, and the
/// topic.
type SharedTopicEntry = ((InstanceHandle, TypeId), String, Weak<SharedTopic>);

/// The topics the connectors of this process created: connectors that use
/// a topic of one name and data type in one DomainParticipant share the
/// topic and its listener, and the last one that is removed deletes it.
static SHARED_TOPICS: Mutex<Vec<SharedTopicEntry>> = Mutex::new(Vec::new());

/// Shares the `TopicListener` of `topic`, a topic of `participant` that
/// the connector did not create (the Cache of a DLRL port creates its
/// topics, §8.4.1): `error_listener` is called on INCONSISTENT_TOPIC as
/// the listeners of the other connectors of the topic in that
/// DomainParticipant are. The first share sets the listener on the
/// topic. The closure gives the share back; the last one takes the
/// listener off the topic. The topic is not deleted here.
pub(crate) fn share_topic_listener<T: DdsType + Send + Sync + 'static>(
    participant: &DomainParticipant,
    topic: &Topic<T>,
    error_listener: &Arc<ErrorListener>,
) -> impl FnOnce() + Send + 'static {
    let key = (participant.instance_handle(), TypeId::of::<T>());
    let name = topic.name().to_owned();
    let mut shared_topics = lock(&SHARED_TOPICS);
    shared_topics.retain(|(.., shared)| shared.strong_count() > 0);
    let existing = shared_topics
        .iter()
        .find(|(owner, of_name, _)| *owner == key && *of_name == name)
        .and_then(|(.., shared)| shared.upgrade());
    let shared = if let Some(shared) = existing {
        shared.listener.add(Arc::clone(error_listener));
        shared
    } else {
        let listener = Arc::new(TopicStatusListener::new(Arc::clone(error_listener)));
        let mask = listener.mask();
        topic.set_listener(Some(Arc::clone(&listener) as ArcTopicListener), mask);
        let shared = Arc::new(SharedTopic {
            listener,
            topic: Box::new(topic.clone()),
        });
        shared_topics.push((key, name, Arc::downgrade(&shared)));
        shared
    };
    drop(shared_topics);
    let (error_listener, topic) = (Arc::clone(error_listener), topic.clone());
    move || {
        let mut shared_topics = lock(&SHARED_TOPICS);
        if shared.listener.remove(&error_listener) {
            shared_topics.retain(|(.., of_topic)| !of_topic.ptr_eq(&Arc::downgrade(&shared)));
            topic.set_listener(None, 0);
        }
    }
}

/// How to delete an entity a fragment created. The argument says whether
/// a removal called before it in the same pass failed, so that what it
/// would have deleted may still use the entity. One that fails is called
/// again by the next removal of the connector.
type Removal = Box<dyn FnMut(bool) -> Result<(), InternalError> + Send>;

/// The fragments of a mono-topic connector for the data type `T`: what
/// `DDS_State` and `DDS_Event` share.
struct Fragments<T: DdsType> {
    base: DdsTopicBase,
    /// The HISTORY kind of the pattern, for the DataWriters and
    /// DataReaders whose profile does not set HISTORY.
    history: HistoryKind,
    /// The topic of the connector in each DomainParticipant a fragment
    /// lives in.
    topics: Mutex<Vec<(DomainParticipant, Topic<T>)>>,
    /// The entities the fragments created, in the order of creation.
    created: Mutex<Vec<Removal>>,
    /// Held while a fragment is created and while the fragments are
    /// removed: a fragment that fails takes back its entities and its
    /// entries in `topics` before another fragment can use them.
    changing: Mutex<()>,
}

/// What one fragment created so far: kept apart from
/// [`Fragments::created`] until the fragment is complete, and deleted when
/// it fails.
#[derive(Default)]
struct Created {
    removals: Vec<Removal>,
    /// The DomainParticipants whose entry in [`Fragments::topics`] the
    /// fragment added.
    topics: Vec<InstanceHandle>,
}

impl Created {
    fn push(&mut self, removal: impl FnMut(bool) -> Result<(), InternalError> + Send + 'static) {
        self.removals.push(Box::new(removal));
    }
}

impl<T: DdsType + Send + Sync + 'static> Fragments<T> {
    fn new(history: HistoryKind) -> Self {
        Self {
            base: DdsTopicBase::new(),
            history,
            topics: Mutex::new(Vec::new()),
            created: Mutex::new(Vec::new()),
            changing: Mutex::new(()),
        }
    }

    /// Creates one fragment with `build`. What `build` created stays with
    /// the connector when it succeeds; when it fails, it is deleted, the
    /// last first, and the error of `build` is returned.
    fn fragment<R>(
        &self,
        build: impl FnOnce(&mut Created) -> Result<R, ConnectorError>,
    ) -> Result<R, ConnectorError> {
        let _changing = lock(&self.changing);
        let mut created = Created::default();
        match build(&mut created) {
            Ok(fragment) => {
                lock(&self.created).extend(created.removals);
                Ok(fragment)
            }
            Err(error) => {
                lock(&self.topics).retain(|(participant, _)| {
                    !created.topics.contains(&participant.instance_handle())
                });
                let mut failed = false;
                for mut removal in created.removals.into_iter().rev() {
                    // The error of the fragment is the one to report; the
                    // removals that follow are still tried.
                    failed |= removal(failed).is_err();
                }
                Err(error)
            }
        }
    }

    /// The Publisher or Subscriber of a fragment for `component` and the
    /// topic of the connector in its DomainParticipant.
    fn place<G: Group + Send + Sync + 'static>(
        &self,
        component: &Component,
        created: &mut Created,
    ) -> Result<Placed<'_, G, T>, ConnectorError> {
        let (profile, topic_name) = self.base.configured()?;
        let placement = component.place::<G>(self.base.domain_id(), profile, &topic_name)?;
        if placement.dedicated_participant {
            let dedicated = placement.participant.clone();
            created.push(move |_| Ok(delete_participant(&dedicated)?));
        }
        if placement.dedicated_group {
            let dedicated = Arc::clone(&placement.group);
            created.push(move |_| Ok(dedicated.delete()?));
        }
        let topic = self.topic_in(&placement.participant, profile, &topic_name, created)?;
        Ok(Placed {
            group: placement.group,
            participant: placement.participant,
            dedicated_participant: placement.dedicated_participant,
            topic,
            profile,
            topic_name,
        })
    }

    /// The topic of the connector in `participant`, created at the first
    /// fragment there with the `topic_qos` of the profile and the
    /// `TopicListener` that reports to `error_listener`. A topic another
    /// connector of the data type has there under the name is the same
    /// topic; its listener reports to both connectors. So is a topic the
    /// application created there.
    ///
    /// A topic that exists keeps its QoS. It serves the connector when
    /// every policy the `topic_qos` of the profile sets has the same value
    /// there, as a default entity does (§7.4.3); otherwise the fragment is
    /// refused with [`ConnectorError::TopicQos`].
    fn topic_in(
        &self,
        participant: &DomainParticipant,
        profile: &Profile,
        topic_name: &str,
        created: &mut Created,
    ) -> Result<Topic<T>, ConnectorError> {
        let handle = participant.instance_handle();
        let mut topics = lock(&self.topics);
        if let Some((_, topic)) = topics.iter().find(|(p, _)| p.instance_handle() == handle) {
            return Ok(topic.clone());
        }
        let error_listener = self.base.error_listener();
        let suits = |topic: &Topic<T>| {
            let policies = profile.topic_qos_differences(topic_name, &topic.qos());
            if policies.is_empty() {
                Ok(())
            } else {
                Err(ConnectorError::TopicQos {
                    topic_name: topic_name.to_owned(),
                    policies,
                })
            }
        };
        // Under the lock of the shared topics: no other connector of the
        // topic comes or goes in between.
        let mut shared_topics = lock(&SHARED_TOPICS);
        shared_topics.retain(|(.., shared)| shared.strong_count() > 0);
        let key = (handle, TypeId::of::<T>());
        let existing = shared_topics
            .iter()
            .find(|(owner, name, shared)| {
                *owner == key && name == topic_name && shared.strong_count() > 0
            })
            .and_then(|(.., shared)| shared.upgrade());
        let (shared, topic) = match existing {
            Some(shared) => {
                let Some(topic) = shared.topic.downcast_ref::<Topic<T>>().cloned() else {
                    // Not reached: the key holds the data type.
                    return Err(ConnectorError::Dds(InternalError::code(
                        return_code::PRECONDITION_NOT_MET,
                    )));
                };
                suits(&topic)?;
                shared.listener.add(Arc::clone(&error_listener));
                (shared, topic)
            }
            None => {
                let topic =
                    participant.create_topic::<T>(topic_name, profile.topic_qos(topic_name))?;
                // `create_topic` returns a topic of the name and type that
                // is there with the QoS it has, not with the one asked for.
                if let Err(error) = suits(&topic) {
                    // Obtained once more, so given back once: the topic
                    // stays for who created it.
                    participant.delete_topic(&topic)?;
                    return Err(error);
                }
                let listener = Arc::new(TopicStatusListener::new(Arc::clone(&error_listener)));
                let mask = listener.mask();
                topic.set_listener(Some(Arc::clone(&listener) as ArcTopicListener), mask);
                let shared = Arc::new(SharedTopic {
                    listener,
                    topic: Box::new(topic.clone()),
                });
                shared_topics.push((key, topic_name.to_owned(), Arc::downgrade(&shared)));
                (shared, topic)
            }
        };
        drop(shared_topics);
        topics.push((participant.clone(), topic.clone()));
        created.topics.push(handle);
        let (owner, of_owner) = (participant.clone(), topic.clone());
        // Set when the connector gave up its share of the topic; a
        // deletion of the topic that failed is tried again without it.
        let mut released = false;
        created.push(move |failed_before| {
            let mut shared_topics = lock(&SHARED_TOPICS);
            if !released {
                if !shared.listener.remove(&error_listener) {
                    // Another connector uses the topic in this
                    // DomainParticipant: it stays, with its listener.
                    return Ok(());
                }
                shared_topics.retain(|(.., of_topic)| !of_topic.ptr_eq(&Arc::downgrade(&shared)));
                released = true;
            }
            match owner.delete_topic(&of_owner) {
                // A DataReader or DataWriter whose removal failed before
                // may be what still uses the topic: tried again with it.
                Err(error @ DdsError::PreconditionNotMet { .. }) if failed_before => {
                    Err(error.into())
                }
                // A topic the application uses beside the connector
                // stays.
                Ok(()) | Err(DdsError::PreconditionNotMet { .. }) => Ok(()),
                Err(error) => Err(error.into()),
            }
        });
        Ok(topic)
    }

    /// The DataWriter of a publishing port of `component`.
    fn writer(&self, component: &Component) -> Result<DataWriter<T>, ConnectorError> {
        self.fragment(|created| {
            let at = self.place::<Publisher>(component, created)?;
            self.writer_at(&at, created)
        })
    }

    /// The DataWriter of a publishing port placed at `at`.
    fn writer_at(
        &self,
        at: &Placed<'_, Publisher, T>,
        created: &mut Created,
    ) -> Result<DataWriter<T>, ConnectorError> {
        let qos = at
            .profile
            .datawriter_qos_of_pattern(&at.topic_name, self.history);
        let writer = at.group.create_datawriter::<T>(&at.topic, qos)?;
        let (publisher, of_publisher) = (Arc::clone(&at.group), writer.clone());
        created.push(move |_| Ok(publisher.delete_datawriter(&of_publisher)?));
        Ok(writer)
    }

    /// Completes the subscribing `port` of `component` with the entities
    /// of the connector. The data the DataReader holds go to the listener
    /// of a push port after the fragment is complete and `changing` is
    /// released: a listener that connects a port to the connector, or
    /// removes it, does not wait for itself.
    fn subscribe(
        &self,
        component: &Component,
        port: &Arc<Subscribing<T>>,
    ) -> Result<(), ConnectorError> {
        let delivery = self.fragment(|created| {
            let at = self.place::<Subscriber>(component, created)?;
            let delivery = port.configure(
                &at.participant,
                &at.group,
                &at.topic,
                at.profile
                    .datareader_qos_of_pattern(&at.topic_name, self.history),
                self.base.error_listener(),
                component.thread_pool()?,
            )?;
            let port = Arc::clone(port);
            created.push(move |_| port.remove());
            Ok(delivery)
        })?;
        delivery.deliver();
        Ok(())
    }

    /// Port type `DDS_Update` for `component`. With `is_global_scope` the
    /// fragment has a DataReader of its own to look the instances up on;
    /// its statuses go to `error_listener`.
    fn update(
        &self,
        component: &Component,
        is_global_scope: bool,
    ) -> Result<DdsUpdate<T>, ConnectorError> {
        self.fragment(|created| self.update_in(component, is_global_scope, created))
    }

    /// The fragment of [`Self::update`].
    fn update_in(
        &self,
        component: &Component,
        is_global_scope: bool,
        created: &mut Created,
    ) -> Result<DdsUpdate<T>, ConnectorError> {
        let at = self.place::<Publisher>(component, created)?;
        let writer = self.writer_at(&at, created)?;
        if !is_global_scope {
            return Ok(DdsUpdate::new(writer, self.base.error_listener()));
        }
        let (subscriber, topic) = if at.dedicated_participant {
            // §7.4.3: "specific entities dedicated to the 'non-compatible'
            // port". The DomainParticipant dedicated to the port holds
            // both of its entities, the DataReader in a Subscriber
            // dedicated to the port as well.
            let requested = Subscriber::requested(at.profile, Some(&at.topic_name));
            let subscriber = Arc::new(Subscriber::create(&at.participant, requested)?);
            let dedicated = Arc::clone(&subscriber);
            created.push(move |_| Ok(dedicated.delete()?));
            (subscriber, at.topic.clone())
        } else {
            // A default DomainParticipant suits the port: the Subscriber
            // is found as for a subscribing port.
            let beside = self.place::<Subscriber>(component, created)?;
            (beside.group, beside.topic)
        };
        let qos = at
            .profile
            .datareader_qos_of_pattern(&at.topic_name, self.history);
        let observer = subscriber.create_datareader(&topic, qos)?;
        let of_subscriber = observer.clone();
        created.push(move |_| Ok(subscriber.delete_datareader(&of_subscriber)?));
        Ok(DdsUpdate::with_global_scope(
            writer,
            observer,
            self.base.error_listener(),
        ))
    }

    /// Deletes what the fragments created, the last first. Returns the
    /// first error; the others are still tried. What could not be deleted
    /// stays with the connector, and the next removal tries it again.
    fn remove(&self) -> Result<(), InternalError> {
        let _changing = lock(&self.changing);
        let created = std::mem::take(&mut *lock(&self.created));
        lock(&self.topics).clear();
        let mut first = Ok(());
        let mut left = Vec::new();
        for mut removal in created.into_iter().rev() {
            if let Err(error) = removal(first.is_err()) {
                if first.is_ok() {
                    first = Err(error);
                }
                // Tried again by the next removal. An entity deleted
                // meanwhile, as the contents of a DomainParticipant
                // dedicated to a port that this removal deleted, is gone:
                // nothing is left to delete.
                left.push(Box::new(move |failed_before| match removal(failed_before) {
                    Err(error) if error.error_code == return_code::ALREADY_DELETED => Ok(()),
                    other => other,
                }) as Removal);
            }
        }
        // In the order of creation, as `created` keeps them; no fragment
        // was added meanwhile, `changing` is held.
        left.reverse();
        *lock(&self.created) = left;
        first
    }
}

/// What the two pattern connectors share: the base, the completion and
/// the removal.
macro_rules! topic_connector {
    ($connector:ident, $history:expr) => {
        impl<T: DdsType + Send + Sync + 'static> Deref for $connector<T> {
            type Target = DdsTopicBase;

            fn deref(&self) -> &DdsTopicBase {
                &self.fragments.base
            }
        }

        impl<T: DdsType + Send + Sync + 'static> Default for $connector<T> {
            fn default() -> Self {
                Self::new()
            }
        }

        impl<T: DdsType + Send + Sync + 'static> $connector<T> {
            /// A connector that is not configured: `domain_id` 0, no QoS
            /// profile, an empty `topic_name`, no `key_fields`.
            #[must_use]
            pub fn new() -> Self {
                Self {
                    fragments: Fragments::new($history),
                }
            }

            /// Completes the configuration; see
            /// [`DdsTopicBase::configuration_complete`].
            ///
            /// # Errors
            /// As [`DdsTopicBase::configuration_complete`].
            pub fn configuration_complete(&self) -> Result<(), ConnectorError> {
                self.fragments.base.configuration_complete::<T>()
            }

            /// Removes the fragments of the connector: deletes the
            /// DataReaders and DataWriters of its ports, the Publishers,
            /// Subscribers and DomainParticipants dedicated to a port and
            /// the topic. The attributes stay fixed.
            ///
            /// # Errors
            /// `InternalError` with the code of the first DDS error.
            pub fn remove(&self) -> Result<(), InternalError> {
                self.fragments.remove()
            }
        }
    };
}

/// Connector `DDS_State` (§7.3.2), pattern State Transfer: "participants
/// that publish the state of data they manage (role observable),
/// associated with other participants that subscribe to get the
/// information (role observer)".
///
/// "Typically, with this pattern, HISTORY QoS should be set to
/// KEEP_LAST": the DataWriters and DataReaders of the connector have
/// KEEP_LAST where the QoS profile does not set HISTORY.
///
/// ```text
/// connector DDS_State : DDS_TopicBase {
///     mirrorport DDS_Update      observable;
///     mirrorport DDS_Read        passive_observer;
///     mirrorport DDS_Get         pull_observer;
///     mirrorport DDS_Listen      push_observer;
///     mirrorport DDS_StateListen push_state_observer;
/// };
/// ```
pub struct DdsState<T: DdsType> {
    fragments: Fragments<T>,
}

/// Connector `DDS_Event` (§7.3.3), pattern Event Transfer: "participants
/// sending events over DDS (role supplier), while others consume them
/// (role consumer)".
///
/// "Typically, with this pattern, HISTORY QoS should be set to
/// KEEP_ALL": the DataWriters and DataReaders of the connector have
/// KEEP_ALL where the QoS profile does not set HISTORY.
///
/// ```text
/// connector DDS_Event : DDS_TopicBase {
///     mirrorport DDS_Write  supplier;
///     mirrorport DDS_Get    pull_consumer;
///     mirrorport DDS_Listen push_consumer;
/// };
/// ```
pub struct DdsEvent<T: DdsType> {
    fragments: Fragments<T>,
}

topic_connector!(DdsState, HistoryKind::KeepLast);
topic_connector!(DdsEvent, HistoryKind::KeepAll);

impl<T: DdsType + Send + Sync + 'static> DdsState<T> {
    /// Mirror port `observable`: the port `DDS_Update` of `component`,
    /// which publishes the state. `is_global_scope` is the attribute of
    /// its `Updater`.
    ///
    /// # Errors
    /// [`ConnectorError::NotConfigured`] before `configuration_complete`,
    /// [`ConnectorError::Dds`] if DDS refuses an entity.
    pub fn observable(
        &self,
        component: &Component,
        is_global_scope: bool,
    ) -> Result<DdsUpdate<T>, ConnectorError> {
        self.fragments.update(component, is_global_scope)
    }

    /// Mirror port `passive_observer`: connects the port `DDS_Read` of
    /// `component`, which reads the state when it wants, and completes
    /// its configuration.
    ///
    /// # Errors
    /// As [`Self::observable`]; `PRECONDITION_NOT_MET` for a port that is
    /// configured.
    pub fn passive_observer(
        &self,
        component: &Component,
        port: &DdsRead<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }

    /// Mirror port `pull_observer`: connects the port `DDS_Get` of
    /// `component`, which gets the state changes.
    ///
    /// # Errors
    /// As [`Self::passive_observer`].
    pub fn pull_observer(
        &self,
        component: &Component,
        port: &DdsGet<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }

    /// Mirror port `push_observer`: connects the port `DDS_Listen` of
    /// `component`, which is notified of the state changes.
    ///
    /// # Errors
    /// As [`Self::passive_observer`].
    pub fn push_observer(
        &self,
        component: &Component,
        port: &DdsListen<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }

    /// Mirror port `push_state_observer`: connects the port
    /// `DDS_StateListen` of `component`, which is notified "with
    /// different operations depending on the instance status".
    ///
    /// # Errors
    /// As [`Self::passive_observer`].
    pub fn push_state_observer(
        &self,
        component: &Component,
        port: &DdsStateListen<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }
}

impl<T: DdsType + Send + Sync + 'static> DdsEvent<T> {
    /// Mirror port `supplier`: the port `DDS_Write` of `component`, which
    /// sends the events.
    ///
    /// # Errors
    /// [`ConnectorError::NotConfigured`] before `configuration_complete`,
    /// [`ConnectorError::Dds`] if DDS refuses an entity.
    pub fn supplier(&self, component: &Component) -> Result<DdsWrite<T>, ConnectorError> {
        let writer = self.fragments.writer(component)?;
        Ok(DdsWrite::new(writer, self.fragments.base.error_listener()))
    }

    /// Mirror port `pull_consumer`: connects the port `DDS_Get` of
    /// `component`, which gets the events, and completes its
    /// configuration.
    ///
    /// # Errors
    /// As [`Self::supplier`]; `PRECONDITION_NOT_MET` for a port that is
    /// configured.
    pub fn pull_consumer(
        &self,
        component: &Component,
        port: &DdsGet<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }

    /// Mirror port `push_consumer`: connects the port `DDS_Listen` of
    /// `component`, which is notified of the events.
    ///
    /// # Errors
    /// As [`Self::pull_consumer`].
    pub fn push_consumer(
        &self,
        component: &Component,
        port: &DdsListen<T>,
    ) -> Result<(), ConnectorError> {
        self.fragments.subscribe(component, port.fragment())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::dds_type::{DecodeError, EncodeError};
    use zerodds_types::PrimitiveKind;
    use zerodds_types::builder::{StructBuilder, TypeObjectBuilder};

    /// `struct { @key long id; @key long zone; long value; }`.
    fn keyed_structure(name: &str) -> StructBuilder {
        let long = || TypeIdentifier::Primitive(PrimitiveKind::Int32);
        TypeObjectBuilder::struct_type(name)
            .member("id", long(), |member| member.key())
            .member("zone", long(), |member| member.key())
            .member("value", long(), |member| member)
    }

    macro_rules! keyed_type {
        ($name:ident, $structure:expr) => {
            struct $name;

            impl DdsType for $name {
                const TYPE_NAME: &'static str = concat!("test::Dds4Ccm", stringify!($name));
                const HAS_KEY: bool = true;

                fn type_object() -> Option<TypeObject> {
                    let structure = $structure.build_complete();
                    Some(TypeObject::Complete(CompleteTypeObject::Struct(structure)))
                }

                fn encode(&self, _out: &mut Vec<u8>) -> Result<(), EncodeError> {
                    Ok(())
                }

                fn decode(_bytes: &[u8]) -> Result<Self, DecodeError> {
                    Ok(Self)
                }
            }
        };
    }

    // A type that names its key members.
    keyed_type!(Named, keyed_structure("test::Named"));
    // A type with a base type: the key members of the base are not in its
    // own `TypeObject`.
    keyed_type!(
        Derived,
        keyed_structure("test::Derived").base(TypeIdentifier::Primitive(PrimitiveKind::Int32))
    );

    fn completed<T: DdsType>(key_fields: &[&str]) -> Result<(), ConnectorError> {
        let connector = DdsTopicBase::new();
        connector.set_topic_name("Dds4CcmKeyFields").unwrap();
        connector
            .set_key_fields(key_fields.iter().map(|field| (*field).to_owned()).collect())
            .unwrap();
        connector.configuration_complete::<T>()
    }

    fn refused(key_fields: &[&str], key_of_type: Option<&[&str]>) -> Result<(), ConnectorError> {
        let owned = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
        Err(ConnectorError::KeyFields {
            key_fields: owned(key_fields),
            key_of_type: key_of_type.map(owned),
        })
    }

    /// §7.3: `key_fields` is "the list of fields making up the key for
    /// that Topic". For a type that names its key members the list must
    /// be those members, in any order.
    #[test]
    fn dds4ccm_7_3_1_key_fields_are_compared_with_the_key_members_the_type_names() {
        assert_eq!(
            key_members_of::<Named>(),
            Some(vec!["id".into(), "zone".into()])
        );
        assert_eq!(completed::<Named>(&["id", "zone"]), Ok(()));
        assert_eq!(completed::<Named>(&["zone", "id"]), Ok(()));
        let of_type = Some(&["id", "zone"][..]);
        for wrong in [&["id"][..], &[], &["id", "zone", "value"], &["id", "value"]] {
            assert_eq!(completed::<Named>(wrong), refused(wrong, of_type));
        }
    }

    /// A type that does not name all its key members (a base type, or no
    /// complete `TypeObject`) is checked for having a key at all.
    #[test]
    fn dds4ccm_7_3_1_key_fields_of_a_type_that_does_not_name_its_key_are_checked_for_presence() {
        assert_eq!(key_members_of::<Derived>(), None);
        assert_eq!(completed::<Derived>(&["inherited"]), Ok(()));
        assert_eq!(completed::<Derived>(&[]), refused(&[], None));
    }

    /// A connector completed on its `DDS_Base` alone has unchecked topic
    /// attributes: they are fixed, and it is not configured as a topic
    /// connector.
    #[test]
    fn dds4ccm_7_3_1_a_topic_connector_is_completed_as_one() {
        let connector = DdsTopicBase::new();
        DdsBase::configuration_complete(&connector).unwrap();
        assert_eq!(connector.set_topic_name("Late"), Err(NonChangeable));
        assert_eq!(
            connector.configured().err(),
            Some(ConnectorError::NotConfigured)
        );
        assert_eq!(
            connector.configuration_complete::<Named>(),
            Err(ConnectorError::AlreadyConfigured)
        );
    }

    #[test]
    fn dds4ccm_7_3_1_a_connector_error_says_what_is_wrong() {
        let text = |error: ConnectorError| error.to_string();
        assert_eq!(text(ConnectorError::TopicName), "topic_name is empty");
        assert_eq!(
            text(completed::<Named>(&["id"]).unwrap_err()),
            "key_fields [\"id\"] are not the key members [\"id\", \"zone\"] of the data type"
        );
        assert_eq!(
            text(completed::<Derived>(&[]).unwrap_err()),
            "key_fields is empty and the data type has a key"
        );
        let profile = ConnectorError::QosProfile(ProfileError::NoProfile);
        assert_eq!(text(profile), "qos_profile: the document has no profile");
        let topic_qos = ConnectorError::TopicQos {
            topic_name: "Track".into(),
            policies: vec!["durability", "history"],
        };
        assert_eq!(
            text(topic_qos),
            "the topic `Track` exists with other values than the topic_qos of qos_profile sets: \
             durability, history"
        );
    }
}
