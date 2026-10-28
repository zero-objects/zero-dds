// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-path hook points for `zerodds-monitor` (zerodds-monitor-1.1 §2.3).
//!
//! Exists only under `cfg(feature = "metrics")`. Call sites in
//! `publisher.rs`/`subscriber.rs` etc. carry their own
//! `#[cfg(feature = "metrics")]` attribute.

use std::sync::Arc;

use zerodds_monitor::{Counter, LabeledHistogram, Labels, default_registry, metric_names};

/// Builds the label set with the `topic` label per the active
/// [`zerodds_monitor::TopicLabelPolicy`] (zerodds-monitor-1.1 §9). `Drop` omits
/// the `topic` label entirely.
fn topic_labels(topic: &str) -> Labels {
    match zerodds_monitor::topic_label(topic) {
        Some(v) => Labels::new().with("topic", v),
        None => Labels::new(),
    }
}

fn topic_counter(name: &'static str, topic: &str, help: &'static str) -> Arc<Counter> {
    let r = default_registry();
    r.set_help(name, help);
    r.counter(name, topic_labels(topic))
}

fn topic_histogram(name: &'static str, topic: &str, help: &'static str) -> Arc<LabeledHistogram> {
    let r = default_registry();
    r.set_help(name, help);
    r.histogram(name, topic_labels(topic))
}

/// The two series a writer's every `write` updates, looked up once.
///
/// [`inc_sample_written`] and [`record_sample_size`] build the label set,
/// take the registry's lock and hash the key on every call: measured on
/// the Linux test host, nine heap allocations per 1 kB BEST_EFFORT
/// sample of the 34 a write made. The registry hands out the same
/// `Arc` for the same name and labels, so holding it is the same series
/// and what the registry renders does not change.
#[derive(Debug)]
pub struct WriterMetrics {
    written: Arc<Counter>,
    size: Arc<LabeledHistogram>,
}

impl WriterMetrics {
    /// The series of `topic`, under the label policy in force now.
    #[must_use]
    pub fn for_topic(topic: &str) -> Self {
        Self {
            written: topic_counter(
                metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
                topic,
                "Geschriebene Samples (zerodds-monitor-1.1 §2.3)",
            ),
            size: topic_histogram(
                metric_names::DDS_DCPS_SAMPLE_SIZE_BYTES,
                topic,
                "Sample-Groessen (zerodds-monitor-1.1 §2.3)",
            ),
        }
    }

    /// One sample of `bytes` octets written.
    pub fn sample_written(&self, bytes: usize) {
        self.written.inc();
        self.size.record_ns(bytes as u64);
    }
}

/// The series a reader's every materialised sample updates, looked up
/// once; see [`WriterMetrics`].
#[derive(Debug)]
pub struct ReaderMetrics {
    read: Arc<Counter>,
}

impl ReaderMetrics {
    /// The series of `topic`, under the label policy in force now.
    #[must_use]
    pub fn for_topic(topic: &str) -> Self {
        Self {
            read: topic_counter(
                metric_names::DDS_DCPS_SAMPLES_READ_TOTAL,
                topic,
                "Gelesene Samples (zerodds-monitor-1.1 §2.3)",
            ),
        }
    }

    /// `n` samples read.
    pub fn samples_read(&self, n: u64) {
        self.read.add(n);
    }
}

/// Increment `dds_dcps_samples_written_total{topic=...}`.
pub fn inc_sample_written(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        topic,
        "Geschriebene Samples (zerodds-monitor-1.1 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_samples_read_total{topic=...}` by `n`.
pub fn add_samples_read(topic: &str, n: u64) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_READ_TOTAL,
        topic,
        "Gelesene Samples (zerodds-monitor-1.1 §2.3)",
    )
    .add(n);
}

