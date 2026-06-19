# DDS-Security 1.2 Cross-Vendor Bench-Baseline 2026-05-27

Self-Roundtrip mit DDS-Security 1.2 (PKI-DH-Auth + Access-Control +
AES-GCM-Crypto), apples-to-apples gleiche IDL + governance/permissions
+ Test-CA-Setup.

## Setup

Test-CA-Generation: `tests/perf/dds-roundtrip-bench/security/gen.sh`
- ECDSA P-256 Identity-CA (self-signed)
- Permissions-CA = Identity-CA (für Tests)
- Per Participant (ping/pong) Identity-Cert (CA-signed)
- Governance + Permissions XML signed mit CMS (`openssl smime -sign`)

Governance-Policy: domain 200 mit `discovery_protection=ENCRYPT`,
`liveliness_protection=ENCRYPT`, `rtps_protection=ENCRYPT`,
`metadata_protection=ENCRYPT`, `data_protection=ENCRYPT`.

Bench-Env: codepit (4 cores, Debian 13 trixie), UDPv4 Loopback.

## Cyclone DDS Security (via `CYCLONEDDS_URI` XML)

XML-Config (gen-Template in `security/cyclone_security_{ping,pong}.xml`):
- `<Authentication>` mit Plugin `dds_security_auth` + cert/key/CA paths
- `<AccessControl>` mit Plugin `dds_security_ac` + governance/permissions
- `<Cryptographic>` mit Plugin `dds_security_crypto` (AES-GCM-GMAC)

Cyclone-Build muss mit `-DENABLE_SECURITY=ON` (default in 11.0.x ✓).

| Mode | min | p50 | p90 | p99 | max | Δ vs plain |
|---|---:|---:|---:|---:|---:|---:|
| Cyclone plain UDPv4 | 30 | 36 | 50 | 57 | 87 | (baseline) |
| **Cyclone DDS-Security UDPv4** | 82 | **100** | 160 | 181 | 193 | **+2.8×** |

## Fast-DDS Security (via Property-QoS)

App-Code-Setup: `apply_security(pqos)` setzt `properties().properties()`
mit `dds.sec.auth.plugin=builtin.PKI-DH`, identity/CA/permissions-Paths,
`dds.sec.crypto.plugin=builtin.AES-GCM-GMAC`.

Fast-DDS-Build muss mit `-DSECURITY=ON` (default `OFF` — rebuild nötig).

| Mode | min | p50 | p90 | p99 | max | Δ vs plain |
|---|---:|---:|---:|---:|---:|---:|
| Fast-DDS plain UDPv4 | 31 | 58 | 90 | 119 | 122 | (baseline) |
| **Fast-DDS DDS-Security UDPv4** | 174 | **201** | 243 | 313 | 368 | **+3.5×** |

## RTI Connext Security

App-Code-Setup analog (`com.rti.serv.load_plugin=com.rti.serv.secure` +
auth/access/crypto paths).

**Status: blockiert auf RTI Connext LM/Eval-Lizenz.** DP-create
schlägt mit `Failed to create DomainParticipant` fehl ohne weitere
Diagnose-Info. Vermutlich erfordert RTI-Security eine Pro-Lizenz
oder Subscription-Feature. Eigener Sprint mit Pro-Eval-License oder
RTI-Support-Anfrage.

| Mode | Status |
|---|---|
| RTI plain UDPv4 | ✓ baseline aus 5x5-Cross-Matrix |
| RTI DDS-Security UDPv4 | ✗ license_eval_blocked |

## OpenDDS Security

OpenDDS-Build muss mit `./configure --security` rekonfiguriert + neu
gebaut werden (default ist `--no-security`).

**Status: rebuild pending.** Wenn fertig: `opendds_rtps_sec.ini` mit
`[domain/0]` security-section + cert-paths analog Cyclone-XML.

| Mode | Status |
|---|---|
| OpenDDS plain UDPv4 | ✓ baseline aus 5x5-Cross-Matrix |
| OpenDDS DDS-Security UDPv4 | rebuild läuft (15-20min ETA) |

## ZeroDDS Security

ZeroDDS-Stack hat DDS-Security 1.2 voll implementiert (siehe
Memory `project_k6_security_status` — 50 done / 0 partial / 0 open
inkl. BuiltinDataTagging + CRL + Conformance-Matrix, 5172 Tests).

