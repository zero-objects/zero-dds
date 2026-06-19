// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex-A IDL configuration schema (normative).
//!
//! Spec source: DDS-AMQP-1.0 Annex A — `module zerodds::amqp`.
//! This module file mirrors the IDL structures one-to-one into Rust;
//! the fields and enum values adopt the spec spelling exactly
//! (cf. `MODE_PASSTHROUGH` etc.) so that the spec audit can verify
//! byte-for-byte.
//!
//! The structure is deserialized from XML via the `crate::config_xml`
//! loader (feature `std`); a codegen pipeline from the Annex-A IDL is
//! planned as a later optimization of the build pipeline.

use alloc::string::String;
use alloc::vec::Vec;

// ============================================================
// Enums (§A)
// ============================================================

/// Spec Annex A — `enum SaslMechanism`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    /// `SASL_PLAIN`
    SaslPlain,
    /// `SASL_ANONYMOUS`
    SaslAnonymous,
    /// `SASL_EXTERNAL`
    SaslExternal,
    /// `SASL_SCRAM_SHA_256`
    SaslScramSha256,
}

impl SaslMechanism {
    /// Annex-A-IDL-Symbol.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::SaslPlain => "SASL_PLAIN",
            Self::SaslAnonymous => "SASL_ANONYMOUS",
            Self::SaslExternal => "SASL_EXTERNAL",
            Self::SaslScramSha256 => "SASL_SCRAM_SHA_256",
        }
    }

    /// Inverse decode from the IDL symbol string. Also accepts the
    /// AMQP wire form without the `SASL_` prefix (`PLAIN`/`ANONYMOUS`/...).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "SASL_PLAIN" | "PLAIN" => Some(Self::SaslPlain),
            "SASL_ANONYMOUS" | "ANONYMOUS" => Some(Self::SaslAnonymous),
            "SASL_EXTERNAL" | "EXTERNAL" => Some(Self::SaslExternal),
            "SASL_SCRAM_SHA_256" | "SCRAM-SHA-256" | "SCRAM_SHA_256" => Some(Self::SaslScramSha256),
            _ => None,
        }
    }
}

/// Spec Annex A — `enum BodyEncodingMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BodyEncodingMode {
    /// `MODE_PASSTHROUGH` (Default).
    #[default]
    ModePassthrough,
    /// `MODE_JSON`.
    ModeJson,
    /// `MODE_AMQP_NATIVE`.
    ModeAmqpNative,
}

impl BodyEncodingMode {
    /// Annex-A-IDL-Symbol.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::ModePassthrough => "MODE_PASSTHROUGH",
            Self::ModeJson => "MODE_JSON",
            Self::ModeAmqpNative => "MODE_AMQP_NATIVE",
        }
    }

    /// Inverse decode.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "MODE_PASSTHROUGH" | "PASSTHROUGH" | "PASS_THROUGH" => Some(Self::ModePassthrough),
            "MODE_JSON" | "JSON" => Some(Self::ModeJson),
            "MODE_AMQP_NATIVE" | "AMQP_NATIVE" => Some(Self::ModeAmqpNative),
            _ => None,
        }
    }
}

/// Spec Annex A — `enum TimeMapping`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeMapping {
    /// `MAPPING_STANDARD` — AMQP timestamp + dds:nsec (Default).
    #[default]
    MappingStandard,
    /// `MAPPING_COMPOSITE` — dds-time described composite (opt-in).
    MappingComposite,
}

impl TimeMapping {
    /// Annex-A-IDL-Symbol.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::MappingStandard => "MAPPING_STANDARD",
            Self::MappingComposite => "MAPPING_COMPOSITE",
        }
    }

    /// Inverse decode.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "MAPPING_STANDARD" | "STANDARD" => Some(Self::MappingStandard),
            "MAPPING_COMPOSITE" | "COMPOSITE" => Some(Self::MappingComposite),
            _ => None,
        }
    }
}

/// Spec Annex A — `enum DescriptorForm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DescriptorForm {
    /// `DESC_TRUNCATED` (Default; ulong-form).
    #[default]
    DescTruncated,
    /// `DESC_FULL` (symbol-form).
    DescFull,
}

impl DescriptorForm {
    /// Annex-A-IDL-Symbol.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::DescTruncated => "DESC_TRUNCATED",
            Self::DescFull => "DESC_FULL",
        }
    }

    /// Inverse decode.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "DESC_TRUNCATED" | "TRUNCATED" => Some(Self::DescTruncated),
            "DESC_FULL" | "FULL" => Some(Self::DescFull),
            _ => None,
        }
    }
}

/// Spec Annex A — `enum LinkDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkDirection {
    /// `DIR_PRODUCER_TO_DDS`.
    DirProducerToDds,
    /// `DIR_DDS_TO_CONSUMER`.
    DirDdsToConsumer,
    /// `DIR_BOTH` (Default).
    #[default]
    DirBoth,
}

