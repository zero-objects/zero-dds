# `zerodds-coap-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-coap-bridge-1.0.md`

Implementation:

- `crates/coap-bridge/` — DDS↔CoAP bridge.

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-coap-bridged CLI

**Spec:** §2 — options
`--config`/`--bind`/`--domain`/`--dtls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level
`domain`/`coap`/`oscore`/`topics`/`content_format`/`acl`/`metrics`; ENV
substitution.

**Repo:** `crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/yaml.rs`, `crates/coap-bridge/src/daemon/mod.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::make_test_config`
(config construction). Inline tests in `config.rs::tests`.

**Status:** done

## §4 CoAP wire protocol

### §4.1 RFC-7252 header + token + options

**Spec:** §4.1 — a 4-byte header (Ver/T/TKL/Code/Message-ID), token,
options, payload marker `0xFF`.

**Repo:** `crates/coap-bridge/src/codec.rs`,
`crates/coap-bridge/src/message.rs`, `crates/coap-bridge/src/option.rs`,
`crates/coap-bridge/src/method_props.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in codec/message/option;
`crates/coap-bridge/tests/daemon_e2e.rs` covers the header/token round-trip.

**Status:** done

### §4.2 POST/PUT/DELETE → DDS write/dispose

**Spec:** §4.2 — POST → DDS write (`2.04 Changed`), PUT idempotent, DELETE
→ dispose; `4.00`/`4.13`/`5.00` error mapping.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/bridge.rs`, `crates/coap-bridge/src/method_props.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §4.3 GET + Observe (RFC 7641) → DDS→CoAP push

**Spec:** §4.3 — GET with `Observe:0` registers, notify per sample with
`Observe:<seq>`; cancel via `Observe:1` or RST.

**Repo:** `crates/coap-bridge/src/observe.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::observe_register_returns_initial_content_with_observe_option`.

**Status:** done

### §4.4 Block-wise transfer (RFC 7959)

**Spec:** §4.4 — Block1 (POST) + Block2 (notify); `block_size` (SZX
16..1024); defragmentation cap.

**Repo:** `crates/coap-bridge/src/blockwise.rs`,
`crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/option.rs` (Block1/Block2 options),
`crates/coap-bridge/src/daemon/server.rs` (block wire-up).

**Tests:** inline `#[cfg(test)] mod tests` in `blockwise.rs` covers the
SZX/block round-trip; `crates/coap-bridge/tests/daemon_e2e.rs` (cluster-C
block-wise E2E with a multi-block payload).

**Status:** done

### §4.5 Content-format registry 65000/65001/65002/50/60

**Spec:** §4.5 — vendor range 65000-65535 for CDR2-LE/BE + CDR1-LE;
`50=application/json`, `60=application/cbor`.

**Repo:** `crates/coap-bridge/src/option.rs` (content-format tags),
`crates/coap-bridge/src/bridge.rs` (CDR decoder).

**Tests:** inline tests in `option.rs::tests` (content-format
encode/decode).

**Status:** done

## §5 Topic mapping

### §5.1 Slug algorithm DDS → CoAP URI

**Spec:** §5.1 — lowercase, `::`→`/`, non-`[a-z0-9/_-]`→`_`; override per
`coap_uri_path`.

**Repo:** `crates/coap-bridge/src/uri.rs`,
`crates/coap-bridge/src/daemon/config.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `uri.rs`.

**Status:** done

### §5.2 /.well-known/core (RFC 6690) catalog

**Spec:** §5.2 — `/.well-known/core` returns a link-format resource list
with `rt="dds.topic"`, `ct=65000`, `type="..."`.

**Repo:** `crates/coap-bridge/src/core_link.rs`,
`crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::well_known_core_returns_link_format_catalog`.

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → CoAP-behavior map

**Spec:** §6 — Reliable→CON, BestEffort→NON, Volatile/TransientLocal,
Lifespan→`Max-Age`, Deadline→`5.03`, Liveliness→ping, Partition→filter.

**Repo:** `crates/coap-bridge/src/reliability.rs`,
`crates/coap-bridge/src/observe.rs`, `crates/coap-bridge/src/bridge.rs`,
`crates/coap-bridge/src/daemon/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`
(reliable POST→write); a QoS matrix in
`crates/coap-bridge/src/daemon/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 DTLS coaps:// + cipher suites

**Spec:** §7.1 — `coaps://` mode per `coap.dtls.enabled`, PSK/cert/hybrid
cipher; SIGHUP cert rotation. The wire-DTLS path is available as the opt-in
feature `dtls` (DTLS 1.2 via `webrtc-dtls`, crates.io/MIT-Apache), labelled
an experimental profile — not a default (ADR 0011). Auth+ACL run over vendor
option 65000 (CoAP application auth token), cluster-B-wired.

**Repo:** `crates/coap-bridge/src/dtls.rs` (DTLS mode/config),
`crates/coap-bridge/src/dtls_transport.rs` (feature `dtls`:
`DtlsCoapServer`/`DtlsCoapClient`/`DtlsCoapSession` — real DTLS 1.2
handshake + CoAP-over-DTLS), `crates/coap-bridge/src/daemon/security.rs`
(option-65000 auth wire-up).

