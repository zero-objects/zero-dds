// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-amqp-endpoint`. Safety classification: **STANDARD**.
//!
//! DDS-AMQP 1.0 bidirektionaler Endpoint-Stack — pure-Rust
//! `no_std + alloc` (mit optionaler `std`-Loader-Schicht),
//! `forbid(unsafe_code)`. Implementiert die Protokoll-Schichten
//! oberhalb des Wire-Codecs (`zerodds-amqp-bridge`):
//!
//! Spec: OMG DDS-AMQP-1.0 (formal/2024-08-01) §2.1 (Endpoint-Profile),
//! §6.1 (Direct-Embed-Topology), §7 (Mapping), §11 (Errors), Annex A
//! (Configuration-Schema). OASIS AMQP-1.0 §2.4 (Connection-State),
//! §2.5 (Session-State), §2.6 (Link-Lifecycle).
//!
//! ## Schichten-Position
//!
//! Layer 5 — Bridges. Sitzt auf
//! [`zerodds-amqp-bridge`](../zerodds_amqp_bridge/index.html) (Wire-
//! Codec). Der TCP/TLS-Listener selbst lebt im Daemon-Crate
//! `tools/amqp-dds-endpoint/`; diese Crate liefert die wire-
//! unabhaengigen Protokoll-Schichten.
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - [`sasl`] — SASL-Frame-Layer (PLAIN / ANONYMOUS / EXTERNAL)
//!   gemaess Spec §10.2.
//! - [`session`] — Connection/Session-State-Machine + Idle-Timeout +
//!   DoS-Caps gemaess Spec §6.1.
//! - [`link`] — Sender-/Receiver-Link-Acceptance + Settlement-
//!   Tracking + Disposition-Mapper-Wire-up (§7.4 + §7.7.3).
//! - [`routing`] — Address-Resolution + Wildcard-Mapping gemaess
//!   Spec §7.3.
//! - [`mapping`] — Body-Encoding-Mode-Mapping (Pass-Through / JSON /
//!   AMQP-Native) gemaess Spec §8.1.
//! - [`properties`] — Application-Properties-Codec mit
//!   `dds:operation` / `dds:instance-handle` / `dds:type-id`.
//! - [`dds_bridge`] — Trait-Surfaces ([`DdsOperationDispatcher`] +
//!   [`DispositionMapper`]) fuer Caller-DCPS-Bruecken.
//! - [`management`] — Catalog + Audit-Producers + Metrics-Snapshots.
//! - [`metrics`] — Mandatory-Metric-Hub.
//! - [`security`] — Access-Control-Plugin-Surface + Governance.
//! - [`coexistence`] — Multi-Bridge-Hop-Cap + Inbound-Decision.
//! - [`rpc_correlation`] — Outstanding-Calls + Reply-Routing.
//! - [`errors`] — Spec-§11 Error-Conditions als typisierte Errors.
//! - [`limits`] — `ResourceLimits`-Datenmodell aus Annex A.
//! - [`keyhash`] — SHA-256 group-id-Hashing fuer §7.6.1.
//! - [`annex_a`] — IDL-Spiegelung des Annex-A-Configuration-Schemas.
//! - [`codegen_helpers`] — Helpers fuer den Annex-A-IDL-Codegen.
//! - [`config_xml`] (Feature `std`) — XML-Configuration-Loader (§9.2).
//!
//! ## Was Caller-Layer ist
//!
//! - **TCP-Listener-Operations** und **TLS-Termination** —
//!   Daemon-Crate (`tools/amqp-dds-endpoint/`) mit `tokio` + `rustls`.
//! - **Discovery-Wire** — DDS-SPDP/SEDP-Bridge zur DDS-Side lebt in
//!   `crates/discovery/`.
//! - **Live-Interop-Smoke** gegen pika/qpid-proton/Azure-SB —
//!   Test-Harness im Daemon-Crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod annex_a;
pub mod backoff;
// Bridge-Security-Adapter (Bridge-Spec §7.1 TLS via rustls, §7.2 Auth-Modes,
// §7.3 Topic-ACL). std-only (rustls braucht std).
#[cfg(feature = "std")]
pub mod bridge_security;
// Daemon-Runtime-Wireup (§5.2 Catalog, §8.2 Prometheus, §8.3 OTLP,
// §9.2 Graceful Shutdown). std-only.
#[cfg(feature = "std")]
pub mod daemon_runtime;
// DDS-QoS → AMQP-Behavior-Translation (Spec §6).
pub mod codegen_helpers;
pub mod coexistence;
#[cfg(feature = "std")]
pub mod config_xml;
pub mod dds_bridge;
pub mod errors;
pub mod keyhash;
pub mod limits;
pub mod link;
pub mod management;
pub mod mapping;
pub mod metrics;
pub mod properties;
#[cfg(feature = "std")]
pub mod qos_translation;
pub mod routing;
pub mod rpc_correlation;
pub mod sasl;
pub mod security;
pub mod session;

pub use coexistence::{
    BridgeId, CoexistenceConfig, DEFAULT_HOP_CAP, InboundDecision, MAX_HOP_CAP, inspect_inbound,
    stamp_outbound,
};
pub use dds_bridge::{
    AcceptAllDispatcher, DdsOperationDispatcher, DispatchOutcome, DispositionMapper,
    DispositionState, InboundOperation, InstanceTrackingDispatcher, NoopDispositionMapper,
};
pub use errors::{
    AmqpError, AmqpErrorCondition, ErrorDescription, ErrorScope, access_denied, instance_unknown,
    map_mapping_error, map_resolution_error, register_missing_key, resource_limit_exceeded,
    unknown_dds_operation, unsettled_state_not_implemented,
};
pub use limits::ResourceLimits;
pub use link::{
    AttachDurabilityCheck, LinkRole, LinkSession, SettlementMode, TerminusDurability,
    check_attach_durability,
};
pub use management::{
    AddressKind, AuditEvent, AuditProducer, CatalogDirection, CatalogEntry, CatalogProducer,
    CatalogTypeId, addresses, audit_event_sample, classify_address, metrics_snapshot,
};
pub use mapping::{BodyEncodingMode, MappingError, encode_dds_to_amqp_body, parse_amqp_body};
pub use metrics::{MANDATORY_METRIC_NAMES, MetricsHub};
pub use properties::{
    DdsOperation, ProducedProperties, SampleHeader, TypeIdCheck, inspect_dds_type_id, message_id,
    produce_application_properties, produce_properties,
};
pub use routing::{AddressResolution, AddressRouter, ResolutionError, effective_partitions};
pub use rpc_correlation::{
    DEFAULT_MAX_OUTSTANDING_CALLS, DEFAULT_RPC_TIMEOUT_MS, IssueDecision, OutstandingCalls,
    ReplyDecision, ReplyProperties, RpcConfig,
};
pub use sasl::{SaslCode, SaslMechanism, SaslOutcome, SaslState};
pub use security::{
    AccessControlPlugin, AccessDecision, AccessOp, AllowAll, DataProtectionKind, DualIdentity,
    GovernanceDocument, GovernanceRule, IdentityToken, LinkGovernance, SaslSubject,
    StaticAllowList, build_identity_token, class_ids,
};
pub use session::{
    ConnectionState, EndpointConfig, EndpointError, SessionState, advance_connection,
};
