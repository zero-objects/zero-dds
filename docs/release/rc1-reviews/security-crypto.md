# RC1 Review — `zerodds-security-crypto`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`. **Layer:** 4 (Core Services). **Reviewer:** Claude. **Public-Strategy:** 🌐 public.

## 1 Purpose

AES-GCM + HMAC `CryptographicPlugin`-Impl fuer DDS-Security 1.1 §8.5. Wrapper um `ring`. Cross-Vendor-spec-byte-kompatibel.

## 3 Content-Inventur

8 src-Files, **3104 LOC**, 80 Tests grün:
- `lib.rs`, `plugin.rs` (AesGcmCryptoPlugin), `psk_plugin.rs` (PskCryptoPlugin), `suite.rs`, `crypto_transform.rs`, `session_key.rs`, `aes_gcm_hw.rs`, `metrics.rs` (Feature `metrics`).

### 3.4 Coherence-Audit

| Public-Item | Spec-Anker | External Refs | Klassifikation |
|---|---|---|---|
| `AesGcmCryptoPlugin` | DDS-Security 1.1 §8.5 + 1.2 §10.5 | `security-runtime`, `dcps` (cfg security), end-user | CONNECTED |
| `PskCryptoPlugin` + `CLASS_ID_PSK_CRYPTO` + `HKDF_INFO_*` | Vendor-Extension fuer Out-of-Band-PSK | `security-runtime` | CONNECTED |
| `Suite` | §10.5 Tab.79 | `security-rtps`, intern | CONNECTED |
| `crypto_transform::*` | §10.5 Wire-Format | `security-rtps`, `security-runtime` | CONNECTED |
| `session_key::*` (`derive_session_key`, `compute_aad`, Tags) | §10.5.2 Tab.74 | `security-rtps`, intern | CONNECTED |
| `aes_gcm_hw::{Arch, HwCapabilities, report}` | HW-Detection | `tools/perf` (`zerodds-perf hw-info`), end-user | CONNECTED |
| `metrics::CryptoOp` | zerodds-monitor §2.5 | intern (Hot-Path RAII) | CONNECTED |

Ergebnis: **0 ❌-Klassen offen**.

## 6 Cleanup-Findings

- **Forbidden-Token-Sweep:** 0 Treffer.
- **Sprint-Marker pre-cleanup:** `WP 4.3-a/b/c`, `WP 4H-i`, `WP 4.5`, `WP 4.5-b`, `Phase-5 WP 5.D.2`, `MVP`. **Post-Cleanup:** 0.
- **No-op-Sweep:** 0 empty-bodies — clean.
- **SPDX:** alle 8 src-Files post-cleanup mit Header.
- **Doc-Lie korrigiert:** `// MVP: Serialisiert Master-Key plain. In WP 4.5 wrapped` — der Kommentar suggerierte unfertige Spec-Compliance, aber das aktuelle Verhalten IST spec-konform: Tokens werden ueber die already-encrypted DCPSParticipantVolatileMessageSecure-Topic ausgetauscht (Spec §9.5.3.5), eine zusaetzliche Wrap-Schicht waere Doppel-Encrypt. Doc-Comment auf Spec-Argumentation umgeschrieben.

## 7 Cleanup-Actions

1. **F-SECURITY-CRYPTO-1** ✅ (Sprint-Marker + Doc-Lie): `lib.rs`-Roadmap-Sektion + `aes_gcm_hw.rs`-Phase-Marker + `plugin.rs`-MVP-Comment + 4× `WP 4H-*`/`WP 4.3-*`-Marker bereinigt; Doc auf Spec-Argumentation (§9.5.3.5) verlagert.
2. **SPDX** in 8 src-Files.
3. **Cargo.toml-Metadata** komplettiert (`publish = false → true`, alle 5 Felder).
4. **README + CHANGELOG** im RC1-Format mit Suite-Tabelle, Token-Wrap-Argument, Wire-Compat-Statement.
5. **rustdoc-Links:** `[crypto_transform::*]` etc. auf `[crypto_transform]-Modul`-Form reduziert; `[Self::register_psk]` auf prosa-Verweis (Methode existiert nicht unter dem Namen).

## 10 Tests + Lints + Doc-Build

```
cargo test           ✅ 80 passed
cargo clippy --all-features --tests -- -D warnings   ✅
cargo fmt -- --check ✅
cargo doc --no-deps  ✅ (0 Warnings)
zerodds-lint check   ✅ 105 crates / 1028 files
```

## 11 RC1-DoD

Alle 13 Punkte abgehakt; **No-op-Untersuchung 0 Treffer**.

## 12 Sign-off

Crate-Version: `1.0.0-rc.1`. Reviewer: Claude.
