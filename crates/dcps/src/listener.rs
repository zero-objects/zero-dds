// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Listener hierarchy (DDS DCPS 1.4 §2.2.4.2 + §2.2.2.*.3 set_listener).
//!
//! Listeners are asynchronous notification hooks that the middleware
//! layer calls as soon as a communication status changes. Per entity
//! type there is one listener trait with one callback per relevant
//! status:
//!
//! ```text
//! DomainParticipantListener   (13 callbacks — all bubble-up targets)
//! ├── PublisherListener       (4  callbacks — writer-related)
//! │   └── DataWriterListener  (4  callbacks)
//! ├── SubscriberListener      (8  callbacks — reader + on_data_on_readers)
//! │   └── DataReaderListener  (7  callbacks — reader-specific)
//! └── TopicListener           (1  callback — on_inconsistent_topic)
//! ```
//!
//! ## Bubble-up (Spec §2.2.4.2.3)
//!
//! When **no** listener is set on the "smallest" entity (e.g.
//! DataReader) — or the listener does not have the status bit in its
//! mask — the event bubbles up to the next larger entity:
//! `Reader → Subscriber → Participant`. Likewise
//! `Writer → Publisher → Participant`. Topic events bubble directly to
//! the participant. The `bubble_up_consumed` helpers in
//! [`listener_dispatch`](crate::listener_dispatch) encapsulate this
//! resolution.
//!
//! ## Object safety
//!
//! All 6 traits are **object-safe** (no `Self` returns, no generics, no
//! associated types). So that each trait is usable generically over
//! `T: DdsType` (we have `Topic<T>`, `DataWriter<T>`, `DataReader<T>`),
//! we pass the entity handle as an opaque [`InstanceHandle`] — analogous
//! to the DDS-DCPS IDL-PSM, which also gives listener callbacks only the
//! entity *handle* (DCPS 1.4 §2.3.3 IDL).
//!
//! We store the listener as `Box<dyn ListenerTrait + Send + Sync>` in
//! the entity state, so that it is visible cross-thread (the spec does
//! not say that listener callbacks must run from the application
//! thread).
//!
//! All methods take `&self` (not `&mut self`), because the listener is
//! shared in the hot path; the callback body must use interior
//! mutability if it carries state.
//!
//! ## Default impls
//!
//! Every method has an empty body. Users override only the callbacks
//! they actually need.

extern crate alloc;

use alloc::boxed::Box;

use crate::entity::StatusMask;
use crate::instance_handle::InstanceHandle;
use crate::psm_constants::status as status_bits;
use crate::status::{
    InconsistentTopicStatus, LivelinessChangedStatus, LivelinessLostStatus,
    OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, PublicationMatchedStatus,
    RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, SampleLostStatus,
    SampleRejectedStatus, SubscriptionMatchedStatus,
};

// ============================================================================
// TopicListener (Spec §2.2.2.3.2)
// ============================================================================

/// `TopicListener` — Spec §2.2.2.3.2 + §2.2.4.2.5.
///
/// Exactly one callback: `on_inconsistent_topic`. The `topic` parameter
/// is passed as an opaque [`InstanceHandle`] (Spec §2.3.3 IDL-PSM).
pub trait TopicListener: Send + Sync {
    /// Spec §2.2.4.2.5 — called when another topic with the same name
    /// but a different type definition is discovered.
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, _status: InconsistentTopicStatus) {}
}

// ============================================================================
// DataWriterListener (Spec §2.2.2.4.4)
// ============================================================================

/// `DataWriterListener` — Spec §2.2.2.4.4 + §2.2.4.2.4.
///
/// 4 callbacks: `on_offered_deadline_missed`, `on_offered_incompatible_qos`,
/// `on_liveliness_lost`, `on_publication_matched`.
pub trait DataWriterListener: Send + Sync {
    /// Spec §2.2.4.2.4.1 — the writer did not honor the offered DEADLINE
    /// promise.
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Spec §2.2.4.2.4.2 — a matched reader has incompatible requested
    /// QoS.
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Spec §2.2.4.2.4.3 — the writer was declared not_alive from the
    /// readers' point of view.
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Spec §2.2.4.2.4.4 — a new compatible reader matched (or one
    /// disappeared).
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}
}

