// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Topic — the typed rendezvous point between DataWriter and DataReader.
//!
//! Spec reference:
//! - OMG DDS 1.4 §2.2.2.3.1 `TopicDescription` (base class for
//!   `Topic`, `ContentFilteredTopic`, `MultiTopic`),
//! - §2.2.2.3.2 `Topic` (concrete TopicDescription with type +
//!   Topic QoS),
//! - §2.2.2.2.1.12 `lookup_topicdescription` (untyped lookup),
//! - §2.2.2.2.1.13 `create_contentfilteredtopic`.
//!
//! In v1.2 we stay simple: the `Topic<T>` is a handle that carries
//! name + type_name and a generic `PhantomData<fn() -> T>` for static type
//! safety. The topic references its `DomainParticipant` so that
//! `TopicDescription::get_participant` works faithfully to spec. The
//! cycle is broken in the topic registry: it holds only
//! `Arc<TopicInner>`, **not** the cloned `DomainParticipant` handle.

extern crate alloc;
use alloc::borrow::Cow;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::sync::RwLock;

use zerodds_sql_filter::{Expr, RowAccess, Value};

use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
use crate::listener::ArcTopicListener;
use crate::participant::DomainParticipant;
use crate::qos::TopicQos;

/// `TopicDescription` trait — base interface for anything a
/// `DataReader` (typed via `T`) can obtain samples from.
///
/// Spec reference: OMG DDS 1.4 §2.2.2.3.1 "TopicDescription is the
/// most abstract description of a topic. It encapsulates the
/// information that is common to all the kinds of topic that can be
/// used: `Topic`, `ContentFilteredTopic`, `MultiTopic`."
///
/// The trait is **object-safe** — we want to be able to return it as
/// `&dyn TopicDescription` from `lookup_topicdescription` and
/// `find_topic`.
pub trait TopicDescription {
    /// Type name (e.g. `"std_msgs::msg::String"`).
    /// Spec §2.2.2.3.1 `get_type_name`.
    fn get_type_name(&self) -> &str;
    /// Topic name (e.g. `"ChatterTopic"`).
    /// Spec §2.2.2.3.1 `get_name`.
    fn get_name(&self) -> &str;
    /// Owning participant.
    /// Spec §2.2.2.3.1 `get_participant`.
    fn get_participant(&self) -> &DomainParticipant;
}

/// How many of each kind of user one topic has right now, as
/// `DomainParticipant::delete_topic` counts them. An all-zero value is the
/// precondition of DDS 1.4 §2.2.2.2.1.6 satisfied: "The deletion of a Topic
/// is not allowed if there are any existing DataReader, DataWriter,
/// ContentFilteredTopic, or MultiTopic objects that are using the Topic."
///
/// Read it with [`crate::DomainParticipant::topic_users`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TopicUserCounts {
    /// DataWriters created on the topic (§2.2.2.4.2).
    pub data_writers: usize,
    /// DataReaders created on the topic (§2.2.2.5.2). A reader created on a
    /// `ContentFilteredTopic` of the topic counts here as well.
    pub data_readers: usize,
    /// ContentFilteredTopics over the topic (§2.2.2.3.3).
    pub content_filtered_topics: usize,
    /// MultiTopics over the topic (§2.2.2.3.4).
    pub multi_topics: usize,
}

/// Which of the four object kinds the spec names in `delete_topic`'s
/// precondition a ledger entry counts. `TopicUserKind` is internal; the
/// counts it maintains are read through [`TopicUserCounts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopicUserKind {
    /// A `DataWriter` on the topic (§2.2.2.4.2).
    DataWriter,
    /// A `DataReader` on the topic (§2.2.2.5.2).
    DataReader,
    /// A `ContentFilteredTopic` on the topic (§2.2.2.3.3).
    ContentFilteredTopic,
    /// A `MultiTopic` over the topic (§2.2.2.3.4).
    MultiTopic,
}

impl TopicUserCounts {
    /// `true` when nothing uses the topic, i.e. when `delete_topic` may
    /// delete it.
    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn field(&mut self, kind: TopicUserKind) -> &mut usize {
        match kind {
            TopicUserKind::DataWriter => &mut self.data_writers,
            TopicUserKind::DataReader => &mut self.data_readers,
            TopicUserKind::ContentFilteredTopic => &mut self.content_filtered_topics,
            TopicUserKind::MultiTopic => &mut self.multi_topics,
        }
    }

    /// One more user of that kind.
    pub(crate) fn add(&mut self, kind: TopicUserKind) {
        let field = self.field(kind);
        *field = field.saturating_add(1);
    }

    /// The sum of two counts.
    pub(crate) fn plus(self, other: Self) -> Self {
        Self {
            data_writers: self.data_writers.saturating_add(other.data_writers),
            data_readers: self.data_readers.saturating_add(other.data_readers),
            content_filtered_topics: self
                .content_filtered_topics
                .saturating_add(other.content_filtered_topics),
            multi_topics: self.multi_topics.saturating_add(other.multi_topics),
        }
    }

    /// One user fewer. A count only ever reaches zero: each object that
    /// adds itself removes itself exactly once, on the same topic name, so
    /// there is no case where this is asked to take below zero. The
    /// saturating subtraction makes that a floor rather than a wrap.
    pub(crate) fn remove(&mut self, kind: TopicUserKind) {
        let field = self.field(kind);
        *field = field.saturating_sub(1);
    }
}

/// The deletion state a ContentFilteredTopic or MultiTopic shares between
/// all its handles. DDS 1.4 §2.2.2.2.1.8 and §2.2.2.2.1.10: deleting one
/// "with existing DataReader objects attached to it" returns
/// PRECONDITION_NOT_MET; a deleted one answers ALREADY_DELETED
/// (§2.2.1.1).
#[derive(Debug)]
pub(crate) struct DescriptionLife {
    state: std::sync::Mutex<DescriptionLifeState>,
}

#[derive(Debug)]
struct DescriptionLifeState {
    deleted: bool,
    /// Live handles that count as users of the related topics.
    handles: usize,
    /// DataReaders created on the description that still exist.
    readers: usize,
}

impl DescriptionLife {
    /// A description with its first handle.
    pub(crate) fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(DescriptionLifeState {
                deleted: false,
                handles: 1,
                readers: 0,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, DescriptionLifeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// One more handle. `true` if it is a user of the related topics,
    /// `false` if the description is deleted and released them already.
    pub(crate) fn add_handle(&self) -> bool {
        let mut st = self.lock();
        st.handles = st.handles.saturating_add(1);
        !st.deleted
    }

    /// One handle fewer. `true` if it was a user of the related topics.
    pub(crate) fn drop_handle(&self) -> bool {
        let mut st = self.lock();
        st.handles = st.handles.saturating_sub(1);
        !st.deleted
    }

    /// `true` once `delete` has succeeded.
    pub(crate) fn is_deleted(&self) -> bool {
        self.lock().deleted
    }

    /// `AlreadyDeleted` once `delete` has succeeded.
    pub(crate) fn check_not_deleted(&self) -> Result<()> {
        if self.lock().deleted {
            return Err(DdsError::AlreadyDeleted);
        }
        Ok(())
    }

    /// Deletes the description. Returns how many handles counted as users
    /// of the related topics until now; the caller gives those back.
    ///
    /// # Errors
    /// `AlreadyDeleted` if it is deleted; `PreconditionNotMet` while a
    /// DataReader created on it exists.
    pub(crate) fn delete(&self) -> Result<usize> {
        let mut st = self.lock();
        if st.deleted {
            return Err(DdsError::AlreadyDeleted);
        }
        if st.readers > 0 {
            return Err(DdsError::PreconditionNotMet {
                reason: "DataReader objects are attached to the topic description",
            });
        }
        st.deleted = true;
        Ok(st.handles)
    }

    /// Records a DataReader created on the description, held by the
    /// returned guard until the reader is deleted.
    ///
    /// # Errors
    /// `AlreadyDeleted` if the description is deleted.
    pub(crate) fn attach_reader(self: &Arc<Self>) -> Result<DescriptionReaderUse> {
        let mut st = self.lock();
        if st.deleted {
            return Err(DdsError::AlreadyDeleted);
        }
        st.readers = st.readers.saturating_add(1);
        Ok(DescriptionReaderUse {
            life: Arc::clone(self),
        })
    }
}

/// One DataReader's hold on its ContentFilteredTopic or MultiTopic; the end
/// of the reader gives it back.
#[derive(Debug)]
pub(crate) struct DescriptionReaderUse {
    life: Arc<DescriptionLife>,
}

impl Drop for DescriptionReaderUse {
    fn drop(&mut self) {
        let mut st = self.life.lock();
        st.readers = st.readers.saturating_sub(1);
    }
}

/// Typed topic handle.
#[derive(Debug)]
pub struct Topic<T: DdsType> {
    inner: Arc<TopicInner>,
    codec: Arc<dyn TopicCodec<T>>,
    /// Owning participant (no cycle: in the topic registry the
    /// participant holds only the inner, not the handle). `None` for
    /// builtin topics constructed without a participant (see
    /// [`Self::new_orphan`]).
    participant: Option<DomainParticipant>,
    /// A topic holds no `T`, so it is `Send` and `Sync` whatever `T` is.
    _t: PhantomData<fn() -> T>,
}

/// Body and key codecs; framing bounds and entity policies remain with callers.
///
/// Implementations are Rust-only. Reader paths invoke this contract under
/// cache locks, so it must not admit Python or other foreign callbacks.
pub(crate) trait TopicCodec<T: DdsType>: core::fmt::Debug + Send + Sync {
    fn dependent_type_objects(&self) -> &[zerodds_types::TypeObject] {
        &[]
    }
    /// The strongly connected component the type belongs to
    /// ([`DdsType::type_component`]).
    fn type_component(&self) -> Option<&zerodds_types::resolve::TypeComponent> {
        None
    }
    fn validate_sample(&self, _sample: &T) -> Result<()> {
        Ok(())
    }
    fn extensibility(&self) -> crate::dds_type::Extensibility {
        T::extensibility()
    }

    fn validates_static_metadata(&self) -> bool {
        true
    }
    fn encode(
        &self,
        sample: &T,
        out: &mut Vec<u8>,
        xcdr1: bool,
        big_endian: bool,
    ) -> core::result::Result<(), crate::dds_type::EncodeError>;

    fn decode(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> core::result::Result<T, crate::dds_type::DecodeError>;

    /// Actual hash and holder, distinct from the absence of a keyed instance.
    /// The static adapter retains the existing infallible holder contract.
    fn key_material(&self, sample: &T) -> Option<([u8; 16], Vec<u8>)>;

    /// The key holder alone, with the statically known maximum length that
    /// decides how XTypes 1.3 §7.6.8 hashes it, for a caller that may
    /// already know the hash of these octets. `None` where the codec has no
    /// holder of its own to give; the caller then asks [`Self::key_material`],
    /// which also answers for a keyless type.
    fn key_holder(&self, _sample: &T) -> Option<(Vec<u8>, usize)> {
        None
    }

    /// [`Self::key_holder`] written into `out`, which is cleared first and
    /// keeps its capacity; the statically known maximum length is the
    /// answer, `None` as there.
    fn key_holder_into(&self, sample: &T, out: &mut Vec<u8>) -> Option<usize> {
        let (holder, max) = self.key_holder(sample)?;
        *out = holder;
        Some(max)
    }

    fn decode_key_holder(
        &self,
        bytes: &[u8],
    ) -> core::result::Result<T, crate::dds_type::DecodeError>;
}

/// The KeyHash under which an endpoint of a topic without a key tracks its
/// one instance (DDS 1.4 §2.2.1.2.2). It is not sent: a keyless writer's
/// DATA carries no PID_KEY_HASH.
pub(crate) const KEYLESS_INSTANCE_KEY: [u8; 16] = [0u8; 16];

/// The key holder of the last sample an endpoint keyed and its KeyHash.
///
/// XTypes 1.3 §7.6.8 makes the hash of a holder that can exceed sixteen
/// octets the MD5 of it, and a writer usually writes — and a reader
/// usually receives — the same instance again. [`Self::key_material`]
/// compares the holder with the last one and reuses its hash, so a
/// repeated instance costs a comparison and not a digest. Measured on the
/// Linux test host with a 1 kB string key (`zerodds_perf`): the MD5 was
/// about half the writing thread's time, and the reading thread spent
/// as much on it for samples `KEEP_LAST` then replaced.
///
/// [`Self::key_hash`] also keeps the buffer it writes the holder into, so a
/// writer that only needs the hash allocates nothing per sample.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct KeyMaterialCache(std::sync::Mutex<KeyMaterial>);

/// What [`KeyMaterialCache`] holds between calls.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub(crate) struct KeyMaterial {
    /// The last holder and its KeyHash.
    last: Option<(Vec<u8>, [u8; 16])>,
    /// The buffer the next holder is written into.
    scratch: Vec<u8>,
}

#[cfg(feature = "std")]
impl KeyMaterialCache {
    /// The KeyHash of `sample`, taken from the last call when the holder is
    /// the same octets. Without allocating once the buffers have grown: the
    /// holder is written into a buffer kept here, and a new holder's buffer
    /// and the last one's trade places. `None` for a keyless type.
    pub(crate) fn key_hash<T: DdsType>(
        &self,
        codec: &dyn TopicCodec<T>,
        sample: &T,
    ) -> Option<[u8; 16]> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut holder = core::mem::take(&mut state.scratch);
        let Some(max) = codec.key_holder_into(sample, &mut holder) else {
            state.scratch = holder;
            drop(state);
            return codec.key_material(sample).map(|(hash, _)| hash);
        };
        if let Some((bytes, hash)) = state.last.as_ref() {
            if *bytes == holder {
                let hash = *hash;
                state.scratch = holder;
                return Some(hash);
            }
        }
        let hash = crate::dds_type::compute_key_hash(&holder, max);
        state.scratch = state
            .last
            .replace((holder, hash))
            .map(|(bytes, _)| bytes)
            .unwrap_or_default();
        Some(hash)
    }

    /// The key holder that a KeyHash spells out, for an instance known by
    /// its hash alone: DDSI-RTPS 2.5 §9.6.4.8 Step 5.1 makes the KeyHash of
    /// a key of at most sixteen octets that key's PLAIN_CDR2-BE octets,
    /// padded with zeros. The octets count as a holder only if they hash
    /// back to `hash`; the MD5 of Step 5.2 never does.
    pub(crate) fn holder_from_hash<T: DdsType>(
        &self,
        codec: &dyn TopicCodec<T>,
        hash: [u8; 16],
    ) -> Option<Vec<u8>> {
        let key = codec.decode_key_holder(&hash).ok()?;
        (self.key_hash(codec, &key)? == hash)
            .then(|| self.holder_of(hash))
            .flatten()
    }

    /// A copy of the last holder [`Self::key_hash`] wrote, when its hash is
    /// `hash` — for the instance tracker, which keeps the holder of a new
    /// instance, without writing the holder a second time. `None` when
    /// another key was written since.
    pub(crate) fn holder_of(&self, hash: [u8; 16]) -> Option<Vec<u8>> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state
            .last
            .as_ref()
            .filter(|(_, last)| *last == hash)
            .map(|(bytes, _)| bytes.clone())
    }

