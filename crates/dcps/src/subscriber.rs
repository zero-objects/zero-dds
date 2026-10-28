// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Subscriber + DataReader — the receive end of the DCPS API.
//!
//! Spec reference: OMG DDS 1.4 §2.2.2.5 `Subscriber`, §2.2.2.5.2
//! `DataReader`.
//!
//! # Scope v1.2
//!
//! - `Subscriber::create_datareader<T>(topic, qos)` → `DataReader<T>`.
//! - `DataReader::take()` removes all cached samples.
//! - `DataReader::read()` peeks without removing (offline: identical to
//!   take, no state change — spec §2.2.2.5.3.4 sample-state is
//!   implemented in live mode).
//! - Listener / WaitSet: live mode.

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::marker::PhantomData;

#[cfg(feature = "std")]
use std::sync::Mutex;
#[cfg(feature = "std")]
use std::sync::mpsc;

use crate::dds_type::DdsType;
use crate::entity::StatusMask;
use crate::error::{DdsError, Result};
#[cfg(feature = "std")]
use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
#[cfg(feature = "std")]
use crate::instance_tracker::InstanceTracker;
use crate::listener::{ArcDataReaderListener, ArcSubscriberListener};
use crate::qos::{DataReaderQos, EndpointQosArg, SubscriberQos};
#[cfg(feature = "std")]
use crate::sample::Sample;
#[cfg(feature = "std")]
use crate::sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind, instance_state_mask,
    sample_state_mask, view_state_mask,
};
#[cfg(feature = "std")]
use crate::time::{Time, get_current_time};
use crate::topic::Topic;

#[cfg(feature = "std")]
use crate::runtime::DcpsRuntime;
#[cfg(feature = "std")]
use zerodds_qos::ReliabilityKind;
#[cfg(feature = "std")]
use zerodds_rtps::wire_types::EntityId;

/// Decodes a received sample body with the encoder that matches its
/// **encapsulation** — both the XCDR version (`representation`: `0` = XCDR1 /
/// classic CDR, `1` = XCDR2) and the byte order (`big_endian`), as extracted
/// from the RTPS encapsulation header (RTPS 2.5 §10.5) when the sample was
/// staged (`UserSample::Alive`).
///
/// This is essential for cross-vendor interop: CycloneDDS (and legacy RTI /
/// OpenDDS) default their writers to **XCDR1** for `@final` types, so a ZeroDDS
/// reader must decode XCDR1 with the classic-CDR alignment rule (max-align 8, no
/// DHEADER) rather than the XCDR2 rule (max-align 4) — otherwise every body with
/// an 8-byte member or an `@appendable` DHEADER would be mis-aligned. The
/// per-representation framing lives in `DdsType::{decode, decode_be,
/// decode_xcdr1, decode_xcdr1_be}` (XTypes 1.3 §7.4.3.4.2).
#[inline]
fn decode_for_encap<T: DdsType>(
    codec: &dyn crate::topic::TopicCodec<T>,
    bytes: &[u8],
    representation: u8,
    big_endian: bool,
) -> core::result::Result<T, crate::dds_type::DecodeError> {
    let body = if representation == 0 {
        bytes
    } else {
        delimited_extent_for(bytes, big_endian, codec.extensibility())?
    };
    codec.decode(body, representation, big_endian)
}

/// Narrows an XCDR2 sample body to the extent its DHEADER declares.
///
/// XTypes 1.3 §7.4.3.4.2: a DELIMITED_CDR (`@appendable`) or PL_CDR2
/// (`@mutable`) object begins with a `uint32` DHEADER stating the length of its
/// content, and that length bounds the object — a member reaching past it is not
/// a member of this object. The decoder generated for such a type reads the
/// DHEADER and then reads members while it has not reached the declared end, so
/// nothing stops it from reading a member that starts inside the extent and ends
/// outside it. Handing it a slice that ends at the extent turns exactly that
/// case into the truncation error it is.
///
/// This is the cross-vendor case behind issue #30: a `@final` peer's XCDR2 body
/// carries no DHEADER, so an `@appendable` reader takes the peer's first member
/// for one. With a `Robot{id: 1, …}` from CycloneDDS the reader read a DHEADER
/// of 1, then read `label`'s octets as `id` and defaulted `label` — a sample the
/// application could not tell from a correct one. Bounding the slice makes the
/// same body fail to decode.
///
/// `@final` bodies have no DHEADER and pass through untouched.
fn delimited_extent_for(
    bytes: &[u8],
    big_endian: bool,
    extensibility: crate::dds_type::Extensibility,
) -> core::result::Result<&[u8], crate::dds_type::DecodeError> {
    use crate::dds_type::{DHEADER_LEN, Extensibility};
    if matches!(extensibility, Extensibility::Final) {
        return Ok(bytes);
    }
    let Some(head) = bytes.get(..DHEADER_LEN) else {
        // Shorter than the DHEADER it must begin with: not a delimited object.
        return Err(crate::dds_type::DecodeError::Invalid {
            what: "XCDR2 body of a delimited type is shorter than its DHEADER",
        });
    };
    let mut raw = [0u8; DHEADER_LEN];
    raw.copy_from_slice(head);
    let declared = if big_endian {
        u32::from_be_bytes(raw)
    } else {
        u32::from_le_bytes(raw)
    } as usize;
    // The trailing octets a body may have beyond the extent are the RTPS
    // padding to the next 4-octet boundary; a DHEADER that reaches past the body
    // describes an object that was never received.
    bytes
        .get(..DHEADER_LEN.saturating_add(declared))
        .ok_or(crate::dds_type::DecodeError::Invalid {
            what: "DHEADER declares more content than the sample body holds",
        })
}

/// Builds a diagnostic `WireError` for a failed sample decode. Beyond the inner
/// error it records the received encapsulation (representation + byte order) and
/// this reader type's own extensibility, and — because the most common
/// cross-vendor cause is an extensibility/framing mismatch — names that as a
/// *plausible* cause. It is deliberately not asserted: without the remote type
/// the true cause cannot be confirmed here. See issue #27.
/// A sample the receiving type discarded, recorded so that the drop is
/// visible.
///
/// XTypes 1.3 §7.2.4.1 makes a discarded sample an **absence** and not
/// an error — "the default behavior is to discard the O2 object to
/// avoid misinterpretation" — so it must not fail the `take()` around
/// it. That makes it a drop, and a drop nobody can see is the thing
/// this tree does not do: it goes out at `Debug`, once per sample, with
/// the reader's type and the position that did not fit.
///
/// `Info` and not `Warn` deliberately. `Level::Warn` is "an abnormal
/// but non-fatal situation" and this is neither abnormal nor a
/// situation: the two types are compatible, the annotation asked for
/// exactly this, and a reader narrowing a high-rate topic would drown a
/// warning. The signal the application acts on is the sample simply not
/// arriving, which is what the OMG suite checks (`DATA_NOT_RECEIVED`).
#[cfg(feature = "std")]
fn record_discarded_sample<T: DdsType>(
    runtime: Option<&Arc<DcpsRuntime>>,
    inner: &crate::dds_type::DecodeError,
) {
    let Some(rt) = runtime else {
        return;
    };
    rt.config.observability.record(
        &zerodds_foundation::observability::Event::new(
            zerodds_foundation::observability::Level::Info,
            zerodds_foundation::observability::Component::Dcps,
            "sample.discarded",
        )
        .with_attr("type", T::type_name())
        .with_attr("reason", alloc::string::ToString::to_string(inner).as_str()),
    );
}

fn decode_wire_error<T: DdsType>(
    inner: &crate::dds_type::DecodeError,
    representation: u8,
    big_endian: bool,
) -> DdsError {
    use crate::dds_type::Extensibility;
    let repr = if representation == 0 {
        "XCDR1"
    } else {
        "XCDR2"
    };
    let endian = if big_endian {
        "big-endian"
    } else {
        "little-endian"
    };
    let ext = match T::extensibility() {
        Extensibility::Final => "final",
        Extensibility::Appendable => "appendable",
        Extensibility::Mutable => "mutable",
    };
    DdsError::WireError {
        message: alloc::format!(
            "decode error: {inner} (received {repr} {endian}; this reader's type '{}' is @{ext}. \
             A plausible cross-vendor cause is an extensibility mismatch — in XCDR2 \
             @appendable/@mutable carry a DHEADER length prefix and @final does not, so a peer \
             whose type has a different extensibility fails to decode. Not confirmed: the remote \
             type is not available here to verify.)",
            T::type_name(),
        ),
    }
}

/// Subscriber — entity group for DataReaders.
#[derive(Debug)]
pub struct Subscriber {
    pub(crate) inner: Arc<SubscriberInner>,
}

pub(crate) struct SubscriberInner {
    #[cfg(feature = "std")]
    pub(crate) qos: std::sync::Mutex<SubscriberQos>,
    #[cfg(not(feature = "std"))]
    #[allow(dead_code)]
    pub(crate) qos: SubscriberQos,
    pub(crate) entity_state: alloc::sync::Arc<crate::entity::EntityState>,
    /// The QoS `create_datareader` takes for `DATAREADER_QOS_DEFAULT`, and
    /// `set_qos` of a DataReader of this subscriber (DDS 1.4
    /// §2.2.2.5.2.15).
    #[cfg(feature = "std")]
    default_datareader_qos: std::sync::Mutex<DataReaderQos>,
    #[cfg(feature = "std")]
    pub(crate) runtime: Option<Arc<DcpsRuntime>>,
    /// Optional `SubscriberListener` + StatusMask.
    /// Bubble-up target for reader events.
    #[cfg(feature = "std")]
    pub(crate) listener: std::sync::Mutex<Option<(ArcSubscriberListener, StatusMask)>>,
    /// Weak back-pointer to the participant (bubble-up, cycle avoidance
    /// via Weak).
    #[cfg(feature = "std")]
    pub(crate) participant:
        std::sync::Mutex<Option<alloc::sync::Weak<crate::participant::ParticipantInner>>>,
    /// Group access scope for §2.2.2.5.2.8/.9 begin/end_access.
    /// Counter-based (recursively nestable per spec).
    pub(crate) access_scope: Arc<crate::coherent_set::GroupAccessScope>,
    /// The DataReaders this subscriber created, held weakly, for
    /// `lookup_datareader`, `get_datareaders`, `notify_datareaders`,
    /// `delete_contained_entities` and the participant's `contains_entity`
    /// (DDS 1.4 §2.2.2.5.2.7, .10, .11, .14, §2.2.2.2.1.10).
    #[cfg(feature = "std")]
    pub(crate) readers: crate::contained::ContainedList<dyn crate::contained::ContainedReader>,
    /// The runtime entity ids of the live DataReaders of this subscriber,
    /// which a `set_qos` of its PARTITION or GROUP_DATA reaches.
    #[cfg(feature = "std")]
    pub(crate) reader_entity_ids: std::sync::Mutex<alloc::vec::Vec<EntityId>>,
    /// The Subscriber's `EntityId`, a Reader Group (DDSI-RTPS 2.5 Table
    /// 9.1); its readers announce it as `PID_GROUP_GUID` (Table 9.19).
    #[cfg(feature = "std")]
    pub(crate) entity_id: Option<EntityId>,
    /// With `PRESENTATION.access_scope = GROUP` and ordered or coherent
    /// access, the history where samples of a GROUP-scope publisher's
    /// writers are ordered across its readers (DDSI-RTPS 2.5 §8.7.5,
    /// §8.7.6).
    #[cfg(feature = "std")]
    group_order: crate::presentation::GroupOrdering<InstanceHandle>,
    /// The hook the runtime runs on group progress, held here; the runtime
    /// holds it weakly.
    #[cfg(feature = "std")]
    group_hook: Mutex<Option<crate::runtime::StatusHook>>,
}

#[cfg(feature = "std")]
impl SubscriberInner {
    /// DDS 1.4 §2.2.4.2.2: DATA_ON_READERS "becomes FALSE when any of the
    /// following events occurs: The corresponding listener operation
    /// (on_data_on_readers) is called. The on_data_available listener
    /// operation is called on any DataReader belonging to the Subscriber.
    /// The read or take operation (or their variants) is called on any
    /// DataReader belonging to the Subscriber." Covers what each reader
    /// has now; `reading`, a reader that reads, resets its own part itself.
    pub(crate) fn cover_data_on_readers(&self, reading: Option<InstanceHandle>) {
        for reader in self.readers.live() {
            if Some(reader.handle()) != reading {
                reader.cover_data_on_readers();
            }
        }
    }

    /// DATA_ON_READERS (§2.2.4.2.2): "becomes TRUE when any of the
    /// DataReader entities that belong to the Subscriber have their
    /// DATA_AVAILABLE StatusChangedFlag become TRUE", and stays so until
    /// [`Self::cover_data_on_readers`].
    fn data_on_readers(&self) -> bool {
        self.readers
            .live()
            .iter()
            .any(|reader| reader.has_data_on_readers())
    }

    /// `PRESENTATION.access_scope` is GROUP with ordered or coherent access:
    /// the subscriber orders a GROUP-scope publisher's samples across its
    /// readers (DDS 1.4 §2.2.3.6, DDSI-RTPS 2.5 §8.7.5).
    pub(crate) fn group_order_active(&self) -> bool {
        self.qos.lock().is_ok_and(|q| {
            matches!(
                q.presentation.access_scope,
                zerodds_qos::PresentationAccessScope::Group
            ) && (q.presentation.ordered_access || q.presentation.coherent_access)
        })
    }

    /// Takes `item` into the group order when it is a sample of a
    /// GROUP-scope publisher's writer and the subscriber orders by group;
    /// gives it back otherwise.
    pub(crate) fn offer_to_group(
        &self,
        reader: InstanceHandle,
        item: crate::runtime::UserSample,
    ) -> Option<crate::runtime::UserSample> {
        if !self.group_order_active() {
            return Some(item);
        }
        let Some(rt) = self.runtime.as_ref() else {
            return Some(item);
        };
        self.group_order.offer(rt, reader, item)
    }

    /// How many samples for `reader` the group order holds back.
    pub(crate) fn group_held_for(&self, reader: InstanceHandle) -> usize {
        self.group_order.held_for(&reader)
    }

    /// Hands what waits on the channels of the subscriber's readers other
    /// than `me` to the group order.
    pub(crate) fn pump_group(&self, me: InstanceHandle) {
        for reader in self.readers.live() {
            if reader.handle() != me {
                reader.pump_group_channel();
            }
        }
    }

    /// Commits what the group order allows and hands each sample to its
    /// reader's inbox, under the group order's lock
    /// ([`crate::presentation::GroupOrdering::release`]).
    ///
    /// The samples of a reader went to its inbox from the group hook and
    /// back to the caller from a reader's own ingest, and a reader that had
    /// drained its inbox before a hook released its earlier sample staged
    /// its later one first: measured `[22, 21, 23]` and `[3, 4, 1, 2]` in
    /// the GROUP tests under load. Every release goes to the inbox now.
    pub(crate) fn release_group(&self) {
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let live = self.readers.live();
        let readers: Vec<(EntityId, InstanceHandle)> = live
            .iter()
            .filter_map(|r| Some((r.entity_id()?, r.handle())))
            .collect();
        self.group_order.release(rt, &readers, |reader, item| {
            match live.iter().find(|r| r.handle() == *reader) {
                Some(r) => {
                    r.accept_group_released(item);
                    true
                }
                None => false,
            }
        });
    }
}

/// The Subscriber's group id is the participant's no longer once it is gone
/// (DDSI-RTPS 2.5 §8.2.4.4): the runtime may hand its key out again.
#[cfg(feature = "std")]
impl Drop for SubscriberInner {
    fn drop(&mut self) {
        if let (Some(rt), Some(id)) = (&self.runtime, self.entity_id) {
            rt.release_entity_key(id);
        }
    }
}

impl core::fmt::Debug for SubscriberInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let listener_present = self.listener.lock().map(|s| s.is_some()).unwrap_or(false);
        f.debug_struct("SubscriberInner")
            .field("entity_state", &self.entity_state)
            .field("listener_present", &listener_present)
            .finish_non_exhaustive()
    }
}

impl Subscriber {
    #[cfg(feature = "std")]
    pub(crate) fn new(qos: SubscriberQos, runtime: Option<Arc<DcpsRuntime>>) -> Self {
        let entity_id = runtime.as_ref().map(|rt| {
            rt.new_group_entity_id(zerodds_rtps::wire_types::EntityKind::UserReaderGroup)
        });
        let entity_state = crate::entity::EntityState::new();
        entity_state.set_autoenable_children(qos.entity_factory.autoenable_created_entities);
        let subscriber = Self {
            inner: Arc::new(SubscriberInner {
                qos: std::sync::Mutex::new(qos),
                entity_state,
                default_datareader_qos: std::sync::Mutex::new(DataReaderQos::default()),
                runtime,
                listener: std::sync::Mutex::new(None),
                participant: std::sync::Mutex::new(None),
                access_scope: crate::coherent_set::GroupAccessScope::new(),
                readers: crate::contained::ContainedList::new(),
                reader_entity_ids: std::sync::Mutex::new(alloc::vec::Vec::new()),
                entity_id,
                group_order: crate::presentation::GroupOrdering::new(),
                group_hook: Mutex::new(None),
            }),
        };
        // `StatusCondition::get_entity` (DDS 1.4 §2.2.2.1.9.3).
        let entity_weak = Arc::downgrade(&subscriber.inner);
        subscriber.inner.entity_state.set_entity_ref(move || {
            entity_weak
                .upgrade()
                .map(|inner| crate::entity::EntityRef::Subscriber(Subscriber { inner }))
        });
        // DATA_ON_READERS (DDS 1.4 §2.2.4.2.2) from what its readers have.
        // Weak: the state must not keep the subscriber alive.
        let weak = Arc::downgrade(&subscriber.inner);
        subscriber
            .inner
            .entity_state
            .set_status_probe(Arc::new(move || match weak.upgrade() {
                Some(inner) if inner.data_on_readers() => {
                    crate::psm_constants::status::DATA_ON_READERS
                }
                _ => 0,
            }));
        // HEARTBEATs and GAPs with group information, and the tick, let the
        // group order commit (DDSI-RTPS 2.5 §8.7.5).
        if let Some(rt) = subscriber.inner.runtime.as_ref() {
            let weak = Arc::downgrade(&subscriber.inner);
            let hook: crate::runtime::StatusHook = Arc::new(move || {
                if let Some(inner) = weak.upgrade() {
                    if inner.group_order_active() {
                        inner.release_group();
                    }
                }
            });
            rt.add_group_hook(&hook);
            if let Ok(mut slot) = subscriber.inner.group_hook.lock() {
                *slot = Some(hook);
            }
        }
        subscriber
    }

    /// DDS 1.4 §2.2.2.5.2, Subscriber operation `copy_from_topic_qos`: the
    /// reader half of `Publisher::copy_from_topic_qos`. The policies
    /// TopicQos and DataReaderQos both carry: DURABILITY, DEADLINE,
    /// LATENCY_BUDGET, LIVELINESS, RELIABILITY, DESTINATION_ORDER, HISTORY,
    /// RESOURCE_LIMITS and OWNERSHIP, and DATA_REPRESENTATION (the whole
    /// list, XTypes 1.3 §7.6.3.1.1). TOPIC_DATA is not copied, for the
    /// reason the writer half gives.
    ///
    /// # Errors
    /// None today; the `Result` is the spec's `ReturnCode_t`.
    pub fn copy_from_topic_qos(
        dr_qos: &mut DataReaderQos,
        topic_qos: &crate::qos::TopicQos,
    ) -> Result<()> {
        dr_qos.durability = topic_qos.durability;
        dr_qos.deadline = topic_qos.deadline;
        dr_qos.latency_budget = topic_qos.latency_budget;
        dr_qos.liveliness = topic_qos.liveliness;
        dr_qos.reliability = topic_qos.reliability;
        dr_qos.destination_order = topic_qos.destination_order;
        dr_qos.history = topic_qos.history;
        dr_qos.resource_limits = topic_qos.resource_limits;
        dr_qos.ownership = topic_qos.ownership;
        // §7.6.3.1.1: the whole list of the Topic applies to readers.
        dr_qos.data_representation =
            crate::qos::effective_data_representation(topic_qos.data_representation.as_deref());
        Ok(())
    }

    /// DDS 1.4 §2.2.2.5.2.15 `set_default_datareader_qos`, as
    /// `Publisher::set_default_datawriter_qos`.
    ///
    /// # Errors
    /// `AlreadyDeleted` after the subscriber's deletion;
    /// `InconsistentPolicy`.
    #[cfg(feature = "std")]
    pub fn set_default_datareader_qos(
        &self,
        qos: impl Into<crate::qos::QosArg<DataReaderQos>>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let qos = qos.into().resolve(DataReaderQos::default);
        qos.check_consistency()?;
        crate::participant::set_default(
            &self.inner.default_datareader_qos,
            crate::qos::QosArg::Value(qos),
        )
    }

    /// DDS 1.4 §2.2.2.5.2.16 `get_default_datareader_qos`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_default_datareader_qos(&self) -> DataReaderQos {
        crate::participant::get_default(&self.inner.default_datareader_qos)
    }

    /// DDS 1.4 §2.2.2.5.2.13 `get_participant`, as
    /// `Publisher::get_participant`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_participant(&self) -> Option<crate::participant::DomainParticipant> {
        let participant = self.inner.participant.lock().ok()?;
        participant
            .as_ref()?
            .upgrade()
            .map(crate::participant::DomainParticipant::from_inner)
    }

    /// The QoS `create_datareader` creates with on `topic`, the Topic of
    /// the TopicDescription (DDS 1.4 §2.2.2.5.2.5: "In the case of a
    /// ContentFilteredTopic, the Topic in question is the
    /// ContentFilteredTopic's 'related Topic.'").
    fn resolve_reader_qos<T: DdsType>(
        &self,
        topic: &Topic<T>,
        qos: EndpointQosArg<DataReaderQos>,
    ) -> Result<DataReaderQos> {
        Ok(match qos {
            EndpointQosArg::Value(qos) => qos,
            EndpointQosArg::Default => self.get_default_datareader_qos(),
            EndpointQosArg::UseTopicQos => {
                let mut qos = self.get_default_datareader_qos();
                Self::copy_from_topic_qos(&mut qos, &crate::entity::Entity::get_qos(topic))?;
                qos
            }
        })
    }

    /// Spec §2.2.2.2.1.10 — `true` if `handle` is a DataReader created
    /// via this Subscriber.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn contains_reader(&self, handle: crate::instance_handle::InstanceHandle) -> bool {
        self.inner.readers.contains(handle)
    }

    /// DDS 1.4 §2.2.2.5.2.6 `delete_datareader`: deletes a DataReader this
    /// subscriber created. Every handle of it returns
    /// [`DdsError::AlreadyDeleted`] from then on. A `DataReader` of this
    /// API lends no samples (its read and take return owned values), so no
    /// outstanding loan can stand in the way.
    ///
    /// # Errors
    /// `PreconditionNotMet`, with `reader` left as it was, if another
    /// subscriber created it or it still holds a ReadCondition or
    /// QueryCondition it created; `AlreadyDeleted` if `reader` or this
    /// subscriber is deleted.
    #[cfg(feature = "std")]
    pub fn delete_datareader<T: DdsType + Send + 'static>(
        &self,
        reader: &DataReader<T>,
    ) -> Result<()> {
        use crate::contained::ContainedReader;
        self.inner.entity_state.check_not_deleted()?;
        if !Arc::ptr_eq(&reader.inner.subscriber, &self.inner) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the DataReader belongs to another subscriber",
            });
        }
        reader.inner.entity_state.check_not_deleted()?;
        if reader.inner.status.is_builtin() {
            return Err(DdsError::PreconditionNotMet {
                reason: "a built-in DataReader is not deleted by the application",
            });
        }
        if reader.inner.has_conditions() {
            return Err(DdsError::PreconditionNotMet {
                reason: "the DataReader holds ReadConditions or QueryConditions",
            });
        }
        reader.inner.delete();
        Ok(())
    }

    /// DDS 1.4 §2.2.2.5.2.7 `lookup_datareader`: a handle to a DataReader
    /// of this subscriber attached to the topic named `topic_name`, or
    /// `None`. Of several, the oldest; the specification leaves it open.
    /// A reader of another type on a topic of that name is not a `T`
    /// reader and is not returned. The builtin readers are the builtin
    /// subscriber's (`DomainParticipant::get_builtin_subscriber`).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_datareader<T: DdsType + Send + 'static>(
        &self,
        topic_name: &str,
    ) -> Option<DataReader<T>> {
        self.inner
            .readers
            .find::<ReaderInner<T>>(topic_name)
            .map(DataReader::from_inner_arc)
    }

    /// DDS 1.4 §2.2.2.5.2.10 `get_datareaders`: the DataReaders of this
    /// subscriber that hold samples in the three state masks
    /// ([`crate::sample_info::sample_state_mask`] and its view and
    /// instance siblings).
    ///
    /// Under PRESENTATION `access_scope` GROUP the call belongs inside
    /// [`Self::begin_access`] / [`Self::end_access`]. With GROUP and
    /// `ordered_access` the result is a list with a reader once per
    /// sample, in the order the samples were written across the readers:
    /// by group sequence number where the samples come from one GROUP-scope
    /// publisher (DDSI-RTPS 2.5 §8.7.5), otherwise by source timestamp, each
    /// reader's samples in the order it presents them, a tie to the older
    /// reader. Otherwise it is a set, each reader
    /// once, in creation order.
    ///
    /// # Errors
    /// `PreconditionNotMet` under GROUP scope outside `begin_access`, or on
    /// a poisoned lock; `AlreadyDeleted` if this subscriber is deleted;
    /// `NotEnabled` if a reader is not enabled.
    #[cfg(feature = "std")]
    pub fn get_datareaders(
        &self,
        sample_states: u32,
        view_states: u32,
        instance_states: u32,
    ) -> Result<Vec<AnyDataReader>> {
        // §2.2.2.1.1.7: not an operation a disabled subscriber takes.
        self.inner.entity_state.check_enabled()?;
        let (group, ordered) = {
            let qos = self
                .inner
                .qos
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "subscriber qos poisoned",
                })?;
            let group = matches!(
                qos.presentation.access_scope,
                zerodds_qos::PresentationAccessScope::Group
            );
            (group, group && qos.presentation.ordered_access)
        };
        // §2.2.2.5.2.8, and not inside a data listener (§2.2.2.5.1).
        if group
            && !self.inner.access_scope.is_active()
            && !crate::listener_dispatch::in_data_listener()
        {
            return Err(DdsError::PreconditionNotMet {
                reason: "get_datareaders under GROUP access scope needs begin_access",
            });
        }
        let mut held = Vec::new();
        for reader in self.inner.readers.live() {
            let times = Arc::clone(&reader).matching_samples(
                sample_states,
                view_states,
                instance_states,
            )?;
            if !times.is_empty() {
                held.push((reader, times));
            }
        }
        if !ordered {
            return Ok(held
                .into_iter()
                .map(|(inner, _)| AnyDataReader { inner })
                .collect());
        }
        // A merge of the readers' sample sequences in write order: by group
        // sequence number within a group, else by source timestamp.
        let mut next = alloc::vec![0usize; held.len()];
        let total: usize = held.iter().map(|(_, times)| times.len()).sum();
        let mut out = Vec::with_capacity(total);
        while out.len() < total {
            let mut pick: Option<usize> = None;
            for (i, (_, times)) in held.iter().enumerate() {
                let Some(t) = times.get(next[i]) else {
                    continue;
                };
                if pick.is_none_or(|p| t.written_before(&held[p].1[next[p]])) {
                    pick = Some(i);
                }
            }
            let Some(i) = pick else {
                break;
            };
            next[i] += 1;
            out.push(AnyDataReader {
                inner: Arc::clone(&held[i].0),
            });
        }
        Ok(out)
    }

    /// DDS 1.4 §2.2.2.5.2.11 `notify_datareaders`: calls `on_data_available`
    /// on the DataReaderListener of every reader of this subscriber whose
    /// DATA_AVAILABLE changed and was handed to `on_data_on_readers`
    /// instead (§2.2.4.4). Meant for `on_data_on_readers` to delegate to
    /// the readers' listeners.
    ///
    /// # Errors
    /// `AlreadyDeleted` if this subscriber is deleted.
    #[cfg(feature = "std")]
    pub fn notify_datareaders(&self) -> Result<()> {
        // §2.2.2.1.1.7: not an operation a disabled subscriber takes.
        self.inner.entity_state.check_enabled()?;
        for reader in self.inner.readers.live() {
            reader.notify_data_available();
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.5.2.14 `delete_contained_entities`: deletes every
    /// DataReader this subscriber created, with the ReadConditions and
    /// QueryConditions each reader created.
    ///
    /// # Errors
    /// `AlreadyDeleted` if this subscriber is deleted;
    /// `PreconditionNotMet` while a group access is open (`begin_access`
    /// without its `end_access`) and on a poisoned lock.
    #[cfg(feature = "std")]
    pub fn delete_contained_entities(&self) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        // A reader is not deleted while the application is inside a group
        // access (§2.2.2.5.2.8): it is in a state where it cannot be.
        if self.inner.access_scope.is_active() {
            return Err(DdsError::PreconditionNotMet {
                reason: "a group access (begin_access) is open on the subscriber",
            });
        }
        for reader in self.inner.readers.live() {
            reader.delete_conditions()?;
            reader.delete()?;
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.5.2.12 `get_sample_lost_status`: the SAMPLE_LOST
    /// communication status of the Subscriber, the sum over the DataReaders
    /// it created: `total_count` is the samples they lost in all,
    /// `total_count_change` the loss since the last look at each reader's
    /// status or this one. Looking resets the change of every reader, and
    /// the SAMPLE_LOST flag of each, as `DataReader::get_sample_lost_status`
    /// does. A disabled Subscriber has no status to read (all zero,
    /// §2.2.2.1.1.7).
    ///
    /// # Errors
    /// `AlreadyDeleted` after delete; `PreconditionNotMet` while a reader
    /// cannot be looked at.
    #[cfg(feature = "std")]
    pub fn get_sample_lost_status(&self) -> Result<crate::status::SampleLostStatus> {
        self.inner.entity_state.check_not_deleted()?;
        let mut sum = crate::status::SampleLostStatus::default();
        if !self.inner.entity_state.is_enabled() {
            return Ok(sum);
        }
        for reader in self.inner.readers.live() {
            let one = reader.take_sample_lost_status()?;
            sum.total_count = sum.total_count.saturating_add(one.total_count);
            sum.total_count_change = sum
                .total_count_change
                .saturating_add(one.total_count_change);
        }
        Ok(sum)
    }

    /// `true` if this subscriber holds a DataReader that is not deleted;
    /// `DomainParticipant::delete_subscriber` refuses such a subscriber
    /// (DDS 1.4 §2.2.2.2.1.4).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn has_datareaders(&self) -> bool {
        !self.inner.readers.is_empty()
    }

    #[cfg(feature = "std")]
    /// Spec §2.2.3.20 + §2.2.2.1.1.7: `create_datareader` enables the new
    /// reader when this subscriber's
    /// `entity_factory.autoenable_created_entities` is TRUE. A disabled
    /// subscriber creates disabled readers regardless of the setting.
    fn autoenable_child(&self, child: &Arc<crate::entity::EntityState>) -> Result<()> {
        let autoenable = self
            .inner
            .qos
            .lock()
            .map(|q| q.entity_factory.autoenable_created_entities)
            .unwrap_or(true);
        if self.inner.entity_state.is_enabled() && autoenable {
            return child.enable_entity();
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    pub(crate) fn track_reader<T: DdsType + Send + 'static>(&self, reader: &DataReader<T>) {
        let weak: alloc::sync::Weak<dyn crate::contained::ContainedReader> =
            Arc::downgrade(&reader.inner) as _;
        self.inner
            .readers
            .add(reader.inner.entity_state.instance_handle(), weak);
    }
    #[cfg(not(feature = "std"))]
    pub(crate) fn new(qos: SubscriberQos) -> Self {
        Self {
            inner: Arc::new(SubscriberInner {
                qos,
                entity_state: crate::entity::EntityState::new(),
                access_scope: crate::coherent_set::GroupAccessScope::new(),
            }),
        }
    }

    /// Spec §2.2.2.5.2.8 `begin_access` — marks the start of a coherent
    /// read set. Nesting is allowed; each call increments an internal
    /// counter, each `end_access` decrements it.
    ///
    /// # Errors
    /// `NotEnabled` before the subscriber is enabled (DDS 1.4 §2.2.2.1.1.7:
    /// not among the operations a disabled entity takes); `AlreadyDeleted`
    /// after its deletion. It returned nothing.
    pub fn begin_access(&self) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        self.inner.access_scope.begin();
        Ok(())
    }

    /// Spec §2.2.2.5.2.9 `end_access` — counterpart to `begin_access`.
    ///
    /// # Errors
    /// `DdsError::PreconditionNotMet` if `end_access` is called without a
    /// preceding `begin_access`; `NotEnabled` as for `begin_access`.
    pub fn end_access(&self) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        self.inner.access_scope.end()
    }

    /// `true` if a group access is currently open.
    #[must_use]
    pub fn is_access_open(&self) -> bool {
        self.inner.access_scope.is_active()
    }

    /// Sets the `SubscriberListener` + StatusMask. `None` clears the
    /// slot. Spec §2.2.2.5.6.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcSubscriberListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0).then(|| {
                alloc::sync::Arc::new(crate::listener::NilListener) as ArcSubscriberListener
            })
        });
        if let Ok(mut slot) = self.inner.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(self.inner.runtime.as_ref());
    }

    /// Current listener clone.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcSubscriberListener> {
        self.inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
            .filter(|l| !l.is_nil())
    }

    /// Sets the weak back-pointer to the participant.
    #[cfg(feature = "std")]
    pub(crate) fn attach_participant(
        &self,
        participant: alloc::sync::Weak<crate::participant::ParticipantInner>,
    ) {
        if let Ok(mut slot) = self.inner.participant.lock() {
            *slot = Some(participant);
        }
    }

    /// Snapshot of the reader bubble-up chain: the given
    /// `reader_listener` tuple + subscriber stage + participant stage.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn snapshot_reader_chain(
        &self,
        reader_listener: Option<(ArcDataReaderListener, StatusMask)>,
    ) -> crate::listener_dispatch::ReaderListenerChain {
        let subscriber = self
            .inner
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        let participant = {
            let weak = self.inner.participant.lock().ok().and_then(|s| s.clone());
            weak.and_then(|w| w.upgrade()).and_then(|inner| {
                inner
                    .listener
                    .lock()
                    .ok()
                    .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)))
            })
        };
        crate::listener_dispatch::ReaderListenerChain {
            reader: reader_listener,
            subscriber,
            participant,
        }
    }

    /// DDS 1.4 §2.2.2.5.2.5 `create_datareader(a_topic, qos, a_listener,
    /// mask)`: the reader with its listener. `None` with a non-empty `mask` is
    /// the 'nil' listener (§2.2.2.1.1.3). What happened before the listener was
    /// set is heard at its first look.
    ///
    /// # Errors
    /// As [`Self::create_datareader`].
    #[cfg(feature = "std")]
    pub fn create_datareader_with_listener<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: impl Into<EndpointQosArg<DataReaderQos>>,
        listener: Option<ArcDataReaderListener>,
        mask: StatusMask,
    ) -> Result<DataReader<T>> {
        let reader = self.create_datareader(topic, qos)?;
        reader.set_listener(listener, mask);
        Ok(reader)
    }

    /// Creates a typed `DataReader<T>` (DDS 1.4 §2.2.2.5.2.5). With
    /// [`DATAREADER_QOS_DEFAULT`] the reader gets
    /// [`Self::get_default_datareader_qos`]; with
    /// [`DATAREADER_QOS_USE_TOPIC_QOS`], that QoS with the Topic QoS copied
    /// over it ([`Self::copy_from_topic_qos`]).
    ///
    /// [`DATAREADER_QOS_DEFAULT`]: crate::psm_constants::DATAREADER_QOS_DEFAULT
    /// [`DATAREADER_QOS_USE_TOPIC_QOS`]: crate::psm_constants::DATAREADER_QOS_USE_TOPIC_QOS
    ///
    /// # Errors
    /// `PreconditionNotMet` if the type metadata changed since topic creation.
    pub fn create_datareader<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: impl Into<EndpointQosArg<DataReaderQos>>,
    ) -> Result<DataReader<T>> {
        let qos = self.resolve_reader_qos(topic, qos.into())?;
        self.create_reader(topic, qos, None)
    }

    /// Creates a typed `DataReader<T>` on a [`ContentFilteredTopic`](crate::topic::ContentFilteredTopic) — the
    /// `create_datareader` of DDS 1.4 §2.2.2.5.2.5 with a TopicDescription
    /// that is a ContentFilteredTopic (§2.2.2.3.3).
    ///
    /// The reader matches the writers of the related topic, announces the
    /// filter in its SEDP subscription (`PID_CONTENT_FILTER_PROPERTY`,
    /// DDSI-RTPS 2.5 §9.6.3.4) and delivers only the samples the expression
    /// accepts with the parameters current at delivery. A sample the
    /// expression cannot be evaluated against — a field
    /// [`DdsType::field_value`] does not answer, a type mismatch — is not
    /// delivered. `set_filter_parameters` on the ContentFilteredTopic
    /// re-announces the subscription.
    ///
    /// # Errors
    /// As [`Self::create_datareader`]: `PreconditionNotMet`, the nil result of
    /// §2.2.2.5.2.5, if the related topic of `cft` belongs to another
    /// participant.
    pub fn create_datareader_with_cft<T: DdsType + Send + 'static>(
        &self,
        cft: &crate::topic::ContentFilteredTopic<T>,
        qos: impl Into<EndpointQosArg<DataReaderQos>>,
    ) -> Result<DataReader<T>> {
        // DDS 1.4 §2.2.1.1: a deleted ContentFilteredTopic has no readers.
        // §2.2.2.2.1.8: `delete_contentfilteredtopic` refuses while this
        // reader exists; a reader that is not created gives the hold back.
        let held = cft.attach_reader()?;
        let topic = cft.get_related_topic();
        let qos = self.resolve_reader_qos(topic, qos.into())?;
        let reader = self.create_reader(topic, qos, Some(cft.link()))?;
        if let Ok(mut slot) = reader.inner.description_use.lock() {
            *slot = Some(held);
        }
        #[cfg(feature = "std")]
        let reader = {
            let filter = cft.reader_filter();
            reader.with_filter(move |sample| filter(sample))
        };
        Ok(reader)
    }

    /// Creates a typed `DataReader<T>` on a [`MultiTopic`](crate::topic::MultiTopic) — the
    /// `create_datareader` of DDS 1.4 §2.2.2.5.2.5 with a TopicDescription
    /// that is a MultiTopic (§2.2.2.3.4).
    ///
    /// The reader reads the topics of the `FROM` clause through readers of
    /// its own on this subscriber (they take `qos`, end with the reader of
    /// the MultiTopic, and are not visible to the application: not in
    /// [`Self::get_datareaders`] or [`Self::lookup_datareader`], not among
    /// what `delete_contained_entities` deletes or `delete_subscriber`
    /// counts, and their statuses reach no listener above them),
    /// joins their instances, keeps the fields of the `SELECT` list, applies
    /// `WHERE`, and delivers samples of the result type; see
    /// `crate::multitopic` for the model. Data and instance state reach the
    /// reader's listener and conditions as for any reader.
    ///
    /// # Errors
    /// As [`Self::create_datareader`]; `PreconditionNotMet` if `multitopic`
    /// belongs to another participant; `AlreadyDeleted` if it is deleted;
    /// `BadParameter` for `TOPIC_QOS` (a MultiTopic has no Topic QoS).
    #[cfg(feature = "std")]
    pub fn create_datareader_with_multitopic<T: DdsType + Send + 'static>(
        &self,
        multitopic: &crate::topic::MultiTopic<T>,
        qos: impl Into<EndpointQosArg<DataReaderQos>>,
    ) -> Result<DataReader<T>> {
        self.inner.entity_state.check_not_deleted()?;
        let Some(participant) = self.inner.participant_inner() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "the participant of this subscriber is gone",
            });
        };
        if !crate::topic::TopicDescription::get_participant(multitopic).is(&participant) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the MultiTopic was created by another DomainParticipant",
            });
        }
        // §2.2.2.2.1.10: `delete_multitopic` refuses while this reader
        // exists; a reader that is not created gives the hold back.
        let held = multitopic.attach_reader()?;
        let qos = match qos.into() {
            EndpointQosArg::Value(qos) => qos,
            EndpointQosArg::Default => self.get_default_datareader_qos(),
            EndpointQosArg::UseTopicQos => {
                return Err(DdsError::BadParameter {
                    what: "a MultiTopic has no Topic QoS to take",
                });
            }
        };
        qos.check_consistency()?;
        let participant = crate::participant::DomainParticipant::from_inner(participant);
        let mut readers = Vec::new();
        for source in multitopic.plan().topics() {
            let (topic, _) = participant.multitopic_source(&source.name)?;
            readers.push(
                self.create_reader_as::<crate::serialized_type::SerializedSample>(
                    &topic,
                    qos.clone(),
                    None,
                    true,
                )?,
            );
        }
        let result = DataReader::<T>::new_offline(
            multitopic.result_topic().clone(),
            qos,
            self.inner.clone(),
        );
        result
            .inner
            .entity_state
            .set_factory(&self.inner.entity_state);
        let engine = Arc::new(crate::multitopic::MultiTopicEngine::new(
            Arc::clone(multitopic.plan()),
            readers,
            Arc::clone(&result.inner.inbox),
            result.inner.instances.clone(),
            multitopic.parameter_values(),
            self.inner.runtime.clone(),
        ));
        let weak = Arc::downgrade(&result.inner);
        engine.set_notify(Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                DataReader::from_inner_arc(inner).notify_data_arrived();
            }
        }));
        // The readers of the topics tell the reader of the result when
        // data arrives (§2.2.2.3.4).
        let listener: ArcDataReaderListener = Arc::new(MultiTopicSourceListener {
            engine: Arc::downgrade(&engine),
        });
        for reader in engine.source_readers() {
            reader.set_listener(
                Some(Arc::clone(&listener)),
                crate::psm_constants::status::DATA_AVAILABLE,
            );
        }
        // Enabling the reader of the result enables the readers of the
        // topics (a subscriber with autoenable off creates them disabled).
        let to_enable: Vec<Arc<crate::entity::EntityState>> = engine
            .source_readers()
            .iter()
            .map(|r| Arc::clone(&r.inner.entity_state))
            .collect();
        result.inner.entity_state.set_enable_work(move || {
            for state in &to_enable {
                state.enable_entity()?;
            }
            Ok(())
        });
        if let Ok(mut slot) = result.inner.description_use.lock() {
            *slot = Some(held);
        }
        let _ = result.inner.multitopic_engine.set(engine);
        self.autoenable_child(&result.inner.entity_state)?;
        self.track_reader(&result);
        Ok(result)
    }

    fn create_reader<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataReaderQos,
        content_filter: Option<crate::topic::CftLink>,
    ) -> Result<DataReader<T>> {
        self.create_reader_as(topic, qos, content_filter, false)
    }

    /// [`Self::create_reader`]; `internal` makes the reader one the
    /// application does not see: it is not in the subscriber's list of
    /// readers (`get_datareaders`, `lookup_datareader`,
    /// `delete_contained_entities`, `delete_subscriber`'s precondition) and
    /// its statuses end at its own listener. A reader of the topics of a
    /// MultiTopic (DDS 1.4 §2.2.2.3.4) is one; the reader of the MultiTopic
    /// owns it.
    fn create_reader_as<T: DdsType + Send + 'static>(
        &self,
        topic: &Topic<T>,
        qos: DataReaderQos,
        content_filter: Option<crate::topic::CftLink>,
        internal: bool,
    ) -> Result<DataReader<T>> {
        #[cfg(not(feature = "std"))]
        let _ = content_filter;
        #[cfg(feature = "std")]
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.5.2.5: "If the TopicDescription was created from a
        // different DomainParticipant, the operation will fail and return a
        // nil result."
        #[cfg(feature = "std")]
        if topic.created_by_another_than(&self.inner.participant) {
            return Err(DdsError::PreconditionNotMet {
                reason: "the TopicDescription was created by another DomainParticipant",
            });
        }
        qos.check_consistency()?;
        let metadata = topic.checked_type_metadata()?;
        #[cfg(not(feature = "std"))]
        let _ = metadata;
        #[cfg(feature = "std")]
        if let Some(rt) = self.inner.runtime.as_ref() {
            // Gap 2 (#24): auto-register the local TypeObject (if codegen or
            // a hand impl provides one) so the runtime's TypeLookup server
            // can answer a peer's getTypes and the local match sites can
            // resolve this reader's TYPE_IDENTIFIER structurally against the
            // registry. Default `type_object()` is `None` → no-op.
            //
            // Ahead of the endpoint for the reason given in
            // `Publisher::create_datawriter`: the SEDP subscription is built
            // and sent inside `register_user_reader_kind` and carries
            // PID_TYPE_INFORMATION derived from this registry entry (issue
            // #42, XTypes 1.3 §7.6.3.2).
            for obj in topic
                .codec()
                .dependent_type_objects()
                .iter()
                .chain(metadata.type_object.iter())
            {
                rt.register_type_object(obj.clone())
                    .map_err(|error| DdsError::WireError {
                        message: format!("type registration: {error:?}"),
                    })?;
            }
            // A type in a strongly connected component is registered as
            // that component (XTypes 1.3 §7.3.4.9): the hash of a member's
            // object is not the key it is looked up by.
            if let Some(component) = topic.codec().type_component() {
                rt.register_type_component(component)
                    .map_err(|error| DdsError::WireError {
                        message: format!("type registration: {error:?}"),
                    })?;
            }
            // Derive entityKind from the type's keyedness (spec §9.3.1.2:
            // 0x04=NoKey / 0x07=WithKey). A keyless type (`HAS_KEY=false`)
            // MUST produce a NoKey reader; otherwise cross-vendor writers
            // (CycloneDDS/ROS 2) reject the endpoint match due to an
            // entityKind mismatch (keyed vs no-key) — silently, with no log.
            // Mirror of `Publisher::create_datawriter`: DDS 1.4
            // §2.2.3.6/§2.2.3.13/§2.2.3.3 put PRESENTATION, PARTITION and
            // GROUP_DATA on the Subscriber, and the value the RxO check
            // compares travels in this reader's
            // `SubscriptionBuiltinTopicData`. The per-reader field stays an
            // override (see `qos::effective_presentation`).
            // DDS 1.4 §2.2.2.1.1.7, as in `Publisher::create_datawriter`:
            // the reader is registered with the runtime — announced and
            // matched — at its first `enable`, with the QoS it has then
            // ([`ReaderRegistration`]). Its sample channel exists from here.
            let eid = rt.reserve_user_reader_id(metadata.keyed);
            let (sample_tx, rx) = std::sync::mpsc::channel();
            let dr = DataReader::new_live(
                topic.clone(),
                qos,
                self.inner.clone(),
                Arc::clone(rt),
                eid,
                rx,
            );
            let registration = ReaderRegistration {
                runtime: Arc::clone(rt),
                eid,
                topic_name: topic.name().into(),
                type_name: metadata.type_name.to_string(),
                type_identifier: metadata.type_identifier.clone(),
                qos: Arc::clone(&dr.inner.qos),
                subscriber: self.inner.clone(),
                topic: topic.inner(),
                content_filter,
                sample_tx,
                cache: Arc::clone(&dr.inner.cache),
                pulled: Arc::clone(&dr.inner.pulled),
                status: Arc::downgrade(&dr.inner.status),
            };
            dr.inner
                .entity_state
                .set_enable_work(move || registration.register());
            dr.inner.entity_state.set_factory(&self.inner.entity_state);
            self.autoenable_child(&dr.inner.entity_state)?;
            if internal {
                dr.inner.status.set_internal();
            } else {
                self.track_reader(&dr);
            }
            topic.attach_user(crate::topic::TopicUserKind::DataReader);
            return Ok(dr);
        }
        let dr = DataReader::new_offline(topic.clone(), qos, self.inner.clone());
        dr.inner.entity_state.set_factory(&self.inner.entity_state);
        self.autoenable_child(&dr.inner.entity_state)?;
        #[cfg(feature = "std")]
        if internal {
            dr.inner.status.set_internal();
        } else {
            self.track_reader(&dr);
        }
        topic.attach_user(crate::topic::TopicUserKind::DataReader);
        Ok(dr)
    }
}

