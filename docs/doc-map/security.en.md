# Security

German original: [security.md](security.md)

This page describes how security is built into a ZeroDDS system: (a) DDS-Security in the DDS layer with the crates `crates/security*`, (b) the security layer of the bridge daemons in `crates/bridge-security`, (c) one checklist each for a DDS system and for a bridge. It is based on the code. Statements that come only from a doc comment are marked "(per doc comment)". Nothing on this page was built or run for this page.

Related pages: [README.en.md](README.en.md) · [architecture.en.md](architecture.en.md) · [crates-core.en.md](crates-core.en.md) · [crates-bridges-services.en.md](crates-bridges-services.en.md) · [crates-languages-codegen.en.md](crates-languages-codegen.en.md) · [config.en.md](config.en.md) · [cli-options.en.md](cli-options.en.md) · [tools-and-man.en.md](tools-and-man.en.md) · [system-setup.en.md](system-setup.en.md) · [security.en.md](security.en.md) · [migration.en.md](migration.en.md) · [interop-and-dialects.en.md](interop-and-dialects.en.md) · [ros2.en.md](ros2.en.md)

## Summary

- DDS-Security is an opt-in feature: `zerodds-dcps` with `features = ["security"]` or `zerodds-c-api` with `--features security`. Without the feature, RTPS runs in plaintext (`crates/dcps/Cargo.toml`).
- A participant is secured in one of three ways: `ParticipantBuilder::with_security(SecurityBundle)` (Rust), `dds.sec.*` properties in the `DomainParticipantQos` (Rust), or the C API `zerodds_runtime_create_secure` / `zerodds_runtime_create_secure_from_env` with an SROS2 enclave (`security.enclave_dir`).
- Governance and permissions are signed via CMS/S/MIME with the permissions CA certificate: `zerodds-secure-permissions sign` or `openssl smime -sign -text`.
- Bridges: TLS (or DTLS, SSLIOP), auth mode per daemon, topic ACL with default "deny" as soon as an auth mode is set. SIGHUP reloads TLS material and ACL.

## (a) DDS-Security in the DDS layer

### Crates and plugins

| Crate | Task | Specification (per crate docs) |
|---|---|---|
| `zerodds-security` | Plugin SPI: traits `AuthenticationPlugin`, `AccessControlPlugin`, `CryptographicPlugin`, `LoggingPlugin`, `DataTaggingPlugin`, tokens, generic messages | DDS-Security §8.3–§8.7 |
| `zerodds-security-pki` | Authentication PKI-DH: X.509 (RSA-PSS, ECDSA P-256/P-384, Ed25519), handshake tokens, OCSP stapling, CRL, delegation chains, PSK variant `PskAuthenticationPlugin`; checks via `rustls-webpki` | §8.3, §9.3, §10.7 |
| `zerodds-security-keyexchange` | ephemeral ECDH for the handshake (P-256, P-384, X25519), HKDF-SHA256 | §8.3.2 |
| `zerodds-security-permissions` | Access control: parser for governance and permissions XML, CMS/PKCS#7 verification (`cms`), topic wildcards `*`/`?`, delegation check, PSK access | §9.4 |
| `zerodds-security-crypto` | Cryptographic: AES-128-GCM (kind 0x01, default), AES-256-GCM (0x02), HMAC-SHA256 authentication only (0x03, for `SIGN`), receiver-specific MACs, session keys | §8.5, §10.5 |
| `zerodds-security-rtps` | Wire format: `SEC_PREFIX`/`SEC_BODY`/`SEC_POSTFIX`, `SRTPS_PREFIX`/`SRTPS_POSTFIX`, RTPS header AAD | §7.3.6, §9.5 |
| `zerodds-security-logging` | Logging plugins: `StderrLoggingPlugin`, `JsonLinesLoggingPlugin`, `SyslogLoggingPlugin` (RFC 5424 UDP, `LOCAL0`), `FanOutLoggingPlugin` | §8.6 |
| `zerodds-security-runtime` | Wiring: `SecurityGate`/`SharedSecurityGate`, `GovernancePolicyEngine`, `SecurityProfile`, `SecurityBundle`, peer capabilities, data tagging, anti-squatter, `<peer_class>` matching | – |

