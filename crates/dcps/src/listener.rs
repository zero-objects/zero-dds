// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Listener-Hierarchie (DDS DCPS 1.4 §2.2.4.2 + §2.2.2.*.3 set_listener).
//!
//! Listener sind asynchrone Notification-Hooks, die der Middleware-
//! Layer aufruft, sobald sich ein Communication-Status ändert. Pro
//! Entity-Typ gibt es einen Listener-Trait mit einem Callback je
//! relevantem Status:
//!
//! ```text
//! DomainParticipantListener   (13 Callbacks — alle Bubble-Up-Targets)
//! ├── PublisherListener       (4  Callbacks — Writer-bezogen)
//! │   └── DataWriterListener  (4  Callbacks)
//! ├── SubscriberListener      (8  Callbacks — Reader + on_data_on_readers)
//! │   └── DataReaderListener  (7  Callbacks — Reader-spezifisch)
//! └── TopicListener           (1  Callback — on_inconsistent_topic)
//! ```
//!
//! ## Bubble-Up (Spec §2.2.4.2.3)
//!
//! Wenn auf der "kleinsten" Entity (z.B. DataReader) **kein** Listener
//! gesetzt ist (oder der Listener das Status-Bit nicht in seiner Mask
//! hat), bubblet das Event nach oben zur nächst-grösseren Entity:
//! `Reader → Subscriber → Participant`. Analog
//! `Writer → Publisher → Participant`. Topic-Events bubbeln direkt zum
//! Participant. Die `bubble_up_consumed`-Helfer in
//! [`listener_dispatch`](crate::listener_dispatch) kapseln diese
//! Resolution.
//!
//! ## Object-Safety
//!
//! Alle 6 Traits sind **object-safe** (keine `Self`-Returns, keine
//! Generics, keine assoziierten Typen). Damit der jeweilige Trait
//! generisch über `T: DdsType` einsetzbar ist (wir haben `Topic<T>`,
//! `DataWriter<T>`, `DataReader<T>`), übergeben wir den Entity-Handle
//! als opaken [`InstanceHandle`] — analog zum DDS-DCPS-IDL-PSM, das
//! Listener-Callbacks ebenfalls nur den Entity-*Handle* gibt
//! (DCPS 1.4 §2.3.3 IDL).
//!
//! Wir speichern den Listener als
//! `Box<dyn ListenerTrait + Send + Sync>` im Entity-State, damit er
//! Cross-Thread sichtbar ist (Spec sagt nicht, dass Listener-Callbacks
//! aus dem Application-Thread laufen müssen).
//!
//! Alle Methoden haben `&self` (nicht `&mut self`), weil der Listener
//! im hot path geteilt wird; der Callback-Body muss interior mutability
//! verwenden, falls er State führt.
//!
//! ## Default-Impls
//!
//! Jede Methode hat ein Empty-Body. Anwender überschreiben nur die
//! Callbacks, die sie wirklich brauchen.

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
/// Genau ein Callback: `on_inconsistent_topic`. Der `topic`-Parameter
/// wird als opaker [`InstanceHandle`] übergeben (Spec §2.3.3 IDL-PSM).
pub trait TopicListener: Send + Sync {
    /// Spec §2.2.4.2.5 — wird gerufen, wenn ein anderes Topic mit
    /// gleichem Namen, aber unterschiedlichem Type-Definition entdeckt
    /// wird.
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, _status: InconsistentTopicStatus) {}
}

// ============================================================================
// DataWriterListener (Spec §2.2.2.4.4)
// ============================================================================

