// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Bubble-up dispatcher for DCPS listeners (Spec §2.2.4.2.3).
//!
//! Spec core: when no listener is set on the "smallest" entity
//! (Reader/Writer/Topic) — or its [`StatusMask`] does not contain the
//! bit of the current status — the event bubbles up one level:
//!
//! ```text
//! DataReader  → Subscriber → DomainParticipant
//! DataWriter  → Publisher  → DomainParticipant
//! Topic       →             → DomainParticipant
//! ```
//!
//! The dispatcher functions in this module encapsulate the resolution
//! per status kind. The most important properties:
//!
//! 1. **Lock discipline**: we clone the listener Arc under the mutex,
//!    release it, and call the callback outside — otherwise there is a
//!    deadlock risk if the callback itself accesses other entities.
//! 2. **Panic safety**: every callback runs inside
//!    [`std::panic::catch_unwind`]; a panicking listener does not bring
//!    down the reader tick. The panic is swallowed — the spec is
//!    agnostic about what happens on a listener panic, but
//!    Cyclone/Fast-DDS behave identically.
//! 3. **Status bits**: we use the constants in
//!    [`crate::psm_constants::status`] for the mask check.
//!
//! Per status kind there is a `dispatch_<event>` function that the hot
//! path code (discovery match, reader cache insert, ...) calls.

#![allow(clippy::module_name_repetitions)]

extern crate alloc;

use alloc::sync::Arc;

use crate::entity::StatusMask;
use crate::instance_handle::InstanceHandle;
use crate::listener::{
    DataReaderListener, DataWriterListener, DomainParticipantListener, PublisherListener,
    SubscriberListener, TopicListener,
};
use crate::psm_constants::status as bits;
use crate::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};

// ============================================================================
// Listener bundles — per hot-path entry we duplicate the listener clones
// here, instead of passing individual references around. This gives the
// dispatch functions a clear, small footprint.
// ============================================================================

/// Listener clones for a DataWriter incl. bubble-up targets. Each field
/// is the tuple `(Listener, Mask)`. If an entry is `None`, there is no
/// listener at that level and the event bubbles up.
pub struct WriterListenerChain {
    /// DataWriter level.
    pub writer: Option<(Arc<dyn DataWriterListener>, StatusMask)>,
    /// Publisher level.
    pub publisher: Option<(Arc<dyn PublisherListener>, StatusMask)>,
    /// DomainParticipant level.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

/// Like [`WriterListenerChain`], but for the reader side.
pub struct ReaderListenerChain {
    /// DataReader level.
    pub reader: Option<(Arc<dyn DataReaderListener>, StatusMask)>,
    /// Subscriber level.
    pub subscriber: Option<(Arc<dyn SubscriberListener>, StatusMask)>,
    /// DomainParticipant level.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

/// Bubble-up chain of a topic (Topic → Participant).
pub struct TopicListenerChain {
    /// Topic level.
    pub topic: Option<(Arc<dyn TopicListener>, StatusMask)>,
    /// DomainParticipant level.
    pub participant: Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
}

/// Whether a stage of a chain takes the status `bit`: it has a listener,
/// and the listener's mask names the status. `takes` of a chain adds that the
/// first such stage is not a 'nil' listener, which reads nothing (DDS 1.4
/// §2.2.4.2.1).
fn stage_takes<L: ?Sized>(stage: &Option<(Arc<L>, StatusMask)>, bit: u32) -> bool {
    stage.as_ref().is_some_and(|(_, mask)| (mask & bit) != 0)
}

impl WriterListenerChain {
    /// Whether a listener of the chain takes the status `bit`.
    #[must_use]
    pub fn takes(&self, bit: u32) -> bool {
        (stage_takes(&self.writer, bit)
            || stage_takes(&self.publisher, bit)
            || stage_takes(&self.participant, bit))
            && !self.nil_takes(bit)
    }
}

impl ReaderListenerChain {
    /// Whether a listener of the chain takes the status `bit`.
    #[must_use]
    pub fn takes(&self, bit: u32) -> bool {
        (stage_takes(&self.reader, bit)
            || stage_takes(&self.subscriber, bit)
            || stage_takes(&self.participant, bit))
            && !self.nil_takes(bit)
    }
}

/// The first stage of a chain whose mask names `bit` decides: `Some(true)`
/// if its listener is the 'nil' listener, `Some(false)` if it is another,
/// `None` if the stage takes nothing of `bit`.
fn stage_nil<L: ?Sized + NilCheck>(stage: &Option<(Arc<L>, StatusMask)>, bit: u32) -> Option<bool> {
    stage
        .as_ref()
        .filter(|(_, mask)| (mask & bit) != 0)
        .map(|(l, _)| l.nil())
}

/// `is_nil` of the listener traits, through one name.
trait NilCheck {
    fn nil(&self) -> bool;
}

impl NilCheck for dyn DataReaderListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}
impl NilCheck for dyn SubscriberListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}
impl NilCheck for dyn DomainParticipantListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}
impl NilCheck for dyn DataWriterListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}
impl NilCheck for dyn PublisherListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}
impl NilCheck for dyn TopicListener {
    fn nil(&self) -> bool {
        self.is_nil()
    }
}

impl WriterListenerChain {
    /// Whether the stage that would take the status `bit` is a 'nil'
    /// listener: the status is then handled by doing nothing, and not read
    /// (DDS 1.4 §2.2.4.3.1).
    #[must_use]
    pub fn nil_takes(&self, bit: u32) -> bool {
        stage_nil(&self.writer, bit)
            .or_else(|| stage_nil(&self.publisher, bit))
            .or_else(|| stage_nil(&self.participant, bit))
            .unwrap_or(false)
    }
}

