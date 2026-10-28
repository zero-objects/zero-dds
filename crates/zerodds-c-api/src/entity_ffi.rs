// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Entity operations of the C-FFI: `enable`, `get_status_changes` and
//! `get_instance_handle` for DomainParticipant, Topic, Publisher,
//! Subscriber, DataWriter and DataReader (DDS 1.4 §2.2.2.1.1.6 to
//! §2.2.2.1.1.8), and the enabled/disabled life cycle they rest on
//! (§2.2.2.1.1.7, §2.2.3.20 ENTITY_FACTORY).
//!
//! ## Life cycle
//!
//! A Topic, Publisher, Subscriber, DataWriter or DataReader handle carries a
//! [`Lifecycle`]. It is created enabled when the factory that creates it is
//! enabled and its `entity_factory.autoenable_created_entities` is TRUE, and
//! disabled otherwise ("Entities created from a factory that is disabled, are
//! created disabled regardless of the setting of the ENTITY_FACTORY Qos
//! policy"). A DataWriter or DataReader created disabled holds a reserved
//! `EntityId` and is not yet registered with the runtime: it is not
//! announced over SEDP and not matched. Its first `enable` registers it with
//! the QoS it has by then, as the Rust `DataWriter`/`DataReader` do. The
//! DomainParticipant's state is the one of its `dcps::DomainParticipant`.
//!
//! Operations the specification does not list as callable on a disabled
//! entity return `ZERODDS_NOT_ENABLED` through [`not_enabled`].

use core::ffi::{c_int, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use zerodds_dcps::builtin_topics::TopicBuiltinTopicData;
use zerodds_dcps::entity::Entity;
use zerodds_dcps::instance_handle::InstanceHandle;
use zerodds_rtps::subscription_data::ContentFilterProperty;
use zerodds_rtps::wire_types::{EntityId, Guid};

use crate::ZeroDdsStatus;
use crate::entities::{
    ZeroDdsDataReader, ZeroDdsDataWriter, ZeroDdsDomainParticipant, ZeroDdsPublisher,
    ZeroDdsSubscriber, ZeroDdsTopic,
};

// ============================================================================
// Life cycle
// ============================================================================

/// What an endpoint does once, at its first `enable`: it registers with the
/// runtime. The argument is the ContentFilteredTopic property a reader
/// announces (DDSI-RTPS 2.5 §8.5.3.2), as it is then. An `Err` leaves the
/// entity disabled and the work in place for the next `enable`.
pub type Registration =
    Box<dyn FnMut(Option<ContentFilterProperty>) -> Result<(), ZeroDdsStatus> + Send>;

struct Pending {
    run: Registration,
    content_filter: Option<ContentFilterProperty>,
}

const DISABLED: u8 = 0;
const REGISTERING: u8 = 1;
const ENABLED: u8 = 2;

/// The enabled state of an entity handle (DDS 1.4 §2.2.2.1.1.7).
pub struct Lifecycle {
    state: AtomicU8,
    pending: Mutex<Option<Pending>>,
    /// A ReadCondition or QueryCondition was made on the reader while it was
    /// disabled; its data waker is set at the registration.
    waker_wanted: AtomicBool,
}

impl core::fmt::Debug for Lifecycle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Lifecycle")
            .field("state", &self.state.load(Ordering::Acquire))
            .finish()
    }
}

