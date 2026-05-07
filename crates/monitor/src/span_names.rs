// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Standard-Span-Namen + Attr-Keys (Spec §5).

/// Span: User-Code → DataWriter::write.
pub const SPAN_DDS_PUBLISH: &str = "dds.publish";
/// Span: Sample serialisieren (CDR/PL_CDR1/PL_CDR2).
pub const SPAN_DDS_SAMPLE_SERIALIZE: &str = "dds.sample.serialize";
/// Span: Sample uebermitteln (Transport-Layer).
pub const SPAN_DDS_SAMPLE_TRANSMIT: &str = "dds.sample.transmit";
/// Span: Sample empfangen (Reader-Side, ggf. mit follows-from).
pub const SPAN_DDS_SAMPLE_RECEIVE: &str = "dds.sample.receive";
/// Span: Sample deserialisieren.
pub const SPAN_DDS_SAMPLE_DESERIALIZE: &str = "dds.sample.deserialize";
/// Span: Sample an User-Reader liefern.
pub const SPAN_DDS_SAMPLE_DELIVER: &str = "dds.sample.deliver";
/// Span: RTPS-Reliable-NACK-Cycle.
pub const SPAN_DDS_RTPS_RELIABLE_NACK: &str = "dds.rtps.reliable.nack";
/// Span: Discovery-Match-Computation.
pub const SPAN_DDS_DISCOVERY_MATCH: &str = "dds.discovery.match";
/// Span: Security-Authenticate-Handshake.
pub const SPAN_DDS_SECURITY_AUTHENTICATE: &str = "dds.security.authenticate";

/// Liste aller 9 Spec-Span-Namen.
pub const ALL: &[&str] = &[
    SPAN_DDS_PUBLISH,
    SPAN_DDS_SAMPLE_SERIALIZE,
    SPAN_DDS_SAMPLE_TRANSMIT,
    SPAN_DDS_SAMPLE_RECEIVE,
    SPAN_DDS_SAMPLE_DESERIALIZE,
    SPAN_DDS_SAMPLE_DELIVER,
    SPAN_DDS_RTPS_RELIABLE_NACK,
    SPAN_DDS_DISCOVERY_MATCH,
    SPAN_DDS_SECURITY_AUTHENTICATE,
];

/// Attribut-Keys (DDS-Namespace).
pub mod attr {
    /// Topic-Name.
    pub const DDS_TOPIC: &str = "dds.topic";
    /// Writer-GUID (Hex).
    pub const DDS_WRITER_GUID: &str = "dds.writer_guid";
    /// Reader-GUID (Hex).
    pub const DDS_READER_GUID: &str = "dds.reader_guid";
    /// Source-GUID (Receiver-Side).
    pub const DDS_SOURCE_GUID: &str = "dds.source_guid";
    /// Sample-Groesse in Bytes.
    pub const DDS_SAMPLE_SIZE: &str = "dds.sample_size";
    /// Wire-Repraesentation (CDR / PL_CDR1 / PL_CDR2 / XCDR2).
    pub const DDS_REPRESENTATION: &str = "dds.representation";
    /// Transport (udp / tcp / shm / uds).
    pub const DDS_TRANSPORT: &str = "dds.transport";
    /// Destination-Locator.
    pub const DDS_DESTINATION: &str = "dds.destination";
    /// Anzahl Fragmente.
    pub const DDS_FRAGMENTS: &str = "dds.fragments";
    /// QoS-Reliability-Kind.
    pub const DDS_QOS_RELIABILITY: &str = "dds.qos.reliability";
    /// Local-Entity (Discovery-Match).
    pub const DDS_LOCAL_ENTITY: &str = "dds.local_entity";
    /// Remote-Entity (Discovery-Match).
    pub const DDS_REMOTE_ENTITY: &str = "dds.remote_entity";
    /// Compatibility-Flag (Discovery-Match).
    pub const DDS_IS_COMPATIBLE: &str = "dds.is_compatible";
    /// Identity-CA-Subject (Security-Authenticate).
    pub const DDS_IDENTITY_CA: &str = "dds.identity_ca";
    /// Auth-Result (success/failure).
    pub const DDS_RESULT: &str = "dds.result";
    /// Anzahl Missing-Sequence-Numbers (Reliable-NACK).
    pub const DDS_MISSING_SN_COUNT: &str = "dds.missing_sn_count";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_count_matches_spec() {
        assert_eq!(ALL.len(), 9);
    }

    #[test]
    fn all_have_dds_prefix() {
        for n in ALL {
            assert!(n.starts_with("dds."), "missing dds. prefix: {n}");
        }
    }
}
