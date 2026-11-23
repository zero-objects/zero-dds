// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DLRL run-time entities over `zerodds-dcps` (DDS 1.2 §8.1.5,
//! §8.1.6): [`CacheFactory`], [`Cache`], [`CacheAccess`], [`Contract`],
//! [`ObjectHome`], [`ObjectRoot`], [`CacheListener`],
//! [`ObjectListener`].
//!
//! # Operational mapping (§8.1.5)
//!
//! * A Cache has one Publisher (usage `WRITE_ONLY` or `READ_WRITE`)
//!   and one Subscriber (`READ_ONLY` or `READ_WRITE`) of the
//!   DomainParticipant of its [`CacheDescription`], created by
//!   `create_cache` for its use only and not enabled; every DataWriter
//!   and DataReader of the Cache belongs to them (§8.1.5.1).
//! * `register_all_for_pubsub` creates, for every topic in which the
//!   classes of the registered homes have rows, the Topic, a DataWriter
//!   and a DataReader, not enabled (ENTITY_FACTORY of the Publisher and
//!   Subscriber has `autoenable_created_entities` FALSE);
//!   `enable_all_for_pubsub` enables the Publisher and Subscriber, then
//!   the DataWriters and DataReaders, which performs the subscriptions
//!   (§8.1.5.2).
//! * Between the two, the application sets QoS on these entities, also
//!   the policies that are immutable once enabled, such as PRESENTATION:
//!   the Publisher and Subscriber are `the_publisher` and `the_subscriber`;
//!   `ObjectHome::get_topic_name` and `get_all_topic_names` name the
//!   topics, and `lookup_datawriter` / `lookup_datareader` of the
//!   Publisher and Subscriber with a topic name give the DataWriter and
//!   DataReader of `DynamicSample` (§8.1.5.3). The DataReaders are
//!   created RELIABLE, the DataWriters with the default QoS (RELIABLE).
//!   A Topic is enabled when it is created: its QoS is given before
//!   `register_all_for_pubsub` ([`Cache::set_topic_qos`], or
//!   [`Cache::set_new_topic_qos`] for a Topic the DomainParticipant does
//!   not have yet), the default QoS of DDS otherwise.
//! * With PRESENTATION `coherent_access` on the Publisher, a
//!   `CacheAccess::write` is one coherent set; with `access_scope` GROUP
//!   it spans the topics, and a Subscriber with GROUP scope and
//!   `coherent_access` takes the samples of all DataReaders in one
//!   `begin_access`/`end_access`, so that an update round has the whole
//!   set or none of it (§8.1.5.1: a relation set to a newly created
//!   object arrives with that object).
//!
//! # Functional mapping (§8.1.6.1)
//!
//! * Publishing: `ObjectHome::create_object` in a CacheAccess that
//!   allows writing, `ObjectRoot::set`, `ObjectRoot::destroy`,
//!   `CacheAccess::write`; one CacheAccess per thread of modifications.
//! * Subscribing: the samples of the DataReaders are the rows of the
//!   tables of §8.1.4.2; an update round takes all waiting samples,
//!   reconstructs the objects whose rows changed, and reports them to
//!   the listeners (§8.1.6.4). With `updates_enabled` the rounds run on a
//!   thread of the Cache as samples arrive; `disable_updates` keeps the
//!   samples taken for later, `load` (and the Cache's `refresh`) applies
//!   them without listener, `enable_updates` applies them as a round
//!   with listeners. A `disable_updates` in the middle of a round
//!   interrupts it ([`Cache::last_round_interrupted`]).
//! * Navigation stays inside the subscriptions: a relation to a class
//!   without registered home raises `NotFound` (§8.1.6.1.2.1).
//! * `Cache::lock` holds the update rounds and the other threads off
//!   until `Cache::unlock` (§8.1.6.3.4); `delete_cache` returns while a
//!   thread holds the lock, and a thread that waits for it stops waiting
//!   with `AlreadyDeleted`; an `ObjectHome` with
//!   `auto_deref` FALSE leaves the state of its objects in the rows of
//!   the tables until the application accesses it (§8.1.6.3.7).
//!
//! A sample that cannot be read as a row of its topic, or rows from which
//! no object can be reconstructed, change no object; each is noted in
//! [`Cache::rejected_updates`].

mod access;
mod collection;
mod home;
mod object;
mod selection;

pub use access::{CacheAccess, Contract, UNLIMITED_RELATED_OBJECTS};
pub use collection::{Collection, IntMap, List, Set, StrMap};
pub use home::{ObjectHome, ObjectListener};
pub use object::{ObjectRoot, ObjectScope, ObjectState};
pub use selection::{
    FilterCriterion, QueryCriterion, Selection, SelectionCriterion, SelectionListener,
};

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::{JoinHandle, ThreadId};
use std::time::{Duration, Instant};

use zerodds_dcps::listener::{ArcDataReaderListener, DataReaderListener};
use zerodds_dcps::psm_constants::status::DATA_AVAILABLE;
use zerodds_dcps::qos::{PresentationAccessScope, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::sample_info::InstanceStateKind;
use zerodds_dcps::serialized_type::{DynamicDataReader, DynamicDataWriter, SerializedTypeSupport};
use zerodds_dcps::topic::Topic;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipant, Entity, HANDLE_NIL, InstanceHandle, Publisher,
    PublisherQos, Subscriber, SubscriberQos, TopicQos,
};
use zerodds_types::dynamic::{DynamicSample, DynamicType};

use crate::dcps_binding;
use crate::mapping::{KeyContent, Mapping, TopicDescription};
use crate::metamodel::DlrlException;
use crate::model::{Member, OBJECT_ROOT_NAMES, ObjectRef};
use crate::rows::{Cell, KeyAssociation, ObjectData, Row};
use crate::types::DlrlOid;

use home::RoundLists;
use object::{ObjectCell, Owner};

/// PSM `CacheUsage` (§8.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheUsage {
    /// `READ_ONLY`: no publication.
    ReadOnly,
    /// `WRITE_ONLY`: no subscription.
    WriteOnly,
    /// `READ_WRITE`: both.
    ReadWrite,
}

impl CacheUsage {
    /// The usage allows writing.
    #[must_use]
    pub const fn writes(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }

    /// The usage allows reading.
    #[must_use]
    pub const fn reads(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }
}

/// PSM `CacheKind` (§8.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheKind {
    /// `CACHE_KIND`.
    Cache,
    /// `CACHEACCESS_KIND`.
    CacheAccess,
}

/// PSM `DCPSState` (§8.2.1): the state of a Cache with respect to the
/// DCPS entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DcpsState {
    /// `INITIAL`: no DCPS entity of the homes yet.
    Initial,
    /// `REGISTERED`: created, not enabled.
    Registered,
    /// `ENABLED`.
    Enabled,
}

/// `CacheListener` (§8.1.6.3.5): the application is made aware of the
/// update rounds of a Cache.
pub trait CacheListener: Send + Sync {
    /// Updates follow; the Cache is changed when this returns.
    fn on_begin_updates(&self);
    /// No more update of this round. Whether `disable_updates`
    /// interrupted it is [`Cache::last_round_interrupted`] (§8.1.6.3.4;
    /// the operation has no parameter in the table and the IDL).
    fn on_end_updates(&self);
    /// The Cache switched to automatic update mode.
    fn on_updates_enabled(&self);
    /// The Cache switched to manual update mode.
    fn on_updates_disabled(&self);
}

/// PSM `CacheDescription` (§8.2.1): the name of a Cache and its
/// DomainParticipant.
#[derive(Clone)]
pub struct CacheDescription {
    /// `name`.
    pub name: String,
    /// `domain`.
    pub domain: DomainParticipant,
}

/// `CacheBase` (§8.1.6.3.2): a [`Cache`] or a [`CacheAccess`].
#[derive(Clone, Debug, PartialEq)]
pub enum CacheBase {
    /// A Cache.
    Cache(Cache),
    /// A CacheAccess.
    Access(CacheAccess),
}

impl CacheBase {
    /// `kind`.
    #[must_use]
    pub fn kind(&self) -> CacheKind {
        match self {
            Self::Cache(_) => CacheKind::Cache,
            Self::Access(_) => CacheKind::CacheAccess,
        }
    }

    /// `cache_usage`.
    #[must_use]
    pub fn cache_usage(&self) -> CacheUsage {
        match self {
            Self::Cache(c) => c.cache_usage(),
            Self::Access(a) => a.cache_usage(),
        }
    }

    /// `objects`.
    #[must_use]
    pub fn objects(&self) -> Vec<ObjectRoot> {
        match self {
            Self::Cache(c) => c.objects(),
            Self::Access(a) => a.objects(),
        }
    }

    /// `refresh`.
    ///
    /// # Errors
    /// As [`Cache::refresh`] and [`CacheAccess::refresh`].
    pub fn refresh(&self) -> Result<(), DlrlException> {
        match self {
            Self::Cache(c) => c.refresh(),
            Self::Access(a) => a.refresh(),
        }
    }

    pub(crate) fn lookup(&self, target: &ObjectRef) -> Option<ObjectRoot> {
        match self {
            Self::Cache(c) => c.lookup(target),
            Self::Access(a) => a.lookup(target),
        }
    }

    /// `AlreadyDeleted` for a deleted Cache or CacheAccess.
    pub(crate) fn alive(&self) -> Result<(), DlrlException> {
        match self {
            Self::Cache(c) => c.alive(),
            Self::Access(a) => a.alive(),
        }
    }
}

/// `CacheFactory` (§8.1.6.3.1): the unique instance creates the Caches.
pub struct CacheFactory {
    caches: Mutex<BTreeMap<String, Cache>>,
}

static FACTORY: OnceLock<CacheFactory> = OnceLock::new();