Key agreement, checked in `crates/security-runtime/src/qos_properties.rs` (`kx_suite_from_properties`): when unspecified, `ECDHE-CEUM+P256`; permitted: `ECDHE-CEUM+P384` and `X25519`; everything else is rejected. On the wire, `security-pki` sends `ECDH+prime256v1-CEUM` (per doc comment, based on captures from Cyclone DDS and Fast DDS).

The private key is read as PKCS#8 (`BEGIN PRIVATE KEY`), PKCS#1 (`BEGIN RSA PRIVATE KEY`) or SEC1 (`BEGIN EC PRIVATE KEY`) (`crates/security-pki/src/identity.rs`). With `dds.sec.auth.password` set, an encrypted key opens (`crates/security-pki/src/encrypted_key.rs`): PKCS#8 `BEGIN ENCRYPTED PRIVATE KEY` with the password as passphrase, and the OpenSSL form (`Proc-Type: 4,ENCRYPTED`, AES-128/192/256-CBC) with the password first as the base64 AES-128 key of DDS-Security 1.2 §10.3.1 Table 50 and then as the OpenSSL passphrase Cyclone DDS and Fast DDS read it as.

### Configuring a participant

Way 1, Rust with `SecurityBundle` (`crates/dcps/src/factory.rs`, `crates/security-runtime/src/bundle.rs`, `profile.rs`):

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

- Alternatively `SecurityProfile::from_files(&SecurityProfileConfig { domain_id, identity_ca_pem, identity_cert_pem, identity_key_pem, permissions_ca_pem, governance_p7s, permissions_p7s }, guid)`.
- `RuntimeConfig::with_security_bundle` sets the gate, the logger and the GUID prefix derived from the identity certificate (DDS-Security §9.3.3). Without this prefix, the remote side rejects the handshake (per doc comment in `crates/dcps/src/runtime.rs`).

Way 2, properties in `DomainParticipantQos.property` (`crates/dcps/src/participant.rs:323`, `crates/security-runtime/src/qos_properties.rs`):

