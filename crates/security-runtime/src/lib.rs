// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-runtime`. Safety classification: **SAFE** (pure adapter without its own crypto primitives — delegates to `security-crypto` + `security-rtps`).
//!
//! Security runtime: governance-driven plugin lifecycle, peer-capabilities cache,
//! outbound/inbound verdict engine, built-in data tagging, anti-squatter,
//! heterogeneous-mesh gateway bridge. Adapter layer between the governance-XML policy
//! and the secure-submessage wrapper.
//!
//! ## Layer position
//!
//! Layer 4 — core services. Consumes `zerodds-security` (SPI) +
//! `zerodds-security-crypto` + `-permissions` + `-pki` + `-rtps` +
//! `zerodds-rtps` + `zerodds-qos`. Fed by the DCPS runtime via
//! `Box<dyn ...>` plugins (feature `security`).
//!
//! ## Public API
//!
//! - [`SecurityGate`] — high-level adapter between governance + crypto + RTPS wrap.
//! - `engine::*` — `GovernancePolicyEngine` default impl + `PolicyEngine` trait.
//! - `policy::*` — `PolicyDecision` with suite, receiver MACs, topic class.
//! - `caps::*` — `PeerCapabilities` + `PeerCapabilitiesCache`.
//! - `caps_wire::*` — SPDP mapping for peer capabilities (wire codec).
//! - `peer_class::*` — `<peer_class>` match (CIDR, subject patterns).
//! - `endpoint::*` — endpoint slot lookup.
//! - `data_tagging::*` — built-in DataTaggingPlugin (spec §8.7).
//! - `builtin_topics::*` — DCPSParticipantStatelessMessage + DCPSParticipantVolatileMessageSecure.
//! - `anti_squatter::*` — spec §8.5.3 anti-squatter logic.
//! - `gateway_bridge::*` — heterogeneous-mesh gateway bridge (edge ↔ backend).
//! - `shared::*` — shared inbound/outbound verdict types.
//!
//! # Example
//!
//! ```no_run
//! use zerodds_security_crypto::AesGcmCryptoPlugin;
//! use zerodds_security_permissions::parse_governance_xml;
//! use zerodds_security_runtime::SecurityGate;
//!
//! let governance = parse_governance_xml(GOVERNANCE_XML).unwrap();
//! let mut crypto = AesGcmCryptoPlugin::new();
//! let mut gate = SecurityGate::new(0, governance, &mut crypto);
//!
//! // Outbound:
//! let wire = gate.encode_outbound("Chatter", b"hello").unwrap();
//!
//! // Inbound (at the peer):
//! let plain = gate.decode_inbound("Chatter", &wire).unwrap();
//! # const GOVERNANCE_XML: &str = "";
//! ```

// Needs `std`: built on security-crypto, security-pki and security-permissions, which
// need it.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod anti_squatter;
pub mod builtin_attributes;
pub mod builtin_topics;
#[cfg(feature = "std")]
pub mod bundle;
pub mod caps;
pub mod caps_wire;
pub mod data_tagging;
pub mod endpoint;
#[cfg(feature = "std")]
pub mod endpoint_access;
mod engine;
mod gate;
mod gate_crypto;
pub mod gateway_bridge;
pub mod peer_class;
#[cfg(feature = "std")]
pub mod pkcs11;
pub mod plugin_attributes;
pub mod policy;
#[cfg(feature = "std")]
pub mod profile;
mod psk_gate;
#[cfg(feature = "std")]
pub mod qos_properties;
mod shared;

