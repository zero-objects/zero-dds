# `zerodds-coap-bridge` v1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-coap-bridge-1.0.md`

## §1 Conformance-Levels

### §1 L1-L6 Conformance-Matrix

**Spec:** §1 — sechs Levels (Wire/DDS/Bridging/Config/Auth/Multi-
Tenant); L1–L4 Pflicht, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI-Surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — Optionen `--config`/`--bind`/`--domain`/`--dtls-*`/
`--topic`/`--log-level`/`--metrics`/`--version`/`--help`; Exit-Codes
0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

## §3 Config-File-Format

### §3 YAML-Loader mit ENV-Substitution

**Spec:** §3 — Top-Level `domain`/`coap`/`oscore`/`topics`/
`content_format`/`acl`/`metrics`; ENV-Substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/yaml.rs`,
`crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::make_test_config`
(Config-Construction). Inline-Tests in `config.rs::tests`.

**Status:** done

## §4 CoAP-Wire-Protocol

### §4.1 RFC-7252 Header + Token + Options

**Spec:** §4.1 — 4-Byte-Header (Ver/T/TKL/Code/Message-ID), Token,
Options, Payload-Marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`,
`crates/coap-bridge/src/option.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in codec/message/option;
`crates/coap-bridge/tests/daemon_e2e.rs` deckt Header/Token-Roundtrip.

**Status:** done

### §4.2 POST/PUT/DELETE → DDS-Write/Dispose

**Spec:** §4.2 — POST → DDS-Write (`2.04 Changed`), PUT idempotent,
DELETE → Dispose; `4.00`/`4.13`/`5.00` Error-Mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP-Push

**Spec:** §4.3 — GET mit `Observe:0` registriert, Notify pro Sample
mit `Observe:<seq>`; Cancel via `Observe:1` oder RST.

**Repo:** `crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option`.

**Status:** done

### §4.4 Block-Wise-Transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (Notify); `block_size` (SZX
16..1024); Defragmentation-Cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`,
`crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/option.rs` (Block1/Block2-Options),
`crates/coap-bridge/src/daemon/server.rs` (Block-Wireup).

**Tests:** Inline `#[cfg(test)] mod tests` in `blockwise.rs` deckt
SZX/Block-Roundtrip; `crates/coap-bridge/tests/daemon_e2e.rs`
(Cluster-C Block-Wise-E2E mit Multi-Block-Payload).

**Status:** done

### §4.5 Content-Format-Registry 65000/65001/65002/50/60

**Spec:** §4.5 — Vendor-Range 65000-65535 für CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/option.rs` (Content-Format-Tags),
`crates/coap-bridge/src/bridge.rs` (CDR-Decoder).

**Tests:** Inline-Tests in `option.rs::tests` (Content-Format
encode/decode).

**Status:** done

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus DDS → CoAP-URI

**Spec:** §5.1 — Lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; Override
per `coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** Inline `#[cfg(test)] mod tests` in `uri.rs`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) Catalog

**Spec:** §5.2 — `/.well-known/core` liefert Link-Format-Resource-List
mit `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/core_link.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::well_known_core_returns_link_format_catalog`.

**Status:** done

## §6 QoS-Translation

### §6 DDS-QoS → CoAP-Behavior Map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→Ping, Partition→Filter.