impl Lifecycle {
    /// A handle that is enabled (or disabled) from its creation and has no
    /// registration work.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self {
            state: AtomicU8::new(if enabled { ENABLED } else { DISABLED }),
            pending: Mutex::new(None),
            waker_wanted: AtomicBool::new(false),
        }
    }

    /// Sets the registration the first `enable` runs, and the content filter
    /// property it announces.
    pub fn set_registration(
        &self,
        run: Registration,
        content_filter: Option<ContentFilterProperty>,
    ) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(Pending {
                run,
                content_filter,
            });
        }
    }

    /// Whether the entity is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.state.load(Ordering::Acquire) == ENABLED
    }

    /// Whether the entity is registered with the runtime or being so.
    #[must_use]
    pub fn is_registered(&self) -> bool {
        self.state.load(Ordering::Acquire) != DISABLED
    }

    /// Notes that the reader's data waker is to be set when it registers.
    pub fn want_waker(&self) {
        self.waker_wanted.store(true, Ordering::Release);
    }

    /// Whether [`Self::want_waker`] was called.
    #[must_use]
    pub fn waker_wanted(&self) -> bool {
        self.waker_wanted.load(Ordering::Acquire)
    }

    /// Replaces the content filter property a still disabled reader will
    /// announce. `false` if there is no registration to patch (the reader is
    /// enabled, or has none).
    pub fn patch_content_filter(&self, property: &ContentFilterProperty) -> bool {
        match self.pending.lock() {
            Ok(mut pending) => match pending.as_mut() {
                Some(p) => {
                    p.content_filter = Some(property.clone());
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }

    /// DDS 1.4 §2.2.2.1.1.7 `enable`, without the check of the factory:
    /// runs the registration, then marks the entity enabled. Idempotent.
    ///
    /// # Errors
    /// The registration's error, with the entity still disabled.
    pub fn enable(&self) -> Result<(), ZeroDdsStatus> {
        let mut pending = self.pending.lock().map_err(|_| ZeroDdsStatus::Error)?;
        if self.is_enabled() {
            return Ok(());
        }
        if let Some(p) = pending.as_mut() {
            self.state.store(REGISTERING, Ordering::Release);
            let filter = p.content_filter.clone();
            if let Err(error) = (p.run)(filter) {
                self.state.store(DISABLED, Ordering::Release);
                return Err(error);
            }
        }
        *pending = None;
        self.state.store(ENABLED, Ordering::Release);
        Ok(())
    }
}

/// A handle with a [`Lifecycle`].
pub trait HasLifecycle {
    /// The handle's life cycle.
    fn lifecycle(&self) -> &Lifecycle;
}

macro_rules! has_lifecycle {
    ($($t:ty),*) => {$(
        impl HasLifecycle for $t {
            fn lifecycle(&self) -> &Lifecycle {
                &self.lifecycle
            }
        }
    )*};
}
has_lifecycle!(
    ZeroDdsTopic,
    ZeroDdsPublisher,
    ZeroDdsSubscriber,
    ZeroDdsDataWriter,
    ZeroDdsDataReader
);

/// `Some(ZERODDS_NOT_ENABLED)` when `handle` is a live, disabled entity: the
/// return code of an operation the specification does not allow on a
/// disabled entity (DDS 1.4 §2.2.2.1.1.7). `None` for an enabled entity and
/// for NULL, which the operation reports itself.
///
/// # Safety
/// `handle` is NULL or a live handle.
pub unsafe fn not_enabled<T: HasLifecycle>(handle: *const T) -> Option<c_int> {
    if handle.is_null() {
        return None;
    }
    // SAFETY: non-null, live per the caller's contract.
    if unsafe { (*handle).lifecycle() }.is_enabled() {
        None
    } else {
        Some(ZeroDdsStatus::NotEnabled as c_int)
    }
}

/// Writes the untriggered status (all fields zero) to `out` and returns
/// `true` when `handle` is a live, disabled entity: "get_status_changes and
/// other get status operations (although the status of a disabled entity
/// never changes)" are callable on a disabled entity (DDS 1.4 §2.2.2.1.1.7),
/// and its statuses are untriggered (§2.2.2.1.1.6). `false` for an enabled
/// entity and for NULL, which the operation reports itself.
///
/// # Safety
/// `handle` is NULL or a live handle; `out` is NULL or writable for an `S`
/// whose zero value is the untriggered status.
pub unsafe fn untriggered_if_disabled<T: HasLifecycle, S>(handle: *const T, out: *mut S) -> bool {
    if handle.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: non-null, live per the caller's contract.
    if unsafe { (*handle).lifecycle() }.is_enabled() {
        return false;
    }
    // SAFETY: `out` is writable per the caller's contract.
    unsafe { core::ptr::write_bytes(out, 0, 1) };
    true
}

/// Whether the entities a participant creates are created enabled: the
/// participant is enabled and ENTITY_FACTORY says so (DDS 1.4 §2.2.3.20).
#[must_use]
pub fn participant_autoenables(pp: &ZeroDdsDomainParticipant) -> bool {
    pp.dp.is_enabled() && pp.dp.qos().entity_factory.autoenable_created_entities
}

/// [`participant_autoenables`] for a publisher and its DataWriters.
#[must_use]
pub fn publisher_autoenables(pp: &ZeroDdsPublisher) -> bool {
    pp.lifecycle.is_enabled()
        && pp
            .qos
            .lock()
            .map(|q| q.entity_factory.autoenable_created_entities)
            .unwrap_or(true)
}

/// [`participant_autoenables`] for a subscriber and its DataReaders.
#[must_use]
pub fn subscriber_autoenables(sb: &ZeroDdsSubscriber) -> bool {
    sb.lifecycle.is_enabled()
        && sb
            .qos
            .lock()
            .map(|q| q.entity_factory.autoenable_created_entities)
            .unwrap_or(true)
}

/// Gives a ContentFilteredTopic parameter change to a reader that is not
/// registered yet: its first announcement then carries the parameters (DDS
/// 1.4 §2.2.2.3.3 `set_expression_parameters`). `true` if `eid` names such a
/// reader of `participant`.
///
/// # Safety
/// `participant` is a live participant handle.
pub unsafe fn patch_pending_reader_filter(
    participant: *mut ZeroDdsDomainParticipant,
    eid: EntityId,
    property: &ContentFilterProperty,
) -> bool {
    if participant.is_null() {
        return false;
    }
    // SAFETY: live per the caller's contract; the lists hold handles from
    // `Box::into_raw`.
    unsafe {
        let subscribers = (*participant)
            .subscribers
            .lock()
            .map(|l| l.clone())
            .unwrap_or_default();
        for sb in subscribers {
            let readers = (*sb)
                .datareaders
                .lock()
                .map(|l| l.clone())
                .unwrap_or_default();
            for dr in readers {
                if (*dr).eid == eid && !(*dr).lifecycle.is_registered() {
                    return (*dr).lifecycle.patch_content_filter(property);
                }
            }
        }
    }
    false
}

// ============================================================================
// Instance handles
// ============================================================================

fn endpoint_handle(rt: &zerodds_dcps::runtime::DcpsRuntime, eid: EntityId) -> InstanceHandle {
    InstanceHandle::from_guid(Guid::new(rt.guid_prefix, eid))
}

fn participant_handle(pp: &ZeroDdsDomainParticipant) -> Option<InstanceHandle> {
    let handle = pp.dp.participant_handle();
    if handle.is_nil() { None } else { Some(handle) }
}

fn topic_handle(t: &ZeroDdsTopic) -> InstanceHandle {
    // The `DCPSTopic` entry of a Topic is one of its participant (DDS 1.4
    // §2.2.5), so the prefix is that of the participant's runtime.
    // SAFETY: a topic's participant handle outlives the topic, per the
    // caller's contract of the FFI.
    let prefix = unsafe { t.participant.as_ref() }
        .and_then(|pp| pp.dp.runtime().map(|rt| rt.guid_prefix))
        .unwrap_or(zerodds_rtps::wire_types::GuidPrefix::from_bytes([0; 12]));
    InstanceHandle::from_guid(TopicBuiltinTopicData::synthesize_key(
        prefix,
        &t.name,
        &t.type_name,
    ))
}

fn publisher_handle(
    pp: &ZeroDdsPublisher,
    rt: &zerodds_dcps::runtime::DcpsRuntime,
) -> Option<InstanceHandle> {
    pp.entity_id.map(|eid| endpoint_handle(rt, eid))
}

fn subscriber_handle(
    sb: &ZeroDdsSubscriber,
    rt: &zerodds_dcps::runtime::DcpsRuntime,
) -> Option<InstanceHandle> {
    sb.entity_id.map(|eid| endpoint_handle(rt, eid))
}

/// Whether `handle` names `pp` or an entity created from it: the
/// participant itself, its topics, publishers, subscribers, DataWriters and
/// DataReaders (DDS 1.4 §2.2.2.2.1.10 `contains_entity`). The handles are
/// the ones `get_instance_handle` returns.
///
/// # Safety
/// `pp` is a live participant handle.
pub unsafe fn contains_entity(pp: *mut ZeroDdsDomainParticipant, handle: InstanceHandle) -> bool {
    if pp.is_null() || handle.is_nil() {
        return false;
    }
    // SAFETY: live per the caller's contract; the lists hold handles from
    // `Box::into_raw`.
    unsafe {
        let p = &*pp;
        if participant_handle(p) == Some(handle) {
            return true;
        }
        let Some(rt) = p.rt.as_ref() else {
            return false;
        };
        let topics = p.topics.lock().map(|l| l.clone()).unwrap_or_default();
        if topics.iter().any(|t| topic_handle(&**t) == handle) {
            return true;
        }
        let publishers = p.publishers.lock().map(|l| l.clone()).unwrap_or_default();
        for publisher in publishers {
            if publisher_handle(&*publisher, rt) == Some(handle) {
                return true;
            }
            let writers = (*publisher)
                .datawriters
                .lock()
                .map(|l| l.clone())
                .unwrap_or_default();
            if writers
                .iter()
                .any(|w| endpoint_handle(rt, (**w).eid) == handle)
            {
                return true;
            }
        }
        let subscribers = p.subscribers.lock().map(|l| l.clone()).unwrap_or_default();
        for subscriber in subscribers {
            if subscriber_handle(&*subscriber, rt) == Some(handle) {
                return true;
            }
            let readers = (*subscriber)
                .datareaders
                .lock()
                .map(|l| l.clone())
                .unwrap_or_default();
            if readers
                .iter()
                .any(|r| endpoint_handle(rt, (**r).eid) == handle)
            {
                return true;
            }
        }
    }
    false
}

fn write_handle(out: *mut u64, handle: Option<InstanceHandle>) -> c_int {
    match handle {
        Some(handle) => {
            // SAFETY: `out` is non-null, checked by every caller.
            unsafe { *out = handle.as_raw() };
            ZeroDdsStatus::Ok as c_int
        }
        None => ZeroDdsStatus::PreconditionNotMet as c_int,
    }
}

fn write_changes(out: *mut u32, changes: u32) -> c_int {
    // SAFETY: `out` is non-null, checked by every caller.
    unsafe { *out = changes };
    ZeroDdsStatus::Ok as c_int
}

fn status_code(error: ZeroDdsStatus) -> c_int {
    error as c_int
}

// ============================================================================
// DomainParticipant
// ============================================================================

/// `enable` of a DomainParticipant (DDS 1.4 §2.2.2.1.1.7): the participant
/// announces itself over SPDP from here; with ENTITY_FACTORY
/// `autoenable_created_entities` it then enables the Topics, Publishers and
/// Subscribers it created, and they theirs. Idempotent. The factory is
/// always enabled, so there is no precondition.
///
/// # Safety
/// `p` is a live participant handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_enable(p: *mut ZeroDdsDomainParticipant) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    let pp = unsafe { &*p };
    if pp.dp.is_enabled() {
        return ZeroDdsStatus::Ok as c_int;
    }
    match pp.dp.enable() {
        Ok(()) => {}
        Err(zerodds_dcps::DdsError::PreconditionNotMet { .. }) => {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        Err(_) => return ZeroDdsStatus::Error as c_int,
    }
    if !pp.dp.qos().entity_factory.autoenable_created_entities {
        return ZeroDdsStatus::Ok as c_int;
    }
    let mut first = ZeroDdsStatus::Ok as c_int;
    let mut note = |rc: c_int| {
        if rc != ZeroDdsStatus::Ok as c_int && first == ZeroDdsStatus::Ok as c_int {
            first = rc;
        }
    };
    let topics = pp.topics.lock().map(|l| l.clone()).unwrap_or_default();
    let publishers = pp.publishers.lock().map(|l| l.clone()).unwrap_or_default();
    let subscribers = pp.subscribers.lock().map(|l| l.clone()).unwrap_or_default();
    // SAFETY: the lists hold live handles from `Box::into_raw`.
    unsafe {
        for t in topics {
            note(zerodds_topic_enable(t));
        }
        for publisher in publishers {
            note(zerodds_pub_enable(publisher));
        }
        for subscriber in subscribers {
            note(zerodds_sub_enable(subscriber));
        }
    }
    first
}

/// `get_status_changes` of a DomainParticipant (DDS 1.4 §2.2.2.1.1.6): the
/// StatusMask of its triggered communication statuses, written to `*out`.
///
/// # Safety
/// `p` is a live participant handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_status_changes(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut u32,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    let pp = unsafe { &*p };
    let changes = if pp.dp.is_enabled() {
        pp.dp.get_status_changes()
    } else {
        0
    };
    write_changes(out, changes)
}

/// `get_instance_handle` of a DomainParticipant (DDS 1.4 §2.2.2.1.1.8): the
/// handle of its participant GUID, which its builtin topic data carries and
/// `contains_entity` accepts. `ZERODDS_PRECONDITION_NOT_MET` for a
/// participant without a runtime.
///
/// # Safety
/// `p` is a live participant handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dp_get_instance_handle(
    p: *mut ZeroDdsDomainParticipant,
    out: *mut u64,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    write_handle(out, participant_handle(unsafe { &*p }))
}