impl LinkDirection {
    /// Annex-A-IDL-Symbol.
    #[must_use]
    pub const fn as_idl(self) -> &'static str {
        match self {
            Self::DirProducerToDds => "DIR_PRODUCER_TO_DDS",
            Self::DirDdsToConsumer => "DIR_DDS_TO_CONSUMER",
            Self::DirBoth => "DIR_BOTH",
        }
    }

    /// Inverse decode.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "DIR_PRODUCER_TO_DDS" | "PRODUCER_TO_DDS" | "producer-to-dds" => {
                Some(Self::DirProducerToDds)
            }
            "DIR_DDS_TO_CONSUMER" | "DDS_TO_CONSUMER" | "dds-to-consumer" => {
                Some(Self::DirDdsToConsumer)
            }
            "DIR_BOTH" | "BOTH" | "both" => Some(Self::DirBoth),
            _ => None,
        }
    }
}

// ============================================================
// Structs (§A)
// ============================================================

/// Spec Annex A — `struct TopicMapping`.
#[derive(Debug, Clone)]
pub struct TopicMapping {
    /// `string<256> amqp_address`.
    pub amqp_address: String,
    /// `string<256> dds_topic`.
    pub dds_topic: String,
    /// `string<256> dds_type_name`.
    pub dds_type_name: String,
    /// `uint32 dds_domain_id` (default 0).
    pub dds_domain_id: u32,
    /// `sequence<string<128>, 16> dds_partition` (default empty).
    pub dds_partition: Vec<String>,
    /// `BodyEncodingMode mode` (default `MODE_PASSTHROUGH`).
    pub mode: BodyEncodingMode,
    /// `TimeMapping time_mapping` (default `MAPPING_STANDARD`).
    pub time_mapping: TimeMapping,
    /// `DescriptorForm descriptor_form` (default `DESC_TRUNCATED`).
    pub descriptor_form: DescriptorForm,
    /// `boolean rpc_aware` (default false; see Annex D).
    pub rpc_aware: bool,
    /// `uint32 rpc_timeout_ms` (default 30000; only when rpc_aware=true).
    pub rpc_timeout_ms: u32,
    /// `LinkDirection direction` (default `DIR_BOTH`).
    pub direction: LinkDirection,
}

impl Default for TopicMapping {
    fn default() -> Self {
        Self {
            amqp_address: String::new(),
            dds_topic: String::new(),
            dds_type_name: String::new(),
            dds_domain_id: 0,
            dds_partition: Vec::new(),
            mode: BodyEncodingMode::default(),
            time_mapping: TimeMapping::default(),
            descriptor_form: DescriptorForm::default(),
            rpc_aware: false,
            rpc_timeout_ms: 30_000,
            direction: LinkDirection::default(),
        }
    }
}

/// Spec Annex A — `struct TlsConfig`.
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// `boolean enabled`.
    pub enabled: bool,
    /// `string<512> cert_path`.
    pub cert_path: String,
    /// `string<512> key_path`.
    pub key_path: String,
    /// `string<512> ca_path`.
    pub ca_path: String,
    /// `boolean require_client_cert`.
    pub require_client_cert: bool,
}

/// Spec Annex A — `struct SaslConfig`.
#[derive(Debug, Clone, Default)]
pub struct SaslConfig {
    /// `sequence<SaslMechanism, 8> enabled_mechanisms`.
    pub enabled_mechanisms: Vec<SaslMechanism>,
    /// `string<256> credential_store_uri`.
    pub credential_store_uri: String,
}

/// Spec Annex A — `struct ResourceLimits`.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// `uint32 max_connections`.
    pub max_connections: u32,
    /// `uint32 max_sessions_per_connection`.
    pub max_sessions_per_connection: u32,
    /// `uint32 max_links_per_session`.
    pub max_links_per_session: u32,
    /// `uint32 max_frame_size`.
    pub max_frame_size: u32,
    /// `uint64 idle_timeout_ms`.
    pub idle_timeout_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        // Spec §7.10 + DoS caps from `crate::limits::ResourceLimits`.
        Self {
            max_connections: 1024,
            max_sessions_per_connection: 8,
            max_links_per_session: 16,
            max_frame_size: 1_048_576, // 1 MiB
            idle_timeout_ms: 60_000,
        }
    }
}

/// Spec Annex A — `struct DynamicTopicConfig`.
#[derive(Debug, Clone, Default)]
pub struct DynamicTopicConfig {
    /// `boolean permit_dynamic_topics`.
    pub permit_dynamic_topics: bool,
    /// `string<256> dynamic_topic_default_type`.
    pub dynamic_topic_default_type: String,
    /// `BodyEncodingMode default_mode`.
    pub default_mode: BodyEncodingMode,
}