    /// [`TopicCodec::key_material`] for `sample`, the hash taken from the
    /// last call when the holder is the same octets.
    pub(crate) fn key_material<T: DdsType>(
        &self,
        codec: &dyn TopicCodec<T>,
        sample: &T,
    ) -> Option<([u8; 16], Vec<u8>)> {
        let Some((holder, max)) = codec.key_holder(sample) else {
            return codec.key_material(sample);
        };
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((bytes, hash)) = state.last.as_ref() {
            if *bytes == holder {
                return Some((*hash, holder));
            }
        }
        let hash = crate::dds_type::compute_key_hash(&holder, max);
        state.last = Some((holder.clone(), hash));
        Some((hash, holder))
    }
}

/// The XTypes description of a topic type that is not the Rust type the topic
/// carries: the `CompleteTypeObject` of the type and the objects it names by
/// hash. See [`crate::DomainParticipant::create_topic_with_type_information`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicTypeInformation {
    /// The complete object of the topic type.
    pub object: zerodds_types::TypeObject,
    /// Every object the root names by hash, directly or through another.
    pub dependencies: Vec<zerodds_types::TypeObject>,
}

/// Exact delegation to the existing static DdsType codec defaults and overrides.
///
/// Carries the type's dependency objects (`DdsType::type_object_dependencies`)
/// so that endpoint creation registers the whole closure the type's
/// own object names by hash, not the top-level object alone.
#[derive(Debug)]
pub(crate) struct StaticTopicCodec {
    dependencies: Vec<zerodds_types::TypeObject>,
    /// `false` for a topic created under a type name other than the one
    /// `T` carries ([`Self::named_of`]): its metadata is then not `T`'s
    /// own and is not checked against it.
    validates_metadata: bool,
    component: Option<zerodds_types::resolve::TypeComponent>,
}

impl StaticTopicCodec {
    pub(crate) fn of<T: DdsType>() -> Self {
        Self {
            dependencies: T::type_object_dependencies(),
            validates_metadata: true,
            component: T::type_component(),
        }
    }

    /// The codec of a topic whose type is described by `dependencies`
    /// and not by `T`'s own objects.
    pub(crate) fn described_by<T: DdsType>(dependencies: Vec<zerodds_types::TypeObject>) -> Self {
        Self {
            dependencies,
            ..Self::named_of::<T>()
        }
    }

    /// The codec of a topic whose type name the creator chose.
    pub(crate) fn named_of<T: DdsType>() -> Self {
        Self {
            validates_metadata: false,
            ..Self::of::<T>()
        }
    }
}

impl<T: DdsType> TopicCodec<T> for StaticTopicCodec {
    fn dependent_type_objects(&self) -> &[zerodds_types::TypeObject] {
        &self.dependencies
    }

    fn validates_static_metadata(&self) -> bool {
        self.validates_metadata
    }

    fn type_component(&self) -> Option<&zerodds_types::resolve::TypeComponent> {
        self.component.as_ref()
    }

    fn key_material(&self, sample: &T) -> Option<([u8; 16], Vec<u8>)> {
        if !T::has_key() {
            return None;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
        sample.encode_key_holder_be(&mut holder);
        let max = T::key_holder_max_size().unwrap_or(usize::MAX);
        let hash = crate::dds_type::compute_key_hash(holder.as_bytes(), max);
        // The holder's own buffer, moved: a copy here was one allocation per
        // write of a keyed sample.
        Some((hash, holder.into_bytes()))
    }

    fn key_holder(&self, sample: &T) -> Option<(Vec<u8>, usize)> {
        if !T::has_key() {
            return None;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::new();
        sample.encode_key_holder_be(&mut holder);
        Some((
            holder.into_bytes(),
            T::key_holder_max_size().unwrap_or(usize::MAX),
        ))
    }

    fn key_holder_into(&self, sample: &T, out: &mut Vec<u8>) -> Option<usize> {
        if !T::has_key() {
            return None;
        }
        let mut holder = crate::dds_type::PlainCdr2BeKeyHolder::reusing(core::mem::take(out));
        sample.encode_key_holder_be(&mut holder);
        *out = holder.into_bytes();
        Some(T::key_holder_max_size().unwrap_or(usize::MAX))
    }

    fn decode_key_holder(
        &self,
        bytes: &[u8],
    ) -> core::result::Result<T, crate::dds_type::DecodeError> {
        T::decode_key_holder_be(bytes)
    }

    fn encode(
        &self,
        sample: &T,
        out: &mut Vec<u8>,
        xcdr1: bool,
        big_endian: bool,
    ) -> core::result::Result<(), crate::dds_type::EncodeError> {
        match (xcdr1, big_endian) {
            (false, false) => sample.encode(out),
            (false, true) => sample.encode_be(out),
            (true, false) => sample.encode_xcdr1(out),
            (true, true) => sample.encode_xcdr1_be(out),
        }
    }

    fn decode(
        &self,
        bytes: &[u8],
        representation: u8,
        big_endian: bool,
    ) -> core::result::Result<T, crate::dds_type::DecodeError> {
        match (representation, big_endian) {
            (0, false) => T::decode_xcdr1(bytes),
            (0, true) => T::decode_xcdr1_be(bytes),
            (_, false) => T::decode(bytes),
            (_, true) => T::decode_be(bytes),
        }
    }
}

/// Metadata captured once per topic from static accessors or validated type support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopicTypeMetadata {
    pub type_name: Cow<'static, str>,
    pub type_identifier: zerodds_types::TypeIdentifier,
    pub type_object: Option<zerodds_types::type_object::TypeObject>,
    pub keyed: bool,
    pub extensibility: crate::dds_type::Extensibility,
    pub key_holder_max_size: Option<usize>,
    pub max_serialized_size: Option<usize>,
}

impl TopicTypeMetadata {
    pub(crate) fn of<T: DdsType>() -> Self {
        Self {
            type_name: Cow::Borrowed(T::type_name()),
            type_identifier: T::type_identifier(),
            type_object: T::type_object(),
            keyed: T::has_key(),
            extensibility: T::extensibility(),
            key_holder_max_size: T::key_holder_max_size(),
            max_serialized_size: T::max_serialized_size(),
        }
    }
}

/// Shared topic identity with owned TypeObject and TypeIdentifier.
pub(crate) struct TopicInner {
    pub metadata: TopicTypeMetadata,
    /// Topic name (e.g. "ChatterTopic").
    pub name: String,
    /// Type name from `T::type_name()` (eager-copied so the inner has no
    /// generic type — this simplifies the participant registry data
    /// structure in .2a).
    pub type_name: Cow<'static, str>,
    /// Topic QoS — mutable via `Entity::set_qos`.
    #[cfg(feature = "std")]
    pub qos: std::sync::Mutex<TopicQos>,
    #[cfg(not(feature = "std"))]
    pub qos: TopicQos,
    /// Entity lifecycle (DCPS §2.2.2.1).
    pub entity_state: Arc<crate::entity::EntityState>,
    /// Optional `TopicListener` + StatusMask. Spec §2.2.2.3.2.x
    /// set_listener / bubble-up §2.2.4.2.3.
    #[cfg(feature = "std")]
    pub listener: std::sync::Mutex<Option<(ArcTopicListener, StatusMask)>>,
    /// Counter for InconsistentTopic detections (spec §2.2.4.2.5).
    /// Incremented in `record_inconsistent_topic`, read out in
    /// `inconsistent_topic_status` with delta detection.
    #[cfg(feature = "std")]
    pub inconsistent_topic_count: std::sync::atomic::AtomicI64,
    /// The total when the status was last read or handed to a listener;
    /// `total_count_change` counts from it (DDS 1.4 §2.2.4.1).
    #[cfg(feature = "std")]
    pub read_inconsistent_topic: std::sync::atomic::AtomicI64,
    /// What the runtime's listener dispatcher runs for this topic; set
    /// once, by the first handle that has a participant with a runtime.
    #[cfg(feature = "std")]
    pub driver: std::sync::OnceLock<Arc<TopicStatusDriver>>,
    /// How many times the application obtained the topic through
    /// `create_topic` and has not deleted it: DDS 1.4 §2.2.2.2.1.11 "If a
    /// Topic is obtained multiple times by means of find_topic or
    /// create_topic, it must also be deleted that same number of times
    /// using delete_topic." The last `delete_topic` deletes it.
    pub obtained: core::sync::atomic::AtomicU32,
    /// The dependency closure of the topic's `TypeObject`
    /// ([`TopicCodec::dependent_type_objects`]), recorded when the
    /// participant creates the topic: what a MultiTopic needs to read the
    /// members of the topic's type.
    pub type_dependencies: std::sync::OnceLock<Vec<zerodds_types::TypeObject>>,
}

impl TopicInner {
    /// A topic with no listener, nothing counted and no status driver yet.
    fn new(name: String, metadata: TopicTypeMetadata, qos: TopicQos) -> Self {
        Self {
            name,
            type_name: metadata.type_name.clone(),
            metadata,
            #[cfg(feature = "std")]
            qos: std::sync::Mutex::new(qos),
            #[cfg(not(feature = "std"))]
            qos,
            entity_state: crate::entity::EntityState::new(),
            #[cfg(feature = "std")]
            listener: std::sync::Mutex::new(None),
            #[cfg(feature = "std")]
            inconsistent_topic_count: std::sync::atomic::AtomicI64::new(0),
            #[cfg(feature = "std")]
            read_inconsistent_topic: std::sync::atomic::AtomicI64::new(0),
            #[cfg(feature = "std")]
            driver: std::sync::OnceLock::new(),
            obtained: core::sync::atomic::AtomicU32::new(1),
            type_dependencies: std::sync::OnceLock::new(),
        }
    }
}

impl core::fmt::Debug for TopicInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "std")]
        let listener_present = self.listener.lock().map(|s| s.is_some()).unwrap_or(false);
        #[cfg(not(feature = "std"))]
        let listener_present = false;
        f.debug_struct("TopicInner")
            .field("name", &self.name)
            .field("type_name", &self.type_name)
            .field("listener_present", &listener_present)
            .finish_non_exhaustive()
    }
}

