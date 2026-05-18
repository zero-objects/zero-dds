# RC1 Review — `zerodds-security-keyexchange`

> **Layer:** 4 (Core Services). **Reviewer:** Claude. **Public-Strategy:** 🌐 public.

## 1 Purpose

Ephemeral-Diffie-Hellman Key-Agreement (X25519 + P-256-ECDH) fuer den DDS-Security 1.1 Authentication-Handshake (§8.3.2). Wrapper um `ring::agreement` + `ring::hkdf`.

## 3 Content-Inventur

1 src-File `lib.rs` (~330 LOC nach Drop), **11 + 1 Tests** grün.

### 3.4 Coherence-Audit

| Public-Item | Spec | External Refs | Klassifikation |
|---|---|---|---|
| `KeyExchange` + `KxSuite::{X25519, EcdhP256}` | DDS-Security 1.1 §8.3.2 | `security-pki` (Handshake-State-Machine) | CONNECTED |

Ergebnis: **0 ❌-Klassen**.

## 6 Cleanup-Findings

- **Forbidden-Token-Sweep:** 0.
- **Sprint-Marker pre:** `WP 4.5-a`, `WP 4.5-b`, `WP 4.5-c`, `WP 4.1-b`. **Post:** 0.
- **No-op-Sweep:** 0.
- **SPDX:** lib.rs post mit Header.
- **Phantom-API entfernt:** `rsa_wrap`-Modul + `RsaKeyWrap`-Struct gedropt — `wrap_secret` war explizit "platzhaltert"-Stub ("ring 0.17 exponiert keine RSA-Encrypt-API; aktuell liefert die Funktion die Eingabe mit einer 16-byte zufaelligen Mask davor"). 0 externe Refs. RSA-OAEP ist Spec-§8.3.2.11 optional Alternative; alle relevanten Vendoren (Cyclone/FastDDS/RTI) sprechen ECDH/X25519. Re-Add via `rsa`-Crate als Major-2.0-additive moeglich.

## 7 Cleanup-Actions

1. **F-SECURITY-KEYEXCHANGE-1** ✅ (Phantom-API + Sprint-Marker): `rsa_wrap.rs` (110 LOC) gedropt; lib.rs-Roadmap-Sektion durch "Nicht-Ziele"-Block mit Major-2.0-Re-Add-Pfad ersetzt; SPDX in lib.rs.
2. Cargo.toml-Metadata komplettiert (`publish = false → true`).
3. README + CHANGELOG im RC1-Format mit Suite-Tabelle + Removal-Doc.

## 10 Tests + Lints + Doc-Build

```
cargo test           ✅ 11 + 1 doc passed
cargo clippy --tests -- -D warnings   ✅
cargo fmt -- --check ✅
cargo doc --no-deps  ✅ (0 Warnings)
zerodds-lint check   ✅ 105 crates / 1027 files
```

## 11 RC1-DoD

Alle 13 Punkte abgehakt; **No-op-Untersuchung 0 Treffer** (post-Drop).

## 12 Sign-off

Crate-Version `1.0.0-rc.1`. Reviewer Claude.
