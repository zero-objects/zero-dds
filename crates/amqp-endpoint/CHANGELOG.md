# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initial release materialization of the `zerodds-amqp-endpoint` crate.

### Spec references

- **OMG DDS-AMQP-1.0** (formal/2024-08-01): §2.1 (endpoint profile), §6.1 (direct-embed topology), §7.3 (address resolution), §7.4 (settlement-mode mapping), §7.6.1 (group-id), §7.7.2 (inbound operation signals), §7.7.3 (disposition mapping), §8.1 (body-encoding modes), §9.1-§9.2 (Annex-A IDL + XML config loader), §10.2 (SASL), §11.2-§11.3 (errors), Annex A (configuration schema).
- **OASIS AMQP 1.0**: §2.4 (connection state), §2.5 (session state), §2.6 (link lifecycle), §3.4 (disposition states), §3.5.3 (terminus durability).

### Public API

**SASL (`sasl` module):**
- `SaslState`, `SaslMechanism::{Plain, Anonymous, External}`, `SaslCode`, `SaslOutcome`.

**Session/Connection (`session` module):**
- `ConnectionState::{Start, HdrRcvd, HdrExch, OpenRcvd, Opened, CloseRcvd, CloseSent, End}`.
- `SessionState::{Unmapped, BeginRcvd, Mapped, EndRcvd, EndSent, Discarded}`.
- `EndpointConfig`, `EndpointError`, `InboundFrameKind`.
- `advance_connection(state, frame) -> Result<ConnectionState, EndpointError>`.

**Link (`link` module):**
- `LinkRole::{Sender, Receiver}`, `SettlementMode::{Settled, Unsettled}`.
- `TerminusDurability::{None, Configuration, UnsettledState}` + `from_wire`.
- `AttachDurabilityCheck::{Accept, RejectNotImplemented}` + `check_attach_durability` (§7.4.2).
- `LinkSession::{new, grant_credit, deliver, settle, settle_with_mapper}`.
- `DeliverError`.

**DDS-Bridge (`dds_bridge` module):**
- `DispositionState::{Accepted, Rejected, Released, Modified}` (§3.4).
- `DispositionMapper`-Trait (§7.7.3) + `NoopDispositionMapper`.
- `DdsOperationDispatcher`-Trait (§7.7.2) + `AcceptAllDispatcher` + `InstanceTrackingDispatcher`.
- `InboundOperation`, `DispatchOutcome::{Accepted, UnknownInstance, RegisterMissingKey, UnknownOperation}` + `to_amqp_error`.

**Routing (`routing` module):**
- `AddressRouter`, `AddressResolution`, `ResolutionError`, `effective_partitions`.

**Mapping (`mapping` module):**
- `BodyEncodingMode::{PassThrough, Json, AmqpNative}`, `MappingError`.
- `encode_dds_to_amqp_body`, `parse_amqp_body`.

**Properties (`properties` module):**
- `DdsOperation::{Write, Register, Unregister, Dispose}`, `ProducedProperties`, `SampleHeader`, `TypeIdCheck`.
- `inspect_dds_type_id`, `message_id`, `produce_application_properties`, `produce_properties`.

**Errors (`errors` module):**
- `AmqpError`, `AmqpErrorCondition`, `ErrorScope`, `ErrorDescription`.
- Helpers: `access_denied`, `instance_unknown`, `map_mapping_error`, `map_resolution_error`, `register_missing_key`, `resource_limit_exceeded`, `unknown_dds_operation`, `unsettled_state_not_implemented`.

**Limits + Keyhash (`limits` / `keyhash` modules):**
- `ResourceLimits` (max-connections / max-frame-size / idle-timeout).
- `keyhash::*` — SHA-256 group-id hashing for §7.6.1.

**Management (`management` module):**
- `AddressKind`, `CatalogEntry`, `CatalogProducer`, `CatalogTypeId`, `CatalogDirection`.
- `AuditEvent`, `AuditProducer`, `audit_event_sample`.
- `addresses`, `classify_address`, `metrics_snapshot`.

**Metrics (`metrics` module):**
- `MetricsHub`, `MANDATORY_METRIC_NAMES`.

**Security (`security` module):**
- `AccessControlPlugin`-Trait, `AccessOp`, `AccessDecision`, `AllowAll`, `StaticAllowList`.
- `IdentityToken`, `DualIdentity`, `SaslSubject`, `build_identity_token`.
- `GovernanceDocument`, `GovernanceRule`, `LinkGovernance`, `DataProtectionKind`, `class_ids`.