impl<T: DdsType> Topic<T> {
    /// Whether the topic was created by another participant than `owner`
    /// (a publisher's or subscriber's). `false` where either is unknown: a
    /// builtin topic belongs to no participant.
    pub(crate) fn created_by_another_than(
        &self,
        owner: &std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    ) -> bool {
        let Some(mine) = self.participant.as_ref() else {
            return false;
        };
        let Some(owner) = owner
            .lock()
            .ok()
            .and_then(|o| o.as_ref().and_then(|w| w.upgrade()))
        else {
            return false;
        };
        !mine.is(&owner)
    }

    /// Creates a new topic handle. Normally called by
    /// `DomainParticipant::create_topic<T>(name, qos)`.
    #[must_use]
    pub fn new(name: String, qos: TopicQos, participant: DomainParticipant) -> Self {
        let metadata = TopicTypeMetadata::of::<T>();
        Self::with_static_metadata(name, qos, participant, metadata)
    }

    /// Retain the exact snapshot already validated by the participant registry.
    /// Only static DdsType creation calls this, not a foreign descriptor factory.
    pub(crate) fn with_static_metadata(
        name: String,
        qos: TopicQos,
        participant: DomainParticipant,
        metadata: TopicTypeMetadata,
    ) -> Self {
        let topic = Self {
            inner: Arc::new(TopicInner::new(name, metadata, qos)),
            participant: Some(participant),
            codec: Arc::new(StaticTopicCodec::of::<T>()),
            _t: PhantomData,
        };
        #[cfg(feature = "std")]
        topic.register_status_driver();
        topic
    }

    /// Hands this topic's INCONSISTENT_TOPIC status to the runtime's
    /// listener dispatcher, once per topic and only where a runtime
    /// exists. The driver holds the topic and the participant weakly.
    #[cfg(feature = "std")]
    fn register_status_driver(&self) {
        let Some(participant) = self.participant.as_ref() else {
            return;
        };
        let Some(runtime) = participant.runtime() else {
            return;
        };
        let mut created = false;
        let driver = self.inner.driver.get_or_init(|| {
            created = true;
            Arc::new(TopicStatusDriver {
                inner: Arc::downgrade(&self.inner),
                participant: participant.downgrade(),
            })
        });
        if created {
            runtime
                .listener_dispatcher()
                .register(Arc::downgrade(driver)
                    as alloc::sync::Weak<dyn crate::listener_dispatcher::StatusDriver>);
        }
    }

    /// Creates a topic handle **without** a participant — for builtin
    /// topics (DCPSParticipant/Topic/Publication/Subscription) that are
    /// created in the BuiltinSubscriber constructor before the
    /// DomainParticipant is ready (chicken-and-egg problem).
    /// `get_participant()` panics on an orphan topic — builtin readers
    /// avoid it.
    #[must_use]
    pub fn new_orphan(name: String, qos: TopicQos) -> Self {
        let metadata = TopicTypeMetadata::of::<T>();
        Self {
            inner: Arc::new(TopicInner::new(name, metadata, qos)),
            participant: None,
            codec: Arc::new(StaticTopicCodec::of::<T>()),
            _t: PhantomData,
        }
    }

    /// Topic name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Type name (from `T::type_name()`).
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.inner.type_name
    }

    /// Accessors may describe runtime types: reject changed identity rather
    /// than announcing contradictory endpoints under one shared topic.
    /// This does not authorize later mutation of the type's codecs.
    pub(crate) fn checked_type_metadata(&self) -> Result<&TopicTypeMetadata> {
        if self.codec.validates_static_metadata()
            && self.inner.metadata != TopicTypeMetadata::of::<T>()
        {
            return Err(DdsError::PreconditionNotMet {
                reason: "DdsType metadata changed after topic creation (name, identifier, object, keyedness, extensibility or key maximum)",
            });
        }
        Ok(&self.inner.metadata)
    }

    /// Sets the `TopicListener` + StatusMask. `None` clears the slot.
    /// Spec §2.2.2.3.2.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcTopicListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0)
                .then(|| alloc::sync::Arc::new(crate::listener::NilListener) as ArcTopicListener)
        });
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(
            self.participant
                .as_ref()
                .and_then(DomainParticipant::runtime),
        );
    }

    /// Current listener clone, if any.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcTopicListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
            .filter(|l| !l.is_nil())
    }

    /// Records an InconsistentTopic detection (e.g. when a remote topic
    /// with the same name but a different type name is discovered, or
    /// when `create_topic` is called with the same name but a different
    /// type). Spec §2.2.4.2.5.
    #[cfg(feature = "std")]
    pub fn record_inconsistent_topic(&self) {
        self.inner
            .inconsistent_topic_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    /// `InconsistentTopicStatus` as it stands, after the listener path
    /// had its look; does not reset the change field.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn inconsistent_topic_status(&self) -> crate::status::InconsistentTopicStatus {
        poll_inconsistent_topic(&self.inner, self.participant.as_ref());
        inconsistent_topic_record(&self.inner, self.participant.as_ref())
    }

    /// `get_inconsistent_topic_status` (DDS 1.4 §2.2.2.3.2, §2.2.4.1): the
    /// total and its change since the status was last read or handed to a
    /// listener. Reading resets the change field.
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    #[cfg(feature = "std")]
    pub fn get_inconsistent_topic_status(
        &self,
    ) -> crate::error::Result<crate::status::InconsistentTopicStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        poll_inconsistent_topic(&self.inner, self.participant.as_ref());
        let status = inconsistent_topic_record(&self.inner, self.participant.as_ref());
        self.inner.read_inconsistent_topic.store(
            i64::from(status.total_count),
            std::sync::atomic::Ordering::Release,
        );
        // Read before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
        // look that saw the status unread looks again after setting the
        // flag (`poll_ledger`) and finds it read.
        self.inner
            .entity_state
            .clear_status_changes(crate::psm_constants::status::INCONSISTENT_TOPIC);
        Ok(status)
    }

    /// Topic QoS (cloned). The inner mutex allows set_qos.
    #[must_use]
    pub fn qos(&self) -> TopicQos {
        #[cfg(feature = "std")]
        {
            self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.inner.qos.clone()
        }
    }

    /// `true` if the topic's type has a key. DDS 1.4 §2.2.1.2.2: "If no key
    /// is provided, the data set associated with the Topic is restricted to
    /// a single instance."
    pub(crate) fn is_keyed(&self) -> bool {
        self.inner.metadata.keyed
    }

    /// Internal access to the shared state (for Publisher/Subscriber).
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> Arc<TopicInner> {
        Arc::clone(&self.inner)
    }

    /// The participant that created this topic, or `None` for a builtin
    /// `Topic::new_orphan` — which no `create_topic` registered and which
    /// [`crate::DomainParticipant::delete_topic`] therefore refuses, the
    /// way [`TopicDescription::get_participant`] panics instead of
    /// answering.
    pub(crate) fn owning_participant(&self) -> Option<&DomainParticipant> {
        self.participant.as_ref()
    }

    /// Registers one more live user of this topic on the participant that
    /// owns it — the one whose `delete_topic` decides on the count (DDS 1.4
    /// §2.2.2.2.1.6). A `Publisher`/`Subscriber` of another participant may
    /// create an endpoint on this topic, and the count belongs where the
    /// topic is. Nothing to record for a builtin topic, which no participant
    /// owns.
    pub(crate) fn attach_user(&self, kind: TopicUserKind) {
        if let Some(owner) = self.participant.as_ref() {
            owner
                .inner()
                .add_typed_topic_user(&self.inner.name, &self.inner.type_name, kind);
        }
    }

    /// The counterpart of [`Self::attach_user`]: the `Drop` of a DataWriter,
    /// DataReader or ContentFilteredTopic.
    pub(crate) fn detach_user(&self, kind: TopicUserKind) {
        if let Some(owner) = self.participant.as_ref() {
            owner
                .inner()
                .remove_typed_topic_user(&self.inner.name, &self.inner.type_name, kind);
        }
    }

    /// Internal constructor for shared-handle rehydration in the
    /// participant topic registry (`create_topic` second round with the
    /// same name + type).
    pub(crate) fn _from_inner_impl(inner: Arc<TopicInner>, participant: DomainParticipant) -> Self {
        Self {
            inner,
            participant: Some(participant),
            codec: Arc::new(StaticTopicCodec::of::<T>()),
            _t: PhantomData,
        }
    }

    pub(crate) fn codec(&self) -> &dyn TopicCodec<T> {
        self.codec.as_ref()
    }

    /// The codec as a shared handle, for what outlives a borrow of the
    /// topic (a ReadCondition's trigger).
    pub(crate) fn codec_arc(&self) -> Arc<dyn TopicCodec<T>> {
        Arc::clone(&self.codec)
    }

    pub(crate) fn with_codec(mut self, codec: Arc<dyn TopicCodec<T>>) -> Self {
        self.codec = codec;
        self
    }
}

/// The bubble-up chain (Topic → Participant) of one topic.
#[cfg(feature = "std")]
fn topic_listener_chain(
    inner: &TopicInner,
    participant: Option<&DomainParticipant>,
) -> crate::listener_dispatch::TopicListenerChain {
    let topic = inner
        .listener
        .lock()
        .ok()
        .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
    let participant = participant.and_then(|p| p.snapshot_listener());
    crate::listener_dispatch::TopicListenerChain { topic, participant }
}

#[cfg(feature = "std")]
fn inconsistent_topic_total(inner: &TopicInner, participant: Option<&DomainParticipant>) -> i64 {
    // Two counters, one status. The `Topic`'s own counts what
    // `create_topic` and `record_inconsistent_topic` saw locally; the
    // runtime's counts what the two-step rule of XTypes 1.3 §7.6.3.4.2
    // rejected against a discovered peer, per topic name. They used to
    // be unconnected, so a type inconsistency found in discovery
    // raised no `on_inconsistent_topic` at all — and for a suite whose
    // whole question is type consistency that is the one status that
    // says why a pair produced nothing.
    let from_runtime = participant
        .and_then(|p| p.runtime())
        .map_or(0, |rt| rt.inconsistent_topic_count_for(&inner.name));
    let local = inner
        .inconsistent_topic_count
        .load(std::sync::atomic::Ordering::Acquire);
    local.saturating_add(i64::try_from(from_runtime).unwrap_or(i64::MAX))
}