/// What the readers of a MultiTopic's topics tell the reader of the
/// MultiTopic: data arrived (DDS 1.4 §2.2.2.3.4).
#[cfg(feature = "std")]
struct MultiTopicSourceListener {
    engine: alloc::sync::Weak<crate::multitopic::MultiTopicEngine>,
}

#[cfg(feature = "std")]
impl crate::listener::DataReaderListener for MultiTopicSourceListener {
    fn on_data_available(&self, _reader: InstanceHandle) {
        if let Some(engine) = self.engine.upgrade() {
            engine.pump_and_notify();
        }
    }
}

#[cfg(feature = "std")]
impl SubscriberInner {
    /// The participant this subscriber came from, while it is still alive.
    pub(crate) fn participant_inner(&self) -> Option<Arc<crate::participant::ParticipantInner>> {
        self.participant
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().and_then(alloc::sync::Weak::upgrade))
    }
}

// ============================================================================
// Entity-Trait (DCPS §2.2.2.1) —
// ============================================================================

#[cfg(feature = "std")]
impl crate::entity::Entity for Subscriber {
    type Qos = SubscriberQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// The subscriber half of `Publisher::set_qos`: PRESENTATION is
    /// Changeable=NO (DDS 1.4 §2.2.3), a new PARTITION or GROUP_DATA
    /// reaches every DataReader that does not set its own.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos.into().try_resolve(|| {
            self.get_participant()
                .map(|p| p.get_default_subscriber_qos())
                .ok_or(DdsError::PreconditionNotMet {
                    reason: "the participant of this subscriber is gone",
                })
        })?;
        let enabled = self.inner.entity_state.is_enabled();
        let Ok(mut current) = self.inner.qos.lock() else {
            return Err(DdsError::PreconditionNotMet {
                reason: "subscriber qos poisoned",
            });
        };
        if enabled && current.presentation != qos.presentation {
            return Err(crate::entity::immutable_if_enabled("PRESENTATION"));
        }
        // DDS 1.4 §2.2.2.1.1.1: "In all other cases, none of the policies is
        // modified." ENTITY_FACTORY changed before the PARTITION check, and
        // stayed changed when that refused.
        if let Some(rt) = self.inner.runtime.as_ref() {
            if current.partition != qos.partition || current.group_data != qos.group_data {
                let readers = self
                    .inner
                    .reader_entity_ids
                    .lock()
                    .map(|ids| ids.clone())
                    .unwrap_or_default();
                for eid in &readers {
                    rt.check_user_group_partition(*eid, &qos.partition.names)?;
                }
                for eid in readers {
                    rt.update_user_group_qos(eid, &qos.partition.names, &qos.group_data.value)?;
                }
            }
        }
        self.inner
            .entity_state
            .set_autoenable_children(qos.entity_factory.autoenable_created_entities);
        *current = qos;
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7; with ENTITY_FACTORY.autoenable_created_entities
    /// it enables the readers this subscriber created.
    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable_entity()
    }

    fn entity_state(&self) -> alloc::sync::Arc<crate::entity::EntityState> {
        alloc::sync::Arc::clone(&self.inner.entity_state)
    }
}

/// What a read or take of DDS 1.4 §2.2.2.5.3 asks for beside the instance:
/// `max_samples` (`LENGTH_UNLIMITED`, -1, for all) and the three state
/// masks ([`crate::sample_info::sample_state_mask`], `view_state_mask`,
/// `instance_state_mask`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadSelection {
    /// The most samples to return; `-1` is `LENGTH_UNLIMITED`.
    pub max_samples: i32,
    /// SampleStateMask.
    pub sample_states: u32,
    /// ViewStateMask.
    pub view_states: u32,
    /// InstanceStateMask.
    pub instance_states: u32,
}

impl ReadSelection {
    /// Every sample in every state: `ANY_SAMPLE_STATE`, `ANY_VIEW_STATE`,
    /// `ANY_INSTANCE_STATE` and `LENGTH_UNLIMITED`.
    pub const ANY: Self = Self {
        max_samples: -1,
        sample_states: sample_state_mask::ANY,
        view_states: view_state_mask::ANY,
        instance_states: instance_state_mask::ANY,
    };

    /// [`Self::ANY`] limited to `max_samples`.
    #[must_use]
    pub const fn limited(max_samples: i32) -> Self {
        Self {
            max_samples,
            ..Self::ANY
        }
    }
}

/// Typed DataReader — removes samples that the RTPS reader has received
/// for the topic.
///
/// Live mode: `rx: Some` delivers samples from the runtime mpsc.
/// Offline mode: in-memory `inbox` for unit tests.
///
/// A `DataReader` is a handle: [`Clone`] gives another handle to the same
/// reader, as [`Subscriber::lookup_datareader`] does. The reader is deleted
/// by [`Subscriber::delete_datareader`], by the subscriber's or
/// participant's `delete_contained_entities`, or when its last handle is
/// dropped; an operation on a handle of a deleted reader returns
/// [`DdsError::AlreadyDeleted`].
pub struct DataReader<T: DdsType> {
    pub(crate) inner: Arc<ReaderInner<T>>,
}

impl<T: DdsType> Clone for DataReader<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// The reader the handles of a [`DataReader`] share.
pub(crate) struct ReaderInner<T: DdsType> {
    topic: Topic<T>,
    /// Shared with the reader's registration ([`ReaderRegistration`]).
    qos: Arc<Mutex<DataReaderQos>>,
    /// Entity lifecycle (DCPS §2.2.2.1).
    entity_state: Arc<crate::entity::EntityState>,
    /// Parent subscriber — for bubble-up to the subscriber and
    /// participant listeners.
    subscriber: Arc<SubscriberInner>,
    /// The listener and the last seen value of every status it is told
    /// about, shared with the runtime's listener dispatcher.
    #[cfg(feature = "std")]
    status: Arc<ReaderStatus>,
    /// Offline fallback inbox. Stores full [`UserSample`] values
    /// (including writer_guid + writer_strength for Alive) so that
    /// `take()`/`read()` can apply the exclusive-ownership filter in a
    /// spec-compliant way.
    inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    /// What RESOURCE_LIMITS refused under RELIABLE, and everything that
    /// arrived after it, in arrival order: the next ingest tries it first.
    /// DDS 1.4 §2.2.4.1 says of SAMPLE_REJECTED that "a rejected sample may
    /// be accepted in the future"; the RTPS layer has acknowledged it, so
    /// no writer sends it again, and dropping it here lost it — measured,
    /// a reader taking every 3 s with `max_samples` 100 000 under KEEP_ALL
    /// received 300 000 of 680 100 samples.
    #[cfg(feature = "std")]
    held_back: Arc<Mutex<Vec<StagedIngest>>>,
    /// The hold on the ContentFilteredTopic the reader was created on
    /// (DDS 1.4 §2.2.2.2.1.8: it cannot be deleted while a reader exists);
    /// given back when the reader is deleted.
    description_use: Mutex<Option<crate::topic::DescriptionReaderUse>>,
    /// The last sample a reader of a keyless topic received, with its
    /// representation and byte order. The one instance of such a topic has
    /// no key to rebuild a value from, so the invalid-data sample of a
    /// dispose or unregister carries this value (DDS 1.4 §2.2.2.5.1.13).
    keyless_last: Mutex<Option<(crate::sample_bytes::SampleBytes, u8, bool)>>,
    /// The join of a reader of a MultiTopic, which reads the topics of its
    /// `FROM` clause through readers of its own (DDS 1.4 §2.2.2.3.4).
    multitopic_engine: std::sync::OnceLock<Arc<crate::multitopic::MultiTopicEngine>>,
    /// The last received key holder and its hash; see
    /// [`crate::topic::KeyMaterialCache`].
    #[cfg(feature = "std")]
    last_key: crate::topic::KeyMaterialCache,
    /// The series every materialised sample updates, looked up at the
    /// first; see [`crate::metrics::ReaderMetrics`].
    #[cfg(feature = "metrics")]
    metrics: std::sync::OnceLock<crate::metrics::ReaderMetrics>,
    /// Members of coherent sets that are still open (DDS 1.4 §2.2.2.4.1.8-9,
    /// §2.2.3.6 PRESENTATION `coherent_access`). While the reader's
    /// Subscriber has `coherent_access` and an `access_scope` above INSTANCE,
    /// no sample of an open set is visible to the application; the members
    /// wait here, in write order, until the set's end-of-coherent-set marker
    /// (DDSI-RTPS 2.5 §8.7.6) arrives and releases them together. Empty
    /// whenever no set is open, and unused when `coherent_access` is off.
    #[cfg(feature = "std")]
    coherent_hold: Arc<crate::presentation::CoherentHold>,
    /// The last sample of each instance the TIME_BASED_FILTER held back
    /// under RELIABLE, delivered once it is due (DDS 1.4 §2.2.3.12).
    #[cfg(feature = "std")]
    tbf_hold:
        Mutex<crate::presentation::TbfHold<crate::instance_tracker::KeyHash, RawIngestSample>>,
    /// The tick hook that delivers it, made with the first held sample;
    /// the runtime holds it weakly.
    #[cfg(feature = "std")]
    tbf_hook: Mutex<Option<crate::runtime::StatusHook>>,
    /// NOT_ALIVE_NO_WRITERS markers `apply_unmatched_writers` put into the
    /// cache, counted: a condition or `get_datareaders` that made them
    /// leaves them as news (DDS 1.4 §2.2.4.2.2).
    #[cfg(feature = "std")]
    departure_markers: core::sync::atomic::AtomicU64,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    runtime: Option<Arc<DcpsRuntime>>,
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    entity_id: Option<EntityId>,
    /// Runtime channel for incoming samples (live mode).
    #[cfg(feature = "std")]
    rx: Option<Mutex<mpsc::Receiver<crate::runtime::UserSample>>>,
    /// How many items this reader took off `rx`; the runtime counts what
    /// it put on (`user_reader_channel_sent`).
    #[cfg(feature = "std")]
    pulled: Arc<std::sync::atomic::AtomicU64>,
    /// Optional content-filter closure. Applied to each sample in
    /// `take()` after decoding; returns `true` → sample is delivered,
    /// `false` → discarded.
    ///
    /// Spec reference: OMG DDS 1.4 §2.2.2.5.4 `ContentFilteredTopic`.
    /// This Rust closure variant is more idiomatic than the spec's SQL
    /// expression syntax and is sufficient for all in-process use cases.
    /// SQL parser + cross-vendor SEDP propagation follow later.
    #[allow(clippy::type_complexity)]
    filter: std::sync::RwLock<Option<Arc<dyn Fn(&T) -> bool + Send + Sync>>>,
    /// `true` once [`DataReader::with_filter`] set a filter: a reader
    /// without one takes no lock per sample.
    has_filter: core::sync::atomic::AtomicBool,
    /// Instance bookkeeping (spec §2.2.2.5.1).
    #[cfg(feature = "std")]
    instances: InstanceTracker,
    /// Sample cache with resolved [`SampleInfo`]. The cache is filled on
    /// arrival via `ingest_bytes`; `take`/`read`/`take_with_info`/
    /// `read_with_info` read from it.
    #[cfg(feature = "std")]
    cache: Arc<SampleCache>,
    /// Optionally configured Flatdata SlotBackend for the same-host
    /// zero-copy read path (`zerodds-flatdata-1.0` §4.1 + §9.1). Set via
    /// `set_flat_backend`; `read_flat()` falls back to classic `take()`
    /// when `None`.
    #[cfg(all(feature = "std", feature = "flatdata-integration"))]
    #[allow(clippy::type_complexity)]
    pub(crate) flat_backend: Mutex<
        Option<(
            Arc<dyn zerodds_flatdata::SlotBackend>,
            u8, // reader_index (0..31)
            std::sync::atomic::AtomicU32,
        )>,
    >,
    /// The ReadConditions and QueryConditions created on this reader
    /// (DDS 1.4 §2.2.2.5.3): what `delete_readcondition` may delete and
    /// `delete_contained_entities` deletes.
    #[cfg(feature = "std")]
    read_conditions: Mutex<Vec<ReaderCondition>>,
    _t: PhantomData<fn() -> T>,
}

/// A condition [`DataReader::create_readcondition`] or
/// [`DataReader::create_querycondition`] made, with the flag its trigger
/// reads: a deleted condition no longer triggers.
#[cfg(feature = "std")]
struct ReaderCondition {
    read: Arc<crate::condition::ReadCondition>,
    /// Held so the QueryCondition lives as long as the reader keeps it:
    /// its trigger reaches it weakly.
    _query: Option<Arc<crate::condition::QueryCondition>>,
    alive: Arc<core::sync::atomic::AtomicBool>,
}

/// A DataReader's sample cache. Releasing its lock reports the number of
/// samples to the reader's [`crate::runtime::ReaderRoom`], so the runtime's
/// release budget follows every admission, take and purge without each of
/// them having to say so.
#[cfg(feature = "std")]
pub(crate) struct SampleCache {
    /// Append-ordered, oldest first. A `VecDeque`: a full BEST_EFFORT
    /// KEEP_ALL cache gives up its oldest sample for every new one, and
    /// `Vec::remove(0)` moved all the others along each time — with
    /// 100 000 samples cached, a reader on the Linux test host spent 55 s in
    /// one `take` working through its backlog.
    samples: Mutex<alloc::collections::VecDeque<CachedSample>>,
    /// Attached when the reader is registered with the runtime and its
    /// QoS gives it a room.
    room: std::sync::OnceLock<Arc<crate::runtime::ReaderRoom>>,
}

/// The cache's lock was poisoned by a panic while it was held.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct CachePoisoned;

#[cfg(feature = "std")]
impl SampleCache {
    pub(crate) fn new() -> Self {
        Self {
            samples: Mutex::new(alloc::collections::VecDeque::new()),
            room: std::sync::OnceLock::new(),
        }
    }

    /// The reader's room, once it has one.
    fn room(&self) -> Option<&crate::runtime::ReaderRoom> {
        self.room.get().map(|room| &**room)
    }

    /// Locks the cache.
    ///
    /// # Errors
    /// [`CachePoisoned`] when a panic poisoned the lock.
    pub(crate) fn lock(&self) -> core::result::Result<CacheGuard<'_>, CachePoisoned> {
        let samples = self.samples.lock().map_err(|_| CachePoisoned)?;
        Ok(CacheGuard {
            samples,
            room: self.room(),
        })
    }
}

/// The locked cache; see [`SampleCache`].
#[cfg(feature = "std")]
pub(crate) struct CacheGuard<'a> {
    samples: std::sync::MutexGuard<'a, alloc::collections::VecDeque<CachedSample>>,
    room: Option<&'a crate::runtime::ReaderRoom>,
}

#[cfg(feature = "std")]
impl core::ops::Deref for CacheGuard<'_> {
    type Target = alloc::collections::VecDeque<CachedSample>;
    fn deref(&self) -> &alloc::collections::VecDeque<CachedSample> {
        &self.samples
    }
}

#[cfg(feature = "std")]
impl core::ops::DerefMut for CacheGuard<'_> {
    fn deref_mut(&mut self) -> &mut alloc::collections::VecDeque<CachedSample> {
        &mut self.samples
    }
}

#[cfg(feature = "std")]
impl Drop for CacheGuard<'_> {
    fn drop(&mut self) {
        if let Some(room) = self.room {
            room.set_cached(self.samples.len());
        }
    }
}

/// Internal: a decoded sample in the reader cache.
///
/// We carry the bytes (instead of `T`) so the reader cache is not bound
/// to `T: Clone` and so `T::decode` can happen lazily. Lifecycle markers
/// (dispose/unregister) have `bytes == None`.
#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct CachedSample {
    /// Zero-copy container: SampleBytes holds an Arc<[u8]> slice onto the
    /// RTPS wire datagram. None for lifecycle markers (spec §2.2.2.5.1.13).
    pub bytes: Option<crate::sample_bytes::SampleBytes>,
    pub info: SampleInfo,
    /// The received encapsulation of `bytes` (`0` = XCDR1 / `1` = XCDR2) — the
    /// cache stores the RAW body, so materialization MUST re-decode with the
    /// same representation via [`decode_for_encap`], not a fixed `T::decode`
    /// (which is XCDR2). Without this an XCDR1 body (no DHEADER) fails to
    /// materialize. Irrelevant for lifecycle markers (`bytes == None`).
    pub representation: u8,
    /// Big-endian flag of the received encapsulation (paired with
    /// [`Self::representation`]).
    pub big_endian: bool,
    /// DDS 1.4 §2.2.3.16 LIFESPAN expiration time of this sample: the
    /// writer's source timestamp plus the writer's announced LIFESPAN
    /// duration. `None` for an INFINITE lifespan (the default) and for
    /// lifecycle markers, which never expire. Once `get_current_time()`
    /// has passed this instant the sample is removed from the reader
    /// cache and is never delivered to the application.
    pub expires_at: Option<Time>,
    /// The `T` admission decoded `bytes` into — to evaluate the content
    /// filter and exclusive ownership and to key the sample — for the
    /// first `take` to hand out instead of decoding `bytes` a second time.
    /// A `read` decodes `bytes` and leaves it, as it leaves the sample.
    /// Measured on the Linux test host with a KEEP_ALL reader of 1 kB
    /// samples: decoding was 27 % of the taking thread, half of it the
    /// second decode. `None` for lifecycle markers and where the cache was
    /// filled otherwise; the `T` of the reader the cache belongs to.
    pub decoded: Option<Box<dyn core::any::Any + Send>>,
}

/// The `T` admission left in [`CachedSample::decoded`], taken out of it.
/// `None` when there is none, or when it is not a `T` — which only a cache
/// shared between readers of different types could hold, and then the
/// caller decodes the bytes as before.
#[cfg(feature = "std")]
fn take_decoded<T: 'static>(decoded: &mut Option<Box<dyn core::any::Any + Send>>) -> Option<T> {
    decoded.take()?.downcast::<T>().ok().map(|sample| *sample)
}

impl<T: DdsType> core::fmt::Debug for DataReader<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DataReader")
            .field("topic", &self.inner.topic.name())
            .field("type", &T::type_name())
            .field("qos", &self.inner.qos)
            .finish_non_exhaustive()
    }
}

/// One drained inbox entry awaiting decode in `ingest_into_cache`:
/// `(bytes, writer_guid, writer_strength, source_timestamp, big_endian,
/// representation, lifespan_nanos, reception_timestamp)`. `bytes` is the
/// refcounted zero-copy `SampleBytes` (Wave 2.1); `lifespan_nanos` is the
/// emitting writer's DDS 1.4 §2.2.3.16 LIFESPAN (`0` == INFINITE).
#[cfg(feature = "std")]
type RawIngestSample = (
    crate::sample_bytes::SampleBytes,
    [u8; 16],
    i32,
    Option<zerodds_rtps::header_extension::HeTimestamp>,
    bool,
    u8,
    u64,
    Option<Time>,
);

/// The reader QoS admission applies to a received sample
/// ([`DataReader::ingest_alive`]), read once for an ingest: read for every
/// sample, it was two locks of the QoS per sample.
#[cfg(feature = "std")]
#[derive(Clone, Copy)]
struct AdmissionQos {
    min_sep_nanos: u128,
    /// The clock `min_sep_nanos` is measured on.
    tbf_clock: crate::runtime::TimeBasedFilterClock,
    by_source_ts: bool,
    reliable: bool,
    exclusive_ownership: bool,
    history_kind: zerodds_qos::HistoryKind,
    history_depth: i32,
    max_samples: i32,
    max_samples_per_instance: i32,
    max_instances: i32,
}

/// One entry of `ingest_into_cache`'s ordered run: a data sample, or a
/// lifecycle marker (dispose / unregister) at its place in arrival order,
/// with the key holder the runtime decoded from its `serializedKey` and the
/// GUID of its writer (`[0; 16]` where none is known).
#[cfg(feature = "std")]
enum StagedIngest {
    Alive(RawIngestSample),
    /// Key hash, key holder, the instance state it brings, the writer, and
    /// the writer's source timestamp.
    Lifecycle(
        crate::instance_tracker::KeyHash,
        Option<Vec<u8>>,
        crate::sample_info::InstanceStateKind,
        [u8; 16],
        Option<zerodds_rtps::header_extension::HeTimestamp>,
    ),
}

#[cfg(feature = "std")]
impl StagedIngest {
    /// `None` for a coherent-set end marker: it only closes a set, which
    /// `coherent_gate` has already acted on.
    fn from_user_sample(item: crate::runtime::UserSample) -> Option<Self> {
        match item {
            crate::runtime::UserSample::Alive {
                payload,
                writer_guid,
                writer_strength,
                source_timestamp,
                big_endian,
                representation,
                lifespan_nanos,
                reception_timestamp,
                source_sequence_number,
                related_sample_identity,
                group,
                ..
            } => Some(Self::Alive((
                payload.with_wire(crate::sample_bytes::SampleWire {
                    writer_guid,
                    sequence_number: source_sequence_number,
                    related_sample_identity,
                    group: group.map(|g| g.info),
                }),
                writer_guid,
                writer_strength,
                source_timestamp,
                big_endian,
                representation,
                lifespan_nanos,
                reception_timestamp,
            ))),
            crate::runtime::UserSample::Lifecycle {
                key_hash,
                key_holder,
                kind,
                writer_guid,
                source_timestamp,
                ..
            } => Some(Self::Lifecycle(
                key_hash,
                key_holder,
                lifecycle_instance_state(kind),
                writer_guid,
                source_timestamp,
            )),
            crate::runtime::UserSample::CoherentSetEnd { .. } => None,
        }
    }
}

/// The instance state a lifecycle change kind puts an instance in.
#[cfg(feature = "std")]
fn lifecycle_instance_state(
    kind: zerodds_rtps::history_cache::ChangeKind,
) -> crate::sample_info::InstanceStateKind {
    match kind {
        zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
        | zerodds_rtps::history_cache::ChangeKind::NotAliveDisposedUnregistered => {
            crate::sample_info::InstanceStateKind::NotAliveDisposed
        }
        zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered => {
            crate::sample_info::InstanceStateKind::NotAliveNoWriters
        }
        _ => crate::sample_info::InstanceStateKind::Alive,
    }
}

/// DDS 1.4 §2.2.3.16 LIFESPAN: the expiration time of a sample is its source
/// timestamp plus the emitting writer's LIFESPAN duration. `lifespan_nanos ==
/// 0` encodes the INFINITE default and yields `None` — that sample never
/// expires.
#[cfg(feature = "std")]
fn lifespan_expiry(source_timestamp: Time, lifespan_nanos: u64) -> Option<Time> {
    if lifespan_nanos == 0 {
        return None;
    }
    let sec = i32::try_from(lifespan_nanos / 1_000_000_000).unwrap_or(i32::MAX);
    let nanosec = (lifespan_nanos % 1_000_000_000) as u32;
    Some(source_timestamp.add_duration(crate::time::Duration::new(sec, nanosec)))
}

/// DDS 1.4 §2.2.2.5.5 `SampleInfo.publication_handle` — identifies the
/// `DataWriter` that produced the sample, for the reader's own
/// `get_matched_publication_data` (spec §2.2.2.5.3.30) and echo-suppression
/// use cases. Derived from the writer's GUID via
/// [`InstanceHandle::from_guid`] — the exact derivation
/// `DcpsRuntime::user_reader_matched_publication_handles` already uses for
/// `get_matched_publications()`'s remote-writer handles and for a
/// same-participant intra-runtime writer's handle, so the two APIs agree on
/// one writer's identity.
///
/// `[0; 16]` is the sentinel a caller uses when no real writer GUID is
/// available (the offline `__push_raw` test hook, the builtin-topic inject
/// path) — those map to `HANDLE_NIL`, not a spurious hash of sixteen zero
/// bytes, matching `sample_info_lifecycle.rs`'s documented offline contract.
#[cfg(feature = "std")]
fn writer_publication_handle(writer_guid: [u8; 16]) -> InstanceHandle {
    if writer_guid == [0u8; 16] {
        return HANDLE_NIL;
    }
    InstanceHandle::from_guid(zerodds_rtps::wire_types::Guid::from_bytes(writer_guid))
}

/// DDS 1.4 §2.2.3.16: `true` if the sample's expiration time (its source
/// timestamp plus the emitting writer's LIFESPAN) has already passed. Used
/// only by the `not(feature = "std")` fallback bodies of `take()`/`read()` —
/// the `std` build routes every sample through `ingest_into_cache`, which
/// applies the identical expiry via [`purge_expired_by_lifespan`] against the
/// real cache instead. Costs one integer compare for the INFINITE default
/// (`0`).
///
/// A sample with no source timestamp at all (a caller that wrote without
/// one, or a builtin-topic inject) has nothing to measure against and is
/// therefore never expired here.
///
/// `now` is the caller's reception clock ([`DataReader::wall_now`]) — C2(6)'s
/// pluggable [`crate::runtime::RuntimeConfig::time_source`], not a direct
/// [`get_current_time`] call, so a configured/test clock governs this check
/// too.
#[cfg(not(feature = "std"))]
fn live_sample_expired(
    src_ts: Option<zerodds_rtps::header_extension::HeTimestamp>,
    lifespan_nanos: u64,
    now: Time,
) -> bool {
    if lifespan_nanos == 0 {
        return false;
    }
    let Some(source) = src_ts.map(crate::time::he_timestamp_to_time) else {
        return false;
    };
    matches!(lifespan_expiry(source, lifespan_nanos), Some(e) if e <= now)
}

/// DDS 1.4 §2.2.3.16: "Once the sample expires, the data will be removed from
/// the DataReader caches". Drops every cached sample whose expiration time has
/// passed, so neither `read` nor `take` can ever hand an expired sample to the
/// application.
///
/// `instances` gets the matching `drain_samples` for every purged sample of a
/// real (non-`HANDLE_NIL`) instance — without it `InstanceState::
/// samples_in_cache` drifts upward forever past a LIFESPAN purge, since the
/// only other decrement site is a `take()`. A drifted count would make the
/// HISTORY/RESOURCE_LIMITS admission below (`ingest_into_cache`) reject or
/// evict against a number `cache` itself no longer has.
#[cfg(feature = "std")]
fn purge_expired_by_lifespan(
    now: Time,
    cache: &mut alloc::collections::VecDeque<CachedSample>,
    instances: &InstanceTracker,
) {
    cache.retain(|s| {
        let expired = matches!(s.expires_at, Some(e) if e <= now);
        if expired && !s.info.instance_handle.is_nil() {
            instances.drain_samples(s.info.instance_handle, 1);
        }
        !expired
    });
}

/// RAII teardown (Spec §2.2.2.5.1.2 — deleting a `DataReader`). Dropping the
/// user's handle deregisters the reader from the runtime: it removes the slot,
/// rebuilds the intra-runtime route, and sends an SEDP dispose so remote peers
/// drop the matched reader at once. Offline readers (no runtime) are no-ops.
/// The part of a `DataReader` its status listeners need: the listener,
/// the parents of the bubble-up chain and the last seen value of every
/// communication status (DDS 1.4 §2.2.4.1). Shared with the runtime's
/// listener dispatcher, which runs [`ReaderStatus::drive`] when a status
/// moves, so a listener hears of a change without the application
/// polling.
/// Where a DATA_AVAILABLE or a DATA_ON_READERS flag was last reset (DDS
/// 1.4 §2.2.4.2.2): what had arrived when a listener was called for it, and
/// whether samples came into the cache outside a read or take since.
///
/// A read or take resets the flags by itself: it takes in everything on the
/// channel and in the inbox, so whatever is there after it is new
/// ([`Self::after_read`]). A listener call takes nothing in; the marks
/// remember what it covered ([`Self::cover`]).
#[cfg(feature = "std")]
#[derive(Default)]
pub(crate) struct NewsMarks {
    /// The channel's `sent` count a listener call covered.
    sent: std::sync::atomic::AtomicU64,
    /// The inbox length a listener call covered.
    inbox: std::sync::atomic::AtomicUsize,
    /// The runtime's departure count a listener call covered
    /// ([`DcpsRuntime::reader_departures`]).
    departures: std::sync::atomic::AtomicU64,
    /// Samples came into the cache outside a read or take — through
    /// `get_datareaders` — since the last reset.
    fresh: std::sync::atomic::AtomicBool,
    /// Samples the Subscriber's group order held for the reader when a
    /// listener call covered them (they were off the channel already). Each
    /// one that the release then puts into the inbox is covered too
    /// ([`Self::released`]).
    held: std::sync::atomic::AtomicUsize,
}