// ============================================================================
// Topic
// ============================================================================

/// `enable` of a Topic (DDS 1.4 §2.2.2.1.1.7). `ZERODDS_PRECONDITION_NOT_MET`
/// while its participant is not enabled.
///
/// # Safety
/// `t` is a live topic handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_enable(t: *mut ZeroDdsTopic) -> c_int {
    if t.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract; `participant` is the
    // creating participant.
    unsafe {
        let topic = &*t;
        if topic.participant.is_null() || !(*topic.participant).dp.is_enabled() {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        match topic.lifecycle.enable() {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(error) => status_code(error),
        }
    }
}

/// `get_status_changes` of a Topic: INCONSISTENT_TOPIC while its count moved
/// since it was read (DDS 1.4 §2.2.4.1); 0 while the Topic is disabled
/// (§2.2.2.1.1.6).
///
/// # Safety
/// `t` is a live topic handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_status_changes(
    t: *mut ZeroDdsTopic,
    out: *mut u32,
) -> c_int {
    status_changes(
        t as *mut c_void,
        out,
        t.is_null() || {
            // SAFETY: non-null, live per the caller's contract.
            !unsafe { (*t).lifecycle.is_enabled() }
        },
    )
}

/// `get_instance_handle` of a Topic: the handle of the key its builtin topic
/// data has.
///
/// # Safety
/// `t` is a live topic handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_topic_get_instance_handle(
    t: *mut ZeroDdsTopic,
    out: *mut u64,
) -> c_int {
    if t.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    write_handle(out, Some(topic_handle(unsafe { &*t })))
}