/// INCONSISTENT_TOPIC as it stands: the total, and its change since the
/// status was last read or handed to a listener.
#[cfg(feature = "std")]
fn inconsistent_topic_record(
    inner: &TopicInner,
    participant: Option<&DomainParticipant>,
) -> crate::status::InconsistentTopicStatus {
    let total = inconsistent_topic_total(inner, participant);
    let read = inner
        .read_inconsistent_topic
        .load(std::sync::atomic::Ordering::Acquire);
    crate::status::InconsistentTopicStatus {
        total_count: i32::try_from(total).unwrap_or(i32::MAX),
        total_count_change: i32::try_from(total - read).unwrap_or(i32::MAX),
    }
}

/// Calls `on_inconsistent_topic` while the status has a change nobody
/// read: the total grew since it was last read or handed to a listener.
/// A listener that took the status reads it: its change counts from
/// there. With no listener the change stays, for the getter or for a
/// listener set later.
#[cfg(feature = "std")]
fn poll_inconsistent_topic(inner: &TopicInner, participant: Option<&DomainParticipant>) {
    let status = inconsistent_topic_record(inner, participant);
    let total = i64::from(status.total_count);
    if status.total_count_change == 0 {
        return;
    }
    let bit = crate::psm_constants::status::INCONSISTENT_TOPIC;
    inner.entity_state.set_status_bits(bit);
    let chain = topic_listener_chain(inner, participant);
    if crate::listener_dispatch::dispatch_inconsistent_topic(
        &chain,
        inner.entity_state.instance_handle(),
        status,
    ) {
        inner
            .read_inconsistent_topic
            .store(total, std::sync::atomic::Ordering::Release);
        inner.entity_state.clear_status_changes(bit);
    }
}

/// What the runtime's listener dispatcher runs for a topic. Weak on both
/// sides: the topic owns it, and it must not keep the topic or its
/// participant alive.
#[cfg(feature = "std")]
pub(crate) struct TopicStatusDriver {
    inner: alloc::sync::Weak<TopicInner>,
    participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
}

#[cfg(feature = "std")]
impl crate::listener_dispatcher::StatusDriver for TopicStatusDriver {
    fn drive(&self) {
        let (Some(inner), Some(participant)) = (self.inner.upgrade(), self.participant.upgrade())
        else {
            return;
        };
        let participant = DomainParticipant::from_inner(participant);
        poll_inconsistent_topic(&inner, Some(&participant));
    }
}

impl<T: DdsType> Clone for Topic<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            participant: self.participant.clone(),
            codec: Arc::clone(&self.codec),
            _t: PhantomData,
        }
    }
}

impl<T: DdsType> TopicDescription for Topic<T> {
    fn get_type_name(&self) -> &str {
        &self.inner.type_name
    }
    fn get_name(&self) -> &str {
        &self.inner.name
    }
    /// Returns the owning participant. Panics on builtin (orphan) topics
    /// constructed via [`Topic::new_orphan`] — builtin readers do not
    /// call `get_participant()`.
    #[allow(clippy::expect_used, clippy::panic)]
    fn get_participant(&self) -> &DomainParticipant {
        match &self.participant {
            Some(p) => p,
            None => panic!(
                "get_participant on orphan (builtin) topic — builtin readers must not call this"
            ),
        }
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for Topic<T> {
    type Qos = TopicQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// TopicQos. DDS 1.4 §2.2.3: TOPIC_DATA, DEADLINE, LATENCY_BUDGET,
    /// TRANSPORT_PRIORITY and LIFESPAN are Changeable=YES; DURABILITY,
    /// DURABILITY_SERVICE, LIVELINESS, RELIABILITY, DESTINATION_ORDER,
    /// HISTORY, RESOURCE_LIMITS, OWNERSHIP and DATA_REPRESENTATION (XTypes
    /// 1.3 §7.6.3.1.1) are not, once the topic is enabled. A new TOPIC_DATA reaches every DataWriter and DataReader of
    /// the topic that does not set its own, in its SEDP announcement
    /// (§2.2.3.2). The other changeable ones are defaults a DataWriter or
    /// DataReader copies at creation (`copy_from_topic_qos`) and do not
    /// reach an existing one.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos.into().try_resolve(|| {
            self.participant
                .as_ref()
                .map(DomainParticipant::get_default_topic_qos)
                .ok_or(DdsError::PreconditionNotMet {
                    reason: "a built-in topic has no participant default",
                })
        })?;
        qos.check_consistency()?;
        let enabled = self.inner.entity_state.is_enabled();
        let Ok(mut current) = self.inner.qos.lock() else {
            return Ok(());
        };
        if enabled {
            let immutable = [
                ("DURABILITY", current.durability != qos.durability),
                (
                    "DURABILITY_SERVICE",
                    current.durability_service != qos.durability_service,
                ),
                ("LIVELINESS", current.liveliness != qos.liveliness),
                ("RELIABILITY", current.reliability != qos.reliability),
                (
                    "DESTINATION_ORDER",
                    current.destination_order != qos.destination_order,
                ),
                ("HISTORY", current.history != qos.history),
                (
                    "RESOURCE_LIMITS",
                    current.resource_limits != qos.resource_limits,
                ),
                ("OWNERSHIP", current.ownership != qos.ownership),
                (
                    "DATA_REPRESENTATION",
                    current.data_representation != qos.data_representation,
                ),
            ];
            if let Some((policy, _)) = immutable.iter().find(|(_, changed)| *changed) {
                return Err(crate::entity::immutable_if_enabled(policy));
            }
        }
        if current.topic_data != qos.topic_data {
            if let Some(rt) = self.participant.as_ref().and_then(|p| p.runtime()) {
                rt.update_user_topic_data(&self.inner.name, &qos.topic_data.value)?;
            }
        }
        let changed = *current != qos;
        *current = qos;
        drop(current);
        // DDSI-RTPS 2.5 Table 8.84: "each time the QoS of an existing
        // Topic is modified" — announced again, under its GUID.
        if enabled && changed {
            if let Some(rt) = self.participant.as_ref().and_then(|p| p.runtime()) {
                crate::participant::announce_topic(rt, &self.inner)?;
            }
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7: refused while the participant is disabled.
    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable_entity()
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

/// What a DataReader on a [`ContentFilteredTopic`] needs when it registers
/// with the runtime, which may be at its `enable` long after its creation:
/// the filter property with the parameters current then, and the list of
/// readers a later `set_filter_parameters` announces again.
#[cfg(feature = "std")]
#[derive(Clone)]
pub(crate) struct CftLink {
    name: String,
    related_topic_name: String,
    filter_class_name: String,
    filter_expression: String,
    params: Arc<RwLock<FilterParams>>,
    readers: Arc<std::sync::Mutex<Vec<zerodds_rtps::wire_types::EntityId>>>,
}

#[cfg(feature = "std")]
impl CftLink {
    /// `PID_CONTENT_FILTER_PROPERTY` with the parameters as they are now.
    pub(crate) fn property(&self) -> zerodds_rtps::subscription_data::ContentFilterProperty {
        zerodds_rtps::subscription_data::ContentFilterProperty {
            content_filtered_topic_name: self.name.clone(),
            related_topic_name: self.related_topic_name.clone(),
            filter_class_name: self.filter_class_name.clone(),
            filter_expression: self.filter_expression.clone(),
            expression_parameters: self
                .params
                .read()
                .map(|p| p.raw.clone())
                .unwrap_or_default(),
        }
    }

    /// Records a registered DataReader of the topic.
    pub(crate) fn attach(&self, eid: zerodds_rtps::wire_types::EntityId) {
        if let Ok(mut readers) = self.readers.lock() {
            readers.push(eid);
        }
    }
}

/// What kind of TopicDescription a [`TopicDescriptionHandle`] names (DDS 1.4
/// §2.2.2.3: Topic, ContentFilteredTopic and MultiTopic specialize
/// TopicDescription).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicDescriptionKind {
    /// A [`Topic`].
    Topic,
    /// A [`ContentFilteredTopic`].
    ContentFilteredTopic,
    /// A [`MultiTopic`].
    MultiTopic,
}

/// Builds one more handle on a ContentFilteredTopic or MultiTopic that lives
/// in a participant's registry; `None` if it is deleted or gone. The handle
/// is a `Box<ContentFilteredTopic<T>>`-like value behind `dyn Any`.
#[cfg(feature = "std")]
#[derive(Clone)]
pub(crate) struct Reviver(
    pub(crate) Arc<dyn Fn() -> Option<Box<dyn core::any::Any + Send>> + Send + Sync>,
);

#[cfg(feature = "std")]
impl core::fmt::Debug for Reviver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Reviver")
    }
}

/// A ContentFilteredTopic or MultiTopic in the registry of its participant:
/// what `lookup_topicdescription` finds (DDS 1.4 §2.2.2.2.1.12). It holds
/// the description weakly, so it neither keeps it alive nor counts as a user
/// of its related topics; it is gone with the last handle or once deleted.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct DescriptionEntry {
    pub(crate) name: String,
    pub(crate) type_name: String,
    pub(crate) kind: TopicDescriptionKind,
    life: alloc::sync::Weak<DescriptionLife>,
    revive: Reviver,
    /// Deletes the description, as `delete_contentfilteredtopic` or
    /// `delete_multitopic` does (`delete_contained_entities`).
    delete: DeleteDescription,
}

/// See [`DescriptionEntry::delete`].
#[cfg(feature = "std")]
#[derive(Clone)]
pub(crate) struct DeleteDescription(pub(crate) Arc<dyn Fn() -> Result<()> + Send + Sync>);

#[cfg(feature = "std")]
impl core::fmt::Debug for DeleteDescription {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DeleteDescription")
    }
}

#[cfg(feature = "std")]
impl DescriptionEntry {
    /// `true` while a handle exists and the description is not deleted.
    pub(crate) fn is_live(&self) -> bool {
        self.life.upgrade().is_some_and(|life| !life.is_deleted())
    }

    /// `true` if `held` is a DataReader's hold on this description.
    pub(crate) fn is_held_by(&self, held: &DescriptionReaderUse) -> bool {
        self.life
            .upgrade()
            .is_some_and(|life| Arc::ptr_eq(&life, &held.life))
    }

    /// The deletion as a value, to call outside the registry lock.
    pub(crate) fn delete_handle(&self) -> DeleteDescription {
        self.delete.clone()
    }

    /// The handle the application gets from the lookup.
    pub(crate) fn handle(&self, participant: DomainParticipant) -> TopicDescriptionHandle {
        TopicDescriptionHandle {
            name: self.name.clone(),
            type_name: self.type_name.clone(),
            participant,
            kind: self.kind,
            remote: false,
            revive: Some(self.revive.clone()),
        }
    }
}

/// Untyped `TopicDescription` handle returned from
/// `DomainParticipant::lookup_topicdescription` /
/// `DomainParticipant::find_topic`.
///
/// The spec API returns the abstract base class because the caller
/// generally does not yet know the type (e.g. after discovery). We pack
/// name + type name + participant into a cloneable handle; the caller
/// reads `kind()` and `get_type_name()` and gets the typed object with
/// [`Self::topic`], [`Self::content_filtered_topic`] or
/// [`Self::multitopic`], from which a DataWriter or DataReader is created.
#[derive(Debug, Clone)]
pub struct TopicDescriptionHandle {
    name: String,
    type_name: String,
    participant: DomainParticipant,
    kind: TopicDescriptionKind,
    /// Announced by a remote endpoint and not created here (`find_topic`).
    remote: bool,
    #[cfg(feature = "std")]
    revive: Option<Reviver>,
}

impl TopicDescriptionHandle {
    /// Constructs a handle on a Topic (internal; mainly for tests + the
    /// lookup API).
    pub(crate) fn new(name: String, type_name: String, participant: DomainParticipant) -> Self {
        Self {
            name,
            type_name,
            participant,
            kind: TopicDescriptionKind::Topic,
            remote: false,
            #[cfg(feature = "std")]
            revive: None,
        }
    }

    /// The handle of a topic that a remote endpoint announced and this
    /// participant does not have.
    pub(crate) fn remote(name: String, type_name: String, participant: DomainParticipant) -> Self {
        Self {
            remote: true,
            ..Self::new(name, type_name, participant)
        }
    }

