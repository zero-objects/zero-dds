# `zerodds-corba-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-corba-bridge-1.0.md`

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — Optionen `--config`/`--iiop-bind`/`--ssliop-bind`/
`--domain`/`--naming-service`/`--orb-id`/`--tls-*`/`--topic`/
`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes 0/1/2/3/4/5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::version_flag_emits_one_line`,
`::unknown_arg_yields_exit_1`,
`::dump_iors_writes_stringified_ior`,
`::giop_request_yields_no_exception_reply`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`corba`/`mappings`/`acl`/`metrics`;
ENV-Substitution.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(Config-Parser), `crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `mapping.rs`;
`bridge_e2e.rs::dump_iors_writes_stringified_ior`.

**Status:** done

## §4 GIOP/IIOP-Wire-Protocol

### §4.1 IOR-Generation IIOP/SSLIOP/Components

**Spec:** §4.1 — IOR mit `type_id`/profile_count/IIOP-Profile (host/
port/object_key/Components Tag 0x06/0x20/0x21); Stringified-IOR-File +
NameService-Bind.

**Repo:** `crates/corba-dds-bridge/src/wire.rs`,
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::dump_iors_writes_stringified_ior`.

**Status:** done

### §4.2 GIOP-Frame-Format Header + Body

**Spec:** §4.2 — 12-Byte-Header (`GIOP`/major/minor/flags/msg_type)
+ message_size + CDR-Body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (GIOP-Codec),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `wire.rs`;
`bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §4.3 Request-Reply-Mapping CORBA→DDS

**Spec:** §4.3 — Parse RequestHeader → Mapping-Lookup → CDR-decode →
DDS-Publish → korrelierte Reply → GIOP-Reply mit
`reply_status=NO_EXCEPTION`; Timeout → SYSTEM_EXCEPTION.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/servant.rs`,
`crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §4.4 Notify-Mapping DDS→CORBA

**Spec:** §4.4 — DDS-Sample → GIOP-Request gegen jeden Target-IOR;
oneway/two-way.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/servant.rs`,
`crates/corba-dds-bridge/src/notify.rs` (Cluster-C Notify-Mapping).

**Tests:** Inline `#[cfg(test)] mod tests` in `notify.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (Notify-One-Way-
Sequence via Cluster-C).

**Status:** done

### §4.5 Object-Key-Generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (object-key helper),
`crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `wire.rs`/`mapping.rs`.

**Status:** done

### §4.6 Fragment-Handling GIOP 1.1+

**Spec:** §4.6 — `fragment_size`-Cap, `more_fragments=1`-Flag, Re-
Assembly.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (Fragment-Codec),
delegiert an `crates/corba-giop/` (Cluster-C Fragment-Wireup).

**Tests:** Inline `#[cfg(test)] mod tests` in `wire.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (Multi-Fragment-Message
> fragment_size via Cluster-C).

**Status:** done

### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` mit `OBJECT_HERE`/`UNKNOWN_OBJECT`/
`OBJECT_FORWARD` (alternative-IOR).

**Repo:** `crates/corba-dds-bridge/src/sync.rs` (Locate-Handler),
`crates/corba-dds-bridge/src/locate.rs` (Cluster-C
LocateRequest/Reply Wireup).

**Tests:** Inline `#[cfg(test)] mod tests` in `locate.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (OBJECT_HERE/UNKNOWN/
FORWARD-Sequence via Cluster-C).

**Status:** done

## §5 Topic-Mapping

### §5.1 DDS-Topic-Slug per Operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` /
`...::Reply` Default; Override per `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `mapping.rs`.

**Status:** done

### §5.2 Type-Discovery via idl-rust Codegen

**Spec:** §5.2 — Per Mapping struct mit in/inout/out + request_id +
Result + Exception-Variant; IDL in `/var/lib/zerodds/...`.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (Type-Generation
Hook + Cluster-C IDL-Codegen-Wireup), abhängig von `crates/idl-rust/`.

**Tests:** Inline `#[cfg(test)] mod tests` in `mapping.rs` (Cluster-C
IDL-Codegen + Auto-Generated-Types).

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → CORBA-Behavior Map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/
Partition Map; `BEST_EFFORT` nur für Notify.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs`,
`crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`
(reliable RR); QoS-Matrix in
`crates/corba-dds-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL-Component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` aktiviert TLS; SSL-Component
mit target_supports/target_requires/port; SIGHUP-Cert-Rotation.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (SSL-Component-Tag),
`crates/corba-dds-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`,
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(SSLIOP-Bind).

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs` (SSLIOP +
Cert-Rotation via Cluster-B-Foundation).