/// `DataWriterListener` — Spec §2.2.2.4.4 + §2.2.4.2.4.
///
/// 4 Callbacks: `on_offered_deadline_missed`, `on_offered_incompatible_qos`,
/// `on_liveliness_lost`, `on_publication_matched`.
pub trait DataWriterListener: Send + Sync {
    /// Spec §2.2.4.2.4.1 — Writer hat das offered DEADLINE-Versprechen
    /// nicht eingehalten.
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Spec §2.2.4.2.4.2 — ein matched Reader hat inkompatible
    /// requested-QoS.
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Spec §2.2.4.2.4.3 — Writer wurde aus Sicht der Reader als
    /// not_alive deklariert.
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Spec §2.2.4.2.4.4 — ein neuer kompatibler Reader matched (oder
    /// einer ist verschwunden).
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}
}

// ============================================================================
// PublisherListener (Spec §2.2.2.4.3)
// ============================================================================

/// `PublisherListener` — Spec §2.2.2.4.3.
///
/// Inheritance-Form (Spec): "is a listener of the writers contained
/// within the publisher". Wir spiegeln die 4 DataWriterListener-Methoden
/// 1:1, damit der Publisher als Bubble-Up-Target funktioniert.
pub trait PublisherListener: Send + Sync {
    /// Bubble-Up von [`DataWriterListener::on_offered_deadline_missed`].
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-Up von [`DataWriterListener::on_offered_incompatible_qos`].
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-Up von [`DataWriterListener::on_liveliness_lost`].
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Bubble-Up von [`DataWriterListener::on_publication_matched`].
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}
}

// ============================================================================
// DataReaderListener (Spec §2.2.2.5.7)
// ============================================================================

/// `DataReaderListener` — Spec §2.2.2.5.7 + §2.2.4.2.6.
///
/// 7 Reader-spezifische Callbacks (das achte, `on_data_on_readers`,
/// gehört zum [`SubscriberListener`]).
pub trait DataReaderListener: Send + Sync {
    /// Spec §2.2.4.2.6.1 — neue Daten sind zum Reader gekommen.
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Spec §2.2.4.2.6.2 — ein Sample wurde nie empfangen
    /// (z.B. überschrieben durch einen jüngeren).
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Spec §2.2.4.2.6.3 — ein Sample wurde abgewiesen
    /// (RESOURCE_LIMITS).
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Spec §2.2.4.2.6.4 — der Reader hat keine Sample innerhalb des
    /// requested DEADLINE bekommen.
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Spec §2.2.4.2.6.5 — ein matched Writer hat inkompatible
    /// offered-QoS.
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Spec §2.2.4.2.6.6 — Liveliness-Status der matched Writer
    /// hat sich geändert.
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Spec §2.2.4.2.6.7 — neuer kompatibler Writer matched (oder weg).
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// SubscriberListener (Spec §2.2.2.5.6)
// ============================================================================

/// `SubscriberListener` — Spec §2.2.2.5.6 + §2.2.4.2.7.
///
/// Erbt alle 7 Reader-Callbacks + 1 zusätzlichen `on_data_on_readers`.
pub trait SubscriberListener: Send + Sync {
    /// Spec §2.2.4.2.7.1 — irgendein Reader des Subscribers hat neue
    /// Daten (Subscriber-Level-Notification).
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {}

    /// Bubble-Up von [`DataReaderListener::on_data_available`].
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Bubble-Up von [`DataReaderListener::on_sample_lost`].
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Bubble-Up von [`DataReaderListener::on_sample_rejected`].
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Bubble-Up von [`DataReaderListener::on_requested_deadline_missed`].
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-Up von [`DataReaderListener::on_requested_incompatible_qos`].
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-Up von [`DataReaderListener::on_liveliness_changed`].
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Bubble-Up von [`DataReaderListener::on_subscription_matched`].
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// DomainParticipantListener (Spec §2.2.2.2.3)
// ============================================================================

/// `DomainParticipantListener` — Spec §2.2.2.2.3 + §2.2.4.2.8.
///
/// Vereinigt alle Status-Callbacks aller untergeordneten Entities, weil
/// jedes Event spec-treu nach ganz oben bubblen kann, wenn auf der
/// engeren Entity kein Listener installiert ist.
///
/// Die Spec listet **13 Callbacks** (Vereinigung aller Status-Hooks):
/// - 1 Topic       (`on_inconsistent_topic`)
/// - 4 Writer-     (`on_offered_*`, `on_liveliness_lost`, `on_publication_matched`)
/// - 7 Reader-     (`on_data_available`, `on_sample_*`,
///                  `on_requested_*`, `on_liveliness_changed`,
///                  `on_subscription_matched`)
/// - 1 Subscriber- (`on_data_on_readers`)
pub trait DomainParticipantListener: Send + Sync {
    // -------- Topic --------