impl ReaderListenerChain {
    /// As [`WriterListenerChain::nil_takes`]; DATA_ON_READERS starts at the
    /// subscriber, it has no reader stage.
    #[must_use]
    pub fn nil_takes(&self, bit: u32) -> bool {
        let reader = if bit == bits::DATA_ON_READERS {
            None
        } else {
            stage_nil(&self.reader, bit)
        };
        reader
            .or_else(|| stage_nil(&self.subscriber, bit))
            .or_else(|| stage_nil(&self.participant, bit))
            .unwrap_or(false)
    }
}

impl TopicListenerChain {
    /// As [`WriterListenerChain::nil_takes`].
    #[must_use]
    pub fn nil_takes(&self, bit: u32) -> bool {
        stage_nil(&self.topic, bit)
            .or_else(|| stage_nil(&self.participant, bit))
            .unwrap_or(false)
    }
}

// ============================================================================
// Entity form of the listener calls (DDS 1.4 §2.3.3)
// ============================================================================

/// Calls the entity form `$entity_form` of a listener operation with the
/// entity `$handle` names, and the handle form `$handle_form` when no entity
/// has that handle (a handle the caller made up, or an entity that is gone).
macro_rules! call_with_entity {
    ($resolve:ident, $l:expr, $handle:expr, $handle_form:ident, $entity_form:ident $(, $arg:expr)*) => {
        match $resolve($handle) {
            Some(entity) => $l.$entity_form(&entity $(, $arg)*),
            None => $l.$handle_form($handle $(, $arg)*),
        }
    };
}

fn resolve_reader(handle: InstanceHandle) -> Option<crate::subscriber::AnyDataReader> {
    match crate::entity::resolve_entity(handle)? {
        crate::entity::EntityRef::DataReader(reader) => Some(reader),
        _ => None,
    }
}

fn resolve_writer(handle: InstanceHandle) -> Option<crate::publisher::AnyDataWriter> {
    match crate::entity::resolve_entity(handle)? {
        crate::entity::EntityRef::DataWriter(writer) => Some(writer),
        _ => None,
    }
}

fn resolve_subscriber(handle: InstanceHandle) -> Option<crate::subscriber::Subscriber> {
    match crate::entity::resolve_entity(handle)? {
        crate::entity::EntityRef::Subscriber(subscriber) => Some(subscriber),
        _ => None,
    }
}

fn resolve_topic(handle: InstanceHandle) -> Option<crate::topic::TopicDescriptionHandle> {
    match crate::entity::resolve_entity(handle)? {
        crate::entity::EntityRef::Topic(topic) => Some(topic),
        _ => None,
    }
}

// ============================================================================
// Panic-Wrapper
// ============================================================================

/// Calls the closure body and swallows any panic that may occur. That
/// way a faulty listener does not crash the reader tick.
///
/// We wrap with [`std::panic::AssertUnwindSafe`] because `dyn` listener
/// traits have no [`std::panic::UnwindSafe`] bound — the listener body
/// takes responsibility for its own state, and we want process safety,
/// not listener-internal safety.
#[inline]
fn safe_call<F: FnOnce()>(f: F) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

// ============================================================================
// Reader path — Reader → Subscriber → Participant
// ============================================================================