impl CacheFactory {
    /// `get_instance`: the singleton, the same at every call.
    pub fn get_instance() -> &'static CacheFactory {
        FACTORY.get_or_init(|| CacheFactory {
            caches: Mutex::new(BTreeMap::new()),
        })
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<String, Cache>> {
        self.caches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `create_cache`: a Cache for `cache_usage` on the DomainParticipant
    /// of `description`, with a Publisher, a Subscriber or both for its
    /// use only.
    ///
    /// # Errors
    /// `AlreadyExisting` when a Cache of that name exists;
    /// `DCPSError` when the participant refuses an entity.
    pub fn create_cache(
        &self,
        cache_usage: CacheUsage,
        description: CacheDescription,
    ) -> Result<Cache, DlrlException> {
        let mut caches = self.lock();
        if caches.contains_key(&description.name) {
            return Err(DlrlException::AlreadyExisting {
                message: alloc::format!("a Cache named {} exists", description.name),
            });
        }
        let participant = description.domain;
        let mut pq = PublisherQos::default();
        pq.entity_factory.autoenable_created_entities = false;
        let mut sq = SubscriberQos::default();
        sq.entity_factory.autoenable_created_entities = false;
        // Not enabled: QoS that is immutable once enabled (PRESENTATION)
        // is set between register_all_for_pubsub and enable_all_for_pubsub
        // (§8.1.5.3).
        let publisher = cache_usage
            .writes()
            .then(|| Arc::new(participant.create_disabled_publisher(pq)));
        let subscriber = cache_usage
            .reads()
            .then(|| Arc::new(participant.create_disabled_subscriber(sq)));
        let (host, pid, seed) = oid_creator();
        let cache = Cache {
            inner: Arc::new(CacheInner {
                name: description.name.clone(),
                usage: cache_usage,
                participant,
                publisher,
                subscriber,
                state: Mutex::new(CacheState {
                    pubsub: DcpsState::Initial,
                    updates_enabled: true,
                    homes: Vec::new(),
                    mapping: None,
                    listeners: Vec::new(),
                    accesses: Vec::new(),
                    topics: Vec::new(),
                    topic_qos: BTreeMap::new(),
                    objects: BTreeMap::new(),
                    keys: KeyAssociation::process_wide(host, pid, seed),
                    pending: BTreeMap::new(),
                    owners: BTreeMap::new(),
                    published: BTreeMap::new(),
                    rejected: Rejections::default(),
                    in_round: None,
                    interrupted: false,
                    dirty: Vec::new(),
                }),
                gate: Gate::default(),
                wake: Arc::new(Wake::default()),
                thread: Mutex::new(None),
                deleted: AtomicBool::new(false),
            }),
        };
        caches.insert(description.name, cache.clone());
        Ok(cache)
    }

    /// `find_cache_by_name`.
    #[must_use]
    pub fn find_cache_by_name(&self, name: &str) -> Option<Cache> {
        self.lock().get(name).cloned()
    }

    /// `delete_cache`: deletes the Cache and releases all its resources:
    /// its CacheAccesses (purged and deleted), its thread, its
    /// DataWriters, DataReaders, Topics, Publisher and Subscriber. From
    /// then on every operation of the Cache and of its CacheAccesses
    /// raises `AlreadyDeleted` (§8.1.6.2, §8.2.1.1).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a Cache deleted already; `PreconditionNotMet`
    /// for a Cache this factory does not hold; `DCPSError` when DCPS
    /// refuses to delete an entity.
    pub fn delete_cache(&self, a_cache: &Cache) -> Result<(), DlrlException> {
        a_cache.alive()?;
        let removed = {
            let mut caches = self.lock();
            match caches.get(&a_cache.inner.name) {
                Some(c) if c == a_cache => caches.remove(&a_cache.inner.name),
                _ => None,
            }
        };
        if removed.is_none() {
            return Err(DlrlException::precondition(
                "the Cache is not one of this factory",
            ));
        }
        a_cache.inner.deleted.store(true, Ordering::Release);
        // No round waits any more for a lock an application thread holds
        // (this one or another): the update thread ends and is joined.
        a_cache.inner.gate.close();
        let accesses = core::mem::take(&mut a_cache.state().accesses);
        for access in &accesses {
            access.delete();
        }
        a_cache.shutdown()
    }
}

/// What the oids of every Cache of this process are made of: the host id
/// of the participants' GUID prefixes (a hash of the host name) as first
/// value, the process id marked with its PID namespace as second, and the
/// start of the third, taken from the randomness of the operating system
/// once per process, so that a later process with the same id starts
/// elsewhere. The third value comes from one counter for all Caches of the
/// process ([`KeyAssociation::process_wide`]): two Caches never allocate
/// one oid, also not on two participants. None of it depends on the
/// participant, so it holds with a GUID prefix that DDS-Security derived
/// from an identity too.
fn oid_creator() -> (i32, i32, u32) {
    static CREATOR: OnceLock<(i32, i32, u32)> = OnceLock::new();
    *CREATOR.get_or_init(|| {
        use std::hash::{BuildHasher, Hasher};
        use zerodds_dcps::participant::{host_id_bytes, namespaced_pid, pid_namespace_inode};
        let host = i32::from_be_bytes(host_id_bytes());
        let pid = namespaced_pid(std::process::id(), pid_namespace_inode());
        let h = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        let seed = u32::try_from((h ^ (h >> 32)) & u64::from(u32::MAX)).unwrap_or(1);
        (host, i32::from_ne_bytes(pid.to_ne_bytes()), seed)
    })
}

#[derive(Default)]
struct WakeState {
    signaled: bool,
    stop: bool,
}

#[derive(Default)]
pub(crate) struct Wake {
    state: Mutex<WakeState>,
    cv: Condvar,
}

impl Wake {
    fn lock(&self) -> MutexGuard<'_, WakeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn signal(&self) {
        self.lock().signaled = true;
        self.cv.notify_all();
    }

    fn stop(&self) {
        self.lock().stop = true;
        self.cv.notify_all();
    }

    /// Waits for a signal; false once stopped.
    fn wait(&self) -> bool {
        let mut st = self.lock();
        while !st.signaled && !st.stop {
            let (next, _) = self
                .cv
                .wait_timeout(st, Duration::from_millis(200))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            st = next;
            if !st.signaled && !st.stop {
                // A periodic look: a sample may have arrived before the
                // listener was installed.
                break;
            }
        }
        st.signaled = false;
        !st.stop
    }
}

/// The exclusion of a Cache's modifications (§8.1.6.3.4): an update
/// round, `load`, `refresh` and the Selections' examinations hold it, and
/// `Cache::lock` holds it for the application until `Cache::unlock`. The
/// thread that holds it may take it again.
///
/// `delete_cache` closes the gate: from then on it is given to nobody,
/// and a thread that waits for it stops waiting. The update thread, which
/// may wait for a gate an application thread holds, thus ends, and
/// `delete_cache` can join it.
#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<GateState>,
    cv: Condvar,
}

#[derive(Default)]
struct GateState {
    holder: Option<ThreadId>,
    depth: u32,
    closed: bool,
}

impl Gate {
    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Closes the gate for good and wakes every thread that waits for it.
    fn close(&self) {
        self.lock().closed = true;
        self.cv.notify_all();
    }