#[cfg(feature = "std")]
impl NewsMarks {
    /// A sample the group order held and covered now reaches the inbox: the
    /// inbox mark moves with it, so the release is no news (DDS 1.4
    /// §2.2.4.2.2).
    fn released(&self) {
        use std::sync::atomic::Ordering;
        let covered = self
            .held
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok();
        if covered {
            self.inbox.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn cover_held(&self, held: usize) {
        self.held.store(held, std::sync::atomic::Ordering::Release);
    }

    fn after_read(&self) {
        use std::sync::atomic::Ordering;
        self.held.store(0, Ordering::Release);
        self.sent.store(0, Ordering::Release);
        self.inbox.store(0, Ordering::Release);
        self.fresh.store(false, Ordering::Release);
    }

    fn cover(&self, (sent, inbox, departures): (u64, usize, u64)) {
        use std::sync::atomic::Ordering;
        self.sent.fetch_max(sent, Ordering::AcqRel);
        self.inbox.store(inbox, Ordering::Release);
        self.departures.fetch_max(departures, Ordering::AcqRel);
        self.fresh.store(false, Ordering::Release);
    }
}

/// How a read or take presents its samples (DDS 1.4 §2.2.2.5.3.8).
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    /// The samples of an instance are consecutive (rules 1, 2, 4).
    Grouped,
    /// In the order they were written across instances (rule 3, TOPIC
    /// with `ordered_access`).
    InWriteOrder,
    /// At most one sample (rule 5, GROUP with `ordered_access`).
    OnePerRead,
}

/// A stable sort of `items` by instance handle, a no-op while they are of
/// one instance (a keyless topic's are all `HANDLE_NIL`).
#[cfg(feature = "std")]
fn group_by_instance<X>(items: &mut [X], handle_of: impl Fn(&X) -> InstanceHandle) {
    let Some(first) = items.first().map(&handle_of) else {
        return;
    };
    if items.iter().all(|x| handle_of(x) == first) {
        return;
    }
    items.sort_by_key(|x| handle_of(x));
}

/// What a DataReader has that the application has not been told of (DDS
/// 1.4 §2.2.4.2.2): the DATA_AVAILABLE flag and the reader's part of its
/// Subscriber's DATA_ON_READERS flag. "The DATA_AVAILABLE StatusChangedFlag
/// becomes FALSE when either the corresponding listener operation
/// (on_data_available) is called or the read or take operation (or their
/// variants) is called on the associated DataReader"; DATA_ON_READERS
/// becomes FALSE when `on_data_on_readers` is called, `on_data_available`
/// on any DataReader of the Subscriber, or a read or take on any of them.
///
/// The flags were the state of the reader: up while a sample was on the
/// channel, in the inbox, or NOT_READ in the cache. A `read_instance` of one
/// instance left them up for the unread samples of another, and a listener
/// call left them up until the application read.
#[cfg(feature = "std")]
pub(crate) struct ReaderNews {
    runtime: Option<Arc<DcpsRuntime>>,
    entity_id: Option<EntityId>,
    pulled: Arc<std::sync::atomic::AtomicU64>,
    /// The runtime's count of what it put on the reader's channel, from
    /// the reader's registration on
    /// ([`DcpsRuntime::user_reader_sent_counter`]).
    sent: std::sync::OnceLock<Arc<std::sync::atomic::AtomicU64>>,
    inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
    instances: InstanceTracker,
    /// The reader's DATA_AVAILABLE.
    available: NewsMarks,
    /// The reader's part of its Subscriber's DATA_ON_READERS.
    on_readers: NewsMarks,
}

#[cfg(feature = "std")]
impl ReaderNews {
    fn new(
        runtime: Option<Arc<DcpsRuntime>>,
        entity_id: Option<EntityId>,
        pulled: &Arc<std::sync::atomic::AtomicU64>,
        inbox: &Arc<Mutex<Vec<crate::runtime::UserSample>>>,
        instances: &InstanceTracker,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            entity_id,
            pulled: Arc::clone(pulled),
            sent: std::sync::OnceLock::new(),
            inbox: Arc::clone(inbox),
            instances: instances.shared(),
            available: NewsMarks::default(),
            on_readers: NewsMarks::default(),
        })
    }

    /// What the runtime put on the channel; 0 offline or before the
    /// reader's registration.
    fn sent(&self) -> u64 {
        use std::sync::atomic::Ordering;
        if let Some(counter) = self.sent.get() {
            return counter.load(Ordering::Acquire);
        }
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return 0;
        };
        let Some(counter) = rt.user_reader_sent_counter(eid) else {
            return 0;
        };
        let sent = counter.load(Ordering::Acquire);
        let _ = self.sent.set(counter);
        sent
    }

    /// What has arrived so far: the channel's `sent` count, the inbox
    /// length, the departures noted.
    fn now(&self) -> (u64, usize, u64) {
        let departures = match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => rt.reader_departures(eid).map_or(0, |(noted, _)| noted),
            _ => 0,
        };
        let inbox = self.inbox.lock().map(|i| i.len()).unwrap_or(0);
        (self.sent(), inbox, departures)
    }

    /// Whether something arrived past `marks`: a sample on the channel past
    /// what was taken off and what a listener call covered, one in the inbox
    /// past what a listener call covered, one `get_datareaders` took into
    /// the cache, or a writer that left and leaves an instance without a
    /// writer (§2.2.2.5.1.3).
    fn has_news(&self, marks: &NewsMarks) -> bool {
        use std::sync::atomic::Ordering;
        if marks.fresh.load(Ordering::Acquire) {
            return true;
        }
        let taken = self
            .pulled
            .load(Ordering::Acquire)
            .max(marks.sent.load(Ordering::Acquire));
        if self.sent() > taken {
            return true;
        }
        if self
            .inbox
            .lock()
            .map(|i| i.len() > marks.inbox.load(Ordering::Acquire))
            .unwrap_or(false)
        {
            return true;
        }
        match (&self.runtime, self.entity_id) {
            (Some(rt), Some(eid)) => rt.reader_departures(eid).is_some_and(|(noted, gone)| {
                noted > marks.departures.load(Ordering::Acquire)
                    && self
                        .instances
                        .would_leave_writerless(&gone, crate::sample_info::view_state_mask::ANY)
            }),
            _ => false,
        }
    }

    /// `get_datareaders` took samples into the cache: they are news until
    /// a read, a take or a listener call.
    fn note_fresh(&self) {
        use std::sync::atomic::Ordering;
        self.available.fresh.store(true, Ordering::Release);
        self.on_readers.fresh.store(true, Ordering::Release);
    }

    /// `wait_for_data` moved `moved` samples, the channel's sample after
    /// `pulled_before`, to the inbox: a listener call that had covered it
    /// on the channel covers them in the inbox too.
    fn note_parked(&self, pulled_before: u64, moved: usize) {
        use std::sync::atomic::Ordering;
        for marks in [&self.available, &self.on_readers] {
            if pulled_before < marks.sent.load(Ordering::Acquire) {
                marks.inbox.fetch_add(moved, Ordering::AcqRel);
            }
        }
    }
}

#[cfg(feature = "std")]
pub(crate) struct ReaderStatus {
    runtime: Option<Arc<DcpsRuntime>>,
    entity_id: Option<EntityId>,
    entity_state: Arc<crate::entity::EntityState>,
    subscriber: Arc<SubscriberInner>,
    /// Optional `DataReaderListener` + StatusMask.
    listener: Mutex<Option<(ArcDataReaderListener, StatusMask)>>,
    /// The runtime's liveliness change counter when the listener last
    /// looked (`-1`: never).
    last_liveliness_changes: std::sync::atomic::AtomicI64,
    /// The reader's instances: the runtime names a missed deadline's
    /// instance by key hash, this turns it into the handle.
    instances: InstanceTracker,
    /// The runtime's count of samples handed to this reader when a
    /// DATA_AVAILABLE or DATA_ON_READERS listener was last told of them.
    last_delivered: std::sync::atomic::AtomicU64,
    /// That count itself, shared with the runtime from the reader's
    /// registration on ([`DcpsRuntime::user_reader_delivered_counter`]).
    delivered: std::sync::OnceLock<Arc<std::sync::atomic::AtomicU64>>,
    /// The runtime's count of writers that left this reader when a data
    /// listener was last told of the NOT_ALIVE_NO_WRITERS markers they
    /// leave ([`DcpsRuntime::reader_departures`]).
    departures_told: std::sync::atomic::AtomicU64,
    /// A thread is in [`Self::dispatch_data`] for this reader.
    data_dispatching: std::sync::atomic::AtomicBool,
    /// Samples arrived since the thread in [`Self::dispatch_data`] last
    /// looked: it looks again before it leaves.
    data_again: std::sync::atomic::AtomicBool,
    /// [`crate::listener_dispatcher::listener_generation`] plus one when
    /// `wants_data` was last worked out (`0`: never).
    wants_data_generation: std::sync::atomic::AtomicU64,
    /// Whether a data listener was in the chain then.
    wants_data: std::sync::atomic::AtomicBool,
    /// Samples were handed over while no data listener was in the chain;
    /// the dispatcher's pass tells a listener set since.
    data_unhanded: std::sync::atomic::AtomicBool,
    /// The delivered count at the dispatcher's previous pass.
    data_seen_by_pass: std::sync::atomic::AtomicU64,
    /// DATA_AVAILABLE changed and `on_data_on_readers` was called for it
    /// instead of `on_data_available` (DDS 1.4 §2.2.4.4); a read or take,
    /// or the call `notify_datareaders` makes, ends it (§2.2.4.2.2).
    data_available_pending: std::sync::atomic::AtomicBool,
    /// The DATA_AVAILABLE and DATA_ON_READERS flags (§2.2.4.2.2).
    news: Arc<ReaderNews>,
    /// The reader belongs to a MultiTopic reader, not to the application:
    /// its statuses end at its own listener and do not reach the subscriber
    /// or the participant, and its reads do not reset the subscriber's
    /// DATA_ON_READERS.
    internal: std::sync::atomic::AtomicBool,
    /// A built-in DataReader (DDS 1.4 §2.2.5): it belongs to the built-in
    /// Subscriber and is not deleted by the application.
    builtin: std::sync::atomic::AtomicBool,
}

#[cfg(feature = "std")]
impl ReaderStatus {
    fn new(
        runtime: Option<Arc<DcpsRuntime>>,
        entity_id: Option<EntityId>,
        entity_state: &Arc<crate::entity::EntityState>,
        subscriber: &Arc<SubscriberInner>,
        instances: &InstanceTracker,
        news: &Arc<ReaderNews>,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            entity_id,
            entity_state: Arc::clone(entity_state),
            subscriber: Arc::clone(subscriber),
            listener: Mutex::new(None),
            last_liveliness_changes: std::sync::atomic::AtomicI64::new(-1),
            instances: instances.clone(),
            last_delivered: std::sync::atomic::AtomicU64::new(0),
            delivered: std::sync::OnceLock::new(),
            departures_told: std::sync::atomic::AtomicU64::new(0),
            data_dispatching: std::sync::atomic::AtomicBool::new(false),
            data_again: std::sync::atomic::AtomicBool::new(false),
            wants_data_generation: std::sync::atomic::AtomicU64::new(0),
            wants_data: std::sync::atomic::AtomicBool::new(false),
            data_unhanded: std::sync::atomic::AtomicBool::new(false),
            data_seen_by_pass: std::sync::atomic::AtomicU64::new(0),
            data_available_pending: std::sync::atomic::AtomicBool::new(false),
            news: Arc::clone(news),
            internal: std::sync::atomic::AtomicBool::new(false),
            builtin: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Marks the reader as a MultiTopic reader's own.
    fn set_internal(&self) {
        self.internal
            .store(true, std::sync::atomic::Ordering::Release);
    }

    fn is_internal(&self) -> bool {
        self.internal.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Marks the reader as one of the built-in Subscriber.
    fn set_builtin(&self) {
        self.builtin
            .store(true, std::sync::atomic::Ordering::Release);
    }

    fn is_builtin(&self) -> bool {
        self.builtin.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Whether a listener of the chain takes DATA_AVAILABLE or
    /// DATA_ON_READERS (DDS 1.4 §2.2.4.4): the reader's, its subscriber's
    /// or its participant's.
    fn listens_for_data(chain: &crate::listener_dispatch::ReaderListenerChain) -> bool {
        use crate::psm_constants::status::{DATA_AVAILABLE, DATA_ON_READERS};
        let reader_bits = DATA_AVAILABLE;
        let upper_bits = DATA_AVAILABLE | DATA_ON_READERS;
        chain
            .reader
            .as_ref()
            .is_some_and(|(_, mask)| mask & reader_bits != 0)
            || chain
                .subscriber
                .as_ref()
                .is_some_and(|(_, mask)| mask & upper_bits != 0)
            || chain
                .participant
                .as_ref()
                .is_some_and(|(_, mask)| mask & upper_bits != 0)
    }

    /// Whether the receive path should wake the dispatcher for this
    /// reader's data; see [`Self::install_data_signal`]. Worked out from
    /// the chain once per listener change and not per run of samples: the
    /// chain is three locks, and the receive thread asks after every
    /// datagram.
    fn wants_data_signal(&self) -> bool {
        use std::sync::atomic::Ordering;
        let generation = crate::listener_dispatcher::listener_generation() + 1;
        if self.wants_data_generation.load(Ordering::Acquire) == generation {
            return self.wants_data.load(Ordering::Acquire);
        }
        let wants = Self::listens_for_data(&self.listener_chain());
        self.wants_data.store(wants, Ordering::Release);
        self.wants_data_generation
            .store(generation, Ordering::Release);
        wants
    }

    /// Has the receive path call this reader's data listeners after each
    /// run of samples it hands the reader, while a data listener is in the
    /// chain ([`Self::dispatch_data`]). Installed when the reader's slot is
    /// created, at its registration.
    ///
    /// The listeners run on the thread that handed the samples over, as
    /// Cyclone DDS and Fast DDS call them. They used to run on the listener
    /// dispatcher's thread, which the receive thread woke after every
    /// datagram — a `futex` call, 8 % of the receive thread of a 1 kB
    /// BEST_EFFORT stream on the Linux test host — or which spun for the
    /// next one instead, and then held a core for the whole stream (0.98 to
    /// 1.15 µs of CPU per sample).
    ///
    /// A live reader's `on_data_available` used to be called only from
    /// `DataReader::wait_for_data`, when the application itself waited: a
    /// reader with a listener and no wait heard nothing. Measured on the
    /// Linux test host, `zerodds_perf sub` taking in its listener read 957
    /// samples of a 13 million sample stream.
    fn install_data_signal(self: &Arc<Self>) -> Result<()> {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return Ok(());
        };
        if let Some(counter) = rt.user_reader_delivered_counter(eid) {
            let _ = self.delivered.set(counter);
        }
        let status = Arc::downgrade(self);
        let installed = rt.set_user_reader_data_signal(
            eid,
            Some(Arc::new(move || {
                let Some(status) = status.upgrade() else {
                    return;
                };
                if status.wants_data_signal() {
                    status.dispatch_data();
                } else {
                    status
                        .data_unhanded
                        .store(true, std::sync::atomic::Ordering::Release);
                }
            })),
        );
        if installed {
            Ok(())
        } else {
            Err(DdsError::PreconditionNotMet {
                reason: "datareader slot missing for its data signal",
            })
        }
    }

    /// [`Self::poll_data_available`] on one thread at a time: the
    /// receive threads, a writing thread delivering to a reader of its own
    /// participant, and the dispatcher's status pass may all ask at once,
    /// and a reader's data listeners are called in turn. A thread that
    /// finds another in the pass leaves the samples to it and returns
    /// without waiting; the one in the pass looks again before it leaves.
    #[cfg(feature = "std")]
    pub(crate) fn dispatch_data(&self) {
        use std::sync::atomic::Ordering;
        self.data_again.store(true, Ordering::Release);
        loop {
            if self
                .data_dispatching
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
            {
                let _scope = crate::listener_dispatch::DataListenerScope::enter();
                while self.data_again.swap(false, Ordering::AcqRel) {
                    self.poll_data_available();
                }
            }
            self.data_dispatching.store(false, Ordering::Release);
            // Samples announced between the last look and the release went
            // to a thread that found the pass taken.
            if !self.data_again.load(Ordering::Acquire) {
                return;
            }
        }
    }

    /// Calls `on_data_on_readers` and `on_data_available` when samples
    /// reached the reader since a data listener was last told (DDS 1.4
    /// §2.2.4.1, §2.2.4.2.6.1): the runtime counts what it hands the
    /// reader. Without a data listener in the chain nothing is consumed, so
    /// a listener set later hears of samples that came before it. Called
    /// through [`Self::dispatch_data`], one thread at a time.
    #[cfg(feature = "std")]
    pub(crate) fn poll_data_available(&self) {
        use std::sync::atomic::Ordering;
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        let delivered = self.delivered_count(rt, eid);
        // One thread at a time polls (`dispatch_data`); a read or take
        // moves `last_delivered` on too (`note_read_or_take`).
        let arrived = delivered > self.last_delivered.load(Ordering::Acquire);
        // A writer that left, or lost its liveliness, leaves the instances
        // only it wrote NOT_ALIVE_NO_WRITERS, and the next read or take
        // hands out their markers (DDS 1.4 §2.2.2.5.1.3): DATA_AVAILABLE
        // changed with them (§2.2.4.2.2).
        let departures = rt
            .reader_departures(eid)
            .filter(|(noted, _)| *noted > self.departures_told.load(Ordering::Acquire));
        if !arrived && departures.is_none() {
            return;
        }
        let chain = self.listener_chain();
        if !Self::listens_for_data(&chain) {
            return;
        }
        if let Some((noted, gone)) = departures {
            self.departures_told.store(noted, Ordering::Release);
            if !arrived
                && !self
                    .instances
                    .would_leave_writerless(&gone, crate::sample_info::view_state_mask::ANY)
            {
                return;
            }
        }
        // A read or take may have moved it on meanwhile.
        self.last_delivered.fetch_max(delivered, Ordering::AcqRel);
        let reader = self.entity_state.instance_handle();
        let subscriber = self.subscriber.entity_state.instance_handle();
        // DDS 1.4 §2.2.4.4: on_data_on_readers first; only if no listener
        // takes it, on_data_available. §2.2.4.2.2: each call resets the
        // flag it is for, and either resets the Subscriber's
        // DATA_ON_READERS.
        let covered = self.news.now();
        let held = self.held_now();
        if crate::listener_dispatch::dispatch_data_on_readers(&chain, subscriber) {
            self.data_available_pending.store(true, Ordering::Release);
        } else {
            crate::listener_dispatch::dispatch_data_available(&chain, reader);
            self.news.available.cover(covered);
            self.news.available.cover_held(held);
        }
        if !self.is_internal() {
            self.subscriber.cover_data_on_readers(None);
        }
    }

    /// How many samples the Subscriber's group order holds for this reader.
    fn held_now(&self) -> usize {
        self.subscriber
            .group_held_for(self.entity_state.instance_handle())
    }

    /// The runtime's count of samples handed to this reader.
    #[cfg(feature = "std")]
    fn delivered_count(&self, rt: &DcpsRuntime, eid: EntityId) -> u64 {
        match self.delivered.get() {
            Some(counter) => counter.load(std::sync::atomic::Ordering::Acquire),
            None => rt.user_reader_samples_delivered(eid),
        }
    }

    /// Whether the dispatcher's pass has data listeners to call. Samples
    /// are the delivering thread's: its data signal calls the listeners
    /// there ([`Self::install_data_signal`]). The pass takes what that
    /// leaves: samples handed over while no data listener was in the
    /// chain, for one set since; samples still untold at two passes in a
    /// row, which no signal announced; and writers that left, which no
    /// delivery announces. Before, every pass called the listeners for
    /// whatever had arrived, and one that came first ran them on the
    /// dispatcher's thread.
    #[cfg(feature = "std")]
    fn data_left_to_the_pass(&self) -> bool {
        use std::sync::atomic::Ordering;
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return false;
        };
        let delivered = self.delivered_count(rt, eid);
        let previous = self.data_seen_by_pass.swap(delivered, Ordering::AcqRel);
        if delivered > self.last_delivered.load(Ordering::Acquire) && previous == delivered {
            return true;
        }
        if self.data_unhanded.load(Ordering::Acquire) && self.wants_data_signal() {
            self.data_unhanded.store(false, Ordering::Release);
            return true;
        }
        rt.reader_departures(eid)
            .is_some_and(|(noted, _)| noted > self.departures_told.load(Ordering::Acquire))
    }

    /// DDS 1.4 §2.2.2.5.2.11 `notify_datareaders`, for this reader: calls
    /// `on_data_available` on the DataReaderListener if DATA_AVAILABLE
    /// changed and `on_data_on_readers` was called for it. Without such a
    /// listener the change stays for a later call.
    pub(crate) fn notify_data_available(&self) {
        use std::sync::atomic::Ordering;
        let chain = self.listener_chain();
        let listens = chain
            .reader
            .as_ref()
            .is_some_and(|(_, mask)| mask & crate::psm_constants::status::DATA_AVAILABLE != 0);
        if listens && self.data_available_pending.swap(false, Ordering::AcqRel) {
            let covered = self.news.now();
            let held = self.held_now();
            if crate::listener_dispatch::dispatch_reader_data_available(
                &chain,
                self.entity_state.instance_handle(),
            ) {
                self.news.available.cover(covered);
                self.news.available.cover_held(held);
                if !self.is_internal() {
                    self.subscriber.cover_data_on_readers(None);
                }
            }
        }
    }

    /// A read or take ends a pending DATA_AVAILABLE change, resets the
    /// reader's DATA_AVAILABLE and its Subscriber's DATA_ON_READERS
    /// (§2.2.4.2.2).
    fn note_read_or_take(&self) {
        // §2.2.4.2.2: the read or take lowers DATA_AVAILABLE for what was
        // delivered until now; the dispatcher calls on_data_available for
        // nothing of it. A listener set after a take was called for the
        // samples the take had handed out.
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            let delivered = match self.delivered.get() {
                Some(counter) => counter.load(std::sync::atomic::Ordering::Acquire),
                None => rt.user_reader_samples_delivered(eid),
            };
            self.last_delivered
                .fetch_max(delivered, std::sync::atomic::Ordering::AcqRel);
        }
        self.data_available_pending
            .store(false, std::sync::atomic::Ordering::Release);
        self.news.available.after_read();
        self.news.on_readers.after_read();
        if !self.is_internal() {
            self.subscriber
                .cover_data_on_readers(Some(self.entity_state.instance_handle()));
        }
    }

    /// Snapshot of the bubble-up chain (Reader → Subscriber → Participant)
    /// for hot-path listener dispatch.
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::ReaderListenerChain {
        let reader = self
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, m)| (Arc::clone(l), *m)));
        // A MultiTopic reader's own reader answers to nobody above it.
        if self.is_internal() {
            return crate::listener_dispatch::ReaderListenerChain {
                reader,
                subscriber: None,
                participant: None,
            };
        }
        let sub_handle = Subscriber {
            inner: Arc::clone(&self.subscriber),
        };
        sub_handle.snapshot_reader_chain(reader)
    }

    /// Calls `on_subscription_matched` while SUBSCRIPTION_MATCHED changed
    /// since it was last read or handed to a listener (DDS 1.4 §2.2.4.1),
    /// with the status as the runtime keeps it: `total_count` cumulative,
    /// the last writer that joined or left. A listener that took the status
    /// acknowledges its changes; a listener set after the change hears it
    /// at its first look.
    #[cfg(feature = "std")]
    pub(crate) fn poll_subscription_matched(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_reader_subscription_matched_status(eid),
            &self.entity_state,
            crate::psm_constants::status::SUBSCRIPTION_MATCHED,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::SUBSCRIPTION_MATCHED)
            },
            |status| {
                crate::listener_dispatch::dispatch_subscription_matched(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_reader_subscription_matched_status(eid, status, reads)
            },
        );
    }

    /// `on_requested_deadline_missed` while the status is unread; see
    /// [`Self::poll_subscription_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_deadline_missed(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || {
                rt.observe_user_reader_requested_deadline_missed_status(eid)
                    .map(|(unread, status, reads)| {
                        (unread, self.with_missed_instance(status), reads)
                    })
            },
            &self.entity_state,
            crate::psm_constants::status::REQUESTED_DEADLINE_MISSED,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::REQUESTED_DEADLINE_MISSED)
            },
            |status| {
                crate::listener_dispatch::dispatch_requested_deadline_missed(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_reader_requested_deadline_missed_status(eid, status, reads)
            },
        );
    }

    /// `status` with the handle of the instance whose deadline was missed
    /// last; the runtime knows it by key hash only.
    #[cfg(feature = "std")]
    pub(crate) fn with_missed_instance(
        &self,
        mut status: crate::status::RequestedDeadlineMissedStatus,
    ) -> crate::status::RequestedDeadlineMissedStatus {
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            if let Some(handle) = rt
                .user_reader_missed_deadline_instance(eid)
                .and_then(|key| self.instances.lookup(&key))
            {
                status.last_instance_handle = handle;
            }
        }
        status
    }

    /// Calls `on_liveliness_changed` while the reader's liveliness changed
    /// since the status was last read or handed to a listener (DDS 1.4
    /// §2.2.4.1). The runtime's change counter decides, not the counts: one
    /// writer lost and another gained leaves them where they were.
    ///
    /// The status's change fields are reset by what a listener was handed,
    /// and only when a listener took it; with none attached they stay for
    /// `get_liveliness_changed_status`, or for a listener set later.
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_changed(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        let changes = i64::try_from(rt.user_reader_liveliness_changes(eid)).unwrap_or(i64::MAX);
        let prev = self
            .last_liveliness_changes
            .load(std::sync::atomic::Ordering::Acquire);
        if prev == changes || (prev < 0 && changes == 0) {
            return;
        }
        let Some(status) = rt.user_reader_liveliness_changed(eid) else {
            return;
        };
        self.entity_state
            .set_status_bits(crate::psm_constants::status::LIVELINESS_CHANGED);
        let chain = self.listener_chain();
        if crate::listener_dispatch::dispatch_liveliness_changed(
            &chain,
            self.entity_state.instance_handle(),
            status,
        ) {
            rt.acknowledge_user_reader_liveliness_changed(eid, &status);
            self.entity_state
                .clear_status_changes(crate::psm_constants::status::LIVELINESS_CHANGED);
            self.read_liveliness_changes(changes);
        }
    }

    /// The liveliness changes up to `changes` were read or handed to a
    /// listener. Until then a look finds them pending — with no listener
    /// attached, for the getter or for a listener set later.
    #[cfg(feature = "std")]
    fn read_liveliness_changes(&self, changes: i64) {
        self.last_liveliness_changes
            .store(changes, std::sync::atomic::Ordering::Release);
    }

    /// `on_requested_incompatible_qos` while the status is unread; see
    /// [`Self::poll_subscription_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_incompatible_qos(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_reader_requested_incompatible_qos_status(eid),
            &self.entity_state,
            crate::psm_constants::status::REQUESTED_INCOMPATIBLE_QOS,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::REQUESTED_INCOMPATIBLE_QOS)
            },
            |status| {
                crate::listener_dispatch::dispatch_requested_incompatible_qos(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| {
                rt.acknowledge_user_reader_requested_incompatible_qos_status(eid, status, reads)
            },
        );
    }

    /// `on_sample_lost` while the status is unread; see [`Self::poll_subscription_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_lost(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_reader_sample_lost_status(eid),
            &self.entity_state,
            crate::psm_constants::status::SAMPLE_LOST,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::SAMPLE_LOST)
            },
            |status| {
                crate::listener_dispatch::dispatch_sample_lost(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| rt.acknowledge_user_reader_sample_lost_status(eid, status, reads),
        );
    }

    /// `on_sample_rejected` while the status is unread; see
    /// [`Self::poll_subscription_matched`]. A reader that never had a
    /// sample rejected finds no move: "0 samples rejected so far" is not
    /// an event.
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_rejected(&self) {
        let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) else {
            return;
        };
        crate::listener_dispatch::poll_ledger(
            || rt.observe_user_reader_sample_rejected_status(eid),
            &self.entity_state,
            crate::psm_constants::status::SAMPLE_REJECTED,
            || {
                self.listener_chain()
                    .takes(crate::psm_constants::status::SAMPLE_REJECTED)
            },
            |status| {
                crate::listener_dispatch::dispatch_sample_rejected(
                    &self.listener_chain(),
                    self.entity_state.instance_handle(),
                    status,
                )
            },
            |status, reads| rt.acknowledge_user_reader_sample_rejected_status(eid, status, reads),
        );
    }
}

#[cfg(feature = "std")]
impl crate::listener_dispatcher::StatusDriver for ReaderStatus {
    /// Every reader status against its last seen value, read from the
    /// runtime as the getters read it. Offline there is nothing to read.
    fn drive(&self) {
        if self.data_left_to_the_pass() {
            self.dispatch_data();
        }
        self.poll_subscription_matched();
        self.poll_requested_deadline_missed();
        self.poll_liveliness_changed();
        self.poll_requested_incompatible_qos();
        self.poll_sample_lost();
        self.poll_sample_rejected();
    }
}

/// The part of a reader its [`Subscriber`] holds; see
/// [`crate::contained`].
#[cfg(feature = "std")]
impl<T: DdsType + Send + 'static> crate::contained::Contained for ReaderInner<T> {
    fn handle(&self) -> InstanceHandle {
        self.entity_state.instance_handle()
    }

    fn topic_name(&self) -> &str {
        self.topic.name()
    }

    fn is_deleted(&self) -> bool {
        self.entity_state.is_deleted()
    }

    fn delete(&self) -> Result<()> {
        if self.status.is_builtin() {
            return Err(DdsError::PreconditionNotMet {
                reason: "a built-in DataReader is not deleted by the application",
            });
        }
        ReaderInner::delete(self);
        Ok(())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
}

#[cfg(feature = "std")]
impl<T: DdsType + Send + 'static> crate::contained::ContainedReader for ReaderInner<T> {
    fn has_conditions(&self) -> bool {
        self.read_conditions
            .lock()
            .map(|c| !c.is_empty())
            .unwrap_or(true)
    }

    fn take_sample_lost_status(&self) -> Result<crate::status::SampleLostStatus> {
        ReaderInner::take_sample_lost_status(self)
    }

    fn delete_conditions(&self) -> Result<()> {
        let mut conditions =
            self.read_conditions
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader conditions poisoned",
                })?;
        for gone in conditions.drain(..) {
            gone.alive
                .store(false, core::sync::atomic::Ordering::Release);
        }
        Ok(())
    }

    fn matching_samples(
        self: Arc<Self>,
        sample_states: u32,
        view_states: u32,
        instance_states: u32,
    ) -> Result<Vec<crate::contained::HeldSample>> {
        let reader = DataReader::from_inner_arc(self);
        let pulled_before = reader
            .inner
            .pulled
            .load(core::sync::atomic::Ordering::Acquire);
        let markers_before = reader
            .inner
            .departure_markers
            .load(core::sync::atomic::Ordering::Acquire);
        let staged = reader
            .inner
            .inbox
            .lock()
            .map(|i| !i.is_empty())
            .unwrap_or(false);
        reader.ingest_pending()?;
        // DDS 1.4 §2.2.4.2.2: what this took into the cache is news until a
        // read, a take or a listener call — get_datareaders is neither, nor
        // a condition. The NOT_ALIVE_NO_WRITERS marker of a departed writer
        // counts too: a WaitSet's ReadCondition that made it took the
        // departure the DATA_AVAILABLE flag and the data listeners looked
        // at.
        if staged
            || reader
                .inner
                .pulled
                .load(core::sync::atomic::Ordering::Acquire)
                > pulled_before
            || reader
                .inner
                .departure_markers
                .load(core::sync::atomic::Ordering::Acquire)
                > markers_before
        {
            reader.inner.status.news.note_fresh();
        }
        let cache = reader
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        Ok(cache
            .iter()
            .filter(|s| {
                s.info
                    .matches_states(sample_states, view_states, instance_states)
            })
            .map(|s| crate::contained::HeldSample {
                time: s.info.source_timestamp,
                group: s
                    .bytes
                    .as_ref()
                    .and_then(|b| b.wire())
                    .and_then(|w| w.group),
            })
            .collect())
    }

    fn notify_data_available(&self) {
        self.status.notify_data_available();
    }

    fn accept_group_released(&self, item: crate::runtime::UserSample) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.push(item);
            // A sample a listener call covered while the group order held
            // it is not new when the release puts it here.
            self.status.news.available.released();
            self.status.news.on_readers.released();
        }
    }

    fn entity_id(&self) -> Option<EntityId> {
        self.entity_id
    }

    fn has_data_on_readers(&self) -> bool {
        self.status.news.has_news(&self.status.news.on_readers)
    }

    fn cover_data_on_readers(&self) {
        self.status.news.on_readers.cover(self.status.news.now());
        self.status
            .news
            .on_readers
            .cover_held(self.status.held_now());
    }

    fn pump_group_channel(self: Arc<Self>) {
        let Some(rx_mu) = self.rx.as_ref() else {
            return;
        };
        // The reader drains its channel itself right now: it hands its
        // samples to the group order as it goes.
        let Ok(rx) = rx_mu.try_lock() else {
            return;
        };
        let me = self.entity_state.instance_handle();
        let reader = DataReader::from_inner_arc(Arc::clone(&self));
        let mut keep = Vec::new();
        while let Ok(raw_item) = rx.try_recv() {
            // As `wait_for_data` pulls into the inbox: staged, then pulled.
            if let Some(room) = self.cache.room() {
                room.note_staged();
            }
            self.pulled
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            if let Some(item) = self.subscriber.offer_to_group(me, raw_item) {
                keep.extend(reader.coherent_gate(item));
            }
        }
        drop(rx);
        if !keep.is_empty() {
            if let Ok(mut inbox) = self.inbox.lock() {
                inbox.extend(keep);
            }
        }
    }
}

/// A DataReader of a [`Subscriber`] whatever its type: what
/// [`Subscriber::get_datareaders`] returns (DDS 1.4 §2.2.2.5.2.10).
/// [`Self::typed`] gives the [`DataReader`] handle of its type.
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct AnyDataReader {
    inner: Arc<dyn crate::contained::ContainedReader>,
}

#[cfg(feature = "std")]
impl core::fmt::Debug for AnyDataReader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AnyDataReader")
            .field("topic", &self.inner.topic_name())
            .field("handle", &self.inner.handle())
            .finish()
    }
}

#[cfg(feature = "std")]
impl AnyDataReader {
    pub(crate) fn from_contained(inner: Arc<dyn crate::contained::ContainedReader>) -> Self {
        Self { inner }
    }

    /// The reader's `InstanceHandle` (DDS 1.4 §2.2.2.1.1.9).
    #[must_use]
    pub fn instance_handle(&self) -> InstanceHandle {
        self.inner.handle()
    }

    /// The name of the topic the reader reads.
    #[must_use]
    pub fn topic_name(&self) -> &str {
        self.inner.topic_name()
    }

    /// The reader as a `DataReader<T>`, or `None` if it reads another type.
    #[must_use]
    pub fn typed<T: DdsType + Send + 'static>(&self) -> Option<DataReader<T>> {
        Arc::clone(&self.inner)
            .into_any()
            .downcast::<ReaderInner<T>>()
            .ok()
            .map(DataReader::from_inner_arc)
    }

    /// `true` if `self` and `other` are handles of the same reader.
    #[must_use]
    pub fn same_reader(&self, other: &Self) -> bool {
        self.inner.handle() == other.inner.handle()
    }
}

/// The end of the last handle deletes a reader that is not deleted yet.
impl<T: DdsType> Drop for ReaderInner<T> {
    fn drop(&mut self) {
        self.delete();
    }
}

impl<T: DdsType> ReaderInner<T> {
    /// The body of `get_sample_lost_status` of the reader and, summed over
    /// the readers, of its Subscriber: the status, with the change since
    /// the last look, which this look resets.
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled
    /// entity is untriggered, all zero (§2.2.2.1.1.7).
    pub(crate) fn take_sample_lost_status(&self) -> Result<crate::status::SampleLostStatus> {
        self.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.runtime, self.entity_id) {
            self.status.poll_sample_lost();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_reader_sample_lost_status(eid)
                .unwrap_or_default();
            self.entity_state
                .clear_status_changes(crate::psm_constants::status::SAMPLE_LOST);
            return Ok(status);
        }
        Ok(crate::status::SampleLostStatus::default())
    }

    /// Deletes the reader (DDS 1.4 §2.2.2.5.2.6) with the ReadConditions
    /// and QueryConditions it created (§2.2.2.5.2.14); the first call does
    /// it, a later one finds it deleted and returns. Removing the runtime
    /// slot sends the SEDP dispose of the subscription.
    pub(crate) fn delete(&self) {
        if !self.entity_state.mark_deleted() {
            return;
        }
        #[cfg(feature = "std")]
        {
            self.subscriber
                .readers
                .remove(self.entity_state.instance_handle());
            let mut conditions = self
                .read_conditions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for gone in conditions.drain(..) {
                gone.alive
                    .store(false, core::sync::atomic::Ordering::Release);
            }
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (self.runtime.as_ref(), self.entity_id) {
            if let Ok(mut ids) = self.subscriber.reader_entity_ids.lock() {
                ids.retain(|e| *e != eid);
            }
            rt.remove_user_reader(eid);
        }
        // A deleted reader is one user of its topic less: DDS 1.4
        // §2.2.2.2.1.6 refuses `delete_topic` only while one exists.
        self.topic
            .detach_user(crate::topic::TopicUserKind::DataReader);
        if let Ok(mut held) = self.description_use.lock() {
            held.take();
        }
        // The readers of a MultiTopic's topics go with the reader of it.
        if let Some(engine) = self.multitopic_engine.get() {
            engine.shutdown();
        }
    }
}

