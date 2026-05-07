// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-Path-Hook-Points fuer `zerodds-monitor` (zerodds-monitor-1.0 §2.2).
//!
//! Dieses Modul existiert **ausschliesslich** wenn das `metrics`-Feature
//! aktiv ist (`cfg(feature = "metrics")` auf der `pub mod metrics`-
//! Deklaration in `lib.rs`). Damit gibt es keine Phantom-API: jede in
//! diesem Modul exportierte Funktion ist eine echte Counter-Operation
//! gegen [`zerodds_monitor::default_registry`].
//!
//! Call-Sites in `reliable_writer.rs` u. a. tragen jeweils ein eigenes
//! `#[cfg(feature = "metrics")]`-Attribut, sodass der Hot-Path im
//! `no_std + alloc`-Build *ohne* Counter-Funktionsaufruf compiliert.

use std::sync::{Arc, OnceLock};

use zerodds_monitor::{Counter, Labels, default_registry, metric_names};

struct RtpsCounters {
    heartbeats_sent: Arc<Counter>,
    acknacks_received: Arc<Counter>,
    retransmits: Arc<Counter>,
    fragmented_samples: Arc<Counter>,
    samples_dropped: Arc<Counter>,
    unknown_submessages: Arc<Counter>,
}

fn counters() -> &'static RtpsCounters {
    static C: OnceLock<RtpsCounters> = OnceLock::new();
    C.get_or_init(|| {
        let r = default_registry();
        r.set_help(
            metric_names::DDS_RTPS_HEARTBEATS_SENT_TOTAL,
            "Heartbeats gesendet (zerodds-monitor-1.0 §2.2)",
        );
        r.set_help(
            metric_names::DDS_RTPS_ACKNACKS_RECEIVED_TOTAL,
            "Acknacks empfangen (zerodds-monitor-1.0 §2.2)",
        );
        r.set_help(
            metric_names::DDS_RTPS_RETRANSMITS_TOTAL,
            "Retransmissions (zerodds-monitor-1.0 §2.2)",
        );
        r.set_help(
            metric_names::DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL,
            "Fragmentierte Samples (zerodds-monitor-1.0 §2.2)",
        );
        r.set_help(
            metric_names::DDS_RTPS_SAMPLES_DROPPED_TOTAL,
            "Samples gedropped (zerodds-monitor-1.0 §2.2)",
        );
        r.set_help(
            metric_names::DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL,
            "Unbekannte Submessage-Kinds (zerodds-monitor-1.0 §2.2)",
        );
        let writer = || Labels::new().with("writer_kind", "reliable");
        RtpsCounters {
            heartbeats_sent: r.counter(metric_names::DDS_RTPS_HEARTBEATS_SENT_TOTAL, writer()),
            acknacks_received: r.counter(metric_names::DDS_RTPS_ACKNACKS_RECEIVED_TOTAL, writer()),
            retransmits: r.counter(metric_names::DDS_RTPS_RETRANSMITS_TOTAL, writer()),
            fragmented_samples: r
                .counter(metric_names::DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL, writer()),
            samples_dropped: r.counter(
                metric_names::DDS_RTPS_SAMPLES_DROPPED_TOTAL,
                Labels::new()
                    .with("writer_kind", "reliable")
                    .with("reason", "history_limit"),
            ),
            unknown_submessages: r.counter(
                metric_names::DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL,
                Labels::new().with("vendor_id", "unknown"),
            ),
        }
    })
}

/// `dds_rtps_heartbeats_sent_total++`.
pub fn inc_heartbeat_sent() {
    counters().heartbeats_sent.inc();
}

/// `dds_rtps_acknacks_received_total++`.
pub fn inc_acknack_received() {
    counters().acknacks_received.inc();
}

/// `dds_rtps_retransmits_total++`.
pub fn inc_retransmit() {
    counters().retransmits.inc();
}

/// `dds_rtps_fragmented_samples_total++`.
pub fn inc_fragmented_sample() {
    counters().fragmented_samples.inc();
}

/// `dds_rtps_samples_dropped_total++`.
pub fn inc_sample_dropped() {
    counters().samples_dropped.inc();
}

/// `dds_rtps_unknown_submessages_total++`.
pub fn inc_unknown_submessage() {
    counters().unknown_submessages.inc();
}
