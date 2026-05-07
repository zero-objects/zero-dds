# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-security-keyexchange`-Crate.

### Spec-Referenzen

- **OMG DDS-Security 1.1** §8.3.2 (Authentication-Handshake) + §8.3.2.11 (Key-Establishment).

### Public-API

- `KeyExchange::{new, with_suite, public_key, derive_shared_secret}`.
- `Suite::{X25519, P256Ecdh}`.

### Implementierung

`KeyExchange::new` erzeugt ein ephemerales Schluesselpaar via `ring::agreement::EphemeralPrivateKey` (X25519 oder P-256-ECDH). `derive_shared_secret(&remote_pub)` ruft `ring::agreement::agree_ephemeral` und expandiert das Ergebnis via HKDF-SHA256 auf 32 byte. Beide Seiten der DH-Operation berechnen denselben SharedSecret deterministisch.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `zerodds-security` (Plugin-Trait + Errors), `ring` (Crypto-Primitives).
- **Dependents (out):** `zerodds-security-pki` (Handshake-State-Machine).
- **Feature-Flags:** `std` (default).

### Stabilitaet

Public-API + Wire-Format (Public-Key-Encoding) RC1-stabil.

### Removed (RsaKeyWrap)

Pre-Cleanup gab es ein `rsa_wrap`-Modul mit `RsaKeyWrap`-Struct. Die `wrap_secret`-Implementation war explizit ein Platzhalter ("ring 0.17 exponiert keine RSA-Encrypt-API; aktuell liefert die Funktion die Eingabe mit einer 16-byte zufaelligen Mask davor, damit Integrationstests den Call-Pfad validieren koennen"). Das war Phantom-API ohne Spec-Compliance — gedropt fuer RC1, weil:
1. 0 externe Production-Refs (nur eigene Tests).
2. X25519 + P-256-ECDH decken alle modernen Vendoren ab.
3. RSA-OAEP-Key-Transport (Spec §8.3.2.11) ist eine optionale Alternative.

Falls ein konkreter Legacy-Use-Case auftaucht, wird der Pfad ueber die `rsa`-Crate als Major-2.0-additive-Erweiterung wieder eingefuehrt.