// ============================================================================
// Publisher
// ============================================================================

/// `enable` of a Publisher. `ZERODDS_PRECONDITION_NOT_MET` while its
/// participant is not enabled. With ENTITY_FACTORY
/// `autoenable_created_entities` it then enables the DataWriters it created.
///
/// # Safety
/// `p` is a live publisher handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_enable(p: *mut ZeroDdsPublisher) -> c_int {
    if p.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let publisher = &*p;
        if publisher.participant.is_null() || !(*publisher.participant).dp.is_enabled() {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if publisher.lifecycle.is_enabled() {
            return ZeroDdsStatus::Ok as c_int;
        }
        if let Err(error) = publisher.lifecycle.enable() {
            return status_code(error);
        }
        if !publisher_autoenables(publisher) {
            return ZeroDdsStatus::Ok as c_int;
        }
        let writers = publisher
            .datawriters
            .lock()
            .map(|l| l.clone())
            .unwrap_or_default();
        let mut first = ZeroDdsStatus::Ok as c_int;
        for dw in writers {
            let rc = zerodds_dw_enable(dw);
            if rc != ZeroDdsStatus::Ok as c_int && first == ZeroDdsStatus::Ok as c_int {
                first = rc;
            }
        }
        first
    }
}

/// `get_status_changes` of a Publisher: a Publisher has no communication
/// status of its own, so 0 (DDS 1.4 §2.2.2.1.1.6, §2.2.4.1).
///
/// # Safety
/// `p` is a live publisher handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_status_changes(
    p: *mut ZeroDdsPublisher,
    out: *mut u32,
) -> c_int {
    status_changes(p as *mut c_void, out, p.is_null())
}

