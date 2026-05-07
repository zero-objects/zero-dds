// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-security-runtime`. Safety classification: **SAFE** (reiner Adapter ohne eigene Crypto-Primitiven — delegiert an `security-crypto` + `security-rtps`).
//!
//! Security-Runtime: Governance-driven Plugin-Lifecycle, Peer-Capabilities-Cache,
//! Outbound-/Inbound-Verdict-Engine, Built-in DataTagging, Anti-Squatter,
//! Heterogeneous-Mesh-Gateway-Bridge. Adapter-Schicht zwischen Governance-XML-Policy
//! und dem Secure-Submessage-Wrapper.
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services. Konsumiert `zerodds-security` (SPI) +
//! `zerodds-security-crypto` + `-permissions` + `-pki` + `-rtps` +
//! `zerodds-rtps` + `zerodds-qos`. Wird vom DCPS-Runtime via
//! `Box<dyn ...>`-Plugins gefuettert (Feature `security`).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`SecurityGate`] — High-Level-Adapter zwischen Governance + Crypto + RTPS-Wrap.
//! - `engine::*` — `GovernancePolicyEngine`-Default-Impl + `PolicyEngine`-Trait.
//! - `policy::*` — `PolicyDecision` mit Suite, Receiver-MACs, Topic-Class.
//! - `caps::*` — `PeerCapabilities` + `PeerCapabilitiesCache`.
//! - `caps_wire::*` — SPDP-Mapping fuer Peer-Capabilities (Wire-Codec).
//! - `peer_class::*` — `<peer_class>`-Match (CIDR, Subject-Patterns).
//! - `endpoint::*` — Endpoint-Slot-Lookup.
//! - `data_tagging::*` — Built-in DataTaggingPlugin (Spec §8.7).
//! - `builtin_topics::*` — DCPSParticipantStatelessMessage + DCPSParticipantVolatileMessageSecure.
//! - `anti_squatter::*` — Spec §8.5.3 Anti-Squatter-Logik.
//! - `gateway_bridge::*` — Heterogeneous-Mesh-Gateway-Bridge (Edge ↔ Backend).
//! - `shared::*` — Shared-Inbound/Outbound-Verdict-Types.
//!
//! # Beispiel
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
//! // Inbound (am Peer):
//! let plain = gate.decode_inbound("Chatter", &wire).unwrap();
//! # const GOVERNANCE_XML: &str = "";
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod anti_squatter;
pub mod builtin_topics;
pub mod caps;
pub mod caps_wire;
pub mod data_tagging;
pub mod endpoint;
mod engine;
mod gate;
pub mod gateway_bridge;
pub mod peer_class;
pub mod policy;
mod shared;

pub use anti_squatter::{BindingDecision, GuidPrefixBytes, IdentityBindingCache};
pub use caps::{PeerCache, PeerCapabilities, Validity};
pub use caps_wire::{advertise_security_caps, parse_peer_caps};
pub use data_tagging::{BuiltinDataTaggingPlugin, TAG_PROPERTY_PREFIX};
pub use endpoint::{EndpointMatch, EndpointProtection, MatchRejectReason, match_endpoints};
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
pub use shared::{InboundVerdict, PeerKey, SharedSecurityGate};

// Re-exports aus zerodds-security fuer Downstream-Crates, die nur
// `zerodds-security-runtime` depen (vor allem `dcps` fuer die Security-
// Logger-Integration).
pub use zerodds_security::logging::{LogLevel, LoggingPlugin};