    /// The `InstanceHandle` of the Topic the handle names (DDS 1.4
    /// §2.2.2.1.1.9 `get_instance_handle`); `None` for a ContentFilteredTopic
    /// or a MultiTopic, and for a topic this participant does not have.
    #[must_use]
    pub fn instance_handle(&self) -> Option<crate::instance_handle::InstanceHandle> {
        if self.kind != TopicDescriptionKind::Topic {
            return None;
        }
        self.participant
            .registered_topic(&self.name, &self.type_name)
            .map(|topic| topic.entity_state.instance_handle())
    }

    /// What the handle names.
    #[must_use]
    pub fn kind(&self) -> TopicDescriptionKind {
        self.kind
    }

    /// The [`Topic`] of type `T` the handle names, to create DataWriters and
    /// DataReaders from (DDS 1.4 §2.2.2.2.1.11, §2.2.2.2.1.12). A topic that
    /// only a remote endpoint announced is created here with the
    /// participant's default Topic QoS ("middleware implementations could
    /// choose to propagate topics and make remotely created topics locally
    /// available").
    ///
    /// # Errors
    /// `BadParameter` if the handle names a ContentFilteredTopic or a
    /// MultiTopic, or the type name of `T` is not the handle's;
    /// `AlreadyDeleted` if the topic is gone; any error of `create_topic`
    /// for a remote topic.
    #[cfg(feature = "std")]
    pub fn topic<T: DdsType>(&self) -> Result<Topic<T>> {
        if self.kind != TopicDescriptionKind::Topic {
            return Err(DdsError::BadParameter {
                what: "the handle names a ContentFilteredTopic or a MultiTopic, not a Topic",
            });
        }
        if T::type_name() != self.type_name {
            return Err(DdsError::BadParameter {
                what: "the type name of T is not the type name of the topic",
            });
        }
        if self.remote {
            return self
                .participant
                .create_topic::<T>(&self.name, self.participant.get_default_topic_qos());
        }
        let inner = self
            .participant
            .registered_topic(&self.name, &self.type_name)
            .ok_or(DdsError::AlreadyDeleted)?;
        Ok(Topic::<T>::from_inner(inner, self.participant.clone()))
    }

    /// The [`ContentFilteredTopic`] of type `T` the handle names: one more
    /// handle on it, which counts as a user of the related topic until it is
    /// dropped.
    ///
    /// # Errors
    /// `BadParameter` if the handle names another kind or `T` is not its
    /// type; `AlreadyDeleted` if it is deleted or gone.
    #[cfg(feature = "std")]
    pub fn content_filtered_topic<T: DdsType + 'static>(&self) -> Result<ContentFilteredTopic<T>> {
        self.revived(TopicDescriptionKind::ContentFilteredTopic)
    }

    /// The [`MultiTopic`] of type `T` the handle names; as
    /// [`Self::content_filtered_topic`].
    ///
    /// # Errors
    /// As [`Self::content_filtered_topic`].
    #[cfg(feature = "std")]
    pub fn multitopic<T: DdsType + 'static>(&self) -> Result<MultiTopic<T>> {
        self.revived(TopicDescriptionKind::MultiTopic)
    }

    #[cfg(feature = "std")]
    fn revived<D: 'static>(&self, kind: TopicDescriptionKind) -> Result<D> {
        if self.kind != kind {
            return Err(DdsError::BadParameter {
                what: "the handle names another kind of TopicDescription",
            });
        }
        let Some(revive) = self.revive.as_ref() else {
            return Err(DdsError::AlreadyDeleted);
        };
        let Some(object) = (revive.0)() else {
            return Err(DdsError::AlreadyDeleted);
        };
        object
            .downcast::<D>()
            .map(|boxed| *boxed)
            .map_err(|_| DdsError::BadParameter {
                what: "the TopicDescription is of another type than T",
            })
    }
}

impl TopicDescription for TopicDescriptionHandle {
    fn get_type_name(&self) -> &str {
        &self.type_name
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

/// `ContentFilteredTopic<T>` — a sub-topic of a `Topic<T>` with a filter
/// expression. Spec reference: OMG DDS 1.4 §2.2.2.3.3.
///
/// "ContentFilteredTopic is a specialization of TopicDescription that
/// allows for content-based subscriptions. The selection of the
/// content is done using a filter_expression with parameters
/// (filter_parameters)."
///
/// The filter expression is a SQL subset (DDS-DCPS Annex B, BNF):
/// `field op literal-or-param`, `AND`/`OR`/`NOT` composition, parens,
/// `LIKE`. The concrete parser lives in `zerodds-sql-filter`.
///
/// **Lifecycle:** the CFT holds a clone of the related topic (shared
/// `Arc<TopicInner>`) and is itself cloneable. Filter parameters are
/// mutable at runtime (spec §2.2.2.3.3.7).
#[derive(Debug)]
pub struct ContentFilteredTopic<T: DdsType> {
    name: String,
    related_topic: Topic<T>,
    /// Raw form of the expression (read-only — the spec is explicit that
    /// only the `filter_parameters` are mutable, not the expression
    /// itself).
    filter_expression: String,
    /// The filter: the pre-parsed `DDSSQL` AST (parsed once in the
    /// constructor so that `evaluate` is on the hot path), or the filter
    /// class an application registered.
    filter: FilterImpl<T>,
    /// Filter parameters (`%0`, `%1`, ...). Passed as strings (spec
    /// §2.2.2.3.3 — the parameters are strings substituted into the
    /// expression). We keep them as strings + parsed `Value` in parallel
    /// so `set_filter_parameters` does the String→Value conversion only
    /// once.
    #[cfg(feature = "std")]
    params: Arc<RwLock<FilterParams>>,
    #[cfg(not(feature = "std"))]
    params: FilterParams,
    participant: DomainParticipant,
    /// The live DataReaders created on this ContentFilteredTopic, whose SEDP
    /// subscriptions carry the filter and are re-announced when the
    /// parameters change.
    #[cfg(feature = "std")]
    readers: Arc<std::sync::Mutex<Vec<zerodds_rtps::wire_types::EntityId>>>,
    /// Deleted flag and reader count, shared by the handles.
    life: Arc<DescriptionLife>,
    /// See [`Topic`]: no `T` is held.
    _t: PhantomData<fn() -> T>,
}

/// A content filter class the application registers
/// ([`DomainParticipant::register_contentfilter`]) and names in
/// [`DomainParticipant::create_contentfilteredtopic_with_class`].
///
/// DDSI-RTPS 2.5 §8.7.3.4: an implementation of content-filtered topics
/// recognizes the `DDSSQL` class, and "for all other filter classes, both
/// implementations must allow the user to register the same custom filter
/// class". The class name travels in the reader's
/// `PID_CONTENT_FILTER_PROPERTY`; a peer that has not registered the class
/// ignores the filter (§8.7.3.4) and the samples are filtered on the reader
/// side.
pub trait ContentFilter<T>: Send + Sync {
    /// Checks an expression with its parameters, when a
    /// ContentFilteredTopic is created and when its parameters are set.
    ///
    /// # Errors
    /// `BadParameter` if the expression or a parameter is not valid for
    /// this class.
    fn compile(&self, expression: &str, parameters: &[String]) -> Result<()>;

    /// Whether `sample` passes the filter with the parameters current now.
    fn evaluate(&self, sample: &T, expression: &str, parameters: &[String]) -> bool;
}

/// How a ContentFilteredTopic filters.
pub(crate) enum FilterImpl<T> {
    /// `DDSSQL`: the parsed expression.
    Sql(Arc<Expr>),
    /// A registered filter class.
    Custom {
        class: String,
        filter: Arc<dyn ContentFilter<T>>,
    },
}

impl<T> core::fmt::Debug for FilterImpl<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FilterImpl")
            .field("class", &self.class_name())
            .finish()
    }
}

impl<T> Clone for FilterImpl<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Sql(e) => Self::Sql(Arc::clone(e)),
            Self::Custom { class, filter } => Self::Custom {
                class: class.clone(),
                filter: Arc::clone(filter),
            },
        }
    }
}

impl<T> FilterImpl<T> {
    /// The filter class name of `PID_CONTENT_FILTER_PROPERTY`.
    fn class_name(&self) -> &str {
        match self {
            Self::Sql(_) => zerodds_rtps::subscription_data::filter_class::DDSSQL,
            Self::Custom { class, .. } => class,
        }
    }
}

/// A ContentFilteredTopic's expression with its live parameters.
#[cfg(feature = "std")]
pub(crate) struct CftEvaluator {
    parsed: Arc<Expr>,
    params: Arc<RwLock<FilterParams>>,
}