/// `get_instance_handle` of a Publisher: the handle of its Writer Group
/// GUID, which its writers announce as `PID_GROUP_GUID`.
///
/// # Safety
/// `p` is a live publisher handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_pub_get_instance_handle(
    p: *mut ZeroDdsPublisher,
    out: *mut u64,
) -> c_int {
    if p.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let publisher = &*p;
        let handle = if publisher.participant.is_null() {
            None
        } else {
            (*publisher.participant)
                .rt
                .as_ref()
                .and_then(|rt| publisher_handle(publisher, rt))
        };
        write_handle(out, handle)
    }
}

// ============================================================================
// Subscriber
// ============================================================================

/// `enable` of a Subscriber, as [`zerodds_pub_enable`] for its DataReaders.
///
/// # Safety
/// `s` is a live subscriber handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_enable(s: *mut ZeroDdsSubscriber) -> c_int {
    if s.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let subscriber = &*s;
        if subscriber.participant.is_null() || !(*subscriber.participant).dp.is_enabled() {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        if subscriber.lifecycle.is_enabled() {
            return ZeroDdsStatus::Ok as c_int;
        }
        if let Err(error) = subscriber.lifecycle.enable() {
            return status_code(error);
        }
        if !subscriber_autoenables(subscriber) {
            return ZeroDdsStatus::Ok as c_int;
        }
        let readers = subscriber
            .datareaders
            .lock()
            .map(|l| l.clone())
            .unwrap_or_default();
        let mut first = ZeroDdsStatus::Ok as c_int;
        for dr in readers {
            let rc = zerodds_dr_enable(dr);
            if rc != ZeroDdsStatus::Ok as c_int && first == ZeroDdsStatus::Ok as c_int {
                first = rc;
            }
        }
        first
    }
}