    /// Bubble-Up von [`TopicListener::on_inconsistent_topic`].
    fn on_inconsistent_topic(&self, _topic: InstanceHandle, _status: InconsistentTopicStatus) {}

    // -------- Writer-Seite --------

    /// Bubble-Up von [`PublisherListener::on_offered_deadline_missed`].
    fn on_offered_deadline_missed(
        &self,
        _writer: InstanceHandle,
        _status: OfferedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-Up von [`PublisherListener::on_offered_incompatible_qos`].
    fn on_offered_incompatible_qos(
        &self,
        _writer: InstanceHandle,
        _status: OfferedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-Up von [`PublisherListener::on_liveliness_lost`].
    fn on_liveliness_lost(&self, _writer: InstanceHandle, _status: LivelinessLostStatus) {}

    /// Bubble-Up von [`PublisherListener::on_publication_matched`].
    fn on_publication_matched(&self, _writer: InstanceHandle, _status: PublicationMatchedStatus) {}

    // -------- Reader-Seite --------

    /// Bubble-Up von [`SubscriberListener::on_data_on_readers`].
    fn on_data_on_readers(&self, _subscriber: InstanceHandle) {}

    /// Bubble-Up von [`SubscriberListener::on_data_available`].
    fn on_data_available(&self, _reader: InstanceHandle) {}

    /// Bubble-Up von [`SubscriberListener::on_sample_lost`].
    fn on_sample_lost(&self, _reader: InstanceHandle, _status: SampleLostStatus) {}

    /// Bubble-Up von [`SubscriberListener::on_sample_rejected`].
    fn on_sample_rejected(&self, _reader: InstanceHandle, _status: SampleRejectedStatus) {}

    /// Bubble-Up von [`SubscriberListener::on_requested_deadline_missed`].
    fn on_requested_deadline_missed(
        &self,
        _reader: InstanceHandle,
        _status: RequestedDeadlineMissedStatus,
    ) {
    }

    /// Bubble-Up von [`SubscriberListener::on_requested_incompatible_qos`].
    fn on_requested_incompatible_qos(
        &self,
        _reader: InstanceHandle,
        _status: RequestedIncompatibleQosStatus,
    ) {
    }

    /// Bubble-Up von [`SubscriberListener::on_liveliness_changed`].
    fn on_liveliness_changed(&self, _reader: InstanceHandle, _status: LivelinessChangedStatus) {}

    /// Bubble-Up von [`SubscriberListener::on_subscription_matched`].
    fn on_subscription_matched(&self, _reader: InstanceHandle, _status: SubscriptionMatchedStatus) {
    }
}

// ============================================================================
// Boxed-Listener-Aliases (für Speicherung im Entity-State)
// ============================================================================

/// Heap-allokierter, threadsicherer Box-Wrapper für die 6
/// Listener-Traits. So speichert die jeweilige Entity ihren Listener.
pub type BoxedTopicListener = Box<dyn TopicListener>;
/// Vgl. [`BoxedTopicListener`].
pub type BoxedDataWriterListener = Box<dyn DataWriterListener>;
/// Vgl. [`BoxedTopicListener`].
pub type BoxedPublisherListener = Box<dyn PublisherListener>;
/// Vgl. [`BoxedTopicListener`].
pub type BoxedDataReaderListener = Box<dyn DataReaderListener>;
/// Vgl. [`BoxedTopicListener`].
pub type BoxedSubscriberListener = Box<dyn SubscriberListener>;
/// Vgl. [`BoxedTopicListener`].
pub type BoxedDomainParticipantListener = Box<dyn DomainParticipantListener>;

/// Arc-Variante fuer per Slot speichern wir den Listener als
/// `Arc<dyn ...>`, weil der Hot-Path den Listener kurz unter dem
/// Slot-Mutex klont und dann ausserhalb des Locks ruft (Deadlock-
/// Vermeidung). Box wuerde das nicht zulassen.
pub type ArcTopicListener = alloc::sync::Arc<dyn TopicListener>;
/// Vgl. [`ArcTopicListener`].
pub type ArcDataWriterListener = alloc::sync::Arc<dyn DataWriterListener>;
/// Vgl. [`ArcTopicListener`].
pub type ArcPublisherListener = alloc::sync::Arc<dyn PublisherListener>;
/// Vgl. [`ArcTopicListener`].
pub type ArcDataReaderListener = alloc::sync::Arc<dyn DataReaderListener>;
/// Vgl. [`ArcTopicListener`].
pub type ArcSubscriberListener = alloc::sync::Arc<dyn SubscriberListener>;
/// Vgl. [`ArcTopicListener`].
pub type ArcDomainParticipantListener = alloc::sync::Arc<dyn DomainParticipantListener>;

// ============================================================================
// Bubble-Up-Helpers
// ============================================================================

/// True wenn `mask` das Bit für `status` setzt **und** der Listener
/// nicht-`None` ist. Diese Kombi entscheidet, ob ein Event auf der
/// aktuellen Stufe konsumiert wird (Spec §2.2.4.2.3).
#[inline]
#[must_use]
pub fn listener_handles(listener_present: bool, mask: StatusMask, status_bit: u32) -> bool {
    listener_present && (mask & status_bit) != 0
}

/// Vorab-Hilfsfunktion: liefert den Status-Bit-Wert zu einem
/// Status-Namen. Nur in Tests + Doku-Beispielen verwendet — Hot-Path
/// nutzt direkt die Konstanten in [`crate::psm_constants::status`].
#[must_use]
pub fn status_bit_for_inconsistent_topic() -> u32 {
    status_bits::INCONSISTENT_TOPIC
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU32, Ordering};

    // -------- Object-Safety: alle 6 Traits müssen als `dyn` benutzbar sein --------

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

    // -------- Default-Impls dürfen leeren Body haben --------

    #[test]
    fn default_callbacks_do_not_panic() {
        // Empty-Impl auf allen 6 Traits.
        struct Noop;
        impl TopicListener for Noop {}
        impl DataWriterListener for Noop {}
        impl PublisherListener for Noop {}
        impl DataReaderListener for Noop {}
        impl SubscriberListener for Noop {}
        impl DomainParticipantListener for Noop {}
        // Wir können sie zumindest konstruieren + boxen — der
        // Aufruf braucht eine Entity (s. Tests in entity.rs).
        let _: BoxedDomainParticipantListener = Box::new(Noop);
    }

    #[test]
    fn listener_handles_respects_mask_and_presence() {
        let bit = status_bit_for_inconsistent_topic();
        assert!(listener_handles(true, bit, bit));
        assert!(!listener_handles(false, bit, bit));
        assert!(!listener_handles(true, 0, bit));
        // Bit nicht in Mask.
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
        // Stresst die Default-Bodies aller 6 Listener-Traits.
        // Da alle Default-Methoden Empty-Bodies haben, gehen wir
        // einfach durch und rufen sie auf einer Noop-Instanz.
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
        // Methoden, die wir nicht ueberschrieben haben, sollten als
        // Default-No-Op funktionieren.
        c.on_subscription_matched(h, SubscriptionMatchedStatus::default());
        assert_eq!(c.avail.load(Ordering::Relaxed), 2);
        assert_eq!(c.lost.load(Ordering::Relaxed), 1);
    }
}
