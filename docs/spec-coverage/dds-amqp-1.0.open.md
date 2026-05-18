# DDS-AMQP 1.0 — Open + Partial Items

Aggregat aus `dds-amqp-1.0.md` (Spec-Tag
`spec-dds-amqp-v1.0.0-beta1`). Vor jedem Audit-Lauf neu aus dem
Hauptfile generiert.

---

## Open + Partial Items

### §2.1 Endpoint Profile — Clause 7 (Mandatory TLS for PLAIN)

**Status:** `partial` — Filter-Logik (PLAIN nicht angeboten) erfüllt;
SASL-Outcome-Code beim Inbound-Reject ist intern
`UnsupportedMechanism` statt des spec-konformen `auth`-Codes (1).
OASIS AMQP 1.0 `amqp-1.0-security` §5.3.3.6 definiert nur
`ok`/`auth`/`sys`/`sys-perm`/`sys-temp` (Codes 0-4);
`UnsupportedMechanism` ist kein gültiger Wire-Code. Spec
verlangt explizit `auth` (Code 1). Aufwand 0.25 PW (Code-Variant-
Umbenennung in `Failed { reason }` + Wire-Code-Mapping +
Test-Anpassung).

## Decision-Records (`n/a (rejected)`)

### §10.2 SASL Mechanisms — Optional SCRAM-SHA-256

**Begruendung:** Spec §10.2 listet SCRAM-SHA-256 als OPTIONAL
neben den drei MANDATORY-Mechanismen PLAIN, ANONYMOUS, EXTERNAL.
Workspace hat keine HMAC/PBKDF2-Crate-Dependency; die drei
MANDATORY-Mechanismen decken die in AMQP-1.0-Brokern verbreiteten
Auth-Pfade (PLAIN+TLS, EXTERNAL via Client-Zertifikat, ANONYMOUS).
Spec §2.2 Cl. 5 erzwingt TLS fuer PLAIN, womit der SCRAM-Vorteil
gegenueber EXTERNAL+TLS gering ist.

**Impl-Auswirkung:** ~150 Zeilen pure-Rust HMAC/PBKDF2 oder
neue Workspace-Abhaengigkeit auf `pbkdf2`/`hmac`-Crates. Neuer
SCRAM-State-Machine-Pfad in `crates/amqp-endpoint/src/sasl.rs`
und `crates/amqp-endpoint/src/client.rs`. Erweiterung
`SaslMechanism::ScramSha256` plus Test-Abdeckung in
`sasl::tests`.

**Impl-Vorteil:** Vollstaendige SASL-Suite (alle vier Spec-
Mechanismen). Username-/Password-Auth ohne Klartext-Wire bei
nicht-TLS-Pfaden — wegen §2.2 Cl. 5 (PLAIN-ohne-TLS verboten)
in der Praxis ueberlappend mit EXTERNAL+TLS.