**Status: C-FFI live (2026-05-27).** Neue `crates/zerodds-c-api`
unter Feature-Flag `security`:
- 6 Setter `zerodds_security_set_{identity_ca,identity_cert,
  private_key,permissions_ca,governance,permissions}_path` (alle
  `(*mut ZeroDdsSecurityConfig, *const c_char) -> i32`).
- `zerodds_security_config_create()` / `_destroy()` Builder-Lifecycle.
- `zerodds_runtime_create_secure(domain, *const ZeroDdsSecurityConfig)`
  — synchroner PKI+CMS-Verify+Governance-Parse+Gate-Build; NULL bei
  jedem Fehler-Pfad mit `eprintln!` auf stderr.
- `apply_security` env-driven Helper in `zerodds_app.cpp` analog
  FastDDS — env-vars `ZERODDS_BENCH_SECURITY=1`,
  `ZERODDS_BENCH_SEC_NAME`, `ZERODDS_BENCH_SEC_DIR`.

Backend-Glue: neuer `crates/security-runtime/src/profile.rs`
mit `SecurityProfile::from_files(SecurityProfileConfig,
participant_guid) -> SecurityProfile` — bundelt PKI
(`PkiAuthenticationPlugin::validate_with_config`), CMS-Verify
(`CmsPkcs7Verifier::new(permissions_ca)`), Governance/Permissions-XML
und `AesGcmCryptoPlugin` zu einem konsumfertigen
`SharedSecurityGate`.

Test-Asset-Tree (`tests/perf/dds-roundtrip-bench/security/gen.sh`)
auf PKCS#8-Keys (`openssl genpkey -algorithm EC`) und
End-Entity-Cert-Signer (`authority_cert.pem`) angepasst — Root-CA
darf laut RFC-5280 §6 nicht direkt als CMS-Signer auftreten
(`CaUsedAsEndEntity`). Cyclone/FastDDS sind hier permissiver, der
EE-Signer-Tree ist aber cross-vendor-konsistent.

Smoke-Validation (macOS-Loopback, 200 Samples):

| Mode | min | p50 | p90 | p99 | max | Δ vs plain |
|---|---:|---:|---:|---:|---:|---:|
| ZeroDDS plain UDPv4 | 17 | 50 | 60 | 81 | 136 | (baseline) |
| **ZeroDDS DDS-Security UDPv4** | 15 | **51** | 61 | 117 | 131 | **+1%** |

Crypto-Overhead nahe Null auf Loopback. Apples-to-apples auf
codepit (4-core Debian) folgt sobald der Host wieder up ist (pve
ist 2026-05-27 nicht erreichbar).

## codepit 4×4 Cross-Vendor-Matrix (2026-05-28)

Hardware: codepit (4-core Debian 13 trixie), UDPv4-Loopback.
Governance: `data_protection_kind=ENCRYPT` (User-Payload-Crypto) +
`discovery_protection_kind=NONE` — konsistenter, von allen vier
Vendoren unterstützter Modus (siehe "Befunde" unten, warum nicht
voll-ENCRYPT). RTI ausgelassen (Security-Plugin = Pro/LM-Lizenz, kein
Kauf). n=2000, payload=64.

### Volle 4×4 (p50, ping-Vendor × pong-Vendor, n=2000)

| ping ╲ pong | cyclone | fastdds | opendds | zerodds |
|---|---:|---:|---:|---:|
| **cyclone** | 44µs | 45µs | 45µs | 53µs |
| **fastdds** | 106µs | 109µs | 110µs | 110µs |
| **opendds** | 133µs | 125µs | 132µs | 125µs |
| **zerodds** | FAIL | FAIL | FAIL | 65µs |

**15 von 16 Zellen liefern Cross-Vendor-Secured-Roundtrips** (AES-GCM-
GMAC data_protection). Cyclone, Fast-DDS UND OpenDDS matchen als
Initiator gegen **alle vier** Pong-Vendoren — inkl. ZeroDDS als
Responder (cyclone→zerodds 53µs, fastdds→zerodds 110µs,
opendds→zerodds 125µs). Das ist echte 4-Vendor-DDS-Security-Interop
auf der User-Payload-Crypto-Ebene.

Die einzige nicht-matchende Richtung ist **ZeroDDS als Initiator gegen
fremde Vendoren** (zerodds→cyclone/fastdds/opendds = Discovery-Timeout,
kein Crypto-Error). ZeroDDS-self-secure (zerodds→zerodds) läuft mit
65µs, und ZeroDDS als **Responder** matcht cross-vendor mit allen drei
fremden Initiatoren. Es ist also eine **Richtungs-Asymmetrie im
Cross-Vendor-Discovery** (ZeroDDS-Initiator + Fremd-Responder), nicht
ein Crypto-Defekt — die gleiche directional Discovery-Limitation, die
ZeroDDS auch im plain-Cross-Vendor-Pfad hat. Folge-Task.