**Coexistence (`coexistence` module):**
- `BridgeId`, `CoexistenceConfig`, `InboundDecision`.
- `inspect_inbound`, `stamp_outbound`, `DEFAULT_HOP_CAP = 8`, `MAX_HOP_CAP = 64`.

**RPC-Correlation (`rpc_correlation` module):**
- `OutstandingCalls`, `RpcConfig`, `IssueDecision`, `ReplyDecision`, `ReplyProperties`.
- `DEFAULT_MAX_OUTSTANDING_CALLS`, `DEFAULT_RPC_TIMEOUT_MS`.

**Annex-A (`annex_a` + `codegen_helpers` + `config_xml` modules):**
- IDL mirror of the `module zerodds::amqp` from DDS-AMQP-1.0 Annex A.
- Codegen helpers for the Annex-A IDL-to-Rust mapping.
- XML config loader (`config_xml`, feature `std`) per §9.2.

### Implementation

`session::advance_connection` is a pure state machine over the OASIS-AMQP-1.0 §2.4 diagram: Start → HdrRcvd → HdrExch → OpenRcvd → Opened → CloseRcvd → End. Within `Opened` all performatives (Begin/Attach/Flow/Transfer/Disposition/Detach/End) are allowed; the state remains `Opened` until Close arrives. Invalid transitions return `IllegalStateTransition`.

`link::LinkSession::settle_with_mapper` is the spec §7.7.3-conformant wire-up path for DDS-AMQP endpoints with a DDS bridge: on receiving an AMQP disposition frame, the caller's `DispositionMapper` is invoked with the decoded `sample_handle` and [`DispositionState`], then the pending counter is decremented. The old `settle()` method remains for AMQP-only workflows (counter decrement only, no DDS-side state update).

`link::check_attach_durability` (§7.4.2) rejects `terminus.durable=unsettled-state` with `amqp:not-implemented` — broker-level message durability is explicitly out of scope of the DDS-AMQP spec.

`dds_bridge::InstanceTrackingDispatcher` implements §11.3 instance-lifecycle failures: register WITHOUT a body key returns `RegisterMissingKey` (→ `amqp:decode-error`), unregister/dispose on an unknown instance returns `UnknownInstance` (→ `amqp:precondition-failed`).

`coexistence::inspect_inbound`, together with its `stamp_outbound` counterpart, enforces a hop cap (default 8, max 64) against multi-bridge loops — `Reject` if the bridge-id list exceeds the cap, otherwise `Accept`.

`rpc_correlation::OutstandingCalls` holds the outstanding DDS-RPC requests with a timeout (default 30s) + max-outstanding cap (default 256) — `IssueDecision::ReplyTimeout` is emitted when a reply for an expired request arrives.

`#![forbid(unsafe_code)]` is set. `extern crate alloc;`. SHA-256 via `sha2` (workspace dep). XML loader via `roxmltree` (feature `std`-only).

### Architecture

- **Layer:** 5 (Bridges, Tier-B).
- **Dependencies (in):** `zerodds-amqp-bridge` (wire codec), `sha2` (group-id hashing), `roxmltree` (config loader, feature `std`).
- **Dependents (out):** `tools/amqp-dds-endpoint` (daemon with TCP/TLS listener).
- **Feature flags:** `std` (default, enables the XML loader + `std::error::Error` impls), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format: fixed by DDS-AMQP-1.0 + OASIS AMQP-1.0.
- `LinkSession` methods: `settle()` and `settle_with_mapper()` are both stable; callers choose depending on workflow.
- Error discriminants: stable; new discriminants are major-additive.

### Resolved findings (against pre-review)

- **F-AMQP-EP-DISPOSITION-MAPPER-WIRED:** the `DispositionMapper` trait was referenced TEST-ONLY before review (no production caller in the workspace); it is now wired in productively through `LinkSession::settle_with_mapper`. Two new tests (`settle_with_mapper_calls_apply_and_decrements_pending` + `settle_with_mapper_underflow_safe_at_zero`) prove the wire-up.

### Added — daemon wire-up

- Cross-cutting daemon runtime: the `daemon` feature enables
  Prometheus metrics (§8.2), the catalog/healthz/metrics admin endpoint
  (§5.2), a signal watcher for graceful shutdown (§9.2), and the
  OTLP span exporter (§8.3).
- Bridge security: TLS client connector (rustls 0.23 ClientConnection)
  + SASL/Bearer + topic ACL via `zerodds-bridge-security`
  (Bridge-Spec §7.1/§7.2/§7.3).
- Backoff for broker reconnect with exponential backoff +
  cross-vendor interop module.
- DDS-QoS → AMQP-behavior translation in `qos_translation` (Spec §6).