impl<T: DdsType + Send + 'static> DataReader<T> {
    fn from_inner(inner: ReaderInner<T>) -> Self {
        let reader = Self {
            inner: Arc::new(inner),
        };
        // `StatusCondition::get_entity` (DDS 1.4 §2.2.2.1.9.3).
        #[cfg(feature = "std")]
        {
            let weak = Arc::downgrade(&reader.inner);
            reader.inner.entity_state.set_entity_ref(move || {
                weak.upgrade().map(|inner| {
                    crate::entity::EntityRef::DataReader(AnyDataReader {
                        inner: inner as Arc<dyn crate::contained::ContainedReader>,
                    })
                })
            });
        }
        reader
    }

    #[cfg(feature = "std")]
    fn from_inner_arc(inner: Arc<ReaderInner<T>>) -> Self {
        Self { inner }
    }

    #[cfg(feature = "std")]
    fn new_offline(topic: Topic<T>, qos: DataReaderQos, subscriber: Arc<SubscriberInner>) -> Self {
        let entity_state = crate::entity::EntityState::new();
        let instances = InstanceTracker::new();
        let pulled = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let inbox = Arc::new(Mutex::new(Vec::new()));
        let news = ReaderNews::new(None, None, &pulled, &inbox, &instances);
        let status = ReaderStatus::new(None, None, &entity_state, &subscriber, &instances, &news);
        let reader = Self::from_inner(ReaderInner {
            topic,
            qos: Arc::new(Mutex::new(qos)),
            entity_state,
            subscriber,
            status,
            inbox,
            #[cfg(feature = "std")]
            held_back: Arc::new(Mutex::new(Vec::new())),
            description_use: Mutex::new(None),
            keyless_last: Mutex::new(None),
            multitopic_engine: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            coherent_hold: Arc::new(crate::presentation::CoherentHold::new()),
            tbf_hold: Mutex::new(Default::default()),
            tbf_hook: Mutex::new(None),
            departure_markers: core::sync::atomic::AtomicU64::new(0),
            runtime: None,
            entity_id: None,
            rx: None,
            pulled,
            filter: std::sync::RwLock::new(None),
            has_filter: core::sync::atomic::AtomicBool::new(false),
            instances,
            cache: Arc::new(SampleCache::new()),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            #[cfg(feature = "std")]
            read_conditions: Mutex::new(Vec::new()),
            _t: PhantomData,
        });
        reader.install_status_probe();
        reader
    }

    #[cfg(feature = "std")]
    fn new_live(
        topic: Topic<T>,
        qos: DataReaderQos,
        subscriber: Arc<SubscriberInner>,
        runtime: Arc<DcpsRuntime>,
        entity_id: EntityId,
        rx: mpsc::Receiver<crate::runtime::UserSample>,
    ) -> Self {
        let entity_state = crate::entity::EntityState::new();
        let instances = InstanceTracker::new();
        let pulled = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let inbox = Arc::new(Mutex::new(Vec::new()));
        let news = ReaderNews::new(
            Some(Arc::clone(&runtime)),
            Some(entity_id),
            &pulled,
            &inbox,
            &instances,
        );
        let status = ReaderStatus::new(
            Some(Arc::clone(&runtime)),
            Some(entity_id),
            &entity_state,
            &subscriber,
            &instances,
            &news,
        );
        // The dispatcher holds it weakly: dropping the reader ends it.
        runtime
            .listener_dispatcher()
            .register(Arc::downgrade(&status)
                as alloc::sync::Weak<dyn crate::listener_dispatcher::StatusDriver>);
        let reader = Self::from_inner(ReaderInner {
            topic,
            qos: Arc::new(Mutex::new(qos)),
            entity_state,
            subscriber,
            status,
            inbox,
            #[cfg(feature = "std")]
            held_back: Arc::new(Mutex::new(Vec::new())),
            description_use: Mutex::new(None),
            keyless_last: Mutex::new(None),
            multitopic_engine: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            coherent_hold: Arc::new(crate::presentation::CoherentHold::new()),
            tbf_hold: Mutex::new(Default::default()),
            tbf_hook: Mutex::new(None),
            departure_markers: core::sync::atomic::AtomicU64::new(0),
            runtime: Some(runtime),
            entity_id: Some(entity_id),
            rx: Some(Mutex::new(rx)),
            pulled,
            filter: std::sync::RwLock::new(None),
            has_filter: core::sync::atomic::AtomicBool::new(false),
            instances,
            cache: Arc::new(SampleCache::new()),
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            #[cfg(feature = "std")]
            read_conditions: Mutex::new(Vec::new()),
            _t: PhantomData,
        });
        reader.install_status_probe();
        reader
    }

    #[cfg(not(feature = "std"))]
    fn new(topic: Topic<T>, qos: DataReaderQos, subscriber: Arc<SubscriberInner>) -> Self {
        Self::from_inner(ReaderInner {
            topic,
            qos,
            subscriber,
            inbox: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "std")]
            held_back: Arc::new(Mutex::new(Vec::new())),
            description_use: Mutex::new(None),
            keyless_last: Mutex::new(None),
            multitopic_engine: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            #[cfg(feature = "std")]
            coherent_hold: Arc::new(crate::presentation::CoherentHold::new()),
            tbf_hold: Mutex::new(Default::default()),
            tbf_hook: Mutex::new(None),
            departure_markers: core::sync::atomic::AtomicU64::new(0),
            filter: std::sync::RwLock::new(None),
            has_filter: core::sync::atomic::AtomicBool::new(false),
            _t: PhantomData,
        })
    }

    /// Constructor for builtin-topic readers.
    ///
    /// Unlike `new_offline`, this reader shares the inbox with the
    /// `DcpsRuntime` discovery hook: SPDP/SEDP receive pushes an encoded
    /// sample through the same `Arc<Mutex<Vec<crate::runtime::UserSample>>>`,
    /// which is read here via `take()`/`read()`.
    ///
    /// `cache`/`instances` are likewise shared, not fresh, so that every
    /// `DataReader` clone obtained via `BuiltinSubscriber::participant_reader()`
    /// (etc.) and `lookup_datareader` sees the one, consistent post-
    /// `ingest_into_cache` state: `take()`/`read()` (Round 3) move samples
    /// out of the shared `inbox` into the reader's own `cache` on first
    /// access, so a `cache` that were fresh per clone would make a second
    /// clone see nothing (already drained by the first) and would make
    /// `DomainParticipant::delete_contained_entities`'s inbox clear (it
    /// only reaches the shared `inbox`/sink Arcs, not a reader's private
    /// `cache`) leave stale samples behind for any clone that had already
    /// called `take()`/`read()` once.
    #[cfg(feature = "std")]
    pub(crate) fn new_builtin(
        topic: Topic<T>,
        qos: DataReaderQos,
        subscriber: Arc<SubscriberInner>,
        inbox: Arc<Mutex<Vec<crate::runtime::UserSample>>>,
        cache: Arc<SampleCache>,
        instances: InstanceTracker,
    ) -> Self {
        // A builtin reader exists, enabled, with its participant (DDS 1.4
        // §2.2.5): reading it is no operation of a disabled entity.
        let entity_state = crate::entity::EntityState::new_enabled();
        let pulled = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let news = ReaderNews::new(None, None, &pulled, &inbox, &instances);
        let status = ReaderStatus::new(None, None, &entity_state, &subscriber, &instances, &news);
        let reader = Self::from_inner(ReaderInner {
            topic,
            qos: Arc::new(Mutex::new(qos)),
            entity_state,
            subscriber,
            status,
            inbox,
            held_back: Arc::new(Mutex::new(Vec::new())),
            description_use: Mutex::new(None),
            keyless_last: Mutex::new(None),
            multitopic_engine: std::sync::OnceLock::new(),
            last_key: crate::topic::KeyMaterialCache::default(),
            #[cfg(feature = "metrics")]
            metrics: std::sync::OnceLock::new(),
            coherent_hold: Arc::new(crate::presentation::CoherentHold::new()),
            tbf_hold: Mutex::new(Default::default()),
            tbf_hook: Mutex::new(None),
            departure_markers: core::sync::atomic::AtomicU64::new(0),
            runtime: None,
            entity_id: None,
            rx: None,
            pulled,
            filter: std::sync::RwLock::new(None),
            has_filter: core::sync::atomic::AtomicBool::new(false),
            instances,
            cache,
            #[cfg(feature = "flatdata-integration")]
            flat_backend: Mutex::new(None),
            #[cfg(feature = "std")]
            read_conditions: Mutex::new(Vec::new()),
            _t: PhantomData,
        });
        reader.inner.status.set_builtin();
        reader.install_status_probe();
        reader
    }

    /// The StatusChangedFlags (DDS 1.4 §2.2.4.1) as `get_status_changes`
    /// and a StatusCondition see them. DATA_AVAILABLE is the flag of
    /// §2.2.4.2.2 ([`ReaderNews`]), up from an arrival until a read, a take
    /// or an `on_data_available` call. The communication statuses come from
    /// the runtime's ledgers, changed since they were last read or handed
    /// to a listener: at the call, not once a listener look raised the
    /// flag. A look the dispatcher thread runs makes the reader's own look
    /// return at once (`listener_dispatch::poll_ledger`), so until the
    /// dispatcher got to it a status that had changed read as unchanged.
    #[cfg(feature = "std")]
    fn install_status_probe(&self) {
        let news = Arc::clone(&self.inner.status.news);
        // Weak: the entity state must not keep the runtime alive.
        let ledgers = self
            .inner
            .runtime
            .as_ref()
            .map(Arc::downgrade)
            .zip(self.inner.entity_id);
        self.inner.entity_state.set_status_probe(Arc::new(move || {
            let available = if news.has_news(&news.available) {
                crate::psm_constants::status::DATA_AVAILABLE
            } else {
                0
            };
            let communication = ledgers.as_ref().map_or(0, |(runtime, eid)| {
                runtime
                    .upgrade()
                    .map_or(0, |runtime| runtime.user_reader_status_changes(*eid))
            });
            available | communication
        }));
    }

    /// The reception clock behind LIFESPAN purge and DESTINATION_ORDER=
    /// BY_RECEPTION_TIMESTAMP: [`crate::runtime::RuntimeConfig::time_source`]
    /// (C2(6)) when this reader has a runtime, else the platform clock —
    /// same as an offline/builtin reader had before that field existed.
    #[cfg(feature = "std")]
    fn wall_now(&self) -> Time {
        self.inner
            .runtime
            .as_ref()
            .map_or_else(get_current_time, |rt| rt.config.time_source.now())
    }

    /// Sets a content filter that is evaluated on every sample in the
    /// `take()` path. Returning `false` discards the sample.
    ///
    /// Builder style: `reader.with_filter(|s| s.value > 0)`.
    ///
    /// .7a — SQL expression syntax via `set_filter_expression` follows
    /// later.
    #[must_use]
    pub fn with_filter<F>(self, filter: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        *self
            .inner
            .filter
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(filter));
        self.inner
            .has_filter
            .store(true, core::sync::atomic::Ordering::Release);
        self
    }

    /// DDS 1.4 §2.2.2.5.3.27 `get_topicdescription`: the TopicDescription
    /// the reader was created with. A reader from `create_datareader` gives
    /// its Topic; one from `create_datareader_with_cft` the
    /// ContentFilteredTopic, one from `create_datareader_with_multitopic`
    /// the MultiTopic. [`Self::topic`] is the Topic the reader reads from,
    /// which for a ContentFilteredTopic is its related Topic.
    ///
    /// # Errors
    /// `AlreadyDeleted` if the reader or its description is deleted;
    /// `PreconditionNotMet` once the participant is gone.
    #[cfg(feature = "std")]
    pub fn get_topicdescription(&self) -> Result<crate::topic::TopicDescriptionHandle> {
        self.inner.entity_state.check_not_deleted()?;
        let participant =
            self.get_subscriber()
                .get_participant()
                .ok_or(DdsError::PreconditionNotMet {
                    reason: "the participant of the reader is gone",
                })?;
        let held = self
            .inner
            .description_use
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader description hold poisoned",
            })?;
        if let Some(held) = held.as_ref() {
            return participant
                .description_held_by(held)
                .ok_or(DdsError::AlreadyDeleted);
        }
        Ok(crate::topic::TopicDescriptionHandle::new(
            self.inner.topic.name().to_string(),
            self.inner.topic.type_name().to_string(),
            participant,
        ))
    }

    /// The topic being read from.
    #[must_use]
    pub fn topic(&self) -> &Topic<T> {
        &self.inner.topic
    }

    /// The Subscriber that created this reader (DDS 1.4 §2.2.2.5.3
    /// `get_subscriber`).
    #[must_use]
    pub fn get_subscriber(&self) -> Subscriber {
        Subscriber {
            inner: Arc::clone(&self.inner.subscriber),
        }
    }

    /// Spec §2.2.2.5.3.6 / §2.2.2.1.1 — `InstanceHandle` of this
    /// DataReader. A stable identity for
    /// `DomainParticipant::contains_entity`.
    #[must_use]
    pub fn subscription_handle(&self) -> crate::instance_handle::InstanceHandle {
        self.inner.entity_state.instance_handle()
    }

    /// Sets the `DataReaderListener` + StatusMask. `None` clears the
    /// slot. Spec §2.2.2.5.7.x set_listener.
    #[cfg(feature = "std")]
    pub fn set_listener(&self, listener: Option<ArcDataReaderListener>, mask: StatusMask) {
        let listener = listener.or_else(|| {
            (mask != 0).then(|| {
                alloc::sync::Arc::new(crate::listener::NilListener) as ArcDataReaderListener
            })
        });
        if let Ok(mut slot) = self.inner.status.listener.lock() {
            *slot = listener.map(|l| (l, mask));
        }
        self.inner.entity_state.set_listener_mask(mask);
        crate::listener_dispatcher::ListenerDispatcher::listener_set(self.inner.runtime.as_ref());
    }

    /// Current listener clone, if any.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn get_listener(&self) -> Option<ArcDataReaderListener> {
        self.inner
            .status
            .listener
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(l, _)| Arc::clone(l)))
            .filter(|l| !l.is_nil())
    }

    /// Snapshot of the bubble-up chain (Reader → Subscriber → Participant)
    /// for hot-path listener dispatch.
    #[cfg(feature = "std")]
    #[must_use]
    pub(crate) fn listener_chain(&self) -> crate::listener_dispatch::ReaderListenerChain {
        self.inner.status.listener_chain()
    }

    /// Current QoS (cloned, .1).
    #[must_use]
    pub fn qos(&self) -> DataReaderQos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Takes all cached samples and removes them from the cache. Returns
    /// an empty Vec if there is nothing.
    ///
    /// Round 3: routed through the exact same admission
    /// (`ingest_into_cache`/`cache`) `take_with_info()` and the rest of the
    /// `_filtered`/`_w_condition` family use, instead of a second,
    /// hand-rolled loop over the raw mpsc/inbox. That second loop is
    /// exactly how HISTORY/RESOURCE_LIMITS (and, in earlier rounds,
    /// DESTINATION_ORDER/TIME_BASED_FILTER) went unenforced here for as
    /// long as they did: every rule added to `ingest_into_cache` needed a
    /// second, easy-to-forget copy in this function and in `read()`. One
    /// admission step, read from two different places, cannot drift
    /// again. Measured, not assumed, to still be fast enough:
    /// `tests/take_read_perf_probe.rs` (10k samples) — before this change,
    /// ~370–580 ns/sample; after, ~410–650 ns/sample. The cache decodes
    /// every sample once (to evaluate the content filter and compute the
    /// key); it keeps that `T` (`CachedSample::decoded`) and this hands
    /// it out, so a sample is decoded once on its way through `take`.
    ///
    /// # Errors
    /// - `WireError` if a stored payload can no longer be decoded
    ///   (type-eval mismatch).
    pub fn take(&self) -> Result<Vec<T>> {
        self.inner.entity_state.check_enabled()?;
        // Cross-vendor same-host zero-copy: if this reader was
        // `enable_cyclone_iox`-ed, return Cyclone-published samples drained
        // from iceoryx (classic-CDR-decoded) when any are queued — a
        // separate channel this reader's own cache/inbox never sees.
        #[cfg(all(feature = "std", feature = "cyclone-iox", target_os = "linux"))]
        {
            let iox = crate::cyclone_iox_integration::take::<T>(self.inner.topic.name());
            if !iox.is_empty() {
                return Ok(iox);
            }
        }
        #[cfg(feature = "std")]
        {
            Ok(self
                .take_with_wire()?
                .into_iter()
                .map(|(sample, _)| sample)
                .collect())
        }
        #[cfg(not(feature = "std"))]
        {
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            let raw = core::mem::take(&mut *inbox);
            drop(inbox);
            let mut out = Vec::with_capacity(raw.len());
            for staged_item in raw {
                let crate::runtime::UserSample::Alive {
                    payload: bytes,
                    writer_guid,
                    writer_strength,
                    representation,
                    big_endian,
                    source_timestamp,
                    lifespan_nanos,
                    ..
                } = staged_item
                else {
                    continue;
                };
                if live_sample_expired(source_timestamp, lifespan_nanos, self.wall_now()) {
                    continue;
                }
                let sample = match decode_for_encap::<T>(
                    self.inner.topic.codec(),
                    &bytes,
                    representation,
                    big_endian,
                ) {
                    Ok(s) => s,
                    Err(e) if e.is_discarded() => {
                        record_discarded_sample::<T>(self.inner.runtime.as_ref(), &e);
                        continue;
                    }
                    Err(e) => return Err(decode_wire_error::<T>(&e, representation, big_endian)),
                };
                if !self.sample_passes_filter(&sample) {
                    continue;
                }
                if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                    continue;
                }
                out.push(sample);
            }
            Ok(out)
        }
    }

    /// [`Self::take`], with where each sample came from: the RTPS writer
    /// GUID and sequence number of its DATA and its
    /// `PID_RELATED_SAMPLE_IDENTITY`. DDS-RPC 1.0 §7.8.2 identifies a
    /// request by the first two and correlates a reply with the third.
    /// `None` for a sample whose delivery path did not record them (a
    /// sample injected without a wire).
    ///
    /// # Errors
    /// Same as [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_with_wire(&self) -> Result<Vec<(T, Option<crate::sample_bytes::SampleWire>)>> {
        self.inner.entity_state.check_enabled()?;
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        let mut out: Vec<(InstanceHandle, T, Option<crate::sample_bytes::SampleWire>)> =
            Vec::with_capacity(cache.len());
        // The instance of the samples taken last and how many: one
        // tracker call for a run of samples of one instance, where it
        // was two a sample.
        let mut taken: Option<(InstanceHandle, u32)> = None;
        // §2.2.2.5.3.8 rule 5: at most one sample under GROUP with
        // `ordered_access`; what it does not take stays.
        let limit = self.presentation_limit();
        let mut rest = alloc::collections::VecDeque::new();
        for mut s in cache.drain(..) {
            if out.len() >= limit {
                rest.push_back(s);
                continue;
            }
            // A lifecycle marker (dispose/unregister) has no payload to
            // decode into a `T`. `take_with_info()` represents that via
            // `Sample::info.valid_data == false`; the plain
            // take()/read() pair has no such slot (bare `Vec<T>`, no
            // `SampleInfo`) and never did — unchanged from before this
            // routing change.
            let Some(bytes) = s.bytes.as_deref() else {
                continue;
            };
            let wire = s.bytes.as_ref().and_then(|b| b.wire().copied());
            let sample = match take_decoded::<T>(&mut s.decoded) {
                Some(sample) => sample,
                None => match decode_for_encap::<T>(
                    self.inner.topic.codec(),
                    bytes,
                    s.representation,
                    s.big_endian,
                ) {
                    Ok(sample) => sample,
                    Err(e) if e.is_discarded() => {
                        record_discarded_sample::<T>(self.inner.runtime.as_ref(), &e);
                        continue;
                    }
                    Err(e) => {
                        if let Some((handle, n)) = taken {
                            self.inner.instances.note_taken(handle, n);
                        }
                        return Err(decode_wire_error::<T>(&e, s.representation, s.big_endian));
                    }
                },
            };
            // A keyless sample has no instance to note it in.
            let handle = s.info.instance_handle;
            if handle != HANDLE_NIL {
                match taken.as_mut() {
                    Some((run, n)) if *run == handle => *n += 1,
                    _ => {
                        if let Some((run, n)) = taken.replace((handle, 1)) {
                            self.inner.instances.note_taken(run, n);
                        }
                    }
                }
            }
            out.push((handle, sample, wire));
        }
        *cache = rest;
        if let Some((handle, n)) = taken {
            self.inner.instances.note_taken(handle, n);
        }
        // §2.2.2.5.3.8: the samples of an instance consecutive.
        if self.presentation() == Presentation::Grouped {
            group_by_instance(&mut out, |(handle, _, _)| *handle);
        }
        Ok(out
            .into_iter()
            .map(|(_, sample, wire)| (sample, wire))
            .collect())
    }

    /// Helper — evaluates the content filter if set.
    fn sample_passes_filter(&self, sample: &T) -> bool {
        if !self
            .inner
            .has_filter
            .load(core::sync::atomic::Ordering::Acquire)
        {
            return true;
        }
        let filter = self
            .inner
            .filter
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        filter.is_none_or(|f| f(sample))
    }

    /// Spec §2.2.3.23 / §2.2.2.5.5 — exclusive-ownership filter.
    ///
    /// Returns `true` if the sample may be delivered:
    /// - Reader ownership QoS = Shared → always `true` (no filter).
    /// - Keyless topic → always `true` (no per-instance owner state).
    /// - Otherwise: computes the KeyHash and consults
    ///   [`instance_tracker::InstanceTracker::should_accept_sample_under_exclusive_ownership`],
    ///   which holds the (writer_guid, writer_strength) of the
    ///   currently-winning source per instance and rejects samples from
    ///   weaker writers.
    #[cfg(feature = "std")]
    fn passes_exclusive_ownership(
        &self,
        sample: &T,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> bool {
        let (kind, deadline) = {
            let qos = self.inner.qos.lock().unwrap_or_else(|e| e.into_inner());
            (qos.ownership.kind, qos.deadline.period)
        };
        if kind != zerodds_qos::OwnershipKind::Exclusive {
            return true;
        }
        // Spec §2.2.3.23: ownership resolution applies per instance; for
        // keyless topics we treat the topic as a single instance with a
        // synthetic all-zero KeyHash.
        let (kh, key_bytes) = self
            .inner
            .topic
            .codec()
            .key_material(sample)
            .unwrap_or_else(|| ([0u8; 16], Vec::new()));
        // The instance must be registered so the owner tracker can
        // create the slot (`should_accept` otherwise returns `true` for
        // an unknown instance, which bypasses the filtering).
        let _ = self.inner.instances.observe_sample(kh, key_bytes, None);
        // §2.2.3.9.2 (d): the owner committed to the reader's DEADLINE.
        let deadline_nanos = (!deadline.is_infinite()).then(|| deadline.to_nanos());
        self.inner.instances.accept_under_exclusive_ownership(
            &kh,
            writer_guid,
            writer_strength,
            self.wall_now(),
            deadline_nanos,
        )
    }

    /// Removes the oldest not-yet-taken `CachedSample` of `handle` from
    /// `cache` (the entry closest to the front — `cache` is append-ordered,
    /// oldest first) and drains the tracker's count for it. `false` if none
    /// was found (defensive; `samples_in_cache(handle) > 0` should always
    /// mean one exists).
    #[cfg(feature = "std")]
    fn evict_oldest_of_instance(
        &self,
        cache: &mut alloc::collections::VecDeque<CachedSample>,
        handle: InstanceHandle,
    ) -> bool {
        let Some(pos) = cache.iter().position(|s| s.info.instance_handle == handle) else {
            return false;
        };
        cache.remove(pos);
        self.inner.instances.drain_samples(handle, 1);
        true
    }

    /// Removes the globally oldest not-yet-taken `CachedSample` in `cache`
    /// (`cache[0]` — append-ordered) regardless of instance, for
    /// RESOURCE_LIMITS.max_samples' BEST_EFFORT admission path: the newest
    /// sample is let in, and whichever sample has sat longest anywhere in
    /// the reader's cache is discarded (DDS 1.4 §2.2.3.18).
    #[cfg(feature = "std")]
    fn evict_oldest_overall(&self, cache: &mut alloc::collections::VecDeque<CachedSample>) {
        // The oldest sample that holds data: a marker without data holds no room
        // (§2.2.2.5.3.8) and evicting it would free none.
        let Some(victim) = cache
            .iter()
            .position(|c| c.info.valid_data)
            .and_then(|at| cache.remove(at))
        else {
            return;
        };
        if !victim.info.instance_handle.is_nil() {
            self.inner
                .instances
                .drain_samples(victim.info.instance_handle, 1);
        }
    }

    /// Records a RESOURCE_LIMITS rejection on the runtime's user-reader
    /// slot, when live (a no-op offline — like every other status counter,
    /// offline mode has no reader slot to store it in).
    #[cfg(feature = "std")]
    fn note_sample_rejected(
        &self,
        kind: crate::status::SampleRejectedStatusKind,
        handle: InstanceHandle,
    ) {
        if let (Some(rt), Some(eid)) = (self.inner.runtime.as_ref(), self.inner.entity_id) {
            rt.record_sample_rejected(eid, kind, handle);
        }
    }

    /// DDS 1.4 §2.2.3.6 PRESENTATION: `coherent_access` makes a coherent set
    /// atomic for the subscriber — the application sees either every sample of
    /// the set or none of it. The policy sits on the Subscriber (§2.2.3.6 is a
    /// Publisher/Subscriber policy), so the value is read from the parent
    /// Subscriber's QoS, not from the reader's own.
    #[cfg(feature = "std")]
    fn coherent_access_active(&self) -> bool {
        let Ok(qos) = self.inner.subscriber.qos.lock() else {
            return false;
        };
        // DDS 1.4 §2.2.3.6: `access_scope` bounds how far a coherent change
        // may reach, not whether one is honoured at all. At INSTANCE scope
        // "order and coherent changes apply to each instance separately" — the
        // members of a set are still presented together, the guarantee simply
        // does not extend across instances. Gating on `coherent_access` alone
        // is what makes the set visible as a set at every scope; holding a set
        // until its end marker satisfies the INSTANCE guarantee as well as the
        // wider ones.
        if !qos.presentation.coherent_access {
            return false;
        }
        // `dcps.presentation_instance_coherence = none`: the reading of
        // §2.2.3.6 (p. 115) by which a coherent change has "no effect" with
        // INSTANCE scope.
        let instance_scope =
            qos.presentation.access_scope == zerodds_qos::PresentationAccessScope::Instance;
        !(instance_scope
            && self.inner.runtime.as_ref().is_some_and(|rt| {
                rt.config.presentation_instance_coherence
                    == crate::runtime::PresentationInstanceCoherence::None
            }))
    }

    /// How a read or take of this reader presents its samples, from its
    /// Subscriber's PRESENTATION (DDS 1.4 §2.2.2.5.3.8).
    #[cfg(feature = "std")]
    fn presentation(&self) -> Presentation {
        let Ok(qos) = self.inner.subscriber.qos.lock() else {
            return Presentation::Grouped;
        };
        match (
            qos.presentation.access_scope,
            qos.presentation.ordered_access,
        ) {
            (zerodds_qos::PresentationAccessScope::Group, true) => Presentation::OnePerRead,
            (zerodds_qos::PresentationAccessScope::Topic, true) => Presentation::InWriteOrder,
            _ => Presentation::Grouped,
        }
    }

    /// The most samples one read or take returns under
    /// [`Self::presentation`]: one under GROUP with `ordered_access`
    /// (§2.2.2.5.3.8 rule 5), else no limit.
    #[cfg(feature = "std")]
    fn presentation_limit(&self) -> usize {
        match self.presentation() {
            Presentation::OnePerRead => 1,
            _ => usize::MAX,
        }
    }

    /// Puts a batch in write order into the order [`Self::presentation`]
    /// asks for: §2.2.2.5.3.8 rules 1, 2 and 4, "samples belonging to the
    /// same data-instance are consecutive", whatever the scope unless it is
    /// TOPIC or GROUP with `ordered_access`. A stable sort by instance
    /// handle; the writer's order stays inside each instance. Only
    /// `ordered_access` with INSTANCE scope grouped; the default
    /// presentation interleaved the instances.
    #[cfg(feature = "std")]
    fn present_by_access_scope(&self, out: &mut [Sample<T>]) {
        if self.presentation() == Presentation::Grouped {
            group_by_instance(out, |s| s.info.instance_handle);
        }
        self.rank_samples(out);
    }

    /// Fills `sample_rank`, `generation_rank` and `absolute_generation_rank`
    /// of a returned batch (§2.2.2.5.1.6) from the batch itself and the
    /// instance's current generation counts.
    #[cfg(feature = "std")]
    fn rank_samples(&self, out: &mut [Sample<T>]) {
        let mut infos: Vec<SampleInfo> = out.iter().map(|s| s.info).collect();
        crate::sample_info::compute_ranks(&mut infos, |h| {
            self.inner
                .instances
                .get_by_handle(h)
                .map(|st| (st.disposed_generation_count, st.no_writers_generation_count))
                .unwrap_or((0, 0))
        });
        for (s, i) in out.iter_mut().zip(infos) {
            s.info.sample_rank = i.sample_rank;
            s.info.generation_rank = i.generation_rank;
            s.info.absolute_generation_rank = i.absolute_generation_rank;
        }
    }

    /// Delivers into the cache what the TIME_BASED_FILTER held back and is
    /// due (DDS 1.4 §2.2.3.12). `true` if something was. A sample the
    /// RESOURCE_LIMITS refuse waits with the held-back ones.
    #[cfg(feature = "std")]
    fn release_due_tbf(&self) -> Result<bool> {
        let now = self.wall_now();
        let due = match self.inner.tbf_hold.lock() {
            Ok(mut hold) if !hold.is_empty() => hold.take_due(now),
            _ => return Ok(false),
        };
        if due.is_empty() {
            return Ok(false);
        }
        let aq = self.admission_qos();
        let mut refused = Vec::new();
        {
            let mut cache = self
                .inner
                .cache
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader cache poisoned",
                })?;
            for (_, raw) in due {
                if let Some(back) =
                    self.ingest_alive_filtered(&mut cache, now, raw, true, &aq, false)?
                {
                    refused.push(StagedIngest::Alive(back));
                }
            }
        }
        if !refused.is_empty() {
            if let Ok(mut held) = self.inner.held_back.lock() {
                held.extend(refused);
            }
        }
        Ok(true)
    }

    /// Makes the tick hook that delivers what the TIME_BASED_FILTER holds
    /// once it is due, and tells the application of it as of a new sample
    /// (DATA_AVAILABLE, the listeners, a WaitSet). Once per reader.
    #[cfg(feature = "std")]
    fn arm_tbf_hook(&self) {
        let Some(rt) = self.inner.runtime.as_ref() else {
            return;
        };
        let Ok(mut slot) = self.inner.tbf_hook.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
        let weak = Arc::downgrade(&self.inner);
        let hook: crate::runtime::StatusHook = Arc::new(move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let reader = DataReader::from_inner_arc(inner);
            if matches!(reader.release_due_tbf(), Ok(true)) {
                reader.inner.status.news.note_fresh();
                if let Some(rt) = reader.inner.runtime.as_ref() {
                    rt.notify_status_changed();
                }
                crate::condition::signal_data_available();
            }
        });
        rt.add_tick_hook(&hook);
        *slot = Some(hook);
    }

    /// Coherent-set gate for one incoming channel item (DDS 1.4
    /// §2.2.2.4.1.8-9 + §2.2.3.6, wire mechanism DDSI-RTPS 2.5 §8.7.6):
    /// [`crate::presentation::CoherentHold::gate`] under the Subscriber's
    /// `coherent_access`. Returns the items the reader may act on now —
    /// zero, one, or the whole released set.
    #[cfg(feature = "std")]
    fn coherent_gate(&self, item: crate::runtime::UserSample) -> Vec<crate::runtime::UserSample> {
        self.inner
            .coherent_hold
            .gate(item, self.coherent_access_active(), |members| {
                self.report_incomplete_set(members)
            })
    }

    /// Reports the discard of an incomplete coherent set of `members`
    /// changes: a loud event, never a silent loss.
    #[cfg(feature = "std")]
    fn report_incomplete_set(&self, members: usize) {
        if let Some(rt) = self.inner.runtime.as_ref() {
            crate::presentation::report_incomplete_set(rt, self.inner.topic.name(), members);
        }
    }

    /// Number of samples currently held back because their coherent set is
    /// still open (see `Self::coherent_gate`). Introspection hook for tests
    /// and diagnostics.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn coherent_held_len(&self) -> usize {
        let own = self.inner.coherent_hold.len();
        // A GROUP-scope publisher's samples wait in the subscriber's group
        // order (DDSI-RTPS 2.5 §8.7.6).
        own + self
            .inner
            .subscriber
            .group_held_for(self.inner.entity_state.instance_handle())
    }

    /// Reads all samples without removing them. Currently identical to
    /// `take` minus the removal. Sample state (`ReadCondition`
    /// §2.2.2.5.8) follows during wire-up.
    ///
    /// Round 3: routed through `ingest_into_cache`/`cache`, the same
    /// admission `read_with_info()`/`read_filtered()` use — see the
    /// doc comment on `take()` for why the old bypass (a second,
    /// hand-rolled admission loop with its own scan-local
    /// TIME_BASED_FILTER/DESTINATION_ORDER baseline) is gone rather than
    /// patched again.
    ///
    /// # Errors
    /// Same as `take`.
    pub fn read(&self) -> Result<Vec<T>> {
        #[cfg(feature = "std")]
        {
            self.ingest_into_cache()?;
            let mut cache = self
                .inner
                .cache
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader cache poisoned",
                })?;
            let mut out: Vec<(InstanceHandle, T)> = Vec::with_capacity(cache.len());
            // §2.2.2.5.3.8 rule 5, as in `take()`.
            let limit = self.presentation_limit();
            for s in cache.iter_mut() {
                if out.len() >= limit {
                    break;
                }
                // See `take()`: a lifecycle marker has no payload to decode
                // into a bare `T` and never did.
                let Some(bytes) = s.bytes.as_deref() else {
                    continue;
                };
                let sample = match decode_for_encap::<T>(
                    self.inner.topic.codec(),
                    bytes,
                    s.representation,
                    s.big_endian,
                ) {
                    Ok(sample) => sample,
                    Err(e) if e.is_discarded() => {
                        record_discarded_sample::<T>(self.inner.runtime.as_ref(), &e);
                        continue;
                    }
                    Err(e) => {
                        return Err(decode_wire_error::<T>(&e, s.representation, s.big_endian));
                    }
                };
                // Sample-state transition NOT_READ -> READ (spec
                // §2.2.2.5.3.4), same as `read_filtered`. Not consumed —
                // `drain_samples` is not called.
                s.info.sample_state = SampleStateKind::Read;
                self.inner.instances.mark_view_seen(s.info.instance_handle);
                out.push((s.info.instance_handle, sample));
            }
            // §2.2.2.5.3.8: the samples of an instance consecutive.
            if self.presentation() == Presentation::Grouped {
                group_by_instance(&mut out, |(handle, _)| *handle);
            }
            Ok(out.into_iter().map(|(_, sample)| sample).collect())
        }
        #[cfg(not(feature = "std"))]
        {
            let raw = {
                let inbox = self
                    .inner
                    .inbox
                    .lock()
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "datareader inbox poisoned",
                    })?;
                inbox.clone()
            };
            let mut out = Vec::with_capacity(raw.len());
            for staged_item in raw {
                let crate::runtime::UserSample::Alive {
                    payload: bytes,
                    writer_guid,
                    writer_strength,
                    representation,
                    big_endian,
                    source_timestamp,
                    lifespan_nanos,
                    ..
                } = staged_item
                else {
                    continue;
                };
                if live_sample_expired(source_timestamp, lifespan_nanos, self.wall_now()) {
                    continue;
                }
                let sample = match decode_for_encap::<T>(
                    self.inner.topic.codec(),
                    &bytes,
                    representation,
                    big_endian,
                ) {
                    Ok(s) => s,
                    Err(e) if e.is_discarded() => {
                        record_discarded_sample::<T>(self.inner.runtime.as_ref(), &e);
                        continue;
                    }
                    Err(e) => return Err(decode_wire_error::<T>(&e, representation, big_endian)),
                };
                if !self.sample_passes_filter(&sample) {
                    continue;
                }
                if !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength) {
                    continue;
                }
                out.push(sample);
            }
            Ok(out)
        }
    }

    /// Number of matched remote writers. Always 0 in offline mode.
    ///
    /// Spec: OMG DDS 1.4 §2.2.2.5.3.15 `get_matched_publications`.
    ///
    /// Side effect — when the matched count changes versus the last
    /// call, `on_subscription_matched` is fired via the bubble-up chain
    /// (spec §2.2.4.2.6.7).
    #[must_use]
    pub fn matched_publication_count(&self) -> usize {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_subscription_matched();
            return rt.user_reader_matched_count(eid);
        }
        0
    }

    /// `wait_for_historical_data` (DDS 1.4 §2.2.2.5.3): blocks until the
    /// reader holds the historical data of every writer it is matched
    /// with, or `max_wait` passes. A VOLATILE reader expects no history and
    /// returns at once; so does a reader without a runtime.
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`; [`DdsError::Timeout`]
    /// when `max_wait` passed first.
    pub fn wait_for_historical_data(&self, max_wait: core::time::Duration) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        let volatile = self
            .inner
            .qos
            .lock()
            .map(|q| matches!(q.durability.kind, crate::qos::DurabilityKind::Volatile))
            .unwrap_or(true);
        if volatile {
            return Ok(());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            let deadline = std::time::Instant::now() + max_wait;
            loop {
                let seen = rt.reader_progress_generation();
                if rt.user_reader_historical_data_received(eid) {
                    return Ok(());
                }
                let now = std::time::Instant::now();
                if now >= deadline {
                    return Err(DdsError::Timeout);
                }
                rt.wait_reader_progress(seen, deadline - now);
            }
        }
        let _ = max_wait;
        Ok(())
    }

    /// This reader's 16-byte RTPS GUID, when it is bound to a live runtime.
    /// `None` offline.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn rtps_guid(&self) -> Option<[u8; 16]> {
        Some(
            self.inner
                .runtime
                .as_ref()?
                .writer_guid(self.inner.entity_id?),
        )
    }

    /// Announces the DDS-RPC discovery extension of this reader
    /// (DDS-RPC 1.0 §7.6.2.1.2): `service_instance_name`,
    /// `related_datawriter_key` and `topic_aliases`. A reader without a
    /// runtime is discovered by nobody and has nothing to announce.
    ///
    /// # Errors
    /// [`DdsError::BadParameter`] for a `string<256>` that is longer;
    /// otherwise as `DcpsRuntime::announce_subscription`.
    #[cfg(feature = "std")]
    pub fn set_rpc_discovery(&self, ext: &crate::runtime::RpcDiscoveryExt) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt.set_user_reader_rpc_discovery(eid, ext),
            _ => Ok(()),
        }
    }

    /// The writers this reader is matched with and what each announced of
    /// the DDS-RPC discovery extension (§7.6.2.1.1). Empty offline.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn matched_rpc_writers(&self) -> Vec<crate::runtime::RpcPeer> {
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => rt.user_reader_matched_rpc_writers(eid),
            _ => Vec::new(),
        }
    }

    /// The handles of the publications this reader is matched with (DDS
    /// 1.4 §2.2.2.5.3 `get_matched_publications`): discovered writers and
    /// writers of this participant. Empty offline.
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`.
    pub fn get_matched_publications(&self) -> Result<Vec<InstanceHandle>> {
        self.matched_publications()
    }

    /// The handles of the publications this reader is matched with, as
    /// [`Self::get_matched_publications`].
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`.
    pub fn matched_publications(&self) -> Result<Vec<InstanceHandle>> {
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            return Ok(rt.user_reader_matched_publication_handles(eid));
        }
        Ok(Vec::new())
    }

    /// The DCPSPublication data of the publication `handle` names (DDS
    /// 1.4 §2.2.2.5.3 `get_matched_publication_data`), as it was last
    /// announced.
    ///
    /// # Errors
    /// [`DdsError::NotEnabled`] before `enable`; [`DdsError::BadParameter`]
    /// for a handle that names no publication this reader is matched
    /// with, as the specification has it.
    pub fn matched_publication_data(
        &self,
        handle: InstanceHandle,
    ) -> Result<crate::builtin_topics::PublicationBuiltinTopicData> {
        self.inner.entity_state.check_enabled()?;
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            if let Some(data) = rt.user_reader_matched_publication_data(eid, handle) {
                return Ok(crate::builtin_topics::PublicationBuiltinTopicData::from_wire(&data));
            }
        }
        let _ = handle;
        Err(DdsError::BadParameter {
            what: "no publication matched with this reader has this handle",
        })
    }

    /// See [`ReaderStatus::poll_subscription_matched`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_subscription_matched(&self) {
        self.inner.status.poll_subscription_matched();
    }

    /// See [`ReaderStatus::poll_requested_deadline_missed`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_deadline_missed(&self) {
        self.inner.status.poll_requested_deadline_missed();
    }

    /// See [`ReaderStatus::poll_liveliness_changed`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_liveliness_changed(&self) {
        self.inner.status.poll_liveliness_changed();
    }

    /// See [`ReaderStatus::poll_requested_incompatible_qos`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_requested_incompatible_qos(&self) {
        self.inner.status.poll_requested_incompatible_qos();
    }

    /// See [`ReaderStatus::poll_sample_lost`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_lost(&self) {
        self.inner.status.poll_sample_lost();
    }

    /// See [`ReaderStatus::poll_sample_rejected`].
    #[cfg(feature = "std")]
    pub(crate) fn poll_sample_rejected(&self) {
        self.inner.status.poll_sample_rejected();
    }

    /// Blocks until at least `min_count` remote writers are matched or
    /// `timeout` elapses. Event-driven via the runtime condvar
    /// (D.5e phase 1) — wakeup directly when SEDP propagates a match, no
    /// more 20-ms polling.
    ///
    /// # Errors
    /// [`DdsError::Timeout`] if `min_count` is not reached within the
    /// time window.
    #[cfg(feature = "std")]
    pub fn wait_for_matched_publication(
        &self,
        min_count: usize,
        timeout: core::time::Duration,
    ) -> Result<()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if self.matched_publication_count() >= min_count {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(DdsError::Timeout);
            }
            // Live mode: park on the runtime match event. Spurious
            // wakeups are fine — we check the count on the next iteration.
            if let Some(rt) = self.inner.runtime.as_ref() {
                let _ = rt.wait_match_event(deadline - now);
            } else {
                // Offline mode: no match events, sleep fallback.
                std::thread::sleep(core::time::Duration::from_millis(20));
            }
        }
    }

    /// Counter for requested-deadline violations (spec §2.2.4.2.11
    /// `REQUESTED_DEADLINE_MISSED_STATUS`). Monotonically increasing;
    /// rises by 1 per expired deadline window without a received sample.
    /// Offline / INFINITE → 0.
    ///
    /// May fire `on_requested_deadline_missed`.
    #[must_use]
    pub fn requested_deadline_missed_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_requested_deadline_missed();
            return rt.user_reader_requested_deadline_missed(eid);
        }
        0
    }

    /// Current `RequestedIncompatibleQosStatus`. Spec §2.2.4.2.6.5.
    /// May trigger `on_requested_incompatible_qos`.
    #[must_use]
    pub fn requested_incompatible_qos_status(
        &self,
    ) -> crate::status::RequestedIncompatibleQosStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_requested_incompatible_qos();
            return rt.user_reader_requested_incompatible_qos(eid);
        }
        crate::status::RequestedIncompatibleQosStatus::default()
    }

    /// SampleLost counter. Spec §2.2.4.2.6.2.
    #[must_use]
    pub fn sample_lost_count(&self) -> u64 {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_sample_lost();
            return rt.user_reader_sample_lost(eid);
        }
        0
    }

    /// SampleRejected status. Spec §2.2.4.2.6.3.
    #[must_use]
    pub fn sample_rejected_status(&self) -> crate::status::SampleRejectedStatus {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_sample_rejected();
            return rt.user_reader_sample_rejected(eid);
        }
        crate::status::SampleRejectedStatus::default()
    }

    /// Polls all reader statuses once and fires pending listeners.
    /// Convenience helper for tests + periodic tick callers.
    #[cfg(feature = "std")]
    pub fn drive_listeners(&self) {
        crate::listener_dispatcher::StatusDriver::drive(&*self.inner.status);
    }

    /// The two counts of `LIVELINESS_CHANGED` (DDS 1.4 §2.2.4.1):
    /// `(any matched writer alive, alive_count, not_alive_count)` — the
    /// numbers of matched writers currently alive and currently not alive.
    /// Does not reset the status's change fields; see
    /// [`Self::get_liveliness_changed_status`].
    ///
    /// Offline: `(false, 0, 0)`.
    #[must_use]
    pub fn liveliness_changed_status(&self) -> (bool, u64, u64) {
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_liveliness_changed();
            return rt.user_reader_liveliness_status(eid);
        }
        (false, 0, 0)
    }

    /// `get_liveliness_changed_status` (DDS 1.4 §2.2.4.1): the counts, the
    /// changes since the status was last read or handed to a listener, and
    /// the last writer that changed them. Reading resets the change fields.
    ///
    /// Offline: the default status.
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_liveliness_changed_status(&self) -> Result<crate::status::LivelinessChangedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_liveliness_changed();
            let changes = i64::try_from(rt.user_reader_liveliness_changes(eid)).unwrap_or(i64::MAX);
            let status = rt
                .take_user_reader_liveliness_changed(eid)
                .unwrap_or_default();
            self.inner.status.read_liveliness_changes(changes);
            // Read before the flag is cleared, as the other getters; see
            // `get_subscription_matched_status`.
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::LIVELINESS_CHANGED);
            return Ok(status);
        }
        Ok(crate::status::LivelinessChangedStatus::default())
    }

    /// `get_subscription_matched_status` (DDS 1.4 §2.2.2.5.3, §2.2.4.1):
    /// `total_count` counts every writer that ever matched, `current_count`
    /// those matched now, the changes count since the status was last read
    /// or handed to a listener, and the last handle is the writer that last
    /// joined or left. Reading resets the change fields.
    ///
    /// Offline: the default status.
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_subscription_matched_status(
        &self,
    ) -> Result<crate::status::SubscriptionMatchedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_subscription_matched();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_reader_subscription_matched_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::SUBSCRIPTION_MATCHED);
            return Ok(status);
        }
        Ok(crate::status::SubscriptionMatchedStatus::default())
    }

    /// `get_requested_deadline_missed_status` (DDS 1.4 §2.2.2.5.3); as
    /// [`Self::get_subscription_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_requested_deadline_missed_status(
        &self,
    ) -> Result<crate::status::RequestedDeadlineMissedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_requested_deadline_missed();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = self.inner.status.with_missed_instance(
                rt.take_user_reader_requested_deadline_missed_status(eid)
                    .unwrap_or_default(),
            );
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::REQUESTED_DEADLINE_MISSED);
            return Ok(status);
        }
        Ok(crate::status::RequestedDeadlineMissedStatus::default())
    }

    /// `get_requested_incompatible_qos_status` (DDS 1.4 §2.2.2.5.3); as
    /// [`Self::get_subscription_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_requested_incompatible_qos_status(
        &self,
    ) -> Result<crate::status::RequestedIncompatibleQosStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_requested_incompatible_qos();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_reader_requested_incompatible_qos_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::REQUESTED_INCOMPATIBLE_QOS);
            return Ok(status);
        }
        Ok(crate::status::RequestedIncompatibleQosStatus::default())
    }

    /// `get_sample_lost_status` (DDS 1.4 §2.2.2.5.3); as
    /// [`Self::get_subscription_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_sample_lost_status(&self) -> Result<crate::status::SampleLostStatus> {
        self.inner.take_sample_lost_status()
    }

    /// `get_sample_rejected_status` (DDS 1.4 §2.2.2.5.3); as
    /// [`Self::get_subscription_matched_status`].
    ///
    /// # Errors
    /// [`DdsError::AlreadyDeleted`] after delete. The status of a disabled entity is untriggered, all zero (§2.2.2.1.1.7).
    pub fn get_sample_rejected_status(&self) -> Result<crate::status::SampleRejectedStatus> {
        self.inner.entity_state.check_not_deleted()?;
        // DDS 1.4 §2.2.2.1.1.7: "get_status_changes and other get status
        // operations (although the status of a disabled entity never
        // changes)" are allowed on a disabled entity; its status is untriggered.
        if !self.inner.entity_state.is_enabled() {
            return Ok(Default::default());
        }
        #[cfg(feature = "std")]
        if let (Some(rt), Some(eid)) = (&self.inner.runtime, self.inner.entity_id) {
            self.poll_sample_rejected();
            // Taken before the flag is cleared (DDS 1.4 §2.2.4.1): a listener
            // look that saw the status unread sets the flag and then looks
            // again (`poll_ledger`), so the order is what keeps a read status
            // from being flagged once more.
            let status = rt
                .take_user_reader_sample_rejected_status(eid)
                .unwrap_or_default();
            self.inner
                .entity_state
                .clear_status_changes(crate::psm_constants::status::SAMPLE_REJECTED);
            return Ok(status);
        }
        Ok(crate::status::SampleRejectedStatus::default())
    }

    /// Blocks up to `timeout` for at least one sample to be available, staging
    /// one delivered mpsc sample into the inbox (live mode). Returns
    /// `Timeout` if none arrives. Offline mode: `Ok` iff the inbox is non-empty.
    ///
    /// # Errors
    /// `Timeout` on no data; `PreconditionNotMet` if an internal lock is poisoned.
    pub fn wait_for_data(&self, timeout: core::time::Duration) -> Result<()> {
        self.inner.entity_state.check_enabled()?;
        let Some(rx_mu) = self.inner.rx.as_ref() else {
            // Offline mode: if the inbox already has something, OK,
            // otherwise timeout.
            let inbox_has = self
                .inner
                .inbox
                .lock()
                .map(|i| !i.is_empty())
                .unwrap_or(false);
            if inbox_has {
                return Ok(());
            }
            return Err(DdsError::Timeout);
        };

        // Anything already in the staging inbox?
        {
            let inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            if !inbox.is_empty() {
                return Ok(());
            }
        }

        let rx = rx_mu.lock().map_err(|_| DdsError::PreconditionNotMet {
            reason: "datareader rx poisoned",
        })?;
        let result = match rx.recv_timeout(timeout) {
            Ok(raw_item) => {
                // Staged before pulled: see `ReaderRoom::budget`.
                if let Some(room) = self.inner.cache.room() {
                    room.note_staged();
                }
                let pulled_before = self
                    .inner
                    .pulled
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                // DDS 1.4 §2.2.3.6: a member of an open coherent set must not
                // become visible, so it never reaches the staging inbox. The
                // wake-up itself still counts — the caller loops.
                // A lifecycle marker is staged like a data sample: applied
                // here, it would overtake a data sample of its instance still
                // waiting in the inbox (see `ingest_into_cache`).
                // The group order before the inbox: `release_group` takes
                // them in that order.
                let me = self.inner.entity_state.instance_handle();
                let gated = match self.inner.subscriber.offer_to_group(me, raw_item) {
                    Some(raw_item) => self.coherent_gate(raw_item),
                    None => Vec::new(),
                };
                {
                    let mut inbox =
                        self.inner
                            .inbox
                            .lock()
                            .map_err(|_| DdsError::PreconditionNotMet {
                                reason: "datareader inbox poisoned",
                            })?;
                    let before = inbox.len();
                    for item in gated {
                        match item {
                            crate::runtime::UserSample::CoherentSetEnd { .. } => {
                                // Consumed by `coherent_gate` — never yielded.
                            }
                            sample => inbox.push(sample),
                        }
                    }
                    self.inner
                        .status
                        .news
                        .note_parked(pulled_before, inbox.len() - before);
                }
                if self.inner.subscriber.group_order_active() {
                    self.inner.subscriber.pump_group(me);
                    self.inner.subscriber.release_group();
                }
                Ok(())
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(DdsError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(DdsError::PreconditionNotMet {
                    reason: "datareader rx disconnected",
                })
            }
        };
        drop(rx);
        // The listeners hear of the sample from the dispatcher, which the
        // receive path woke when it queued it (`install_data_signal`);
        // firing them here too told them twice. A WaitSet re-checks.
        if result.is_ok() {
            crate::condition::signal_data_available();
        }
        result
    }

    /// Builtin-topic helper: returns the Arc to the shared inbox (reader
    /// clones share the same buffer).
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn __inbox_handle(&self) -> Arc<Mutex<Vec<crate::runtime::UserSample>>> {
        Arc::clone(&self.inner.inbox)
    }

    /// Builtin-topic helper: returns the Arc to the shared reader cache
    /// (reader clones share the same buffer) — see `new_builtin`'s doc
    /// comment for why this must travel with `__inbox_handle`.
    #[cfg(feature = "std")]
    pub(crate) fn __cache_handle(&self) -> Arc<SampleCache> {
        Arc::clone(&self.inner.cache)
    }

    /// Builtin-topic helper: returns a handle to the shared
    /// `InstanceTracker` (its `Clone` shares the inner `Arc`, not a fresh
    /// tracker) — see `new_builtin`'s doc comment.
    #[cfg(feature = "std")]
    pub(crate) fn __instances_handle(&self) -> InstanceTracker {
        self.inner.instances.clone()
    }

    /// Builtin-topic helper: drops every already-ingested cached sample
    /// and per-instance tracker entry. Clearing a builtin reader's shared
    /// `inbox` (as `DomainParticipant::delete_contained_entities` already
    /// did before `take()`/`read()` routed through the cache) is no
    /// longer enough on its own — a prior `take()`/`read()` call may have
    /// already moved samples out of the inbox and into the cache, where
    /// an inbox clear alone cannot reach them.
    #[cfg(feature = "std")]
    pub(crate) fn __reset_builtin_state(&self) {
        if let Ok(mut cache) = self.inner.cache.lock() {
            cache.clear();
        }
        self.inner.instances.clear();
    }

    /// Test helper: inserts an encoded payload into the inbox.
    /// At runtime this is replaced by the ReliableReader delivery path.
    ///
    /// Triggers the listener bubble-up chain `on_data_on_readers`
    /// (subscriber stage) and `on_data_available` (reader stage). Spec
    /// §2.2.4.2.7.1 / §2.2.4.2.6.1.
    #[doc(hidden)]
    pub fn __push_raw(&self, bytes: Vec<u8>) -> Result<()> {
        self.__push_raw_with_writer(bytes, [0u8; 16], 0)
    }

    /// Test hook: pushes a stripped body with an explicit encapsulation
    /// (`representation`: `0`=XCDR1 / `1`=XCDR2, `big_endian`) so the cache
    /// materialization path can be exercised for each wire form. Regression
    /// guard for the reader that must re-decode a cached XCDR1 body as XCDR1
    /// (not the fixed XCDR2 `T::decode`).
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn __push_raw_encap(
        &self,
        bytes: Vec<u8>,
        representation: u8,
        big_endian: bool,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        {
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            inbox.push(crate::runtime::UserSample::Alive {
                key_hash: None,
                payload: crate::sample_bytes::SampleBytes::from_vec(bytes),
                writer_guid: [0u8; 16],
                writer_strength: 0,
                representation,
                big_endian,
                source_timestamp: None,
                reception_timestamp: None,
                source_sequence_number: -1,
                coherent_set_sn: None,
                // Test hook: no LIFESPAN — `0` == INFINITE.
                lifespan_nanos: 0,
                related_sample_identity: None,
                group: None,
                crypto_content: false,
            });
        }
        self.notify_data_arrived();
        Ok(())
    }

    /// Test hook: pushes a sample with an explicit writer GUID and
    /// `ownership_strength` into the inbox. Used by the Cyclone interop
    /// harness and the exclusive-ownership tests.
    #[doc(hidden)]
    pub fn __push_raw_with_writer(
        &self,
        bytes: Vec<u8>,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> Result<()> {
        // A deleted reader receives nothing.
        self.inner.entity_state.check_not_deleted()?;
        {
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            inbox.push(crate::runtime::UserSample::Alive {
                key_hash: None,
                payload: crate::sample_bytes::SampleBytes::from_vec(bytes),
                writer_guid,
                writer_strength,
                // Test hook: raw-bytes injection, XCDR1 baseline, little-endian.
                representation: 0,
                big_endian: false,
                source_timestamp: None,
                reception_timestamp: None,
                source_sequence_number: -1,
                coherent_set_sn: None,
                // Test hook: no LIFESPAN — `0` == INFINITE.
                lifespan_nanos: 0,
                related_sample_identity: None,
                group: None,
                crypto_content: false,
            });
        }
        // Listener notify outside the inbox lock to avoid re-entrancy.
        self.notify_data_arrived();
        Ok(())
    }

    /// Test helper: like [`Self::__push_raw_with_writer`], with the RTPS
    /// sequence number of the sample and the sample it relates to, as the
    /// receive path hands them over from the DATA submessage.
    #[doc(hidden)]
    pub fn __push_raw_related(
        &self,
        bytes: Vec<u8>,
        writer_guid: [u8; 16],
        sequence_number: i64,
        related: Option<zerodds_rtps::inline_qos::SampleIdentityBytes>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        {
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            inbox.push(crate::runtime::UserSample::Alive {
                key_hash: None,
                payload: crate::sample_bytes::SampleBytes::from_vec(bytes),
                writer_guid,
                writer_strength: 0,
                representation: 0,
                big_endian: false,
                source_timestamp: None,
                reception_timestamp: None,
                source_sequence_number: sequence_number,
                coherent_set_sn: None,
                lifespan_nanos: 0,
                related_sample_identity: related,
                crypto_content: false,
                group: None,
            });
        }
        self.notify_data_arrived();
        Ok(())
    }

    /// Tells the listeners of the arrival, as the receive path does
    /// ([`ReaderStatus::poll_data_available`]): DDS 1.4 §2.2.4.4 tries
    /// `on_data_on_readers` with the subscriber first, and only if no
    /// listener takes it `on_data_available` with the reader; "if
    /// on_data_on_readers is called, then the middleware will not try to
    /// call on_data_available", which `Subscriber::notify_datareaders`
    /// can then do.
    ///
    /// Spec §2.2.4.6: both notification mechanisms may be enabled at once.
    /// The listener chain runs first, then every blocked `WaitSet` is told
    /// that a sample landed — so a reader whose samples arrive over this
    /// path (the inbox sites, and the channel drain in `wait_for_data`)
    /// wakes a wait the same way the receive path in `runtime.rs` does.
    #[cfg(feature = "std")]
    pub(crate) fn notify_data_arrived(&self) {
        // DDS 1.4 §2.2.2.1.1.7: "The Listeners associated with an entity are
        // not called until the entity is enabled."
        if !self.inner.entity_state.is_enabled() {
            crate::condition::signal_data_available();
            return;
        }
        let chain = self.listener_chain();
        let reader_handle = self.inner.entity_state.instance_handle();
        let subscriber = self.inner.subscriber.entity_state.instance_handle();
        if crate::listener_dispatch::dispatch_data_on_readers(&chain, subscriber) {
            self.inner
                .status
                .data_available_pending
                .store(true, core::sync::atomic::Ordering::Release);
        } else {
            crate::listener_dispatch::dispatch_data_available(&chain, reader_handle);
        }
        crate::condition::signal_data_available();
    }

    // ========================================================================
    // SampleInfo statechart + instance lifecycle.
    // Spec §2.2.2.5.1, §2.2.2.5.3.{5,27,28}.
    // ========================================================================

    /// Returns the current [`InstanceTracker`] (shared with the internal
    /// bookkeeping). Mainly for tests / inspection.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn instance_tracker(&self) -> InstanceTracker {
        self.inner.instances.clone()
    }

    /// This reader's instance handle (GUID-derived). Lets the application
    /// ignore the reader's own subscription — e.g. a durability service
    /// ignoring its ingest reader on the replay-writer side to avoid an echo
    /// loop. Mirrors [`crate::DataWriter::instance_handle`].
    #[must_use]
    pub fn instance_handle(&self) -> crate::instance_handle::InstanceHandle {
        self.inner.entity_state.instance_handle()
    }

    /// Returns (Runtime, EntityId) when the reader runs in live mode.
    /// Cross-crate hook for the async layer (dcps-async), which must
    /// register the waker slot directly.
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn runtime_handle(
        &self,
    ) -> Option<(alloc::sync::Arc<crate::runtime::DcpsRuntime>, EntityId)> {
        match (&self.inner.runtime, self.inner.entity_id) {
            (Some(rt), Some(eid)) => Some((alloc::sync::Arc::clone(rt), eid)),
            _ => None,
        }
    }

    /// Spec §2.2.3.23 — hook for "writer X lost liveliness". Does two
    /// things:
    ///   1. clears the OWNERSHIP=EXCLUSIVE owner for all instances whose
    ///      owner was this writer (so the next sample from another writer
    ///      can win again via
    ///      `should_accept_sample_under_exclusive_ownership`);
    ///   2. returns the number of affected instances.
    ///
    /// The reader clears the ownership of a writer that lost its liveliness
    /// itself (`apply_unmatched_writers`, fed by `LivelinessTracker`); this
    /// hook is for a caller that knows the writer is gone earlier
    /// (`tests/ownership_failover.rs`).
    #[must_use]
    pub fn notify_writer_liveliness_lost(&self, writer_guid: [u8; 16]) -> usize {
        self.inner.instances.clear_owner_for_writer(writer_guid)
    }

    /// Like [`Self::notify_writer_liveliness_lost`], but matches only on
    /// the first 12 bytes (GuidPrefix). Allows failover when only the
    /// participant identity is known (a failover that learns of the loss of a
    /// participant).
    #[must_use]
    pub fn notify_participant_liveliness_lost(&self, prefix: [u8; 12]) -> usize {
        self.inner.instances.clear_owner_for_writer_prefix(prefix)
    }

    /// Turns a sample value into its corresponding local
    /// [`InstanceHandle`], or [`HANDLE_NIL`] if unknown / non-keyed.
    /// Spec §2.2.2.5.3.26 `lookup_instance` (reader variant).
    #[cfg(feature = "std")]
    #[must_use]
    pub fn lookup_instance(&self, instance: &T) -> InstanceHandle {
        if !self.inner.topic.is_keyed() {
            return self
                .inner
                .instances
                .lookup(&crate::topic::KEYLESS_INSTANCE_KEY)
                .unwrap_or(HANDLE_NIL);
        }
        let Some((kh, _)) = self.inner.topic.codec().key_material(instance) else {
            return HANDLE_NIL;
        };
        self.inner.instances.lookup(&kh).unwrap_or(HANDLE_NIL)
    }

    /// Spec §2.2.2.5.3.25 `get_key_value`. Returns the sample value with
    /// only the `@key` fields filled in (reconstructed from the stored
    /// key holder via `T::decode_key_holder_be`).
    ///
    /// # Errors
    /// `BadParameter` if the handle is unknown, or if only the hash of the
    /// instance is known and it does not spell the key out; `WireError` if
    /// `T::decode_key_holder_be` cannot read it.
    #[cfg(feature = "std")]
    pub fn get_key_value(&self, handle: InstanceHandle) -> Result<T> {
        self.inner.entity_state.check_enabled()?;
        let Some(state) = self.inner.instances.get_by_handle(handle) else {
            return Err(DdsError::BadParameter {
                what: "unknown instance handle",
            });
        };
        if !self.inner.topic.is_keyed() {
            return self.decode_or_keyholder(None, 0, false, handle);
        }
        // An instance known by its hash alone: a key of at most sixteen
        // octets is spelled out by it (DDSI-RTPS 2.5 §9.6.4.8 Step 5.1).
        let holder = state
            .key_holder
            .filter(|holder| !holder.is_empty())
            .or_else(|| {
                let hash = self.inner.instances.key_hash_for_handle(handle)?;
                self.inner
                    .last_key
                    .holder_from_hash(self.inner.topic.codec(), hash)
            });
        let Some(bytes) = holder else {
            return Err(DdsError::BadParameter {
                what: "the key of this instance is known by its hash only, which does not spell it out",
            });
        };
        self.inner
            .topic
            .codec()
            .decode_key_holder(&bytes)
            .map_err(|e| DdsError::WireError {
                message: alloc::string::ToString::to_string(&e),
            })
    }

    /// Drains all pending bytes from rx + inbox into the internal sample
    /// cache. For each sample the KeyHash is computed, the instance is
    /// registered (if new), and a matching [`SampleInfo`] is created.
    ///
    /// Called automatically by the `*_with_info`/`*_instance` APIs.
    #[cfg(feature = "std")]
    fn ingest_into_cache(&self) -> Result<()> {
        self.check_group_access()?;
        self.inner.status.note_read_or_take();
        self.ingest_pending()
    }

    /// DDS 1.4 §2.2.2.5.2.8: under a Subscriber with PRESENTATION
    /// `access_scope = GROUP`, "the operation begin_access must be called
    /// prior to calling any of the sample-accessing operations, namely:
    /// get_datareaders on the Subscriber and read, take, read_w_condition,
    /// take_w_condition on any DataReader. Otherwise the sample-accessing
    /// operations will return the error PRECONDITION_NOT_MET." Inside a data
    /// listener it is not required (§2.2.2.5.1, the pattern of
    /// `on_data_on_readers`). Only `get_datareaders` asked for it.
    #[cfg(feature = "std")]
    fn check_group_access(&self) -> Result<()> {
        let group = self.inner.subscriber.qos.lock().is_ok_and(|qos| {
            qos.presentation.access_scope == zerodds_qos::PresentationAccessScope::Group
        });
        if group
            && !self.inner.subscriber.access_scope.is_active()
            && !crate::listener_dispatch::in_data_listener()
        {
            return Err(DdsError::PreconditionNotMet {
                reason: "a read or take under GROUP access scope needs begin_access",
            });
        }
        Ok(())
    }

    /// What arrived since the last ingest, into the cache; without the
    /// read or take that [`Self::ingest_into_cache`] stands for.
    fn ingest_pending(&self) -> Result<()> {
        // DDS 1.4 §2.2.2.1.1.7: read and take are not among the operations
        // a disabled entity takes; they return NOT_ENABLED.
        self.inner.entity_state.check_enabled()?;
        // A reader of a MultiTopic joins what its topics received since the
        // last ingest first (DDS 1.4 §2.2.2.3.4).
        if let Some(engine) = self.inner.multitopic_engine.get() {
            engine.pump()?;
        }
        // What the TIME_BASED_FILTER held back and is due arrived before
        // everything below.
        self.release_due_tbf()?;
        // Everything delivered since the last ingest, in the order it
        // arrived: the staging inbox first (`wait_for_data` parks there what
        // it received), then the channel. A lifecycle marker is applied at
        // its place in this order, never before a data sample that arrived
        // ahead of it: under HISTORY KEEP_LAST (§2.2.3.18) a data sample
        // ingested after the marker of its instance would evict the marker,
        // and the application would never see the NOT_ALIVE transition.
        //
        // Wave 2.1 zero-copy: a data sample carries `SampleBytes`
        // (refcounted Arc<[u8]>); decode goes via Deref<[u8]> without a
        // copy. Spec: docs/specs/zerodds-zero-copy-1.0.md §6 wave 2.1.
        // What a RELIABLE admission held back last time goes first: it
        // arrived before everything below.
        let mut staged: Vec<StagedIngest> =
            core::mem::take(&mut *self.inner.held_back.lock().map_err(|_| {
                DdsError::PreconditionNotMet {
                    reason: "datareader held-back samples poisoned",
                }
            })?);
        // Its first entry was counted as rejected when it was refused; a
        // second refusal of the same sample is not a second rejection.
        let already_counted = !staged.is_empty();
        {
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            for item in inbox.drain(..) {
                if let Some(entry) = StagedIngest::from_user_sample(item) {
                    staged.push(entry);
                }
            }
        }
        if let Some(rx_mu) = self.inner.rx.as_ref() {
            let rx = rx_mu.lock().map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader rx poisoned",
            })?;
            while let Ok(raw_item) = rx.try_recv() {
                // Staged before pulled: see `ReaderRoom::budget`.
                if let Some(room) = self.inner.cache.room() {
                    room.note_staged();
                }
                self.inner
                    .pulled
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                // A GROUP-scope publisher's sample waits in the subscriber's
                // group order (DDSI-RTPS 2.5 §8.7.5); it comes back below,
                // or to this reader's inbox, once committed.
                // A reader of a MultiTopic reader's own is not in the
                // subscriber's list of readers, to which the group order
                // would hand a sample back: its samples go past it.
                let offered = if self.inner.status.is_internal() {
                    Some(raw_item)
                } else {
                    self.inner
                        .subscriber
                        .offer_to_group(self.inner.entity_state.instance_handle(), raw_item)
                };
                let Some(raw_item) = offered else {
                    continue;
                };
                // DDS 1.4 §2.2.3.6: members of an open coherent set are held
                // back and never enter the reader cache.
                for item in self.coherent_gate(raw_item) {
                    if let Some(entry) = StagedIngest::from_user_sample(item) {
                        staged.push(entry);
                    }
                }
            }
        }
        if self.inner.subscriber.group_order_active() {
            // The siblings' channels first: a number this reader's samples
            // wait for may sit on one of them. What the release hands this
            // reader is in its inbox, behind what an earlier release put
            // there.
            let me = self.inner.entity_state.instance_handle();
            self.inner.subscriber.pump_group(me);
            self.inner.subscriber.release_group();
            let mut inbox = self
                .inner
                .inbox
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader inbox poisoned",
                })?;
            for item in inbox.drain(..) {
                if let Some(entry) = StagedIngest::from_user_sample(item) {
                    staged.push(entry);
                }
            }
        }
        let now = self.wall_now();
        {
            let mut cache = self
                .inner
                .cache
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "datareader cache poisoned",
                })?;
            // §2.2.3.16 LIFESPAN runs on every ingest, with or without new
            // bytes: a sample already in the cache expires while the reader
            // is idle.
            purge_expired_by_lifespan(now, &mut cache, &self.inner.instances);
        }
        let mut pending = staged.into_iter().peekable();
        let mut first_entry = true;
        let mut refused: Option<RawIngestSample> = None;
        let aq = self.admission_qos();
        let ran =
            (|| -> Result<()> {
                'run: while let Some(entry) = pending.next() {
                    let count_rejection = !(first_entry && already_counted);
                    first_entry = false;
                    match entry {
                        StagedIngest::Lifecycle(kh, holder, kind, writer, source_ts) => {
                            let (kh, holder) = self.lifecycle_instance(kh, holder);
                            let writer = (writer != [0u8; 16]).then_some(writer);
                            if let (true, Some(w)) = (aq.exclusive_ownership, writer) {
                                match kind {
                                    // DDS 1.4 §2.2.2.5.1.3: under EXCLUSIVE
                                    // the instance "becomes
                                    // NOT_ALIVE_DISPOSED only if the
                                    // DataWriter that 'owns' the instance
                                    // explicitly disposes it".
                                    InstanceStateKind::NotAliveDisposed
                                        if self
                                            .inner
                                            .instances
                                            .owner_of(&kh)
                                            .is_some_and(|owner| owner != w) =>
                                    {
                                        continue;
                                    }
                                    // §2.2.3.23.3: "if the current owner of
                                    // an instance unregisters it, then it
                                    // will relinquish ownership".
                                    InstanceStateKind::NotAliveNoWriters => {
                                        self.inner.instances.release_owner_of(&kh, w);
                                    }
                                    _ => {}
                                }
                            }
                            // `push_lifecycle_with_holder` takes the cache lock itself.
                            let source_ts = source_ts.map(crate::time::he_timestamp_to_time);
                            self.push_lifecycle_from(kh, holder, kind, writer, source_ts)?;
                        }
                        StagedIngest::Alive(first) => {
                            // A run of data samples goes in under one lock.
                            let mut cache = self.inner.cache.lock().map_err(|_| {
                                DdsError::PreconditionNotMet {
                                    reason: "datareader cache poisoned",
                                }
                            })?;
                            if let Some(sample) =
                                self.ingest_alive(&mut cache, now, first, count_rejection, &aq)?
                            {
                                refused = Some(sample);
                                break 'run;
                            }
                            while let Some(StagedIngest::Alive(_)) = pending.peek() {
                                if let Some(StagedIngest::Alive(next)) = pending.next() {
                                    if let Some(sample) =
                                        self.ingest_alive(&mut cache, now, next, true, &aq)?
                                    {
                                        refused = Some(sample);
                                        break 'run;
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            })();
        ran?;
        // A refused sample holds back what came after it, so that nothing
        // overtakes it: the next ingest starts there.
        if let Some(sample) = refused {
            let mut held =
                self.inner
                    .held_back
                    .lock()
                    .map_err(|_| DdsError::PreconditionNotMet {
                        reason: "datareader held-back samples poisoned",
                    })?;
            held.push(StagedIngest::Alive(sample));
            held.extend(pending);
        }
        // A writer that left (§2.2.2.5.1 NOT_ALIVE_NO_WRITERS) delivered what
        // it sent before it left; that has been ingested above, so the
        // no-writers markers come after it.
        self.apply_unmatched_writers()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        // Spec §2.2.3.22 ReaderDataLifecycle: remove instances from the
        // tracker and cache that have been in NotAlive-Disposed or
        // NotAlive-NoWriters longer than autopurge_*_samples_delay.
        self.run_reader_autopurge(now, &mut cache);
        // The cache reports its count as the lock goes; what waits outside
        // it is reported after, so the room is never seen larger than it is.
        drop(cache);
        self.report_staged()
    }

    /// Reports to the reader's room ([`crate::runtime::ReaderRoom`]) the
    /// samples this reader took off its channel and holds outside its
    /// cache: staged by `wait_for_data`, held back for room, or held in an
    /// open coherent set.
    fn report_staged(&self) -> Result<()> {
        let Some(room) = self.inner.cache.room() else {
            return Ok(());
        };
        let poisoned = DdsError::PreconditionNotMet {
            reason: "datareader staging poisoned",
        };
        let staged = self.inner.inbox.lock().map_err(|_| poisoned.clone())?.len()
            + self
                .inner
                .held_back
                .lock()
                .map_err(|_| poisoned.clone())?
                .len()
            + self.inner.coherent_hold.len();
        room.set_staged(staged);
        Ok(())
    }

    /// [`AdmissionQos`] from the reader's QoS as it is now.
    #[cfg(feature = "std")]
    fn admission_qos(&self) -> AdmissionQos {
        let qos = self.inner.qos.lock().unwrap_or_else(|e| e.into_inner());
        AdmissionQos {
            min_sep_nanos: qos.time_based_filter.minimum_separation.to_nanos(),
            tbf_clock: self
                .inner
                .runtime
                .as_ref()
                .map(|rt| rt.config.reader_time_based_filter_clock)
                .unwrap_or_default(),
            by_source_ts: qos.destination_order.kind
                == zerodds_qos::DestinationOrderKind::BySourceTimestamp,
            reliable: qos.reliability.kind == ReliabilityKind::Reliable,
            exclusive_ownership: qos.ownership.kind == zerodds_qos::OwnershipKind::Exclusive,
            history_kind: qos.history.kind,
            history_depth: qos.history.depth,
            max_samples: qos.resource_limits.max_samples,
            max_samples_per_instance: qos.resource_limits.max_samples_per_instance,
            max_instances: qos.resource_limits.max_instances,
        }
    }

    /// One data sample of [`Self::ingest_into_cache`]'s ordered run: decode,
    /// filters, HISTORY/RESOURCE_LIMITS admission, into `cache`.
    #[cfg(feature = "std")]
    fn ingest_alive(
        &self,
        cache: &mut alloc::collections::VecDeque<CachedSample>,
        now: Time,
        raw: RawIngestSample,
        count_rejection: bool,
        aq: &AdmissionQos,
    ) -> Result<Option<RawIngestSample>> {
        self.ingest_alive_filtered(cache, now, raw, count_rejection, aq, true)
    }

    /// [`Self::ingest_alive`]; `time_filter` off for a sample the
    /// TIME_BASED_FILTER held back and now delivers.
    #[cfg(feature = "std")]
    fn ingest_alive_filtered(
        &self,
        cache: &mut alloc::collections::VecDeque<CachedSample>,
        now: Time,
        raw: RawIngestSample,
        count_rejection: bool,
        aq: &AdmissionQos,
        time_filter: bool,
    ) -> Result<Option<RawIngestSample>> {
        let (
            bytes,
            writer_guid,
            writer_strength,
            src_ts,
            big_endian,
            representation,
            lifespan_nanos,
            reception_ts,
        ) = raw;
        // RTPS-F1: use the writer's INFO_TS source timestamp for the
        // SampleInfo + DESTINATION_ORDER; fall back to reception time
        // (`now`) when the writer sent none.
        let sample_source_ts = src_ts.map_or(now, crate::time::he_timestamp_to_time);
        // DDS 1.4 §2.2.3.16 LIFESPAN: "The 'expiration time' of each sample
        // is computed by adding the duration specified by the LIFESPAN QoS
        // to the source timestamp." A sample that already expired while in
        // flight is never delivered to the application.
        let expires_at = lifespan_expiry(sample_source_ts, lifespan_nanos);
        if matches!(expires_at, Some(e) if e <= now) {
            return Ok(None);
        }
        // Decode T to (a) evaluate the filter and (b) compute the
        // KeyHash.
        // §7.2.4.1, as above.
        let sample = match decode_for_encap::<T>(
            self.inner.topic.codec(),
            &bytes,
            representation,
            big_endian,
        ) {
            Ok(s) => s,
            Err(e) if e.is_discarded() => {
                record_discarded_sample::<T>(self.inner.runtime.as_ref(), &e);
                return Ok(None);
            }
            Err(e) => return Err(decode_wire_error::<T>(&e, representation, big_endian)),
        };
        if !self.sample_passes_filter(&sample) {
            return Ok(None);
        }
        // §2.2.3.23 exclusive-ownership filter: reject samples from
        // weaker writers before they enter the cache.
        if aq.exclusive_ownership
            && !self.passes_exclusive_ownership(&sample, writer_guid, writer_strength)
        {
            return Ok(None);
        }
        let AdmissionQos {
            min_sep_nanos,
            tbf_clock,
            by_source_ts,
            reliable,
            history_kind,
            history_depth,
            max_samples,
            max_samples_per_instance,
            max_instances,
            ..
        } = *aq;
        // DDS 1.4 §2.2.1.2.2: a topic without a key has one instance, so
        // every sample belongs to the instance `instance_key_hash` names.
        let kh = self.instance_key_hash(&sample);
        let info = {
            // QoS filter BEFORE observe_sample so discarded samples do
            // not affect the sample state.
            // Spec §2.2.3.12 TIME_BASED_FILTER: drop if less than
            // minimum_separation has elapsed since the last delivered
            // sample of this instance, on the clock
            // `reader.time_based_filter_clock` names. It measured with
            // `now`, the time of this ingest, which runs at the
            // application's read or take.
            let filter_ts = tbf_clock.time_of(
                src_ts.map(crate::time::he_timestamp_to_time),
                reception_ts.unwrap_or(now),
            );
            if time_filter
                && !self.inner.instances.should_deliver_under_time_based_filter(
                    &kh,
                    filter_ts,
                    min_sep_nanos,
                )
            {
                // §2.2.3.12: under RELIABLE the last of them is delivered
                // once the writer went quiet; the filter dropped it.
                if reliable {
                    if let Ok(mut hold) = self.inner.tbf_hold.lock() {
                        hold.hold(
                            kh,
                            (
                                bytes,
                                writer_guid,
                                writer_strength,
                                src_ts,
                                big_endian,
                                representation,
                                lifespan_nanos,
                                reception_ts,
                            ),
                            reception_ts.unwrap_or(now),
                            min_sep_nanos,
                        );
                    }
                    self.arm_tbf_hook();
                }
                return Ok(None);
            }
            if min_sep_nanos > 0 && reliable {
                if let Ok(mut hold) = self.inner.tbf_hold.lock() {
                    hold.supersede(&kh);
                }
            }
            // Spec §2.2.3.18 DESTINATION_ORDER: under BY_SOURCE_TIMESTAMP
            // the ordering key is the writer's source timestamp (from
            // INFO_TS), not the reception time — only deliver samples with a
            // strictly greater source_ts. Under BY_RECEPTION_TIMESTAMP the
            // reception clock (`now`) is the key.
            let order_ts = if by_source_ts { sample_source_ts } else { now };
            if !self.inner.instances.should_deliver_under_destination_order(
                &kh,
                order_ts,
                by_source_ts,
            ) {
                return Ok(None);
            }
            // Spec §2.2.3.19 RESOURCE_LIMITS.max_instances: a genuinely
            // new instance beyond the configured ceiling is refused
            // before it is ever created — unlike the per-instance/total
            // sample ceilings below, there is no existing instance here
            // to evict in its place. `in_cache` counts this sample.
            let Some((handle, mut in_cache)) = self.inner.instances.observe_admitted(
                kh,
                || {
                    self.inner.last_key.holder_of(kh).unwrap_or_else(|| {
                        let codec = self.inner.topic.codec();
                        codec
                            .key_holder(&sample)
                            .map(|(holder, _)| holder)
                            .or_else(|| codec.key_material(&sample).map(|(_, holder)| holder))
                            .unwrap_or_default()
                    })
                },
                now,
                max_instances,
            ) else {
                // DDS 1.4 §2.2.4.1: SAMPLE_REJECTED, "A (received) sample has
                // been rejected", whatever the reliability. A BEST_EFFORT
                // writer does not send it again: it is gone. It was
                // counted as SAMPLE_LOST, which is a sample "never
                // received".
                if count_rejection {
                    self.note_sample_rejected(
                        crate::status::SampleRejectedStatusKind::RejectedByInstancesLimit,
                        HANDLE_NIL,
                    );
                }
                if !reliable {
                    return Ok(None);
                }
                return Ok(Some((
                    bytes,
                    writer_guid,
                    writer_strength,
                    src_ts,
                    big_endian,
                    representation,
                    lifespan_nanos,
                    reception_ts,
                )));
            };
            // Spec §2.2.3.18 HISTORY = KEEP_LAST(depth): silently evict
            // the oldest not-yet-taken sample of this instance once it
            // already holds `depth` — ordinary QoS-directed retention,
            // not a failure, so neither SAMPLE_REJECTED nor SAMPLE_LOST
            // changes.
            if history_kind == zerodds_qos::HistoryKind::KeepLast {
                let depth = history_depth.max(0) as u32;
                while in_cache > depth {
                    if !self.evict_oldest_of_instance(cache, handle) {
                        break;
                    }
                    in_cache -= 1;
                }
            }
            // Spec §2.2.3.19 RESOURCE_LIMITS.max_samples_per_instance:
            // still over the ceiling even after HISTORY's own eviction
            // above — reachable under KEEP_ALL (no eviction ran at all)
            // or an inconsistent HISTORY.depth > max_samples_per_instance.
            if max_samples_per_instance >= 0 && in_cache > max_samples_per_instance as u32 {
                if reliable {
                    // §2.2.4.1 SAMPLE_REJECTED_STATUS: "a rejected sample
                    // may be accepted in the future" — refuse the
                    // incoming sample outright; what is already cached
                    // stays untouched. Roll back `observe_sample`'s
                    // optimistic increment since this sample never
                    // enters `cache`.
                    self.inner.instances.drain_samples(handle, 1);
                    if count_rejection {
                        self.note_sample_rejected(
                            crate::status::SampleRejectedStatusKind::RejectedBySamplesPerInstanceLimit,
                            handle,
                        );
                    }
                    return Ok(Some((
                        bytes,
                        writer_guid,
                        writer_strength,
                        src_ts,
                        big_endian,
                        representation,
                        lifespan_nanos,
                        reception_ts,
                    )));
                }
                // BEST_EFFORT: admit the new sample, evict this
                // instance's own oldest to make room. §2.2.3.18 KEEP_ALL:
                // "If the reliability kind is BEST_EFFORT, then the old
                // values will be discarded" — QoS-directed retention, as
                // KEEP_LAST's, so no status changes: the evicted sample was
                // received (SAMPLE_LOST is a sample "never received") and
                // was not refused (SAMPLE_REJECTED, §2.2.4.1).
                self.evict_oldest_of_instance(cache, handle);
            }
            // Spec §2.2.3.19 RESOURCE_LIMITS.max_samples (total, across
            // every instance).
            // Samples that contain no data (the NOT_ALIVE markers) "do not count towards
            // the limits imposed by the RESOURCE_LIMITS QoS policy" (§2.2.2.5.3.8).
            if max_samples >= 0
                && cache.iter().filter(|c| c.info.valid_data).count() >= max_samples as usize
            {
                if reliable {
                    self.inner.instances.drain_samples(handle, 1);
                    if count_rejection {
                        self.note_sample_rejected(
                            crate::status::SampleRejectedStatusKind::RejectedBySamplesLimit,
                            handle,
                        );
                    }
                    return Ok(Some((
                        bytes,
                        writer_guid,
                        writer_strength,
                        src_ts,
                        big_endian,
                        representation,
                        lifespan_nanos,
                        reception_ts,
                    )));
                }
                // BEST_EFFORT: as for max_samples_per_instance above.
                self.evict_oldest_overall(cache);
            }
            // Every check above passed (or made room): this sample is
            // really being delivered now, so — unlike the checks
            // above, which only *decide* — this is the point where
            // TIME_BASED_FILTER/DESTINATION_ORDER's own "last
            // delivered" bookkeeping must update. Doing this earlier
            // (right after `observe_sample`, as before) would have
            // recorded a sample that RESOURCE_LIMITS then went on to
            // reject as "delivered" — corrupting the baseline the
            // *next* sample's admission is compared against.
            //
            // §2.2.2.5.1: the instance is written by this writer. When the
            // last writer of an instance goes away the instance becomes
            // NOT_ALIVE_NO_WRITERS, so the set is kept per instance.
            let Some((view_new, instance_kind, disposed_count, no_writers_count)) = self
                .inner
                .instances
                .note_admitted(&kh, order_ts, filter_ts, writer_guid)
            else {
                return Ok(None); // should never happen — defensive
            };
            SampleInfo {
                sample_state: SampleStateKind::NotRead,
                view_state: if view_new {
                    ViewStateKind::New
                } else {
                    ViewStateKind::NotNew
                },
                instance_state: instance_kind,
                disposed_generation_count: disposed_count,
                no_writers_generation_count: no_writers_count,
                source_timestamp: sample_source_ts,
                instance_handle: handle,
                publication_handle: writer_publication_handle(writer_guid),
                valid_data: true,
                ..SampleInfo::default()
            }
        };
        if !self.inner.topic.is_keyed() {
            // The invalid-data sample of the one instance of a keyless
            // type has no key to rebuild a value from; it takes the last
            // value received (see `decode_or_keyholder`).
            if let Ok(mut last) = self.inner.keyless_last.lock() {
                *last = Some((bytes.clone(), representation, big_endian));
            }
        }
        cache.push_back(CachedSample {
            bytes: Some(bytes),
            info,
            representation,
            big_endian,
            expires_at,
            decoded: Some(Box::new(sample)),
        });
        Ok(None)
    }

    /// DDS 1.4 §2.2.2.5.1: `NOT_ALIVE_NO_WRITERS_INSTANCE_STATE` "indicates
    /// the instance has been declared as not-alive by the DataReader because
    /// it detected that there are no live DataWriter objects writing that
    /// instance". The runtime records every unmatched remote writer (its SEDP
    /// dispose arrived or its participant left; a writer whose liveliness
    /// lease expired stays matched and is counted not alive by the
    /// `LivelinessTracker`, DDS 1.4 §2.2.4.1); this drains that record, drops the writer from the
    /// per-instance writer sets, and puts an invalid-data sample
    /// (§2.2.2.5.1.13) into the cache for every instance that lost its last
    /// writer — that sample is how the application learns of the transition.
    ///
    /// A publisher that simply exits without unregistering anything is
    /// exactly this case: no lifecycle marker is ever sent, and the state
    /// change comes from the discovery event alone.
    #[cfg(feature = "std")]
    fn apply_unmatched_writers(&self) -> Result<()> {
        let (Some(rt), Some(eid)) = (self.inner.runtime.as_ref(), self.inner.entity_id) else {
            return Ok(());
        };
        // A matched writer that lost its liveliness is no live writer of its
        // instances any more (DDS 1.4 §2.2.2.5.1.3: NOT_ALIVE_NO_WRITERS
        // once "there are no live DataWriter entities writing that
        // instance"), and it gives up the ownership of the EXCLUSIVE ones
        // it held (§2.2.3.9.2 (c)). It writes again as a writer that
        // arrives: its sample makes the instance ALIVE.
        // A writer that announced another strength holds the instances it
        // owns at that strength (§2.2.3.9.2 (b)).
        for (writer, strength) in rt.take_reader_strength_changes(eid) {
            self.inner.instances.update_owner_strength(writer, strength);
        }
        let mut gone = rt.take_reader_lost_liveliness_writers(eid);
        // An unmatched writer — deleted, or its participant gone — gives up
        // its ownership as well, and its strength with it.
        gone.extend(rt.take_reader_unmatched_writers(eid));
        if gone.is_empty() {
            return Ok(());
        }
        let now = self.wall_now();
        for writer in &gone {
            self.inner.instances.clear_owner_for_writer(*writer);
            for handle in self.inner.instances.drop_writer(writer, Some(now)) {
                let Some(state) = self.inner.instances.get_by_handle(handle) else {
                    continue;
                };
                let info = SampleInfo {
                    source_timestamp: now,
                    valid_data: false,
                    instance_handle: handle,
                    // The writer whose departure changed the instance
                    // (§2.2.2.5.5 `publication_handle`).
                    publication_handle: writer_publication_handle(*writer),
                    instance_state: state.kind,
                    disposed_generation_count: state.disposed_generation_count,
                    no_writers_generation_count: state.no_writers_generation_count,
                    view_state: if state.reader_view_new {
                        ViewStateKind::New
                    } else {
                        ViewStateKind::NotNew
                    },
                    ..SampleInfo::default()
                };
                let mut cache =
                    self.inner
                        .cache
                        .lock()
                        .map_err(|_| DdsError::PreconditionNotMet {
                            reason: "datareader cache poisoned",
                        })?;
                cache.push_back(CachedSample {
                    bytes: None,
                    info,
                    representation: 0,
                    big_endian: false,
                    // A no-writers marker is not a data sample; §2.2.3.16
                    // LIFESPAN does not apply to it.
                    expires_at: None,
                    decoded: None,
                });
                self.inner
                    .departure_markers
                    .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
            }
        }
        Ok(())
    }

    /// Applies `ReaderDataLifecycle.autopurge_*`: removes expired
    /// instances from the tracker + cache. Called by `ingest_into_cache`
    /// and when reading with no new bytes.
    #[cfg(feature = "std")]
    fn run_reader_autopurge(
        &self,
        now: Time,
        cache: &mut alloc::collections::VecDeque<CachedSample>,
    ) {
        let (purge_disp, purge_now) = {
            let qos = self.inner.qos.lock().unwrap_or_else(|e| e.into_inner());
            (
                qos.reader_data_lifecycle
                    .autopurge_disposed_samples_delay
                    .to_nanos(),
                qos.reader_data_lifecycle
                    .autopurge_nowriter_samples_delay
                    .to_nanos(),
            )
        };
        if purge_disp == u128::MAX && purge_now == u128::MAX {
            return;
        }
        let purged = self.inner.instances.autopurge(now, purge_disp, purge_now);
        if purged > 0 {
            cache.retain(|s| {
                s.info.instance_handle.is_nil()
                    || self
                        .inner
                        .instances
                        .get_by_handle(s.info.instance_handle)
                        .is_some()
            });
        }
    }

    /// Pushes a pure lifecycle marker (dispose / unregister) into the
    /// cache. Called by the runtime as soon as a writer sends
    /// `dispose`/`unregister_instance`.
    #[cfg(feature = "std")]
    #[doc(hidden)]
    pub fn __push_lifecycle(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        key_holder: Vec<u8>,
        kind: InstanceStateKind,
    ) -> Result<()> {
        self.push_lifecycle_with_holder(keyhash, Some(key_holder), kind)
    }

    /// The instance a sample belongs to: its KeyHash, or for a topic
    /// without a key the one instance of the topic (DDS 1.4 §2.2.1.2.2).
    #[cfg(feature = "std")]
    fn instance_key_hash(&self, sample: &T) -> crate::instance_tracker::KeyHash {
        if !self.inner.topic.is_keyed() {
            return crate::topic::KEYLESS_INSTANCE_KEY;
        }
        self.inner
            .last_key
            .key_hash(self.inner.topic.codec(), sample)
            .unwrap_or(crate::topic::KEYLESS_INSTANCE_KEY)
    }

    /// The instance a dispose or unregister names, found the way
    /// [`Self::ingest_alive`] finds that of a data sample: from the key.
    ///
    /// DDS 1.4 §2.2.2.5.1 identifies an instance by its key; DDSI-RTPS 2.5
    /// §9.6.4.8 derives `PID_KEY_HASH` from the key members. A
    /// data sample's instance is the hash this reader computes from the
    /// decoded sample, whatever `PID_KEY_HASH` it came with. A marker with a
    /// `serializedKey` gets the same: the holder is decoded and hashed here,
    /// so a writer whose `PID_KEY_HASH` is not the one §9.6.4.8 gives (zzdds
    /// 0.2.0 sends the key's octets where a key of more than sixteen octets
    /// takes the MD5) still disposes the instance its samples went to. A
    /// marker that came with `PID_KEY_HASH` alone keeps it.
    #[cfg(feature = "std")]
    fn lifecycle_instance(
        &self,
        wire_hash: crate::instance_tracker::KeyHash,
        holder: Option<Vec<u8>>,
    ) -> (crate::instance_tracker::KeyHash, Option<Vec<u8>>) {
        // A topic without a key has one instance whatever hash the marker
        // carries (DDS 1.4 §2.2.1.2.2).
        if !self.inner.topic.is_keyed() {
            return (crate::topic::KEYLESS_INSTANCE_KEY, Some(Vec::new()));
        }
        let Some(holder) = holder else {
            return (wire_hash, None);
        };
        let codec = self.inner.topic.codec();
        let Ok(key) = codec.decode_key_holder(&holder) else {
            return (wire_hash, Some(holder));
        };
        match self.inner.last_key.key_hash(codec, &key) {
            Some(hash) => {
                let own = self.inner.last_key.holder_of(hash).unwrap_or(holder);
                (hash, Some(own))
            }
            None => (wire_hash, Some(holder)),
        }
    }

    /// The key holder of a marker that came with `PID_KEY_HASH` alone, for
    /// an instance this reader does not know: the KeyHash itself where it
    /// is the holder. DDSI-RTPS 2.5 §9.6.4.8 Step 5.1 makes the KeyHash of
    /// a key of at most sixteen octets that key's PLAIN_CDR2-BE octets,
    /// padded with zeros. The octets are read as a holder and count only if
    /// they hash back to `keyhash`; the MD5 of Step 5.2 never does.
    #[cfg(feature = "std")]
    fn holder_from_key_hash(&self, keyhash: crate::instance_tracker::KeyHash) -> Option<Vec<u8>> {
        let codec = self.inner.topic.codec();
        let key = codec.decode_key_holder(&keyhash).ok()?;
        (self.inner.last_key.key_hash(codec, &key)? == keyhash)
            .then(|| self.inner.last_key.holder_of(keyhash))
            .flatten()
    }

    /// Reports a dispose or unregister for an instance this reader does
    /// not know and whose key it cannot read from `PID_KEY_HASH`: a loud
    /// event, the marker is not taken in.
    #[cfg(feature = "std")]
    fn report_unknown_instance_marker(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        kind: InstanceStateKind,
    ) {
        use zerodds_foundation::observability::{Component, Event, Level};
        if let Some(rt) = self.inner.runtime.as_ref() {
            rt.config.observability.record(
                &Event::new(
                    Level::Warn,
                    Component::Dcps,
                    "reader.lifecycle.unknown_instance",
                )
                .with_attr("topic", self.inner.topic.name())
                .with_attr("key_hash", alloc::format!("{keyhash:02x?}").as_str())
                .with_attr("kind", alloc::format!("{kind:?}").as_str()),
            );
        }
    }

    /// Takes a dispose or unregister marker into the cache.
    ///
    /// A marker that came with `PID_KEY_HASH` alone carries the instance's
    /// identity, not its key. For an instance the reader knows, the key
    /// holder is the one it has. For one it does not know, the key is read
    /// from the KeyHash where §9.6.4.8 Step 5.1 makes it the key
    /// ([`Self::holder_from_key_hash`]); where the KeyHash is an MD5, the
    /// marker is reported and not taken in. Taken in without a key holder,
    /// it made every `take` and `read` that selected it fail with
    /// `Unsupported`, whatever other samples the cache held. Cyclone DDS
    /// drops such a marker the same way ("keyhash is MD5 and can't be
    /// converted to key value").
    #[cfg(feature = "std")]
    fn push_lifecycle_with_holder(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        key_holder: Option<Vec<u8>>,
        kind: InstanceStateKind,
    ) -> Result<()> {
        self.push_lifecycle_from(keyhash, key_holder, kind, None, None)
    }

    /// [`Self::push_lifecycle_with_holder`] for a marker of `writer`: an
    /// unregister of a known writer takes that writer out of the instance's
    /// writers ([`crate::instance_tracker::InstanceTracker::unregister_writer`]).
    /// The marker's SampleInfo names the writer as its `publication_handle`
    /// and carries the writer's `source_timestamp` (DDS 1.4 §2.2.2.5.5,
    /// §2.2.2.4.2.11 `dispose_w_timestamp`, §2.2.2.4.2.8); it had the
    /// reader's clock and HANDLE_NIL.
    #[cfg(feature = "std")]
    fn push_lifecycle_from(
        &self,
        keyhash: crate::instance_tracker::KeyHash,
        key_holder: Option<Vec<u8>>,
        kind: InstanceStateKind,
        writer: Option<[u8; 16]>,
        source_timestamp: Option<Time>,
    ) -> Result<()> {
        // A topic without a key has one instance, whatever hash names it.
        let keyhash = if self.inner.topic.is_keyed() {
            keyhash
        } else {
            crate::topic::KEYLESS_INSTANCE_KEY
        };
        // The invalid-data sample of a keyless topic's instance takes the
        // last value received; without one there is nothing to hand out, and
        // the marker is reported rather than taken in, as for an unknown
        // instance below.
        if !self.inner.topic.is_keyed()
            && self
                .inner
                .keyless_last
                .lock()
                .map_or(true, |last| last.is_none())
        {
            self.report_unknown_instance_marker(keyhash, kind);
            return Ok(());
        }
        let key_holder = match key_holder {
            Some(holder) => Some(holder),
            None if self.inner.instances.lookup(&keyhash).is_some() => None,
            None => match self.holder_from_key_hash(keyhash) {
                Some(holder) => Some(holder),
                None => {
                    self.report_unknown_instance_marker(keyhash, kind);
                    return Ok(());
                }
            },
        };
        let now = self.wall_now();
        // The instance the marker names, known from now on; a marker does
        // not bring it back to ALIVE.
        let handle = self
            .inner
            .instances
            .observe_lifecycle(keyhash, key_holder, Some(now));
        match kind {
            InstanceStateKind::NotAliveDisposed => {
                self.inner.instances.dispose(handle, Some(now));
            }
            InstanceStateKind::NotAliveNoWriters => match writer {
                Some(w) => {
                    self.inner.instances.unregister_writer(handle, w, Some(now));
                    // Another writer still writes the instance: its state
                    // did not change, and an invalid sample reports a change
                    // (§2.2.2.5.1.4). The marker counted in the cache by
                    // `observe_lifecycle` is not there.
                    if self
                        .inner
                        .instances
                        .get_by_handle(handle)
                        .is_some_and(|s| s.kind == InstanceStateKind::Alive)
                    {
                        self.inner.instances.drain_samples(handle, 1);
                        return Ok(());
                    }
                }
                None => {
                    self.inner.instances.unregister(handle, Some(now));
                }
            },
            InstanceStateKind::Alive => {}
        }
        let Some(state) = self.inner.instances.get_by_handle(handle) else {
            return Ok(()); // should never happen — defensive
        };
        let info = SampleInfo {
            source_timestamp: source_timestamp.unwrap_or(now),
            valid_data: false,
            instance_handle: handle,
            publication_handle: writer.map_or(HANDLE_NIL, writer_publication_handle),
            instance_state: state.kind,
            disposed_generation_count: state.disposed_generation_count,
            no_writers_generation_count: state.no_writers_generation_count,
            view_state: if state.reader_view_new {
                ViewStateKind::New
            } else {
                ViewStateKind::NotNew
            },
            ..SampleInfo::default()
        };
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        cache.push_back(CachedSample {
            bytes: None,
            info,
            // Lifecycle marker: reconstructed from the key holder, not `bytes`;
            // representation is irrelevant.
            representation: 0,
            big_endian: false,
            // §2.2.3.16 applies to data samples; a dispose/unregister marker
            // carries no LIFESPAN and never expires.
            expires_at: None,
            decoded: None,
        });
        Ok(())
    }

    /// Brings the instance state and view state in the SampleInfo of every
    /// cached sample to where its instance stands now. DDS 1.4 §2.2.2.5.1.3:
    /// "The instance_state available in the SampleInfo is a snapshot of the
    /// instance_state of the instance at the time the collection was
    /// obtained (i.e., at the time read or take was called)"; §2.2.2.5.1.8
    /// says the same of the view_state. Kept from the arrival, a sample read
    /// after its instance was disposed showed ALIVE, a second read NEW, and
    /// the state masks selected by the old state. The generation counts
    /// stay as they were when the sample arrived (§2.2.2.5.1.5). Called with
    /// the cache locked, before a read or take looks at the samples.
    #[cfg(feature = "std")]
    fn refresh_states(&self, cache: &mut alloc::collections::VecDeque<CachedSample>) {
        if cache.is_empty() {
            return;
        }
        let states = self
            .inner
            .instances
            .current_states(cache.iter().map(|s| s.info.instance_handle));
        for s in cache.iter_mut() {
            if let Some(&(kind, view_new)) = states.get(&s.info.instance_handle) {
                s.info.instance_state = kind;
                s.info.view_state = if view_new {
                    ViewStateKind::New
                } else {
                    ViewStateKind::NotNew
                };
            }
        }
    }

    /// `take` with full [`SampleInfo`]. Spec §2.2.2.5.3.5 `take`.
    /// Consumes the samples from the cache (the `NOT_READ → READ`
    /// transition is moot since they are gone).
    ///
    /// # Errors
    /// Same as [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.take_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `read` with full [`SampleInfo`]. Does not consume — only marks
    /// the samples as `READ` (spec §2.2.2.5.3.4).
    ///
    /// # Errors
    /// Same as [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_with_info(&self) -> Result<Vec<Sample<T>>> {
        self.read_filtered(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        )
    }

    /// `take` with state masks (spec §2.2.2.5.3.6 `take_w_condition`).
    ///
    /// # Errors
    /// Same as [`Self::take`].
    #[cfg(feature = "std")]
    pub fn take_filtered(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
    ) -> Result<Vec<Sample<T>>> {
        self.take_core(sample_mask, view_mask, instance_mask, usize::MAX, None)
    }

    /// The body of every take with state masks: at most `max` samples, of
    /// `instance` only if one is given.
    #[cfg(feature = "std")]
    fn take_core(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
        max: usize,
        instance: Option<InstanceHandle>,
    ) -> Result<Vec<Sample<T>>> {
        let wanted = |s: &CachedSample| {
            s.info.matches_states(sample_mask, view_mask, instance_mask)
                && instance.is_none_or(|h| s.info.instance_handle == h)
        };
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(cache.len());
        for s in cache.iter().filter(|s| wanted(s)) {
            self.require_known_lifecycle_holder(s)?;
        }
        // §2.2.2.5.3.8 rule 5: at most one sample under GROUP with
        // `ordered_access`; what it does not take stays.
        let limit = self.presentation_limit().min(max);
        for s in cache.drain(..) {
            if out.len() < limit && wanted(&s) {
                let sample = self.materialize(s)?;
                self.inner
                    .instances
                    .mark_view_seen(sample.info.instance_handle);
                if sample.info.instance_handle != HANDLE_NIL {
                    self.inner
                        .instances
                        .drain_samples(sample.info.instance_handle, 1);
                }
                out.push(sample);
            } else {
                keep.push(s);
            }
        }
        *cache = keep.into();
        // §2.2.2.5.3.8: the samples of an instance consecutive.
        self.present_by_access_scope(&mut out);
        Ok(out)
    }

    /// `read` with state masks (spec §2.2.2.5.3.3 `read_w_condition`).
    ///
    /// # Errors
    /// Same as [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_filtered(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
    ) -> Result<Vec<Sample<T>>> {
        self.read_core(sample_mask, view_mask, instance_mask, usize::MAX, None)
    }

    /// The body of every read with state masks: at most `max` samples, of
    /// `instance` only if one is given.
    #[cfg(feature = "std")]
    fn read_core(
        &self,
        sample_mask: u32,
        view_mask: u32,
        instance_mask: u32,
        max: usize,
        instance: Option<InstanceHandle>,
    ) -> Result<Vec<Sample<T>>> {
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        let mut out = Vec::with_capacity(cache.len());
        // §2.2.2.5.3.8 rule 5, as in `take_filtered`.
        let limit = self.presentation_limit().min(max);
        for s in cache.iter_mut() {
            if out.len() >= limit {
                break;
            }
            if !s.info.matches_states(sample_mask, view_mask, instance_mask)
                || instance.is_some_and(|h| s.info.instance_handle != h)
            {
                continue;
            }
            // Build a snapshot (with the current sample-state view).
            let snapshot = Sample::new(
                self.decode_or_keyholder(
                    s.bytes.as_deref(),
                    s.representation,
                    s.big_endian,
                    s.info.instance_handle,
                )?,
                s.info,
            );
            // Sample-state transition NOT_READ → READ (spec §2.2.2.5.3.4).
            s.info.sample_state = SampleStateKind::Read;
            self.inner.instances.mark_view_seen(s.info.instance_handle);
            out.push(snapshot);
        }
        // §2.2.2.5.3.8: the samples of an instance consecutive.
        self.present_by_access_scope(&mut out);
        Ok(out)
    }

    /// `read_w_condition` (spec §2.2.2.5.3.7) — in addition to the state
    /// mask, applies the QueryCondition's SQL filter per sample. Samples
    /// stay in the cache (sample state NOT_READ → READ).
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning or SQL eval error.
    #[cfg(feature = "std")]
    pub fn read_w_condition<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
    ) -> Result<Vec<Sample<T>>> {
        self.query_core(condition, usize::MAX, false, None)
    }

    /// `read_w_condition` with its `max_samples` (§2.2.2.5.3.8: "limited to
    /// the specified max_samples"; `LENGTH_UNLIMITED`, -1, for all) and
    /// NO_DATA for an empty result. The limit applies after the `ORDER BY`
    /// of the condition, and only the samples returned become READ.
    ///
    /// # Errors
    /// `NoData` if no sample meets the condition; `BadParameter` for a
    /// `max_samples` below `LENGTH_UNLIMITED`; the errors of
    /// [`Self::read_w_condition`].
    #[cfg(feature = "std")]
    pub fn read_w_condition_selected<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
        max_samples: i32,
    ) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(&ReadSelection::limited(max_samples))?;
        Self::or_no_data(self.query_core(condition, max, false, None))
    }

    /// [`Self::read_w_condition_selected`] for `take_w_condition`
    /// (§2.2.2.5.3.9).
    ///
    /// # Errors
    /// As [`Self::read_w_condition_selected`].
    #[cfg(feature = "std")]
    pub fn take_w_condition_selected<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
        max_samples: i32,
    ) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(&ReadSelection::limited(max_samples))?;
        Self::or_no_data(self.query_core(condition, max, true, None))
    }

    /// DDS 1.4 §2.2.2.5.3.10: the condition of `read_w_condition` and
    /// `take_w_condition` "must be a ReadCondition created by the
    /// DataReader"; any other is PRECONDITION_NOT_MET.
    ///
    /// # Errors
    /// `PreconditionNotMet` for a condition of another reader or of none.
    #[cfg(feature = "std")]
    fn check_condition_is_mine(&self, base: &Arc<crate::condition::ReadCondition>) -> Result<()> {
        let mine = base
            .get_datareader()
            .is_some_and(|r| r.instance_handle() == self.instance_handle());
        if mine {
            Ok(())
        } else {
            Err(DdsError::PreconditionNotMet {
                reason: "the condition was not created by this DataReader",
            })
        }
    }

    /// The body of every read and take with a QueryCondition: the samples in
    /// the condition's state masks that its expression accepts, in the order
    /// of its `ORDER BY` or else of §2.2.2.5.3.8, the first `max` of them.
    /// Only those change: they become READ, or they are taken.
    #[cfg(feature = "std")]
    fn query_core<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
        max: usize,
        take: bool,
        instance: Option<InstanceHandle>,
    ) -> Result<Vec<Sample<T>>> {
        let base = condition.read_condition();
        self.check_condition_is_mine(base)?;
        let query = condition.query_condition();
        let sample_mask = base.get_sample_state_mask();
        let view_mask = base.get_view_state_mask();
        let instance_mask = base.get_instance_state_mask();

        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        if take {
            for s in cache.iter().filter(|s| {
                s.info.matches_states(sample_mask, view_mask, instance_mask)
                    && instance.is_none_or(|h| s.info.instance_handle == h)
            }) {
                self.require_known_lifecycle_holder(s)?;
            }
        }
        let mut chosen: Vec<(usize, Sample<T>)> = Vec::new();
        for (index, s) in cache.iter().enumerate() {
            if !s.info.matches_states(sample_mask, view_mask, instance_mask) {
                continue;
            }
            if instance.is_some_and(|h| s.info.instance_handle != h) {
                continue;
            }
            let decoded = self.decode_or_keyholder(
                s.bytes.as_deref(),
                s.representation,
                s.big_endian,
                s.info.instance_handle,
            )?;
            // A filter that cannot be evaluated rejects the sample.
            if let Some(query) = query {
                if !query
                    .evaluate(&crate::dds_type::DdsTypeRow::new(&decoded))
                    .unwrap_or(false)
                {
                    continue;
                }
            }
            chosen.push((index, Sample::new(decoded, s.info)));
        }
        let order_by = query.map_or(&[][..], |q| q.order_by());
        if order_by.is_empty() {
            if self.presentation() == Presentation::Grouped {
                group_by_instance(&mut chosen, |(_, sample)| sample.info.instance_handle);
            }
        } else {
            chosen.sort_by(|(_, a), (_, b)| {
                query.map_or(core::cmp::Ordering::Equal, |q| {
                    q.compare_rows(
                        &crate::dds_type::DdsTypeRow::new(&a.data),
                        &crate::dds_type::DdsTypeRow::new(&b.data),
                    )
                })
            });
        }
        // §2.2.2.5.3.8 rule 5: at most one sample under GROUP with
        // `ordered_access`; the rest stays.
        chosen.truncate(self.presentation_limit().min(max));
        for (index, sample) in &chosen {
            self.inner
                .instances
                .mark_view_seen(sample.info.instance_handle);
            if take {
                if sample.info.instance_handle != HANDLE_NIL {
                    self.inner
                        .instances
                        .drain_samples(sample.info.instance_handle, 1);
                }
            } else if let Some(entry) = cache.get_mut(*index) {
                entry.info.sample_state = SampleStateKind::Read;
            }
        }
        if take {
            let taken: alloc::collections::BTreeSet<usize> =
                chosen.iter().map(|(index, _)| *index).collect();
            let all: Vec<_> = cache.drain(..).collect();
            *cache = all
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !taken.contains(index))
                .map(|(_, s)| s)
                .collect::<Vec<_>>()
                .into();
        }
        let mut out: Vec<Sample<T>> = chosen.into_iter().map(|(_, sample)| sample).collect();
        self.rank_samples(&mut out);
        Ok(out)
    }

    /// DDS 1.4 §2.2.2.5.3 `create_readcondition`: a ReadCondition on
    /// this reader, triggered while the reader holds a sample whose sample,
    /// view and instance states are in the three masks (§2.2.2.5.8). A
    /// sample the reader has received and not yet taken into its cache — it
    /// does so on the next read or take — is NOT_READ and counts for any
    /// view and instance state.
    ///
    /// # Errors
    /// `PreconditionNotMet` on a poisoned lock.
    #[cfg(feature = "std")]
    pub fn create_readcondition(
        &self,
        sample_states: u32,
        view_states: u32,
        instance_states: u32,
    ) -> Result<Arc<crate::condition::ReadCondition>> {
        self.inner.entity_state.check_not_deleted()?;
        let alive = Arc::new(core::sync::atomic::AtomicBool::new(true));
        let holds = self.holds_matching(Arc::clone(&alive), None);
        let read = crate::condition::ReadCondition::new(
            sample_states,
            view_states,
            instance_states,
            holds,
        );
        read.set_reader(Arc::downgrade(
            &(Arc::clone(&self.inner) as Arc<dyn crate::contained::ContainedReader>),
        ));
        self.keep_condition(ReaderCondition {
            read: Arc::clone(&read),
            _query: None,
            alive,
        })?;
        Ok(read)
    }

    /// DDS 1.4 §2.2.2.5.3 `create_querycondition`: a QueryCondition on
    /// this reader, triggered while it holds a sample in the three state
    /// masks for which `query_expression` with `query_parameters` holds
    /// (§2.2.2.5.9), as [`Self::read_w_condition`] evaluates it. A sample not
    /// yet taken into the cache counts as for
    /// [`Self::create_readcondition`]: its fields are known once a read or
    /// take decoded it.
    ///
    /// # Errors
    /// `BadParameter` for an expression that does not parse;
    /// `PreconditionNotMet` on a poisoned lock.
    #[cfg(feature = "std")]
    pub fn create_querycondition(
        &self,
        sample_states: u32,
        view_states: u32,
        instance_states: u32,
        query_expression: &str,
        query_parameters: Vec<alloc::string::String>,
    ) -> Result<Arc<crate::condition::QueryCondition>> {
        self.inner.entity_state.check_not_deleted()?;
        let alive = Arc::new(core::sync::atomic::AtomicBool::new(true));
        // The trigger evaluates the condition's own expression and
        // parameters, which exist once the condition does: it holds the
        // condition weakly, set below.
        let query: Arc<std::sync::OnceLock<alloc::sync::Weak<crate::condition::QueryCondition>>> =
            Arc::new(std::sync::OnceLock::new());
        let holds = self.holds_matching(Arc::clone(&alive), Some(Arc::clone(&query)));
        let base = crate::condition::ReadCondition::new(
            sample_states,
            view_states,
            instance_states,
            holds,
        );
        base.set_reader(Arc::downgrade(
            &(Arc::clone(&self.inner) as Arc<dyn crate::contained::ContainedReader>),
        ));
        let condition =
            crate::condition::QueryCondition::new(base, query_expression, query_parameters)?;
        let _ = query.set(Arc::downgrade(&condition));
        self.keep_condition(ReaderCondition {
            read: Arc::clone(condition.base()),
            _query: Some(Arc::clone(&condition)),
            alive,
        })?;
        Ok(condition)
    }

    /// DDS 1.4 §2.2.2.5.3 `delete_readcondition`: deletes a ReadCondition
    /// this reader created; for a QueryCondition, pass its
    /// [`crate::condition::QueryCondition::base`]. A deleted condition no
    /// longer triggers.
    ///
    /// # Errors
    /// `PreconditionNotMet` when `condition` was not created by this
    /// reader, or was deleted already; on a poisoned lock.
    #[cfg(feature = "std")]
    pub fn delete_readcondition(
        &self,
        condition: &Arc<crate::condition::ReadCondition>,
    ) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        let mut conditions = self.conditions()?;
        let Some(pos) = conditions
            .iter()
            .position(|c| Arc::ptr_eq(&c.read, condition))
        else {
            return Err(DdsError::PreconditionNotMet {
                reason: "the ReadCondition was not created by this DataReader",
            });
        };
        let gone = conditions.remove(pos);
        gone.alive
            .store(false, core::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// DDS 1.4 §2.2.2.5.3 `delete_contained_entities`: deletes every
    /// ReadCondition and QueryCondition this reader created.
    ///
    /// # Errors
    /// `PreconditionNotMet` on a poisoned lock.
    #[cfg(feature = "std")]
    pub fn delete_contained_entities(&self) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        crate::contained::ContainedReader::delete_conditions(&*self.inner)
    }

    /// The conditions this reader created and has not deleted.
    #[cfg(feature = "std")]
    fn conditions(&self) -> Result<std::sync::MutexGuard<'_, Vec<ReaderCondition>>> {
        self.inner
            .read_conditions
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader conditions poisoned",
            })
    }

    /// Keeps a new condition, unless the reader was deleted meanwhile:
    /// the deletion marks the reader and then drains the list under its
    /// lock, so a condition is either refused here or drained there.
    #[cfg(feature = "std")]
    fn keep_condition(&self, condition: ReaderCondition) -> Result<()> {
        let mut conditions = self.conditions()?;
        self.inner.entity_state.check_not_deleted()?;
        conditions.push(condition);
        Ok(())
    }

    /// The trigger of a condition this reader creates: whether the reader
    /// holds a sample in the given states — and, for a QueryCondition
    /// (`query`), one its expression holds for. Reads the cache as
    /// [`Self::read_w_condition`] does, without changing it.
    #[cfg(feature = "std")]
    #[allow(clippy::type_complexity)]
    fn holds_matching(
        &self,
        alive: Arc<core::sync::atomic::AtomicBool>,
        query: Option<
            Arc<std::sync::OnceLock<alloc::sync::Weak<crate::condition::QueryCondition>>>,
        >,
    ) -> impl Fn(u32, u32, u32) -> bool + Send + Sync + 'static {
        let runtime = self.inner.runtime.clone();
        let eid = self.inner.entity_id;
        let pulled = Arc::clone(&self.inner.pulled);
        let inbox = Arc::clone(&self.inner.inbox);
        let cache = Arc::clone(&self.inner.cache);
        let codec = self.inner.topic.codec_arc();
        let instances = self.inner.instances.shared();
        let state = Arc::downgrade(&self.inner.entity_state);
        move |sm, vm, im| {
            if !alive.load(core::sync::atomic::Ordering::Acquire) {
                return false;
            }
            // DDS 1.4 §2.2.2.1.1.7: a Condition of an entity that is not
            // enabled is inactive, its trigger_value FALSE.
            if !state.upgrade().is_some_and(|state| state.is_enabled()) {
                return false;
            }
            let query = match &query {
                None => None,
                Some(slot) => match slot.get().and_then(alloc::sync::Weak::upgrade) {
                    Some(q) => Some(q),
                    None => return false,
                },
            };
            if sm & crate::sample_info::sample_state_mask::NOT_READ != 0 {
                let queued = match (&runtime, eid) {
                    (Some(rt), Some(eid)) => {
                        rt.user_reader_channel_sent(eid)
                            > pulled.load(core::sync::atomic::Ordering::Acquire)
                    }
                    _ => false,
                };
                if queued || inbox.lock().map(|i| !i.is_empty()).unwrap_or(false) {
                    return true;
                }
                // A writer that left leaves NOT_READ NOT_ALIVE_NO_WRITERS
                // markers for the next read or take (DDS 1.4
                // §2.2.2.5.1.3); a query evaluates on the instance's key,
                // as on a marker in the cache.
                if im & crate::sample_info::instance_state_mask::NOT_ALIVE_NO_WRITERS != 0 {
                    if let (Some(rt), Some(eid)) = (&runtime, eid) {
                        let writerless = rt
                            .reader_departures(eid)
                            .map(|(_, gone)| instances.left_writerless(&gone, vm))
                            .unwrap_or_default();
                        let holds = writerless.iter().any(|handle| {
                            let Some(query) = query.as_ref() else {
                                return true;
                            };
                            instances
                                .get_key_holder(*handle)
                                .and_then(|holder| codec.decode_key_holder(&holder).ok())
                                .is_some_and(|sample: T| {
                                    query
                                        .evaluate(&crate::dds_type::DdsTypeRow::new(&sample))
                                        .unwrap_or(false)
                                })
                        });
                        if holds {
                            return true;
                        }
                    }
                }
            }
            let Ok(cache) = cache.lock() else {
                return false;
            };
            cache.iter().any(|s| {
                if !s.info.matches_states(sm, vm, im) {
                    return false;
                }
                let Some(query) = query.as_ref() else {
                    return true;
                };
                // The `T` admission decoded, or the one read_w_condition
                // decodes: from the bytes, or a marker's key holder.
                let decoded = match s.decoded.as_ref().and_then(|d| d.downcast_ref::<T>()) {
                    Some(sample) => {
                        return query
                            .evaluate(&crate::dds_type::DdsTypeRow::new(sample))
                            .unwrap_or(false);
                    }
                    None => match s.bytes.as_deref() {
                        Some(bytes) => decode_for_encap::<T>(
                            codec.as_ref(),
                            bytes,
                            s.representation,
                            s.big_endian,
                        )
                        .ok(),
                        None => instances
                            .get_key_holder(s.info.instance_handle)
                            .and_then(|holder| codec.decode_key_holder(&holder).ok()),
                    },
                };
                decoded.is_some_and(|sample| {
                    query
                        .evaluate(&crate::dds_type::DdsTypeRow::new(&sample))
                        .unwrap_or(false)
                })
            })
        }
    }

    /// `take_w_condition` (spec §2.2.2.5.3.8) — like `read_w_condition`,
    /// but consumes the samples (removes them from the cache).
    ///
    /// # Errors
    /// `PreconditionNotMet` on lock poisoning or SQL eval error.
    #[cfg(feature = "std")]
    pub fn take_w_condition<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
    ) -> Result<Vec<Sample<T>>> {
        self.query_core(condition, usize::MAX, true, None)
    }

    /// The checked form of a [`ReadSelection`]: the number of samples as a
    /// limit.
    ///
    /// # Errors
    /// `BadParameter` for a `max_samples` below `LENGTH_UNLIMITED`.
    fn selection_limit(selection: &ReadSelection) -> Result<usize> {
        match selection.max_samples {
            -1 => Ok(usize::MAX),
            n => usize::try_from(n).map_err(|_| DdsError::BadParameter {
                what: "max_samples below LENGTH_UNLIMITED",
            }),
        }
    }

    /// DDS 1.4 §2.2.2.5.3.8 `read` with its parameters `max_samples`,
    /// `sample_states`, `view_states` and `instance_states`: at most
    /// `max_samples` samples (`LENGTH_UNLIMITED` for all) in the states
    /// asked for. "If the DataReader has no samples that meet the
    /// constraints, the return value will be NO_DATA."
    ///
    /// # Errors
    /// `NoData` if no sample meets the constraints; `BadParameter` for a
    /// `max_samples` below `LENGTH_UNLIMITED`; the errors of
    /// [`Self::read`].
    #[cfg(feature = "std")]
    pub fn read_selected(&self, selection: &ReadSelection) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(selection)?;
        Self::or_no_data(self.read_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            None,
        ))
    }

    /// [`Self::read_selected`] for `take` (§2.2.2.5.3.9).
    ///
    /// # Errors
    /// As [`Self::read_selected`].
    #[cfg(feature = "std")]
    pub fn take_selected(&self, selection: &ReadSelection) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(selection)?;
        Self::or_no_data(self.take_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            None,
        ))
    }

    /// DDS 1.4 §2.2.2.5.3.14 `read_instance` with its parameters: the
    /// samples of the instance `handle` in the states asked for, at most
    /// `max_samples`.
    ///
    /// # Errors
    /// `BadParameter` for `HANDLE_NIL`; otherwise as
    /// [`Self::read_selected`].
    #[cfg(feature = "std")]
    pub fn read_instance_selected(
        &self,
        handle: InstanceHandle,
        selection: &ReadSelection,
    ) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "read_instance with HANDLE_NIL",
            });
        }
        let max = Self::selection_limit(selection)?;
        Self::or_no_data(self.read_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            Some(handle),
        ))
    }

    /// [`Self::read_instance_selected`] for `take_instance`
    /// (§2.2.2.5.3.15).
    ///
    /// # Errors
    /// As [`Self::read_instance_selected`].
    #[cfg(feature = "std")]
    pub fn take_instance_selected(
        &self,
        handle: InstanceHandle,
        selection: &ReadSelection,
    ) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "take_instance with HANDLE_NIL",
            });
        }
        let max = Self::selection_limit(selection)?;
        Self::or_no_data(self.take_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            Some(handle),
        ))
    }

    /// DDS 1.4 §2.2.2.5.3.16 `read_next_instance` with its parameters: the
    /// samples of the instance with the smallest handle above `previous`
    /// that has samples in the states asked for.
    ///
    /// # Errors
    /// `NoData` if no instance above `previous` has such samples; otherwise
    /// as [`Self::read_selected`].
    #[cfg(feature = "std")]
    pub fn read_next_instance_selected(
        &self,
        previous: InstanceHandle,
        selection: &ReadSelection,
    ) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(selection)?;
        let Some(next) = self.next_instance_in_states(previous, selection)? else {
            return Err(DdsError::NoData);
        };
        Self::or_no_data(self.read_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            Some(next),
        ))
    }

    /// [`Self::read_next_instance_selected`] for `take_next_instance`
    /// (§2.2.2.5.3.17).
    ///
    /// # Errors
    /// As [`Self::read_next_instance_selected`].
    #[cfg(feature = "std")]
    pub fn take_next_instance_selected(
        &self,
        previous: InstanceHandle,
        selection: &ReadSelection,
    ) -> Result<Vec<Sample<T>>> {
        let max = Self::selection_limit(selection)?;
        let Some(next) = self.next_instance_in_states(previous, selection)? else {
            return Err(DdsError::NoData);
        };
        Self::or_no_data(self.take_core(
            selection.sample_states,
            selection.view_states,
            selection.instance_states,
            max,
            Some(next),
        ))
    }

    /// DDS 1.4 §2.2.2.5.3.12 `read_next_sample`: "the next not-previously-
    /// accessed data value", one sample: `read` with `max_samples` 1,
    /// `NOT_READ_SAMPLE_STATE` and any view and instance state. The sample
    /// becomes READ.
    ///
    /// # Errors
    /// `NoData` if the reader holds no sample it has not accessed; the
    /// errors of [`Self::read_selected`].
    #[cfg(feature = "std")]
    pub fn read_next_sample(&self) -> Result<Sample<T>> {
        self.next_sample(false)
    }

    /// DDS 1.4 §2.2.2.5.3.13 `take_next_sample`: as
    /// [`Self::read_next_sample`], and the sample leaves the reader.
    ///
    /// # Errors
    /// As [`Self::read_next_sample`].
    #[cfg(feature = "std")]
    pub fn take_next_sample(&self) -> Result<Sample<T>> {
        self.next_sample(true)
    }

    #[cfg(feature = "std")]
    fn next_sample(&self, take: bool) -> Result<Sample<T>> {
        let selection = ReadSelection {
            sample_states: sample_state_mask::NOT_READ,
            ..ReadSelection::limited(1)
        };
        let mut samples = if take {
            self.take_selected(&selection)?
        } else {
            self.read_selected(&selection)?
        };
        if samples.is_empty() {
            return Err(DdsError::NoData);
        }
        Ok(samples.remove(0))
    }

    /// DDS 1.4 §2.2.2.5.3.18 `read_next_instance_w_condition`: the samples
    /// of the instance with the smallest handle above `previous` that has
    /// samples the condition accepts (its three state masks and, for a
    /// QueryCondition, its expression), as [`Self::read_w_condition`]
    /// accepts them. The text of the spec says "greater than or equal"
    /// (`>=`) in this one operation and "greater" in `read_next_instance`;
    /// the iteration of §2.2.2.5.3.16, which runs until NO_DATA, needs
    /// "greater", which this follows.
    ///
    /// # Errors
    /// `NoData` if no instance above `previous` has such samples;
    /// `PreconditionNotMet` for a condition this reader did not create;
    /// the errors of [`Self::read_w_condition`].
    #[cfg(feature = "std")]
    pub fn read_next_instance_w_condition<C: crate::condition::ReadConditionArg>(
        &self,
        previous: InstanceHandle,
        condition: &C,
    ) -> Result<Vec<Sample<T>>> {
        self.next_instance_w_condition(previous, condition, false)
    }

    /// DDS 1.4 §2.2.2.5.3.19 `take_next_instance_w_condition`: as
    /// [`Self::read_next_instance_w_condition`], and the samples leave the
    /// reader.
    ///
    /// # Errors
    /// As [`Self::read_next_instance_w_condition`].
    #[cfg(feature = "std")]
    pub fn take_next_instance_w_condition<C: crate::condition::ReadConditionArg>(
        &self,
        previous: InstanceHandle,
        condition: &C,
    ) -> Result<Vec<Sample<T>>> {
        self.next_instance_w_condition(previous, condition, true)
    }

    #[cfg(feature = "std")]
    fn next_instance_w_condition<C: crate::condition::ReadConditionArg>(
        &self,
        previous: InstanceHandle,
        condition: &C,
        take: bool,
    ) -> Result<Vec<Sample<T>>> {
        let base = condition.read_condition();
        self.check_condition_is_mine(base)?;
        // The instances with a sample the condition accepts, found by the
        // read that would return them, without taking or marking anything:
        // a copy of the cache decides, the real access follows.
        let mut candidates: Vec<InstanceHandle> = self
            .query_core_handles(condition)?
            .into_iter()
            .filter(|h| !h.is_nil() && *h > previous)
            .collect();
        candidates.sort();
        let Some(next) = candidates.first().copied() else {
            return Err(DdsError::NoData);
        };
        Self::or_no_data(self.query_core(condition, usize::MAX, take, Some(next)))
    }

    /// The instance handles of the samples the condition accepts.
    #[cfg(feature = "std")]
    fn query_core_handles<C: crate::condition::ReadConditionArg>(
        &self,
        condition: &C,
    ) -> Result<Vec<InstanceHandle>> {
        let base = condition.read_condition();
        let query = condition.query_condition();
        let (sample_mask, view_mask, instance_mask) = (
            base.get_sample_state_mask(),
            base.get_view_state_mask(),
            base.get_instance_state_mask(),
        );
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        let mut handles = Vec::new();
        for s in cache.iter() {
            if !s.info.matches_states(sample_mask, view_mask, instance_mask) {
                continue;
            }
            if let Some(query) = query {
                let decoded = self.decode_or_keyholder(
                    s.bytes.as_deref(),
                    s.representation,
                    s.big_endian,
                    s.info.instance_handle,
                )?;
                if !query
                    .evaluate(&crate::dds_type::DdsTypeRow::new(&decoded))
                    .unwrap_or(false)
                {
                    continue;
                }
            }
            handles.push(s.info.instance_handle);
        }
        Ok(handles)
    }

    /// DDS 1.4 §2.2.2.5.3.20 `return_loan`. `read` and `take` of this API
    /// return the samples owned (`Vec<Sample<T>>`) and loan nothing from the
    /// reader, so there is no loan to return: "calling return_loan on a
    /// collection that does not have a loan is safe and has no side
    /// effects" (the text of the operation). `samples` is consumed, as the
    /// collection of a returned loan is.
    ///
    /// # Errors
    /// `AlreadyDeleted` if the reader is deleted.
    pub fn return_loan(&self, samples: Vec<Sample<T>>) -> Result<()> {
        self.inner.entity_state.check_not_deleted()?;
        drop(samples);
        Ok(())
    }

    /// The smallest instance handle above `previous` with a sample in the
    /// states of `selection` (§2.2.2.5.3.16 (a) and (b)).
    #[cfg(feature = "std")]
    fn next_instance_in_states(
        &self,
        previous: InstanceHandle,
        selection: &ReadSelection,
    ) -> Result<Option<InstanceHandle>> {
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        Ok(cache
            .iter()
            .filter(|s| {
                s.info.matches_states(
                    selection.sample_states,
                    selection.view_states,
                    selection.instance_states,
                )
            })
            .map(|s| s.info.instance_handle)
            .filter(|h| !h.is_nil() && *h > previous)
            .min())
    }

    /// An empty result is NO_DATA (§2.2.2.5.3.8: "If the DataReader has no
    /// samples that meet the constraints, the return value will be
    /// NO_DATA").
    #[cfg(feature = "std")]
    fn or_no_data(result: Result<Vec<Sample<T>>>) -> Result<Vec<Sample<T>>> {
        match result {
            Ok(samples) if samples.is_empty() => Err(DdsError::NoData),
            other => other,
        }
    }

    /// `read_instance` (spec §2.2.2.5.3.27). Returns only samples of the
    /// given instance.
    ///
    /// # Errors
    /// `BadParameter` if `handle == HANDLE_NIL`.
    #[cfg(feature = "std")]
    pub fn read_instance(&self, handle: InstanceHandle) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "read_instance with HANDLE_NIL",
            });
        }
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        let mut out = Vec::new();
        // §2.2.2.5.3.14: "exactly like read" — at most one sample under
        // GROUP with `ordered_access` (§2.2.2.5.3.8 rule 5).
        let limit = self.presentation_limit();
        for s in cache.iter_mut() {
            if out.len() >= limit {
                break;
            }
            if s.info.instance_handle != handle {
                continue;
            }
            let snap = Sample::new(
                self.decode_or_keyholder(
                    s.bytes.as_deref(),
                    s.representation,
                    s.big_endian,
                    s.info.instance_handle,
                )?,
                s.info,
            );
            s.info.sample_state = SampleStateKind::Read;
            self.inner.instances.mark_view_seen(handle);
            out.push(snap);
        }
        self.rank_samples(&mut out);
        Ok(out)
    }

    /// `take_instance` (spec §2.2.2.5.3.27, take variant). Consumes.
    ///
    /// # Errors
    /// `BadParameter` if `handle == HANDLE_NIL`.
    #[cfg(feature = "std")]
    pub fn take_instance(&self, handle: InstanceHandle) -> Result<Vec<Sample<T>>> {
        if handle.is_nil() {
            return Err(DdsError::BadParameter {
                what: "take_instance with HANDLE_NIL",
            });
        }
        self.ingest_into_cache()?;
        let mut cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        self.refresh_states(&mut cache);
        let mut out = Vec::new();
        let mut keep = Vec::with_capacity(cache.len());
        for s in cache.iter().filter(|s| s.info.instance_handle == handle) {
            self.require_known_lifecycle_holder(s)?;
        }
        // As `read_instance`.
        let limit = self.presentation_limit();
        for s in cache.drain(..) {
            if out.len() < limit && s.info.instance_handle == handle {
                out.push(self.materialize(s)?);
            } else {
                keep.push(s);
            }
        }
        *cache = keep.into();
        if !out.is_empty() {
            self.inner.instances.mark_view_seen(handle);
            self.inner.instances.drain_samples(handle, out.len() as u32);
        }
        self.rank_samples(&mut out);
        Ok(out)
    }

    /// DDS 1.4 §2.2.2.5.3.16 `read_next_instance`: the samples of one
    /// instance, "as if the DataReader invoked read_instance passing the
    /// smallest instance_handle among all the ones that (a) are greater than
    /// previous_handle and (b) have available samples". `HANDLE_NIL` is
    /// less than every handle. The order is that of the handles, "as if each
    /// instance handle was represented as a unique integer", and is defined
    /// for a handle the reader does not manage.
    ///
    /// It went by the order of the key hashes, stopped at an instance
    /// without samples, returned nothing for a handle the reader did not
    /// know, and under `ordered_access` above INSTANCE scope returned the
    /// samples of every instance.
    ///
    /// # Errors
    /// Same as `read`.
    #[cfg(feature = "std")]
    pub fn read_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        // A fresh reader discovers its instances in what it takes in.
        self.ingest_into_cache()?;
        match self.next_instance_with_samples(previous)? {
            Some(next) => self.read_instance(next),
            None => Ok(Vec::new()),
        }
    }

    /// `take_next_instance` (spec §2.2.2.5.3.17). Take variant of
    /// [`Self::read_next_instance`].
    ///
    /// # Errors
    /// Same as `take`.
    #[cfg(feature = "std")]
    pub fn take_next_instance(&self, previous: InstanceHandle) -> Result<Vec<Sample<T>>> {
        self.ingest_into_cache()?;
        match self.next_instance_with_samples(previous)? {
            Some(next) => self.take_instance(next),
            None => Ok(Vec::new()),
        }
    }

    /// The smallest instance handle greater than `previous` that has a
    /// sample in the cache (§2.2.2.5.3.16 (a) and (b)).
    #[cfg(feature = "std")]
    fn next_instance_with_samples(
        &self,
        previous: InstanceHandle,
    ) -> Result<Option<InstanceHandle>> {
        let cache = self
            .inner
            .cache
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader cache poisoned",
            })?;
        Ok(cache
            .iter()
            .map(|s| s.info.instance_handle)
            .filter(|h| !h.is_nil() && *h > previous)
            .min())
    }

    /// Helper: turns a CachedSample into a `Sample<T>`. For lifecycle
    /// markers (`bytes == None`), `T` is reconstructed from the stored
    /// key holder (spec §2.2.2.5.1.13: `data` then contains only the key
    /// portion).
    #[cfg(feature = "std")]
    fn materialize(&self, mut s: CachedSample) -> Result<Sample<T>> {
        let data = match take_decoded::<T>(&mut s.decoded) {
            Some(data) => data,
            None => self.decode_or_keyholder(
                s.bytes.as_deref(),
                s.representation,
                s.big_endian,
                s.info.instance_handle,
            )?,
        };
        #[cfg(feature = "metrics")]
        self.inner
            .metrics
            .get_or_init(|| crate::metrics::ReaderMetrics::for_topic(self.inner.topic.name()))
            .samples_read(1);
        Ok(Sample::new(data, s.info))
    }

    /// Reject an unmaterializable hash-only marker before a consuming read
    /// drains the cache. The instance state and its SampleInfo remain available.
    #[cfg(feature = "std")]
    fn require_known_lifecycle_holder(&self, sample: &CachedSample) -> Result<()> {
        if sample.bytes.is_none()
            && self
                .inner
                .instances
                .get_key_holder(sample.info.instance_handle)
                .is_none()
        {
            return Err(DdsError::BadParameter {
                what: "the key of this instance is known by its hash only, which does not spell it out",
            });
        }
        Ok(())
    }

    /// Decode helper: for `Some(bytes)` via [`decode_for_encap`] (honoring the
    /// received `representation`/`big_endian`, so an XCDR1 body is read as XCDR1
    /// and not with the XCDR2 `T::decode`), for `None` (lifecycle marker) via the
    /// instance's key holder. If that is unavailable, returns `BadParameter`
    /// rather than constructing a value from an empty or invented holder.
    #[cfg(feature = "std")]
    fn decode_or_keyholder(
        &self,
        bytes: Option<&[u8]>,
        representation: u8,
        big_endian: bool,
        handle: InstanceHandle,
    ) -> Result<T> {
        if let Some(b) = bytes {
            return decode_for_encap::<T>(self.inner.topic.codec(), b, representation, big_endian)
                .map_err(|e| DdsError::WireError {
                    message: alloc::string::ToString::to_string(&e),
                });
        }
        // A topic without a key has no key members; the marker takes the last
        // value received (see `keyless_last`).
        if !self.inner.topic.is_keyed() {
            let last = self
                .inner
                .keyless_last
                .lock()
                .ok()
                .and_then(|last| last.clone());
            if let Some((b, representation, big_endian)) = last {
                return decode_for_encap::<T>(
                    self.inner.topic.codec(),
                    &b,
                    representation,
                    big_endian,
                )
                .map_err(|e| DdsError::WireError {
                    message: alloc::string::ToString::to_string(&e),
                });
            }
            return Err(DdsError::BadParameter {
                what: "the instance of a topic without a key has no value yet",
            });
        }
        // §2.2.2.5.1.13: a dispose/unregister marker carries no body, and on
        // such a sample "only the fields that form the key" are meaningful.
        // The instance's key holder is what holds them — and it is read back
        // with the reader XTypes 1.3 §7.6.8 step 4 wrote it with, not with
        // `T::decode`: the holder is PLAIN_CDR2-BE, key members only, without
        // DHEADER or member headers, so the type's own stream reader lands on
        // the wrong octets.
        if let Some(holder) = self.inner.instances.get_key_holder(handle) {
            return self
                .inner
                .topic
                .codec()
                .decode_key_holder(&holder)
                .map_err(|e| DdsError::WireError {
                    message: alloc::string::ToString::to_string(&e),
                });
        }
        Err(DdsError::BadParameter {
            what: "the key of this instance is known by its hash only, which does not spell it out",
        })
    }
}

