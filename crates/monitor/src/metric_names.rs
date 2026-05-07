// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standard-Metric-Namen (Spec §2). 31 Konstanten — die volle
//! ZeroDDS-Telemetry-Domain.

// Transport (§2.1)
/// Transport-Domain — RTPS-Pakete gesendet.
pub const DDS_TRANSPORT_PACKETS_SENT_TOTAL: &str = "dds_transport_packets_sent_total";
/// Transport-Domain — RTPS-Pakete empfangen.
pub const DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL: &str = "dds_transport_packets_received_total";
/// Transport-Domain — Bytes gesendet.
pub const DDS_TRANSPORT_BYTES_SENT_TOTAL: &str = "dds_transport_bytes_sent_total";
/// Transport-Domain — Bytes empfangen.
pub const DDS_TRANSPORT_BYTES_RECEIVED_TOTAL: &str = "dds_transport_bytes_received_total";
/// Transport-Domain — Send-Fehler.
pub const DDS_TRANSPORT_SEND_ERRORS_TOTAL: &str = "dds_transport_send_errors_total";
/// Transport-Domain — Socket-Buffer-Auslastung (Gauge).
pub const DDS_TRANSPORT_SOCKET_BUFFER_BYTES: &str = "dds_transport_socket_buffer_bytes";

// RTPS (§2.2)
/// RTPS-Domain — Heartbeats gesendet.
pub const DDS_RTPS_HEARTBEATS_SENT_TOTAL: &str = "dds_rtps_heartbeats_sent_total";
/// RTPS-Domain — Acknacks empfangen.
pub const DDS_RTPS_ACKNACKS_RECEIVED_TOTAL: &str = "dds_rtps_acknacks_received_total";
/// RTPS-Domain — Retransmissions.
pub const DDS_RTPS_RETRANSMITS_TOTAL: &str = "dds_rtps_retransmits_total";
/// RTPS-Domain — Samples gedropped (History-Limit, etc.).
pub const DDS_RTPS_SAMPLES_DROPPED_TOTAL: &str = "dds_rtps_samples_dropped_total";
/// RTPS-Domain — Fragmentierte Samples.
pub const DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL: &str = "dds_rtps_fragmented_samples_total";
/// RTPS-Domain — Unbekannte Submessage-Kinds (Interop-Indikator).
pub const DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL: &str = "dds_rtps_unknown_submessages_total";

// DCPS (§2.3)
/// DCPS-Domain — Geschriebene Samples.
pub const DDS_DCPS_SAMPLES_WRITTEN_TOTAL: &str = "dds_dcps_samples_written_total";
/// DCPS-Domain — Gelesene Samples.
pub const DDS_DCPS_SAMPLES_READ_TOTAL: &str = "dds_dcps_samples_read_total";
/// DCPS-Domain — SAMPLE_LOST-Status.
pub const DDS_DCPS_SAMPLES_LOST_TOTAL: &str = "dds_dcps_samples_lost_total";
/// DCPS-Domain — Deadline-Misses.
pub const DDS_DCPS_DEADLINE_MISSED_TOTAL: &str = "dds_dcps_deadline_missed_total";
/// DCPS-Domain — Liveliness-Lost-Events.
pub const DDS_DCPS_LIVELINESS_LOST_TOTAL: &str = "dds_dcps_liveliness_lost_total";
/// DCPS-Domain — Neue Subscriber-Matches.
pub const DDS_DCPS_SUBSCRIPTION_MATCHED_TOTAL: &str = "dds_dcps_subscription_matched_total";
/// DCPS-Domain — Verlorene Subscriber-Matches.
pub const DDS_DCPS_SUBSCRIPTION_UNMATCHED_TOTAL: &str = "dds_dcps_subscription_unmatched_total";
/// DCPS-Domain — QoS-Inkompatibilitaeten.
pub const DDS_DCPS_INCOMPATIBLE_QOS_TOTAL: &str = "dds_dcps_incompatible_qos_total";
/// DCPS-Domain — Sample-Latency (Histogram, Sekunden).
pub const DDS_DCPS_SAMPLE_LATENCY_SECONDS: &str = "dds_dcps_sample_latency_seconds";
/// DCPS-Domain — Sample-Groessen (Histogram, Bytes).
pub const DDS_DCPS_SAMPLE_SIZE_BYTES: &str = "dds_dcps_sample_size_bytes";