#[cfg(feature = "std")]
impl CftEvaluator {
    /// Whether `row` passes; one the expression cannot be evaluated against
    /// does not (DDS 1.4 §2.2.2.3.3).
    pub(crate) fn accepts<R: RowAccess>(&self, row: &R) -> bool {
        let Ok(params) = self.params.read() else {
            return false;
        };
        self.parsed.evaluate(row, &params.values).unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
struct FilterParams {
    raw: Vec<String>,
    values: Vec<Value>,
}

impl<T: DdsType> ContentFilteredTopic<T> {
    /// Constructor (called internally by
    /// `DomainParticipant::create_contentfilteredtopic`).
    ///
    /// # Errors
    /// `BadParameter` if the expression does not parse or a referenced
    /// `%N` index does not exist in the `filter_parameters` vec.
    pub(crate) fn new(
        name: String,
        related_topic: Topic<T>,
        filter_expression: String,
        filter_parameters: Vec<String>,
        participant: DomainParticipant,
        custom: Option<(String, Arc<dyn ContentFilter<T>>)>,
    ) -> Result<Self> {
        let filter = match custom {
            // A registered class checks its own expression syntax.
            Some((class, filter)) => {
                filter.compile(&filter_expression, &filter_parameters)?;
                FilterImpl::Custom { class, filter }
            }
            None => {
                let parsed = zerodds_sql_filter::parse(&filter_expression).map_err(|_| {
                    DdsError::BadParameter {
                        what: "filter expression syntax",
                    }
                })?;
                // Check parameter indices.
                let used = parsed.collect_param_indices();
                if let Some(max) = used.iter().max() {
                    if (*max as usize) >= filter_parameters.len() {
                        return Err(DdsError::BadParameter {
                            what: "filter parameter %N out of range",
                        });
                    }
                }
                FilterImpl::Sql(Arc::new(parsed))
            }
        };
        let values: Vec<Value> = filter_parameters
            .iter()
            .map(|s| param_string_to_value(s))
            .collect();
        let fp = FilterParams {
            raw: filter_parameters,
            values,
        };
        // A ContentFilteredTopic is one of the four object kinds that hold
        // its related Topic (DDS 1.4 §2.2.2.2.1.6), so from here the topic
        // cannot be deleted. `Drop` gives the count back.
        related_topic.attach_user(TopicUserKind::ContentFilteredTopic);
        Ok(Self {
            name,
            related_topic,
            filter_expression,
            filter,
            #[cfg(feature = "std")]
            params: Arc::new(RwLock::new(fp)),
            #[cfg(not(feature = "std"))]
            params: fp,
            participant,
            #[cfg(feature = "std")]
            readers: Arc::new(std::sync::Mutex::new(Vec::new())),
            life: Arc::new(DescriptionLife::new()),
            _t: PhantomData,
        })
    }

    /// The filter as a reader announces it over SEDP (DDSI-RTPS 2.5
    /// §9.6.3.4): this topic's name, the related topic's, the `DDSSQL`
    /// class, the expression and the current parameters.
    #[must_use]
    pub fn content_filter_property(
        &self,
    ) -> zerodds_rtps::subscription_data::ContentFilterProperty {
        zerodds_rtps::subscription_data::ContentFilterProperty {
            content_filtered_topic_name: self.name.clone(),
            related_topic_name: self.related_topic.name().into(),
            filter_class_name: self.filter.class_name().into(),
            filter_expression: self.filter_expression.clone(),
            expression_parameters: self.get_filter_parameters(),
        }
    }

    /// The filter of a DataReader created on this topic: parameters set
    /// later reach it. `DDSSQL` evaluates the expression against the sample's
    /// fields; a registered class evaluates through its [`ContentFilter`].
    #[cfg(feature = "std")]
    pub(crate) fn reader_filter(&self) -> Arc<dyn Fn(&T) -> bool + Send + Sync>
    where
        T: 'static,
    {
        match &self.filter {
            FilterImpl::Sql(parsed) => {
                let evaluator = CftEvaluator {
                    parsed: Arc::clone(parsed),
                    params: Arc::clone(&self.params),
                };
                Arc::new(move |sample: &T| {
                    evaluator.accepts(&crate::dds_type::DdsTypeRow::new(sample))
                })
            }
            FilterImpl::Custom { filter, .. } => {
                let filter = Arc::clone(filter);
                let params = Arc::clone(&self.params);
                let expression = self.filter_expression.clone();
                Arc::new(move |sample: &T| {
                    let Ok(params) = params.read() else {
                        return false;
                    };
                    filter.evaluate(sample, &expression, &params.raw)
                })
            }
        }
    }

    /// What a DataReader on this topic needs at its registration
    /// ([`CftLink`]).
    #[cfg(feature = "std")]
    pub(crate) fn link(&self) -> CftLink {
        CftLink {
            name: self.name.clone(),
            related_topic_name: self.related_topic.name().into(),
            filter_class_name: self.filter.class_name().into(),
            filter_expression: self.filter_expression.clone(),
            params: Arc::clone(&self.params),
            readers: Arc::clone(&self.readers),
        }
    }

    /// DDS 1.4 §2.2.2.2.1.8: deletes this ContentFilteredTopic for every
    /// handle of it; its uses of the related topic end here.
    ///
    /// # Errors
    /// `AlreadyDeleted` if it is deleted already; `PreconditionNotMet`
    /// while a DataReader created on it exists.
    pub(crate) fn delete_description(&self) -> Result<()> {
        let handles = self.life.delete()?;
        for _ in 0..handles {
            self.related_topic
                .detach_user(TopicUserKind::ContentFilteredTopic);
        }
        Ok(())
    }

    /// This ContentFilteredTopic as the participant registry keeps it.
    #[cfg(feature = "std")]
    pub(crate) fn description_entry(&self) -> DescriptionEntry
    where
        T: 'static,
    {
        let name = self.name.clone();
        let related = self.related_topic.inner();
        let codec = self.related_topic.codec_arc();
        let expression = self.filter_expression.clone();
        let filter = self.filter.clone();
        let params = Arc::clone(&self.params);
        let readers = Arc::clone(&self.readers);
        let participant = Arc::downgrade(self.participant.inner());
        let life = Arc::downgrade(&self.life);
        let revive_name = name.clone();
        let revive: Arc<dyn Fn() -> Option<Box<dyn core::any::Any + Send>> + Send + Sync> =
            Arc::new(move || {
                let life = life.upgrade()?;
                if life.is_deleted() {
                    return None;
                }
                let participant = DomainParticipant::from_inner(participant.upgrade()?);
                let related_topic =
                    Topic::<T>::from_inner(Arc::clone(&related), participant.clone())
                        .with_codec(Arc::clone(&codec));
                if life.add_handle() {
                    related_topic.attach_user(TopicUserKind::ContentFilteredTopic);
                }
                Some(Box::new(ContentFilteredTopic::<T> {
                    name: revive_name.clone(),
                    related_topic,
                    filter_expression: expression.clone(),
                    filter: filter.clone(),
                    params: Arc::clone(&params),
                    participant,
                    readers: Arc::clone(&readers),
                    life,
                    _t: PhantomData,
                }))
            });
        let reviver = Arc::clone(&revive);
        let delete: Arc<dyn Fn() -> Result<()> + Send + Sync> = Arc::new(move || {
            let Some(object) = reviver() else {
                return Ok(());
            };
            match object.downcast::<ContentFilteredTopic<T>>() {
                Ok(cft) => cft.delete_description(),
                Err(_) => Ok(()),
            }
        });
        DescriptionEntry {
            name,
            type_name: self.related_topic.type_name().into(),
            kind: TopicDescriptionKind::ContentFilteredTopic,
            life: Arc::downgrade(&self.life),
            revive: Reviver(revive),
            delete: DeleteDescription(delete),
        }
    }

    /// Records a DataReader created on this topic (DDS 1.4 §2.2.2.2.1.8).
    pub(crate) fn attach_reader(&self) -> Result<DescriptionReaderUse> {
        self.life.attach_reader()
    }

    /// Spec §2.2.2.3.3.4 `get_filter_expression`.
    #[must_use]
    pub fn get_filter_expression(&self) -> &str {
        &self.filter_expression
    }

    /// Spec §2.2.2.3.3.5 `get_expression_parameters` (the IDL name; the
    /// operation list of §2.2.2.3.3 names it so).
    #[must_use]
    pub fn get_expression_parameters(&self) -> Vec<String> {
        self.get_filter_parameters()
    }

    /// Spec §2.2.2.3.3.5 `get_filter_parameters`.
    #[must_use]
    pub fn get_filter_parameters(&self) -> Vec<String> {
        #[cfg(feature = "std")]
        {
            self.params
                .read()
                .map(|p| p.raw.clone())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.params.raw.clone()
        }
    }

    /// Spec §2.2.2.3.3.6 `set_filter_parameters`. Swaps the parameters
    /// (equal count is not enforced — the spec says "should match the
    /// number of `%n` tokens" as a recommendation; we verify it
    /// strictly).
    ///
    /// # Errors
    /// `BadParameter` if a `%N` index is outside the new vec;
    /// `AlreadyDeleted` if the topic is deleted.
    pub fn set_filter_parameters(&self, params: Vec<String>) -> Result<()> {
        self.life.check_not_deleted()?;
        match &self.filter {
            FilterImpl::Sql(parsed) => {
                let used = parsed.collect_param_indices();
                if let Some(max) = used.iter().max() {
                    if (*max as usize) >= params.len() {
                        return Err(DdsError::BadParameter {
                            what: "filter parameter %N out of range",
                        });
                    }
                }
            }
            FilterImpl::Custom { filter, .. } => {
                filter.compile(&self.filter_expression, &params)?;
            }
        }
        let values: Vec<Value> = params.iter().map(|s| param_string_to_value(s)).collect();
        let fp = FilterParams {
            raw: params,
            values,
        };
        #[cfg(feature = "std")]
        {
            let mut w = self
                .params
                .write()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "filter params poisoned",
                })?;
            *w = fp;
        }
        // Every live reader announces the new parameters. A reader that was
        // deleted since has no announcement left and is dropped from the
        // list.
        #[cfg(feature = "std")]
        if let Some(rt) = self.participant.runtime() {
            let property = self.content_filter_property();
            if let Ok(mut readers) = self.readers.lock() {
                readers.retain(|eid| {
                    rt.update_user_reader_content_filter(*eid, Some(property.clone()))
                        .is_ok()
                });
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // Not kept mutable in the no_std path — parameters are set in
            // the constructor; this path is not active in the v1.2 MVP
            // anyway (dcps requires std).
            let _ = fp;
            return Err(DdsError::PreconditionNotMet {
                reason: "set_filter_parameters needs std feature",
            });
        }
        Ok(())
    }

    /// Spec §2.2.2.3.3.3 `get_related_topic`.
    #[must_use]
    pub fn get_related_topic(&self) -> &Topic<T> {
        &self.related_topic
    }

    /// Evaluates the filter against a decoded sample. Returns
    /// `Ok(true)` if the sample should pass, `Ok(false)` if it is
    /// filtered out, `Err` if the expression does not fit the row schema
    /// (caller's decision: filter denies or error).
    ///
    /// # Errors
    /// - `PreconditionNotMet` if the filter-parameter lock is poisoned.
    /// - `BadParameter` if a field in the expression does not exist in
    ///   the row or a type mismatch occurs.
    pub fn evaluate<R: RowAccess>(&self, row: &R) -> Result<bool> {
        #[cfg(feature = "std")]
        let params = {
            let r = self
                .params
                .read()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "filter params poisoned",
                })?;
            r.values.clone()
        };
        #[cfg(not(feature = "std"))]
        let params = self.params.values.clone();
        let FilterImpl::Sql(parsed) = &self.filter else {
            return Err(DdsError::BadParameter {
                what: "evaluate is for the DDSSQL filter class; a registered class evaluates samples",
            });
        };
        parsed
            .evaluate(row, &params)
            .map_err(|e| DdsError::BadParameter {
                what: match e {
                    zerodds_sql_filter::EvalError::UnknownField(_) => "filter unknown field",
                    zerodds_sql_filter::EvalError::MissingParam(_) => "filter missing param",
                    zerodds_sql_filter::EvalError::TypeMismatch(_) => "filter type mismatch",
                },
            })
    }
}

impl<T: DdsType> Clone for ContentFilteredTopic<T> {
    fn clone(&self) -> Self {
        // A clone is one more handle on the same ContentFilteredTopic, and
        // for as long as it is a handle it is a user of the related topic
        // (DDS 1.4 §2.2.2.2.1.6); `Drop` takes it back again.
        if self.life.add_handle() {
            self.related_topic
                .attach_user(TopicUserKind::ContentFilteredTopic);
        }
        Self {
            name: self.name.clone(),
            related_topic: self.related_topic.clone(),
            filter_expression: self.filter_expression.clone(),
            filter: self.filter.clone(),
            #[cfg(feature = "std")]
            params: Arc::clone(&self.params),
            #[cfg(not(feature = "std"))]
            params: self.params.clone(),
            participant: self.participant.clone(),
            #[cfg(feature = "std")]
            readers: Arc::clone(&self.readers),
            life: Arc::clone(&self.life),
            _t: PhantomData,
        }
    }
}

/// The ContentFilteredTopic is gone with its last handle, so it stops using
/// its related topic (DDS 1.4 §2.2.2.2.1.6 counts *existing*
/// ContentFilteredTopics). Every handle added itself in `new`/`clone` and
/// removes itself here.
impl<T: DdsType> Drop for ContentFilteredTopic<T> {
    fn drop(&mut self) {
        // A deleted ContentFilteredTopic gave its uses back at `delete`.
        if self.life.drop_handle() {
            self.related_topic
                .detach_user(TopicUserKind::ContentFilteredTopic);
        }
    }
}

