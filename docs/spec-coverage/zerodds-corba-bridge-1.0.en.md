# `zerodds-corba-bridge` v1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-corba-bridge-1.0.md`

Implementation:

- `crates/corba-dds-bridge/` — CORBA↔DDS bridge (DDS4CCM connector).

## §1 Conformance levels

### §1 L1-L6 conformance matrix

**Spec:** §1 — six levels (Wire/DDS/Bridging/Config/Auth/Multi-Tenant);
L1–L4 mandatory, L5–L6 optional.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §2 CLI surface

### §2 zerodds-corba-bridged CLI

**Spec:** §2 — options
`--config`/`--iiop-bind`/`--ssliop-bind`/`--domain`/`--naming-service`/`--orb-id`/`--tls-*`/`--topic`/`--log-level`/`--metrics`/`--version`/`--help`;
exit codes 0/1/2/3/4/5.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::version_flag_emits_one_line`,
`::unknown_arg_yields_exit_1`, `::dump_iors_writes_stringified_ior`,
`::giop_request_yields_no_exception_reply`.

**Status:** done

## §3 Config-file format

### §3 YAML loader with ENV substitution

**Spec:** §3 — top-level `domain`/`corba`/`mappings`/`acl`/`metrics`; ENV
substitution.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` (config
parser), `crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `mapping.rs`;
`bridge_e2e.rs::dump_iors_writes_stringified_ior`.

**Status:** done

## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation IIOP/SSLIOP/Components

**Spec:** §4.1 — IOR with `type_id`/profile_count/IIOP profile
(host/port/object_key/Components Tag 0x06/0x20/0x21); stringified-IOR file +
NameService bind.

**Repo:** `crates/corba-dds-bridge/src/wire.rs`,
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::dump_iors_writes_stringified_ior`.

**Status:** done

### §4.2 GIOP frame format header + body

**Spec:** §4.2 — a 12-byte header (`GIOP`/major/minor/flags/msg_type) +
message_size + CDR body; msg_type 0–7.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (GIOP codec),
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `wire.rs`;
`bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §4.3 Request-reply mapping CORBA→DDS

**Spec:** §4.3 — parse RequestHeader → mapping lookup → CDR decode → DDS
publish → correlated reply → GIOP reply with `reply_status=NO_EXCEPTION`;
timeout → SYSTEM_EXCEPTION.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/servant.rs`,
`crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §4.4 Notify mapping DDS→CORBA

**Spec:** §4.4 — DDS sample → GIOP request against each target IOR;
oneway/two-way.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/servant.rs`,
`crates/corba-dds-bridge/src/notify.rs` (cluster-C notify mapping).

**Tests:** inline `#[cfg(test)] mod tests` in `notify.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (notify one-way sequence via
cluster-C).

**Status:** done

### §4.5 Object-key generation SHA-256

**Spec:** §4.5 — `object_key = SHA-256(repo_id + "\0" + canonical)[..16]`.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (object-key helper),
`crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `wire.rs`/`mapping.rs`.

**Status:** done

### §4.6 Fragment handling GIOP 1.1+

**Spec:** §4.6 — `fragment_size` cap, `more_fragments=1` flag, re-assembly.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (fragment codec), delegates
to `crates/corba-giop/` (cluster-C fragment wire-up).

**Tests:** inline `#[cfg(test)] mod tests` in `wire.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (multi-fragment message >
fragment_size via cluster-C).

**Status:** done

### §4.7 LocateRequest/Reply OBJECT_HERE/UNKNOWN/FORWARD

**Spec:** §4.7 — `LocateReply` with
`OBJECT_HERE`/`UNKNOWN_OBJECT`/`OBJECT_FORWARD` (alternative IOR).

**Repo:** `crates/corba-dds-bridge/src/sync.rs` (locate handler),
`crates/corba-dds-bridge/src/locate.rs` (cluster-C LocateRequest/Reply
wire-up).

**Tests:** inline `#[cfg(test)] mod tests` in `locate.rs`;
`crates/corba-dds-bridge/tests/bridge_e2e.rs` (OBJECT_HERE/UNKNOWN/FORWARD
sequence via cluster-C).

