# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-amqp-endpoint`-Crate.

### Spec-Referenzen

- **OMG DDS-AMQP-1.0** (formal/2024-08-01): §2.1 (Endpoint-Profile), §6.1 (Direct-Embed-Topology), §7.3 (Address-Resolution), §7.4 (Settlement-Mode-Mapping), §7.6.1 (group-id), §7.7.2 (Inbound-Operation-Signals), §7.7.3 (Disposition-Mapping), §8.1 (Body-Encoding-Modes), §9.1-§9.2 (Annex-A IDL + XML-Config-Loader), §10.2 (SASL), §11.2-§11.3 (Errors), Annex A (Configuration-Schema).
- **OASIS AMQP 1.0**: §2.4 (Connection-State), §2.5 (Session-State), §2.6 (Link-Lifecycle), §3.4 (Disposition-States), §3.5.3 (Terminus-Durability).

### Public-API

**SASL (`sasl`-Modul):**
- `SaslState`, `SaslMechanism::{Plain, Anonymous, External}`, `SaslCode`, `SaslOutcome`.

**Session/Connection (`session`-Modul):**
- `ConnectionState::{Start, HdrRcvd, HdrExch, OpenRcvd, Opened, CloseRcvd, CloseSent, End}`.
- `SessionState::{Unmapped, BeginRcvd, Mapped, EndRcvd, EndSent, Discarded}`.
- `EndpointConfig`, `EndpointError`, `InboundFrameKind`.
- `advance_connection(state, frame) -> Result<ConnectionState, EndpointError>`.

**Link (`link`-Modul):**
- `LinkRole::{Sender, Receiver}`, `SettlementMode::{Settled, Unsettled}`.
- `TerminusDurability::{None, Configuration, UnsettledState}` + `from_wire`.
- `AttachDurabilityCheck::{Accept, RejectNotImplemented}` + `check_attach_durability` (§7.4.2).
- `LinkSession::{new, grant_credit, deliver, settle, settle_with_mapper}`.
- `DeliverError`.

**DDS-Bridge (`dds_bridge`-Modul):**
- `DispositionState::{Accepted, Rejected, Released, Modified}` (§3.4).
- `DispositionMapper`-Trait (§7.7.3) + `NoopDispositionMapper`.
- `DdsOperationDispatcher`-Trait (§7.7.2) + `AcceptAllDispatcher` + `InstanceTrackingDispatcher`.
- `InboundOperation`, `DispatchOutcome::{Accepted, UnknownInstance, RegisterMissingKey, UnknownOperation}` + `to_amqp_error`.

**Routing (`routing`-Modul):**
- `AddressRouter`, `AddressResolution`, `ResolutionError`, `effective_partitions`.

**Mapping (`mapping`-Modul):**
- `BodyEncodingMode::{PassThrough, Json, AmqpNative}`, `MappingError`.
- `encode_dds_to_amqp_body`, `parse_amqp_body`.

**Properties (`properties`-Modul):**
- `DdsOperation::{Write, Register, Unregister, Dispose}`, `ProducedProperties`, `SampleHeader`, `TypeIdCheck`.
- `inspect_dds_type_id`, `message_id`, `produce_application_properties`, `produce_properties`.

**Errors (`errors`-Modul):**
- `AmqpError`, `AmqpErrorCondition`, `ErrorScope`, `ErrorDescription`.
- Helpers: `access_denied`, `instance_unknown`, `map_mapping_error`, `map_resolution_error`, `register_missing_key`, `resource_limit_exceeded`, `unknown_dds_operation`, `unsettled_state_not_implemented`.

**Limits + Keyhash (`limits` / `keyhash`-Module):**
- `ResourceLimits` (max-connections / max-frame-size / idle-timeout).
- `keyhash::*` — SHA-256 group-id-Hashing fuer §7.6.1.

**Management (`management`-Modul):**
- `AddressKind`, `CatalogEntry`, `CatalogProducer`, `CatalogTypeId`, `CatalogDirection`.
- `AuditEvent`, `AuditProducer`, `audit_event_sample`.
- `addresses`, `classify_address`, `metrics_snapshot`.

**Metrics (`metrics`-Modul):**
- `MetricsHub`, `MANDATORY_METRIC_NAMES`.

**Security (`security`-Modul):**
- `AccessControlPlugin`-Trait, `AccessOp`, `AccessDecision`, `AllowAll`, `StaticAllowList`.
- `IdentityToken`, `DualIdentity`, `SaslSubject`, `build_identity_token`.
- `GovernanceDocument`, `GovernanceRule`, `LinkGovernance`, `DataProtectionKind`, `class_ids`.

**Coexistence (`coexistence`-Modul):**
- `BridgeId`, `CoexistenceConfig`, `InboundDecision`.
- `inspect_inbound`, `stamp_outbound`, `DEFAULT_HOP_CAP = 8`, `MAX_HOP_CAP = 64`.

