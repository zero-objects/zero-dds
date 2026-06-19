# 0010 — CoAP-OSCORE (RFC 8613) — accepted, implemented (supersedes 0007)

- **Status:** accepted (supersedes [0007](0007-coap-oscore-rejected-rc1.md))
- **Datum:** 2026-06-11
- **Kontext:** `crates/coap-bridge`, Spec `zerodds-coap-bridge-1.0` §7.2,
  Spec-Completeness-Programm (Gruppe F2 — „extra Meile")

## Kontext

ADR 0007 klassifizierte OSCORE für RC1 als `rejected/n/a` — begründet rein
mit fehlendem Markt-Pull (DTLS deckt 100% der adressierten Produktions-Cases).
Das war eine **RC1-Scoping-Entscheidung**, keine technische Unmöglichkeit.

Das Spec-Completeness-Programm überschreibt diese Scoping-Entscheidung
explizit: optionale Spec-Profile (hier §7.2 OSCORE) sind ein
**Differenzierungs-Feature**, kein Reject-Grund („andere Vendoren haben das
nicht" zählt nicht). Damit gilt für OSCORE wie für jeden Spec-Mechanismus:
**voll implementieren, kein Stub, kein versteckter TODO** — die in ADR 0007
verworfene Alternative 1 ist jetzt die gewählte.

## Entscheidung

OSCORE wird **voll implementiert**, beginnend mit der Security-Context-
Key-Derivation und aufbauend zur AEAD-Protect/Unprotect-Schicht. Korrektheit
wird gegen die **RFC 8613 Appendix-C-Testvektoren** byte-exakt verankert (die
maßgebliche Ground-Truth, analog zur Cross-Vendor-Capture-Methodik im
DDS-Security- und XCDR2-Stack).

## Architektur

`crates/coap-bridge/src/oscore/` (feature `oscore`, `no_std + alloc`):

| Schicht | Status | Verifikation |
|---|---|---|
| **HKDF-SHA-256** (RFC 5869) | ✅ done | RFC 5869 A.1 (PRK + OKM byte-exakt) |
| **Security Context Key Derivation** (§3.2: Sender/Recipient-Key + Common IV, CBOR-`info`) | ✅ done | RFC 8613 **C.1.1** (leere Sender-ID, `f0910ed7…`), **C.1.2** (Mirror), **C.2.1** (ID-Context-Pfad, `e39a0c7c`/`2ca58fb8`) |
| **AEAD AES-CCM-16-64-128** (COSE alg 10) | ✅ done | `ccm`-Crate; **RFC 3610 PV#1** byte-exakt |
| **Nonce (§5.2) + AAD/Enc_structure (§5.4) + protect/unprotect** | ✅ done | spec-deterministische Nonce/AAD-Vektoren + protect→unprotect-Roundtrip |
| **OSCORE-Option (9) Codec (§6.1)** | ✅ done | RFC 8613 §6.3 Option-Vektoren (encode/decode-Roundtrip) |
| **Message Protect/Unprotect (§8)** + Option-Klassen-Split (§4.1.2) | ✅ done | `oscore/message.rs`; protect→unprotect-Roundtrip (GET + Payload + Tamper-Reject) |
| **Replay-Window** (§3.2.2) | ✅ done | sliding-window unit-tests (in-window/out-of-order/replay) |

Krypto-Bausteine im Workspace vorhanden: `hmac`/`sha2` (HKDF, genutzt),
`aes`/`cipher` (AES-CCM-Substrat), `ciborium` (falls serde-CBOR nötig — die
kleinen festen `info`/`Enc_structure`-Strukturen werden derzeit deterministisch
hand-encodiert). Die einzige neue externe Abhängigkeit ist `ccm` (AES-CCM-Mode),
die `ring`/`security-crypto` (AES-GCM) NICHT abdecken; ihre Aufnahme läuft über
cargo-deny.

## Alternativen

1. **Bei ADR 0007 (rejected) bleiben** — verworfen: widerspricht dem
   Spec-Completeness-Mandat (optionale Profile sind Features).
2. **OSCORE als Stub** — verworfen (wie in 0007): keine echte Sicherheit.
3. **Voll implementieren, RFC-Testvektor-verankert** — gewählt.

## Konsequenzen

Positiv:
- §7.2 wird von `rejected` zu `implemented` — saubere Spec-Lage.
- Key-Derivation ist sofort konformitäts-bewiesen (RFC-C.1/C.2-Vektoren).
- OSCORE ist proxy-tauglich (Object-Security überlebt CoAP-Proxies, anders als
  hop-by-hop DTLS) — echtes Differenzierungs-Feature für LwM2M/Constrained-IoT.

**Vollständig** (2026-06-12): Key-Derivation + AEAD + Nonce/AAD + protect/unprotect + Option-9-Codec + Replay-Window + §8-Message-Pipeline — 20 oscore-Tests grün, RFC-verankert (RFC 5869 A.1, RFC 8613 C.1/C.2, RFC 3610 PV#1). Offen nur die Daemon-Request-Handler-**Verdrahtung** (die fertigen `protect_request_message`/`unprotect_request_message` in den CoAP-Daemon-Pfad einhängen) + DTLS (eigenes Dossier).

## Referenzen

- RFC 8613 — OSCORE (insb. §3.2 Key Derivation, Appendix C Testvektoren)
- RFC 5869 — HKDF; RFC 8152 — COSE; RFC 8949 — CBOR
- `crates/coap-bridge/src/oscore/mod.rs` — Implementierung + Testvektoren
- [0007](0007-coap-oscore-rejected-rc1.md) — vorherige (superseded) Entscheidung