/// Check-and-call helper for the reader path. Returns `true` if the
/// event was consumed at a level.
#[inline]
fn try_call_reader_stage<F>(
    listener: &Option<(Arc<dyn DataReaderListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DataReaderListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_subscriber_stage<F>(
    listener: &Option<(Arc<dyn SubscriberListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn SubscriberListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_participant_stage<F>(
    listener: &Option<(Arc<dyn DomainParticipantListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DomainParticipantListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_writer_stage<F>(
    listener: &Option<(Arc<dyn DataWriterListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn DataWriterListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_publisher_stage<F>(
    listener: &Option<(Arc<dyn PublisherListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn PublisherListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

#[inline]
fn try_call_topic_stage<F>(
    listener: &Option<(Arc<dyn TopicListener>, StatusMask)>,
    bit: u32,
    call: F,
) -> bool
where
    F: FnOnce(Arc<dyn TopicListener>),
{
    if let Some((l, mask)) = listener {
        if (mask & bit) != 0 {
            let l = Arc::clone(l);
            safe_call(move || call(l));
            return true;
        }
    }
    false
}

/// One look of the listener path at a status the runtime keeps in a
/// [`crate::status::StatusLedger`] (DDS 1.4 §2.2.4.1). `observe` reads the
/// ledger's StatusChangedFlag, record and read count together. While the
/// flag is set, its bit is set on `entity` and `dispatch` hands the record
/// to the listener chain. A listener that took it read the status:
/// `acknowledge` takes its changes off, unless a `get_*_status` read them
/// since the look, and the flag is reset. With no listener both stay
/// — for the `get_*_status` read, or for a listener set later, which hears
/// the change at its first look.
///
/// The flag and not a count of moves decides. A count kept here was
/// advanced by a look with no listener attached, and a change that came
/// before `set_listener` — a match a fast discovery made first — was then
/// never handed to the listener at all.
///
/// Two threads look at one status: the listener dispatcher's and the
/// application's, through a `get_*_status` call. Both read the flag
/// before either acknowledged, and a listener was called twice for one
/// change — measured on the Linux test host, `on_sample_rejected` fired
/// twice in 4 of 8 runs of `on_sample_rejected_listener_fires`. So one
/// look at a status runs at a time per entity: every caller asks for a
/// look, one of them runs it, and it runs again for a request that
/// arrived while it was busy. `observe` is read inside the look, never
/// before it, because a value read before another look's acknowledge
/// is the stale one.
pub(crate) fn poll_ledger<S: Clone>(
    mut observe: impl FnMut() -> Option<(bool, S, u64)>,
    entity: &crate::entity::EntityState,
    bit: u32,
    mut takes: impl FnMut() -> bool,
    mut dispatch: impl FnMut(S) -> bool,
    mut acknowledge: impl FnMut(&S, u64),
) {
    // DDS 1.4 §2.2.2.1.1.7: "The Listeners associated with an entity are not
    // called until the entity is enabled."
    if !entity.is_enabled() {
        return;
    }
    entity.request_listener_look(bit);
    loop {
        if !entity.begin_listener_look(bit) {
            // Another look is running; it sees this request.
            return;
        }
        while entity.take_listener_look_request(bit) {
            let Some((unread, record, reads)) = observe() else {
                continue;
            };
            if !unread {
                continue;
            }
            entity.set_status_bits(bit);
            // A getter may have read the status between the look above and
            // the flag: it reads and then clears, so its clear can come
            // before this set and leave a read status flagged (DDS 1.4
            // §2.2.4.1 resets the flag on the read). Looking again after
            // the set closes that: a status read in between is no longer
            // unread here, and the flag goes. Measured on
            // `status_reads::a_status_change_raises_the_flag_and_the_read_lowers_it`,
            // 6 of 80 runs on the Linux test host before.
            if !observe().is_some_and(|(unread, _, _)| unread) {
                entity.clear_status_changes(bit);
                continue;
            }
            // DDS 1.4 §2.2.4.2.1: "if the application calls the get_<plain
            // communication status> from inside the listener it will see the
            // status already reset." The status is read before the listener
            // runs; acknowledged after it, a `get_*_status` in the listener
            // read the changes a second time.
            if takes() {
                acknowledge(&record, reads);
                entity.clear_status_changes(bit);
                let _ = dispatch(record.clone());
            }
        }
        entity.end_listener_look(bit);
        // A request between the last take and the end found the look
        // still running and returned; run it here.
        if !entity.listener_look_requested(bit) {
            return;
        }
    }
}

// ============================================================================
// Reader-path dispatchers
// ============================================================================

/// Bubble-up for `on_data_available` (Reader → Subscriber → Participant).
/// Spec §2.2.4.2.6.1 + §2.2.4.2.3.
pub fn dispatch_data_available(chain: &ReaderListenerChain, reader: InstanceHandle) {
    if chain.nil_takes(bits::DATA_AVAILABLE) {
        return;
    }
    if try_call_reader_stage(&chain.reader, bits::DATA_AVAILABLE, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_data_available,
            on_data_available_entity
        );
    }) {
        return;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::DATA_AVAILABLE, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_data_available,
            on_data_available_entity
        );
    }) {
        return;
    }
    let _ = try_call_participant_stage(&chain.participant, bits::DATA_AVAILABLE, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_data_available,
            on_data_available_entity
        );
    });
}

/// Bubble-up for `on_data_on_readers` (Subscriber → Participant).
/// Spec §2.2.4.2.7.1 — no reader stage, because the event is
/// subscriber-scoped.
///
/// Returns `true` if a listener was called. DDS 1.4 §2.2.4.4: "if
/// on_data_on_readers is called, then the middleware will not try to call
/// on_data_available"; `Subscriber::notify_datareaders` calls it.
pub fn dispatch_data_on_readers(chain: &ReaderListenerChain, subscriber: InstanceHandle) -> bool {
    // A 'nil' listener handles it by doing nothing: no on_data_available
    // follows (DDS 1.4 §2.2.4.4), and the flag is not reset.
    if chain.nil_takes(bits::DATA_ON_READERS) {
        return true;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::DATA_ON_READERS, move |l| {
        call_with_entity!(
            resolve_subscriber,
            l,
            subscriber,
            on_data_on_readers,
            on_data_on_readers_entity
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::DATA_ON_READERS, move |l| {
        call_with_entity!(
            resolve_subscriber,
            l,
            subscriber,
            on_data_on_readers,
            on_data_on_readers_entity
        );
    })
}

/// `on_data_available` on the DataReaderListener alone, without the
/// bubble-up: what `Subscriber::notify_datareaders` invokes (DDS 1.4
/// §2.2.2.5.2.11). Returns `true` if the listener was called.
pub fn dispatch_reader_data_available(chain: &ReaderListenerChain, reader: InstanceHandle) -> bool {
    if chain.nil_takes(bits::DATA_AVAILABLE) {
        return false;
    }
    try_call_reader_stage(&chain.reader, bits::DATA_AVAILABLE, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_data_available,
            on_data_available_entity
        );
    })
}

/// Bubble-up for `on_subscription_matched`.
/// Spec §2.2.4.2.6.7.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_subscription_matched(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SubscriptionMatchedStatus,
) -> bool {
    if chain.nil_takes(bits::SUBSCRIPTION_MATCHED) {
        return false;
    }
    if try_call_reader_stage(&chain.reader, bits::SUBSCRIPTION_MATCHED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_subscription_matched,
            on_subscription_matched_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SUBSCRIPTION_MATCHED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_subscription_matched,
            on_subscription_matched_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::SUBSCRIPTION_MATCHED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_subscription_matched,
            on_subscription_matched_entity,
            status
        );
    })
}

/// Bubble-up for `on_sample_lost`. Spec §2.2.4.2.6.2.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_sample_lost(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SampleLostStatus,
) -> bool {
    if chain.nil_takes(bits::SAMPLE_LOST) {
        return false;
    }
    if try_call_reader_stage(&chain.reader, bits::SAMPLE_LOST, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_lost,
            on_sample_lost_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SAMPLE_LOST, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_lost,
            on_sample_lost_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::SAMPLE_LOST, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_lost,
            on_sample_lost_entity,
            status
        );
    })
}