#[cfg(feature = "std")]
impl<T: DdsType> crate::entity::Entity for DataReader<T> {
    type Qos = DataReaderQos;

    fn get_qos(&self) -> Self::Qos {
        self.inner.qos.lock().map(|q| q.clone()).unwrap_or_default()
    }

    /// Spec §2.2.3 / §2.2.2.5.3: DURABILITY, RELIABILITY, HISTORY,
    /// RESOURCE_LIMITS, OWNERSHIP are Changeable=NO post-enable.
    fn set_qos(&self, qos: impl Into<crate::qos::QosArg<Self::Qos>>) -> Result<()> {
        let qos = qos.into().resolve(|| {
            crate::participant::get_default(&self.inner.subscriber.default_datareader_qos)
        });
        qos.check_consistency()?;
        let enabled = self.inner.entity_state.is_enabled();
        if let Ok(mut current) = self.inner.qos.lock() {
            if enabled {
                if let Some(policy) = crate::qos::immutable_reader_change(&current, &qos) {
                    return Err(crate::entity::immutable_if_enabled(policy));
                }
            }
            // See `DataWriter::set_qos`.
            if let (true, Some(rt), Some(eid)) =
                (enabled, self.inner.runtime.as_ref(), self.inner.entity_id)
            {
                let update = runtime_qos_update(&qos);
                if update != runtime_qos_update(&current) {
                    rt.update_user_reader_qos(eid, &update)?;
                }
            }
            *current = qos;
        }
        Ok(())
    }