**Repo:** `crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/daemon/qos_translation.rs` (Cluster-A QoS-Map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`
(reliable POST→Write); QoS-Matrix in
`crates/coap-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 DTLS coaps:// + Cipher-Suites

**Spec:** §7.1 — `coaps://`-Mode per `coap.dtls.enabled`, PSK/Cert/
Hybrid-Cipher; SIGHUP-Cert-Rotation. Decision-Record:
`docs/adr/0007-coap-oscore-rejected-rc1.md` deckt OSCORE; DTLS-eigener
ADR im RC1-Closeout: Pure-Rust-DTLS-Stack 2026 nicht audit-ready, daher
volle Wire-DTLS-Pfad als `n/a (rejected)`. Auth+ACL über Vendor-Option
65000 (CoAP-Application-Auth-Token) Cluster-B-wired.

**Repo:** `crates/coap-bridge/src/dtls.rs` (DTLS-Codec, deferred
Wire-Bind), `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/security.rs` (Option-65000-Auth-Wireup).

**Tests:** Inline `#[cfg(test)] mod tests` in `dtls.rs` deckt
Codec-Roundtrip; `crates/coap-bridge/tests/security_e2e.rs` deckt
Option-65000-Auth-Wireup.

**Status:** n/a (rejected) — Pure-Rust-DTLS RC1 nicht audit-ready;
Auth+ACL via Cluster-B-Option-65000-Wireup voll abgedeckt.

### §7.2 OSCORE (RFC 8613)

**Spec:** §7.2 — Master-Secret/Salt/ID-Context, HKDF-Sender/Recipient-
Context, Replay-Window 32. Decision-Record:
`docs/adr/0007-coap-oscore-rejected-rc1.md` — OSCORE in RC1-Markt
(LwM2M-nische) nicht relevant, COSE-Stack-Aufwand ohne Customer-Pull.

**Repo:** `crates/coap-bridge/src/daemon/config.rs` (oscore-Block,
Spec-Schema).

**Tests:** —

**Status:** n/a (rejected) — siehe ADR-0007.

### §7.3 ACL pro Topic

**Spec:** §7.3 — Subject = Vendor-Auth-Token-ID (CoAP-Option-65000) oder
Cert-Subject-DN.

**Repo:** `crates/coap-bridge/src/daemon/config.rs` (ACL-Felder),
`crates/coap-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/coap-bridge/tests/security_e2e.rs` (ACL-
Enforcement gegen Subject-Matrix via Cluster-B-Wireup).

**Status:** done

## §8 Operations + Observability

### §8.1 Strukturiertes JSON-Logging

**Spec:** §8.1 — JSON-Log + `--log-level`-Switch.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (Spawn mit
log-level).

**Status:** done

### §8.2 Prometheus-Metrics

**Spec:** §8.2 — `--metrics`-CLI + 10 Counter/Gauge-Familien.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (Counter/Gauge-
Familien Cluster-A-Wireup).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (`/metrics`-Endpoint
via Cluster-A-Wireup).

**Status:** done

### §8.3 OTLP-Spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` aktiviert Span-Emission.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs` (OTLP-Init
via `zerodds-observability-otlp`),
`crates/coap-bridge/src/daemon/server.rs` (Span-Emit pro CoAP-Exchange).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (Daemon-Spawn mit
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup-Sequence

**Spec:** §9.1 — Config → DTLS → DCPS → Reader/Writer → UDP-Bind 5683/
5684 → Signal-Handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — Graceful Drain max 30 s, Observer-Deregister, Cleanup;
SIGHUP TLS+ACL-Reload.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (SIGTERM/SIGINT/
SIGHUP via Cluster-A-Signal-Handler);
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (Daemon-Stop),
`crates/coap-bridge/tests/security_e2e.rs` (SIGHUP-Reload TLS+ACL).

**Status:** done

## §10 Cross-Vendor

### §10 RTPS-Peer + libcoap/californium/aiocoap

**Spec:** §10 — Daemon ist normaler RTPS-Peer; CoAP-Seite gegen
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor RTPS-Peer; libcoap/californium/aiocoap-Matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker Layout

**Spec:** §11 — Binary `zerodds-coap-bridged`; Configs/Services/Docker;
Manuals.

**Repo:** `packaging/linux/systemd/zerodds-coap-bridged.service`,
`packaging/macos/launchd/org.zerodds.coap-bridged.plist`,
`packaging/macos/homebrew/zerodds-coap-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/coap-bridged/`,
`packaging/linux/configs/coap-bridged.yaml.example`,
`man/man1/zerodds-coap-bridged.1`,
`man/man5/zerodds-coap-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit-Tests pro Modul

**Spec:** §12.1 — config/coap_codec/block_assembler/observe_table/dtls/
oscore/dds_pump je ≥ 5 Tests.

**Repo:** `crates/coap-bridge/src/{daemon/config.rs,codec.rs,message.rs,option.rs,blockwise.rs,observe.rs,reliability.rs,uri.rs,core_link.rs,bridge.rs,dtls.rs,multicast.rs,matching.rs,caching_proxy.rs,method_props.rs}`.

**Tests:** Inline `#[cfg(test)] mod tests` pro Modul.

**Status:** done

### §12.2 Integration-Tests bridge_e2e

**Spec:** §12.2 — Spawn Daemon, libcoap-Client, POST/Observe/Block
Roundtrip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §12.3 Multi-Vendor cross_vendor.rs

**Spec:** §12.3 — Cyclone-DDS-Subscriber + libcoap/californium-Client
im Compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs` (Cluster-C
Cross-Vendor-Harness).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs` (Cyclone-DDS-
Subscriber + libcoap-Client + ZeroDDS-CoAP-Bridge).

**Status:** done

## §13 Cross-References

### §13 Verwandte Library + RFCs + Daemons

**Spec:** §13 — Library `crates/coap-bridge/`, RFC 7252/7641/7959/8613,
Wire-Format, Deployment, Sister-Daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer-Bump-Regeln

**Spec:** §14 — Patch=Bugfixes, Minor=additive Config (z.B.
Content-Format-IDs), Major=Wire-Protocol-Change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit-Status

21 done / 0 partial / 0 open / 3 n/a (informative) / 2 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-coap-bridge` — Tests grün, 0 failed.

Offene Punkte und Decision-Records: siehe `zerodds-coap-bridge-1.0.open.md`.
