# RC1 Review — `zerodds-transport-udp`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.7 (Wire — UDP-Implementation)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

UDP/IP-PSM-Implementation des `zerodds-transport::Transport`-Traits.
UDPv4 Unicast + Multicast (Discovery), `SO_REUSEADDR`/`SO_REUSEPORT`,
Multicast-TTL, Bind-Retry-Loop für CI-EADDRINUSE-Race.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Default-UDP-Transport für DDS-Standard-Use-Cases.

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs            # Re-Exports + Crate-Header (29 LOC)
src/udp_transport.rs  # UdpTransport-Implementation (~370 LOC)
```

### 3.2 Public-API-Surface

```rust
pub const MAX_DATAGRAM_SIZE: usize;
pub struct UdpTransport;
pub enum UdpTransportError;
```

### 3.3 Tests

- `cargo test -p zerodds-transport-udp`: ✅ 11 passed (8 lib + 3 doctest).
- `cargo build --no-default-features --features alloc`: ✅ baut.

### 3.4 Coherence-Audit (§1.5b)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `UdpTransport` | DDSI-RTPS 2.5 §9.6.1 (UDP/IP PSM) | 4 (dcps, transport-tcp/loopback-test, tools/isolation-smoke, tools/bench-suite) | CONNECTED | — |
| `MAX_DATAGRAM_SIZE` | DDSI-RTPS §9.6.1 + IP-Datagramm-Limit | 3 | CONNECTED | — |
| `UdpTransportError` | ZeroDDS-Error-Type für `bind_v4`/`with_timeout`/`bind_multicast_v4`/`set_multicast_ttl` | 0 direkte Match-Sites; aber Return-Type aller pub-Konstruktor-Methoden | VENDOR-EXTENSION (Public-API-Error-Contract) | — — Caller mit `?`-Operator nutzen sie implizit |

**Zusammenfassung:** 3/3 Public-Items klassifiziert. 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-rtps = { path = "../rtps" }
zerodds-transport = { path = "../transport" }
socket2 = { version = "0.5", features = ["all"] }
zerodds-inspect-endpoint = { path = "../inspect-endpoint", optional = true }
```

### 4.2 Dependents

`zerodds-dcps` (Hauptkonsument), `zerodds-transport-tcp` (Loopback-Tests),
`tools/isolation-smoke`, `tools/bench-suite`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + UdpSocket |
| `alloc` | ✅ (via std) | — |
| `safety` | ❌ | reserved |
| `inspect` | ❌ | PDE-Tap-Hooks (default OFF, R-034) |

## 5 Spec-Relevanz

- **Spec:** DDSI-RTPS 2.5 §9.6.1 (UDP/IP PSM) — Wire-Mapping; §9.6.1.4 (SPDP-Multicast-Discovery).
- **Coverage-Doc:** `docs/spec-coverage/ddsi-rtps-2.5.md` §9.6.1.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Treffer: keine.

### 6.2 Soft-Review-Treffer

Treffer: keine.

### 6.3 Tech-Debt + Dead Code

Keine.

### 6.4 Public-API-Leaks

Keine.

## 7 Cleanup-Actions

1. SPDX-Header in beiden src-Files.
2. Cargo.toml RC1-Metadata.
3. Crate-Header rewrite: ehrliche Feature-Liste (Multicast war als
   "Phase-1 out-of-scope" markiert — tatsächlich seit WP 0.7-A live).
4. README + CHANGELOG.

## 8 Spec-Doc-Updates

Keine.

## 9 Doc-Artefacts

- [x] Cargo.toml-Metadata vollständig
- [x] lib.rs-Crate-Header mit Spec + Plattform-Tabelle
- [x] README + CHANGELOG
- [x] doc-Examples in Doc-Comments

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-transport-udp                                  # ✅ 11 passed
cargo clippy -p zerodds-transport-udp --all-targets -- -D warnings   # ✅
cargo doc -p zerodds-transport-udp --no-deps                         # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README
- [x] §1.4 CHANGELOG
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (Tabelle, 0 ❌)
- [x] §1.6 Spec-Coverage
- [x] §1.7 Forbidden-Sweep
- [x] §1.8 License-Header
- [x] §1.9 Tests + Lints
- [x] §1.10 Review-Doc
- [x] §1.11 Tracker
- [x] §1.12 Mirror
- [x] §1.13 Spec-Conformance

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
