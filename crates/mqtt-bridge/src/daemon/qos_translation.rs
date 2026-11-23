// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! §6 — DDS QoS → MQTT behaviour, and the QoS of a topic's DDS endpoints.
//!
//! The rows of the table of Spec §6:
//!
//! * Reliability `RELIABLE` → MQTT QoS 1, `BEST_EFFORT` → QoS 0, when
//!   `mqtt_qos` is not set ([`mqtt_qos`]); a value that is set stays.
//! * Durability `VOLATILE` → retain false; `TRANSIENT_LOCAL`, `TRANSIENT`,
//!   `PERSISTENT` → retain true, when `retain` is not set ([`retain`]);
//!   `TRANSIENT` and `PERSISTENT` also go into the daemon's replay buffer
//!   ([`replayed`], `super::replay`).
//! * History `KEEP_LAST(N)` / `KEEP_ALL` → the samples that wait for the
//!   broker (`super::history`) and the HISTORY of the DataWriter.
//! * Lifespan → Message Expiry Interval ([`message_expiry`]) and the
//!   LIFESPAN of the DataWriter.
//! * Deadline → the DEADLINE of the DataReader and the DataWriter; a missed
//!   one is a retained PUBLISH on
//!   `<own_topic_prefix>/<client_id>/deadline_missed/…`.
//! * Liveliness → the LIVELINESS of both endpoints; the CONNECT's will on
//!   `<own_topic_prefix>/<client_id>/liveliness_lost` (`<own_topic_prefix>`
//!   is `mqtt.own_topic_prefix`, default `$zerodds`).
//! * Partition → the PARTITION of both endpoints: the DataReader matches
//!   only writers of its partitions, the samples of others are not
//!   published.

use zerodds_dcps::runtime::{UserReaderConfig, UserWriterConfig};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, Duration, LifespanQosPolicy, LivelinessKind,
    LivelinessQosPolicy,
};

use super::config::TopicConfig;

/// The MQTT QoS of `t`: `mqtt_qos` when set, else 1 for a reliable topic and
/// 0 for a best-effort one.
#[must_use]
pub fn mqtt_qos(t: &TopicConfig) -> u8 {
    t.mqtt_qos.unwrap_or(u8::from(t.is_reliable()))
}

/// The retain flag of `t`: `retain` when set, else `true` for the
/// durabilities that keep samples for late joiners.
#[must_use]
pub fn retain(t: &TopicConfig) -> bool {
    t.retain
        .unwrap_or(!matches!(durability(t), DurabilityKind::Volatile))
}

/// The DURABILITY of `t`'s endpoints; `volatile` when not set.
#[must_use]
pub fn durability(t: &TopicConfig) -> DurabilityKind {
    match t.durability.as_str() {
        "transient_local" => DurabilityKind::TransientLocal,
        "transient" => DurabilityKind::Transient,
        "persistent" => DurabilityKind::Persistent,
        _ => DurabilityKind::Volatile,
    }
}

/// Whether the samples of `t` go into the daemon's replay buffer:
/// `TRANSIENT` (in memory) and `PERSISTENT` (also in a file).
#[must_use]
pub fn replayed(t: &TopicConfig) -> bool {
    matches!(
        durability(t),
        DurabilityKind::Transient | DurabilityKind::Persistent
    )
}

/// A duration of `ms` milliseconds; infinite for `None`.
#[must_use]
pub fn duration_of_ms(ms: Option<u64>) -> Duration {
    let Some(ms) = ms else {
        return Duration::INFINITE;
    };
    let secs = ms / 1000;
    if secs >= u64::from(i32::MAX.unsigned_abs()) {
        return Duration::INFINITE;
    }
    let fraction = ((ms % 1000) << 32).div_ceil(1000);
    Duration {
        seconds: i32::try_from(secs).unwrap_or(i32::MAX),
        fraction: u32::try_from(fraction).unwrap_or(u32::MAX),
    }
}