/// Increment `dds_dcps_subscription_matched_total`.
pub fn inc_subscription_matched(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SUBSCRIPTION_MATCHED_TOTAL,
        topic,
        "Neue Subscriber-Matches (zerodds-monitor-1.1 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_subscription_unmatched_total`.
pub fn inc_subscription_unmatched(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SUBSCRIPTION_UNMATCHED_TOTAL,
        topic,
        "Verlorene Subscriber-Matches (zerodds-monitor-1.1 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_incompatible_qos_total`.
pub fn inc_incompatible_qos(topic: &str, policy_id: u32) {
    let r = default_registry();
    let name = metric_names::DDS_DCPS_INCOMPATIBLE_QOS_TOTAL;
    r.set_help(name, "QoS-Inkompatibilitaeten (zerodds-monitor-1.1 §2.3)");
    r.counter(
        name,
        topic_labels(topic).with("policy_id", format!("{policy_id}")),
    )
    .inc();
}

/// Increment `dds_dcps_deadline_missed_total`.
pub fn inc_deadline_missed(topic: &str, entity_kind: &str) {
    let r = default_registry();
    let name = metric_names::DDS_DCPS_DEADLINE_MISSED_TOTAL;
    r.set_help(name, "Deadline-Misses (zerodds-monitor-1.1 §2.3)");
    r.counter(
        name,
        topic_labels(topic).with("entity_kind", entity_kind.to_string()),
    )
    .inc();
}

/// Increment `dds_dcps_liveliness_lost_total`.
pub fn inc_liveliness_lost(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_LIVELINESS_LOST_TOTAL,
        topic,
        "Liveliness-Lost-Events (zerodds-monitor-1.1 §2.3)",
    )
    .inc();
}

/// Increment `dds_dcps_samples_lost_total`.
pub fn inc_sample_lost(topic: &str) {
    topic_counter(
        metric_names::DDS_DCPS_SAMPLES_LOST_TOTAL,
        topic,
        "SAMPLE_LOST-Status (zerodds-monitor-1.1 §2.3)",
    )
    .inc();
}

/// Records `dds_dcps_sample_size_bytes{topic=...}`.
pub fn record_sample_size(topic: &str, bytes: usize) {
    topic_histogram(
        metric_names::DDS_DCPS_SAMPLE_SIZE_BYTES,
        topic,
        "Sample-Groessen (zerodds-monitor-1.1 §2.3)",
    )
    .record_ns(bytes as u64);
}

/// Records `dds_dcps_sample_latency_seconds{topic=...}` in ns.
pub fn record_sample_latency_ns(topic: &str, ns: u64) {
    topic_histogram(
        metric_names::DDS_DCPS_SAMPLE_LATENCY_SECONDS,
        topic,
        "E2E-Latency Sekunden (zerodds-monitor-1.1 §2.3)",
    )
    .record_ns(ns);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// zerodds-monitor-1.1 §10 cross-layer test: a DCPS write increments
    /// `dds_dcps_samples_written_total`, after which the rendered
    /// default registry contains the increment with the topic label (default policy `Full`).
    #[test]
    fn write_increments_samples_written_and_renders() {
        let topic = "CrossLayerTest.MonitorWireup";
        inc_sample_written(topic);
        let out = default_registry().render_prometheus();
        assert!(
            out.contains(metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL),
            "render must contain the metric name"
        );
        assert!(
            out.contains(topic),
            "render must carry the topic label under the default Full policy"
        );
    }

    /// The series a writer and a reader hold are the registry's: what
    /// they record is what the free functions add to and what the
    /// registry renders.
    #[test]
    fn held_series_are_the_registry_series() {
        let topic = "CrossLayerTest.HeldSeries";
        let writer = WriterMetrics::for_topic(topic);
        writer.sample_written(10);
        writer.sample_written(10);
        inc_sample_written(topic);
        let written = default_registry().counter(
            metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
            topic_labels(topic),
        );
        assert_eq!(written.get(), 3);

        let reader = ReaderMetrics::for_topic(topic);
        reader.samples_read(2);
        add_samples_read(topic, 1);
        let read = default_registry().counter(
            metric_names::DDS_DCPS_SAMPLES_READ_TOTAL,
            topic_labels(topic),
        );
        assert_eq!(read.get(), 3);
    }
}
