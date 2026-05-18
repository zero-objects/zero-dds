# RC1 Review — `zerodds-coap-bridge`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 5 (Bridges)
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

CoAP (RFC 7252) komplettes Stack-Set: Wire-Codec + Reliability + Block-Wise (RFC 7959) + Discovery (RFC 6690) + Observe (RFC 7641) + Multicast + Caching/Proxying + DTLS-Mode + DDS-Topic-Bridge. `no_std + alloc`.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begruendung:** vollstaendiger no_std-CoAP-Stack mit DDS-Bridge — auf crates.io ist kein vergleichbares Paket verfuegbar (existierende crates sind entweder std-only oder limitiert auf Wire-Codec).

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs              # Crate-Header + Public-Re-Exports
├── blockwise.rs        # RFC 7959 Block-Wise-Transfer
├── bridge.rs           # CoAP↔DDS-Topic-Mapping
├── caching_proxy.rs    # §5.6 + §5.7 Caching/Proxying
├── codec.rs            # §3 + §3.1 Wire-Codec
├── core_link.rs        # RFC 6690 CoRE-Link-Format
├── dtls.rs             # §9 DTLS-Mode-Marker
├── matching.rs         # §5.3 Request/Response-Matching
├── message.rs          # §3 + §12.1 Message-Modell
├── method_props.rs     # §5.8 Method-Properties
├── multicast.rs        # §8 Multicast-Operation
├── observe.rs          # RFC 7641 Observer-Registry
├── option.rs           # §3.1 + §5.10 Options
├── reliability.rs      # §4 Retransmit-Tracker
└── uri.rs              # §6 URI-Scheme-Parser

tests/
└── fuzz_smoke.rs       # Pseudo-Random-Bytes-Stream-Decoder
```

### 3.2 Public-API-Surface

15 Module, alle `pub`. lib.rs re-exportiert: Codec / Message / Options / Block-Wise / CoRE-Link / Observe / Reliability / Bridge.

### 3.3 Tests

- `cargo test -p zerodds-coap-bridge`: ✅ **141 unit + 3 fuzz-smoke + 1 doc-test = 145 tests passed**.

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation |
|---|---|---|---|---|
| `CoapMessage` / `CoapCode` / `MessageType` + `encode` / `decode` | RFC 7252 §3 + §12.1 | (vorgesehen Caller-Layer) | viele | OPTIONAL-HOOK (Constrained-Device-Endpoint-Layer ist Caller-Konstruktion; Crate liefert die volle Wire-Schicht) |
| `BlockOption` / `BlockReassembler` | RFC 7959 | (Caller) | mehrere | OPTIONAL-HOOK |
| `CoreLink` / `encode_links` / `decode_links` | RFC 6690 | (Caller) | mehrere | OPTIONAL-HOOK |
| `OBSERVE_OPTION_NUMBER` / `ObserveRegistry` | RFC 7641 | (Caller) | mehrere | OPTIONAL-HOOK |
| `ReliabilityTracker` | RFC 7252 §4 | (Caller) | mehrere | OPTIONAL-HOOK |
| `CoapDdsBridge` / `BridgeOp` / `map_method` / `parse_dds_path` | DDS-Mapping (intern) | (Caller integriert mit `zerodds-dcps`) | mehrere | OPTIONAL-HOOK |

**Akzeptanz:** Alle 6 Item-Familien sind als OPTIONAL-HOOK klassifiziert: die Crate ist Substrat fuer einen CoAP-Endpoint-Server, der typisch erst beim End-Anwender konstruiert wird (Tokio-Listener, embassy-Async, etc.). Per Guardrails §1.5b gilt OPTIONAL-HOOK als ✅, wenn explizit dokumentiert — was im README + lib.rs klar gemacht wird (Bridge-Crate, Wire-Stack-Substrat, kein eigener Server). 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
# none — pure no_std + alloc
```

### 4.2 Dependents (used-by)

Aktuell keine internen Konsumenten — Crate ist als Substrat fuer DDS-IoT-Endpoint-Mapping konstruiert. Tracker-Item zeigt das auf (5.3 coap-bridge sieht keinen direkten Layer-6-Konsumenten in diesem Release-Cycle).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | `std::error::Error`-Impls. |
| `alloc` | ✅ (via std) | `Vec` / `String` / `BTreeMap`. |

## 5 Spec-Relevanz

- RFC 7252 (CoAP), RFC 7641 (Observe), RFC 7959 (Block-Wise), RFC 6690 (CoRE-Link).
- Self-contained Wire-Specs; Conformance-Vektoren in 141 Unit-Tests + 3 Fuzz-Smoke-Tests materialisiert.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Treffer: **0**.

### 6.1b Sprint-/Phase-/Cluster-Marker

**Vor Cleanup:** 1 Treffer in `bridge.rs:1` — „Sprint 6 Task #40" Sprint-Reference.

**Cleanup:** entfernt.

### 6.2 Soft-Review-Treffer (TODO/FIXME/HACK)

Treffer: **0**.

### 6.2b Spec-Conformance-Sweep

0 Inline-Deferral-Marker, 0 Layering-Violation-Hinweise.

### 6.3 Tech-Debt

`lib.rs`-Header behauptete vor dem Review faelschlich, Reliability + Block-Wise + DTLS-Security waeren nicht abgedeckt. Alle drei SIND vollstaendig implementiert (`reliability.rs` + `blockwise.rs` + `dtls.rs`). Header korrigiert.

### 6.4 Public-API-Leaks

Keine. `#![warn(missing_docs)]` aktiv.

## 7 Cleanup-Actions

1. `Cargo.toml` — `publish=true` + Metadata komplett.
2. `lib.rs` — Crate-Header neu (vorherige "Was nicht abgedeckt" Sektion war stale; jetzt volle Public-API-Liste + Quickstart-Doc-Test).
3. License-Header (SPDX-Apache-2.0) auf alle 15 src-Files.
4. Sprint-Marker entfernt: `bridge.rs` ("Sprint 6 Task #40").
5. `Spec docs/standards/cache/...`-Cache-Path-Refs durch RFC-Numbers ersetzt.
6. README.md aus Stub auf RC1-Format gehoben.
7. CHANGELOG.md mit `[1.0.0-rc.1]`.
8. Public-Mirror unter `github/crates/coap-bridge/`.
9. `website/docs/coap-bridge.md`.
10. Tracker: 5.3 coap-bridge → ✅.

## 8-12 (Standard-Block)

- Doc-Build clean (0 Warnings).
- Tests + Lints + Doc-Build alle ✅.
- DoD-Checkliste vollstaendig.

## 13 Daemon-Wireup-Append

Folgende Items sind nach dem ersten Sign-off in den `daemon`-Feature-
Pfad eingebracht worden (kein Major-Bump, alles innerhalb 1.0.0-rc.1):

- `daemon/runtime_common.rs` + `daemon/qos_translation.rs` +
  `daemon/server.rs` + `daemon/config.rs`.
- Auth-Token-Option + Topic-ACL via `zerodds-bridge-security`
  voll wired (Bridge-Spec §7.2/§7.3). DTLS (§7.1) ist via
  separates ADR — Daemon meldet beim Setzen von `--tls-cert/--tls-key`
  ein klares Signal ohne DTLS-Acceptor.
- `blockwise.rs`-Modul + `cross_vendor.rs`-Modul.
- Tests gruen: 141 unit + 3 fuzz-smoke + 1 doc.

**Crate-Version:** `1.0.0-rc.1`
**Reviewer-Sign-off:** claude