/// `get_status_changes` of a Subscriber: DATA_ON_READERS while one of its
/// readers holds data not read yet (DDS 1.4 §2.2.4.1); 0 while the
/// Subscriber is disabled.
///
/// # Safety
/// `s` is a live subscriber handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_status_changes(
    s: *mut ZeroDdsSubscriber,
    out: *mut u32,
) -> c_int {
    status_changes(
        s as *mut c_void,
        out,
        s.is_null() || {
            // SAFETY: non-null, live per the caller's contract.
            !unsafe { (*s).lifecycle.is_enabled() }
        },
    )
}

/// `get_instance_handle` of a Subscriber: the handle of its Reader Group
/// GUID, which its readers announce as `PID_GROUP_GUID`.
///
/// # Safety
/// `s` is a live subscriber handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_sub_get_instance_handle(
    s: *mut ZeroDdsSubscriber,
    out: *mut u64,
) -> c_int {
    if s.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let subscriber = &*s;
        let handle = if subscriber.participant.is_null() {
            None
        } else {
            (*subscriber.participant)
                .rt
                .as_ref()
                .and_then(|rt| subscriber_handle(subscriber, rt))
        };
        write_handle(out, handle)
    }
}

// ============================================================================
// DataWriter
// ============================================================================

/// `enable` of a DataWriter (DDS 1.4 §2.2.2.1.1.7): the first call registers
/// it with the runtime, which announces it over SEDP and matches it.
/// `ZERODDS_PRECONDITION_NOT_MET` while its Publisher is not enabled.
///
/// # Safety
/// `dw` is a live DataWriter handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_enable(dw: *mut ZeroDdsDataWriter) -> c_int {
    if dw.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let writer = &*dw;
        if writer.publisher.is_null() || !(*writer.publisher).lifecycle.is_enabled() {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        match writer.lifecycle.enable() {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(error) => status_code(error),
        }
    }
}