    /// Takes the gate for the calling thread, waiting at most `timeout`
    /// (`None`: without limit); false when the time ran out or the gate
    /// is closed.
    fn acquire(&self, timeout: Option<Duration>) -> bool {
        let me = std::thread::current().id();
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut st = self.lock();
        loop {
            if st.closed {
                return false;
            }
            match st.holder {
                None => {
                    st.holder = Some(me);
                    st.depth = 1;
                    return true;
                }
                Some(h) if h == me => {
                    st.depth += 1;
                    return true;
                }
                Some(_) => {}
            }
            st = match deadline {
                None => self
                    .cv
                    .wait(st)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return false;
                    }
                    self.cv
                        .wait_timeout(st, d - now)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Gives back one taking by the calling thread; false when the
    /// calling thread does not hold the gate. Works on a closed gate too.
    fn release(&self) -> bool {
        let mut st = self.lock();
        if st.holder != Some(std::thread::current().id()) {
            return false;
        }
        st.depth -= 1;
        if st.depth == 0 {
            st.holder = None;
            self.cv.notify_all();
        }
        true
    }
}

/// The gate taken by an update round, released at its end.
pub(crate) struct RoundGuard<'a>(&'a Gate);

impl Drop for RoundGuard<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

struct WakeListener(Arc<Wake>);

impl DataReaderListener for WakeListener {
    fn on_data_available(&self, _reader: InstanceHandle) {
        self.0.signal();
    }
}

/// The owner of a row: its class and the text of its owner key cells.
pub(crate) type OwnerId = (String, String);

/// An object by class and oid.
type ObjectKey = (String, DlrlOid);

pub(crate) struct TopicBinding {
    desc: TopicDescription,
    type_: DynamicType,
    topic: Topic<DynamicSample>,
    writer: Option<DynamicDataWriter>,
    reader: Option<DynamicDataReader>,
    table: BTreeMap<String, (Row, Option<OwnerId>)>,
    by_owner: BTreeMap<OwnerId, BTreeSet<String>>,
}

/// The updates taken from the DataReaders and not yet applied: per topic
/// (its index in `CacheState::topics`) and row key the last one, a row,
/// or `None` for a disposed instance. A round compares the state of each
/// object before and after it, so an earlier update of the same instance
/// changes nothing; in manual mode the updates registered for `load`
/// are thus as many as the instances, not as the samples.
type Updates = BTreeMap<(usize, String), Option<Row>>;

/// The most notes [`Cache::rejected_updates`] keeps; beyond it the oldest
/// is dropped and counted.
pub const MAX_REJECTED_UPDATES: usize = 1024;

/// The notes of [`Cache::rejected_updates`]: at most
/// [`MAX_REJECTED_UPDATES`], with the number of older ones dropped.
#[derive(Default)]
pub(crate) struct Rejections {
    notes: VecDeque<String>,
    dropped: u64,
}

impl Rejections {
    fn push(&mut self, note: String) {
        if self.notes.len() >= MAX_REJECTED_UPDATES {
            self.notes.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.notes.push_back(note);
    }

    /// The notes, oldest first, after one that counts the dropped notes
    /// when there are any; empty afterwards.
    fn take(&mut self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.notes.len() + 1);
        if self.dropped > 0 {
            out.push(alloc::format!(
                "{} earlier note(s) dropped: at most {MAX_REJECTED_UPDATES} are kept",
                self.dropped
            ));
            self.dropped = 0;
        }
        out.extend(self.notes.drain(..));
        out
    }
}

/// A QoS given for a Topic before `register_all_for_pubsub`.
#[derive(Clone)]
struct GivenTopicQos {
    qos: TopicQos,
    /// [`Cache::set_topic_qos`]: a Topic the DomainParticipant has with
    /// another QoS is refused. [`Cache::set_new_topic_qos`]: it is used
    /// with the QoS it has.
    required: bool,
}

pub(crate) struct CacheState {
    pubsub: DcpsState,
    updates_enabled: bool,
    homes: Vec<ObjectHome>,
    mapping: Option<Arc<Mapping>>,
    listeners: Vec<Arc<dyn CacheListener>>,
    accesses: Vec<CacheAccess>,
    topics: Vec<TopicBinding>,
    /// The QoS of the Topics `register_all_for_pubsub` creates, by topic
    /// name ([`Cache::set_topic_qos`], [`Cache::set_new_topic_qos`]); the
    /// default QoS for the others.
    topic_qos: BTreeMap<String, GivenTopicQos>,
    objects: BTreeMap<ObjectKey, ObjectRoot>,
    keys: KeyAssociation,
    pending: Updates,
    /// The object of each owner key of the tables.
    owners: BTreeMap<OwnerId, ObjectKey>,
    published: BTreeMap<ObjectKey, Vec<Row>>,
    rejected: Rejections,
    in_round: Option<ThreadId>,
    /// `disable_updates` was called in the middle of the last update
    /// round with listeners (§8.1.6.3.4); reset when the next one begins.
    interrupted: bool,
    dirty: Vec<ObjectRoot>,
}

pub(crate) struct CacheInner {
    name: String,
    usage: CacheUsage,
    participant: DomainParticipant,
    publisher: Option<Arc<Publisher>>,
    subscriber: Option<Arc<Subscriber>>,
    state: Mutex<CacheState>,
    gate: Gate,
    wake: Arc<Wake>,
    thread: Mutex<Option<JoinHandle<()>>>,
    /// Set by `delete_cache`.
    deleted: AtomicBool,
}

/// `Cache` (§8.1.6.3.4): a set of objects managed, published and/or
/// subscribed consistently, attached to one Publisher and/or one
/// Subscriber.
#[derive(Clone)]
pub struct Cache {
    pub(crate) inner: Arc<CacheInner>,
}

impl core::fmt::Debug for Cache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cache")
            .field("name", &self.inner.name)
            .field("cache_usage", &self.inner.usage)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Cache {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// The updates of one round, with the containers to report for
/// `concerns_contained_objects`.
#[derive(Default)]
struct Round {
    created: Vec<ObjectRoot>,
    modified: Vec<ObjectRoot>,
    deleted: Vec<ObjectRoot>,
    containers: Vec<ObjectRoot>,
}

impl Cache {
    fn state(&self) -> MutexGuard<'_, CacheState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `AlreadyDeleted` once `delete_cache` deleted the Cache: every
    /// operation raises it (§8.1.6.2, §8.2.1.1). The attributes keep
    /// their last value.
    pub(crate) fn alive(&self) -> Result<(), DlrlException> {
        if self.is_deleted() {
            Err(DlrlException::already_deleted(alloc::format!(
                "the Cache {} is deleted",
                self.inner.name
            )))
        } else {
            Ok(())
        }
    }

    /// TRUE once `delete_cache` deleted the Cache.
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.inner.deleted.load(Ordering::Acquire)
    }

    /// The name of the Cache.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// `cache_usage`.
    #[must_use]
    pub fn cache_usage(&self) -> CacheUsage {
        self.inner.usage
    }

    /// `kind`: [`CacheKind::Cache`].
    #[must_use]
    pub fn kind(&self) -> CacheKind {
        CacheKind::Cache
    }

    /// `pubsub_state`.
    #[must_use]
    pub fn pubsub_state(&self) -> DcpsState {
        self.state().pubsub
    }

    /// `updates_enabled`.
    #[must_use]
    pub fn updates_enabled(&self) -> bool {
        self.state().updates_enabled
    }

    /// TRUE when `disable_updates` was called in the middle of the last
    /// update round with listeners: in `on_begin_updates` (the updates of
    /// the round stay registered and no object listener hears them), in
    /// a listener of an object or a Selection, or by another thread
    /// before `on_end_updates`. FALSE from the beginning of the next such
    /// round on.
    ///
    /// §8.1.6.3.4 says that the Listener "will receive end_updates with a
    /// parameter that indicates that the updates have been interrupted",
    /// but `on_end_updates` has no parameter in the table of §8.1.6.3.5
    /// and in the IDL of §8.2.1.2.1; a `CacheListener` reads the
    /// indication here from its `on_end_updates`. Not an attribute of the
    /// PIM.
    #[must_use]
    pub fn last_round_interrupted(&self) -> bool {
        self.state().interrupted
    }

    /// `the_publisher`: `None` for a READ_ONLY Cache.
    #[must_use]
    pub fn the_publisher(&self) -> Option<Arc<Publisher>> {
        self.inner.publisher.clone()
    }

    /// `the_subscriber`: `None` for a WRITE_ONLY Cache.
    #[must_use]
    pub fn the_subscriber(&self) -> Option<Arc<Subscriber>> {
        self.inner.subscriber.clone()
    }

    /// The DomainParticipant of the Cache.
    #[must_use]
    pub fn participant(&self) -> &DomainParticipant {
        &self.inner.participant
    }

    /// `homes`, in registration order.
    #[must_use]
    pub fn homes(&self) -> Vec<ObjectHome> {
        self.state().homes.clone()
    }

    /// `sub_accesses`.
    #[must_use]
    pub fn sub_accesses(&self) -> Vec<CacheAccess> {
        self.state().accesses.clone()
    }

    /// `listeners`.
    #[must_use]
    pub fn listeners(&self) -> Vec<Arc<dyn CacheListener>> {
        self.state().listeners.clone()
    }

    /// `objects`: the objects of the Cache.
    #[must_use]
    pub fn objects(&self) -> Vec<ObjectRoot> {
        self.state().objects.values().cloned().collect()
    }

    /// The samples and rows that changed no object since the last call
    /// (since the Cache was created), with the reason; taken out by this
    /// call. At most [`MAX_REJECTED_UPDATES`] notes are kept, the most
    /// recent ones; when older ones were dropped, the first note says
    /// how many.
    #[must_use]
    pub fn rejected_updates(&self) -> Vec<String> {
        self.state().rejected.take()
    }

    /// `register_home`: registers a home and returns its index.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the DCPS state is not INITIAL, the home
    /// is registered already (with this or another Cache), a home of the
    /// same class is registered, or the home belongs to another mapping
    /// than the homes registered before.
    pub fn register_home(&self, a_home: &ObjectHome) -> Result<u32, DlrlException> {
        self.alive()?;
        let mut st = self.state();
        if st.pubsub != DcpsState::Initial {
            return Err(DlrlException::precondition(
                "register_home needs the DCPS state INITIAL",
            ));
        }
        let mut hs = a_home.lock();
        if hs.cache.is_some() {
            return Err(DlrlException::precondition(
                "the home is registered already",
            ));
        }
        if st
            .homes
            .iter()
            .any(|h| h.class_name() == a_home.class_name())
        {
            return Err(DlrlException::precondition(alloc::format!(
                "a home of class {} is registered already",
                a_home.class_name()
            )));
        }
        match &st.mapping {
            Some(m) if !Arc::ptr_eq(m, a_home.mapping()) && **m != **a_home.mapping() => {
                return Err(DlrlException::precondition(
                    "the home belongs to another mapping than the homes registered before",
                ));
            }
            Some(_) => {}
            None => st.mapping = Some(Arc::clone(a_home.mapping())),
        }
        let index = u32::try_from(st.homes.len())
            .map_err(|_| DlrlException::precondition("too many homes"))?;
        hs.cache = Some(Arc::downgrade(&self.inner));
        hs.index = Some(index);
        drop(hs);
        st.homes.push(a_home.clone());
        Ok(index)
    }

    /// `find_home_by_name`: the registered home of `class_name`, `None`
    /// (NULL) when there is none.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn find_home_by_name(&self, class_name: &str) -> Result<Option<ObjectHome>, DlrlException> {
        self.alive()?;
        Ok(self.home_by_name(class_name))
    }

    /// The registered home of `class_name`.
    pub(crate) fn home_by_name(&self, class_name: &str) -> Option<ObjectHome> {
        self.state()
            .homes
            .iter()
            .find(|h| h.class_name() == class_name)
            .cloned()
    }