impl<T: DdsType> TopicDescription for ContentFilteredTopic<T> {
    /// The type name comes from the related topic — a CFT shares the
    /// schema with the underlying topic.
    fn get_type_name(&self) -> &str {
        self.related_topic.type_name()
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

/// `MultiTopic<T>` — spec §2.2.2.3.4 (DDS 1.4 optional feature).
///
/// A MultiTopic combines the topics of the `FROM` clause of its
/// `subscription_expression` (Annex B.2: `SELECT … FROM … NATURAL JOIN … WHERE
/// …`) into the type named by `type_name`. A DataReader on it
/// ([`crate::Subscriber::create_datareader_with_multitopic`]) reads the
/// topics, joins their instances, keeps the fields of the `SELECT` list,
/// applies `WHERE` and delivers samples of the result type; the model is
/// described in `crate::multitopic`.
///
/// The result type needs a `TypeObject` ([`DdsType::type_object`]), or a
/// type registered under `type_name`
/// ([`crate::serialized_type::SerializedTypeSupport::register_type`]); so do
/// the topics of the `FROM` clause, since their members are named by the
/// expression.
pub struct MultiTopic<T: DdsType> {
    name: String,
    type_name: String,
    /// The topics of the `FROM` clause.
    related_topic_names: Vec<String>,
    subscription_expression: String,
    /// The `WHERE` condition, parsed.
    parsed: Option<Arc<Expr>>,
    /// Mutable filter parameters (spec §2.2.2.3.4.7
    /// `set_expression_parameters`).
    #[cfg(feature = "std")]
    params: Arc<RwLock<FilterParams>>,
    #[cfg(not(feature = "std"))]
    params: FilterParams,
    participant: DomainParticipant,
    /// Deleted flag and reader count, shared by the handles.
    life: Arc<DescriptionLife>,
    /// What the expression computes over the topics.
    #[cfg(feature = "std")]
    plan: Arc<crate::multitopic::MultiTopicPlan>,
    /// The topic description a reader of the MultiTopic is built on: the
    /// result type under the MultiTopic's name. It is not announced.
    #[cfg(feature = "std")]
    result_topic: Topic<T>,
    /// See [`Topic`]: no `T` is held.
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType> core::fmt::Debug for MultiTopic<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MultiTopic")
            .field("name", &self.name)
            .field("type_name", &self.type_name)
            .field("related_topic_names", &self.related_topic_names)
            .field("subscription_expression", &self.subscription_expression)
            .finish_non_exhaustive()
    }
}

impl<T: DdsType> MultiTopic<T> {
    /// Constructor (called internally by
    /// `DomainParticipant::create_multitopic`).
    ///
    /// # Errors
    /// `BadParameter` if a referenced `%N` parameter is outside the
    /// `expression_parameters` vec.
    #[cfg(feature = "std")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        name: String,
        type_name: String,
        subscription_expression: String,
        expression: zerodds_sql_filter::TopicExpression,
        expression_parameters: Vec<String>,
        participant: DomainParticipant,
        plan: Arc<crate::multitopic::MultiTopicPlan>,
        result_topic: Topic<T>,
    ) -> Result<Self> {
        let used = expression
            .condition
            .as_ref()
            .map(Expr::collect_param_indices)
            .unwrap_or_default();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= expression_parameters.len() {
                return Err(DdsError::BadParameter {
                    what: "multitopic expression parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = expression_parameters
            .iter()
            .map(|s| param_string_to_value(s))
            .collect();
        let fp = FilterParams {
            raw: expression_parameters,
            values,
        };
        let related_topic_names = expression.from.clone();
        // A MultiTopic uses each of its related topics (DDS 1.4
        // §2.2.2.2.1.6), so each of those becomes undeletable while this
        // one exists. `related_topic_names` are names, not topic handles:
        // the count is kept on the participant that created the MultiTopic,
        // which is the one that can act on it, and `Drop` gives the counts
        // back under the same names.
        for related in &related_topic_names {
            participant
                .inner()
                .add_topic_user(related, TopicUserKind::MultiTopic);
        }
        Ok(Self {
            name,
            type_name,
            related_topic_names,
            subscription_expression,
            parsed: expression.condition.map(Arc::new),
            params: Arc::new(RwLock::new(fp)),
            participant,
            life: Arc::new(DescriptionLife::new()),
            plan,
            result_topic,
            _t: PhantomData,
        })
    }

    /// What the expression computes over the topics.
    #[cfg(feature = "std")]
    pub(crate) fn plan(&self) -> &Arc<crate::multitopic::MultiTopicPlan> {
        &self.plan
    }

    /// The topic description a reader of this MultiTopic is built on.
    #[cfg(feature = "std")]
    pub(crate) fn result_topic(&self) -> &Topic<T> {
        &self.result_topic
    }

    /// The expression parameters as they are at the moment of the call, for
    /// every evaluation of `WHERE`.
    #[cfg(feature = "std")]
    pub(crate) fn parameter_values(&self) -> crate::multitopic::ParameterValues {
        let params = Arc::clone(&self.params);
        Arc::new(move || params.read().map(|p| p.values.clone()).unwrap_or_default())
    }

    /// DDS 1.4 §2.2.2.2.1.10: deletes this MultiTopic for every handle of
    /// it; its uses of the related topics end here.
    ///
    /// # Errors
    /// `AlreadyDeleted` if it is deleted already; `PreconditionNotMet`
    /// while a DataReader created on it exists.
    pub(crate) fn delete_description(&self) -> Result<()> {
        let handles = self.life.delete()?;
        for _ in 0..handles {
            for related in &self.related_topic_names {
                self.participant
                    .inner()
                    .remove_topic_user(related, TopicUserKind::MultiTopic);
            }
        }
        Ok(())
    }

    /// This MultiTopic as the participant registry keeps it.
    #[cfg(feature = "std")]
    pub(crate) fn description_entry(&self) -> DescriptionEntry
    where
        T: 'static,
    {
        let name = self.name.clone();
        let type_name = self.type_name.clone();
        let related_names = self.related_topic_names.clone();
        let expression = self.subscription_expression.clone();
        let parsed = self.parsed.clone();
        let params = Arc::clone(&self.params);
        let plan = Arc::clone(&self.plan);
        let result_inner = self.result_topic.inner();
        let result_codec = self.result_topic.codec_arc();
        let participant = Arc::downgrade(self.participant.inner());
        let life = Arc::downgrade(&self.life);
        let (revive_name, revive_type) = (name.clone(), type_name.clone());
        let revive: Arc<dyn Fn() -> Option<Box<dyn core::any::Any + Send>> + Send + Sync> =
            Arc::new(move || {
                let life = life.upgrade()?;
                if life.is_deleted() {
                    return None;
                }
                let participant = DomainParticipant::from_inner(participant.upgrade()?);
                if life.add_handle() {
                    for related in &related_names {
                        participant
                            .inner()
                            .add_topic_user(related, TopicUserKind::MultiTopic);
                    }
                }
                Some(Box::new(MultiTopic::<T> {
                    name: revive_name.clone(),
                    type_name: revive_type.clone(),
                    related_topic_names: related_names.clone(),
                    subscription_expression: expression.clone(),
                    parsed: parsed.clone(),
                    params: Arc::clone(&params),
                    participant: participant.clone(),
                    life,
                    plan: Arc::clone(&plan),
                    result_topic: Topic::<T>::from_inner(Arc::clone(&result_inner), participant)
                        .with_codec(Arc::clone(&result_codec)),
                    _t: PhantomData,
                }))
            });
        let reviver = Arc::clone(&revive);
        let delete: Arc<dyn Fn() -> Result<()> + Send + Sync> = Arc::new(move || {
            let Some(object) = reviver() else {
                return Ok(());
            };
            match object.downcast::<MultiTopic<T>>() {
                Ok(multitopic) => multitopic.delete_description(),
                Err(_) => Ok(()),
            }
        });
        DescriptionEntry {
            name,
            type_name,
            kind: TopicDescriptionKind::MultiTopic,
            life: Arc::downgrade(&self.life),
            revive: Reviver(revive),
            delete: DeleteDescription(delete),
        }
    }

    /// Records a DataReader created on this topic (DDS 1.4 §2.2.2.2.1.10).
    pub(crate) fn attach_reader(&self) -> Result<DescriptionReaderUse> {
        self.life.attach_reader()
    }

    /// Spec §2.2.2.3.4.4 `get_subscription_expression`.
    #[must_use]
    pub fn get_subscription_expression(&self) -> &str {
        &self.subscription_expression
    }

    /// Spec §2.2.2.3.4.5 `get_expression_parameters`.
    #[must_use]
    pub fn get_expression_parameters(&self) -> Vec<String> {
        #[cfg(feature = "std")]
        {
            self.params
                .read()
                .map(|p| p.raw.clone())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "std"))]
        {
            self.params.raw.clone()
        }
    }

    /// Spec §2.2.2.3.4.6 `set_expression_parameters`.
    ///
    /// # Errors
    /// `BadParameter` if a referenced `%N` parameter is outside the new
    /// vec; `PreconditionNotMet` on lock poisoning; `AlreadyDeleted` if
    /// the topic is deleted.
    pub fn set_expression_parameters(&self, params: Vec<String>) -> Result<()> {
        self.life.check_not_deleted()?;
        let used = self
            .parsed
            .as_ref()
            .map(|expr| expr.collect_param_indices())
            .unwrap_or_default();
        if let Some(max) = used.iter().max() {
            if (*max as usize) >= params.len() {
                return Err(DdsError::BadParameter {
                    what: "multitopic expression parameter %N out of range",
                });
            }
        }
        let values: Vec<Value> = params.iter().map(|s| param_string_to_value(s)).collect();
        let fp = FilterParams {
            raw: params,
            values,
        };
        #[cfg(feature = "std")]
        {
            let mut w = self
                .params
                .write()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "multitopic params poisoned",
                })?;
            *w = fp;
        }
        #[cfg(not(feature = "std"))]
        {
            let _ = fp;
            return Err(DdsError::PreconditionNotMet {
                reason: "set_expression_parameters needs std feature",
            });
        }
        Ok(())
    }

    /// Spec §2.2.2.3.4.3 `get_related_topic` (the PSM returns a sequence
    /// — we return the names).
    #[must_use]
    pub fn get_related_topic_names(&self) -> &[String] {
        &self.related_topic_names
    }
}

impl<T: DdsType> Clone for MultiTopic<T> {
    fn clone(&self) -> Self {
        // A clone is one more handle on the same MultiTopic, and a handle
        // that exists is a user of every related topic (DDS 1.4
        // §2.2.2.2.1.6). `Drop` takes the counts back.
        if self.life.add_handle() {
            for related in &self.related_topic_names {
                self.participant
                    .inner()
                    .add_topic_user(related, TopicUserKind::MultiTopic);
            }
        }
        Self {
            name: self.name.clone(),
            type_name: self.type_name.clone(),
            related_topic_names: self.related_topic_names.clone(),
            subscription_expression: self.subscription_expression.clone(),
            parsed: self.parsed.clone(),
            #[cfg(feature = "std")]
            params: Arc::clone(&self.params),
            #[cfg(not(feature = "std"))]
            params: self.params.clone(),
            participant: self.participant.clone(),
            life: Arc::clone(&self.life),
            #[cfg(feature = "std")]
            plan: Arc::clone(&self.plan),
            #[cfg(feature = "std")]
            result_topic: self.result_topic.clone(),
            _t: PhantomData,
        }
    }
}

/// The MultiTopic is gone with its last handle, so it stops using its
/// related topics (DDS 1.4 §2.2.2.2.1.6 counts *existing* MultiTopics).
impl<T: DdsType> Drop for MultiTopic<T> {
    fn drop(&mut self) {
        // A deleted MultiTopic gave its uses back at `delete`.
        if self.life.drop_handle() {
            for related in &self.related_topic_names {
                self.participant
                    .inner()
                    .remove_topic_user(related, TopicUserKind::MultiTopic);
            }
        }
    }
}