/// Bubble-up for `on_sample_rejected`. Spec §2.2.4.2.6.3.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_sample_rejected(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: SampleRejectedStatus,
) -> bool {
    if chain.nil_takes(bits::SAMPLE_REJECTED) {
        return false;
    }
    if try_call_reader_stage(&chain.reader, bits::SAMPLE_REJECTED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_rejected,
            on_sample_rejected_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::SAMPLE_REJECTED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_rejected,
            on_sample_rejected_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::SAMPLE_REJECTED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_sample_rejected,
            on_sample_rejected_entity,
            status
        );
    })
}

/// Bubble-up for `on_requested_deadline_missed`. Spec §2.2.4.2.6.4.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_requested_deadline_missed(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: RequestedDeadlineMissedStatus,
) -> bool {
    if chain.nil_takes(bits::REQUESTED_DEADLINE_MISSED) {
        return false;
    }
    if try_call_reader_stage(&chain.reader, bits::REQUESTED_DEADLINE_MISSED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_requested_deadline_missed,
            on_requested_deadline_missed_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_subscriber_stage(
        &chain.subscriber,
        bits::REQUESTED_DEADLINE_MISSED,
        move |l| {
            call_with_entity!(
                resolve_reader,
                l,
                reader,
                on_requested_deadline_missed,
                on_requested_deadline_missed_entity,
                status
            );
        },
    ) {
        return true;
    }
    try_call_participant_stage(
        &chain.participant,
        bits::REQUESTED_DEADLINE_MISSED,
        move |l| {
            call_with_entity!(
                resolve_reader,
                l,
                reader,
                on_requested_deadline_missed,
                on_requested_deadline_missed_entity,
                status
            );
        },
    )
}

/// Bubble-up for `on_requested_incompatible_qos`. Spec §2.2.4.2.6.5.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_requested_incompatible_qos(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: RequestedIncompatibleQosStatus,
) -> bool {
    if chain.nil_takes(bits::REQUESTED_INCOMPATIBLE_QOS) {
        return false;
    }
    let s = status.clone();
    if try_call_reader_stage(&chain.reader, bits::REQUESTED_INCOMPATIBLE_QOS, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_requested_incompatible_qos,
            on_requested_incompatible_qos_entity,
            s
        );
    }) {
        return true;
    }
    let s = status.clone();
    if try_call_subscriber_stage(
        &chain.subscriber,
        bits::REQUESTED_INCOMPATIBLE_QOS,
        move |l| {
            call_with_entity!(
                resolve_reader,
                l,
                reader,
                on_requested_incompatible_qos,
                on_requested_incompatible_qos_entity,
                s
            );
        },
    ) {
        return true;
    }
    let s = status;
    try_call_participant_stage(
        &chain.participant,
        bits::REQUESTED_INCOMPATIBLE_QOS,
        move |l| {
            call_with_entity!(
                resolve_reader,
                l,
                reader,
                on_requested_incompatible_qos,
                on_requested_incompatible_qos_entity,
                s
            );
        },
    )
}

/// Bubble-up for `on_liveliness_changed`. Spec §2.2.4.2.6.6.
///
/// Returns `true` if a listener was called — the status was then read, and
/// its change fields are to be reset by what it was handed.
pub fn dispatch_liveliness_changed(
    chain: &ReaderListenerChain,
    reader: InstanceHandle,
    status: LivelinessChangedStatus,
) -> bool {
    if chain.nil_takes(bits::LIVELINESS_CHANGED) {
        return false;
    }
    if try_call_reader_stage(&chain.reader, bits::LIVELINESS_CHANGED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_liveliness_changed,
            on_liveliness_changed_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_subscriber_stage(&chain.subscriber, bits::LIVELINESS_CHANGED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_liveliness_changed,
            on_liveliness_changed_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::LIVELINESS_CHANGED, move |l| {
        call_with_entity!(
            resolve_reader,
            l,
            reader,
            on_liveliness_changed,
            on_liveliness_changed_entity,
            status
        );
    })
}

// ============================================================================
// Writer-path dispatchers
// ============================================================================

/// Bubble-up for `on_publication_matched`. Spec §2.2.4.2.4.4.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_publication_matched(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: PublicationMatchedStatus,
) -> bool {
    if chain.nil_takes(bits::PUBLICATION_MATCHED) {
        return false;
    }
    if try_call_writer_stage(&chain.writer, bits::PUBLICATION_MATCHED, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_publication_matched,
            on_publication_matched_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_publisher_stage(&chain.publisher, bits::PUBLICATION_MATCHED, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_publication_matched,
            on_publication_matched_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::PUBLICATION_MATCHED, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_publication_matched,
            on_publication_matched_entity,
            status
        );
    })
}