    /// DDS 1.4 §2.2.2.1.1.7: refused while the subscriber is disabled;
    /// the first call registers the reader with the runtime, which
    /// announces it (`ReaderRegistration`).
    fn enable(&self) -> Result<()> {
        self.inner.entity_state.enable_entity()
    }

    fn entity_state(&self) -> Arc<crate::entity::EntityState> {
        Arc::clone(&self.inner.entity_state)
    }
}

/// The reader half of `publisher::WriterRegistration`.
#[cfg(feature = "std")]
struct ReaderRegistration {
    runtime: Arc<DcpsRuntime>,
    eid: EntityId,
    topic_name: String,
    type_name: String,
    type_identifier: zerodds_types::TypeIdentifier,
    qos: Arc<Mutex<DataReaderQos>>,
    subscriber: Arc<SubscriberInner>,
    topic: Arc<crate::topic::TopicInner>,
    content_filter: Option<crate::topic::CftLink>,
    sample_tx: mpsc::Sender<crate::runtime::UserSample>,
    /// The reader's cache and its count of what it took off the channel,
    /// for the room [`Self::register`] gives the reader.
    cache: Arc<SampleCache>,
    pulled: Arc<std::sync::atomic::AtomicU64>,
    /// The reader's listener state, for the data signal [`Self::register`]
    /// installs once the reader's slot exists.
    status: alloc::sync::Weak<ReaderStatus>,
}