**Tests:** inline `#[cfg(test)] mod tests` in `dtls.rs` (codec round-trip);
`crates/coap-bridge/tests/dtls_coap_e2e.rs` (DTLS handshake + CoAP-GET →
2.05-Content over the encrypted channel, feature `dtls`);
`crates/coap-bridge/tests/security_e2e.rs` (option-65000 auth).

**Status:** done (opt-in, experimental) — DTLS 1.2 wire path via
`webrtc-dtls` (ADR 0011); auth+ACL via option 65000 fully covered.

### §7.2 OSCORE (RFC 8613)

**Spec:** §7.2 — master-secret/salt/ID-context, HKDF sender/recipient
context, replay window 32.

**Repo:** `crates/coap-bridge/src/oscore/` — `mod.rs` (security context +
HKDF key derivation, RFC 8613 §3.2 / RFC 5869), `aead.rs` (AES-CCM-16-64-
128, nonce §5.2, AAD §5.4), `message.rs` (protect/unprotect §8.1/§8.2),
`wire.rs` (OSCORE option codec §6.1 + anti-replay window §3.2.2);
`crates/coap-bridge/src/daemon/config.rs` (oscore block, spec schema).

**Tests:** inline `#[cfg(test)] mod tests` across the `oscore` submodules:
RFC 8613 Appendix C.1.1 vector (`mod.rs`), §6.3 option vectors (`wire.rs`),
§5.2 nonce + §5.4 AAD round-trips (`aead.rs`), protect/unprotect round-trip
(`message.rs`).

**Status:** done — full OSCORE implementation (ADR 0010); correctness
anchored to the RFC 8613 Appendix C vectors.

### §7.3 ACL per topic

**Spec:** §7.3 — subject = vendor-auth-token ID (CoAP option 65000) or a
cert subject DN.

**Repo:** `crates/coap-bridge/src/daemon/config.rs` (ACL fields),
`crates/coap-bridge/src/daemon/security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/coap-bridge/tests/security_e2e.rs` (ACL enforcement
against a subject matrix via the cluster-B wire-up).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`,
`crates/coap-bridge/src/daemon/cli.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (spawn with log-level).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/config.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (counter/gauge families
cluster-A wire-up).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (`/metrics` endpoint via
cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables span emission.

**Repo:** `crates/coap-bridge/src/daemon/runtime_common.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/coap-bridge/src/daemon/server.rs`
(span emit per CoAP exchange).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → DTLS → DCPS → reader/writer → UDP bind 5683/5684 →
signal handler.

**Repo:** `crates/coap-bridge/src/daemon/mod.rs`,
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, observer deregister, cleanup;
SIGHUP TLS+ACL reload.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`,
`crates/coap-bridge/src/daemon/runtime_common.rs` (SIGTERM/SIGINT/SIGHUP via
the cluster-A signal handler),
`crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs` (daemon stop),
`crates/coap-bridge/tests/security_e2e.rs` (SIGHUP reload TLS+ACL).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + libcoap/californium/aiocoap

**Spec:** §10 — the daemon is a normal RTPS peer; the CoAP side against
libcoap/californium/aiocoap/Eclipse-Wakaama.

**Repo:** `crates/coap-bridge/src/daemon/server.rs`.

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
RTPS peer; libcoap/californium/aiocoap matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-coap-bridged`; configs/services/Docker;
manuals.

**Repo:** `packaging/linux/systemd/zerodds-coap-bridged.service`,
`packaging/macos/launchd/org.zerodds.coap-bridged.plist`,
`packaging/macos/homebrew/zerodds-coap-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/coap-bridged/`,
`packaging/linux/configs/coap-bridged.yaml.example`,
`man/man1/zerodds-coap-bridged.1`, `man/man5/zerodds-coap-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/coap_codec/block_assembler/observe_table/dtls/oscore/dds_pump,
≥ 5 tests each.

**Repo:** `crates/coap-bridge/src/{daemon/config.rs,codec.rs,message.rs,option.rs,blockwise.rs,observe.rs,reliability.rs,uri.rs,core_link.rs,bridge.rs,dtls.rs,multicast.rs,matching.rs,caching_proxy.rs,method_props.rs}`.

**Tests:** inline `#[cfg(test)] mod tests` per module.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, libcoap client, POST/Observe/Block
round-trip.

**Repo:** `crates/coap-bridge/src/bin/zerodds-coap-bridged.rs`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::post_to_configured_path_returns_2_04_changed`,
`::well_known_core_returns_link_format_catalog`,
`::observe_register_returns_initial_content_with_observe_option`,
`::unknown_path_returns_bad_request`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber + libcoap/californium client in
compose.

**Repo:** `crates/coap-bridge/tests/cross_vendor.rs` (cluster-C cross-vendor
harness).

**Tests:** `crates/coap-bridge/tests/cross_vendor.rs` (Cyclone-DDS subscriber
+ libcoap client + ZeroDDS CoAP bridge).

**Status:** done

## §13 Cross-references

### §13 Related library + RFCs + daemons

**Spec:** §13 — library `crates/coap-bridge/`, RFC 7252/7641/7959/8613, wire
format, deployment, sister daemons.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive config (e.g. content-format
IDs), major=wire-protocol change.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

23 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-coap-bridge` — tests green, 0 failed.
