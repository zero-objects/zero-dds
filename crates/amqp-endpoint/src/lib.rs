// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-amqp-endpoint`. Safety classification: **STANDARD**.
//!
//! DDS-AMQP 1.0 bidirectional endpoint stack — pure-Rust
//! `no_std + alloc` (with an optional `std` loader layer),
//! `forbid(unsafe_code)`. Implements the protocol layers
//! above the wire codec (`zerodds-amqp-bridge`):
//!
//! Spec: OMG DDS-AMQP-1.0 (formal/2024-08-01) §2.1 (endpoint profile),
//! §6.1 (direct-embed topology), §7 (mapping), §11 (errors), Annex A
//! (configuration schema). OASIS AMQP-1.0 §2.4 (connection state),
//! §2.5 (session state), §2.6 (link lifecycle).
//!
//! ## Layer position
//!
//! Layer 5 — bridges. Sits on
//! [`zerodds-amqp-bridge`](../zerodds_amqp_bridge/index.html) (wire
//! codec). The TCP/TLS listener itself lives in the daemon crate
//! `tools/amqp-dds-endpoint/`; this crate provides the
//! wire-independent protocol layers.
//!
//! ## Public API (as of 1.0.0-rc.1)
//!
//! - [`sasl`] — SASL frame layer (PLAIN / ANONYMOUS / EXTERNAL)
//!   per spec §10.2.
//! - [`session`] — connection/session state machine + idle timeout +
//!   DoS caps per spec §6.1.
//! - [`link`] — sender/receiver link acceptance + settlement
//!   tracking + disposition-mapper wireup (§7.4 + §7.7.3).
//! - [`routing`] — address resolution + wildcard mapping per
//!   spec §7.3.
//! - [`mapping`] — body-encoding-mode mapping (pass-through / JSON /
//!   AMQP-native) per spec §8.1.
//! - [`properties`] — application-properties codec with
//!   `dds:operation` / `dds:instance-handle` / `dds:type-id`.
//! - [`dds_bridge`] — trait surfaces ([`DdsOperationDispatcher`] +
//!   [`DispositionMapper`]) for caller DCPS bridges.
//! - [`management`] — catalog + audit producers + metrics snapshots.
//! - [`metrics`] — mandatory metric hub.
//! - [`security`] — access-control plugin surface + governance.
//! - [`coexistence`] — multi-bridge hop cap + inbound decision.
//! - [`rpc_correlation`] — outstanding calls + reply routing.
//! - [`errors`] — §11 spec error conditions as typed errors.
//! - [`limits`] — `ResourceLimits` data model from Annex A.
//! - [`keyhash`] — SHA-256 group-id hashing for §7.6.1.
//! - [`annex_a`] — IDL mirror of the Annex-A configuration schema.
//! - [`codegen_helpers`] — helpers for the Annex-A IDL codegen.
//! - [`config_xml`] (feature `std`) — XML configuration loader (§9.2).
//!
//! ## What is caller-layer
//!
//! - **TCP listener operations** and **TLS termination** —
//!   the daemon crate (`tools/amqp-dds-endpoint/`) with `tokio` + `rustls`.
//! - **Discovery wire** — the DDS-SPDP/SEDP bridge to the DDS side
//!   lives in `crates/discovery/`.
//! - **Live-interop smoke** against pika/qpid-proton/Azure-SB —
//!   the test harness in the daemon crate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub mod annex_a;
pub mod backoff;
// Bridge-security adapter (bridge spec §7.1 TLS via rustls, §7.2 auth modes,
// §7.3 topic ACL). std-only (rustls requires std).
#[cfg(feature = "std")]
pub mod bridge_security;
// Daemon-runtime wireup (§5.2 catalog, §8.2 Prometheus, §8.3 OTLP,
// §9.2 graceful shutdown). std-only.
#[cfg(feature = "std")]
pub mod daemon_runtime;
// DDS-QoS → AMQP-behavior translation (spec §6).
#[cfg(feature = "std")]
pub mod client;
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
pub mod scram;
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