/// Bubble-up for `on_offered_deadline_missed`. Spec §2.2.4.2.4.1.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_offered_deadline_missed(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: OfferedDeadlineMissedStatus,
) -> bool {
    if chain.nil_takes(bits::OFFERED_DEADLINE_MISSED) {
        return false;
    }
    if try_call_writer_stage(&chain.writer, bits::OFFERED_DEADLINE_MISSED, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_offered_deadline_missed,
            on_offered_deadline_missed_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_publisher_stage(&chain.publisher, bits::OFFERED_DEADLINE_MISSED, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_offered_deadline_missed,
            on_offered_deadline_missed_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(
        &chain.participant,
        bits::OFFERED_DEADLINE_MISSED,
        move |l| {
            call_with_entity!(
                resolve_writer,
                l,
                writer,
                on_offered_deadline_missed,
                on_offered_deadline_missed_entity,
                status
            );
        },
    )
}

/// Bubble-up for `on_offered_incompatible_qos`. Spec §2.2.4.2.4.2.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_offered_incompatible_qos(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: OfferedIncompatibleQosStatus,
) -> bool {
    if chain.nil_takes(bits::OFFERED_INCOMPATIBLE_QOS) {
        return false;
    }
    let s = status.clone();
    if try_call_writer_stage(&chain.writer, bits::OFFERED_INCOMPATIBLE_QOS, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_offered_incompatible_qos,
            on_offered_incompatible_qos_entity,
            s
        );
    }) {
        return true;
    }
    let s = status.clone();
    if try_call_publisher_stage(&chain.publisher, bits::OFFERED_INCOMPATIBLE_QOS, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_offered_incompatible_qos,
            on_offered_incompatible_qos_entity,
            s
        );
    }) {
        return true;
    }
    let s = status;
    try_call_participant_stage(
        &chain.participant,
        bits::OFFERED_INCOMPATIBLE_QOS,
        move |l| {
            call_with_entity!(
                resolve_writer,
                l,
                writer,
                on_offered_incompatible_qos,
                on_offered_incompatible_qos_entity,
                s
            );
        },
    )
}

/// Bubble-up for `on_liveliness_lost`. Spec §2.2.4.2.4.3.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_liveliness_lost(
    chain: &WriterListenerChain,
    writer: InstanceHandle,
    status: LivelinessLostStatus,
) -> bool {
    if chain.nil_takes(bits::LIVELINESS_LOST) {
        return false;
    }
    if try_call_writer_stage(&chain.writer, bits::LIVELINESS_LOST, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_liveliness_lost,
            on_liveliness_lost_entity,
            status
        );
    }) {
        return true;
    }
    if try_call_publisher_stage(&chain.publisher, bits::LIVELINESS_LOST, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_liveliness_lost,
            on_liveliness_lost_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::LIVELINESS_LOST, move |l| {
        call_with_entity!(
            resolve_writer,
            l,
            writer,
            on_liveliness_lost,
            on_liveliness_lost_entity,
            status
        );
    })
}

// ============================================================================
// Topic-path dispatchers
// ============================================================================

/// Bubble-up for `on_inconsistent_topic`. Spec §2.2.4.2.5.
///
/// Returns `true` if a listener was called: the status was then read.
pub fn dispatch_inconsistent_topic(
    chain: &TopicListenerChain,
    topic: InstanceHandle,
    status: InconsistentTopicStatus,
) -> bool {
    if chain.nil_takes(bits::INCONSISTENT_TOPIC) {
        return false;
    }
    if try_call_topic_stage(&chain.topic, bits::INCONSISTENT_TOPIC, move |l| {
        call_with_entity!(
            resolve_topic,
            l,
            topic,
            on_inconsistent_topic,
            on_inconsistent_topic_entity,
            status
        );
    }) {
        return true;
    }
    try_call_participant_stage(&chain.participant, bits::INCONSISTENT_TOPIC, move |l| {
        call_with_entity!(
            resolve_topic,
            l,
            topic,
            on_inconsistent_topic,
            on_inconsistent_topic_entity,
            status
        );
    })
}