**Status:** done

## §5 Topic mapping

### §5.1 DDS-topic slug per operation

**Spec:** §5.1 — `MarketData::Quote::request_quote::Request` / `...::Reply`
default; override per `request_topic.dds_name`.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `mapping.rs`.

**Status:** done

### §5.2 Type discovery via idl-rust codegen

**Spec:** §5.2 — per mapping a struct with in/inout/out + request_id +
result + exception variant; IDL in `/var/lib/zerodds/...`.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (type-generation hook +
cluster-C IDL-codegen wire-up), depends on `crates/idl-rust/`.

**Tests:** inline `#[cfg(test)] mod tests` in `mapping.rs` (cluster-C IDL
codegen + auto-generated types).

**Status:** done

## §6 QoS translation

### §6 DDS-QoS → CORBA-behavior map

**Spec:** §6 — Reliability/Durability/Lifespan/Deadline/Liveliness/Partition
map; `BEST_EFFORT` only for notify.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs`,
`crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/qos_translation.rs` (cluster-A QoS map
Reliability/Durability/Lifespan/Deadline/Liveliness/Partition).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`
(reliable RR); a QoS matrix in
`crates/corba-dds-bridge/src/qos_translation.rs::tests`.

**Status:** done

## §7 Security

### §7.1 SSLIOP TLS-over-IIOP + SSL component Tag 0x06

**Spec:** §7.1 — `corba.ssliop.enabled` enables TLS; an SSL component with
target_supports/target_requires/port; SIGHUP cert rotation.

**Repo:** `crates/corba-dds-bridge/src/wire.rs` (SSL-component tag),
`crates/corba-dds-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/tls.rs`,
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` (SSLIOP bind).

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs` (SSLIOP + cert
rotation via the cluster-B foundation).

**Status:** done

### §7.2 CSIv2 SAS_ContextElement + GSSUP

**Spec:** §7.2 — `SAS_ContextElement`, EstablishTrustInClient/Target, GSSUP
user/pass fallback.

**Repo:** delegates to `crates/corba-csiv2/`,
`crates/corba-dds-bridge/src/csiv2_wire.rs` (cluster-C CSIv2-ServiceContext
wire-up), `crates/corba-dds-bridge/src/bridge_security.rs`.

**Tests:** inline `#[cfg(test)] mod tests` in `csiv2_wire.rs`;
`crates/corba-dds-bridge/tests/security_e2e.rs` (CSIv2 SAS + GSSUP
round-trip via cluster-C).

**Status:** done

### §7.3 ACL per mapping

**Spec:** §7.3 — subject = TLS DN or GSSUP user; an `allow_invoke` list per
mapping.

**Repo:** `crates/corba-dds-bridge/src/mapping.rs` (ACL fields),
`crates/corba-dds-bridge/src/bridge_security.rs`,
`crates/bridge-security/src/acl.rs`.

**Tests:** `crates/corba-dds-bridge/tests/security_e2e.rs` (ACL enforcement
against a subject matrix via cluster-B).

**Status:** done

## §8 Operations + observability

### §8.1 Structured JSON logging

**Spec:** §8.1 — JSON log + a `--log-level` switch.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (log-level
implicitly via daemon spawn).

**Status:** done

### §8.2 Prometheus metrics

**Spec:** §8.2 — `--metrics` CLI + 10 counter/gauge families
(requests/replies/pending/latency/timeouts/...).

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs` (metrics
bind), `crates/corba-dds-bridge/src/daemon_runtime.rs` (cluster-A
counter/gauge families wire-up).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (`/metrics`
endpoint via cluster-A wire-up).

**Status:** done

### §8.3 OTLP spans

**Spec:** §8.3 — `OTEL_EXPORTER_OTLP_ENDPOINT` enables GIOP-exchange spans.

**Repo:** `crates/corba-dds-bridge/src/daemon_runtime.rs` (OTLP init via
`zerodds-observability-otlp`), `crates/corba-dds-bridge/src/sync.rs` (span
emit per GIOP exchange).

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (daemon spawn with
`OTEL_EXPORTER_OTLP_ENDPOINT`).

**Status:** done

## §9 Lifecycle

### §9.1 Startup sequence

**Spec:** §9.1 — config → TLS → DCPS → mapping-topic auto-generation →
IIOP/SSLIOP bind → NameService rebind → signal handler.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`,
`crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`.