### Self-Roundtrip (Diagonale, p50, codepit)

| Vendor | plain p50 | DDS-Security data_protection p50 |
|---|---:|---:|
| **Cyclone DDS** | 36µs | **44µs** |
| **Fast-DDS** | 58µs | **109µs** |
| **OpenDDS** | ~90µs | **132µs** |
| **ZeroDDS** | 25µs | **65µs** |

### Cross-Vendor-Befund

Der frühere "0 samples"-Effekt war primär ein Asset-Setup-Problem
(CMS-Signer + MIME-Format + governance-Schema), KEIN reiner
Wire-Crypto-Mismatch — nach den drei Fixes unten kommen Cross-Vendor-
Samples durch (15/16 Zellen, AES-GCM-GMAC data_protection
cross-vendor-interoperabel).

## Befunde dieser Iteration (ehrlich)

Drei reale DDS-Security-1.2-Interop-Hürden + ein ZeroDDS-Befund:

1. **CMS-Signer-Konflikt** (gefixt, Commit `d15d6ca2`): Cyclone/
   FastDDS/OpenDDS nutzen die Permissions-CA **direkt** als
   CMS-Signer (self-signed CA im SignedData, OMG-Real-World-Muster).
   ZeroDDS' `rustls-webpki` lehnte das mit `CaUsedAsEndEntity` ab.
   Fix: Signer-Cert == Trust-Anchor → Chain-Validation entfällt.

2. **CMS-MIME-Format-Konflikt** (gelöst per pro-Vendor-Asset): Cyclone
   + FastDDS brauchen `openssl smime -text` (`PKCS7_TEXT`, text/plain-
   MIME-Wrapper); OpenDDS' `SMIME_read_PKCS7` bricht daran ("mime no
   content type") und braucht rohes XML. Widersprüchlich — aber das
   p7s ist *lokale* Config jedes Participants, kein Wire-Austausch.
   ZeroDDS' Verifier ist flexibel (handhabt beide via
   `strip_text_plain_envelope`).

3. **OpenDDS governance-Format** (gelöst): brauchte
   `xsi:noNamespaceSchemaLocation=".../20170901/..."` + eingerücktes
   XML + `read/write_access_control=false`, sonst "No governance
   exists for this domain" trotz verifizierter Signatur.

4. **ZeroDDS secured-Discovery** (Folge-Task): `runtime_create_secure`
   verdrahtet den Wire-Crypto-Gate (`data_protection` → User-Payload-
   Encryption, voll funktional + getestet), aber NICHT den
   Auth-Handshake-Flow für `discovery_protection=ENCRYPT`
   (DCPSParticipantStatelessMessage + SPDP-Verschlüsselung).
   `enable_security_builtins` allein annonciert nur die
   BuiltinEndpointSet-Bits 22..25, ohne Handshake erwartet der Peer
   einen Handshake der nie kommt → Discovery blockiert (Regression,
   wieder entfernt). Unterstützter Modus: `discovery_protection=NONE`
   + `data_protection=ENCRYPT`.

## Offene Sprints (für nächste Iteration)

1. ~~**ZeroDDS-FFI Security-API**~~ — **erledigt 2026-05-27**
   (data_protection voll funktional, self + als Responder cross-vendor).
2. ~~**OpenDDS-rebuild --security**~~ — **erledigt 2026-05-28**
   (`/opt/opendds-secure`, libOpenDDS_Security.so; self 132µs).
3. ~~**4×4 Security-Cross-Matrix**~~ — **erledigt 2026-05-28**
   (15/16 Zellen, siehe oben).
4. **ZeroDDS-Initiator Cross-Vendor-Discovery**: zerodds→fremd-Vendor
   matcht nicht (directional Discovery-Limitation, auch im plain-Pfad).
5. **ZeroDDS secured-Discovery** (`discovery_protection=ENCRYPT`):
   Auth-Handshake-Flow im FFI an `SecurityProfile.pki` koppeln —
   DCPSParticipantStatelessMessage-Exchange + SPDP-Key-Install.
6. **RTI-Security**: out-of-scope (Pro/LM-Lizenz, kein Kauf).