/// The LIVELINESS of `t`'s endpoints.
#[must_use]
pub fn liveliness(t: &TopicConfig) -> LivelinessQosPolicy {
    LivelinessQosPolicy {
        kind: match t.liveliness_kind.as_str() {
            "manual_by_participant" => LivelinessKind::ManualByParticipant,
            "manual_by_topic" => LivelinessKind::ManualByTopic,
            _ => LivelinessKind::Automatic,
        },
        lease_duration: duration_of_ms(t.liveliness_lease_ms),
    }
}

/// The DataReader of `t` (`out`, `bidir`): reliability, durability,
/// deadline, liveliness and partition of §6; the one data representation it
/// accepts when `data_representation` is set (§3).
#[must_use]
pub fn reader_config(t: &TopicConfig) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: t.is_reliable(),
        durability: durability(t),
        deadline: DeadlineQosPolicy {
            period: duration_of_ms(t.deadline_ms),
        },
        liveliness: liveliness(t),
        partition: t.partition.clone(),
        data_representation_offer: t.data_representation_id().map(|id| vec![id]),
        ..Default::default()
    }
}

/// The DataWriter of `t` (`in`, `bidir`): reliability, durability,
/// deadline, lifespan, liveliness and partition of §6; the one data
/// representation it offers from its creation when `data_representation`
/// is set (§3).
#[must_use]
pub fn writer_config(t: &TopicConfig) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: t.is_reliable(),
        durability: durability(t),
        deadline: DeadlineQosPolicy {
            period: duration_of_ms(t.deadline_ms),
        },
        lifespan: LifespanQosPolicy {
            duration: duration_of_ms(t.lifespan_ms),
        },
        liveliness: liveliness(t),
        partition: t.partition.clone(),
        data_representation_offer: t.data_representation_id().map(|id| vec![id]),
        ..Default::default()
    }
}

/// The lifespan of a sample of `t` in nanoseconds: the writer's
/// (`sample_lifespan_nanos`, 0 for infinite), else `qos.lifespan_ms`; 0 for
/// infinite. A `qos.lifespan_ms` of `i32::MAX` seconds or more is infinite,
/// as the LIFESPAN of the topic's DataWriter is ([`duration_of_ms`]).
#[must_use]
pub fn lifespan_nanos(t: &TopicConfig, sample_lifespan_nanos: u64) -> u64 {
    if sample_lifespan_nanos != 0 {
        return sample_lifespan_nanos;
    }
    match t.lifespan_ms {
        Some(ms) if duration_of_ms(Some(ms)) != Duration::INFINITE => ms.saturating_mul(1_000_000),
        _ => 0,
    }
}