    /// `find_home_by_index`: the home registered with `index`, `None`
    /// (NULL) when there is none.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn find_home_by_index(&self, index: u32) -> Result<Option<ObjectHome>, DlrlException> {
        self.alive()?;
        Ok(usize::try_from(index)
            .ok()
            .and_then(|i| self.state().homes.get(i).cloned()))
    }

    /// The QoS of the Topic `topic_name` that `register_all_for_pubsub`
    /// creates; without a call the Topic has the default QoS of DDS. Not
    /// an operation of the PIM.
    ///
    /// The QoS of the Publisher, the Subscriber, the DataWriters and the
    /// DataReaders is set between `register_all_for_pubsub` and
    /// `enable_all_for_pubsub` (§8.1.5.3). A Topic is enabled when it is
    /// created, and DURABILITY, RELIABILITY, HISTORY and the other
    /// policies DDS does not change on an enabled entity cannot be set on
    /// it afterwards: its QoS is given before. A Topic of that name and
    /// type the DomainParticipant has already (of another Cache, or of the
    /// application) is that Topic, with the QoS it has (DDS 1.4
    /// §2.2.2.2.1.5): it is used when that is the QoS given here, and with
    /// another QoS `register_all_for_pubsub` raises `DCPSError`. Without a
    /// call such a Topic is used as it is ([`Cache::set_new_topic_qos`]
    /// gives a QoS for the Topic only if it is created). A later call for
    /// the topic replaces the QoS of an earlier one.
    ///
    /// # Errors
    /// `PreconditionNotMet` in a DCPS state other than INITIAL, or for a
    /// name that is no topic of the classes of the registered homes;
    /// `BadParameter` for a QoS that is not consistent (DDS 1.4
    /// §2.2.3); `AlreadyDeleted` for a deleted Cache.
    pub fn set_topic_qos(&self, topic_name: &str, qos: TopicQos) -> Result<(), DlrlException> {
        self.give_topic_qos(topic_name, qos, true)
    }

    /// The QoS of the Topic `topic_name` if `register_all_for_pubsub`
    /// creates it. A Topic of that name and type the DomainParticipant has
    /// already (of another Cache, of a DDS4CCM connector, or of the
    /// application) is used with the QoS it has, also one that another
    /// thread creates between this call and `register_all_for_pubsub`;
    /// whoever needs certain values in it reads them from the Topic of the
    /// DataWriter or DataReader before `enable_all_for_pubsub`. Not an
    /// operation of the PIM. A later call for the topic, of this operation
    /// or of [`Cache::set_topic_qos`], replaces an earlier one.
    ///
    /// # Errors
    /// As [`Cache::set_topic_qos`].
    pub fn set_new_topic_qos(&self, topic_name: &str, qos: TopicQos) -> Result<(), DlrlException> {
        self.give_topic_qos(topic_name, qos, false)
    }

    /// [`Cache::set_topic_qos`] (`required`) and
    /// [`Cache::set_new_topic_qos`].
    fn give_topic_qos(
        &self,
        topic_name: &str,
        qos: TopicQos,
        required: bool,
    ) -> Result<(), DlrlException> {
        self.alive()?;
        let mut st = self.state();
        if st.pubsub != DcpsState::Initial {
            return Err(DlrlException::precondition(
                "the QoS of a topic needs the DCPS state INITIAL: the topics exist",
            ));
        }
        let known = st.mapping.as_ref().is_some_and(|m| {
            st.homes.iter().any(|h| {
                m.topics_of_class(h.class_name())
                    .iter()
                    .any(|t| t == topic_name)
            })
        });
        if !known {
            return Err(DlrlException::precondition(alloc::format!(
                "{topic_name} is no topic of the classes of the registered homes"
            )));
        }
        qos.check_consistency()
            .map_err(|e| DlrlException::BadParameter {
                message: alloc::format!("the QoS of topic {topic_name}: {e:?}"),
            })?;
        st.topic_qos
            .insert(topic_name.into(), GivenTopicQos { qos, required });
        Ok(())
    }

    /// `register_all_for_pubsub` (§8.1.5.2): creates the Topic, DataWriter
    /// (usage writes) and DataReader (usage reads) of every topic in
    /// which the classes of the registered homes have rows, not enabled.
    /// A no-op in the state REGISTERED. On an error no entity of the call
    /// is left and the state stays INITIAL: the DCPS types of all topics
    /// are built before the first entity, and the entities created before
    /// one DCPS refuses are deleted again.
    ///
    /// # Errors
    /// `PreconditionNotMet` without registered home or in the state
    /// ENABLED; `BadHomeDefinition` when a registered class refers to a
    /// class (its parent, the target of a relation) without registered
    /// home; `DCPSError` when DCPS refuses an entity or a topic's type;
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn register_all_for_pubsub(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let mut st = self.state();
        match st.pubsub {
            DcpsState::Registered => return Ok(()),
            DcpsState::Enabled => {
                return Err(DlrlException::precondition(
                    "register_all_for_pubsub in the DCPS state ENABLED",
                ));
            }
            DcpsState::Initial => {}
        }
        let Some(mapping) = st.mapping.clone() else {
            return Err(DlrlException::precondition("no ObjectHome is registered"));
        };
        let classes: Vec<String> = st.homes.iter().map(|h| h.class_name().into()).collect();
        let model = mapping.model();
        for class in &classes {
            let mut needed: Vec<String> = Vec::new();
            if let Some(p) = model.parent(class)
                && !OBJECT_ROOT_NAMES.contains(&p.name.as_str())
            {
                needed.push(p.name.clone());
            }
            if let Some(c) = model.class(class) {
                needed.extend(c.relations.iter().map(|r| r.target.clone()));
            }
            if let Some(missing) = needed.iter().find(|n| !classes.contains(n)) {
                return Err(DlrlException::BadHomeDefinition {
                    message: alloc::format!(
                        "the home of {class} refers to class {missing}, whose home is not registered"
                    ),
                });
            }
        }
        let mut names: Vec<String> = Vec::new();
        for class in &classes {
            for t in mapping.topics_of_class(class) {
                if !names.contains(&t) {
                    names.push(t);
                }
            }
        }
        // The types of all topics first: a topic the DCPS binding refuses
        // leaves no entity behind.
        let mut prepared = Vec::with_capacity(names.len());
        for name in &names {
            let desc = mapping
                .topic(name)
                .cloned()
                .ok_or_else(|| DlrlException::dcps(alloc::format!("no description of {name}")))?;
            let type_ = dcps_binding::topic_type(&desc).map_err(DlrlException::dcps)?;
            let support = SerializedTypeSupport::create_type_support(&type_).map_err(|e| {
                DlrlException::dcps(alloc::format!("type of topic {}: {e}", desc.name))
            })?;
            prepared.push((desc, type_, support));
        }
        let mut bindings: Vec<TopicBinding> = Vec::with_capacity(prepared.len());
        for (desc, type_, support) in prepared {
            let given = st.topic_qos.get(&desc.name).cloned();
            match self.create_binding(desc, type_, support, given) {
                Ok(b) => bindings.push(b),
                // DCPS refused an entity: what was created for the topics
                // before goes again, and a later call starts afresh.
                Err(e) => return Err(self.remove_bindings(&bindings, e)),
            }
        }
        st.topics = bindings;
        st.pubsub = DcpsState::Registered;
        Ok(())
    }

    /// The Topic, DataWriter and DataReader of one topic, not enabled; on
    /// an error, what this call created is deleted again.
    fn create_binding(
        &self,
        desc: TopicDescription,
        type_: DynamicType,
        support: Arc<SerializedTypeSupport>,
        given: Option<GivenTopicQos>,
    ) -> Result<TopicBinding, DlrlException> {
        let asked = given
            .as_ref()
            .map(|given| given.qos.clone())
            .unwrap_or_default();
        let topic = self
            .inner
            .participant
            .create_dynamic_topic(&desc.name, asked, support)
            .map_err(|e| DlrlException::dcps(alloc::format!("topic {}: {e:?}", desc.name)))?;
        // `create_dynamic_topic` returns a topic of the name and type that
        // the DomainParticipant has with the QoS it has (DDS 1.4
        // §2.2.2.2.1.5), not with the one asked for: a QoS given with
        // set_topic_qos that it does not have is refused; one given with
        // set_new_topic_qos was only for a topic created here.
        if let Some(given) = given
            && given.required
            && topic.qos() != given.qos
        {
            let refused = DlrlException::dcps(alloc::format!(
                "the topic {} exists in the DomainParticipant with another QoS than \
                 set_topic_qos gave",
                desc.name
            ));
            return Err(match self.inner.participant.delete_topic(&topic) {
                Ok(()) => refused,
                Err(e) => DlrlException::dcps(alloc::format!(
                    "{refused}; giving the topic back failed: {e:?}"
                )),
            });
        }
        let mut binding = TopicBinding {
            desc,
            type_,
            topic,
            writer: None,
            reader: None,
            table: BTreeMap::new(),
            by_owner: BTreeMap::new(),
        };
        match self.create_endpoints(&mut binding) {
            Ok(()) => Ok(binding),
            Err(e) => Err(self.remove_bindings(core::slice::from_ref(&binding), e)),
        }
    }

    /// The DataWriter (usage writes) and the DataReader (usage reads) of
    /// a topic, set in `binding` as they are created.
    fn create_endpoints(&self, binding: &mut TopicBinding) -> Result<(), DlrlException> {
        let name = binding.desc.name.clone();
        if let Some(p) = &self.inner.publisher {
            binding.writer = Some(
                p.create_datawriter::<DynamicSample>(&binding.topic, DataWriterQos::default())
                    .map_err(|e| {
                        DlrlException::dcps(alloc::format!("DataWriter of {name}: {e:?}"))
                    })?,
            );
        }
        if let Some(s) = &self.inner.subscriber {
            let qos = DataReaderQos {
                reliability: ReliabilityQosPolicy {
                    kind: ReliabilityKind::Reliable,
                    ..ReliabilityQosPolicy::default()
                },
                ..DataReaderQos::default()
            };
            let r = s
                .create_datareader::<DynamicSample>(&binding.topic, qos)
                .map_err(|e| DlrlException::dcps(alloc::format!("DataReader of {name}: {e:?}")))?;
            r.set_listener(
                Some(Arc::new(WakeListener(Arc::clone(&self.inner.wake))) as ArcDataReaderListener),
                DATA_AVAILABLE,
            );
            binding.reader = Some(r);
        }
        Ok(())
    }

    /// Deletes the DataReader, DataWriter and Topic of each binding (the
    /// creation of a later one failed with `cause`) and returns `cause`;
    /// when DCPS refuses a deletion too, an error that names both.
    fn remove_bindings(&self, bindings: &[TopicBinding], cause: DlrlException) -> DlrlException {
        let mut failures: Vec<String> = Vec::new();
        for b in bindings.iter().rev() {
            if let (Some(r), Some(s)) = (&b.reader, &self.inner.subscriber)
                && let Err(e) = s.delete_datareader(r)
            {
                failures.push(alloc::format!("DataReader of {}: {e:?}", b.desc.name));
            }
            if let (Some(w), Some(p)) = (&b.writer, &self.inner.publisher)
                && let Err(e) = p.delete_datawriter(w)
            {
                failures.push(alloc::format!("DataWriter of {}: {e:?}", b.desc.name));
            }
            if let Err(e) = self.inner.participant.delete_topic(&b.topic) {
                failures.push(alloc::format!("topic {}: {e:?}", b.desc.name));
            }
        }
        if failures.is_empty() {
            cause
        } else {
            DlrlException::dcps(alloc::format!(
                "{cause}; deleting what was created before failed: {}",
                failures.join("; ")
            ))
        }
    }

    /// `enable_all_for_pubsub` (§8.1.5.2): enables the Publisher and the
    /// Subscriber, then the DataWriters and DataReaders, which performs
    /// the subscriptions; from then on the incoming updates are applied.
    /// A no-op in the state ENABLED.
    ///
    /// # Errors
    /// `PreconditionNotMet` in the state INITIAL; `DCPSError` when DCPS
    /// refuses to enable an entity; `AlreadyDeleted` for a deleted Cache.
    pub fn enable_all_for_pubsub(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let mut st = self.state();
        match st.pubsub {
            DcpsState::Enabled => return Ok(()),
            DcpsState::Initial => {
                return Err(DlrlException::precondition(
                    "enable_all_for_pubsub needs the DCPS state REGISTERED",
                ));
            }
            DcpsState::Registered => {}
        }
        let dcps = |e: zerodds_dcps::DdsError| DlrlException::dcps(alloc::format!("{e:?}"));
        if let Some(p) = &self.inner.publisher {
            p.enable().map_err(dcps)?;
        }
        if let Some(s) = &self.inner.subscriber {
            s.enable().map_err(dcps)?;
        }
        for b in &st.topics {
            if let Some(w) = &b.writer {
                w.enable().map_err(dcps)?;
            }
            if let Some(r) = &b.reader {
                r.enable().map_err(dcps)?;
            }
        }
        st.pubsub = DcpsState::Enabled;
        let reads = st.topics.iter().any(|b| b.reader.is_some());
        drop(st);
        if reads {
            let weak = Arc::downgrade(&self.inner);
            let wake = Arc::clone(&self.inner.wake);
            let handle = std::thread::Builder::new()
                .name(alloc::format!("dlrl-{}", self.inner.name))
                .spawn(move || update_thread(&weak, &wake))
                .map_err(|e| DlrlException::dcps(alloc::format!("update thread: {e}")))?;
            *self
                .inner
                .thread
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handle);
        }
        Ok(())
    }

    /// `attach_listener`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn attach_listener(&self, listener: Arc<dyn CacheListener>) -> Result<(), DlrlException> {
        self.alive()?;
        let mut st = self.state();
        if !st.listeners.iter().any(|l| Arc::ptr_eq(l, &listener)) {
            st.listeners.push(listener);
        }
        Ok(())
    }

    /// `detach_listener`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn detach_listener(&self, listener: &Arc<dyn CacheListener>) -> Result<(), DlrlException> {
        self.alive()?;
        self.state().listeners.retain(|l| !Arc::ptr_eq(l, listener));
        Ok(())
    }

    /// `enable_updates`: automatic update mode. The updates registered
    /// while the mode was manual are applied as one round, with the
    /// listeners (`on_updates_enabled` first).
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn enable_updates(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let listeners = {
            let mut st = self.state();
            if st.updates_enabled {
                return Ok(());
            }
            st.updates_enabled = true;
            st.listeners.clone()
        };
        for l in &listeners {
            l.on_updates_enabled();
        }
        self.pump();
        Ok(())
    }

    /// `disable_updates`: manual update mode. Incoming updates are taken
    /// and registered for a later `load`, `refresh` or `enable_updates`.
    /// Called in the middle of an update round, it interrupts the round:
    /// [`Cache::last_round_interrupted`] is TRUE when `on_end_updates` is
    /// called. In `on_begin_updates` the updates of the round stay
    /// registered; later in the round they are applied already (a round
    /// applies its updates before the first object listener), and the
    /// listeners hear the rest of it.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn disable_updates(&self) -> Result<(), DlrlException> {
        self.alive()?;
        let listeners = {
            let mut st = self.state();
            if !st.updates_enabled {
                return Ok(());
            }
            st.updates_enabled = false;
            if st.in_round.is_some() {
                st.interrupted = true;
            }
            st.listeners.clone()
        };
        for l in &listeners {
            l.on_updates_disabled();
        }
        Ok(())
    }

    /// `load`: with `updates_enabled` nothing (the updates are applied on
    /// the fly); otherwise the waiting updates are taken and applied, as
    /// an update round without listener. The states of the objects of
    /// such a round hold until the next one.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache; the signature is that of
    /// `refresh` (`DCPSError`).
    pub fn load(&self) -> Result<(), DlrlException> {
        self.alive()?;
        if self.updates_enabled() {
            return Ok(());
        }
        let Some(_round) = self.enter_round() else {
            // Inside a round of this thread: nothing to do; after a
            // delete_cache that closed the gate meanwhile: AlreadyDeleted.
            return self.alive();
        };
        let incoming = self.take_incoming();
        let mut st = self.state();
        if self.is_deleted() {
            // delete_cache by another thread since this one took the
            // gate: the tables are gone, and so are the topics the
            // updates refer to.
            st.pending.clear();
            drop(st);
            return self.alive();
        }
        st.pending.extend(incoming);
        let all = core::mem::take(&mut st.pending);
        Self::clean(&mut st);
        let round = self.apply(&mut st, all);
        st.dirty = round
            .created
            .iter()
            .chain(&round.modified)
            .chain(&round.deleted)
            .cloned()
            .collect();
        // The Selections with auto_refresh follow the round, without
        // listener. A FilterCriterion that refreshes a Selection from
        // check_object does not wait for this round.
        st.in_round = Some(std::thread::current().id());
        drop(st);
        self.examine_selections_quietly(&round);
        self.state().in_round = None;
        Ok(())
    }

    /// The Selections with `auto_refresh` examine the objects of a round
    /// (for `concerns_contained` also the containers), without listener.
    fn examine_selections_quietly(&self, round: &Round) {
        let homes = self.homes();
        let Some(mapping) = homes.first().map(|h| Arc::clone(h.mapping())) else {
            return;
        };
        let all = round
            .created
            .iter()
            .chain(&round.modified)
            .chain(&round.deleted)
            .map(|o| (o, false))
            .chain(round.containers.iter().map(|o| (o, true)));
        for (o, contained_only) in all {
            for c in mapping.model().lineage(&o.class_name()) {
                let Some(home) = homes.iter().find(|h| h.class_name() == c.name) else {
                    continue;
                };
                let selections: Vec<Selection> = home
                    .lock()
                    .selections
                    .iter()
                    .filter(|s| s.auto_refresh() && (!contained_only || s.concerns_contained()))
                    .cloned()
                    .collect();
                for s in &selections {
                    s.examine(o, false);
                }
            }
        }
    }

    /// `refresh` of the Cache (§8.1.6.3.2): its contents with respect to
    /// DCPS; the same as [`Cache::load`].
    ///
    /// # Errors
    /// As [`Cache::load`].
    pub fn refresh(&self) -> Result<(), DlrlException> {
        self.load()
    }

    /// `create_access`.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a purpose that allows writing on a Cache
    /// that does not; `AlreadyDeleted` for a deleted Cache.
    pub fn create_access(&self, purpose: CacheUsage) -> Result<CacheAccess, DlrlException> {
        self.alive()?;
        if purpose.writes() && !self.inner.usage.writes() {
            return Err(DlrlException::precondition(
                "only a Cache that allows writing can create a CacheAccess that allows writing",
            ));
        }
        let access = CacheAccess::new(&self.inner, purpose);
        self.state().accesses.push(access.clone());
        Ok(access)
    }

    /// `delete_access`: purges the access, detaches it and deletes it:
    /// from then on its operations raise `AlreadyDeleted`.
    ///
    /// # Errors
    /// `AlreadyDeleted` for a deleted Cache or a deleted access;
    /// `PreconditionNotMet` for an access this Cache did not create.
    pub fn delete_access(&self, access: &CacheAccess) -> Result<(), DlrlException> {
        self.alive()?;
        access.alive()?;
        {
            let mut st = self.state();
            let before = st.accesses.len();
            st.accesses.retain(|a| a != access);
            if st.accesses.len() == before {
                return Err(DlrlException::precondition(
                    "the CacheAccess was not created by this Cache",
                ));
            }
        }
        access.delete();
        Ok(())
    }

    /// `lock` (§8.1.6.3.4): locks the Cache against all other
    /// modifications, from the infrastructure and from other application
    /// threads, so that several operations see one state of the Cache
    /// (such as the `refresh` of several CacheAccesses). Blocks until the
    /// Cache is allocated to the calling thread, at most
    /// `to_in_milliseconds` (0: only when it is free at once).
    ///
    /// While the lock is held, the update rounds of the Cache's thread,
    /// and `load`, `refresh`, `enable_updates` and `Selection::refresh`
    /// of other threads wait; samples that arrive meanwhile stay in the
    /// DataReaders and form the next round. The calling thread itself may
    /// call them. The lock is the calling thread's: a thread that holds it
    /// may lock again and then unlocks as often.
    ///
    /// # Errors
    /// `ExpiredTimeOut` when the time ran out before the lock could be
    /// granted; `BadParameter` for a negative `to_in_milliseconds`;
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn lock(&self, to_in_milliseconds: i32) -> Result<(), DlrlException> {
        self.alive()?;
        let Ok(ms) = u64::try_from(to_in_milliseconds) else {
            return Err(DlrlException::BadParameter {
                message: alloc::format!(
                    "to_in_milliseconds is {to_in_milliseconds}; a time-out is not negative"
                ),
            });
        };
        if self.inner.gate.acquire(Some(Duration::from_millis(ms))) {
            Ok(())
        } else {
            // Closed by a delete_cache while this thread waited.
            self.alive()?;
            Err(DlrlException::ExpiredTimeOut {
                message: alloc::format!(
                    "the Cache {} was not free within {ms} ms",
                    self.inner.name
                ),
            })
        }
    }

    /// `unlock` (§8.1.6.3.4): gives back one `lock` of the calling thread.
    /// On a Cache deleted while the calling thread held the lock, the
    /// lock is given back and then `AlreadyDeleted` is raised.
    ///
    /// # Errors
    /// `PreconditionNotMet` when the calling thread does not hold the
    /// lock: the operations it meant to protect were not protected;
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn unlock(&self) -> Result<(), DlrlException> {
        let released = self.inner.gate.release();
        self.alive()?;
        if released {
            Ok(())
        } else {
            Err(DlrlException::precondition(
                "the calling thread does not hold the lock of the Cache",
            ))
        }
    }

    /// The transformation of an `ObjectReference` into the corresponding
    /// `ObjectRoot` (§8.1.6.3.4, unnamed in the PIM): the object of the
    /// Cache with this class and oid, with its state loaded (an object of
    /// a home with `auto_deref` FALSE is loaded here, §8.1.6.3.7). The
    /// objects of the Cache are not modifiable: changes are made on a
    /// clone in a CacheAccess that allows writing.
    ///
    /// # Errors
    /// `NotFound` when the Cache holds no object of that class and oid;
    /// `AlreadyDeleted` for a deleted Cache.
    pub fn deref_reference(&self, reference: &ObjectRef) -> Result<ObjectRoot, DlrlException> {
        self.alive()?;
        let o = self.lookup(reference).ok_or_else(|| {
            DlrlException::not_found(alloc::format!(
                "the Cache {} holds no object {} of class {}",
                self.inner.name,
                reference.oid,
                reference.class
            ))
        })?;
        o.ensure_loaded();
        Ok(o)
    }

    /// The state of the object whose rows have owner key `owner`, rebuilt
    /// from the tables; `Ok(None)` when the tables have no main row for
    /// it, `Err` when its rows do not give an object.
    fn try_rebuild(
        st: &mut CacheState,
        mapping: &Mapping,
        owner: &OwnerId,
    ) -> Result<Option<ObjectData>, String> {
        let class = &owner.0;
        let topics = mapping.topics_of_class(class);
        let mut rows: Vec<Row> = Vec::new();
        for b in st.topics.iter().filter(|b| topics.contains(&b.desc.name)) {
            if let Some(keys) = b.by_owner.get(owner) {
                rows.extend(
                    keys.iter()
                        .filter_map(|k| b.table.get(k))
                        .map(|(r, _)| r.clone()),
                );
            }
        }
        if rows.is_empty() {
            return Ok(None);
        }
        mapping
            .reconstruct_with(rows.iter(), &mut st.keys)
            .map(|r| r.objects.into_iter().find(|o| &o.class == class))
            .map_err(|e| alloc::format!("object of class {class}: {e}"))
    }

    /// As [`Cache::try_rebuild`]; a failure is noted in
    /// `rejected_updates`.
    fn rebuild(st: &mut CacheState, owner: &OwnerId) -> Option<ObjectData> {
        let mapping = st.mapping.clone()?;
        match Self::try_rebuild(st, &mapping, owner) {
            Ok(d) => d,
            Err(e) => {
                st.rejected.push(e);
                None
            }
        }
    }

    /// The current state of an object that is not loaded (see
    /// `ObjectRoot::current_data`); `None` when its rows are gone.
    pub(crate) fn state_of(&self, owner: &OwnerId, like: &ObjectData) -> Option<ObjectData> {
        let mut st = self.state();
        Self::rebuild(&mut st, owner).filter(|d| d.oid == like.oid && d.class == like.class)
    }

    /// Loads the state of `o` from the tables (§8.1.6.3.7). An object
    /// whose rows are gone keeps class and oid and counts as loaded.
    pub(crate) fn load_state(&self, o: &ObjectRoot, owner: &OwnerId) {
        let mut st = self.state();
        let data = Self::rebuild(&mut st, owner);
        let mut c = o.lock();
        if c.loaded {
            return;
        }
        if let Some(d) = data.filter(|d| d.oid == c.data.oid && d.class == c.data.class) {
            c.data = d;
        }
        c.loaded = true;
    }

    /// The objects of the Cache whose home is `home`, for `deref_all`
    /// and `underef_all`.
    pub(crate) fn objects_of_home(&self, home: &ObjectHome) -> Vec<ObjectRoot> {
        self.state()
            .objects
            .iter()
            .filter(|((class, _), _)| class == home.class_name())
            .map(|(_, o)| o.clone())
            .collect()
    }

    pub(crate) fn lookup(&self, target: &ObjectRef) -> Option<ObjectRoot> {
        self.state()
            .objects
            .get(&(target.class.clone(), target.oid))
            .cloned()
    }

    pub(crate) fn allocate_oid(&self) -> DlrlOid {
        self.state().keys.allocate()
    }

    /// A fresh oid for the key values `key` of an object of the `NoOid`
    /// class `class`.
    pub(crate) fn associate_new_key(
        &self,
        class: &str,
        key: &[Cell],
    ) -> Result<DlrlOid, DlrlException> {
        let mut st = self.state();
        if let Some(existing) = st.keys.lookup(class, key) {
            return Err(DlrlException::AlreadyExisting {
                message: alloc::format!(
                    "the key values are those of object {existing} of class {class}"
                ),
            });
        }
        let oid = st.keys.allocate();
        st.keys
            .associate(class, oid, key)
            .map_err(|o| DlrlException::AlreadyExisting {
                message: alloc::format!("oid {o} is associated already"),
            })?;
        Ok(oid)
    }

    /// The objects of the Cache the contracts cover, each with its current
    /// state (rebuilt from the tables for an object that is not loaded,
    /// §8.1.6.3.7, without loading it).
    pub(crate) fn closure(&self, contracts: &[Contract]) -> Vec<(ObjectRoot, ObjectData)> {
        let mut st = self.state();
        let mut out: Vec<(ObjectRoot, ObjectData)> = Vec::new();
        let mut seen: BTreeSet<ObjectRef> = BTreeSet::new();
        for contract in contracts {
            let (object, scope, depth) = contract.snapshot();
            let start = object.reference();
            let Some(start) = st.objects.get(&(start.class.clone(), start.oid)) else {
                continue;
            };
            if *start != object {
                continue;
            }
            // (object, relation hops so far)
            let mut stack: Vec<(ObjectRoot, i32)> = alloc::vec![(start.clone(), 0)];
            while let Some((o, hops)) = stack.pop() {
                let (r, data, owner_key, mapping) = {
                    let c = o.lock();
                    (
                        c.data.reference(),
                        c.data.clone(),
                        (!c.loaded).then(|| c.owner_key.clone()).flatten(),
                        Arc::clone(&c.mapping),
                    )
                };
                if !seen.insert(r) {
                    continue;
                }
                let data = match owner_key {
                    Some(k) => Self::rebuild(&mut st, &k).unwrap_or(data),
                    None => data,
                };
                out.push((o.clone(), data.clone()));
                if scope == ObjectScope::SimpleObject {
                    continue;
                }
                for (name, v) in &data.values {
                    let Some((_, Member::Relation(rel))) =
                        mapping.model().member(&data.class, name)
                    else {
                        continue;
                    };
                    let follow = rel.is_composition
                        || (scope == ObjectScope::RelatedObjects
                            && (depth == i32::from(UNLIMITED_RELATED_OBJECTS) || hops < depth));
                    if !follow {
                        continue;
                    }
                    let next_hops = if rel.is_composition { hops } else { hops + 1 };
                    for t in object::references_in(v) {
                        if let Some(target) = st.objects.get(&(t.class.clone(), t.oid)) {
                            stack.push((target.clone(), next_hops));
                        }
                    }
                }
            }
        }
        out
    }

    /// The rows of the object `key` that the Cache knows to be in DCPS:
    /// those it published itself, and those its tables hold for the
    /// object's owner key, received from any participant (this one too).
    /// One row of DCPS may come twice, in two versions.
    fn rows_in_dcps(st: &CacheState, key: &ObjectKey) -> Vec<Row> {
        let mut rows: Vec<Row> = st.published.get(key).cloned().unwrap_or_default();
        let owner = st
            .objects
            .get(key)
            .and_then(|o| o.lock().owner_key.clone())
            .or_else(|| {
                st.owners
                    .iter()
                    .find(|(_, k)| *k == key)
                    .map(|(o, _)| o.clone())
            });
        if let Some(owner) = owner {
            for b in &st.topics {
                if let Some(keys) = b.by_owner.get(&owner) {
                    rows.extend(
                        keys.iter()
                            .filter_map(|k| b.table.get(k))
                            .map(|(r, _)| r.clone()),
                    );
                }
            }
        }
        rows
    }

    /// Publishes the changes of a CacheAccess: the rows of the created and
    /// modified objects that are new or changed are written, the rows an
    /// object no longer has and all rows of a destroyed object are
    /// disposed. The rows an object has in DCPS are those this Cache
    /// published and those its tables hold for it
    /// ([`Cache::rows_in_dcps`]): an object received from another
    /// participant and cloned into the CacheAccess, or a constituent the
    /// destruction of its compound cloned, has its rows disposed as well.
    /// A row another participant wrote is registered with this Cache's
    /// DataWriter before it is disposed (DDS 1.4 §2.2.2.4.2.13 refers to
    /// the precondition of `unregister_instance`: an instance the writer
    /// has not registered).
    pub(crate) fn publish(
        &self,
        changes: &[(ObjectData, ObjectState)],
    ) -> Result<(), DlrlException> {
        let mut st = self.state();
        if st.pubsub != DcpsState::Enabled {
            return Err(DlrlException::precondition(
                "the DCPS state of the Cache is not ENABLED",
            ));
        }
        let Some(publisher) = self.inner.publisher.clone() else {
            return Err(DlrlException::precondition("the Cache is READ_ONLY"));
        };
        let Some(mapping) = st.mapping.clone() else {
            return Err(DlrlException::precondition("no ObjectHome is registered"));
        };
        // The whole plan first: nothing is written when one object fails.
        let mut writes: Vec<Row> = Vec::new();
        let mut disposes: Vec<Row> = Vec::new();
        let mut published: Vec<(ObjectKey, Option<Vec<Row>>)> = Vec::new();
        // A row of DCPS: its topic and key.
        let id = |r: &Row| -> Option<String> {
            let desc = mapping.topic(&r.topic)?;
            Some(alloc::format!(
                "{}|{}",
                r.topic,
                dcps_binding::row_key(desc, r)
            ))
        };
        for (data, state) in changes {
            let key = (data.class.clone(), data.oid);
            let previous = Self::rows_in_dcps(&st, &key);
            let mut gone: BTreeSet<Option<String>> = BTreeSet::new();
            if *state == ObjectState::Deleted {
                disposes.extend(previous.into_iter().filter(|r| gone.insert(id(r))));
                published.push((key, None));
                continue;
            }
            let rows = mapping.rows_of_with(data, &st.keys).map_err(|e| {
                DlrlException::precondition(alloc::format!("object {}: {e}", data.oid))
            })?;
            for old in &previous {
                let i = id(old);
                if !rows.iter().any(|n| id(n) == i) && gone.insert(i) {
                    disposes.push(old.clone());
                }
            }
            for new in &rows {
                // Written unless every version of the row DCPS is known
                // to hold is this one.
                let i = id(new);
                let mut known = previous.iter().filter(|o| id(o) == i).peekable();
                if known.peek().is_none() || known.any(|o| o != new) {
                    writes.push(new.clone());
                }
            }
            published.push((key, Some(rows)));
        }
        let mut samples: Vec<(usize, DynamicSample, bool)> = Vec::new();
        for (row, dispose) in writes
            .iter()
            .map(|r| (r, false))
            .chain(disposes.iter().map(|r| (r, true)))
        {
            let at = st
                .topics
                .iter()
                .position(|b| b.desc.name == row.topic)
                .ok_or_else(|| {
                    DlrlException::precondition(alloc::format!(
                        "topic {} has no DataWriter in this Cache",
                        row.topic
                    ))
                })?;
            let b = &st.topics[at];
            let sample =
                dcps_binding::row_to_sample(&b.desc, &b.type_, row).map_err(DlrlException::dcps)?;
            samples.push((at, sample, dispose));
        }
        let coherent = publisher.get_qos().presentation.coherent_access;
        if coherent {
            publisher
                .begin_coherent_changes()
                .map_err(|e| DlrlException::dcps(alloc::format!("{e:?}")))?;
        }
        let mut result = Ok(());
        for (at, sample, dispose) in &samples {
            let Some(writer) = &st.topics[*at].writer else {
                result = Err(DlrlException::precondition("the topic has no DataWriter"));
                break;
            };
            let r = if *dispose {
                let registered = if writer.lookup_instance(sample).is_nil() {
                    writer.register_instance(sample).map(|_| ())
                } else {
                    Ok(())
                };
                registered.and_then(|()| writer.dispose(sample, HANDLE_NIL))
            } else {
                writer.write(sample)
            };
            if let Err(e) = r {
                result = Err(DlrlException::dcps(alloc::format!(
                    "topic {}: {e:?}",
                    st.topics[*at].desc.name
                )));
                break;
            }
        }
        if coherent {
            let end = publisher
                .end_coherent_changes()
                .map_err(|e| DlrlException::dcps(alloc::format!("{e:?}")));
            result = result.and(end);
        }
        result?;
        for (key, rows) in published {
            match rows {
                Some(rows) => {
                    st.published.insert(key, rows);
                }
                None => {
                    st.published.remove(&key);
                    // A destroyed object the Cache does not hold: no
                    // incoming dispose will release its key (§8.1.4.5).
                    Self::release_unheld_key(&mut st, &key.0, key.1);
                }
            }
        }
        Ok(())
    }

    /// Releases the key association of a `NoOid` object of a CacheAccess
    /// that leaves it without a published state (rolled back by
    /// `refresh`, dropped by `purge` or `delete_access`) (§8.1.4.5).
    pub(crate) fn release_key(&self, class: &str, oid: DlrlOid) {
        Self::release_unheld_key(&mut self.state(), class, oid);
    }

    /// Forgets the key association of the `NoOid` object `class`/`oid`
    /// unless the Cache holds the object (its deletion arrives through
    /// the Cache's DataReader and [`Cache::clean`] forgets it then) or
    /// has published rows of it (the object exists in DCPS).
    fn release_unheld_key(st: &mut CacheState, class: &str, oid: DlrlOid) {
        if oid.is_nil() {
            return;
        }
        let key = (String::from(class), oid);
        if st.objects.contains_key(&key) || st.published.contains_key(&key) {
            return;
        }
        if Self::is_no_oid(st, class) {
            st.keys.forget(class, oid);
        }
    }

    fn is_no_oid(st: &CacheState, class: &str) -> bool {
        st.mapping
            .as_ref()
            .and_then(|m| m.class(class))
            .is_some_and(|cm| cm.main_topic.key.content == KeyContent::NoOid)
    }

    /// Takes the gate for an update round; `None` inside a round of the
    /// calling thread (a listener's call) and once `delete_cache` closed
    /// the gate. Waits while another thread holds the gate: a round, or
    /// the application's `lock`.
    pub(crate) fn enter_round(&self) -> Option<RoundGuard<'_>> {
        if self.state().in_round == Some(std::thread::current().id()) {
            return None;
        }
        self.inner
            .gate
            .acquire(None)
            .then(|| RoundGuard(&self.inner.gate))
    }

    /// Takes the waiting samples of all DataReaders; with a Subscriber of
    /// PRESENTATION `access_scope` GROUP and `coherent_access` or
    /// `ordered_access`, inside one `begin_access`/`end_access` (DDS 1.4
    /// §2.2.2.5.2.8), so that a coherent set across the topics is taken
    /// whole or not at all.
    fn take_incoming(&self) -> Updates {
        let mut st = self.state();
        let mut out = Updates::new();
        if st.pubsub != DcpsState::Enabled {
            // Nothing is received before enable_all_for_pubsub.
            return out;
        }
        let mut rejected = Vec::new();
        let group = self.inner.subscriber.as_ref().filter(|s| {
            let p = s.get_qos().presentation;
            p.access_scope == PresentationAccessScope::Group
                && (p.coherent_access || p.ordered_access)
        });
        let bracket = match group {
            Some(s) => match s.begin_access() {
                Ok(()) => Some(s),
                Err(e) => {
                    // Not taken now: the samples wait for the next round.
                    st.rejected.push(alloc::format!(
                        "begin_access of the Subscriber failed: {e:?}"
                    ));
                    return out;
                }
            },
            None => None,
        };
        for (at, b) in st.topics.iter().enumerate() {
            let Some(reader) = &b.reader else { continue };
            // The samples of one instance come in order: the last one
            // stays (see `Updates`).
            let samples = match reader.take_with_info() {
                Ok(s) => s,
                Err(e) => {
                    rejected.push(alloc::format!("topic {}: take failed: {e:?}", b.desc.name));
                    continue;
                }
            };
            for s in samples {
                if s.info.valid_data {
                    match dcps_binding::sample_to_row(&b.desc, &s.data, false) {
                        Ok(row) => {
                            out.insert((at, dcps_binding::row_key(&b.desc, &row)), Some(row));
                        }
                        Err(e) => rejected.push(alloc::format!("topic {}: {e}", b.desc.name)),
                    }
                } else if s.info.instance_state == InstanceStateKind::NotAliveDisposed {
                    match dcps_binding::sample_to_row(&b.desc, &s.data, true) {
                        Ok(row) => {
                            out.insert((at, dcps_binding::row_key(&b.desc, &row)), None);
                        }
                        Err(e) => rejected.push(alloc::format!(
                            "topic {}: disposed instance without key: {e}",
                            b.desc.name
                        )),
                    }
                }
                // An instance without writers (unregistered, writer gone)
                // that was not disposed: the object stays (§8.1.6.3.8,
                // on_object_deleted is for an object deleted by another
                // participant).
            }
        }
        if let Some(s) = bracket
            && let Err(e) = s.end_access()
        {
            rejected.push(alloc::format!("end_access of the Subscriber failed: {e:?}"));
        }
        for note in rejected {
            st.rejected.push(note);
        }
        out
    }

    /// Takes the waiting samples and, in automatic mode, applies them as
    /// a round with listeners; in manual mode registers them.
    fn pump(&self) {
        let Some(_round) = self.enter_round() else {
            return;
        };
        let incoming = self.take_incoming();
        let listeners = {
            let mut st = self.state();
            if self.is_deleted() {
                // delete_cache by another thread since this one took the
                // gate: the topics the updates refer to are gone.
                st.pending.clear();
                return;
            }
            st.pending.extend(incoming);
            if !st.updates_enabled || st.pending.is_empty() {
                return;
            }
            st.listeners.clone()
        };
        {
            let mut st = self.state();
            st.in_round = Some(std::thread::current().id());
            st.interrupted = false;
        }
        for l in &listeners {
            l.on_begin_updates();
        }
        let round = {
            let mut st = self.state();
            if self.is_deleted() {
                // delete_cache in on_begin_updates (or by another thread
                // meanwhile): the tables and the DCPS entities of the
                // Cache are gone, nothing is applied; on_end_updates
                // closes the round.
                None
            } else if st.updates_enabled {
                let all = core::mem::take(&mut st.pending);
                Self::clean(&mut st);
                Some(self.apply(&mut st, all))
            } else {
                // disable_updates in on_begin_updates: the updates stay
                // registered.
                None
            }
        };
        if let Some(round) = &round {
            self.notify(round);
        }
        for l in &listeners {
            l.on_end_updates();
        }
        let mut st = self.state();
        st.in_round = None;
        if let Some(round) = round {
            st.dirty = round
                .created
                .into_iter()
                .chain(round.modified)
                .chain(round.deleted)
                .collect();
            if st.updates_enabled {
                Self::clean(&mut st);
            }
        }
    }

    /// The end of an update round: NEW and MODIFIED objects become
    /// NOT_MODIFIED, DELETED ones leave the Cache (§8.1.6.3.14).
    fn clean(st: &mut CacheState) {
        for o in core::mem::take(&mut st.dirty) {
            let (state, key, owner_key) = {
                let mut c = o.lock();
                let state = c.read_state;
                if matches!(state, ObjectState::New | ObjectState::Modified) {
                    c.read_state = ObjectState::NotModified;
                }
                c.modified.clear();
                c.previous.clear();
                (
                    state,
                    (c.data.class.clone(), c.data.oid),
                    c.owner_key.clone(),
                )
            };
            if state == ObjectState::Deleted {
                if st.objects.get(&key) == Some(&o) {
                    st.objects.remove(&key);
                }
                if let Some(k) = owner_key
                    && st.owners.get(&k) == Some(&key)
                {
                    st.owners.remove(&k);
                }
                if Self::is_no_oid(st, &key.0) {
                    st.keys.forget(&key.0, key.1);
                }
            }
        }
    }

    fn owner_id(st: &CacheState, desc: &TopicDescription, row: &Row) -> Option<OwnerId> {
        let mapping = st.mapping.as_ref()?;
        let key = row.key(desc);
        let (class, cells): (String, Vec<Option<&Cell>>) = match desc.key_content {
            KeyContent::FullOid => match key.first() {
                Some(Some(Cell::String(c))) if desc.classes.contains(c) => {
                    (c.clone(), key.iter().take(2).copied().collect())
                }
                _ => return None,
            },
            KeyContent::SimpleOid => match desc.classes.as_slice() {
                [c] => (c.clone(), key.iter().take(1).copied().collect()),
                _ => return None,
            },
            KeyContent::NoOid => match desc.classes.as_slice() {
                [c] => {
                    let width = mapping.class(c).map_or(0, |cm| cm.identity.len());
                    (c.clone(), key.iter().take(width).copied().collect())
                }
                _ => return None,
            },
        };
        Some((class, alloc::format!("{cells:?}")))
    }

    /// Applies incoming samples to the tables and the objects.
    ///
    /// An object of a home with `auto_deref` FALSE (§8.1.6.3.7) that is
    /// not loaded stays so: its state before the round is rebuilt from
    /// the tables before the round changes them, to find the modified
    /// attributes, and is not kept. A deleted object keeps its last state
    /// loaded, for the listeners and Selections that hear of its
    /// deletion.
    fn apply(&self, st: &mut CacheState, incoming: Updates) -> Round {
        let mut touched: BTreeSet<OwnerId> = BTreeSet::new();
        // The state before the round of each touched object that is not
        // loaded.
        let mut before: BTreeMap<OwnerId, ObjectData> = BTreeMap::new();
        for ((at, key), row) in incoming {
            if at >= st.topics.len() {
                // Not reached while the callers look at `is_deleted`
                // under the state lock: shutdown takes the topics.
                st.rejected.push(alloc::format!(
                    "an update of row {key} of a topic the Cache no longer has"
                ));
                continue;
            }
            let binding = &st.topics[at];
            let owner = row
                .as_ref()
                .and_then(|r| Self::owner_id(st, &binding.desc, r));
            let old_owner = binding.table.get(&key).and_then(|(_, o)| o.clone());
            for o in [&old_owner, &owner].into_iter().flatten() {
                if touched.insert(o.clone()) {
                    Self::note_unloaded_state(st, o, &mut before);
                }
            }
            let b = &mut st.topics[at];
            if let Some((_, Some(old_owner))) = b.table.remove(&key)
                && let Some(set) = b.by_owner.get_mut(&old_owner)
            {
                set.remove(&key);
            }
            if let Some(row) = row {
                if let Some(o) = &owner {
                    b.by_owner.entry(o.clone()).or_default().insert(key.clone());
                }
                b.table.insert(key, (row, owner));
            }
        }
        let mut round = Round::default();
        let Some(mapping) = st.mapping.clone() else {
            return round;
        };
        for owner in touched {
            let class = owner.0.clone();
            let Some(home) = st.homes.iter().find(|h| h.class_name() == class).cloned() else {
                // A class of the mapping without registered home: no
                // subscription for it (§8.1.6.1.2.1).
                continue;
            };
            let fresh = match Self::try_rebuild(st, &mapping, &owner) {
                Ok(f) => f,
                Err(e) => {
                    st.rejected.push(e);
                    continue;
                }
            };
            let oid = match (&fresh, st.owners.get(&owner)) {
                (Some(o), _) => o.oid,
                // No main row: the object, if any, is deleted.
                (None, Some((_, oid))) => *oid,
                (None, None) => continue,
            };
            let key = (class.clone(), oid);
            // The content filter of the home (§8.1.6.3.7): an incoming
            // object or update that does not pass it is not applied. Its
            // fields do not go across a relation (set_content_filter), so
            // it decides on this object's data alone, whatever the order
            // of the objects in the round.
            let (filter, auto_deref) = {
                let h = home.lock();
                (h.content_filter.clone(), h.auto_deref)
            };
            if let (Some(q), Some(data)) = (&filter, &fresh) {
                let no_other_object = |_: &ObjectRef| -> Option<ObjectData> { None };
                if !q.check(data, &no_other_object) {
                    Self::release_unheld_key(st, &class, oid);
                    continue;
                }
            }
            match (st.objects.get(&key).cloned(), fresh) {
                (None, Some(data)) => {
                    let (data, loaded) = if auto_deref {
                        (data, true)
                    } else {
                        (ObjectData::new(&data.class, data.oid), false)
                    };
                    let o = ObjectRoot::new(ObjectCell {
                        data,
                        read_state: ObjectState::New,
                        write_state: ObjectState::Void,
                        modified: BTreeSet::new(),
                        previous: BTreeMap::new(),
                        home: Arc::downgrade(&home.inner),
                        owner: Owner::Cache(Arc::downgrade(&self.inner)),
                        mapping: Arc::clone(&mapping),
                        registered: true,
                        writable: false,
                        loaded,
                        owner_key: Some(owner.clone()),
                    });
                    st.objects.insert(key.clone(), o.clone());
                    st.owners.insert(owner, key);
                    round.created.push(o);
                }
                (Some(o), Some(data)) => {
                    let changed = {
                        let mut c = o.lock();
                        let old = if c.loaded {
                            c.data.clone()
                        } else {
                            before
                                .remove(&owner)
                                .unwrap_or_else(|| ObjectData::new(&data.class, data.oid))
                        };
                        let changed = access::changed_members(&old, &data);
                        if !changed.is_empty() {
                            for name in &changed {
                                c.previous
                                    .insert(name.clone(), old.values.get(name).cloned());
                            }
                            c.modified.extend(changed.iter().cloned());
                            if c.read_state != ObjectState::New {
                                c.read_state = ObjectState::Modified;
                            }
                        }
                        // auto_deref TRUE: the state is always loaded,
                        // also after underef_all.
                        if c.loaded || auto_deref {
                            c.data = data;
                            c.loaded = true;
                        }
                        !changed.is_empty()
                    };
                    if changed {
                        round.modified.push(o);
                    }
                }
                (Some(o), None) => {
                    let mut c = o.lock();
                    c.read_state = ObjectState::Deleted;
                    if !c.loaded {
                        if let Some(last) = before.remove(&owner) {
                            c.data = last;
                        }
                        c.loaded = true;
                    }
                    drop(c);
                    round.deleted.push(o);
                }
                (None, None) => {}
            }
        }
        // The containers of the changed objects (compositions), for the
        // listeners with concerns_contained_objects.
        let changed: BTreeSet<ObjectRef> = round
            .created
            .iter()
            .chain(&round.modified)
            .chain(&round.deleted)
            .map(ObjectRoot::reference)
            .collect();
        if !changed.is_empty() {
            let compound_classes: BTreeSet<String> = mapping
                .model()
                .classes()
                .iter()
                .filter(|cm| {
                    mapping
                        .model()
                        .lineage(&cm.name)
                        .iter()
                        .any(|c| c.relations.iter().any(|r| r.is_composition))
                })
                .map(|cm| cm.name.clone())
                .collect();
            let candidates: Vec<ObjectRoot> = st
                .objects
                .iter()
                .filter(|((class, _), _)| compound_classes.contains(class))
                .map(|(_, o)| o.clone())
                .collect();
            for o in candidates {
                let (data, owner_key) = {
                    let c = o.lock();
                    (c.loaded.then(|| c.data.clone()), c.owner_key.clone())
                };
                let data = match (data, owner_key) {
                    (Some(d), _) => d,
                    (None, Some(k)) => match Self::rebuild(st, &k) {
                        Some(d) => d,
                        None => continue,
                    },
                    (None, None) => continue,
                };
                let contains = data.values.iter().any(|(name, v)| {
                    matches!(
                        mapping.model().member(&data.class, name),
                        Some((_, Member::Relation(r))) if r.is_composition
                    ) && object::references_in(v).iter().any(|t| changed.contains(t))
                });
                if contains && !round.containers.contains(&o) {
                    round.containers.push(o);
                }
            }
        }
        // The lists of the last round, per home.
        for h in &st.homes {
            let pick = |list: &Vec<ObjectRoot>| -> Vec<ObjectRoot> {
                list.iter()
                    .filter(|o| mapping.model().is_kind_of(&o.class_name(), h.class_name()))
                    .cloned()
                    .collect()
            };
            h.lock().last = RoundLists {
                created: pick(&round.created),
                modified: pick(&round.modified),
                deleted: pick(&round.deleted),
            };
        }
        round
    }

    /// Before a round changes the rows of `owner`: when its object is
    /// not loaded, its state is rebuilt from the rows as they are and
    /// noted in `before`.
    fn note_unloaded_state(
        st: &mut CacheState,
        owner: &OwnerId,
        before: &mut BTreeMap<OwnerId, ObjectData>,
    ) {
        let Some(key) = st.owners.get(owner).cloned() else {
            return;
        };
        let unloaded = st.objects.get(&key).is_some_and(|o| !o.lock().loaded);
        if unloaded && let Some(d) = Self::rebuild(st, owner) {
            before.insert(owner.clone(), d);
        }
    }

    /// The listeners of one round (§8.1.6.4.2 to §8.1.6.4.4). Per
    /// object, the ObjectListeners of its home, then of the parents'
    /// homes, until all listeners of one level answer TRUE; and the
    /// Selections with `auto_refresh` of these homes: for a creation the
    /// ObjectListeners first, for a modification or deletion the
    /// Selections first. The modification of a contained object is a
    /// modification of its container for the listeners and Selections
    /// with `concerns_contained`. Once a listener deleted the Cache, no
    /// other listener of the round is called (but `on_end_updates`): not
    /// the next listener of the same home, not those of the parents'
    /// homes, and no further Selection is examined.
    fn notify(&self, round: &Round) {
        let homes = self.homes();
        let Some(mapping) = homes.first().map(|h| Arc::clone(h.mapping())) else {
            return;
        };
        let chain = |class: &str| -> Vec<ObjectHome> {
            mapping
                .model()
                .lineage(class)
                .into_iter()
                .filter_map(|c| homes.iter().find(|h| h.class_name() == c.name).cloned())
                .collect()
        };
        let dispatch = |o: &ObjectRoot,
                        call: &dyn Fn(&dyn ObjectListener, &ObjectRoot) -> bool,
                        contained_only: bool| {
            for home in chain(&o.class_name()) {
                let listeners: Vec<Arc<dyn ObjectListener>> = home
                    .lock()
                    .listeners
                    .iter()
                    .filter(|(_, concerns)| !contained_only || *concerns)
                    .map(|(l, _)| Arc::clone(l))
                    .collect();
                if listeners.is_empty() {
                    continue;
                }
                let mut all = true;
                for l in &listeners {
                    // A listener deleted the Cache: no more calls of this
                    // round but on_end_updates.
                    if self.is_deleted() {
                        return;
                    }
                    all &= call(l.as_ref(), o);
                }
                if all {
                    break;
                }
            }
        };
        let select = |o: &ObjectRoot, contained_only: bool| {
            for home in chain(&o.class_name()) {
                let selections: Vec<Selection> = home
                    .lock()
                    .selections
                    .iter()
                    .filter(|s| s.auto_refresh() && (!contained_only || s.concerns_contained()))
                    .cloned()
                    .collect();
                for s in &selections {
                    if self.is_deleted() {
                        return;
                    }
                    s.examine(o, true);
                }
            }
        };
        for o in &round.created {
            dispatch(o, &|l, o| l.on_object_created(o), false);
            select(o, false);
        }
        for o in &round.modified {
            select(o, false);
            dispatch(o, &|l, o| l.on_object_modified(o), false);
        }
        for o in &round.deleted {
            select(o, false);
            dispatch(o, &|l, o| l.on_object_deleted(o), false);
        }
        for o in &round.containers {
            if round.modified.contains(o) || round.created.contains(o) {
                continue;
            }
            select(o, true);
            dispatch(o, &|l, o| l.on_object_modified(o), true);
        }
    }

    fn shutdown(&self) -> Result<(), DlrlException> {
        self.inner.wake.stop();
        let handle = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(h) = handle
            && h.thread().id() != std::thread::current().id()
        {
            h.join()
                .map_err(|_| DlrlException::dcps("the update thread panicked"))?;
        }
        let topics = {
            let mut st = self.state();
            // The registered updates refer to the topics by index.
            st.pending.clear();
            core::mem::take(&mut st.topics)
        };
        let err = |e: zerodds_dcps::DdsError| DlrlException::dcps(alloc::format!("{e:?}"));
        if let Some(p) = &self.inner.publisher {
            p.delete_contained_entities().map_err(err)?;
            self.inner.participant.delete_publisher(p).map_err(err)?;
        }
        if let Some(s) = &self.inner.subscriber {
            s.delete_contained_entities().map_err(err)?;
            self.inner.participant.delete_subscriber(s).map_err(err)?;
        }
        for b in topics {
            self.inner.participant.delete_topic(&b.topic).map_err(err)?;
        }
        Ok(())
    }
}