// ============================================================================
// PublisherListener (Spec §2.2.2.4.3)
// ============================================================================

/// `PublisherListener` — Spec §2.2.2.4.3.
///
/// Inheritance form (Spec): "is a listener of the writers contained
/// within the publisher". We mirror the 4 DataWriterListener methods
/// 1:1, so that the publisher works as a bubble-up target.
pub trait PublisherListener: Send + Sync {
    /// Bubble-up from [`DataWriterListener::on_offered_deadline_missed`].
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-up from [`DataWriterListener::on_offered_incompatible_qos`].
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-up from [`DataWriterListener::on_liveliness_lost`].
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Bubble-up from [`DataWriterListener::on_publication_matched`].
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}
}

// ============================================================================
// DataReaderListener (Spec §2.2.2.5.7)
// ============================================================================

/// `DataReaderListener` — Spec §2.2.2.5.7 + §2.2.4.2.6.
///
/// 7 reader-specific callbacks (the eighth, `on_data_on_readers`,
/// belongs to the [`SubscriberListener`]).
pub trait DataReaderListener: Send + Sync {
    /// Spec §2.2.4.2.6.1 — new data has arrived at the reader.
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Spec §2.2.4.2.6.2 — a sample was never received (e.g. overwritten
    /// by a newer one).
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Spec §2.2.4.2.6.3 — a sample was rejected (RESOURCE_LIMITS).
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Spec §2.2.4.2.6.4 — the reader did not receive a sample within
    /// the requested DEADLINE.
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Spec §2.2.4.2.6.5 — a matched writer has incompatible offered
    /// QoS.
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Spec §2.2.4.2.6.6 — the liveliness status of the matched writers
    /// has changed.
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Spec §2.2.4.2.6.7 — a new compatible writer matched (or went away).
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// SubscriberListener (Spec §2.2.2.5.6)
// ============================================================================

/// `SubscriberListener` — Spec §2.2.2.5.6 + §2.2.4.2.7.
///
/// Inherits all 7 reader callbacks + 1 additional `on_data_on_readers`.
pub trait SubscriberListener: Send + Sync {
    /// Spec §2.2.4.2.7.1 — some reader of the subscriber has new data
    /// (subscriber-level notification).
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {}

    /// Bubble-up from [`DataReaderListener::on_data_available`].
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Bubble-up from [`DataReaderListener::on_sample_lost`].
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Bubble-up from [`DataReaderListener::on_sample_rejected`].
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Bubble-up from [`DataReaderListener::on_requested_deadline_missed`].
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-up from [`DataReaderListener::on_requested_incompatible_qos`].
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-up from [`DataReaderListener::on_liveliness_changed`].
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Bubble-up from [`DataReaderListener::on_subscription_matched`].
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// DomainParticipantListener (Spec §2.2.2.2.3)
// ============================================================================

/// `DomainParticipantListener` — Spec §2.2.2.2.3 + §2.2.4.2.8.
///
/// Unifies all status callbacks of all subordinate entities, because
/// every event can — per spec — bubble all the way to the top if no
/// listener is installed on the narrower entity.
///
/// The spec lists **13 callbacks** (the union of all status hooks):
/// - 1 topic       (`on_inconsistent_topic`)
/// - 4 writer      (`on_offered_*`, `on_liveliness_lost`, `on_publication_matched`)
/// - 7 reader      (`on_data_available`, `on_sample_*`,
///                  `on_requested_*`, `on_liveliness_changed`,
///                  `on_subscription_matched`)
/// - 1 subscriber  (`on_data_on_readers`)
pub trait DomainParticipantListener: Send + Sync {
    // -------- Topic --------

    /// Bubble-up from [`TopicListener::on_inconsistent_topic`].
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, _status: InconsistentTopicStatus) {}

    // -------- Writer side --------

    /// Bubble-up from [`PublisherListener::on_offered_deadline_missed`].
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-up from [`PublisherListener::on_offered_incompatible_qos`].
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-up from [`PublisherListener::on_liveliness_lost`].
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Bubble-up from [`PublisherListener::on_publication_matched`].
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}

    // -------- Reader side --------

    /// Bubble-up from [`SubscriberListener::on_data_on_readers`].
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {}

    /// Bubble-up from [`SubscriberListener::on_data_available`].
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Bubble-up from [`SubscriberListener::on_sample_lost`].
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Bubble-up from [`SubscriberListener::on_sample_rejected`].
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Bubble-up from [`SubscriberListener::on_requested_deadline_missed`].
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-up from [`SubscriberListener::on_requested_incompatible_qos`].
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-up from [`SubscriberListener::on_liveliness_changed`].
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Bubble-up from [`SubscriberListener::on_subscription_matched`].
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// Boxed listener aliases (for storage in the entity state)
// ============================================================================

/// Heap-allocated, thread-safe box wrapper for the 6 listener traits.
/// This is how each entity stores its listener.
pub type BoxedTopicListener = Box<dyn TopicListener>;
/// Cf. [`BoxedTopicListener`].
pub type BoxedDataWriterListener = Box<dyn DataWriterListener>;
/// Cf. [`BoxedTopicListener`].
pub type BoxedPublisherListener = Box<dyn PublisherListener>;
/// Cf. [`BoxedTopicListener`].
pub type BoxedDataReaderListener = Box<dyn DataReaderListener>;
/// Cf. [`BoxedTopicListener`].
pub type BoxedSubscriberListener = Box<dyn SubscriberListener>;
/// Cf. [`BoxedTopicListener`].
pub type BoxedDomainParticipantListener = Box<dyn DomainParticipantListener>;

/// Arc variant: per slot we store the listener as `Arc<dyn ...>`,
/// because the hot path briefly clones the listener under the slot mutex
/// and then calls it outside the lock (to avoid deadlocks). A Box would
/// not allow that.
pub type ArcTopicListener = alloc::sync::Arc<dyn TopicListener>;
/// Cf. [`ArcTopicListener`].
pub type ArcDataWriterListener = alloc::sync::Arc<dyn DataWriterListener>;
/// Cf. [`ArcTopicListener`].
pub type ArcPublisherListener = alloc::sync::Arc<dyn PublisherListener>;
/// Cf. [`ArcTopicListener`].
pub type ArcDataReaderListener = alloc::sync::Arc<dyn DataReaderListener>;
/// Cf. [`ArcTopicListener`].
pub type ArcSubscriberListener = alloc::sync::Arc<dyn SubscriberListener>;
/// Cf. [`ArcTopicListener`].
pub type ArcDomainParticipantListener = alloc::sync::Arc<dyn DomainParticipantListener>;

// ============================================================================
// Bubble-up helpers
// ============================================================================

/// True if `mask` sets the bit for `status` **and** the listener is
/// non-`None`. This combination decides whether an event is consumed at
/// the current level (Spec §2.2.4.2.3).
#[inline]
#[must_use]
pub fn listener_handles(listener_present: bool, mask: StatusMask, status_bit: u32) -> bool {
    listener_present && (mask & status_bit) != 0
}

/// Convenience helper: returns the status-bit value for a status name.
/// Used only in tests + doc examples — the hot path uses the constants
/// in [`crate::psm_constants::status`] directly.
#[must_use]
pub fn status_bit_for_inconsistent_topic() -> u32 {
    status_bits::INCONSISTENT_TOPIC
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU32, Ordering};

    // -------- Object safety: all 6 traits must be usable as `dyn` --------

    #[test]
    fn topic_listener_is_object_safe() {
        struct Counter(AtomicU32);
        impl TopicListener for Counter {
            fn on_inconsistent_topic(
                &self,
                _topic: InstanceHandle,
                _status: InconsistentTopicStatus,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let _: BoxedTopicListener = Box::new(Counter(AtomicU32::new(0)));
    }

    #[test]
    fn datawriter_listener_is_object_safe() {
        struct L;
        impl DataWriterListener for L {}
        let _: BoxedDataWriterListener = Box::new(L);
    }

    #[test]
    fn publisher_listener_is_object_safe() {
        struct L;
        impl PublisherListener for L {}
        let _: BoxedPublisherListener = Box::new(L);
    }

    #[test]
    fn datareader_listener_is_object_safe() {
        struct L;
        impl DataReaderListener for L {}
        let _: BoxedDataReaderListener = Box::new(L);
    }

    #[test]
    fn subscriber_listener_is_object_safe() {
        struct L;
        impl SubscriberListener for L {}
        let _: BoxedSubscriberListener = Box::new(L);
    }

    #[test]
    fn participant_listener_is_object_safe() {
        struct L;
        impl DomainParticipantListener for L {}
        let _: BoxedDomainParticipantListener = Box::new(L);
    }

    // -------- Default impls may have an empty body --------

    #[test]
    fn default_callbacks_do_not_panic() {
        // Empty impl on all 6 traits.
        struct Noop;
        impl TopicListener for Noop {}
        impl DataWriterListener for Noop {}
        impl PublisherListener for Noop {}
        impl DataReaderListener for Noop {}
        impl SubscriberListener for Noop {}
        impl DomainParticipantListener for Noop {}
        // We can at least construct + box them — the actual call needs
        // an entity (see tests in entity.rs).
        let _: BoxedDomainParticipantListener = Box::new(Noop);
    }

    #[test]
    fn listener_handles_respects_mask_and_presence() {
        let bit = status_bit_for_inconsistent_topic();
        assert!(listener_handles(true, bit, bit));
        assert!(!listener_handles(false, bit, bit));
        assert!(!listener_handles(true, 0, bit));
        // Bit not in mask.
        assert!(!listener_handles(true, status_bits::SAMPLE_LOST, bit));
    }

    #[test]
    fn status_bit_for_inconsistent_topic_matches_psm() {
        assert_eq!(
            status_bit_for_inconsistent_topic(),
            status_bits::INCONSISTENT_TOPIC
        );
    }

    #[test]
    fn all_listener_traits_default_methods_invoke_safely() {
        // Exercises the default bodies of all 6 listener traits.
        // Since all default methods have empty bodies, we simply go
        // through and call them on a Noop instance.
        struct Noop;
        impl TopicListener for Noop {}
        impl DataWriterListener for Noop {}
        impl PublisherListener for Noop {}
        impl DataReaderListener for Noop {}
        impl SubscriberListener for Noop {}
        impl DomainParticipantListener for Noop {}

        let h = InstanceHandle::from_raw(1);
        let n = Noop;
        TopicListener::on_inconsistent_topic(&n, h, InconsistentTopicStatus::default());

        DataWriterListener::on_offered_deadline_missed(
            &n,
            h,
            OfferedDeadlineMissedStatus::default(),
        );
        DataWriterListener::on_offered_incompatible_qos(
            &n,
            h,
            OfferedIncompatibleQosStatus::default(),
        );
        DataWriterListener::on_liveliness_lost(&n, h, LivelinessLostStatus::default());
        DataWriterListener::on_publication_matched(&n, h, PublicationMatchedStatus::default());

        PublisherListener::on_offered_deadline_missed(
            &n,
            h,
            OfferedDeadlineMissedStatus::default(),
        );
        PublisherListener::on_offered_incompatible_qos(
            &n,
            h,
            OfferedIncompatibleQosStatus::default(),
        );
        PublisherListener::on_liveliness_lost(&n, h, LivelinessLostStatus::default());
        PublisherListener::on_publication_matched(&n, h, PublicationMatchedStatus::default());

        DataReaderListener::on_data_available(&n, h);
        DataReaderListener::on_sample_lost(&n, h, SampleLostStatus::default());
        DataReaderListener::on_sample_rejected(&n, h, SampleRejectedStatus::default());
        DataReaderListener::on_requested_deadline_missed(
            &n,
            h,
            RequestedDeadlineMissedStatus::default(),
        );
        DataReaderListener::on_requested_incompatible_qos(
            &n,
            h,
            RequestedIncompatibleQosStatus::default(),
        );
        DataReaderListener::on_liveliness_changed(&n, h, LivelinessChangedStatus::default());
        DataReaderListener::on_subscription_matched(&n, h, SubscriptionMatchedStatus::default());

        SubscriberListener::on_data_on_readers(&n, h);
        SubscriberListener::on_data_available(&n, h);
        SubscriberListener::on_sample_lost(&n, h, SampleLostStatus::default());
        SubscriberListener::on_sample_rejected(&n, h, SampleRejectedStatus::default());
        SubscriberListener::on_requested_deadline_missed(
            &n,
            h,
            RequestedDeadlineMissedStatus::default(),
        );
        SubscriberListener::on_requested_incompatible_qos(
            &n,
            h,
            RequestedIncompatibleQosStatus::default(),
        );
        SubscriberListener::on_liveliness_changed(&n, h, LivelinessChangedStatus::default());
        SubscriberListener::on_subscription_matched(&n, h, SubscriptionMatchedStatus::default());

        DomainParticipantListener::on_inconsistent_topic(&n, h, InconsistentTopicStatus::default());
        DomainParticipantListener::on_offered_deadline_missed(
            &n,
            h,
            OfferedDeadlineMissedStatus::default(),
        );
        DomainParticipantListener::on_offered_incompatible_qos(
            &n,
            h,
            OfferedIncompatibleQosStatus::default(),
        );
        DomainParticipantListener::on_liveliness_lost(&n, h, LivelinessLostStatus::default());
        DomainParticipantListener::on_publication_matched(
            &n,
            h,
            PublicationMatchedStatus::default(),
        );
        DomainParticipantListener::on_data_on_readers(&n, h);
        DomainParticipantListener::on_data_available(&n, h);
        DomainParticipantListener::on_sample_lost(&n, h, SampleLostStatus::default());
        DomainParticipantListener::on_sample_rejected(&n, h, SampleRejectedStatus::default());
        DomainParticipantListener::on_requested_deadline_missed(
            &n,
            h,
            RequestedDeadlineMissedStatus::default(),
        );
        DomainParticipantListener::on_requested_incompatible_qos(
            &n,
            h,
            RequestedIncompatibleQosStatus::default(),
        );
        DomainParticipantListener::on_liveliness_changed(&n, h, LivelinessChangedStatus::default());
        DomainParticipantListener::on_subscription_matched(
            &n,
            h,
            SubscriptionMatchedStatus::default(),
        );
    }

    #[test]
    fn datareader_listener_call_runs_default_methods() {
        struct Counters {
            avail: AtomicU32,
            lost: AtomicU32,
        }
        impl DataReaderListener for Counters {
            fn on_data_available(&self, _r: InstanceHandle) {
                self.avail.fetch_add(1, Ordering::Relaxed);
            }
            fn on_sample_lost(&self, _r: InstanceHandle, _s: SampleLostStatus) {
                self.lost.fetch_add(1, Ordering::Relaxed);
            }
        }
        let c = Counters {
            avail: AtomicU32::new(0),
            lost: AtomicU32::new(0),
        };
        let h = InstanceHandle::from_raw(1);
        c.on_data_available(h);
        c.on_data_available(h);
        c.on_sample_lost(h, SampleLostStatus::default());
        // Methods we did not override should work as a default no-op.
        c.on_subscription_matched(h, SubscriptionMatchedStatus::default());
        assert_eq!(c.avail.load(Ordering::Relaxed), 2);
        assert_eq!(c.lost.load(Ordering::Relaxed), 1);
    }
}