/// `get_status_changes` of a DataWriter: the triggered ones of
/// LIVELINESS_LOST, OFFERED_DEADLINE_MISSED, OFFERED_INCOMPATIBLE_QOS and
/// PUBLICATION_MATCHED (DDS 1.4 §2.2.4.1); 0 while it is disabled.
///
/// # Safety
/// `dw` is a live DataWriter handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_status_changes(
    dw: *mut ZeroDdsDataWriter,
    out: *mut u32,
) -> c_int {
    status_changes(
        dw as *mut c_void,
        out,
        dw.is_null() || {
            // SAFETY: non-null, live per the caller's contract.
            !unsafe { (*dw).lifecycle.is_enabled() }
        },
    )
}

/// `get_instance_handle` of a DataWriter: the handle of its GUID, as
/// `get_matched_publications` of a reader and `SampleInfo.publication_handle`
/// give it.
///
/// # Safety
/// `dw` is a live DataWriter handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dw_get_instance_handle(
    dw: *mut ZeroDdsDataWriter,
    out: *mut u64,
) -> c_int {
    if dw.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    let writer = unsafe { &*dw };
    write_handle(out, Some(endpoint_handle(&writer.rt, writer.eid)))
}

// ============================================================================
// DataReader
// ============================================================================

/// `enable` of a DataReader, as [`zerodds_dw_enable`]. `ZERODDS_PRECONDITION_NOT_MET`
/// while its Subscriber is not enabled.
///
/// # Safety
/// `dr` is a live DataReader handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_enable(dr: *mut ZeroDdsDataReader) -> c_int {
    if dr.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    unsafe {
        let reader = &*dr;
        if reader.subscriber.is_null() || !(*reader.subscriber).lifecycle.is_enabled() {
            return ZeroDdsStatus::PreconditionNotMet as c_int;
        }
        match reader.lifecycle.enable() {
            Ok(()) => ZeroDdsStatus::Ok as c_int,
            Err(error) => status_code(error),
        }
    }
}

/// `get_status_changes` of a DataReader: the triggered ones of its seven
/// statuses, DATA_AVAILABLE while it holds data not read yet (DDS 1.4
/// §2.2.4.1); 0 while it is disabled.
///
/// # Safety
/// `dr` is a live DataReader handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_status_changes(
    dr: *mut ZeroDdsDataReader,
    out: *mut u32,
) -> c_int {
    status_changes(
        dr as *mut c_void,
        out,
        dr.is_null() || {
            // SAFETY: non-null, live per the caller's contract.
            !unsafe { (*dr).lifecycle.is_enabled() }
        },
    )
}

/// `get_instance_handle` of a DataReader: the handle of its GUID, as
/// `get_matched_subscriptions` of a writer gives it.
///
/// # Safety
/// `dr` is a live DataReader handle, `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_dr_get_instance_handle(
    dr: *mut ZeroDdsDataReader,
    out: *mut u64,
) -> c_int {
    if dr.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    // SAFETY: non-null, live per the caller's contract.
    let reader = unsafe { &*dr };
    write_handle(out, Some(endpoint_handle(&reader.rt, reader.eid)))
}

/// The body of the `get_status_changes` entry points: `disabled` (or a NULL
/// entity) reads as no change, otherwise the flags the status machinery has.
fn status_changes(entity: *mut c_void, out: *mut u32, disabled: bool) -> c_int {
    if entity.is_null() || out.is_null() {
        return ZeroDdsStatus::BadHandle as c_int;
    }
    if disabled {
        return write_changes(out, 0);
    }
    // SAFETY: `entity` is a live handle of the kind the entry point named.
    let changes = unsafe { crate::condition_ffi::status_changes_of(entity) };
    write_changes(out, changes)
}