std::thread_local! {
    /// The thread is calling a reader's data listeners
    /// ([`DataListenerScope`]).
    static IN_DATA_LISTENER: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Marks the thread as calling a reader's data listeners for as long as it
/// lives. A data listener runs on the thread that handed the reader its
/// samples, a receive thread among them, and a RELIABLE write from it
/// must not wait for acknowledgements that thread would have to process
/// ([`in_data_listener`]).
pub(crate) struct DataListenerScope {
    outer: bool,
}

impl DataListenerScope {
    pub(crate) fn enter() -> Self {
        Self {
            outer: IN_DATA_LISTENER.with(|flag| flag.replace(true)),
        }
    }
}

impl Drop for DataListenerScope {
    fn drop(&mut self) {
        IN_DATA_LISTENER.with(|flag| flag.set(self.outer));
    }
}

/// Whether this thread is calling a reader's data listeners.
pub(crate) fn in_data_listener() -> bool {
    IN_DATA_LISTENER.with(core::cell::Cell::get)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU32, Ordering};

    // ---------- Test doubles ----------

    struct CountingReader {
        avail: AtomicU32,
        lost: AtomicU32,
        matched: AtomicU32,
    }
    impl CountingReader {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                lost: AtomicU32::new(0),
                matched: AtomicU32::new(0),
            }
        }
    }
    impl DataReaderListener for CountingReader {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
            self.lost.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingSubscriber {
        avail: AtomicU32,
        on_readers: AtomicU32,
        lost: AtomicU32,
        matched: AtomicU32,
    }
    impl CountingSubscriber {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                on_readers: AtomicU32::new(0),
                lost: AtomicU32::new(0),
                matched: AtomicU32::new(0),
            }
        }
    }
    impl SubscriberListener for CountingSubscriber {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_data_on_readers(&self, _s: InstanceHandle) {
            self.on_readers.fetch_add(1, Ordering::Relaxed);
        }
        fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
            self.lost.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingParticipant {
        avail: AtomicU32,
        on_readers: AtomicU32,
        pub_matched: AtomicU32,
        sub_matched: AtomicU32,
        inconsistent: AtomicU32,
        liveliness_lost: AtomicU32,
    }
    impl CountingParticipant {
        fn new() -> Self {
            Self {
                avail: AtomicU32::new(0),
                on_readers: AtomicU32::new(0),
                pub_matched: AtomicU32::new(0),
                sub_matched: AtomicU32::new(0),
                inconsistent: AtomicU32::new(0),
                liveliness_lost: AtomicU32::new(0),
            }
        }
    }
    impl DomainParticipantListener for CountingParticipant {
        fn on_data_available(&self, _r: InstanceHandle) {
            self.avail.fetch_add(1, Ordering::Relaxed);
        }
        fn on_data_on_readers(&self, _s: InstanceHandle) {
            self.on_readers.fetch_add(1, Ordering::Relaxed);
        }
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.pub_matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_subscription_matched(&self, _r: InstanceHandle, _s: SubscriptionMatchedStatus) {
            self.sub_matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
            self.inconsistent.fetch_add(1, Ordering::Relaxed);
        }
        fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
            self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingWriter {
        matched: AtomicU32,
        liveliness_lost: AtomicU32,
    }
    impl CountingWriter {
        fn new() -> Self {
            Self {
                matched: AtomicU32::new(0),
                liveliness_lost: AtomicU32::new(0),
            }
        }
    }
    impl DataWriterListener for CountingWriter {
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
        fn on_liveliness_lost(&self, _w: InstanceHandle, _s: LivelinessLostStatus) {
            self.liveliness_lost.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingPublisher {
        matched: AtomicU32,
    }
    impl CountingPublisher {
        fn new() -> Self {
            Self {
                matched: AtomicU32::new(0),
            }
        }
    }
    impl PublisherListener for CountingPublisher {
        fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
            self.matched.fetch_add(1, Ordering::Relaxed);
        }
    }

    struct CountingTopic {
        n: AtomicU32,
    }
    impl CountingTopic {
        fn new() -> Self {
            Self {
                n: AtomicU32::new(0),
            }
        }
    }
    impl TopicListener for CountingTopic {
        fn on_inconsistent_topic(&self, _t: InstanceHandle, _s: InconsistentTopicStatus) {
            self.n.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn h() -> InstanceHandle {
        InstanceHandle::from_raw(7)
    }

    // ---------- Reader path ----------

    #[test]
    fn data_available_reader_consumes_when_mask_set() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 1);
        assert_eq!(s.avail.load(Ordering::Relaxed), 0);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_available_bubbles_to_subscriber_when_reader_mask_zero() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.avail.load(Ordering::Relaxed), 1);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_available_bubbles_all_the_way_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(p.avail.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn data_available_skipped_if_no_listener_anywhere() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        // No-op: must not panic.
        dispatch_data_available(&chain, h());
    }

    #[test]
    fn data_available_skipped_if_all_masks_zero() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), 0)),
            participant: Some((p.clone(), 0)),
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.avail.load(Ordering::Relaxed), 0);
        assert_eq!(p.avail.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn data_on_readers_does_not_call_reader_stage() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_data_on_readers(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 0);
        assert_eq!(s.on_readers.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn data_on_readers_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_data_on_readers(&chain, h());
        assert_eq!(p.on_readers.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn subscription_matched_consumed_at_reader() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_subscription_matched(&chain, h(), SubscriptionMatchedStatus::default());
        assert_eq!(r.matched.load(Ordering::Relaxed), 1);
        assert_eq!(s.matched.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn subscription_matched_bubbles_to_subscriber_then_participant() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let p = Arc::new(CountingParticipant::new());

        let chain1 = ReaderListenerChain {
            reader: Some((r.clone(), 0)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_subscription_matched(&chain1, h(), SubscriptionMatchedStatus::default());
        assert_eq!(s.matched.load(Ordering::Relaxed), 1);
        assert_eq!(p.sub_matched.load(Ordering::Relaxed), 0);

        let chain2 = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_subscription_matched(&chain2, h(), SubscriptionMatchedStatus::default());
        assert_eq!(p.sub_matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_lost_chain() {
        let r = Arc::new(CountingReader::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::ANY)),
            subscriber: None,
            participant: None,
        };
        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        assert_eq!(r.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_lost_bubbles_to_subscriber() {
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        assert_eq!(s.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sample_rejected_dispatcher_runs_without_listener() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_sample_rejected(&chain, h(), SampleRejectedStatus::default());
    }

    #[test]
    fn requested_deadline_missed_dispatcher_runs() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_requested_deadline_missed(&chain, h(), RequestedDeadlineMissedStatus::default());
    }

    #[test]
    fn requested_incompatible_qos_dispatcher_clones_status() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_requested_incompatible_qos(&chain, h(), RequestedIncompatibleQosStatus::default());
    }

    #[test]
    fn liveliness_changed_dispatcher_runs() {
        let chain = ReaderListenerChain {
            reader: None,
            subscriber: None,
            participant: None,
        };
        dispatch_liveliness_changed(&chain, h(), LivelinessChangedStatus::default());
    }

    // ---------- Writer path ----------

    #[test]
    fn publication_matched_consumed_at_writer() {
        let w = Arc::new(CountingWriter::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), bits::ANY)),
            publisher: None,
            participant: None,
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn publication_matched_bubbles_to_publisher() {
        let w = Arc::new(CountingWriter::new());
        let pubr = Arc::new(CountingPublisher::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), 0)),
            publisher: Some((pubr.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 0);
        assert_eq!(pubr.matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn publication_matched_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(p.pub_matched.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn liveliness_lost_bubble_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_liveliness_lost(&chain, h(), LivelinessLostStatus::default());
        assert_eq!(p.liveliness_lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn offered_deadline_missed_dispatcher_runs() {
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: None,
        };
        dispatch_offered_deadline_missed(&chain, h(), OfferedDeadlineMissedStatus::default());
    }

    #[test]
    fn offered_incompatible_qos_dispatcher_clones_status() {
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: None,
        };
        dispatch_offered_incompatible_qos(&chain, h(), OfferedIncompatibleQosStatus::default());
    }

    // ---------- Topic path ----------

    #[test]
    fn inconsistent_topic_consumed_at_topic() {
        let t = Arc::new(CountingTopic::new());
        let chain = TopicListenerChain {
            topic: Some((t.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_inconsistent_topic(&chain, h(), InconsistentTopicStatus::default());
        assert_eq!(t.n.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn inconsistent_topic_bubbles_to_participant() {
        let p = Arc::new(CountingParticipant::new());
        let chain = TopicListenerChain {
            topic: None,
            participant: Some((p.clone(), bits::ANY)),
        };
        dispatch_inconsistent_topic(&chain, h(), InconsistentTopicStatus::default());
        assert_eq!(p.inconsistent.load(Ordering::Relaxed), 1);
    }

    // ---------- Panic safety ----------

    #[test]
    fn panic_in_listener_does_not_propagate() {
        struct Panicky;
        impl DataReaderListener for Panicky {
            fn on_data_available(&self, _r: InstanceHandle) {
                panic!("intentional panic in listener");
            }
        }
        let chain = ReaderListenerChain {
            reader: Some((Arc::new(Panicky), bits::ANY)),
            subscriber: None,
            participant: None,
        };
        // If this were not swallowed, the whole test would panic.
        dispatch_data_available(&chain, h());
    }

    #[test]
    fn panic_in_participant_listener_does_not_propagate() {
        struct Panicky;
        impl DomainParticipantListener for Panicky {
            fn on_publication_matched(&self, _w: InstanceHandle, _s: PublicationMatchedStatus) {
                panic!("boom");
            }
        }
        let chain = WriterListenerChain {
            writer: None,
            publisher: None,
            participant: Some((Arc::new(Panicky), bits::ANY)),
        };
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
    }

    // ---------- Bit-specific mask behavior ----------

    #[test]
    fn reader_with_only_data_available_bit_passes_other_events_up() {
        let r = Arc::new(CountingReader::new());
        let s = Arc::new(CountingSubscriber::new());
        let chain = ReaderListenerChain {
            reader: Some((r.clone(), bits::DATA_AVAILABLE)),
            subscriber: Some((s.clone(), bits::ANY)),
            participant: None,
        };
        dispatch_data_available(&chain, h());
        assert_eq!(r.avail.load(Ordering::Relaxed), 1);

        dispatch_sample_lost(&chain, h(), SampleLostStatus::default());
        // The reader listener does NOT have the bit — bubbles to the subscriber.
        assert_eq!(r.lost.load(Ordering::Relaxed), 0);
        assert_eq!(s.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn writer_with_only_publication_matched_bit_passes_other_events_up() {
        let w = Arc::new(CountingWriter::new());
        let pubr = Arc::new(CountingPublisher::new());
        let chain = WriterListenerChain {
            writer: Some((w.clone(), bits::PUBLICATION_MATCHED)),
            publisher: Some((pubr.clone(), 0)),
            participant: None,
        };
        // PUBLICATION_MATCHED → Writer.
        dispatch_publication_matched(&chain, h(), PublicationMatchedStatus::default());
        assert_eq!(w.matched.load(Ordering::Relaxed), 1);
        // LIVELINESS_LOST → bubbles through (set nowhere).
        dispatch_liveliness_lost(&chain, h(), LivelinessLostStatus::default());
        assert_eq!(w.liveliness_lost.load(Ordering::Relaxed), 0);
    }

    /// A change a look found with no listener attached stays pending: a
    /// listener set later hears it at its first look, and once it took the
    /// change nothing is handed over again. A count of moves kept by the
    /// look had consumed it, and a match made before `set_listener` never
    /// reached the listener.
    /// One look at a status runs at a time. A look asked for while one
    /// runs does not read the status beside it; the running look serves
    /// it afterwards. Here the listener reads the status from inside its
    /// callback while a second change arrives — the same-thread form of
    /// the dispatcher thread and a `get_*_status` caller both handing
    /// one change to the listener.
    #[test]
    fn a_look_asked_for_during_a_look_runs_after_it_not_inside_it() {
        use core::cell::Cell;
        struct Probe {
            entity: Arc<crate::entity::EntityState>,
            pending: Cell<u32>,
            handed: Cell<u32>,
            inside: Cell<bool>,
            nested: Cell<bool>,
        }
        fn look(probe: &Probe) {
            poll_ledger(
                || Some((probe.pending.get() > 0, probe.pending.get(), 0)),
                &probe.entity,
                crate::psm_constants::status::SAMPLE_REJECTED,
                || true,
                |_| {
                    assert!(!probe.inside.get(), "the listener was entered twice");
                    probe.inside.set(true);
                    probe.handed.set(probe.handed.get() + 1);
                    if !probe.nested.replace(true) {
                        probe.pending.set(probe.pending.get() + 1);
                        look(probe);
                    }
                    probe.inside.set(false);
                    true
                },
                |reported, _| probe.pending.set(probe.pending.get() - reported),
            );
        }
        let probe = Probe {
            entity: crate::entity::EntityState::new_enabled(),
            pending: Cell::new(1),
            handed: Cell::new(0),
            inside: Cell::new(false),
            nested: Cell::new(false),
        };
        look(&probe);
        assert_eq!(
            (probe.handed.get(), probe.pending.get()),
            (2, 0),
            "both changes handed over, each once"
        );
    }

    /// DDS 1.4 §2.2.4.3.2: "There is no implied event queuing in the
    /// invocation of the listeners … if several changes of status of the same
    /// kind occur in sequence, it is not necessary that the DCPS
    /// implementation delivers one listener callback per unit change …
    /// as long as the LivelinessChangedStatus provided to the listener
    /// corresponds to the most current one." Three writers become alive and
    /// one of them is lost before the look: one call, with the status as it
    /// stands after all four; a change after it is the next call's, with its
    /// own change counts.
    #[test]
    fn a_run_of_changes_is_handed_over_once_with_the_most_current_status() {
        use crate::status::{LivelinessChangedStatus, StatusLedger};
        use core::cell::RefCell;
        let ledger = RefCell::new(StatusLedger::<LivelinessChangedStatus>::default());
        for handle in [1_u64, 2, 3] {
            ledger.borrow_mut().update(|s| {
                s.alive_count += 1;
                s.alive_count_change += 1;
                s.last_publication_handle =
                    crate::instance_handle::InstanceHandle::from_raw(handle);
            });
        }
        ledger.borrow_mut().update(|s| {
            s.alive_count -= 1;
            s.alive_count_change -= 1;
            s.not_alive_count += 1;
            s.not_alive_count_change += 1;
            s.last_publication_handle = crate::instance_handle::InstanceHandle::from_raw(2);
        });
        let entity = crate::entity::EntityState::new_enabled();
        let bit = crate::psm_constants::status::LIVELINESS_CHANGED;
        let calls = RefCell::new(Vec::new());
        let look = || {
            poll_ledger(
                || Some(ledger.borrow().observe()),
                &entity,
                bit,
                || true,
                |status| {
                    calls.borrow_mut().push(status);
                    true
                },
                |status, reads| ledger.borrow_mut().acknowledge(status, reads),
            );
        };
        look();
        {
            let calls = calls.borrow();
            assert_eq!(calls.len(), 1, "one call for the run of four changes");
            let status = calls[0];
            assert_eq!((status.alive_count, status.not_alive_count), (2, 1));
            assert_eq!(
                (status.alive_count_change, status.not_alive_count_change),
                (2, 1)
            );
            assert_eq!(status.last_publication_handle.as_raw(), 2);
        }
        // Nothing changed: no call.
        look();
        assert_eq!(calls.borrow().len(), 1);
        // A change after the look is the next call's.
        ledger.borrow_mut().update(|s| {
            s.alive_count += 1;
            s.alive_count_change += 1;
            s.last_publication_handle = crate::instance_handle::InstanceHandle::from_raw(4);
        });
        look();
        let calls = calls.borrow();
        assert_eq!(calls.len(), 2);
        assert_eq!((calls[1].alive_count, calls[1].alive_count_change), (3, 1));
        assert_eq!(calls[1].not_alive_count_change, 0);
    }

    /// DDS 1.4 §2.2.4.2.1: "if the application calls the get_<plain
    /// communication status> from inside the listener it will see the
    /// status already reset." The look acknowledges the change and lowers
    /// the flag before it calls the listener.
    #[test]
    fn the_status_is_read_before_the_listener_runs() {
        let entity = crate::entity::EntityState::new_enabled();
        let bit = crate::psm_constants::status::PUBLICATION_MATCHED;
        let acknowledged = core::cell::Cell::new(0);
        let called = core::cell::Cell::new(false);
        poll_ledger(
            || Some((true, 1_u32, 0)),
            &entity,
            bit,
            || true,
            |_| {
                assert_eq!(acknowledged.get(), 1, "read before the call");
                assert_eq!(entity.status_changes() & bit, 0, "flag lowered");
                called.set(true);
                true
            },
            |_, _| acknowledged.set(acknowledged.get() + 1),
        );
        assert!(called.get());
    }

    #[test]
    fn a_change_no_listener_took_reaches_a_listener_set_later() {
        let entity = crate::entity::EntityState::new_enabled();
        let bit = crate::psm_constants::status::SUBSCRIPTION_MATCHED;
        let acknowledged = core::cell::Cell::new(0);
        let handed = core::cell::Cell::new(0);

        // No listener: the flag is set and the change stays.
        poll_ledger(
            || Some((true, 1_u32, 0)),
            &entity,
            bit,
            || false,
            |_| false,
            |_, _| acknowledged.set(acknowledged.get() + 1),
        );
        assert_ne!(entity.status_changes() & bit, 0);
        assert_eq!(acknowledged.get(), 0);

        // A listener now: it takes the change, which is then read.
        poll_ledger(
            || Some((true, 1_u32, 0)),
            &entity,
            bit,
            || true,
            |_| {
                handed.set(handed.get() + 1);
                true
            },
            |_, _| acknowledged.set(acknowledged.get() + 1),
        );
        assert_eq!((handed.get(), acknowledged.get()), (1, 1));
        assert_eq!(entity.status_changes() & bit, 0);

        // Read: nothing is handed over again.
        poll_ledger(
            || Some((false, 1_u32, 0)),
            &entity,
            bit,
            || true,
            |_| {
                handed.set(handed.get() + 1);
                true
            },
            |_, _| acknowledged.set(acknowledged.get() + 1),
        );
        assert_eq!((handed.get(), acknowledged.get()), (1, 1));
    }
}