impl<T: DdsType> TopicDescription for MultiTopic<T> {
    fn get_type_name(&self) -> &str {
        &self.type_name
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_participant(&self) -> &DomainParticipant {
        &self.participant
    }
}

fn param_string_to_value(s: &str) -> Value {
    Value::from_parameter(s)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dds_type::RawBytes;
    use crate::factory::DomainParticipantFactory;
    use crate::qos::DomainParticipantQos;

    #[test]
    #[allow(clippy::too_many_lines)] // Exercise each accessor against both endpoint factories.
    fn endpoint_creation_refuses_each_metadata_drift_and_shares_snapshot() {
        use crate::dds_type::{DecodeError, EncodeError, Extensibility};
        use crate::qos::{DataReaderQos, DataWriterQos, PublisherQos, SubscriberQos};
        use core::sync::atomic::{AtomicU8, Ordering};
        static CHANGE: AtomicU8 = AtomicU8::new(0);
        struct Changing(RawBytes);
        impl DdsType for Changing {
            const TYPE_NAME: &'static str = "test::TopicMetadata";
            fn type_name() -> &'static str {
                if CHANGE.load(Ordering::SeqCst) == 1 {
                    "test::ChangedName"
                } else {
                    Self::TYPE_NAME
                }
            }
            fn type_identifier() -> zerodds_types::TypeIdentifier {
                if CHANGE.load(Ordering::SeqCst) == 2 {
                    zerodds_types::TypeIdentifier::EquivalenceHashComplete(
                        zerodds_types::EquivalenceHash([1; 14]),
                    )
                } else {
                    zerodds_types::TypeIdentifier::None
                }
            }
            fn type_object() -> Option<zerodds_types::type_object::TypeObject> {
                (CHANGE.load(Ordering::SeqCst) == 3).then(|| {
                    zerodds_types::type_object::TypeObject::Complete(
                        zerodds_types::type_object::CompleteTypeObject::Struct(
                            zerodds_types::builder::TypeObjectBuilder::struct_type(Self::TYPE_NAME)
                                .build_complete(),
                        ),
                    )
                })
            }
            fn has_key() -> bool {
                CHANGE.load(Ordering::SeqCst) == 4
            }
            fn extensibility() -> Extensibility {
                if CHANGE.load(Ordering::SeqCst) == 5 {
                    Extensibility::Appendable
                } else {
                    Extensibility::Final
                }
            }
            fn key_holder_max_size() -> Option<usize> {
                if CHANGE.load(Ordering::SeqCst) == 6 {
                    Some(4)
                } else {
                    None
                }
            }
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                self.0.encode(out)
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                RawBytes::decode(bytes).map(Self)
            }
        }
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<Changing>("Metadata", TopicQos::default())
            .unwrap();
        let cloned = topic.clone();
        assert!(Arc::ptr_eq(&topic.codec, &cloned.codec));
        let reconstructed = p
            .create_topic::<Changing>("Metadata", TopicQos::default())
            .unwrap();
        assert!(core::ptr::eq(
            topic.checked_type_metadata().unwrap(),
            cloned.checked_type_metadata().unwrap()
        ));
        assert!(core::ptr::eq(
            topic.checked_type_metadata().unwrap(),
            reconstructed.checked_type_metadata().unwrap()
        ));
        let publisher = p.create_publisher(PublisherQos::default());
        let subscriber = p.create_subscriber(SubscriberQos::default());
        assert!(
            publisher
                .create_datawriter(&topic, DataWriterQos::default())
                .is_ok()
        );
        assert!(
            subscriber
                .create_datareader(&topic, DataReaderQos::default())
                .is_ok()
        );
        for change in 1..=6 {
            CHANGE.store(change, Ordering::SeqCst);
            let created = p.create_topic::<Changing>("Metadata", TopicQos::default());
            if change == 1 {
                // XTypes 1.3 §7.6.4.1: another type name under the name of
                // a topic is another topic of that name, "provided that all
                // of them […] have consistent QoS with one another", and
                // the QoS here is the same.
                assert!(created.is_ok(), "a second type name is a second topic");
            } else {
                assert!(
                    matches!(created, Err(DdsError::InconsistentPolicy { .. })),
                    "registry must refuse metadata change {change}"
                );
            }
            assert!(
                matches!(
                    publisher.create_datawriter(&topic, DataWriterQos::default()),
                    Err(DdsError::PreconditionNotMet { .. })
                ),
                "metadata change {change}"
            );
            assert!(
                matches!(
                    subscriber.create_datareader(&topic, DataReaderQos::default()),
                    Err(DdsError::PreconditionNotMet { .. })
                ),
                "metadata change {change}"
            );
            assert_eq!(topic.type_name(), Changing::TYPE_NAME);
        }
        CHANGE.store(0, Ordering::SeqCst);
        assert!(topic.checked_type_metadata().is_ok());
        let restored = p
            .create_topic::<Changing>("Metadata", TopicQos::default())
            .unwrap();
        assert!(Arc::ptr_eq(&topic.inner, &restored.inner));
        // Five of the six drifts keep the type name and are inconsistent.
        assert_eq!(
            topic.inner.inconsistent_topic_count.load(Ordering::SeqCst),
            5
        );
    }

    #[test]
    fn topic_implements_topic_description() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        // Trait methods work.
        let td: &dyn TopicDescription = &t;
        assert_eq!(td.get_name(), "Chatter");
        assert_eq!(td.get_type_name(), RawBytes::TYPE_NAME);
        assert_eq!(td.get_participant().domain_id(), 0);
    }

    #[test]
    fn topic_description_handle_is_cloneable() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(7, DomainParticipantQos::default());
        let h = TopicDescriptionHandle::new("X".into(), "T".into(), p.clone());
        let h2 = h.clone();
        assert_eq!(h2.get_name(), "X");
        assert_eq!(h2.get_type_name(), "T");
        assert_eq!(h2.get_participant().domain_id(), 7);
    }

    #[test]
    fn topic_description_trait_is_object_safe() {
        // Verify object safety: we can hold `&dyn TopicDescription` and
        // dispatch non-statically. Collect from several concrete
        // implementations.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(8, DomainParticipantQos::default());
        let t = p
            .create_topic::<RawBytes>("DynA", TopicQos::default())
            .unwrap();
        let h = TopicDescriptionHandle::new("DynB".into(), "T".into(), p.clone());
        let descs: Vec<&dyn TopicDescription> = vec![&t, &h];
        assert_eq!(descs.len(), 2);
        assert_eq!(descs[0].get_name(), "DynA");
        assert_eq!(descs[1].get_name(), "DynB");
    }

    #[test]
    fn topic_description_create_topic_rejects_empty_name() {
        // §2.2.2.2.1.4 create_topic: empty topic name → BadParameter.
        // This is the only place where TopicDescription consumers
        // encounter an invalid TopicDescription construction attempt.
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(9, DomainParticipantQos::default());
        let res = p.create_topic::<RawBytes>("", TopicQos::default());
        assert!(matches!(
            res,
            Err(crate::error::DdsError::BadParameter { .. })
        ));
    }

    #[test]
    fn topic_description_get_participant_returns_owning_participant() {
        // get_participant() returns exactly the participant on which
        // create_topic was called — no other.
        let p1 = DomainParticipantFactory::instance()
            .create_participant_offline(11, DomainParticipantQos::default());
        let p2 = DomainParticipantFactory::instance()
            .create_participant_offline(12, DomainParticipantQos::default());
        let t = p1
            .create_topic::<RawBytes>("Owned", TopicQos::default())
            .unwrap();
        let td: &dyn TopicDescription = &t;
        assert_eq!(td.get_participant().domain_id(), 11);
        assert_ne!(td.get_participant().domain_id(), p2.domain_id());
    }

    // -------- ContentFilteredTopic --------

    use alloc::collections::BTreeMap;
    use zerodds_sql_filter::{RowAccess, Value};

    struct MapRow(BTreeMap<String, Value>);
    impl RowAccess for MapRow {
        fn get(&self, path: &str) -> Option<Value> {
            self.0.get(path).cloned()
        }
    }

    fn row(pairs: &[(&str, Value)]) -> MapRow {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert((*k).into(), v.clone());
        }
        MapRow(m)
    }

    fn mk_p(domain: i32) -> DomainParticipant {
        DomainParticipantFactory::instance()
            .create_participant_offline(domain, DomainParticipantQos::default())
    }

    #[test]
    fn cft_compiles_and_evaluates_filter() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("Chatter", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("ChatterFilt", &topic, "x > 10", alloc::vec::Vec::new())
            .unwrap();
        // CFT trait works.
        let td: &dyn TopicDescription = &cft;
        assert_eq!(td.get_name(), "ChatterFilt");
        assert_eq!(td.get_type_name(), RawBytes::TYPE_NAME);

        // Filter: x > 10
        let r_yes = row(&[("x", Value::Int(20))]);
        let r_no = row(&[("x", Value::Int(5))]);
        assert_eq!(cft.evaluate(&r_yes), Ok(true));
        assert_eq!(cft.evaluate(&r_no), Ok(false));
    }

    #[test]
    fn cft_with_params_can_be_updated() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("Filt", &topic, "color = %0", alloc::vec!["RED".into()])
            .unwrap();
        assert_eq!(cft.get_filter_expression(), "color = %0");
        assert_eq!(cft.get_filter_parameters(), alloc::vec!["RED".to_string()]);

        let r = row(&[("color", Value::String("RED".into()))]);
        assert_eq!(cft.evaluate(&r), Ok(true));

        // Update the parameter.
        cft.set_filter_parameters(alloc::vec!["BLUE".into()])
            .unwrap();
        assert_eq!(cft.evaluate(&r), Ok(false));
    }

    #[test]
    fn cft_get_related_topic() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("Base", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "x = 1", alloc::vec::Vec::new())
            .unwrap();
        assert_eq!(cft.get_related_topic().name(), "Base");
    }

    #[test]
    fn cft_invalid_expression_rejected() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let err = p
            .create_contentfilteredtopic("CF", &topic, "x === bogus", alloc::vec::Vec::new())
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_param_index_out_of_range_rejected() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        // Expression uses %0 + %1, but we provide only one.
        let err = p
            .create_contentfilteredtopic("CF", &topic, "x = %0 AND y = %1", alloc::vec!["1".into()])
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_set_filter_parameters_validates_count() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic(
                "CF",
                &topic,
                "x = %0 AND y = %1",
                alloc::vec!["1".into(), "2".into()],
            )
            .unwrap();
        let err = cft
            .set_filter_parameters(alloc::vec!["1".into()])
            .unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_filter_with_string_param() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        // Strings: the caller provides them without '' quotes (spec
        // examples); the value conversion interprets them as a string
        // default.
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "name LIKE %0", alloc::vec!["foo%".into()])
            .unwrap();
        let r = row(&[("name", Value::String("foobar".into()))]);
        assert_eq!(cft.evaluate(&r), Ok(true));
    }

    #[test]
    fn cft_filter_with_or_and_combination() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic(
                "CF",
                &topic,
                "(x > 10 AND x < 100) OR color = 'RED'",
                alloc::vec::Vec::new(),
            )
            .unwrap();
        // x in range, color irrelevant.
        let r1 = row(&[
            ("x", Value::Int(50)),
            ("color", Value::String("BLUE".into())),
        ]);
        assert_eq!(cft.evaluate(&r1), Ok(true));
        // x out, color RED.
        let r2 = row(&[("x", Value::Int(5)), ("color", Value::String("RED".into()))]);
        assert_eq!(cft.evaluate(&r2), Ok(true));
        // both fail.
        let r3 = row(&[
            ("x", Value::Int(5)),
            ("color", Value::String("BLUE".into())),
        ]);
        assert_eq!(cft.evaluate(&r3), Ok(false));
    }

    #[test]
    fn cft_unknown_field_returns_bad_parameter() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "missing = 1", alloc::vec::Vec::new())
            .unwrap();
        let r = row(&[("x", Value::Int(1))]);
        let err = cft.evaluate(&r).unwrap_err();
        assert!(matches!(err, DdsError::BadParameter { .. }));
    }

    #[test]
    fn cft_clone_shares_params() {
        let p = mk_p(0);
        let topic = p
            .create_topic::<RawBytes>("T", TopicQos::default())
            .unwrap();
        let cft = p
            .create_contentfilteredtopic("CF", &topic, "color = %0", alloc::vec!["RED".into()])
            .unwrap();
        let cft2 = cft.clone();
        // Update via cft → visible in cft2 (Arc<RwLock> shared).
        cft.set_filter_parameters(alloc::vec!["BLUE".into()])
            .unwrap();
        assert_eq!(
            cft2.get_filter_parameters(),
            alloc::vec!["BLUE".to_string()]
        );
    }

    #[test]
    fn param_string_to_value_heuristics() {
        assert_eq!(super::param_string_to_value("42"), Value::Int(42));
        assert_eq!(super::param_string_to_value("2.5"), Value::Float(2.5));
        assert_eq!(super::param_string_to_value("TRUE"), Value::Bool(true));
        assert_eq!(super::param_string_to_value("False"), Value::Bool(false));
        assert_eq!(
            super::param_string_to_value("'hello'"),
            Value::String("hello".into())
        );
        assert_eq!(
            super::param_string_to_value("plain"),
            Value::String("plain".into())
        );
    }
}