**Status:** done

### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target,
GSSUP-User/Pass-Fallback.

**Repo:** delegiert an `crates/corba-csiv2/`,
`crates/corba-dds-bridge/src/csiv2_wire.rs` (Cluster-C
CSIv2-ServiceContext-Wireup),
`crates/corba-dds-bridge/src/bridge_security.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `csiv2_wire.rs`;
`crates/corba-dds-bridge/tests/security_e2e.rs` (CSIv2 SAS +
GSSUP-Roundtrip via Cluster-C).

**Status:** done

### §7.3 ACL pro Mapping

**Spec:** §7.3 — Subject = TLS-DN oder GSSUP-User; pro Mapping
`allow_invoke`-Liste.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (ACL-Felder),
`crates/corba-dds-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs` (ACL-
Enforcement gegen Subject-Matrix via Cluster-B).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (log-level
implizit über Daemon-Spawn).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien
(requests/replies/pending/latency/timeouts/...).

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`
(metrics-Bind),
`crates/corba-dds-bridge/src/daemon_runtime.rs` (Cluster-A Counter/
Gauge-Familien Wireup).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (`/metrics`-
Endpoint via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert
GIOP-Exchange-Spans.

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs` (OTLP-Init
via `zerodds-observability-otlp`),
`crates/corba-dds-bridge/src/sync.rs` (Span-Emit pro GIOP-Exchange).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (Daemon-Spawn
mit `OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → TLS → DCPS → Mapping-Topic-Auto-Generation
→ IIOP/SSLIOP-Bind → NameService-Rebind → Signal-Handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, CloseConnection,
NameService-Unbind; SIGHUP TLS+ACL-Reload.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP
via Cluster-A-Signal-Handler),
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (Daemon-Stop),
`crates/corba-dds-bridge/tests/security_e2e.rs` (SIGHUP-Reload TLS+
ACL).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + TAO/JacORB/omniORB/Ice-Java

**Spec:** §10 — Daemon ist normaler RTPS-Peer; CORBA-Seite gegen
TAO/JacORB/omniORB/Ice-Java-CORBA-Compat.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer; TAO/JacORB/omniORB/Ice-Java-Matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-corba-bridged`; Configs/Services/Docker;
Manuals; IOR-Backup `/var/lib/zerodds/corba-bridge/*.ior`.

**Repo:** `packaging/linux/systemd/zerodds-corba-bridged.service`,
`packaging/macos/launchd/org.zerodds.corba-bridged.plist`,
`packaging/macos/homebrew/zerodds-corba-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/corba-bridged/`,
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man1/zerodds-corba-bridged.1`,
`man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/giop_codec/iiop_transport/ior/object_key/
csiv2/dds_pump/request_correlator je ≥ 5 Tests.

**Repo:** `crates/corba-dds-bridge/src/{wire.rs,sync.rs,servant.rs,mapping.rs}` plus `crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-csiv2/`, `crates/corba-ior/`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, TAO-Client (Docker), DDS-Service-
Process, byte-genauer CDR-Roundtrip.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`,
`::dump_iors_writes_stringified_ior`,
`::version_flag_emits_one_line`,
`::unknown_arg_yields_exit_1`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + TAO-Client + ZeroDDS-
CORBA-Bridge im Compose.

**Repo:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (Cyclone-
DDS-Subscriber + TAO-Client).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + OMG-Specs + Daemons

**Spec:** §13 — Library `crates/corba-{dds-bridge,iiop,giop}/`,
`crates/idl-rust/`; OMG GIOP/IIOP/CSIv2/SSLIOP; Wire-Format;
Deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Mapping-Konfiguration,
Major=Wire-Protocol-Changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

25 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-corba-dds-bridge` — Tests grün, 0 failed.

Offene Punkte und Decision-Records: siehe `zerodds-corba-bridge-1.0.open.md`.
