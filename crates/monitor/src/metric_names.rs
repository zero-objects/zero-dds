// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standard metric names (spec §2). 31 constants — the full
//! ZeroDDS telemetry domain.

// Transport (§2.1)
/// Transport domain — RTPS packets sent.
pub const DDS_TRANSPORT_PACKETS_SENT_TOTAL: &str = "dds_transport_packets_sent_total";
/// Transport domain — RTPS packets received.
pub const DDS_TRANSPORT_PACKETS_RECEIVED_TOTAL: &str = "dds_transport_packets_received_total";
/// Transport domain — bytes sent.
pub const DDS_TRANSPORT_BYTES_SENT_TOTAL: &str = "dds_transport_bytes_sent_total";
/// Transport domain — bytes received.
pub const DDS_TRANSPORT_BYTES_RECEIVED_TOTAL: &str = "dds_transport_bytes_received_total";
/// Transport domain — send error.
pub const DDS_TRANSPORT_SEND_ERRORS_TOTAL: &str = "dds_transport_send_errors_total";
/// Transport domain — socket buffer utilization (gauge).
pub const DDS_TRANSPORT_SOCKET_BUFFER_BYTES: &str = "dds_transport_socket_buffer_bytes";

// RTPS (§2.2)
/// RTPS domain — heartbeats sent.
pub const DDS_RTPS_HEARTBEATS_SENT_TOTAL: &str = "dds_rtps_heartbeats_sent_total";
/// RTPS domain — acknacks received.
pub const DDS_RTPS_ACKNACKS_RECEIVED_TOTAL: &str = "dds_rtps_acknacks_received_total";
/// RTPS domain — retransmissions.
pub const DDS_RTPS_RETRANSMITS_TOTAL: &str = "dds_rtps_retransmits_total";
/// RTPS domain — samples dropped (history limit, etc.).
pub const DDS_RTPS_SAMPLES_DROPPED_TOTAL: &str = "dds_rtps_samples_dropped_total";
/// RTPS domain — fragmented samples.
pub const DDS_RTPS_FRAGMENTED_SAMPLES_TOTAL: &str = "dds_rtps_fragmented_samples_total";
/// RTPS domain — unknown submessage kinds (interop indicator).
pub const DDS_RTPS_UNKNOWN_SUBMESSAGES_TOTAL: &str = "dds_rtps_unknown_submessages_total";

// DCPS (§2.3)
/// DCPS domain — samples written.
pub const DDS_DCPS_SAMPLES_WRITTEN_TOTAL: &str = "dds_dcps_samples_written_total";
/// DCPS domain — samples read.
pub const DDS_DCPS_SAMPLES_READ_TOTAL: &str = "dds_dcps_samples_read_total";
/// DCPS domain — SAMPLE_LOST status.
pub const DDS_DCPS_SAMPLES_LOST_TOTAL: &str = "dds_dcps_samples_lost_total";
/// DCPS domain — deadline misses.
pub const DDS_DCPS_DEADLINE_MISSED_TOTAL: &str = "dds_dcps_deadline_missed_total";
/// DCPS domain — liveliness-lost events.
pub const DDS_DCPS_LIVELINESS_LOST_TOTAL: &str = "dds_dcps_liveliness_lost_total";
/// DCPS domain — new subscriber matches.
pub const DDS_DCPS_SUBSCRIPTION_MATCHED_TOTAL: &str = "dds_dcps_subscription_matched_total";
/// DCPS domain — lost subscriber matches.
pub const DDS_DCPS_SUBSCRIPTION_UNMATCHED_TOTAL: &str = "dds_dcps_subscription_unmatched_total";
/// DCPS domain — QoS incompatibilities.
pub const DDS_DCPS_INCOMPATIBLE_QOS_TOTAL: &str = "dds_dcps_incompatible_qos_total";
/// DCPS domain — sample latency (histogram, seconds).
pub const DDS_DCPS_SAMPLE_LATENCY_SECONDS: &str = "dds_dcps_sample_latency_seconds";
/// DCPS domain — sample sizes (histogram, bytes).
pub const DDS_DCPS_SAMPLE_SIZE_BYTES: &str = "dds_dcps_sample_size_bytes";

// Discovery (§2.4)
/// Discovery domain — known participants (gauge).
pub const DDS_DISCOVERY_PARTICIPANTS_KNOWN: &str = "dds_discovery_participants_known";
/// Discovery domain — known endpoints (gauge).
pub const DDS_DISCOVERY_ENDPOINTS_KNOWN: &str = "dds_discovery_endpoints_known";
/// Discovery domain — SPDP announcements sent.
pub const DDS_DISCOVERY_SPDP_ANNOUNCEMENTS_SENT_TOTAL: &str =
    "dds_discovery_spdp_announcements_sent_total";
/// Discovery domain — SEDP updates.
pub const DDS_DISCOVERY_SEDP_UPDATES_TOTAL: &str = "dds_discovery_sedp_updates_total";
/// Discovery domain — TypeLookup requests.
pub const DDS_DISCOVERY_TYPE_LOOKUPS_TOTAL: &str = "dds_discovery_type_lookups_total";

// Security (§2.5)
/// Security domain — authentication attempts.
pub const DDS_SECURITY_AUTH_ATTEMPTS_TOTAL: &str = "dds_security_auth_attempts_total";
/// Security domain — access-control denials.
pub const DDS_SECURITY_ACCESS_DENIED_TOTAL: &str = "dds_security_access_denied_total";
/// Security domain — crypto operations.
pub const DDS_SECURITY_CRYPTO_OPERATIONS_TOTAL: &str = "dds_security_crypto_operations_total";
/// Security domain — crypto latency (histogram, seconds).
pub const DDS_SECURITY_CRYPTO_LATENCY_SECONDS: &str = "dds_security_crypto_latency_seconds";

/// List of all 31 spec metric names — for coverage tests.
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
