# RC1 Review — `zerodds-bridge-security`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 5 (Bridges) — Substrat-Crate.
> **Reviewer:** claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

Gemeinsamer Security-Layer (TLS / Auth / ACL) fuer alle sechs ZeroDDS
Bridge-Daemons (ws / mqtt / coap / amqp / grpc / corba).

## 2 Public-Strategy

- **Marker:** public
- **Begruendung:** Substrat-Crate, dependent-on durch alle sechs
  Bridge-Crates. Jeder Caller, der eine eigene Bridge bauen moechte,
  muss diese Crate nutzen.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs        # Crate-Header + Re-Exports
├── acl.rs        # §7.3 Topic-ACL (Wildcard + Group-Match)
├── auth.rs       # §7.2 Auth-Modes none|bearer|jwt|mtls|sasl
├── connection.rs # Pro-Connection TLS-Helpers (Server + Client)
├── ctx.rs        # SecurityCtx-Aggregat (Auth + ACL + TLS)
└── tls.rs        # §7.1 rustls-ServerConfig-Builder + PEM-Loader
```

### 3.2 Public-API-Surface

```rust
// acl
pub struct Acl;        pub struct AclEntry;     pub enum AclOp;
// auth
pub enum AuthMode;     pub struct AuthSubject;  pub enum AuthError;
// connection
pub struct RotatingTlsConfig;
pub fn build_client_tls_connector(...);
pub fn parse_server_name(...);
pub fn serve_tls_handshake(...);
// ctx
pub struct SecurityConfig;  pub struct SecurityCtx;  pub enum SecurityError;
pub fn authenticate(...);   pub fn authorize(...);
pub fn build_ctx(...);      pub fn extract_mtls_subject(...);
// tls
pub enum TlsConfigError;    pub fn load_server_config(...);
```

### 3.3 Tests

- `cargo test -p zerodds-bridge-security`: ✅ 42 unit + 1 e2e (TLS-Handshake) = 43 tests.
- E2E: `tests/tls_e2e.rs` spawnt Mini-Server mit Self-Signed-Cert (rcgen),
  verifiziert TLS-Handshake-Roundtrip ueber Loopback.

### 3.4 Coherence-Audit

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `Acl` / `AclEntry` / `AclOp` | §7.3 | 6 (alle Bridges via `bridge_security`-Modul) | unit | CONNECTED | — |
| `AuthMode` / `AuthSubject` / `AuthError` | §7.2 | 6 (alle Bridges) | unit | CONNECTED | — |
| `SecurityConfig` / `SecurityCtx` / `build_ctx` | §7.x Aggregat | 6 (alle Bridges) | unit | CONNECTED | — |
| `authenticate` / `authorize` / `extract_mtls_subject` | §7.2 + §7.3 | 6 (alle Bridges) | unit | CONNECTED | — |
| `RotatingTlsConfig` / `serve_tls_handshake` / `build_client_tls_connector` | §7.1 | ws + grpc + corba (Server-Acceptor); mqtt + amqp (Client-Connector) | e2e + unit | CONNECTED | — |
| `load_server_config` / `TlsConfigError` | §7.1 | indirekt via `RotatingTlsConfig::reload`, plus 2 direkte Refs (foundation-PEM-Loader-Bypass in grpc) | unit | CONNECTED | — |
| `parse_server_name` | §7.1 | mqtt + amqp Client-Connector | unit | CONNECTED | — |

Alle 7 Item-Familien CONNECTED. 0 ❌.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
rustls = { version = "0.23", default-features = false, features = ["std", "ring", "logging", "tls12"] }
rustls-pemfile = "2"
rustls-pki-types = { version = "1.11", default-features = false, features = ["std", "alloc"] }
ring = "0.17"
base64 = { version = "0.22", default-features = false, features = ["std"] }
```

### 4.2 Dependents (used-by)

`zerodds-websocket-bridge`, `zerodds-mqtt-bridge`, `zerodds-coap-bridge`,
`zerodds-amqp-endpoint`, `zerodds-grpc-bridge`, `zerodds-corba-dds-bridge`
(jeweils im `daemon`-Feature).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | Pflicht (rustls 0.23 braucht std). |

## 5 Spec-Relevanz

- **Spec:** ZeroDDS Bridge-Spec 1.0 §7.1 / §7.2 / §7.3.
- **Coverage-Doc:** Bridge-Crates teilen sich diese Doku via ihre
  jeweiligen Spec-Coverage-Files; `bridge-security` ist Substrat ohne
  eigene Spec-Coverage-Doc.
- **Abgedeckte Sektionen:** §7.1 (TLS), §7.2 (Auth-Modes), §7.3
  (Topic-ACL) — vollstaendig.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Hard-Forbidden: 0 Treffer.

### 6.2 Sprint-Marker-Sweep

`tests/tls_e2e.rs` hatte einen "Cluster-B-Auftrag"-Verweis im Doc-Header
— ersetzt durch fachliche Beschreibung "Akzeptanz-Pfad fuer Bridge-Spec
§7.1 TLS".

### 6.3 Soft-Review (TODO/FIXME/HACK)

0 Treffer.

### 6.4 Public-API-Leaks

Keine. `lib.rs` re-exportiert per gezielter `pub use a::{X, Y, Z}`-
Liste, kein Glob-Re-Export.

## 7 Cleanup-Actions

1. Cargo.toml: `keywords` + `categories` + `readme` hinzugefuegt.
2. `lib.rs`: voller RC1-Header mit Safety-Klassifikation, Spec-Ref,
   Layer-Position, Public-API-Aufzaehlung und doc-tested Beispiel.
3. `README.md` neu erstellt aus Crate-Template.
4. `CHANGELOG.md` neu erstellt mit `[1.0.0-rc.1]`-Eintrag (Initial-
   Materialisierung).
5. `tests/tls_e2e.rs` Sprint-Marker entfernt.
6. Public-Mirror unter `github/crates/bridge-security/` materialisiert.
7. `website/docs/bridge-security.md` materialisiert.
8. Tracker-Update: neuer Eintrag 5.10 → ✅ rc1-ready.

## 8 Spec-Doc-Updates

Keine. Bridge-security ist Substrat ohne eigene Coverage-Doc; die
sechs Bridge-Crates referenzieren §7.1/§7.2/§7.3 in ihren eigenen
Coverage-Docs.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig.
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer +
      API-Aufzählung + doc-tested Example.
- [x] `README.md` aus Template.
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Entry (initial-Materialisierung).
- [x] doc-tested Code-Example.

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-bridge-security      # ✅ 42 unit + 1 e2e = 43
cargo clippy -p zerodds-bridge-security --tests -- -D warnings   # ✅
cargo fmt -p zerodds-bridge-security -- --check                  # ✅
cargo doc -p zerodds-bridge-security --no-deps                   # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md aus Template
- [x] §1.4 CHANGELOG.md mit RC1-Entry
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (Tabelle in §3.4)
- [x] §1.6 Spec-Coverage-Update (n/a — Substrat-Crate)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror materialisiert
- [x] §1.13 Spec-Conformance-Audit (0 deferral-Marker)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer-Sign-off:** claude
- **Tracker-Eintrag aktualisiert:** ✅