/// Spec Annex A — `struct AmqpEndpointConfig`.
#[derive(Debug, Clone, Default)]
pub struct AmqpEndpointConfig {
    /// `@key string<128> endpoint_name`.
    pub endpoint_name: String,
    /// `string<256> listen_uri`.
    pub listen_uri: String,
    /// `TlsConfig tls`.
    pub tls: TlsConfig,
    /// `SaslConfig sasl`.
    pub sasl: SaslConfig,
    /// `sequence<TopicMapping> topics`.
    pub topics: Vec<TopicMapping>,
    /// `DynamicTopicConfig dynamic`.
    pub dynamic: DynamicTopicConfig,
    /// `ResourceLimits limits`.
    pub limits: ResourceLimits,
    /// `string<36> bridge_id` (RFC-4122 UUID, empty => generate at startup).
    pub bridge_id: String,
    /// `uint8 bridge_hop_cap` (default 8).
    pub bridge_hop_cap: u8,
}

/// Spec Annex A — `struct AmqpBridgeConfig`.
#[derive(Debug, Clone, Default)]
pub struct AmqpBridgeConfig {
    /// `@key string<128> bridge_name`.
    pub bridge_name: String,
    /// `string<256> upstream_uri`.
    pub upstream_uri: String,
    /// `TlsConfig tls`.
    pub tls: TlsConfig,
    /// `SaslConfig sasl`.
    pub sasl: SaslConfig,
    /// `sequence<TopicMapping> topics`.
    pub topics: Vec<TopicMapping>,
    /// `string<36> bridge_id`.
    pub bridge_id: String,
    /// `uint8 bridge_hop_cap` (default 8).
    pub bridge_hop_cap: u8,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sasl_mechanism_idl_round_trip() {
        for m in [
            SaslMechanism::SaslPlain,
            SaslMechanism::SaslAnonymous,
            SaslMechanism::SaslExternal,
            SaslMechanism::SaslScramSha256,
        ] {
            assert_eq!(SaslMechanism::parse(m.as_idl()), Some(m));
        }
    }

    #[test]
    fn sasl_mechanism_accepts_amqp_wire_aliases() {
        assert_eq!(
            SaslMechanism::parse("PLAIN"),
            Some(SaslMechanism::SaslPlain)
        );
        assert_eq!(
            SaslMechanism::parse("ANONYMOUS"),
            Some(SaslMechanism::SaslAnonymous)
        );
        assert_eq!(
            SaslMechanism::parse("EXTERNAL"),
            Some(SaslMechanism::SaslExternal)
        );
        assert_eq!(
            SaslMechanism::parse("SCRAM-SHA-256"),
            Some(SaslMechanism::SaslScramSha256)
        );
    }

    #[test]
    fn body_encoding_mode_idl_round_trip() {
        for m in [
            BodyEncodingMode::ModePassthrough,
            BodyEncodingMode::ModeJson,
            BodyEncodingMode::ModeAmqpNative,
        ] {
            assert_eq!(BodyEncodingMode::parse(m.as_idl()), Some(m));
        }
        assert_eq!(
            BodyEncodingMode::default(),
            BodyEncodingMode::ModePassthrough
        );
    }

    #[test]
    fn descriptor_form_default_is_truncated() {
        assert_eq!(DescriptorForm::default(), DescriptorForm::DescTruncated);
    }

    #[test]
    fn time_mapping_default_is_standard() {
        assert_eq!(TimeMapping::default(), TimeMapping::MappingStandard);
    }

    #[test]
    fn link_direction_default_is_both() {
        assert_eq!(LinkDirection::default(), LinkDirection::DirBoth);
    }

    #[test]
    fn topic_mapping_defaults_match_spec() {
        let t = TopicMapping::default();
        assert_eq!(t.dds_domain_id, 0);
        assert_eq!(t.mode, BodyEncodingMode::ModePassthrough);
        assert_eq!(t.time_mapping, TimeMapping::MappingStandard);
        assert_eq!(t.descriptor_form, DescriptorForm::DescTruncated);
        assert!(!t.rpc_aware);
        assert_eq!(t.rpc_timeout_ms, 30_000);
        assert_eq!(t.direction, LinkDirection::DirBoth);
    }

    #[test]
    fn endpoint_config_default_is_empty() {
        let c = AmqpEndpointConfig::default();
        assert!(c.endpoint_name.is_empty());
        assert!(c.topics.is_empty());
        assert_eq!(c.bridge_hop_cap, 0);
    }

    #[test]
    fn resource_limits_default_has_dos_caps() {
        let l = ResourceLimits::default();
        assert!(l.max_connections > 0);
        assert!(l.max_frame_size >= 65_536);
        assert!(l.idle_timeout_ms > 0);
    }

    #[test]
    fn link_direction_amqp_aliases() {
        assert_eq!(
            LinkDirection::parse("producer-to-dds"),
            Some(LinkDirection::DirProducerToDds)
        );
        assert_eq!(
            LinkDirection::parse("dds-to-consumer"),
            Some(LinkDirection::DirDdsToConsumer)
        );
        assert_eq!(LinkDirection::parse("both"), Some(LinkDirection::DirBoth));
    }
}