**RPC-Correlation (`rpc_correlation`-Modul):**
- `OutstandingCalls`, `RpcConfig`, `IssueDecision`, `ReplyDecision`, `ReplyProperties`.
- `DEFAULT_MAX_OUTSTANDING_CALLS`, `DEFAULT_RPC_TIMEOUT_MS`.

**Annex-A (`annex_a` + `codegen_helpers` + `config_xml`-Module):**
- IDL-Spiegelung des `module zerodds::amqp` aus DDS-AMQP-1.0 Annex A.
- Codegen-Helpers fuer den Annex-A-IDL-zu-Rust-Mapping.
- XML-Config-Loader (`config_xml`, Feature `std`) per §9.2.

### Implementierung

`session::advance_connection` ist eine reine State-Machine ueber den OASIS-AMQP-1.0-§2.4-Diagramm: Start → HdrRcvd → HdrExch → OpenRcvd → Opened → CloseRcvd → End. Innerhalb von `Opened` sind alle Performatives (Begin/Attach/Flow/Transfer/Disposition/Detach/End) erlaubt; State bleibt `Opened` bis Close eintrifft. Ungueltige Transitions liefern `IllegalStateTransition`.

`link::LinkSession::settle_with_mapper` ist der Spec-§7.7.3-konforme Wire-up-Pfad fuer DDS-AMQP-Endpoints mit DDS-Bridge: beim Empfangen eines AMQP-Disposition-Frames wird der Caller-`DispositionMapper` mit dem dekodierten `sample_handle` und [`DispositionState`] aufgerufen, dann der pending-Counter dekrementiert. Die alte `settle()`-Methode bleibt fuer AMQP-only-Workflows (nur counter-decrement, kein DDS-Side-State-Update).

`link::check_attach_durability` (§7.4.2) lehnt `terminus.durable=unsettled-state` mit `amqp:not-implemented` ab — broker-level message-durability ist explizit out-of-scope der DDS-AMQP-Spec.

`dds_bridge::InstanceTrackingDispatcher` implementiert §11.3 Instance-Lifecycle-Failures: register OHNE Body-Key liefert `RegisterMissingKey` (→ `amqp:decode-error`), unregister/dispose auf unbekannte Instanz liefert `UnknownInstance` (→ `amqp:precondition-failed`).

`coexistence::inspect_inbound` mit dem `stamp_outbound`-Pendant erzwingt einen Hop-Cap (default 8, max 64) gegen Multi-Bridge-Loops — `Reject` wenn Bridge-Id-Liste die Cap ueberschreitet, sonst `Accept`.

`rpc_correlation::OutstandingCalls` haelt die ausstehenden DDS-RPC-Requests mit Timeout (default 30s) + max-outstanding-Cap (default 256) — `IssueDecision::ReplyTimeout` wird emittiert wenn ein Reply fuer einen abgelaufenen Request eintrifft.

`#![forbid(unsafe_code)]` ist gesetzt. `extern crate alloc;`. SHA-256 ueber `sha2` (workspace-dep). XML-Loader ueber `roxmltree` (Feature `std`-only).

### Architektur

- **Layer:** 5 (Bridges, Tier-B).
- **Dependencies (in):** `zerodds-amqp-bridge` (Wire-Codec), `sha2` (group-id-Hashing), `roxmltree` (Config-Loader, Feature `std`).
- **Dependents (out):** `tools/amqp-dds-endpoint` (Daemon mit TCP/TLS-Listener).
- **Feature-Flags:** `std` (default, aktiviert XML-Loader + `std::error::Error`-Impls), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format: durch DDS-AMQP-1.0 + OASIS AMQP-1.0 fixiert.
- `LinkSession`-Methoden: `settle()` und `settle_with_mapper()` sind beide stabil; Caller waehlen je nach Workflow.
- Fehler-Diskriminanten: stabil; neue Diskriminanten sind Major-additive.

### Resolved Findings (gegen pre-Review)

- **F-AMQP-EP-DISPOSITION-MAPPER-WIRED:** der `DispositionMapper`-Trait war pre-Review TEST-ONLY referenziert (kein Production-Caller im Workspace); jetzt durch `LinkSession::settle_with_mapper` produktiv gewired. Zwei neue Tests (`settle_with_mapper_calls_apply_and_decrements_pending` + `settle_with_mapper_underflow_safe_at_zero`) belegen den Wire-up.

### Added — Daemon-Wireup

- Cross-Cutting Daemon-Runtime: `daemon`-Feature aktiviert
  Prometheus-Metrics (§8.2), Catalog/Healthz/Metrics-Admin-Endpoint
  (§5.2), Signal-Watcher fuer Graceful-Shutdown (§9.2), und
  OTLP-Span-Exporter (§8.3).
- Bridge-Security: TLS-Client-Connector (rustls 0.23 ClientConnection)
  + SASL/Bearer + Topic-ACL via `zerodds-bridge-security`
  (Bridge-Spec §7.1/§7.2/§7.3).
- Backoff fuer Broker-Reconnect mit Exponential-Backoff +
  Cross-Vendor-Interop-Modul.
- DDS-QoS → AMQP-Behavior-Translation in `qos_translation` (Spec §6).
