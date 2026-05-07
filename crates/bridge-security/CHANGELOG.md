# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).
Datums-Marker pro Eintrag sind Keep-a-Changelog-Konvention; alles
weitere git-getrackt.

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung.

### Spec-Referenzen

- ZeroDDS Bridge-Spec 1.0 §7.1 — Transport-Layer-Security (TLS).
- ZeroDDS Bridge-Spec 1.0 §7.2 — Auth-Modes `none|bearer|jwt|mtls|sasl`.
- ZeroDDS Bridge-Spec 1.0 §7.3 — Topic-ACL (Read / Write pro Topic-Pattern).

### Public-API

**Re-Exports aus `lib.rs`:**

- `Acl`, `AclEntry`, `AclOp` (Modul `acl`) — Topic-ACL mit Wildcard-
  und Group-Matching.
- `AuthError`, `AuthMode`, `AuthSubject` (Modul `auth`) — Auth-Modes
  und Subject-Datentyp mit Group-Memberships + Free-Form-Claims.
- `RotatingTlsConfig`, `build_client_tls_connector`, `parse_server_name`,
  `serve_tls_handshake` (Modul `connection`) — pro-Connection-TLS-Helpers.
- `SecurityConfig`, `SecurityCtx`, `SecurityError`, `authenticate`,
  `authorize`, `build_ctx`, `extract_mtls_subject` (Modul `ctx`) —
  Aggregat-Ctx aus Auth + ACL + TLS und die Top-Level-Helpers
  `authenticate` / `authorize`.
- `TlsConfigError`, `load_server_config` (Modul `tls`) — `rustls`-
  ServerConfig-Builder mit PEM-Cert/Key-Loader und optionalem Client-
  CA-Trust für mTLS.

### Implementierung

`load_server_config` parst PEM-Files mit `rustls-pemfile`, fuettert sie
in `rustls::ServerConfig::builder()` und unterstuetzt drei Pfade: nur
Server-Cert, Server-Cert mit Client-CA-Trust (mTLS), und das Reload-
Pattern `RotatingTlsConfig::reload()` fuer SIGHUP-getriggerte
Cert-Rotation ohne Connection-Drop.

`AuthMode::validate` ist die Single-Entry-Stelle pro Connection: bekommt
das Wire-spezifische Material (HTTP-Header, MQTT-Connect-Properties,
SASL-PLAIN-Tokens, mTLS-Peer-Cert) und produziert deterministisch ein
`AuthSubject` oder einen `AuthError`. JWT-Validation laeuft ueber `ring`
(RS256-Signature) und vermeidet damit eine zweite Crypto-Dep neben
rustls.

`Acl` matched in zwei Stufen: Subject-Name (Wildcard `*`) und
Group-Membership; pro Topic kann jede Stufe Read oder Write erlauben.

### Architektur

- **Layer:** 5 (Bridges) — gemeinsamer Substrat-Layer fuer alle sechs
  Bridge-Daemons.
- **Dependencies (in):** `rustls 0.23` (mit `ring`-Backend, kein
  `aws-lc-rs`), `rustls-pemfile`, `rustls-pki-types`, `ring`, `base64`.
  Keine ZeroDDS-Internals.
- **Dependents (out):** `zerodds-websocket-bridge`, `zerodds-mqtt-bridge`,
  `zerodds-coap-bridge`, `zerodds-amqp-endpoint`, `zerodds-grpc-bridge`,
  `zerodds-corba-dds-bridge` — alle als optional Dep im `daemon`-Feature.
- **Feature-Flags:**
  | Flag | Default | Zweck |
  |------|---------|-------|
  | `std` | ✅ | Pflicht (rustls 0.23 braucht std). |

### Stabilitaet

- Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump.
- Neue `AuthMode`-Diskriminanten oder neue `AclOp`-Varianten sind
  Major-additive (rustc lehnt non-exhaustive ohne `#[non_exhaustive]`
  ab — wir behalten exhaustive Match-Syntax bei und bumpen Major bei
  Erweiterung).