| Property | Content |
|---|---|
| `dds.sec.auth.identity_ca` | identity CA (PEM) |
| `dds.sec.auth.identity_certificate` | own certificate |
| `dds.sec.auth.private_key` | own key (PKCS#8, PKCS#1 or SEC1; encrypted with a password) |
| `dds.sec.auth.password` | opens an encrypted key; empty = none |
| `dds.sec.auth.key_establishment_algorithm` | see above |
| `dds.sec.access.permissions_ca` | permissions CA |
| `dds.sec.access.governance` | signed governance (`.p7s`) |
| `dds.sec.access.permissions` | signed permissions (`.p7s`) |
| `dds.sec.log.plugin` | `stderr`, `jsonl`, `syslog` (comma-separated list) |
| `org.zerodds.sec.log.level` | default `Informational` |
| `org.zerodds.sec.log.jsonl.path`, `org.zerodds.sec.log.syslog.addr`, `org.zerodds.sec.log.syslog.app`, `org.zerodds.sec.log.syslog.host` | sink parameters |
| `org.zerodds.sec.auth.accept_x509v1_identity_certificate` | deviation from §10.3.1.3, off by default |

- Values are URIs. `file:` is resolved, `data:,` is the content itself (RFC 2397) and `pkcs11:` names an object on a PKCS#11 token (RFC 7512, DDS-Security 1.2 §9.3.1 Table 44). `pkcs11:` needs the crate's `pkcs11` feature; without it the value is rejected with `SecurityPropertyError::Pkcs11FeatureDisabled`.
- If properties are present and a file is missing or a value is invalid, creating the participant fails (`DdsError::SecurityConfiguration`). There is no fallback to plaintext.
- If the `RuntimeConfig` already has a gate (way 1), the properties are not evaluated.

Way 3, C API (`crates/zerodds-c-api/src/security_ffi.rs`):

```c
ZeroDdsSecurityConfig *cfg = zerodds_security_config_create();
zerodds_security_set_identity_ca_path(cfg, "/etc/zerodds/certs/identity_ca.pem");
zerodds_security_set_identity_cert_path(cfg, "/etc/zerodds/certs/cert.pem");
zerodds_security_set_private_key_path(cfg, "/etc/zerodds/certs/key.pem");
zerodds_security_set_permissions_ca_path(cfg, "/etc/zerodds/certs/permissions_ca.pem");
zerodds_security_set_governance_path(cfg, "/etc/zerodds/certs/governance.p7s");
zerodds_security_set_permissions_path(cfg, "/etc/zerodds/certs/permissions.p7s");
ZeroDdsRuntime *rt = zerodds_runtime_create_secure(0, cfg);   /* NULL + stderr on error */
zerodds_security_config_destroy(cfg);                         /* cfg is not taken over */
```

- All six paths are mandatory.
- `zerodds_runtime_create_secure_from_env(domain)` reads `security.enclave_dir` (env `ZERODDS_SECURITY_DIR`) from the runtime configuration. If no directory is set, the function returns NULL; a set directory that does not load is an error.
- Build: `cargo build -p zerodds-c-api --release --features security` (or `fips`, `wolfcrypt`).

### Enclave directory (SROS2 layout)

`SecurityProfile::from_enclave_dir` expects these six file names (`crates/security-runtime/src/profile.rs:583`):

| File | Role |
|---|---|
| `identity_ca.cert.pem` | identity CA |
| `cert.pem` | identity certificate |
| `key.pem` | private key (PKCS#8) |
| `permissions_ca.cert.pem` | permissions CA |
| `governance.p7s` | signed governance |
| `permissions.p7s` | signed permissions |

Readers of `security.enclave_dir`: the ROS 2 shim (`crates/rmw-zerodds-shim/src/lib.rs:765`, feature `security`) and `zerodds_runtime_create_secure_from_env`. The Rust path `DomainParticipantFactory::create(..).build()` does not read this key; there, the caller invokes `from_enclave_dir` itself (inferred: no further occurrence of `security_enclave_dir`).

### Runtime keys related to security

| Key | Environment | Effect |
|---|---|---|
| `security.enclave_dir` | `ZERODDS_SECURITY_DIR` | enclave for the shim and the C API, see above |
| `security.secure_spdp` | `ZERODDS_SECURE_SPDP` | reliable secure SPDP channel (entity IDs 0xff0101c2/c7), which a secured participant announces. On by default. Fast DDS matches a peer's secure SEDP endpoints only after that peer's participant data arrived over it. |
| `transport.interface_bindings` | – (file only) | list of maps `name`, `bind`, `kind` (`loopback`, `localhost`, `wan`, `subnet`, `named:<class>`), `subnet`, `default`. Only with the feature `security`, otherwise a startup error. Basis for `<peer_class>` rules (heterogeneous security). |
| `discovery.multicast_allowlist` | `ZERODDS_MULTICAST_ALLOWLIST` | join only these multicast groups |

### Generating certificates and signing XML

Tools and scripts present in the repository:

| Location | Purpose |
|---|---|
| `tools/secure-permissions` (`zerodds-secure-permissions`) | `sign --signer-cert <ca.pem> --signer-key <ca_key.pem> --in <doc.xml> --out <doc.p7s>`; `verify --ca <permissions_ca.pem> --p7s <doc.p7s> [--out <doc.xml>]`. The output is S/MIME `signed-data`, verified against the tool's own verifier before writing. The signer key must be PKCS#8. |
| `tests/perf/dds-roundtrip-bench/security/gen.sh` | test CA, identity certificates `ping`/`pong`, signing of `governance.xml` and `permissions_{ping,pong}.xml` with `openssl smime -sign -text -signer permissions_ca.pem`. `KEY_ALGO=RSA` for Fast DDS. For tests only (unencrypted keys). |
| `tests/perf/dds-roundtrip-bench/security-matrix/gen_profile.sh` | governance profiles (protection kinds per category) and permissions signed per vendor |
| `crates/security-permissions/tests/fixtures/generate_fixtures.sh` | positive and negative fixtures for the CMS verifier (wrong CA, tampered, expired, intermediate CA, RSA PKCS#1) |
| `crates/zerodds-c-api/tests/security_ffi_e2e.rs` | generates the chain at runtime with `openssl`, skips with a message if `openssl` is absent |

Example XML:

- Governance: `tests/perf/dds-roundtrip-bench/security/governance.xml`, `zerodds-examples/security-governance-gate/governance.xml`
- Permissions: `tests/perf/dds-roundtrip-bench/security/permissions_ping.xml`, `permissions_pong.xml`, `crates/security-permissions/tests/fixtures/permissions.xml`
- Example programs: `zerodds-examples/security-governance-gate`, `zerodds-examples/rust-security-plugin-chain`, `zerodds-examples/rust-audit-securitybundle-realpart`

Rules for assets that all tested vendors accept (per `tests/perf/dds-roundtrip-bench/security-matrix/README.md`):

- The CMS signer is the permissions CA certificate itself, not a separate EE certificate.
- `openssl smime -sign -text`.
- Governance per XSD `20170901` with strict order of the `<xs:sequence>`.

### Cryptography backends

| Feature | Effect | Location |
|---|---|---|
| `ring-backend` | default in `security-pki`, `-permissions`, `-crypto`, `-keyexchange`, `-runtime` | respective `Cargo.toml` |
| `fips` (`zerodds-dcps`, `zerodds-c-api`, `zerodds-security-runtime`) | aws-lc-rs (AWS-LC) as the active cryptography. `ring` remains linked (per comment in `crates/dcps/Cargo.toml`, "D1a limitation"). | `crates/dcps/Cargo.toml` |
| `wolfcrypt` (`zerodds-dcps`, `zerodds-c-api`, `zerodds-security-runtime`) | wolfCrypt via `wolfcrypt-ring-compat`; needs `libwolfssl` or `WOLFSSL_SRC` (wolfSSL 5.9.x) at build time | `crates/dcps/Cargo.toml`, `crates/security-permissions/Cargo.toml:72` |
| `aws-lc` (`zerodds-bridge-security`) | aws-lc-rs for JWT verification and the rustls provider of the bridges; exactly one backend is enforced (`compile_error!`) | `crates/bridge-security/src/lib.rs` |

The six bridge crates forward the backend as their own features: `ring-backend` (default) and `aws-lc`. A FIPS build is `--no-default-features --features <default list without ring-backend>,aws-lc`; all six pass their tests this way (measured locally). `ring` remains linked through the workspace feature of `rustls`; TLS and JWT run over aws-lc-rs.

### Tested interop

Tests in the repository:

| Test | Content | Run |
|---|---|---|
| `crates/dcps/tests/security_matrix_e2e.rs` | ZeroDDS↔ZeroDDS: every suite, every ProtectionKind per category, both auth methods, both data representations | Linux (multicast loopback) |
| `crates/dcps/tests/security_live_e2e.rs` | handshake and crypto token exchange over real UDP multicast | Linux |
| `crates/dcps/tests/security_{bundle,endpoint_access,guid_wiring,log_properties,qos_properties,topic_rule_wire}*.rs` | wiring, endpoint access check, GUID derivation, properties | – |
| `crates/zerodds-c-api/tests/security_ffi_e2e.rs` | C API, EE- and CA-signed assets, full lockdown, rejection without a domain rule | needs `openssl` |
| `crates/security-runtime/tests/cyclone_live_security_caps.rs` | Cyclone DDS does not discard SPDP beacons with ZeroDDS capabilities | `--ignored`, needs a Cyclone host |
| `tests/perf/dds-roundtrip-bench/security-matrix/run_deep_matrix.sh` | 13 governance profiles × 4×4 vendors (Cyclone DDS, Fast DDS, OpenDDS, ZeroDDS), both roles | Linux with vendor stack, `libzerodds.so` with `--features security` |

Results are not in the code. According to `internal/security/cross-vendor-interop-configs.md` and `internal/security/opendds-secure-matrix-closeout.md`: ZeroDDS↔Cyclone DDS 13/13, ZeroDDS↔Fast DDS 13/13, ZeroDDS↔OpenDDS 9/13 profiles. Re-measured with all 13 profiles and EC identities: Fast DDS 3.6.1 accepts EC identities; RSA is not needed.

## (b) Bridge security

### Shared layer `crates/bridge-security`

Used by: `websocket-bridge`, `mqtt-bridge`, `coap-bridge`, `grpc-bridge`, `amqp-endpoint`, `corba-dds-bridge` (`Cargo.toml` dependencies).

| Part | Behavior (checked in `ctx.rs`, `acl.rs`, `auth.rs`) |
|---|---|
| TLS | `tls_cert` and `tls_key` only together; with `client_ca`, a client certificate is required (mTLS). rustls ServerConfig. |
| Auth modes | `none`, `bearer` (token → subject), `jwt` (RS256 only), `mtls`, `sasl` (SASL PLAIN, user → password). Unknown mode: startup error. |
| JWT | `auth.jwt.public_key` (PEM, PKCS#1 or SubjectPublicKeyInfo; mandatory), `auth.jwt.issuer` (`iss`), `auth.jwt.audience` (`aud`), `auth.jwt.required_claim` (`claim=value`). `exp`/`nbf` with 60 s tolerance (`JWT_CLOCK_LEEWAY_SECS`). Subject from `sub`, groups from the claim `groups`. |
| mTLS subject | name = subject DN of the leaf certificate as an RFC 4514 string (`CN=alice,O=Example`), additionally the fingerprint `mtls:<SHA-256 of the DER, hex>`. Empty or unreadable DN: fingerprint only. An ACL rule may name the DN or the fingerprint (`mtls_subject_from_der`). |
| Topic ACL | per topic `read`/`write`; patterns `*`, exact name (or fingerprint), `*group:<g>*`. Topic without an entry: default entry, otherwise rejection. |
| ACL default | No rules and mode `none`: everything allowed. No rules and another mode: everything rejected. With rules: whatever no rule allows is rejected. |
| Reload | `LiveSecurity` replaces the context as a whole on SIGHUP. New connections receive the new TLS configuration, open connections keep their session. |
| Backend | `ring-backend` (default) or `aws-lc` |

In the configuration report (`--check-config`), every value whose path contains a segment with `token`, `password`, `secret`, `passphrase`, `psk`, `sasl_users` or `credentials` is printed as `***`, e.g. `auth.bearer_token`, `mqtt.password`, `oscore.master_secret_hex`, `coap.dtls.psk` (`is_secret_path`, `crates/config/src/layers.rs:504`). Paths to key files (`tls.key_file`) do not count as secrets.

### Differences per daemon

| Daemon | Transport protection | `auth.mode` (checked in the parser) | ACL form | SIGHUP |
|---|---|---|---|---|
| `zerodds-ws-bridged` | `tls.enabled`, `tls.cert_file`, `tls.key_file`, `tls.client_ca_file` | `none`, `bearer`, `jwt`, `mtls` (`crates/websocket-bridge/src/daemon/config.rs:201`) | per topic `acl.read`/`acl.write` | yes |
| `zerodds-grpc-bridged` | `grpc.tls.*`, `client_ca_file`, `require_client_cert` | `none`, `bearer`, `jwt`, `mtls`, checked per call | per topic `acl.publish`/`acl.subscribe` | yes (TLS, auth, ACL) |
| `zerodds-mqtt-bridged` | `mqtt.tls.*` to the broker (`ca_file`, `cert_file`, `key_file`, `server_name`) | `none`, `bearer`, `sasl`, `sasl_plain`, `mtls` (`crates/mqtt-bridge/src/daemon/config.rs:294`) | `acl.default_deny`, `acl.rules[].subject`, `allow_publish`, `allow_subscribe` | yes |
| `zerodds-coap-bridged` | `coap.dtls.*` (certificate or `psk[].identity`/`secret_hex`, `cipher_suites`), `oscore.*` (RFC 8613) | `none`, `bearer`, `jwt`, `mtls` (`crates/coap-bridge/src/daemon/config.rs:376`) | `acl.rules[].allow_post` (write), `allow_observe` (read); subject = PSK identity or certificate DN | yes |
| `zerodds-amqp-bridged` | `amqp.tls.*`, `verify_hostname` (only `true`), `alpn` | SASL to the broker: `amqp.sasl.mechanism` = `PLAIN`, `SCRAM-SHA-256`, `EXTERNAL`, `ANONYMOUS`, `XOAUTH2` (`crates/amqp-endpoint/src/bridge_daemon.rs:349`) | `acl.rules[].topic`, `read`, `write` (SASL subjects) | yes (`crates/amqp-endpoint/src/daemon_runtime.rs`) |
| `zerodds-corba-bridged` | `corba.ssliop.*` (`bind`, `cert_file`, `key_file`, `ca_file`, `require_client_cert`) | no `auth.mode`; subject from the client certificate | `acl.rules[].subject`, `allow_invoke` (repository IDs with `*`) | yes (ACL, SSLIOP) |

CoAP specifics:

- `dtls` and `oscore` are Cargo features of `zerodds-coap-bridge` and are not in the default (`crates/coap-bridge/Cargo.toml`). A build without the feature aborts startup if `coap.dtls.enabled: true` or `oscore.enabled: true` is set (`crates/coap-bridge/src/daemon/server.rs:377`, `:417`).
- DTLS 1.2 via `webrtc-dtls`, marked as experimental in the code (ADR 0011).
- `oscore.enabled: true` without `master_secret_hex` aborts startup (per the example configuration).

CORBA specifics: `corba.csiv2.enabled: true` and `corba.ior.publish_to_naming: true` are rejected because they are not built (per `packaging/linux/configs/corba-bridged.yaml.example`).

Bearer: In the WebSocket and gRPC examples, `bearer` is a single subject (`auth.bearer_token` + `auth.bearer_subject`); `jwt` is intended for multiple users (comment in `ws-bridged.yaml.example`).

Tests: `crates/{websocket-bridge,mqtt-bridge,coap-bridge,grpc-bridge,amqp-endpoint,corba-dds-bridge}/tests/security_e2e.rs`, additionally `crates/grpc-bridge/tests/daemon_security_e2e.rs`.

## (c) Setting up security, step by step

### DDS system

1. Build with security: `zerodds-dcps` with `features = ["security"]` (Rust) or `cargo build -p zerodds-c-api --release --features security`. For FIPS `fips`, for wolfCrypt `wolfcrypt`. For ROS 2: `rmw-zerodds-shim` with `--features security`.
2. Define two CAs: the identity CA (signs participant certificates) and the permissions CA (signs governance and permissions). In tests, both are the same CA (`gen.sh`).
3. Generate a key and a certificate per participant. Key as PKCS#8 (`openssl genpkey`), without a passphrase. EC P-256 for Cyclone DDS, OpenDDS, ZeroDDS; RSA-2048 if Fast DDS is involved (comment in `gen.sh`).
4. Write the governance XML: per domain `rtps_protection_kind`, `discovery_protection_kind`, `liveliness_protection_kind`, `enable_join_access_control`; per topic `metadata_protection_kind`, `data_protection_kind`, `enable_discovery_protection`, `enable_liveliness_protection`, `enable_read_access_control`, `enable_write_access_control` (assignment per DDS-Security §9.4.1.2; element names from `crates/security-permissions/src/governance.rs`). Template: `tests/perf/dds-roundtrip-bench/security/governance.xml`.
5. Permissions XML per participant: `<grant>` with the subject name of the certificate, `<allow_rule>` with `<publish>`/`<subscribe>` and `<topic>`. Template: `permissions_ping.xml`.
6. Sign both XML files:
   ```sh
   zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
       --in governance.xml --out governance.p7s
   zerodds-secure-permissions sign --signer-cert permissions_ca.pem --signer-key permissions_ca_key.pem \
       --in permissions.xml --out permissions.p7s
   zerodds-secure-permissions verify --ca permissions_ca.pem --p7s permissions.p7s
   ```
7. Place the files under the SROS2 names in an enclave directory (table above), e.g. under `/etc/zerodds/certs/` (created by `zerodds-tmpfiles.conf` with mode 0700).
8. Wire the participant: Rust `SecurityProfile::from_enclave_dir` + `SecurityBundle` + `DomainParticipantFactory::create(d).with_security(bundle)`, or `dds.sec.*` properties, or the C API with `security.enclave_dir` and `zerodds_runtime_create_secure_from_env`.
9. Log security events: `dds.sec.log.plugin: "stderr,jsonl"` with `org.zerodds.sec.log.jsonl.path`, or `logging_plugin` in the bundle.
10. With Fast DDS nothing more is needed: `security.secure_spdp` is on.
11. Check: `--check-config` for the runtime keys; handshake errors appear in the security log. Reference run: `crates/dcps/tests/security_live_e2e.rs` on Linux.

### Bridge

1. Place the daemon's certificate and key in `/etc/zerodds/certs/`, owner `zerodds`.
2. Enable transport protection in the daemon YAML (`tls.enabled: true`; for CoAP `coap.dtls.enabled: true` with the build feature `dtls`; for CORBA `corba.ssliop.enabled: true`).
3. For mTLS, set the client CA (`tls.client_ca_file`, `grpc.tls.client_ca_file` + `require_client_cert`, `corba.ssliop.ca_file` + `require_client_cert`) and `auth.mode: mtls`.
4. Otherwise choose an auth mode: `bearer` with `auth.bearer_token` (value via the environment, e.g. `${WS_BRIDGE_TOKEN}`), `jwt` with `auth.jwt.public_key` and `issuer`/`audience`/`required_claim`, `sasl` (MQTT) with `auth.sasl_users`.
5. Create an ACL for each topic. With `auth.mode` other than `none`, a topic without a rule is rejected. Subjects: user name, JWT `sub`, `*group:<g>*`, mTLS DN (`CN=…,O=…`) or `mtls:<sha256>`.
6. Check the configuration: `zerodds-<x>-bridged --config /etc/zerodds/<x>-bridged.yaml --check-config` (secrets appear as `***`).
7. Bind the metrics endpoint only to `127.0.0.1` (default in the examples).
8. Renew a certificate or change the ACL: replace the files, then `systemctl reload zerodds-<x>-bridged` (SIGHUP).
9. Reference tests: `crates/<bridge>/tests/security_e2e.rs`.

## Contradictions and open items

- `security.secure_spdp` on, measured with the security deep matrix (`tests/perf/dds-roundtrip-bench/security-matrix/`, 13 governance profiles, EC identities, both directions): ZeroDDS with Fast DDS 13/13, with Cyclone DDS 13/13, with OpenDDS 9/13 (OpenDDS's limits), with ZeroDDS 13/13.
- According to a doc comment, `discovery.server` has no relay for discovery secured with DDS-Security.

Fixed: `zerodds-examples/rust-audit-securitybundle-realpart` runs the chain `DomainParticipantFactory::create(0).with_security(bundle).build()` (`crates/dcps/src/factory.rs:445`) instead of claiming it does not exist; SROS2 against a secured `rmw_cyclonedds_cpp` listener (two alignment faults: `SEC_BODY` and the encrypted payload padded to 4 bytes, `d6fe961eaca`, `bdbd9107f93`; 13 messages measured), version references to DDS-Security 1.2 with its numbering, the auth modes in the ws example, the statements of the example configurations about unknown keys (measured on all six daemons), the shared logging default `DEFAULT_LOG_LEVEL`.
