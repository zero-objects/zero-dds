# Sicherheit

Diese Seite beschreibt, wie Sicherheit in ein ZeroDDS-System eingebaut wird: (a) DDS-Security in der DDS-Schicht mit den Crates `crates/security*`, (b) die Sicherheitsschicht der Bridge-Daemons in `crates/bridge-security`, (c) je eine Checkliste für ein DDS-System und für eine Bridge. Grundlage ist der Code. Aussagen, die nur aus einem Doku-Kommentar stammen, sind als „(laut Doku-Kommentar)“ markiert. Nichts auf dieser Seite wurde für diese Seite gebaut oder ausgeführt.

Verwandte Seiten: [README.md](README.md) · [architecture.md](architecture.md) · [crates-core.md](crates-core.md) · [crates-bridges-services.md](crates-bridges-services.md) · [crates-languages-codegen.md](crates-languages-codegen.md) · [config.md](config.md) · [cli-options.md](cli-options.md) · [tools-and-man.md](tools-and-man.md) · [system-setup.md](system-setup.md) · [security.md](security.md) · [migration.md](migration.md) · [interop-and-dialects.md](interop-and-dialects.md) · [ros2.md](ros2.md)

## Kurzfassung

- DDS-Security ist ein Opt-in-Feature: `zerodds-dcps` mit `features = ["security"]` bzw. `zerodds-c-api` mit `--features security`. Ohne das Feature läuft RTPS im Klartext (`crates/dcps/Cargo.toml`).
- Ein Teilnehmer wird auf drei Wegen gesichert: `ParticipantBuilder::with_security(SecurityBundle)` (Rust), `dds.sec.*`-Properties in der `DomainParticipantQos` (Rust), oder C-API `zerodds_runtime_create_secure` / `zerodds_runtime_create_secure_from_env` mit einer SROS2-Enklave (`security.enclave_dir`).
- Governance und Permissions werden per CMS/S/MIME mit dem Permissions-CA-Zertifikat signiert: `zerodds-secure-permissions sign` oder `openssl smime -sign -text`.
- Bridges: TLS (bzw. DTLS, SSLIOP), Auth-Modus pro Daemon, Topic-ACL mit Default „deny“, sobald ein Auth-Modus gesetzt ist. SIGHUP lädt TLS-Material und ACL neu.

## (a) DDS-Security in der DDS-Schicht

### Crates und Plugins

| Crate | Aufgabe | Spezifikation (laut Crate-Doku) |
|---|---|---|
| `zerodds-security` | Plugin-SPI: Traits `AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`, `LoggingPlugin`, `DataTaggingPlugin`, Token, Generic Messages | DDS-Security §8.3–§8.7 |
| `zerodds-security-pki` | Authentication PKI-DH: X.509 (RSA-PSS, ECDSA P-256/P-384, Ed25519), Handshake-Token, OCSP-Stapling, CRL, Delegationsketten, PSK-Variante `PskAuthenticationPlugin`; Prüfungen über `rustls-webpki` | §8.3, §9.3, §10.7 |
| `zerodds-security-keyexchange` | ephemeres ECDH für den Handshake (P-256, P-384, X25519), HKDF-SHA256 | §8.3.2 |
| `zerodds-security-permissions` | Access Control: Parser für Governance- und Permissions-XML, CMS/PKCS#7-Prüfung (`cms`), Topic-Wildcards `*`/`?`, Delegationsprüfung, PSK-Access | §9.4 |
| `zerodds-security-crypto` | Cryptographic: AES-128-GCM (Kind 0x01, Default), AES-256-GCM (0x02), HMAC-SHA256 nur Authentisierung (0x03, für `SIGN`), empfängerspezifische MACs, Session-Keys | §8.5, §10.5 |
| `zerodds-security-rtps` | Wire-Format: `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX`, `SRTPS_PREFIX`/`SRTPS_POSTFIX`, RTPS-Header-AAD | §7.3.6, §9.5 |
| `zerodds-security-logging` | Logging-Plugins: `StderrLoggingPlugin`, `JsonLinesLoggingPlugin`, `SyslogLoggingPlugin` (RFC 5424 UDP, `LOCAL0`), `FanOutLoggingPlugin` | §8.6 |
| `zerodds-security-runtime` | Verdrahtung: `SecurityGate`/`SharedSecurityGate`, `GovernancePolicyEngine`, `SecurityProfile`, `SecurityBundle`, Peer-Capabilities, Data Tagging, Anti-Squatter, `<peer_class>`-Abgleich | – |

