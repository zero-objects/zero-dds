# RC1 Review — `zerodds-security-rtps`

> **Layer:** 4. **Reviewer:** Claude. **Public-Strategy:** 🌐 public.

## 1 Purpose

Secure-Submessage-Wrapper (DDS-Security §7.3.6) + RTPS-Header-AAD (§9.5). Wire-Format-Adapter; Crypto delegiert an `CryptographicPlugin`.

## 3 Content-Inventur

4 src-Files, **1327 LOC**, 31 Tests grün.

### 3.4 Coherence-Audit

| Public-Item | Spec | External Refs | Klassifikation |
|---|---|---|---|
| `encode_secured_submessage`/`decode_secured_submessage` | §7.3.6 | `security-runtime`, `dcps` (cfg security) | CONNECTED |
| `*_multi` (Receiver-Specific-MACs) | §7.3.6.3 | `security-runtime` | CONNECTED |
| `srtps::*` | §9.5 | `security-runtime` | CONNECTED |
| `header_aad::*` | §7.3.5 | intern + `security-crypto` (AAD-Bind) | CONNECTED |
| Submessage-ID-Konstanten | §7.3.6 | `security-runtime`, end-user | CONNECTED |

Ergebnis: **0 ❌-Klassen**.

## 6 Cleanup-Findings

- Forbidden-Token-Sweep: 0.
- Sprint-Marker pre: `WP 4.4-a/b/c/d`, `WP 4H-g`, `MVP` (8 Treffer in `lib.rs`/`srtps.rs`/`codec.rs`). Post: **0**.
- No-op-Sweep: 0 empty bodies.
- SPDX in 4 src-Files post.

## 7 Cleanup-Actions

1. **F-SECURITY-RTPS-1** ✅: Sprint-Marker raus. lib.rs in Guardrails §1.2-Form mit voller Public-API + Single-Receiver-vs-Multi-Receiver-Beschreibung. `MVP-null`-Comment in srtps.rs umformuliert auf "Single-Plugin-Pfad" (Spec-konform). MAC-Liste-Comments zeigen jetzt auf `_multi`-Pfad statt auf Phase-Deferral.
2. SPDX in 4 src-Files.
3. Cargo.toml-Metadata + `publish=true`.
4. README + CHANGELOG.

## 10 Tests + Lints + Doc-Build

```
cargo test           ✅ 31 passed
cargo clippy --tests -- -D warnings  ✅
cargo fmt -- --check ✅
zerodds-lint check   ✅
```

## 11 RC1-DoD

Alle 13 Punkte; **No-op 0 Treffer**.

## 12 Sign-off

`1.0.0-rc.1`. Reviewer Claude.