// Discovery (§2.4)
/// Discovery-Domain — Bekannte Participants (Gauge).
pub const DDS_DISCOVERY_PARTICIPANTS_KNOWN: &str = "dds_discovery_participants_known";
/// Discovery-Domain — Bekannte Endpoints (Gauge).
pub const DDS_DISCOVERY_ENDPOINTS_KNOWN: &str = "dds_discovery_endpoints_known";
/// Discovery-Domain — SPDP-Announcements gesendet.
pub const DDS_DISCOVERY_SPDP_ANNOUNCEMENTS_SENT_TOTAL: &str =
    "dds_discovery_spdp_announcements_sent_total";
/// Discovery-Domain — SEDP-Updates.
pub const DDS_DISCOVERY_SEDP_UPDATES_TOTAL: &str = "dds_discovery_sedp_updates_total";
/// Discovery-Domain — TypeLookup-Requests.
pub const DDS_DISCOVERY_TYPE_LOOKUPS_TOTAL: &str = "dds_discovery_type_lookups_total";

// Security (§2.5)
/// Security-Domain — Authentication-Versuche.
pub const DDS_SECURITY_AUTH_ATTEMPTS_TOTAL: &str = "dds_security_auth_attempts_total";
/// Security-Domain — Access-Control-Denials.
pub const DDS_SECURITY_ACCESS_DENIED_TOTAL: &str = "dds_security_access_denied_total";
/// Security-Domain — Crypto-Operationen.
pub const DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL: &str = "dds_security_crypto_operations_total";
/// Security-Domain — Crypto-Latency (Histogram, Sekunden).
pub const DDS_SECURITY_CRYPTO_LATENCY_SECONDS: &str = "dds_security_crypto_latency_seconds";

/// Liste aller 31 Spec-Metric-Namen — fuer Coverage-Tests.
pub const ALL: &[&str] = &[
    DDS_TRANSPORT_PACKETS_SENT_TOTAL,
    DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL,
    DDS_TRANSPORT_BYTES_SENT_TOTAL,
    DDS_TRANSPORT_BYTES_RECEIVED_TOTAL,
    DDS_TRANSPORT_SEND_ERRORS_TOTAL,
    DDS_TRANSPORT_SOCKET_BUFFER_BYTES,
    DDS_RTPS_HEARTBEATS_SENT_TOTAL,
    DDS_RTPS_ACKNACKS_RECEIVED_TOTAL,
    DDS_RTPS_RETRANSMITS_TOTAL,
    DDS_RTPS_SAMPLES_DROPPED_TOTAL,
    DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL,
    DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL,
    DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
    DDS_DCPS_SAMPLES_READ_TOTAL,
    DDS_DCPS_SAMPLES_LOST_TOTAL,
    DDS_DCPS_DEADLINE_MISSED_TOTAL,
    DDS_DCPS_LIVELINESS_LOST_TOTAL,
    DDS_DCPS_SUBSCRIPTION_MATCHED_TOTAL,
    DDS_DCPS_SUBSCRIPTION_UNMATCHED_TOTAL,
    DDS_DCPS_INCOMPATIBLE_QOS_TOTAL,
    DDS_DCPS_SAMPLE_LATENCY_SECONDS,
    DDS_DCPS_SAMPLE_SIZE_BYTES,
    DDS_DISCOVERY_PARTICIPANTS_KNOWN,
    DDS_DISCOVERY_ENDPOINTS_KNOWN,
    DDS_DISCOVERY_SPDP_ANNOUNCEMENTS_SENT_TOTAL,
    DDS_DISCOVERY_SEDP_UPDATES_TOTAL,
    DDS_DISCOVERY_TYPE_LOOKUPS_TOTAL,
    DDS_SECURITY_AUTH_ATTEMPTS_TOTAL,
    DDS_SECURITY_ACCESS_DENIED_TOTAL,
    DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL,
    DDS_SECURITY_CRYPTO_LATENCY_SECONDS,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_count_matches_spec() {
        assert_eq!(ALL.len(), 31);
    }

    #[test]
    fn all_unique() {
        let mut sorted: Vec<&&str> = ALL.iter().collect();
        sorted.sort();
        for w in sorted.windows(2) {
            assert_ne!(w[0], w[1], "duplicate metric name: {}", w[0]);
        }
    }

    #[test]
    fn all_have_dds_prefix() {
        for name in ALL {
            assert!(name.starts_with("dds_"), "missing dds_ prefix: {name}");
        }
    }
}