Schlüsselvereinbarung, geprüft in `crates/security-runtime/src/qos_properties.rs` (`kx_suite_from_properties`): ohne Angabe `ECDHE-CEUM+P256`; zulässig `ECDHE-CEUM+P384` und `X25519`; alles andere wird abgelehnt. Auf dem Draht sendet `security-pki` `ECDH+prime256v1-CEUM` (laut Doku-Kommentar, nach Mitschnitten von Cyclone DDS und Fast DDS).

Der private Schlüssel wird als PKCS#8 (`BEGIN PRIVATE KEY`), PKCS#1 (`BEGIN RSA PRIVATE KEY`) oder SEC1 (`BEGIN EC PRIVATE KEY`) gelesen (`crates/security-pki/src/identity.rs`). Mit gesetztem `dds.sec.auth.password` wird ein verschlüsselter Schlüssel geöffnet (`crates/security-pki/src/encrypted_key.rs`): PKCS#8 `BEGIN ENCRYPTED PRIVATE KEY` mit dem Passwort als Passphrase, die OpenSSL-Form (`Proc-Type: 4,ENCRYPTED`, AES-128/192/256-CBC) mit dem Passwort zuerst als Base64-AES-128-Schlüssel nach DDS-Security 1.2 §10.3.1 Table 50 und danach als OpenSSL-Passphrase, wie Cyclone DDS und Fast DDS es lesen.

### Teilnehmer konfigurieren

Weg 1, Rust mit `SecurityBundle` (`crates/dcps/src/factory.rs`, `crates/security-runtime/src/bundle.rs`, `profile.rs`):

```rust
use zerodds_dcps::DomainParticipantFactory;
use zerodds_security_runtime::{SecurityBundle, SecurityProfile};

let profile = SecurityProfile::from_enclave_dir("/etc/zerodds/certs/enclave", 0, candidate_guid)?;
let bundle = SecurityBundle::builder()
    .security_profile(profile)
    .logging_plugin(Box::new(logger))
    .build();
let participant = DomainParticipantFactory::create(0)
    .with_security(bundle)
    .build()?;
```

- Alternativ `SecurityProfile::from_files(&SecurityProfileConfig { domain_id, identity_ca_pem, identity_cert_pem, identity_key_pem, permissions_ca_pem, governance_p7s, permissions_p7s }, guid)`.
- `RuntimeConfig::with_security_bundle` setzt Gate, Logger und das aus dem Identitätszertifikat abgeleitete GUID-Präfix (DDS-Security §9.3.3). Ohne dieses Präfix lehnt die Gegenseite den Handshake ab (laut Doku-Kommentar in `crates/dcps/src/runtime.rs`).

Weg 2, Properties in `DomainParticipantQos.property` (`crates/dcps/src/participant.rs:323`, `crates/security-runtime/src/qos_properties.rs`):