#[cfg(feature = "std")]
impl ReaderRegistration {
    /// Registers the reader with the QoS it has now and, for a reader on a
    /// ContentFilteredTopic, with the filter parameters current now.
    fn register(&self) -> Result<()> {
        let qos = self
            .qos
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "datareader qos poisoned",
            })?
            .clone();
        // Mirror of `WriterRegistration::register`: DDS 1.4
        // §2.2.3.6/§2.2.3.13/§2.2.3.3 put PRESENTATION, PARTITION and
        // GROUP_DATA on the Subscriber, and the value the RxO check
        // compares travels in this reader's `SubscriptionBuiltinTopicData`.
        // The per-reader PRESENTATION stays an override (see
        // `qos::effective_presentation`); the subscriber's PARTITION and
        // GROUP_DATA and the topic's TOPIC_DATA stay apart from the
        // reader's own in the runtime.
        let group_qos = self
            .subscriber
            .qos
            .lock()
            .map(|q| q.clone())
            .unwrap_or_default();
        let eff_presentation =
            crate::qos::effective_presentation(group_qos.presentation, qos.presentation);
        let topic_data = self
            .topic
            .qos
            .lock()
            .map(|q| q.topic_data.value.clone())
            .unwrap_or_default();
        let inherited = crate::runtime::GroupTopicQos {
            partition: group_qos.partition.names.clone(),
            group_data: group_qos.group_data.value.clone(),
            topic_data,
        };
        self.runtime.register_user_reader_as(
            self.eid,
            crate::runtime::UserReaderConfig {
                topic_name: self.topic_name.clone(),
                type_name: self.type_name.clone(),
                reliable: qos.reliability.kind == ReliabilityKind::Reliable,
                durability: qos.durability.kind,
                deadline: qos.deadline,
                time_based_filter: qos.time_based_filter,
                latency_budget: qos.latency_budget,
                destination_order: qos.destination_order,
                liveliness: qos.liveliness,
                ownership: qos.ownership.kind,
                resource_limits: qos.resource_limits,
                presentation: eff_presentation,
                partition: qos.partition.names.clone(),
                user_data: qos.user_data.value.clone(),
                topic_data: qos.topic_data.value.clone(),
                group_data: qos.group_data.value.clone(),
                // F-TYPES-3 + #30: pass through the topic type identifier
                // and the user-set TypeConsistencyEnforcement QoS
                // (force_type_validation etc.), authoritative for the
                // reader-side assignability guard.
                type_identifier: self.type_identifier.clone(),
                type_consistency: qos.type_consistency,
                // Per-reader DataRepresentation override from the QoS
                // (`None` = runtime default). XTypes 1.3 §7.6.3.1.2.
                data_representation_offer: crate::qos::effective_data_representation(
                    qos.data_representation.as_deref(),
                ),
                content_filter: self
                    .content_filter
                    .as_ref()
                    .map(crate::topic::CftLink::property),
                group_entity_id: self.subscriber.entity_id,
                // A typed DataReader subscribes; relaying is a service's.
                relay: false,
                property: qos.property.clone(),
                data_tags: qos.data_tags.clone(),
                entity_name: None,
            },
            inherited,
            self.sample_tx.clone(),
        )?;
        // DDS 1.4 §2.2.3.14: a RELIABLE KEEP_ALL reader whose
        // RESOURCE_LIMITS bound it is handed no more than it has room for.
        if let Some(room) = crate::runtime::ReaderRoom::for_reader(
            qos.reliability.kind == ReliabilityKind::Reliable,
            qos.history.kind == crate::qos::HistoryKind::KeepAll,
            &qos.resource_limits,
            Arc::clone(&self.pulled),
            &self.runtime,
        ) {
            self.runtime
                .set_user_reader_room(self.eid, Arc::clone(&room))?;
            let _ = self.cache.room.set(room);
        }
        if let Some(status) = self.status.upgrade() {
            status.install_data_signal()?;
        }
        if let Some(link) = self.content_filter.as_ref() {
            link.attach(self.eid);
        }
        if let Ok(mut ids) = self.subscriber.reader_entity_ids.lock() {
            ids.push(self.eid);
        }
        // DDS 1.4 §2.2.3.4: against the fictitious DataWriter of a TRANSIENT
        // or PERSISTENT topic, which offers the Topic QoS.
        if let Some(topic_qos) = self.topic.qos.lock().ok().map(|q| q.clone()) {
            let _ = self
                .runtime
                .check_reader_against_fictitious_writer(self.eid, &topic_qos);
        }
        Ok(())
    }
}