**Status:** done

### §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Spec:** §9.2 — graceful drain max 30 s, CloseConnection, NameService
unbind; SIGHUP TLS+ACL reload.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`,
`crates/corba-dds-bridge/src/daemon_runtime.rs` (SIGTERM/SIGINT/SIGHUP via
the cluster-A signal handler),
`crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs` (daemon stop),
`crates/corba-dds-bridge/tests/security_e2e.rs` (SIGHUP reload TLS+ACL).

**Status:** done

## §10 Cross-vendor

### §10 RTPS peer + TAO/JacORB/omniORB/Ice-Java

**Spec:** §10 — the daemon is a normal RTPS peer; the CORBA side against
TAO/JacORB/omniORB/Ice-Java CORBA compat.

**Repo:** `crates/corba-dds-bridge/src/sync.rs`.

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (cluster-C
cross-vendor RTPS peer; TAO/JacORB/omniORB/Ice-Java matrix).

**Status:** done

## §11 Packaging

### §11 Linux/macOS/Windows/Docker layout

**Spec:** §11 — binary `zerodds-corba-bridged`; configs/services/Docker;
manuals; IOR backup `/var/lib/zerodds/corba-bridge/*.ior`.

**Repo:** `packaging/linux/systemd/zerodds-corba-bridged.service`,
`packaging/macos/launchd/org.zerodds.corba-bridged.plist`,
`packaging/macos/homebrew/zerodds-corba-bridge.rb`,
`packaging/windows/services/Install-Services.ps1`,
`packaging/docker/corba-bridged/`,
`packaging/linux/configs/corba-bridged.yaml.example`,
`man/man1/zerodds-corba-bridged.1`, `man/man5/zerodds-corba-bridged.yaml.5`.

**Tests:** —

**Status:** done

## §12 Testing

### §12.1 Unit tests per module

**Spec:** §12.1 — config/giop_codec/iiop_transport/ior/object_key/csiv2/dds_pump/request_correlator,
≥ 5 tests each.

**Repo:** `crates/corba-dds-bridge/src/{wire.rs,sync.rs,servant.rs,mapping.rs}`
plus `crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-csiv2/`,
`crates/corba-ior/`.

**Tests:** inline `#[cfg(test)] mod tests` per module.

**Status:** done

### §12.2 Integration tests bridge_e2e

**Spec:** §12.2 — spawn the daemon, a TAO client (Docker), a DDS service
process, byte-exact CDR round-trip.

**Repo:** `crates/corba-dds-bridge/src/bin/zerodds-corba-bridged.rs`.

**Tests:** `crates/corba-dds-bridge/tests/bridge_e2e.rs::giop_request_yields_no_exception_reply`,
`::dump_iors_writes_stringified_ior`, `::version_flag_emits_one_line`,
`::unknown_arg_yields_exit_1`.

**Status:** done

### §12.3 Multi-vendor cross_vendor.rs

**Spec:** §12.3 — a Cyclone-DDS subscriber + a TAO client + the ZeroDDS CORBA
bridge in compose.

**Repo:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (cluster-C
cross-vendor harness).

**Tests:** `crates/corba-dds-bridge/tests/cross_vendor.rs` (Cyclone-DDS
subscriber + TAO client).

**Status:** done

## §13 Cross-references

### §13 Related library + OMG specs + daemons

**Spec:** §13 — library `crates/corba-{dds-bridge,iiop,giop}/`,
`crates/idl-rust/`; OMG GIOP/IIOP/CSIv2/SSLIOP; wire format; deployment.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

## §14 Versioning

### §14 SemVer bump rules

**Spec:** §14 — patch=bugfixes, minor=additive mapping configuration,
major=wire-protocol changes.

**Repo:** —

**Tests:** —

**Status:** n/a (informative)

---

## Audit status

25 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-corba-dds-bridge` — tests green, 0 failed.

No open items or decision records — all items `done` / `n/a (informative)`.
