// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-Path-Hook-Points fuer `zerodds-monitor` (zerodds-monitor-1.0 §2.3).
//!
//! Existiert nur unter `cfg(feature = "metrics")`. Call-Sites in
//! `publisher.rs`/`subscriber.rs` u. a. tragen ein eigenes
//! `#[cfg(feature = "metrics")]`-Attribut.

use std::sync::Arc;

use zerodds_monitor::{Counter, LabeledHistogram, Labels, default_registry, metric_names};

fn topic_counter(name: &'static str, topic: &str, help: &'static str) -> Arc<Counter> {
    let r = default_registry();
    r.set_help(name, help);
    r.counter(name, Labels::new().with("topic", topic.to_string()))
}

fn topic_histogram(name: &'static str, topic: &str, help: &'static str) -> Arc<LabeledHistogram> {
    let r = default_registry();
    r.set_help(name, help);
    r.histogram(name, Labels::new().with("topic", topic.to_string()))
}

/// Increment `dds_dcps_samples_written_total{topic=...}`.
pub fn inc_sample_written(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        topic,
        "Geschriebene Samples (zerodds-monitor-1.0 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_samples_read_total{topic=...}` by `n`.
pub fn add_samples_read(topic: &str, n: u64) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_READ_TOTAL,
        topic,
        "Gelesene Samples (zerodds-monitor-1.0 §2.3)",
    )
    .add(n);
}

/// Increment `dds_dcps_subscription_matched_total`.
pub fn inc_subscription_matched(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SUBSCRIPTION_MATCHED_TOTAL,
        topic,
        "Neue Subscriber-Matches (zerodds-monitor-1.0 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_subscription_unmatched_total`.
pub fn inc_subscription_unmatched(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SUBSCRIPTION_UNMATCHED_TOTAL,
        topic,
        "Verlorene Subscriber-Matches (zerodds-monitor-1.0 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_incompatible_qos_total`.
pub fn inc_incompatible_qos(topic: &str, policy_id: u32) {
    let r = default_registry();
    let name = metric_names::DDS_DCPS_INCOMPATIBLE_QOS_TOTAL;
    r.set_help(name, "QoS-Inkompatibilitaeten (zerodds-monitor-1.0 §2.3)");
    r.counter(
        name,
        Labels::new()
            .with("topic", topic.to_string())
            .with("policy_id", format!("{policy_id}")),
    )
    .inc();
}

/// Increment `dds_dcps_deadline_missed_total`.
pub fn inc_deadline_missed(topic: &str, entity_kind: &str) {
    let r = default_registry();
    let name = metric_names::DDS_DCPS_DEADLINE_MISSED_TOTAL;
    r.set_help(name, "Deadline-Misses (zerodds-monitor-1.0 §2.3)");
    r.counter(
        name,
        Labels::new()
            .with("topic", topic.to_string())
            .with("entity_kind", entity_kind.to_string()),
    )
    .inc();
}

/// Increment `dds_dcps_liveliness_lost_total`.
pub fn inc_liveliness_lost(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_LIVELINESS_LOST_TOTAL,
        topic,
        "Liveliness-Lost-Events (zerodds-monitor-1.0 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_samples_lost_total`.
pub fn inc_sample_lost(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_LOST_TOTAL,
        topic,
        "SAMPLE_LOST-Status (zerodds-monitor-1.0 §2.3)",
    )
    .inc();
}

/// Records `dds_dcps_sample_size_bytes{topic=...}`.
pub fn record_sample_size(topic: &str, bytes: usize) {
    topic_histogram(
        metric_names::DDS_DCPS_SAMPLE_SIZE_BYTES,
        topic,
        "Sample-Groessen (zerodds-monitor-1.0 §2.3)",
    )
    .record_ns(bytes as u64);
}

/// Records `dds_dcps_sample_latency_seconds{topic=...}` in ns.
pub fn record_sample_latency_ns(topic: &str, ns: u64) {
    topic_histogram(
        metric_names::DDS_DCPS_SAMPLE_LATENCY_SECONDS,
        topic,
        "E2E-Latency Sekunden (zerodds-monitor-1.0 §2.3)",
    )
    .record_ns(ns);
}