/// The policies of `qos` that the runtime holds and `set_qos` may change
/// once the reader is enabled.
#[cfg(feature = "std")]
fn runtime_qos_update(qos: &DataReaderQos) -> crate::runtime::ReaderQosUpdate {
    crate::runtime::ReaderQosUpdate {
        deadline: qos.deadline,
        time_based_filter: qos.time_based_filter,
        latency_budget: qos.latency_budget,
        user_data: qos.user_data.value.clone(),
        own: crate::runtime::GroupTopicQos {
            partition: qos.partition.names.clone(),
            group_data: qos.group_data.value.clone(),
            topic_data: qos.topic_data.value.clone(),
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// DDS 1.4 §2.2.4.2.2: a sample a listener call covered while the
    /// Subscriber's group order held it does not raise DATA_AVAILABLE and
    /// DATA_ON_READERS again when the release puts it into the inbox; a
    /// sample it did not cover does.
    #[test]
    fn a_group_release_after_a_listener_call_is_no_news_for_what_it_covered() {
        use crate::contained::ContainedReader;
        let sub = offline_subscriber();
        let topic = {
            let p = DomainParticipantFactory::instance()
                .create_participant_offline(0, crate::qos::DomainParticipantQos::default());
            p.create_topic::<RawBytes>("GroupNews", crate::qos::TopicQos::default())
                .unwrap()
        };
        let reader = sub
            .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
            .unwrap();
        let news = Arc::clone(&reader.inner.status.news);
        let item = || crate::runtime::UserSample::Alive {
            key_hash: None,
            payload: crate::sample_bytes::SampleBytes::from_vec(vec![1]),
            writer_guid: [0u8; 16],
            writer_strength: 0,
            representation: 0,
            big_endian: false,
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: -1,
            coherent_set_sn: None,
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        };
        // A listener call covered one held sample, for both flags.
        let covered = news.now();
        news.available.cover(covered);
        news.available.cover_held(1);
        news.on_readers.cover(covered);
        news.on_readers.cover_held(1);
        assert!(!news.has_news(&news.available));
        assert!(!news.has_news(&news.on_readers));
        // Its release is no news.
        reader.inner.accept_group_released(item());
        assert!(!news.has_news(&news.available), "DATA_AVAILABLE");
        assert!(!news.has_news(&news.on_readers), "DATA_ON_READERS");
        // A second release was not covered: news.
        reader.inner.accept_group_released(item());
        assert!(news.has_news(&news.available));
        assert!(news.has_news(&news.on_readers));
    }

    /// A subscriber as a participant creates one with the default
    /// ENTITY_FACTORY: enabled, so what it creates is enabled too
    /// (DDS 1.4 §2.2.3.20).
    fn offline_subscriber() -> Subscriber {
        let s = Subscriber::new(SubscriberQos::default(), None);
        let _ = s.inner.entity_state.enable();
        s
    }
    use crate::dds_type::RawBytes;
    use crate::factory::DomainParticipantFactory;
    use crate::qos::{DomainParticipantQos, TopicQos};

    fn mk_topic() -> Topic<RawBytes> {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        Topic::new("Chatter".into(), TopicQos::default(), p)
    }

    #[derive(Debug, PartialEq)]
    struct HolderProbe(u32);

    impl DdsType for HolderProbe {
        const TYPE_NAME: &'static str = "test::HolderProbe";
        const HAS_KEY: bool = true;
        // Conservative unknown maximum requires MD5, even for a short value.
        fn encode(
            &self,
            out: &mut Vec<u8>,
        ) -> core::result::Result<(), crate::dds_type::EncodeError> {
            out.extend_from_slice(&self.0.to_le_bytes());
            Ok(())
        }
        fn decode(_: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
            Err(crate::dds_type::DecodeError::Invalid {
                what: "not a full sample",
            })
        }
        fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
            holder.write_bytes(&self.0.to_be_bytes());
        }
        fn decode_key_holder_be(
            bytes: &[u8],
        ) -> core::result::Result<Self, crate::dds_type::DecodeError> {
            let bytes: [u8; 4] =
                bytes
                    .try_into()
                    .map_err(|_| crate::dds_type::DecodeError::Invalid {
                        what: "not a four-byte key holder",
                    })?;
            Ok(Self(u32::from_be_bytes(bytes)))
        }
    }

    /// Every policy TopicQos and DataReaderQos both carry is copied.
    #[test]
    fn copy_from_topic_qos_copies_every_policy_both_carry() {
        let mut topic = TopicQos::default();
        topic.durability.kind = crate::qos::DurabilityKind::TransientLocal;
        topic.deadline.period = zerodds_qos::Duration::from_millis(250);
        topic.latency_budget.duration = zerodds_qos::Duration::from_millis(5);
        topic.liveliness.kind = crate::qos::LivelinessKind::ManualByTopic;
        topic.reliability.kind = ReliabilityKind::Reliable;
        topic.destination_order.kind = crate::qos::DestinationOrderKind::BySourceTimestamp;
        topic.history.depth = 9;
        topic.resource_limits.max_samples = 40;
        topic.ownership.kind = crate::qos::OwnershipKind::Exclusive;
        let mut dr = DataReaderQos::default();
        Subscriber::copy_from_topic_qos(&mut dr, &topic).unwrap();
        assert_eq!(dr.durability, topic.durability);
        assert_eq!(dr.deadline, topic.deadline);
        assert_eq!(dr.latency_budget, topic.latency_budget);
        assert_eq!(dr.liveliness, topic.liveliness);
        assert_eq!(dr.reliability, topic.reliability);
        assert_eq!(dr.destination_order, topic.destination_order);
        assert_eq!(dr.history, topic.history);
        assert_eq!(dr.resource_limits, topic.resource_limits);
        assert_eq!(dr.ownership, topic.ownership);
    }

    #[test]
    fn get_key_value_uses_holder_decoder_for_writer_and_reader() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<HolderProbe>("HolderValues", TopicQos::default())
            .unwrap();
        let writer = p
            .create_publisher(crate::qos::PublisherQos::default())
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let reader = p
            .create_subscriber(crate::qos::SubscriberQos::default())
            .create_datareader(&topic, DataReaderQos::default())
            .unwrap();
        let sample = HolderProbe(0x0102_0304);
        let handle = writer.register_instance(&sample).unwrap();
        assert_eq!(writer.get_key_value(handle).unwrap(), sample);
        let holder = sample.0.to_be_bytes().to_vec();
        let hash = crate::dds_type::compute_key_hash(&holder, usize::MAX);
        let (handle, _) = reader.inner.instances.observe_sample(hash, holder, None);
        assert_eq!(reader.get_key_value(handle).unwrap(), sample);
    }

    /// A dispose with `PID_KEY_HASH` alone, for an instance this reader
    /// never saw, whose KeyHash is an MD5 (DDSI-RTPS 2.5 §9.6.4.8 Step
    /// 5.2): the key cannot be read from it, and the marker is not taken in.
    /// Taken in without a key holder, it made every `take` that selected it
    /// fail with `Unsupported`.
    #[test]
    fn a_hash_only_marker_of_an_unknown_md5_instance_is_not_taken_in() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<HolderProbe>("HashOnly", TopicQos::default())
            .unwrap();
        let reader = p
            .create_subscriber(crate::qos::SubscriberQos::default())
            .create_datareader(&topic, DataReaderQos::default())
            .unwrap();
        let holder = 7u32.to_be_bytes().to_vec();
        let hash = crate::dds_type::compute_key_hash(&holder, usize::MAX);
        reader
            .push_lifecycle_with_holder(hash, None, InstanceStateKind::NotAliveDisposed)
            .unwrap();
        assert_eq!(reader.inner.instances.lookup(&hash), None);
        assert!(reader.inner.cache.lock().unwrap().is_empty());
        assert!(reader.take_with_info().unwrap().is_empty());
        assert!(matches!(
            reader.get_key_value(HANDLE_NIL),
            Err(DdsError::BadParameter { .. })
        ));
    }

    /// The same marker for an instance the reader knows takes the key holder
    /// it has: the dispose names the key.
    #[test]
    fn a_hash_only_marker_of_a_known_instance_keeps_its_holder() {
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<HolderProbe>("HashOnlyKnown", TopicQos::default())
            .unwrap();
        let reader = p
            .create_subscriber(crate::qos::SubscriberQos::default())
            .create_datareader(&topic, DataReaderQos::default())
            .unwrap();
        let holder = 7u32.to_be_bytes().to_vec();
        let hash = crate::dds_type::compute_key_hash(&holder, usize::MAX);
        let (handle, _) = reader
            .inner
            .instances
            .observe_sample(hash, holder.clone(), None);
        reader
            .push_lifecycle_with_holder(hash, None, InstanceStateKind::NotAliveDisposed)
            .unwrap();
        assert_eq!(reader.inner.instances.get_key_holder(handle), Some(holder));
        assert_eq!(reader.get_key_value(handle).unwrap(), HolderProbe(7));
        let taken = reader.take_instance(handle).unwrap();
        assert_eq!(taken.len(), 1);
        assert!(!taken[0].info.valid_data);
        assert_eq!(
            taken[0].info.instance_state,
            InstanceStateKind::NotAliveDisposed
        );
        assert_eq!(taken[0].data, HolderProbe(7));
    }

    /// For a key of at most sixteen octets the KeyHash is the key's
    /// PLAIN_CDR2-BE octets padded with zeros (DDSI-RTPS 2.5 §9.6.4.8 Step
    /// 5.1): a marker with `PID_KEY_HASH` alone names its key, and the
    /// reader takes in the dispose of an instance it never saw.
    #[test]
    fn a_hash_only_marker_of_an_unknown_short_key_reads_the_key_from_the_hash() {
        #[derive(Debug, PartialEq)]
        struct ShortKey(u32);
        impl DdsType for ShortKey {
            const TYPE_NAME: &'static str = "test::ShortKey";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);
            fn encode(
                &self,
                out: &mut Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                out.extend_from_slice(&self.0.to_le_bytes());
                Ok(())
            }
            fn decode(_: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                Err(crate::dds_type::DecodeError::Invalid {
                    what: "not a full sample",
                })
            }
            fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
                holder.write_u32(self.0);
            }
            fn decode_key_holder_be(
                bytes: &[u8],
            ) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                let head: [u8; 4] = bytes.get(..4).and_then(|b| b.try_into().ok()).ok_or(
                    crate::dds_type::DecodeError::Invalid {
                        what: "a key holder shorter than four octets",
                    },
                )?;
                Ok(Self(u32::from_be_bytes(head)))
            }
        }
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = p
            .create_topic::<ShortKey>("HashOnlyShort", TopicQos::default())
            .unwrap();
        let reader = p
            .create_subscriber(crate::qos::SubscriberQos::default())
            .create_datareader(&topic, DataReaderQos::default())
            .unwrap();
        let hash = crate::dds_type::compute_key_hash(&9u32.to_be_bytes(), 4);
        reader
            .push_lifecycle_with_holder(hash, None, InstanceStateKind::NotAliveDisposed)
            .unwrap();
        let taken = reader.take_with_info().unwrap();
        assert_eq!(taken.len(), 1);
        assert!(!taken[0].info.valid_data);
        assert_eq!(
            taken[0].info.instance_state,
            InstanceStateKind::NotAliveDisposed
        );
        assert_eq!(taken[0].data, ShortKey(9));
    }

    #[test]
    fn typed_writer_propagates_known_zero_key_but_keyless_stays_unknown() {
        struct ZeroKey;
        impl DdsType for ZeroKey {
            const TYPE_NAME: &'static str = "test::ZeroKey";
            const HAS_KEY: bool = true;
            const KEY_HOLDER_MAX_SIZE: Option<usize> = Some(4);
            // One zero u32 has identical bytes in both representations.
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(
                &self,
                out: &mut Vec<u8>,
            ) -> core::result::Result<(), crate::dds_type::EncodeError> {
                out.extend_from_slice(&[0; 4]);
                Ok(())
            }
            fn decode(_: &[u8]) -> core::result::Result<Self, crate::dds_type::DecodeError> {
                Ok(Self)
            }
            fn encode_key_holder_be(&self, holder: &mut crate::dds_type::PlainCdr2BeKeyHolder) {
                holder.write_bytes(&[0; 4]);
            }
        }
        let domain_lease = zerodds_test_domains::lease_domain();
        let p = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .unwrap();
        let topic = p
            .create_topic::<ZeroKey>("ZeroKey", TopicQos::default())
            .unwrap();
        let publisher = p.create_publisher(crate::qos::PublisherQos::default());
        let writer = publisher
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let (runtime, _) = writer.runtime_handle().unwrap();
        let (_, rx) = runtime
            .register_user_reader_kind(
                crate::runtime::UserReaderConfig {
                    topic_name: topic.name().into(),
                    type_name: ZeroKey::TYPE_NAME.into(),
                    ..crate::runtime::UserReaderConfig::default()
                },
                true,
            )
            .unwrap();
        writer.write(&ZeroKey).unwrap();
        assert!(
            matches!(rx.recv_timeout(core::time::Duration::from_millis(200)).unwrap(),
            crate::runtime::UserSample::Alive { key_hash: Some(key), .. } if key == [0; 16])
        );
        let topic = p
            .create_topic::<RawBytes>("NoKey", TopicQos::default())
            .unwrap();
        let writer = publisher
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let (_, rx) = runtime
            .register_user_reader_kind(
                crate::runtime::UserReaderConfig {
                    topic_name: topic.name().into(),
                    type_name: RawBytes::TYPE_NAME.into(),
                    ..crate::runtime::UserReaderConfig::default()
                },
                false,
            )
            .unwrap();
        writer.write(&RawBytes::new(alloc::vec![1])).unwrap();
        assert!(matches!(
            rx.recv_timeout(core::time::Duration::from_millis(200))
                .unwrap(),
            crate::runtime::UserSample::Alive { key_hash: None, .. }
        ));
    }

    /// DDS 1.4 §2.2.4.1: a StatusChangedFlag is TRUE from the change on
    /// until the status is read or handed to a listener, and
    /// `get_status_changes` (§2.2.2.1.1.6) answers it at the call. It does
    /// not wait for a listener look: here the look of each endpoint is held
    /// open, as the dispatcher thread holds it during its pass, so no look
    /// raises the flag; the matched statuses are still changes of both.
    #[test]
    fn the_matched_statuses_are_changes_while_a_listener_look_runs() {
        use crate::entity::Entity;
        use crate::psm_constants::status;
        let domain_lease = zerodds_test_domains::lease_domain();
        let p = DomainParticipantFactory::instance()
            .create_participant(domain_lease.id(), DomainParticipantQos::default())
            .unwrap();
        let topic = p
            .create_topic::<RawBytes>("HeldLook", TopicQos::default())
            .unwrap();
        // Created disabled (§2.2.3.20 ENTITY_FACTORY), so that no match and
        // no look can come before the looks below are held.
        let disabled = crate::qos::EntityFactoryQosPolicy {
            autoenable_created_entities: false,
        };
        let publisher = p.create_publisher(crate::qos::PublisherQos {
            entity_factory: disabled,
            ..Default::default()
        });
        let subscriber = p.create_subscriber(crate::qos::SubscriberQos {
            entity_factory: disabled,
            ..Default::default()
        });
        let writer = publisher
            .create_datawriter(&topic, crate::qos::DataWriterQos::default())
            .unwrap();
        let reader = subscriber
            .create_datareader::<RawBytes>(&topic, DataReaderQos::default())
            .unwrap();
        let writer_state = writer.entity_state();
        let reader_state = reader.entity_state();
        assert!(writer_state.begin_listener_look(status::PUBLICATION_MATCHED));
        assert!(reader_state.begin_listener_look(status::SUBSCRIPTION_MATCHED));
        writer.enable().unwrap();
        reader.enable().unwrap();
        let deadline = std::time::Instant::now() + core::time::Duration::from_secs(10);
        let (runtime, writer_eid) = writer.runtime_handle().unwrap();
        let reader_eid = reader.inner.entity_id.unwrap();
        while runtime.user_writer_matched_count(writer_eid) == 0
            || runtime.user_reader_matched_count(reader_eid) == 0
        {
            assert!(std::time::Instant::now() < deadline, "no match");
            std::thread::sleep(core::time::Duration::from_millis(5));
        }
        let (writer_changes, reader_changes) =
            (writer.get_status_changes(), reader.get_status_changes());
        writer_state.end_listener_look(status::PUBLICATION_MATCHED);
        reader_state.end_listener_look(status::SUBSCRIPTION_MATCHED);
        assert_ne!(writer_changes & status::PUBLICATION_MATCHED, 0, "writer");
        assert_ne!(reader_changes & status::SUBSCRIPTION_MATCHED, 0, "reader");
        // Read, the flags go (§2.2.4.1).
        let _ = writer.get_publication_matched_status().unwrap();
        let _ = reader.get_subscription_matched_status().unwrap();
        assert_eq!(writer.get_status_changes() & status::PUBLICATION_MATCHED, 0);
        assert_eq!(
            reader.get_status_changes() & status::SUBSCRIPTION_MATCHED,
            0
        );
    }

    /// `decode_for_encap` must pick the decoder matching the sample's
    /// encapsulation — XCDR version *and* byte order — so an XCDR1 writer
    /// (CycloneDDS / legacy RTI default for `@final`) is decoded with the
    /// classic-CDR rule, not the XCDR2 rule. Regression for Bug DR1.
    #[test]
    fn decode_for_encap_dispatches_on_representation_and_endianness() {
        use crate::dds_type::{DecodeError, EncodeError};

        /// Marker whose four decode variants return a distinct value, so the
        /// test can prove which one `decode_for_encap` selected.
        #[derive(Debug, PartialEq)]
        struct Probe(u8);
        impl DdsType for Probe {
            const TYPE_NAME: &'static str = "test::Probe";
            /// @final with no member that XCDR1 and XCDR2 frame differently: one stream.
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.push(1);
                Ok(())
            }
            fn encode_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.push(2);
                Ok(())
            }
            fn encode_xcdr1(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.push(10);
                Ok(())
            }
            fn encode_xcdr1_be(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.push(20);
                Ok(())
            }
            fn decode(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(1)) // XCDR2-LE
            }
            fn decode_be(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(2)) // XCDR2-BE
            }
            fn decode_xcdr1(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(10)) // XCDR1-LE
            }
            fn decode_xcdr1_be(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Ok(Probe(20)) // XCDR1-BE
            }
        }

        // (representation, big_endian) -> expected marker.
        // representation: 1 = XCDR2, 0 = XCDR1.
        let topic = Topic::<Probe>::new_orphan("Probe".into(), crate::qos::TopicQos::default());
        for (xcdr1, big_endian, marker) in [
            (false, false, 1),
            (false, true, 2),
            (true, false, 10),
            (true, true, 20),
        ] {
            let mut body = alloc::vec![99];
            topic
                .codec()
                .encode(&Probe(0), &mut body, xcdr1, big_endian)
                .unwrap();
            assert_eq!(body, [99, marker], "the adapter appends without reframing");
        }
        assert_eq!(
            decode_for_encap::<Probe>(topic.codec(), &[], 1, false).unwrap(),
            Probe(1)
        );
        assert_eq!(
            decode_for_encap::<Probe>(topic.codec(), &[], 1, true).unwrap(),
            Probe(2)
        );
        assert_eq!(
            decode_for_encap::<Probe>(topic.codec(), &[], 0, false).unwrap(),
            Probe(10)
        );
        assert_eq!(
            decode_for_encap::<Probe>(topic.codec(), &[], 0, true).unwrap(),
            Probe(20)
        );
    }

    /// A failed decode carries diagnostic context (encapsulation + this reader's
    /// extensibility) and flags the extensibility/framing mismatch as a
    /// *plausible* — not asserted — cause (issue #27).
    #[test]
    fn decode_wire_error_carries_diagnostic_context() {
        use crate::dds_type::DecodeError;
        let inner = DecodeError::Invalid { what: "boom" };
        // RawBytes is @final by default; received XCDR2 little-endian.
        let msg = match decode_wire_error::<RawBytes>(&inner, 1, false) {
            DdsError::WireError { message } => message,
            _ => alloc::string::String::new(),
        };
        assert!(!msg.is_empty(), "expected a WireError variant");
        assert!(msg.contains("XCDR2"), "{msg}");
        assert!(msg.contains("little-endian"), "{msg}");
        assert!(msg.contains("@final"), "{msg}");
        assert!(msg.contains("DHEADER"), "{msg}");
        assert!(msg.to_lowercase().contains("plausible"), "{msg}");
        assert!(msg.contains("Not confirmed"), "{msg}");
    }

    /// XTypes 1.3 §7.2.4.1: "the default behavior is to discard the O2
    /// object to avoid misinterpretation." A discarded sample is an
    /// **absence** — the two types are compatible and the annotation
    /// asked for exactly this — so it must not fail the `take()` around
    /// it and it must not be delivered truncated either.
    ///
    /// Before this, the whole `take()` returned the decode error and
    /// the samples beside the discarded one went with it.
    #[test]
    fn a_discarded_sample_is_skipped_and_its_neighbours_survive() {
        use crate::dds_type::{DecodeError, EncodeError, Extensibility};

        /// One octet per sample. `0xDD` is the value this type cannot
        /// hold and whose `@try_construct` is DISCARD.
        #[derive(Debug, PartialEq)]
        struct Narrowing(u8);
        impl DdsType for Narrowing {
            const TYPE_NAME: &'static str = "test::Narrowing";
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn extensibility() -> Extensibility {
                Extensibility::Final
            }
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.push(self.0);
                Ok(())
            }
            fn decode(b: &[u8]) -> core::result::Result<Self, DecodeError> {
                match b.first() {
                    Some(0xDD) => Err(DecodeError::Discarded {
                        what: "the test type discards 0xDD",
                    }),
                    Some(v) => Ok(Self(*v)),
                    None => Err(DecodeError::Invalid { what: "empty" }),
                }
            }
        }

        let s = offline_subscriber();
        let topic = Topic::<Narrowing>::new(
            "Chatter".into(),
            crate::qos::TopicQos::default(),
            crate::factory::DomainParticipantFactory::instance()
                .create_participant_offline(0, crate::qos::DomainParticipantQos::default()),
        );
        // KEEP_ALL: three samples on this keyless (single-instance) topic
        // — the discarded one is an absence, but the two around it must
        // both survive; see `end_to_end_in_process_loopback`.
        let r = s
            .create_datareader::<Narrowing>(
                &topic,
                DataReaderQos {
                    history: crate::qos::HistoryQosPolicy {
                        kind: crate::qos::HistoryKind::KeepAll,
                        depth: 1,
                    },
                    ..DataReaderQos::default()
                },
            )
            .unwrap();
        r.__push_raw(alloc::vec![0x01]).unwrap();
        r.__push_raw(alloc::vec![0xDD]).unwrap();
        r.__push_raw(alloc::vec![0x02]).unwrap();

        let samples = r.take().unwrap();
        assert_eq!(
            samples.len(),
            2,
            "the discarded sample is an absence and the two around it are not"
        );
        assert_eq!(samples[0], Narrowing(0x01));
        assert_eq!(samples[1], Narrowing(0x02));
    }

    /// A decode failure that is *not* a narrowing still fails the take:
    /// it says the stream is wrong and the peer needs fixing, which is
    /// the distinction `DecodeError::Discarded` exists to keep.
    #[test]
    fn a_malformed_body_still_fails_the_take() {
        use crate::dds_type::{DecodeError, EncodeError, Extensibility};

        #[derive(Debug, PartialEq)]
        struct Strict;
        impl DdsType for Strict {
            const TYPE_NAME: &'static str = "test::Strict";
            const XCDR1_MATCHES_XCDR2: bool = true;
            fn extensibility() -> Extensibility {
                Extensibility::Final
            }
            fn encode(&self, _out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                Ok(())
            }
            fn decode(_b: &[u8]) -> core::result::Result<Self, DecodeError> {
                Err(DecodeError::Invalid { what: "truncated" })
            }
        }

        let s = offline_subscriber();
        let topic = Topic::<Strict>::new(
            "Chatter".into(),
            crate::qos::TopicQos::default(),
            crate::factory::DomainParticipantFactory::instance()
                .create_participant_offline(0, crate::qos::DomainParticipantQos::default()),
        );
        let r = s
            .create_datareader::<Strict>(&topic, DataReaderQos::default())
            .unwrap();
        r.__push_raw(alloc::vec![0x01]).unwrap();
        assert!(r.take().is_err());
    }

    /// `WireError::Discarded` is what `wire::try_construct` raises under
    /// `TryConstruct::Discard`, and it has to stay distinguishable from
    /// every other wire error all the way to the reader.
    #[test]
    fn a_wire_discard_converts_to_a_decode_discard() {
        use crate::dds_type::DecodeError;
        let discarded = zerodds_wire::WireError::Discarded {
            at: 4,
            expected: alloc::string::String::from("at most 10 octets"),
            found: alloc::string::String::from("12 of them"),
        };
        assert!(DecodeError::from(discarded).is_discarded());

        let truncated = zerodds_wire::WireError::Truncated {
            at: 0,
            needed: 4,
            available: 1,
        };
        assert!(!DecodeError::from(truncated).is_discarded());
    }

    #[test]
    fn subscriber_creates_datareader_for_matching_type() {
        let s = offline_subscriber();
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        assert_eq!(r.topic().name(), "Chatter");
    }

    #[test]
    fn datareader_take_returns_decoded_samples() {
        let s = offline_subscriber();
        // KEEP_ALL: two samples on this keyless (single-instance) topic
        // must both survive — see `end_to_end_in_process_loopback`.
        let r = s
            .create_datareader::<RawBytes>(
                &mk_topic(),
                DataReaderQos {
                    history: crate::qos::HistoryQosPolicy {
                        kind: crate::qos::HistoryKind::KeepAll,
                        depth: 1,
                    },
                    ..DataReaderQos::default()
                },
            )
            .unwrap();
        r.__push_raw(vec![1, 2, 3]).unwrap();
        r.__push_raw(vec![4, 5]).unwrap();
        let samples = r.take().unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].data, vec![1, 2, 3]);
        assert_eq!(samples[1].data, vec![4, 5]);
        // The inbox is now empty.
        let again = r.take().unwrap();
        assert!(again.is_empty());
    }

    /// Regression: a cached body must be materialized with the encapsulation it
    /// was received under. An XCDR1 ShapeType body (no DHEADER) previously
    /// failed here because `materialize`/`decode_or_keyholder` used the fixed
    /// XCDR2 `T::decode`; the reader then returned a `WireError` from
    /// `take_next_instance` and delivered zero samples even though the body
    /// decoded fine for the key. Covers XCDR1 little- and big-endian.
    #[test]
    fn cache_materialize_honors_xcdr1_encapsulation_le_and_be() {
        use crate::factory::DomainParticipantFactory;
        use crate::instance_handle::HANDLE_NIL;
        use crate::interop::ShapeType;
        use crate::qos::{DomainParticipantQos, TopicQos};

        let s = offline_subscriber();
        let p = DomainParticipantFactory::instance()
            .create_participant_offline(0, DomainParticipantQos::default());
        let topic = Topic::<ShapeType>::new("Square".into(), TopicQos::default(), p);
        let r = s
            .create_datareader::<ShapeType>(&topic, DataReaderQos::default())
            .unwrap();

        let sample = ShapeType::with_payload("BLUE", 40, 222, 30, vec![1, 2, 3]);

        // XCDR1 little-endian body (representation 0), via the cache +
        // materialize path exercised by take_next_instance.
        let mut le = Vec::new();
        sample.encode_xcdr1(&mut le).unwrap();
        r.__push_raw_encap(le, 0, false).unwrap();
        let got = r.take_next_instance(HANDLE_NIL).unwrap();
        assert_eq!(got.len(), 1, "XCDR1-LE sample must materialize");
        assert_eq!(got[0].data, sample);

        // XCDR1 big-endian body (representation 0, big_endian true).
        let mut be = Vec::new();
        sample.encode_xcdr1_be(&mut be).unwrap();
        r.__push_raw_encap(be, 0, true).unwrap();
        let got = r.take_next_instance(HANDLE_NIL).unwrap();
        assert_eq!(got.len(), 1, "XCDR1-BE sample must materialize");
        assert_eq!(got[0].data, sample);
    }

    /// A `@final` peer's XCDR2 body must not decode as an `@appendable` sample.
    ///
    /// The measured case (issue #30): a CycloneDDS `@final` writer sends
    /// `Robot{id: 1, label: n}` as PLAIN_CDR2, eight octets with no DHEADER. The
    /// `@appendable` reader takes the first four for a DHEADER of 1, and the
    /// generated decoder — which starts a member whenever it has not yet reached
    /// the declared end — then reads `label`'s octets as `id` and leaves `label`
    /// at its default. The extent bound turns that into a decode error.
    #[test]
    fn appendable_reader_rejects_final_framed_xcdr2_body() {
        use crate::dds_type::{DecodeError, EncodeError, Extensibility};

        /// The shape `zerodds-build` generates for an `@appendable` struct of
        /// two `uint32`s: a DHEADER, then a member read per member as long as
        /// the declared end has not been reached.
        #[derive(Debug, PartialEq)]
        struct Robot {
            id: u32,
            label: u32,
        }
        impl DdsType for Robot {
            const TYPE_NAME: &'static str = "test::Robot";
            const EXTENSIBILITY: Extensibility = Extensibility::Appendable;
            fn encode(&self, out: &mut Vec<u8>) -> core::result::Result<(), EncodeError> {
                out.extend_from_slice(&8u32.to_le_bytes());
                out.extend_from_slice(&self.id.to_le_bytes());
                out.extend_from_slice(&self.label.to_le_bytes());
                Ok(())
            }
            fn decode(bytes: &[u8]) -> core::result::Result<Self, DecodeError> {
                let mut head = [0u8; 4];
                head.copy_from_slice(
                    bytes
                        .get(..4)
                        .ok_or(DecodeError::Invalid { what: "dheader" })?,
                );
                let end = 4 + u32::from_le_bytes(head) as usize;
                let member = |at: usize| -> core::result::Result<u32, DecodeError> {
                    if at >= end {
                        return Ok(0);
                    }
                    let mut b = [0u8; 4];
                    b.copy_from_slice(
                        bytes
                            .get(at..at + 4)
                            .ok_or(DecodeError::Invalid { what: "truncated" })?,
                    );
                    Ok(u32::from_le_bytes(b))
                };
                Ok(Self {
                    id: member(4)?,
                    label: member(8)?,
                })
            }
        }

        let mut delimited = Vec::new();
        Robot { id: 1, label: 7 }.encode(&mut delimited).unwrap();
        assert_eq!(
            decode_for_encap::<Robot>(
                &crate::topic::StaticTopicCodec::of::<Robot>(),
                &delimited,
                1,
                false
            )
            .unwrap(),
            Robot { id: 1, label: 7 },
            "a DELIMITED_CDR body of this very type still decodes"
        );

        let mut final_framed = Vec::new();
        final_framed.extend_from_slice(&1u32.to_le_bytes());
        final_framed.extend_from_slice(&7u32.to_le_bytes());
        assert!(
            decode_for_encap::<Robot>(
                &crate::topic::StaticTopicCodec::of::<Robot>(),
                &final_framed,
                1,
                false
            )
            .is_err(),
            "a PLAIN_CDR2 body read as DELIMITED_CDR must fail, not yield shifted members"
        );
    }

    #[test]
    fn datareader_read_preserves_samples() {
        let s = offline_subscriber();
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        r.__push_raw(vec![0xAA]).unwrap();
        let first = r.read().unwrap();
        let second = r.read().unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    // poll_subscription_matched + listener-slot API.

    use core::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn datareader_set_listener_stores_arc_and_mask() {
        struct L;
        impl crate::listener::DataReaderListener for L {}
        let s = offline_subscriber();
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        assert!(r.get_listener().is_none());
        r.set_listener(Some(Arc::new(L)), crate::psm_constants::status::ANY);
        assert!(r.get_listener().is_some());
        assert_eq!(
            r.inner.entity_state.listener_mask(),
            crate::psm_constants::status::ANY
        );
    }

    /// Offline there is no runtime: a poll reads nothing and calls no
    /// listener, and the getters answer the default status.
    #[test]
    fn an_offline_reader_polls_nothing_and_reads_the_default_status() {
        struct Cnt(AtomicU32);
        impl crate::listener::DataReaderListener for Cnt {
            fn on_subscription_matched(
                &self,
                _r: crate::InstanceHandle,
                _s: crate::status::SubscriptionMatchedStatus,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let s = offline_subscriber();
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        let cnt = Arc::new(Cnt(AtomicU32::new(0)));
        r.set_listener(Some(cnt.clone()), crate::psm_constants::status::ANY);
        r.poll_subscription_matched();
        r.drive_listeners();
        assert_eq!(cnt.0.load(Ordering::Relaxed), 0);
        assert_eq!(
            r.get_subscription_matched_status().unwrap(),
            crate::status::SubscriptionMatchedStatus::default()
        );
        assert_eq!(r.get_sample_lost_status().unwrap().total_count, 0);
    }

    #[test]
    fn notify_data_arrived_fires_data_available_and_data_on_readers() {
        struct ReadCnt(AtomicU32, AtomicU32);
        impl crate::listener::DataReaderListener for ReadCnt {
            fn on_data_available(&self, _r: crate::InstanceHandle) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_subscription_matched(
                &self,
                _r: crate::InstanceHandle,
                _s: crate::status::SubscriptionMatchedStatus,
            ) {
                self.1.fetch_add(1, Ordering::Relaxed);
            }
        }
        let s = offline_subscriber();
        let r = s
            .create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap();
        let rc = Arc::new(ReadCnt(AtomicU32::new(0), AtomicU32::new(0)));
        r.set_listener(Some(rc.clone()), crate::psm_constants::status::ANY);
        r.notify_data_arrived();
        assert_eq!(rc.0.load(Ordering::Relaxed), 1);
        // sub_matched counter unchanged (different status bit).
        assert_eq!(rc.1.load(Ordering::Relaxed), 0);
    }

    // ---- §2.2.2.5.2.8/.9 begin/end_access ----

    #[test]
    fn subscriber_begin_end_access_roundtrip() {
        let s = offline_subscriber();
        assert!(!s.is_access_open());
        s.begin_access().unwrap();
        assert!(s.is_access_open());
        s.end_access().unwrap();
        assert!(!s.is_access_open());
    }

    #[test]
    fn subscriber_end_access_without_begin_returns_precondition_not_met() {
        // Spec §2.2.2.5.2.9 — end without begin is a spec violation.
        let s = offline_subscriber();
        let res = s.end_access();
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }

    #[test]
    fn subscriber_begin_access_is_nestable() {
        // Spec §2.2.2.5.2.8 — nesting allowed; each begin needs its own
        // end.
        let s = offline_subscriber();
        s.begin_access().unwrap();
        s.begin_access().unwrap();
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Still open after the first end (recursive nesting).
        assert!(s.is_access_open());
        s.end_access().unwrap();
        // Only after the second end is the scope closed again.
        assert!(!s.is_access_open());
    }

    #[test]
    fn subscriber_too_many_ends_after_balanced_returns_error() {
        // Negative: after a balanced begin/end, the next end is an
        // underflow → PreconditionNotMet.
        let s = offline_subscriber();
        s.begin_access().unwrap();
        s.end_access().unwrap();
        let res = s.end_access();
        assert!(matches!(
            res,
            Err(crate::error::DdsError::PreconditionNotMet { .. })
        ));
    }

    // ========================================================================
    // DDS 1.4 §2.2.3.16 LIFESPAN — reader-side sample expiry
    // ========================================================================

    /// §2.2.3.16: "The 'expiration time' of each sample is computed by adding
    /// the duration specified by the LIFESPAN QoS to the source timestamp."
    /// The INFINITE default (encoded as `0` nanoseconds on the delivery path)
    /// yields no expiration time at all.
    #[test]
    fn lifespan_expiry_adds_the_writer_duration_to_the_source_timestamp() {
        let src = Time::new(1_700_000_000, 900_000_000);
        // 250 ms — the OMG `dds-rtps` Test_Lifespan_0 value. Crosses the
        // second boundary, so a naive "add the nanos" implementation is caught.
        assert_eq!(
            lifespan_expiry(src, 250_000_000),
            Some(Time::new(1_700_000_001, 150_000_000))
        );
        // Whole seconds.
        assert_eq!(
            lifespan_expiry(src, 2_000_000_000),
            Some(Time::new(1_700_000_002, 900_000_000))
        );
        // `0` == INFINITE (the LIFESPAN default) → never expires.
        assert_eq!(lifespan_expiry(src, 0), None);
    }

    /// §2.2.3.16: "Once the sample expires, the data will be removed from the
    /// DataReader caches." A sample whose expiration time has passed is gone
    /// from the cache; one that is still inside its lifespan stays, and so
    /// does a lifecycle marker (which carries no expiration time).
    #[test]
    fn purge_expired_by_lifespan_drops_only_samples_past_their_expiration() {
        let now = Time::new(1_700_000_010, 0);
        let mk = |expires_at: Option<Time>, handle_seed: u8| CachedSample {
            bytes: Some(crate::sample_bytes::SampleBytes::from_vec(alloc::vec![
                handle_seed
            ])),
            info: SampleInfo::default(),
            representation: 0,
            big_endian: false,
            expires_at,
            decoded: None,
        };
        let cache = alloc::vec![
            // Expired one nanosecond ago.
            mk(Some(Time::new(1_700_000_009, 999_999_999)), 1),
            // Expires exactly now — §2.2.3.16 says "beyond which the data
            // should not be delivered", so reaching the instant expires it.
            mk(Some(now), 2),
            // Still inside its lifespan.
            mk(Some(Time::new(1_700_000_010, 1)), 3),
            // INFINITE lifespan / lifecycle marker — never expires.
            mk(None, 4),
        ];
        let mut cache: alloc::collections::VecDeque<CachedSample> = cache.into();
        purge_expired_by_lifespan(now, &mut cache, &InstanceTracker::new());
        let kept: Vec<u8> = cache
            .iter()
            .filter_map(|c| c.bytes.as_ref().map(|b| b[0]))
            .collect();
        assert_eq!(kept, alloc::vec![3u8, 4u8]);
    }

    /// Regression guard: before this fix, `purge_expired_by_lifespan` never
    /// told the `InstanceTracker` a sample left `cache`, so
    /// `InstanceState::samples_in_cache` drifted upward forever past a
    /// LIFESPAN purge — a count HISTORY/RESOURCE_LIMITS admission
    /// (`ingest_into_cache`) then rejects or evicts against, even though
    /// `cache` itself already dropped the sample.
    #[test]
    fn purge_expired_by_lifespan_drains_the_purged_instance_from_the_tracker() {
        let instances = InstanceTracker::new();
        let (handle, _) = instances.observe_sample([7u8; 16], alloc::vec![7], None);
        instances.observe_sample([7u8; 16], alloc::vec![7], None);
        assert_eq!(instances.get_by_handle(handle).unwrap().samples_in_cache, 2);

        let now = Time::new(1_700_000_010, 0);
        let expired_info = SampleInfo {
            instance_handle: handle,
            ..SampleInfo::default()
        };
        let still_alive_info = SampleInfo {
            instance_handle: handle,
            ..SampleInfo::default()
        };
        let cache = alloc::vec![
            CachedSample {
                bytes: Some(crate::sample_bytes::SampleBytes::from_vec(alloc::vec![1])),
                info: expired_info,
                representation: 0,
                big_endian: false,
                expires_at: Some(Time::new(1_700_000_009, 999_999_999)),
                decoded: None,
            },
            CachedSample {
                bytes: Some(crate::sample_bytes::SampleBytes::from_vec(alloc::vec![2])),
                info: still_alive_info,
                representation: 0,
                big_endian: false,
                expires_at: Some(Time::new(1_700_000_010, 1)),
                decoded: None,
            },
        ];
        let mut cache: alloc::collections::VecDeque<CachedSample> = cache.into();
        purge_expired_by_lifespan(now, &mut cache, &instances);
        assert_eq!(cache.len(), 1, "only the expired sample is purged");
        assert_eq!(
            instances.get_by_handle(handle).unwrap().samples_in_cache,
            1,
            "the purge must drain exactly the one sample it removed from \
             cache, not both and not neither"
        );
    }

    /// An offline reader of a subscriber with `coherent_access`.
    fn coherent_reader() -> DataReader<RawBytes> {
        let mut qos = SubscriberQos::default();
        qos.presentation.coherent_access = true;
        qos.presentation.access_scope = zerodds_qos::PresentationAccessScope::Topic;
        let s = Subscriber::new(qos, None);
        let _ = s.inner.entity_state.enable();
        s.create_datareader::<RawBytes>(&mk_topic(), DataReaderQos::default())
            .unwrap()
    }

    /// DDS-RPC 1.0 §7.8.2: a request is identified by the writer GUID and
    /// sequence number of its DATA, a reply by its related sample
    /// identity; `take_with_wire` hands both over with the sample.
    #[test]
    fn take_with_wire_hands_over_the_identity_and_the_related_identity() {
        // KEEP_ALL: under the default KEEP_LAST(1) the second sample replaces
        // the first (DDS 1.4 §2.2.3.18).
        let mut qos = DataReaderQos::default();
        qos.history.kind = crate::qos::HistoryKind::KeepAll;
        let subscriber = Subscriber::new(SubscriberQos::default(), None);
        let _ = subscriber.inner.entity_state.enable();
        let reader = subscriber
            .create_datareader::<RawBytes>(&mk_topic(), qos)
            .unwrap();
        let related = zerodds_rtps::inline_qos::SampleIdentityBytes::new([9; 16], 42);
        reader
            .__push_raw_related(alloc::vec![1, 2, 3], [7; 16], 5, Some(related))
            .unwrap();
        reader
            .__push_raw_related(alloc::vec![4], [7; 16], 6, None)
            .unwrap();
        let taken = reader.take_with_wire().unwrap();
        assert_eq!(taken.len(), 2);
        let first = taken[0].1.expect("a wire");
        assert_eq!(first.writer_guid, [7; 16]);
        assert_eq!(first.sequence_number, 5);
        assert_eq!(first.related_sample_identity, Some(related));
        let second = taken[1].1.expect("a wire");
        assert_eq!(second.sequence_number, 6);
        assert_eq!(second.related_sample_identity, None);
    }

    const W1: [u8; 16] = [1; 16];
    const W2: [u8; 16] = [2; 16];

    fn alive(writer: [u8; 16], sn: i64, set: Option<i64>) -> crate::runtime::UserSample {
        crate::runtime::UserSample::Alive {
            key_hash: None,
            payload: crate::sample_bytes::SampleBytes::from_vec(alloc::vec![0, 1, 0, 0]),
            writer_guid: writer,
            writer_strength: 0,
            representation: 0,
            big_endian: false,
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: sn,
            coherent_set_sn: set,
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        }
    }

    fn dispose(writer: [u8; 16], set: Option<i64>) -> crate::runtime::UserSample {
        crate::runtime::UserSample::Lifecycle {
            key_hash: [0; 16],
            kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid: writer,
            coherent_set_sn: set,
            source_sequence_number: -1,
        }
    }

    fn ecs(writer: [u8; 16], value: Option<i64>, sn: i64) -> crate::runtime::UserSample {
        ecs_after(writer, value, sn, None)
    }

    fn ecs_after(
        writer: [u8; 16],
        value: Option<i64>,
        sn: i64,
        complete: Option<bool>,
    ) -> crate::runtime::UserSample {
        crate::runtime::UserSample::CoherentSetEnd {
            writer_guid: writer,
            set_first_sn: value,
            ecs_sn: Some(sn),
            group: None,
            complete,
        }
    }

    /// What the gate let through: `(writer, SN)` of a sample, `(writer, 0)`
    /// of a dispose, `(writer, -1)` of an end-of-coherent-set marker.
    fn passed(items: &[crate::runtime::UserSample]) -> Vec<([u8; 16], i64)> {
        items
            .iter()
            .map(|item| match item {
                crate::runtime::UserSample::Alive {
                    writer_guid,
                    source_sequence_number,
                    ..
                } => (*writer_guid, *source_sequence_number),
                crate::runtime::UserSample::Lifecycle { writer_guid, .. } => (*writer_guid, 0),
                crate::runtime::UserSample::CoherentSetEnd { writer_guid, .. } => {
                    (*writer_guid, -1)
                }
            })
            .collect()
    }

    /// DDSI-RTPS 2.5 §8.7.6, Table 9.22: an end-of-coherent-set DATA ends
    /// the writer's set whatever its `PID_COHERENT_SET` names — none or
    /// `SEQUENCENUMBER_UNKNOWN` (Examples 2 and 3), the set's last SN
    /// (HDDS 1.4.8), its first (Table 9.23).
    #[test]
    fn every_end_of_coherent_set_form_releases_the_writers_set() {
        for value in [None, Some(3), Some(1)] {
            let r = coherent_reader();
            assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
            assert!(r.coherent_gate(alive(W1, 2, Some(1))).is_empty());
            assert!(r.coherent_gate(alive(W1, 3, Some(1))).is_empty());
            let out = r.coherent_gate(ecs(W1, value, 4));
            assert_eq!(passed(&out), [(W1, 1), (W1, 2), (W1, 3)], "{value:?}");
            assert_eq!(r.coherent_held_len(), 0);
        }
    }

    /// §8.7.6: "A Data Submessage from this Writer that does not contain a
    /// coherent set in-line QoS parameter" ends the set; the set comes
    /// first, then that DATA.
    #[test]
    fn a_sample_outside_any_set_releases_the_set_ahead_of_itself() {
        let r = coherent_reader();
        assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
        assert!(r.coherent_gate(alive(W1, 2, Some(1))).is_empty());
        let out = r.coherent_gate(alive(W1, 3, None));
        assert_eq!(passed(&out), [(W1, 1), (W1, 2), (W1, 3)]);
    }

    /// §8.7.6: "A Data Submessage from this Writer that belongs to a new
    /// coherent set" ends the earlier one; the new member is held.
    #[test]
    fn a_member_of_a_new_set_releases_the_earlier_set() {
        let r = coherent_reader();
        assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
        let out = r.coherent_gate(alive(W1, 2, Some(2)));
        assert_eq!(passed(&out), [(W1, 1)]);
        assert_eq!(r.coherent_held_len(), 1);
    }

    /// A dispose between `begin_coherent_changes` and `end_coherent_changes`
    /// is one of the set's changes: held with the set, released with it in
    /// write order. One outside a set ends the set like a sample does.
    #[test]
    fn a_dispose_in_a_set_is_held_with_it_one_outside_ends_it() {
        let r = coherent_reader();
        assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
        assert!(r.coherent_gate(dispose(W1, Some(1))).is_empty());
        assert_eq!(r.coherent_held_len(), 2);
        let out = r.coherent_gate(ecs(W1, None, 3));
        assert_eq!(passed(&out), [(W1, 1), (W1, 0)]);

        assert!(r.coherent_gate(alive(W1, 4, Some(4))).is_empty());
        let out = r.coherent_gate(dispose(W1, None));
        assert_eq!(passed(&out), [(W1, 4), (W1, 0)]);
    }

    /// DDSI-RTPS 2.5 §8.7.6: "Incomplete coherent sets shall not be added
    /// to the history of the RTPS DataReaders and the corresponding
    /// CacheChanges shall be discarded." The RTPS reader said so, or the
    /// members show it: a reader made inside the set holds two of the
    /// three changes before the end at SN 4.
    #[test]
    fn an_incomplete_set_is_discarded() {
        let r = coherent_reader();
        assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
        assert!(
            r.coherent_gate(ecs_after(W1, None, 2, Some(false)))
                .is_empty()
        );
        assert_eq!(r.coherent_held_len(), 0);

        assert!(r.coherent_gate(alive(W1, 3, Some(2))).is_empty());
        assert!(r.coherent_gate(alive(W1, 4, Some(2))).is_empty());
        assert!(r.coherent_gate(ecs_after(W1, None, 5, None)).is_empty());
        assert_eq!(r.coherent_held_len(), 0);

        assert!(r.coherent_gate(alive(W1, 6, Some(6))).is_empty());
        let out = r.coherent_gate(ecs_after(W1, None, 7, Some(true)));
        assert_eq!(passed(&out), [(W1, 6)]);
    }

    /// Each writer's set ends on that writer's DATA only.    /// Each writer's set ends on that writer's DATA only.
    #[test]
    fn another_writers_end_leaves_a_set_held() {
        let r = coherent_reader();
        assert!(r.coherent_gate(alive(W1, 1, Some(1))).is_empty());
        assert!(r.coherent_gate(alive(W2, 1, Some(1))).is_empty());
        let out = r.coherent_gate(ecs(W2, None, 2));
        assert_eq!(passed(&out), [(W2, 1)]);
        let out = r.coherent_gate(alive(W2, 3, None));
        assert_eq!(passed(&out), [(W2, 3)]);
        assert_eq!(r.coherent_held_len(), 1);
    }
}