fn update_thread(weak: &Weak<CacheInner>, wake: &Wake) {
    while wake.wait() {
        let Some(inner) = weak.upgrade() else {
            return;
        };
        Cache { inner }.pump();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::model::{AttributeValue, BasicType, ClassModel, Model, SimpleType, Value};
    use zerodds_dcps::runtime::RuntimeConfig;
    use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};

    fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if cond() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for {what}");
    }

    /// §8.1.6.1.2 (`rejected_updates`): the notes of samples and rows that
    /// changed no object are at most `MAX_REJECTED_UPDATES`, the most
    /// recent ones; the first note counts those dropped before; taking
    /// them empties the list.
    #[test]
    fn dlrl_8_1_6_1_2_rejected_notes_are_bounded_and_count_what_they_drop() {
        let mut r = Rejections::default();
        for i in 0..MAX_REJECTED_UPDATES + 5 {
            r.push(alloc::format!("n{i}"));
        }
        let got = r.take();
        assert_eq!(got.len(), MAX_REJECTED_UPDATES + 1);
        assert!(
            got[0].starts_with("5 earlier note(s) dropped"),
            "{}",
            got[0]
        );
        assert_eq!(got[1], "n5");
        assert_eq!(
            got.last().map(String::as_str),
            Some(alloc::format!("n{}", MAX_REJECTED_UPDATES + 4).as_str())
        );
        assert!(r.take().is_empty());
        r.push("one".into());
        assert_eq!(r.take(), ["one"], "the count starts again at zero");
    }

    /// §8.1.6.1.2.2, §8.1.6.3.4 `disable_updates`: in manual mode the
    /// updates taken from the DataReaders and registered for `load` are
    /// one per instance, the last one, however many samples arrived;
    /// `load` applies the last state. A READ_WRITE Cache receives its
    /// own writes.
    #[test]
    fn dlrl_8_1_6_1_2_2_manual_mode_registers_the_last_update_of_each_instance() {
        let model = Model::new(alloc::vec![
            ClassModel::new("Gauge")
                .base(crate::model::OBJECT_ROOT)
                .mono("n", SimpleType::Basic(BasicType::Long)),
        ])
        .unwrap();
        let mapping = Arc::new(Mapping::default_for(model).unwrap());
        let domain = zerodds_test_domains::unique_domain();
        let cfg = RuntimeConfig {
            tick_period: Duration::from_millis(20),
            spdp_period: Duration::from_millis(100),
            nack_response_delay: Duration::ZERO,
            reader_heartbeat_response_delay: Duration::ZERO,
            ..RuntimeConfig::default()
        };
        let participant = DomainParticipantFactory::instance()
            .create_participant_with_config(domain, DomainParticipantQos::default(), cfg)
            .unwrap();
        let cache = CacheFactory::get_instance()
            .create_cache(
                CacheUsage::ReadWrite,
                CacheDescription {
                    name: alloc::format!("manual-{}-{}", std::process::id(), domain),
                    domain: participant,
                },
            )
            .unwrap();
        let home = ObjectHome::new(Arc::clone(&mapping), "Gauge").unwrap();
        cache.register_home(&home).unwrap();
        cache.register_all_for_pubsub().unwrap();
        cache.enable_all_for_pubsub().unwrap();
        cache.disable_updates().unwrap();
        let writer: DynamicDataWriter = cache
            .the_publisher()
            .unwrap()
            .lookup_datawriter("Gauge")
            .unwrap();
        wait_until("the Cache's own subscription", || {
            !writer.get_matched_subscriptions().unwrap().is_empty()
        });
        let registered = |cache: &Cache| -> Vec<Option<i32>> {
            cache
                .state()
                .pending
                .values()
                .map(|r| match r.as_ref().and_then(|r| r.get("n")) {
                    Some(Cell::Long(v)) => Some(*v),
                    _ => None,
                })
                .collect()
        };
        let access = cache.create_access(CacheUsage::ReadWrite).unwrap();
        let gauge = home.create_object(&access).unwrap();
        for n in 1..=5 {
            gauge
                .set("n", AttributeValue::Mono(Value::Long(n)))
                .unwrap();
            access.write().unwrap();
            // Taken by the Cache's thread, which looks every 200 ms.
            wait_until("the update registered", || registered(&cache) == [Some(n)]);
        }
        cache.load().unwrap();
        assert!(cache.state().pending.is_empty());
        let got = home
            .find_object(gauge.oid(), &CacheBase::Cache(cache.clone()))
            .unwrap();
        assert_eq!(got.get("n").unwrap(), AttributeValue::Mono(Value::Long(5)));
        CacheFactory::get_instance().delete_cache(&cache).unwrap();
    }
}