/// The Message Expiry Interval of a sample of `t` (Spec §4.2, §6): its
/// lifespan ([`lifespan_nanos`]) in seconds, rounded, at least 1; `None`
/// for an infinite lifespan.
#[must_use]
pub fn message_expiry(t: &TopicConfig, sample_lifespan_nanos: u64) -> Option<u32> {
    super::pump::message_expiry_of(lifespan_nanos(t, sample_lifespan_nanos))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn t(reliability: &str, durability: &str) -> TopicConfig {
        TopicConfig {
            dds_name: "T".into(),
            dds_type: "T".into(),
            mqtt_topic: "t".into(),
            reliability: reliability.into(),
            durability: durability.into(),
            ..TopicConfig::default()
        }
    }

    /// Spec §3, §6: `data_representation` is the one representation the
    /// DataWriter offers and the DataReader accepts; not set, both take the
    /// runtime's.
    #[test]
    fn mqttbridge_6_data_representation_is_the_offer_of_both_endpoints() {
        use zerodds_rtps::publication_data::data_representation as dr;
        let mut two = t("reliable", "");
        two.data_representation = Some("xcdr2".into());
        assert_eq!(
            writer_config(&two).data_representation_offer,
            Some(vec![dr::XCDR2])
        );
        assert_eq!(
            reader_config(&two).data_representation_offer,
            Some(vec![dr::XCDR2])
        );
        let mut one = t("reliable", "");
        one.data_representation = Some("xcdr1".into());
        assert_eq!(
            writer_config(&one).data_representation_offer,
            Some(vec![dr::XCDR])
        );
        assert_eq!(
            writer_config(&t("reliable", "")).data_representation_offer,
            None
        );
        assert_eq!(
            reader_config(&t("reliable", "")).data_representation_offer,
            None
        );
    }

    /// Spec §6 rows Reliability: RELIABLE → QoS 1, BEST_EFFORT → QoS 0; an
    /// absent reliability is RELIABLE; a set `mqtt_qos` stays, also 0 on a
    /// reliable topic and 2.
    #[test]
    fn mqttbridge_6_reliability_gives_the_mqtt_qos() {
        assert_eq!(mqtt_qos(&t("reliable", "")), 1);
        assert_eq!(mqtt_qos(&t("", "")), 1);
        assert_eq!(mqtt_qos(&t("best_effort", "")), 0);
        let mut set = t("reliable", "");
        set.mqtt_qos = Some(0);
        assert_eq!(mqtt_qos(&set), 0);
        set.mqtt_qos = Some(2);
        assert_eq!(mqtt_qos(&set), 2);
        assert!(reader_config(&t("reliable", "")).reliable);
        assert!(!writer_config(&t("best_effort", "")).reliable);
    }

    /// Spec §6 rows Durability: VOLATILE → retain false, the three others
    /// true; TRANSIENT and PERSISTENT are replayed; a set `retain` stays.
    #[test]
    fn mqttbridge_6_durability_gives_retain_and_the_replay() {
        for (d, want_retain, want_replay) in [
            ("volatile", false, false),
            ("", false, false),
            ("transient_local", true, false),
            ("transient", true, true),
            ("persistent", true, true),
        ] {
            let topic = t("reliable", d);
            assert_eq!(retain(&topic), want_retain, "{d}");
            assert_eq!(replayed(&topic), want_replay, "{d}");
        }
        let mut set = t("reliable", "transient_local");
        set.retain = Some(false);
        assert!(!retain(&set));
        assert_eq!(
            reader_config(&t("", "persistent")).durability,
            DurabilityKind::Persistent
        );
    }

    /// Spec §6 row Lifespan: the writer's lifespan in seconds, rounded, at
    /// least 1; `qos.lifespan_ms` where the writer names none; it is also
    /// the LIFESPAN of the topic's DataWriter.
    #[test]
    fn mqttbridge_6_lifespan_is_the_message_expiry_interval() {
        let mut topic = t("reliable", "");
        assert_eq!(message_expiry(&topic, 0), None);
        assert_eq!(message_expiry(&topic, 2_400_000_000), Some(2));
        assert_eq!(message_expiry(&topic, 1), Some(1));
        topic.lifespan_ms = Some(7_600);
        assert_eq!(message_expiry(&topic, 0), Some(8));
        assert_eq!(message_expiry(&topic, 3_000_000_000), Some(3));
        let w = writer_config(&topic);
        assert_eq!(w.lifespan.duration.to_nanos(), 7_600_000_000);
        assert_eq!(
            writer_config(&t("", "")).lifespan.duration,
            Duration::INFINITE
        );
    }

    /// Spec §3, §6 row Lifespan at the bounds: the interval is rounded
    /// without an overflow and kept within `u32::MAX` (a lifespan within
    /// half a second of `u64::MAX` ns used to wrap to 1 s, or to panic);
    /// a `qos.lifespan_ms` of `i32::MAX` seconds or more is infinite on
    /// both sides, as the LIFESPAN of the DataWriter.
    #[test]
    fn mqttbridge_6_a_lifespan_at_the_bounds_gives_no_wrong_interval() {
        use crate::daemon::pump::message_expiry_of;
        assert_eq!(message_expiry_of(u64::MAX), Some(u32::MAX));
        assert_eq!(message_expiry_of(u64::MAX - 499_999_999), Some(u32::MAX));
        assert_eq!(message_expiry_of(499_999_999), Some(1));
        assert_eq!(message_expiry_of(1_499_999_999), Some(1));
        assert_eq!(message_expiry_of(1_500_000_000), Some(2));
        let cfg = crate::daemon::config::DaemonConfig::load_from_str(
            "topics:\n  - dds_name: T\n    mqtt_topic: t\n    direction: out\n    qos:\n      lifespan_ms: 18446744073709551615\n",
        )
        .unwrap();
        let mut topic = cfg.topics[0].clone();
        assert_eq!(topic.lifespan_ms, Some(u64::MAX));
        assert_eq!(message_expiry(&topic, 0), None);
        assert_eq!(writer_config(&topic).lifespan.duration, Duration::INFINITE);
        let max = u64::from(i32::MAX.unsigned_abs()) * 1000;
        topic.lifespan_ms = Some(max);
        assert_eq!(message_expiry(&topic, 0), None);
        topic.lifespan_ms = Some(max - 1000);
        assert_eq!(message_expiry(&topic, 0), Some(2_147_483_646));
        assert_eq!(
            writer_config(&topic).lifespan.duration.seconds,
            i32::MAX - 1
        );
        // A writer's own lifespan comes before the key.
        assert_eq!(message_expiry(&topic, 3_000_000_000), Some(3));
    }

    /// Spec §6 rows Deadline, Liveliness, Partition: the policies of both
    /// endpoints.
    #[test]
    fn mqttbridge_6_deadline_liveliness_and_partition_reach_the_endpoints() {
        let mut topic = t("reliable", "");
        topic.deadline_ms = Some(250);
        topic.liveliness_kind = "manual_by_topic".into();
        topic.liveliness_lease_ms = Some(1_500);
        topic.partition = vec!["plant-a".into(), "line-*".into()];
        let r = reader_config(&topic);
        let w = writer_config(&topic);
        for (deadline, live, partition) in [
            (r.deadline, r.liveliness, &r.partition),
            (w.deadline, w.liveliness, &w.partition),
        ] {
            assert_eq!(deadline.period.to_nanos(), 250_000_000);
            assert_eq!(live.kind, LivelinessKind::ManualByTopic);
            assert_eq!(live.lease_duration.to_nanos(), 1_500_000_000);
            assert_eq!(partition, &["plant-a".to_string(), "line-*".to_string()]);
        }
        let plain = reader_config(&t("", ""));
        assert_eq!(plain.deadline.period, Duration::INFINITE);
        assert_eq!(plain.liveliness.kind, LivelinessKind::Automatic);
        assert_eq!(plain.liveliness.lease_duration, Duration::INFINITE);
        assert!(plain.partition.is_empty());
        assert_eq!(duration_of_ms(Some(u64::MAX)), Duration::INFINITE);
    }

    /// Spec §6: a duration of the configuration in milliseconds is DDS
    /// seconds and fractions of 2^-32 s (DDS 1.4 §2.3.2), rounded up so that
    /// it is not shorter than configured; `None` and a value of 2^31 - 1 s
    /// or more are infinite.
    #[test]
    fn mqttbridge_6_a_duration_in_milliseconds_is_seconds_and_fractions() {
        let d = duration_of_ms(Some(1_500));
        assert_eq!((d.seconds, d.fraction), (1, 0x8000_0000));
        let d = duration_of_ms(Some(1));
        assert_eq!(d.seconds, 0);
        assert_eq!(d.fraction, 4_294_968, "1 ms rounded up");
        assert!(d.to_nanos() >= 1_000_000);
        assert_eq!(duration_of_ms(Some(0)).to_nanos(), 0);
        assert_eq!(duration_of_ms(None), Duration::INFINITE);
        let max = u64::from(i32::MAX.unsigned_abs()) * 1000;
        assert_eq!(duration_of_ms(Some(max)), Duration::INFINITE);
        assert_eq!(duration_of_ms(Some(max - 1000)).seconds, i32::MAX - 1);
    }
}
