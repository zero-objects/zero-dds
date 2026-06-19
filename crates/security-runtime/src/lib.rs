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
//! ## Public API (as of 1.0.0-rc.1)
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
#[cfg(feature = "std")]
pub mod profile;
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
#[cfg(feature = "std")]
pub use profile::{SecurityProfile, SecurityProfileConfig, SecurityProfileError, strip_file_url};
pub use shared::{InboundVerdict, PeerKey, SharedSecurityGate};

// Re-exports from zerodds-security for downstream crates that only
// depend on `zerodds-security-runtime` (above all `dcps` for the security
// logger integration).
pub use zerodds_security::logging::{LogLevel, LoggingPlugin};