pub use anti_squatter::{BindingDecision, GuidPrefixBytes, IdentityBindingCache};
#[cfg(feature = "std")]
pub use bundle::{SecurityBundle, SecurityBundleBuilder};
pub use caps::{PeerCache, PeerCapabilities, Validity};
pub use caps_wire::{advertise_security_caps, parse_peer_caps};
pub use data_tagging::{BuiltinDataTaggingPlugin, DATA_TAGGING_PLUGIN_CLASS_ID};
pub use endpoint::{EndpointMatch, EndpointProtection, MatchRejectReason, match_endpoints};
#[cfg(feature = "std")]
pub use endpoint_access::arm_endpoint_access_control;
pub use engine::GovernancePolicyEngine;
pub use gate::{SecurityGate, SecurityGateError};
pub use gateway_bridge::{
    GatewayBridge, GatewayBridgeConfig, GatewayBridgeError, GatewayBridgeResult,
};
pub use peer_class::{
    interface_accepts_class, peer_matches_class, resolve_peer_class, resolve_protection,
};
pub use policy::{
    InboundCtx, InterfaceConfig, IpRange, NetInterface, OutboundCtx, PolicyDecision, PolicyEngine,
    ProtectionLevel, SuiteHint, classify_interface,
};
#[cfg(feature = "std")]
pub use profile::{
    Material, SecurityProfile, SecurityProfileConfig, SecurityProfileError, builtin_crypto_plugin,
    gate_with_crypto, governance_from_files, strip_file_url,
};
pub use zerodds_security::{AlgoNameNul, SubjectNameForm};
/// The parsed governance document.
pub use zerodds_security_permissions::Governance;
/// How the partitions of a permissions allow rule match
/// (`SecurityProfileConfig::partition_matching`).
pub use zerodds_security_permissions::PartitionMatching;
// `pkcs11:` URIs (RFC 7512) for the DDS-Security 1.2 §9.3.1 Table 44
// identity material: the parser is always available; the token access
// (reading a certificate, signing with a token key) needs the `pkcs11`
// feature.
#[cfg(all(feature = "std", feature = "pkcs11"))]
pub use pkcs11::Pkcs11Signer;
#[cfg(feature = "std")]
pub use pkcs11::{
    Pkcs11Error, Pkcs11ObjectType, Pkcs11Pin, Pkcs11Uri, Pkcs11UriError, read_certificate_der,
};
// DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 — the participant's `PropertyQosPolicy` is
// how the plugins are selected and configured. `dcps` reads the list through
// these on every participant it creates.
#[cfg(feature = "std")]
pub use qos_properties::{
    PROP_GOVERNANCE, PROP_IDENTITY_CA, PROP_IDENTITY_CERT, PROP_IDENTITY_KEY,
    PROP_IDENTITY_KEY_PASSWORD, PROP_KEY_ESTABLISHMENT, PROP_PERMISSIONS, PROP_PERMISSIONS_CA,
    SecurityPropertyError, cert_policy_from_properties, kx_suite_from_properties,
    profile_config_from_properties, requests_security, resolve_file_uri, resolve_material,
    security_bundle_from_enclave_dir, security_bundle_from_enclave_dir_with_env,
    security_bundle_from_properties, security_bundle_from_properties_with_env,
    vendor_form_settings_from_env,
};
// The identity-certificate validation policy and the X.509 v1 deviation it
// switches on. Re-exported here so that a caller configuring a
// `SecurityProfile` does not have to reach past this crate for the one type
// `from_files_with_cert_policy` takes.
pub use shared::{
    BUILTIN_VOLATILE_MESSAGE_READER, BUILTIN_VOLATILE_MESSAGE_WRITER, EndpointAccess,
    InboundVerdict, PARTICIPANT_WIDE_TOPIC, PROP_BUILTIN_ENDPOINT_NAME, PeerKey, RemotePermissions,
    RevokedPermissions, SharedSecurityGate, SignedDocuments,
};
pub use zerodds_security_pki::subject_attributes;
pub use zerodds_security_pki::{
    AlgorithmNames, CertAcceptance, CertPolicy, KxSuite, LOG_CATEGORY_X509V1_DEVIATION,
    PROP_ACCEPT_X509V1_IDENTITY, SPEC_CLAUSE_IDENTITY_CERTIFICATE, property_enables_deviation,
};

// Re-exports from zerodds-security for downstream crates that only
// depend on `zerodds-security-runtime` (above all `dcps` for the security
// logger integration).
pub use zerodds_security::logging::{LogLevel, LoggingPlugin};