| Property | Inhalt |
|---|---|
| `dds.sec.auth.identity_ca` | Identity-CA (PEM) |
| `dds.sec.auth.identity_certificate` | eigenes Zertifikat |
| `dds.sec.auth.private_key` | eigener Schlüssel (PKCS#8, PKCS#1 oder SEC1; mit Passwort verschlüsselt) |
| `dds.sec.auth.password` | öffnet einen verschlüsselten Schlüssel; leer = keiner |
| `dds.sec.auth.key_establishment_algorithm` | siehe oben |
| `dds.sec.access.permissions_ca` | Permissions-CA |
| `dds.sec.access.governance` | signierte Governance (`.p7s`) |
| `dds.sec.access.permissions` | signierte Permissions (`.p7s`) |
| `dds.sec.log.plugin` | `stderr`, `jsonl`, `syslog` (Komma-Liste) |
| `org.zerodds.sec.log.level` | Default `Informational` |
| `org.zerodds.sec.log.jsonl.path`, `org.zerodds.sec.log.syslog.addr`, `org.zerodds.sec.log.syslog.app`, `org.zerodds.sec.log.syslog.host` | Sink-Parameter |
| `org.zerodds.sec.auth.accept_x509v1_identity_certificate` | Abweichung von §10.3.1.3, Default aus |

- Werte sind URIs. `file:` wird aufgelöst, `data:,` ist der Inhalt selbst (RFC 2397) und `pkcs11:` benennt ein Objekt auf einem PKCS#11-Token (RFC 7512, DDS-Security 1.2 §9.3.1 Tabelle 44). `pkcs11:` braucht das Crate-Feature `pkcs11`; ohne das wird der Wert mit `SecurityPropertyError::Pkcs11FeatureDisabled` abgelehnt.
- Sind Properties vorhanden und fehlt eine Datei oder ist ein Wert ungültig, schlägt das Anlegen des Teilnehmers fehl (`DdsError::SecurityConfiguration`). Es gibt keinen Rückfall auf Klartext.
- Hat die `RuntimeConfig` bereits ein Gate (Weg 1), werden die Properties nicht ausgewertet.

Weg 3, C-API (`crates/zerodds-c-api/src/security_ffi.rs`):

```c
ZeroDdsSecurityConfig *cfg = zerodds_security_config_create();
zerodds_security_set_identity_ca_path(cfg, "/etc/zerodds/certs/identity_ca.pem");
zerodds_security_set_identity_cert_path(cfg, "/etc/zerodds/certs/cert.pem");
zerodds_security_set_private_key_path(cfg, "/etc/zerodds/certs/key.pem");
zerodds_security_set_permissions_ca_path(cfg, "/etc/zerodds/certs/permissions_ca.pem");
zerodds_security_set_governance_path(cfg, "/etc/zerodds/certs/governance.p7s");
zerodds_security_set_permissions_path(cfg, "/etc/zerodds/certs/permissions.p7s");
ZeroDdsRuntime *rt = zerodds_runtime_create_secure(0, cfg);   /* NULL + stderr bei Fehler */
zerodds_security_config_destroy(cfg);                         /* cfg wird nicht übernommen */
```

- Alle sechs Pfade sind Pflicht.
- `zerodds_runtime_create_secure_from_env(domain)` liest `security.enclave_dir` (env `ZERODDS_SECURITY_DIR`) aus der Laufzeitkonfiguration. Ist kein Verzeichnis gesetzt, liefert die Funktion NULL; ein gesetztes Verzeichnis, das nicht lädt, ist ein Fehler.
- Build: `cargo build -p zerodds-c-api --release --features security` (bzw. `fips`, `wolfcrypt`).

### Enklave-Verzeichnis (SROS2-Layout)

`SecurityProfile::from_enclave_dir` erwartet diese sechs Dateinamen (`crates/security-runtime/src/profile.rs:583`):

| Datei | Rolle |
|---|---|
| `identity_ca.cert.pem` | Identity-CA |
| `cert.pem` | Identitätszertifikat |
| `key.pem` | privater Schlüssel (PKCS#8) |
| `permissions_ca.cert.pem` | Permissions-CA |
| `governance.p7s` | signierte Governance |
| `permissions.p7s` | signierte Permissions |

Leser von `security.enclave_dir`: die ROS-2-Shim (`crates/rmw-zerodds-shim/src/lib.rs:765`, Feature `security`) und `zerodds_runtime_create_secure_from_env`. Der Rust-Weg `DomainParticipantFactory::create(..).build()` liest den Schlüssel nicht; dort wird `from_enclave_dir` selbst aufgerufen (abgeleitet: keine weitere Fundstelle von `security_enclave_dir`).

### Laufzeitschlüssel mit Sicherheitsbezug

| Schlüssel | Umgebung | Wirkung |
|---|---|---|
| `security.enclave_dir` | `ZERODDS_SECURITY_DIR` | Enklave für Shim und C-API, siehe oben |
| `security.secure_spdp` | `ZERODDS_SECURE_SPDP` | zuverlässiger Secure-SPDP-Kanal (Entity-IDs 0xff0101c2/c7), den ein gesicherter Teilnehmer ankündigt. Default an. Fast DDS ordnet sichere SEDP-Endpunkte eines Peers erst zu, wenn dessen Teilnehmerdaten über diesen Kanal kamen. |
| `transport.interface_bindings` | – (nur Datei) | Liste von Maps `name`, `bind`, `kind` (`loopback`, `localhost`, `wan`, `subnet`, `named:<class>`), `subnet`, `default`. Nur mit Feature `security`, sonst Startfehler. Grundlage für `<peer_class>`-Regeln (heterogene Sicherheit). |
| `discovery.multicast_allowlist` | `ZERODDS_MULTICAST_ALLOWLIST` | nur diese Multicast-Gruppen beitreten |

### Zertifikate erzeugen und XML signieren

Im Repository vorhandene Werkzeuge und Skripte:

| Fundstelle | Zweck |
|---|---|
| `tools/secure-permissions` (`zerodds-secure-permissions`) | `sign --signer-cert <ca.pem> --signer-key <ca_key.pem> --in <doc.xml> --out <doc.p7s>`; `verify --ca <permissions_ca.pem> --p7s <doc.p7s> [--out <doc.xml>]`. Ausgabe ist S/MIME `signed-data`, vor dem Schreiben gegen den eigenen Prüfer verifiziert. Signerschlüssel muss PKCS#8 sein. |
| `tests/perf/dds-roundtrip-bench/security/gen.sh` | Test-CA, Identitätszertifikate `ping`/`pong`, Signatur von `governance.xml` und `permissions_{ping,pong}.xml` mit `openssl smime -sign -text -signer permissions_ca.pem`. `KEY_ALGO=RSA` für Fast DDS. Nur für Tests (unverschlüsselte Schlüssel). |
| `tests/perf/dds-roundtrip-bench/security-matrix/gen_profile.sh` | Governance-Profile (Schutzarten pro Kategorie) und pro Hersteller signierte Permissions |
| `crates/security-permissions/tests/fixtures/generate_fixtures.sh` | Positiv- und Negativ-Fixtures für den CMS-Prüfer (falsche CA, manipuliert, abgelaufen, Zwischen-CA, RSA-PKCS#1) |
| `crates/zerodds-c-api/tests/security_ffi_e2e.rs` | erzeugt die Kette zur Laufzeit per `openssl`, überspringt ohne `openssl` mit Meldung |

Beispiel-XML:

- Governance: `tests/perf/dds-roundtrip-bench/security/governance.xml`, `zerodds-examples/security-governance-gate/governance.xml`
- Permissions: `tests/perf/dds-roundtrip-bench/security/permissions_ping.xml`, `permissions_pong.xml`, `crates/security-permissions/tests/fixtures/permissions.xml`
- Beispielprogramme: `zerodds-examples/security-governance-gate`, `zerodds-examples/rust-security-plugin-chain`, `zerodds-examples/rust-audit-securitybundle-realpart`

Regeln für Assets, die alle getesteten Hersteller annehmen (laut `tests/perf/dds-roundtrip-bench/security-matrix/README.md`):

- CMS-Signer ist das Permissions-CA-Zertifikat selbst, kein separates EE-Zertifikat.
- `openssl smime -sign -text`.
- Governance nach XSD `20170901` mit strikter Reihenfolge der `<xs:sequence>`.

### Kryptographie-Backends

| Feature | Wirkung | Fundstelle |
|---|---|---|
| `ring-backend` | Default in `security-pki`, `-permissions`, `-crypto`, `-keyexchange`, `-runtime` | jeweilige `Cargo.toml` |
| `fips` (`zerodds-dcps`, `zerodds-c-api`, `zerodds-security-runtime`) | aws-lc-rs (AWS-LC) als aktive Kryptographie. `ring` bleibt gelinkt (laut Kommentar in `crates/dcps/Cargo.toml`, „D1a limitation“). | `crates/dcps/Cargo.toml` |
| `wolfcrypt` (`zerodds-dcps`, `zerodds-c-api`, `zerodds-security-runtime`) | wolfCrypt über `wolfcrypt-ring-compat`; braucht `libwolfssl` bzw. `WOLFSSL_SRC` (wolfSSL 5.9.x) beim Build | `crates/dcps/Cargo.toml`, `crates/security-permissions/Cargo.toml:72` |
| `aws-lc` (`zerodds-bridge-security`) | aws-lc-rs für JWT-Prüfung und rustls-Provider der Bridges; genau ein Backend erzwungen (`compile_error!`) | `crates/bridge-security/src/lib.rs` |

Die sechs Bridge-Crates reichen das Backend als eigene Features weiter: `ring-backend` (Default) und `aws-lc`. Ein FIPS-Build ist `--no-default-features --features <Default-Liste ohne ring-backend>,aws-lc`; alle sechs bestehen ihre Tests so (lokal gemessen). `ring` bleibt über das Workspace-Feature von `rustls` gelinkt, TLS und JWT laufen über aws-lc-rs.

### Getestete Interop

Tests im Repository:

| Test | Inhalt | Lauf |
|---|---|---|
| `crates/dcps/tests/security_matrix_e2e.rs` | ZeroDDS↔ZeroDDS: jede Suite, jede ProtectionKind pro Kategorie, beide Auth-Verfahren, beide Datenrepräsentationen | Linux (Multicast-Loopback) |
| `crates/dcps/tests/security_live_e2e.rs` | Handshake und Crypto-Token-Austausch über echtes UDP-Multicast | Linux |
| `crates/dcps/tests/security_{bundle,endpoint_access,guid_wiring,log_properties,qos_properties,topic_rule_wire}*.rs` | Verdrahtung, Endpoint-Zugriffsprüfung, GUID-Ableitung, Properties | – |
| `crates/zerodds-c-api/tests/security_ffi_e2e.rs` | C-API, EE- und CA-signierte Assets, volle Absperrung, Ablehnung ohne Domain-Regel | braucht `openssl` |
| `crates/security-runtime/tests/cyclone_live_security_caps.rs` | Cyclone DDS verwirft SPDP-Beacons mit ZeroDDS-Capabilities nicht | `--ignored`, braucht Cyclone-Host |
| `tests/perf/dds-roundtrip-bench/security-matrix/run_deep_matrix.sh` | 13 Governance-Profile × 4×4 Hersteller (Cyclone DDS, Fast DDS, OpenDDS, ZeroDDS), beide Rollen | Linux mit Hersteller-Stack, `libzerodds.so` mit `--features security` |

Ergebnisse stehen nicht im Code. Laut `internal/security/cross-vendor-interop-configs.md` und `internal/security/opendds-secure-matrix-closeout.md`: ZeroDDS↔Cyclone DDS 13/13, ZeroDDS↔Fast DDS 13/13, ZeroDDS↔OpenDDS 9/13 Profile. Nachgemessen mit allen 13 Profilen und EC-Identitäten: Fast DDS 3.6.1 nimmt EC-Identitäten an; RSA ist nicht nötig.

## (b) Bridge-Sicherheit

### Gemeinsame Schicht `crates/bridge-security`

Genutzt von: `websocket-bridge`, `mqtt-bridge`, `coap-bridge`, `grpc-bridge`, `amqp-endpoint`, `corba-dds-bridge` (`Cargo.toml`-Abhängigkeiten).

| Teil | Verhalten (geprüft in `ctx.rs`, `acl.rs`, `auth.rs`) |
|---|---|
| TLS | `tls_cert` und `tls_key` nur gemeinsam; mit `client_ca` wird ein Client-Zertifikat verlangt (mTLS). rustls-ServerConfig. |
| Auth-Modi | `none`, `bearer` (Token → Subjekt), `jwt` (nur RS256), `mtls`, `sasl` (SASL-PLAIN, Benutzer → Passwort). Unbekannter Modus: Startfehler. |
| JWT | `auth.jwt.public_key` (PEM, PKCS#1 oder SubjectPublicKeyInfo; Pflicht), `auth.jwt.issuer` (`iss`), `auth.jwt.audience` (`aud`), `auth.jwt.required_claim` (`claim=wert`). `exp`/`nbf` mit 60 s Toleranz (`JWT_CLOCK_LEEWAY_SECS`). Subjekt aus `sub`, Gruppen aus Claim `groups`. |
| mTLS-Subjekt | Name = Subject-DN des Leaf-Zertifikats als RFC-4514-Zeichenkette (`CN=alice,O=Example`), zusätzlich Fingerprint `mtls:<SHA-256 des DER, hex>`. Leerer oder nicht lesbarer DN: nur der Fingerprint. Eine ACL-Regel darf DN oder Fingerprint nennen (`mtls_subject_from_der`). |
| Topic-ACL | pro Topic `read`/`write`; Muster `*`, exakter Name (oder Fingerprint), `*group:<g>*`. Topic ohne Eintrag: Default-Eintrag, sonst Ablehnung. |
| ACL-Default | Keine Regeln und Modus `none`: alles erlaubt. Keine Regeln und anderer Modus: alles abgelehnt. Mit Regeln: abgelehnt, was keine Regel erlaubt. |
| Reload | `LiveSecurity` ersetzt den Kontext bei SIGHUP als Ganzes. Neue Verbindungen erhalten die neue TLS-Konfiguration, offene Verbindungen behalten ihre Sitzung. |
| Backend | `ring-backend` (Default) oder `aws-lc` |

Im Konfigurationsbericht (`--check-config`) wird jeder Wert als `***` ausgegeben, dessen Pfad ein Segment mit `token`, `password`, `secret`, `passphrase`, `psk`, `sasl_users` oder `credentials` enthält, z. B. `auth.bearer_token`, `mqtt.password`, `oscore.master_secret_hex`, `coap.dtls.psk` (`is_secret_path`, `crates/config/src/layers.rs:504`). Pfade zu Schlüsseldateien (`tls.key_file`) gelten nicht als Geheimnis.

### Unterschiede pro Daemon

| Daemon | Transportschutz | `auth.mode` (geprüft im Parser) | ACL-Form | SIGHUP |
|---|---|---|---|---|
| `zerodds-ws-bridged` | `tls.enabled`, `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` | `none`, `bearer`, `jwt`, `mtls` (`crates/websocket-bridge/src/daemon/config.rs:201`) | pro Topic `acl.read`/`acl.write` | ja |
| `zerodds-grpc-bridged` | `grpc.tls.*`, `client_ca_file`, `require_client_cert` | `none`, `bearer`, `jwt`, `mtls`, pro Call geprüft | pro Topic `acl.publish`/`acl.subscribe` | ja (TLS, Auth, ACL) |
| `zerodds-mqtt-bridged` | `mqtt.tls.*` zum Broker (`ca_file`, `cert_file`, `key_file`, `server_name`) | `none`, `bearer`, `sasl`, `sasl_plain`, `mtls` (`crates/mqtt-bridge/src/daemon/config.rs:294`) | `acl.default_deny`, `acl.rules[].subject`, `allow_publish`, `allow_subscribe` | ja |
| `zerodds-coap-bridged` | `coap.dtls.*` (Zertifikat oder `psk[].identity`/`secret_hex`, `cipher_suites`), `oscore.*` (RFC 8613) | `none`, `bearer`, `jwt`, `mtls` (`crates/coap-bridge/src/daemon/config.rs:376`) | `acl.rules[].allow_post` (write), `allow_observe` (read); Subjekt = PSK-Identität oder Zertifikats-DN | ja |
| `zerodds-amqp-bridged` | `amqp.tls.*`, `verify_hostname` (nur `true`), `alpn` | SASL zum Broker: `amqp.sasl.mechanism` = `PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2` (`crates/amqp-endpoint/src/bridge_daemon.rs:349`) | `acl.rules[].topic`, `read`, `write` (SASL-Subjekte) | ja (`crates/amqp-endpoint/src/daemon_runtime.rs`) |
| `zerodds-corba-bridged` | `corba.ssliop.*` (`bind`, `cert_file`, `key_file`, `ca_file`, `require_client_cert`) | kein `auth.mode`; Subjekt aus Client-Zertifikat | `acl.rules[].subject`, `allow_invoke` (Repository-IDs mit `*`) | ja (ACL, SSLIOP) |

CoAP-Besonderheiten:

- `dtls` und `oscore` sind Cargo-Features von `zerodds-coap-bridge` und nicht im Default (`crates/coap-bridge/Cargo.toml`). Ein Build ohne Feature bricht den Start ab, wenn `coap.dtls.enabled: true` bzw. `oscore.enabled: true` gesetzt ist (`crates/coap-bridge/src/daemon/server.rs:377`, `:417`).
- DTLS 1.2 über `webrtc-dtls`, im Code als experimentell gekennzeichnet (ADR 0011).
- `oscore.enabled: true` ohne `master_secret_hex` bricht den Start ab (laut Beispielkonfiguration).

CORBA-Besonderheiten: `corba.csiv2.enabled: true` und `corba.ior.publish_to_naming: true` werden abgelehnt, da nicht gebaut (laut `packaging/linux/configs/corba-bridged.yaml.example`).

Bearer: Im WebSocket- und gRPC-Beispiel ist `bearer` ein einzelnes Subjekt (`auth.bearer_token` + `auth.bearer_subject`); für mehrere Benutzer ist `jwt` vorgesehen (Kommentar in `ws-bridged.yaml.example`).

Tests: `crates/{websocket-bridge,mqtt-bridge,coap-bridge,grpc-bridge,amqp-endpoint,corba-dds-bridge}/tests/security_e2e.rs`, zusätzlich `crates/grpc-bridge/tests/daemon_security_e2e.rs`.

## (c) Sicherheit einrichten, Schritt für Schritt

### DDS-System

1. Build mit Sicherheit: `zerodds-dcps` mit `features = ["security"]` (Rust) oder `cargo build -p zerodds-c-api --release --features security`. Für FIPS `fips`, für wolfCrypt `wolfcrypt`. Für ROS 2: `rmw-zerodds-shim` mit `--features security`.
2. Zwei CAs festlegen: Identity-CA (signiert Teilnehmerzertifikate) und Permissions-CA (signiert Governance und Permissions). In Tests ist beides dieselbe CA (`gen.sh`).
3. Pro Teilnehmer Schlüssel und Zertifikat erzeugen. Schlüssel als PKCS#8 (`openssl genpkey`), ohne Passphrase. EC P-256 für Cyclone DDS, OpenDDS, ZeroDDS; RSA-2048, wenn Fast DDS beteiligt ist (Kommentar in `gen.sh`).
4. Governance-XML schreiben: pro Domain `rtps_protection_kind`, `discovery_protection_kind`, `liveliness_protection_kind`, `enable_join_access_control`; pro Topic `metadata_protection_kind`, `data_protection_kind`, `enable_discovery_protection`, `enable_liveliness_protection`, `enable_read_access_control`, `enable_write_access_control` (Zuordnung nach DDS-Security §9.4.1.2; Elementnamen aus `crates/security-permissions/src/governance.rs`). Vorlage: `tests/perf/dds-roundtrip-bench/security/governance.xml`.
5. Permissions-XML pro Teilnehmer: `<grant>` mit Subject-Name des Zertifikats, `<allow_rule>` mit `<publish>`/`<subscribe>` und `<topic>`. Vorlage: `permissions_ping.xml`.
6. Beide XML signieren:
   ```sh
   zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
       --in governance.xml --out governance.p7s
   zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
       --in permissions.xml --out permissions.p7s
   zerodds-secure-permissions verify --ca permissions_ca.pem --p7s permissions.p7s
   ```
7. Dateien unter den SROS2-Namen in ein Enklave-Verzeichnis legen (Tabelle oben), z. B. unter `/etc/zerodds/certs/` (von `zerodds-tmpfiles.conf` mit Modus 0700 angelegt).
8. Teilnehmer verdrahten: Rust `SecurityProfile::from_enclave_dir` + `SecurityBundle` + `DomainParticipantFactory::create(d).with_security(bundle)`, oder `dds.sec.*`-Properties, oder C-API mit `security.enclave_dir` und `zerodds_runtime_create_secure_from_env`.
9. Sicherheitsereignisse protokollieren: `dds.sec.log.plugin: "stderr,jsonl"` mit `org.zerodds.sec.log.jsonl.path`, oder `logging_plugin` im Bundle.
10. Mit Fast DDS ist nichts weiter einzustellen: `security.secure_spdp` ist an.
11. Prüfen: `--check-config` für die Laufzeitschlüssel; Handshake-Fehler erscheinen im Security-Log. Referenzlauf: `crates/dcps/tests/security_live_e2e.rs` auf Linux.

### Bridge

1. Zertifikat und Schlüssel des Daemons nach `/etc/zerodds/certs/` legen, Eigentümer `zerodds`.
2. Transportschutz im Daemon-YAML einschalten (`tls.enabled: true`, bei CoAP `coap.dtls.enabled: true` mit Build-Feature `dtls`, bei CORBA `corba.ssliop.enabled: true`).
3. Für mTLS die Client-CA setzen (`tls.client_ca_file`, `grpc.tls.client_ca_file` + `require_client_cert`, `corba.ssliop.ca_file` + `require_client_cert`) und `auth.mode: mtls`.
4. Sonst Auth-Modus wählen: `bearer` mit `auth.bearer_token` (Wert über Umgebung, z. B. `${WS_BRIDGE_TOKEN}`), `jwt` mit `auth.jwt.public_key` und `issuer`/`audience`/`required_claim`, `sasl` (MQTT) mit `auth.sasl_users`.
5. Für jedes Topic eine ACL anlegen. Bei `auth.mode` ungleich `none` wird ein Topic ohne Regel abgelehnt. Subjekte: Benutzername, JWT-`sub`, `*group:<g>*`, mTLS-DN (`CN=…,O=…`) oder `mtls:<sha256>`.
6. Konfiguration prüfen: `zerodds-<x>-bridged --config /etc/zerodds/<x>-bridged.yaml --check-config` (Geheimnisse erscheinen als `***`).
7. Metrikendpunkt nur auf `127.0.0.1` binden (Default der Beispiele).
8. Zertifikat erneuern oder ACL ändern: Dateien ersetzen, dann `systemctl reload zerodds-<x>-bridged` (SIGHUP).
9. Referenztests: `crates/<bridge>/tests/security_e2e.rs`.

## Widersprüche und offene Punkte

- `security.secure_spdp` an, gemessen mit der Security-Tiefenmatrix (`tests/perf/dds-roundtrip-bench/security-matrix/`, 13 Governance-Profile, EC-Identitäten, beide Richtungen): ZeroDDS mit Fast DDS 13/13, mit Cyclone DDS 13/13, mit OpenDDS 9/13 (die Grenzen von OpenDDS), mit ZeroDDS 13/13.
- `discovery.server` hat laut Doku-Kommentar kein Relay für DDS-Security-gesicherte Discovery.

Behoben: `zerodds-examples/rust-audit-securitybundle-realpart` führt die Kette `DomainParticipantFactory::create(0).with_security(bundle).build()` aus (`crates/dcps/src/factory.rs:445`), statt ihr Fehlen zu behaupten; SROS2 gegen einen gesicherten `rmw_cyclonedds_cpp`-Listener (zwei Ausrichtungsfehler: `SEC_BODY` und die verschlüsselte Payload auf 4 Byte aufgefüllt, `d6fe961eaca`, `bdbd9107f93`; 13 Nachrichten gemessen), Versionsangaben auf DDS-Security 1.2 mit deren Nummerierung, die Auth-Modi im ws-Beispiel, die Aussagen der Beispielkonfigurationen zu unbekannten Schlüsseln (gemessen an allen sechs Daemons), der gemeinsame Logging-Default `DEFAULT_LOG_LEVEL`.
