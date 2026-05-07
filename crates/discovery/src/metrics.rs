// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Hot-Path-Hook-Points fuer `zerodds-monitor` (zerodds-monitor-1.0 §2.4).
//!
//! Existiert nur unter `cfg(feature = "metrics")`. Call-Sites in
//! `spdp.rs`/`sedp/*.rs` tragen jeweils ein eigenes
//! `#[cfg(feature = "metrics")]`-Attribut, sodass im no_std-Build kein
//! Counter-Funktionsaufruf compiliert.

use std::sync::{Arc, OnceLock};

use zerodds_monitor::{Counter, Gauge, Labels, default_registry, metric_names};

struct DiscoveryCounters {
    participants_known: Arc<Gauge>,
    endpoints_writers: Arc<Gauge>,
    endpoints_readers: Arc<Gauge>,
    spdp_announcements_sent: Arc<Counter>,
    sedp_pub_updates: Arc<Counter>,
    sedp_sub_updates: Arc<Counter>,
    type_lookups: Arc<Counter>,
}

fn counters() -> &'static DiscoveryCounters {
    static C: OnceLock<DiscoveryCounters> = OnceLock::new();
    C.get_or_init(|| {
        let r = default_registry();
        r.set_help(
            metric_names::DDS_DISCOVERY_PARTICIPANTS_KNOWN,
            "Bekannte Participants (zerodds-monitor-1.0 §2.4)",
        );
        r.set_help(
            metric_names::DDS_DISCOVERY_ENDPOINTS_KNOWN,
            "Bekannte Endpoints (zerodds-monitor-1.0 §2.4)",
        );
        r.set_help(
            metric_names::DDS_DISCOVERY_SPDP_ANNOUNCEMENTS_SENT_TOTAL,
            "SPDP-Announcements (zerodds-monitor-1.0 §2.4)",
        );
        r.set_help(
            metric_names::DDS_DISCOVERY_SEDP_UPDATES_TOTAL,
            "SEDP-Updates (zerodds-monitor-1.0 §2.4)",
        );
        r.set_help(
            metric_names::DDS_DISCOVERY_TYPE_LOOKUPS_TOTAL,
            "TypeLookup-Requests (zerodds-monitor-1.0 §2.4)",
        );
        let domain = || Labels::new().with("domain_id", "0");
        DiscoveryCounters {
            participants_known: r.gauge(metric_names::DDS_DISCOVERY_PARTICIPANTS_KNOWN, domain()),
            endpoints_writers: r.gauge(
                metric_names::DDS_DISCOVERY_ENDPOINTS_KNOWN,
                Labels::new().with("domain_id", "0").with("kind", "writer"),
            ),
            endpoints_readers: r.gauge(
                metric_names::DDS_DISCOVERY_ENDPOINTS_KNOWN,
                Labels::new().with("domain_id", "0").with("kind", "reader"),
            ),
            spdp_announcements_sent: r.counter(
                metric_names::DDS_DISCOVERY_SPDP_ANNOUNCEMENTS_SENT_TOTAL,
                domain(),
            ),
            sedp_pub_updates: r.counter(
                metric_names::DDS_DISCOVERY_SEDP_UPDATES_TOTAL,
                Labels::new()
                    .with("domain_id", "0")
                    .with("kind", "publication"),
            ),
            sedp_sub_updates: r.counter(
                metric_names::DDS_DISCOVERY_SEDP_UPDATES_TOTAL,
                Labels::new()
                    .with("domain_id", "0")
                    .with("kind", "subscription"),
            ),
            type_lookups: r.counter(metric_names::DDS_DISCOVERY_TYPE_LOOKUPS_TOTAL, domain()),
        }
    })
}

/// SPDP-Beacon serialisiert + im Begriff zu senden.
pub fn inc_spdp_announcement_sent() {
    counters().spdp_announcements_sent.inc();
}

/// SEDP Publication-Update gesendet.
pub fn inc_sedp_pub_update() {
    counters().sedp_pub_updates.inc();
}

/// SEDP Subscription-Update gesendet.
pub fn inc_sedp_sub_update() {
    counters().sedp_sub_updates.inc();
}

/// TypeLookup-Request gestellt.
pub fn inc_type_lookup() {
    counters().type_lookups.inc();
}

/// Participants-Known-Gauge auf den aktuellen Wert setzen.
pub fn set_participants_known(count: usize) {
    counters().participants_known.set(count as i64);
}

/// Writer-Endpoints-Known-Gauge setzen.
pub fn set_writer_endpoints_known(count: usize) {
    counters().endpoints_writers.set(count as i64);
}

/// Reader-Endpoints-Known-Gauge setzen.
pub fn set_reader_endpoints_known(count: usize) {
    counters().endpoints_readers.set(count as i64);
}
