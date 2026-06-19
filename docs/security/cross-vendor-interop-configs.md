# Cross-Vendor DDS-Security Interop — Config-Referenz (cyclone / FastDDS)

**Stand:** 2026-06-02. **Kontext:** secured discovery=ENCRYPT Cross-Vendor-Matrix.
Recherche-Ergebnis (sros2 / cyclonedds / FastDDS Quellen, URLs unten).

## Bench-Beobachtung (das zu lösende Problem)

- cyclone↔cyclone secured (discovery=ENCRYPT): **läuft** (p50 ~311µs).
- cyclone↔FastDDS **non-secured**: läuft (p50 ~26µs).
- cyclone↔FastDDS **secured**: **scheitert** — Ping `match timeout pub=0 sub=0`.
- ZeroDDS↔FastDDS secured: scheitert mit demselben Muster.

**Wurzel (Trace-belegt):** Auth-Handshake vollendet (`EVENT_VALIDATION_OK_
FINAL_MESSAGE`), aber FastDDS tauscht über die VolatileSecure-Topic nur die
`ff0101`-Crypto-Tokens (participant + ein reader/writer-Paar) aus — **keine
`ff0003`/`ff0004` (DCPSPublications/SubscriptionsSecure = secure-SEDP)**. Ohne
secure-SEDP-Tokens kann der Peer FastDDS' geschützte SEDP nicht decoden → FastDDS'
User-Endpoints werden nie entdeckt → kein Match. Gilt fuer cyclone↔FastDDS
**und** ZeroDDS↔FastDDS → **FastDDS-Config/Governance-Thema, kein ZeroDDS-Bug.**

FastDDS-Version im Bench: **v3.6.1** (modern; „protocol_version 2.3" = RTPS-Wire-
Version, nicht Lib-Version). Die alten 2.3-Ära-Handshake-Bugs sind dort gefixt.

## sros2-Default-Governance (de-facto interop-getestet, cyclone+FastDDS via RMW)

`ros2/sros2 .../policy/defaults/dds/governance.xml` — kanonisches Profil:

```xml
<domain_rule>
  <domains><id>0</id></domains>
  <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
  <enable_join_access_control>true</enable_join_access_control>
  <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
  <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
  <rtps_protection_kind>SIGN</rtps_protection_kind>     <!-- SIGN, nicht ENCRYPT -->
  <topic_access_rules>
    <topic_rule>
      <topic_expression>*</topic_expression>
      <enable_discovery_protection>true</enable_discovery_protection>
      <enable_liveliness_protection>true</enable_liveliness_protection>
      <enable_read_access_control>true</enable_read_access_control>
      <enable_write_access_control>true</enable_write_access_control>
      <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
      <data_protection_kind>ENCRYPT</data_protection_kind>
    </topic_rule>
  </topic_access_rules>
</domain_rule>
```

**Unterschied zur Bench-Governance:** Bench hat discovery=ENCRYPT **aber**
liveliness=NONE, rtps=NONE, read/write_access_control=false. sros2 schaltet
discovery **und** liveliness gemeinsam auf ENCRYPT, rtps=SIGN, access_control ON.
„discovery on, liveliness off" ist eine non-default, weniger getestete Kombination.

**XSD-Reihenfolge ist strikt** (`<xs:sequence>`): domain_rule → domains,
allow_unauth, enable_join, discovery_kind, liveliness_kind, rtps_kind,
topic_access_rules; topic_rule → topic_expression, enable_discovery_protection,
enable_liveliness_protection, enable_read_ac, enable_write_ac, metadata_kind,
data_kind. Falsche Reihenfolge → FastDDS dropt Felder still → Endpoints nicht
angelegt. (Bench-Governance-Reihenfolge ist korrekt, geprüft.)

## FastDDS-Mechanik (warum nur ff0101, nicht ff0003/ff0004)

- Domain-`discovery_protection_kind=ENCRYPT` ist **notwendig, aber nicht
  hinreichend**: erst per-topic `enable_discovery_protection=true` instanziiert
  FastDDS die secure-SEDP-Builtin-Endpoints (ff0003/ff0004). secure-Liveliness
  (ff0101-artig) wird unabhaengig über `enable_liveliness_protection` geschaltet.
- Aktivierung via PropertyPolicyQos `dds.sec.*` (Auth PKI-DH, Access-Permissions,
  Crypto AES-GCM-GMAC). Fuer secure **discovery** ist das Access/Permissions-
  Plugin (Governance) zwingend.
- Build: FastDDS braucht `-DSECURITY=ON` + OpenSSL.

## Bekannte cyclone↔FastDDS Security-Interop-Bugs (Versions-Kontext)

- **FastDDS #3803** — Cyclone/ZeroDDS haengen `\0` an `c.dsign_algo`/
  `c.kagree_algo`; FastDDS' String-Compare scheitert. → **ZeroDDS-Fix `297dc526`
  (compute_hash_c_raw, rohe Wire-Bytes) adressiert genau das.**
- **FastDDS #3802** — DH-Public-Key-Wire-Encoding raw `BN_bn2bin` (FastDDS) vs
  ASN.1 (cyclone) fuer `DH+MODP-2048-256`; in DDS-Security 1.2 (2024) geklärt.
  Workaround: ECDHE-P256 (`prime256v1`) auf beiden Seiten erzwingen.
- **FastDDS #3804** — FastDDS behandelt optionale Reply-Felder (hash_c2/hash_c1/
  dh1) als Pflicht. Cyclone-Workaround:
  `//CycloneDDS/Domain/Security/Authentication/IncludeOptionalFields=true`.
- Cyclone #1547 — cyclone↔FastDDS secured, „Failed to convert octet sequence to
  ASN1 integer" (= #3802); needs-triage, ungelöst → reales Interop-Loch.
- Nicht-Security-Falle: Extensibility-Default cyclone `final` vs FastDDS
  `appendable` → User-Topic-Match bricht; explizit `@final`/`@appendable`.

## Empfehlung (umsetzbar)

1. **Bench-Governance auf sros2-Template umstellen** (discovery+liveliness=ENCRYPT,
   rtps=SIGN, access_control ON) und cyclone↔FastDDS secured isoliert re-testen.
   Höchstwahrscheinlicher Fix fuer das ff0003/ff0004-Token-Loch.
2. **ECDHE-P256** als key_agreement auf allen Seiten pinnen (umgeht #3802).
3. Sicherstellen, dass die signierte (S/MIME) Governance exakt XSD-konform +
   richtig geordnet ist (FastDDS validiert hart).
4. ZeroDDS: Handshake-Fix `297dc526` bleibt; secure-SEDP-Token-Austausch + rtps/
   liveliness-Protection-Support muessen zum gewählten Governance-Profil passen.

## Quellen

- sros2 governance: https://github.com/ros2/sros2/blob/master/sros2/sros2/policy/defaults/dds/governance.xml
- FastDDS security properties: https://fast-dds.docs.eprosima.com/en/v3.1.2/fastdds/property_policies/security.html
- FastDDS governance XSD: https://fast-dds.docs.eprosima.com/en/v2.3.3/fastdds/security/access_control_plugin/governance.html
- FastDDS #3802/#3803/#3804: https://github.com/eProsima/Fast-DDS/issues/3802 (+3803, +3804)
- FastDDS #3259 (FastDDS↔cyclone security): https://github.com/eProsima/Fast-DDS/issues/3259
- Cyclone #1547: https://github.com/eclipse-cyclonedds/cyclonedds/issues/1547
- Cyclone secure discovery: https://cyclonedds.io/docs/cyclonedds/latest/security/dds_secure_discovery.html
- OpenDDS security (Feld-Doku): https://opendds.readthedocs.io/en/latest-release/devguide/dds_security.html
- DDS-Security 1.2 (DH-Encoding §8.3): https://www.omg.org/spec/DDS-SECURITY/1.2/PDF

## NACHTRAG 2026-06-02 — Vollständige Live-Matrix + Root-Cause (tiefe Recherche)

Live verifiziert (codepit, domain 200, alle Profile discovery=ENCRYPT/bridge/rtps=ENCRYPT):

| Paar | secured |
|---|---|
| cyclone ↔ cyclone | ✅ p50 ~311µs |
| FastDDS ↔ FastDDS | ✅ p50 ~125µs |
| cyclone ↔ FastDDS | ❌ **unter JEDEM Profil** (auth-handshake completes, aber User-Match nie → pub=0/sub=0) |

**Root-Cause (bestätigt, OMG + Maintainer):** DDS-Security **1.1** hat das dh1/dh2-Encoding
unterspezifiziert (OMG **DDSSEC12-56**); **1.2 (März 2024) §8.3** klärt: `DH+MODP-2048-256`
= rohe Big-Endian-Bytes (`BN_bn2bin`), `ECDHE-CEUM+P256` = SEC1-Octet-String. **FastDDS
sendet raw (jetzt 1.2-konform); CycloneDDS sendet ASN.1 (`i2d_ASN1_INTEGER`, jetzt
non-konform, Erbe von OpenSplice)** → für den DH-Pfad **wire-inkompatibel**. cyclone bietet
KEINEN Config-Schalter für das kagree-Encoding. Zusätzlich offen: FastDDS **#3803**
(kagree/dsign-Identifier mit NUL-Terminator — cyclone `strlen+1` vs FastDDS
`std::string::compare`; **genau der von ZeroDDS-Commit `297dc526` adressierte Punkt**),
**#3804** (FastDDS behandelt optionale Reply-Felder hash_c1/hash_c2/dh1 als Pflicht;
Workaround cyclone `IncludeOptionalFields=true`). Alle OFFEN, kein Fix-PR.

**ROS 2:** Cross-RMW (rmw_cyclonedds ↔ rmw_fastrtps) + Security ist **nicht garantiert/
unterstützt** (docs.ros.org „Different Middleware Vendors"; sros2 #242, ros2 #1051).

**Konsequenz / ZeroDDS-Positionierung:** cyclone↔FastDDS secured ist ein bekanntes,
ungelöstes Vendor-Loch — **kein ZeroDDS-Bug**, nicht durch Governance-Profil lösbar.
ZeroDDS nutzt **ECDH-prime256v1** (nicht DH+MODP) und hat #3803 gefixt (`297dc526`); der
ZeroDDS↔FastDDS-Auth-Handshake vollendet. Der EC-Pfad umgeht die ASN.1-vs-raw-DH-Falle →
**ZeroDDS kann als 1.2-konformer Impl secured mit BEIDEN interoperieren, wo die Legacy-
Stacks es untereinander nicht können** (Differenzierungs-Merkmal). Verbleibender
ZeroDDS↔FastDDS-Gap (Volatile-Token-Receive, T-INST=0) = selbe Klasse wie Task #28.

Quellen: cyclonedds #1547/#1895/#2184; Fast-DDS #3259/#3802/#3803/#3804;
OMG DDSSEC12-56; DDS-Security 1.2 §8.3; docs.ros.org Different-Middleware-Vendors.

## NACHTRAG 2026-06-08 — FastDDS secured Interop: 13/13 Profile (VOLLSTÄNDIG)

Nach deterministischem Reverse-Engineering (fast↔fast-pcaps, openssl-Signatur-Verify,
In-Rust-GMAC-AAD-Brute-Force) laufen **alle 13 Governance-Profile zerodds↔FastDDS secured,
beide Richtungen** (von 0): rtps-enc, common-subset, data-enc, data-sign, meta-data-enc,
meta-sign-data, liv-data-enc, disc-data-enc, disc-meta-data, all-enc, **rtps-sign-data,
all-sign, sros2-full** (= voller rtps=SIGN-Cluster). **cyclone bleibt 13/13** beide
Richtungen, zerodds-self alle grün — alle Fixes regression-frei (26/26 + 26/26 Live-Matrix
auf codepit verifiziert).

### Config-Options (ZeroDDS, FastDDS-Interop)

| Option | Default | Wirkung | cyclone-Drift? |
|---|---|---|---|
| `RuntimeConfig.enable_secure_spdp` (Env `ZERODDS_SECURE_SPDP=1`) | `false` | Betreibt den reliablen **Secure-SPDP-Kanal** (`0xff0101c2/c7`, FastDDS `ENTITYID_SPDP_RELIABLE_BUILTIN_PARTICIPANT_SECURE_*`): announce + reader-ACK auf FastDDS-HEARTBEAT + writer-resend auf preemptive-ACKNACK + periodischer HEARTBEAT + `ff0101`-Crypto-Tokens. FastDDS announced darüber seine vollen secured Participant-Daten und gated die Crypto-Token-Reziprokation daran. | Nein (additiv; cyclone ignoriert `0xff0101`) |
| Secure-SPDP-SEC-Protection (`protect_secure_spdp`) | auto | Unter `discovery_protection != NONE` wird die Secure-SPDP-DATA SEC-verschlüsselt (per-Endpoint-`ff0101c2`-Writer-Key) — FastDDS erwartet das, plain wird verworfen. Bei `discovery_protection=NONE` plain. Gated auf `enable_secure_spdp`. | Nein |
| no-NUL Algo-Strings (`compute_hash_c`/`build_*_token`) | immer | `c.dsign_algo`/`c.kagree_algo` werden OHNE `\0`-Terminator emittiert + gehasht (FastDDS-konform; #3803). cyclone/OpenDDS hängten `\0` an — kompatibel, weil der Validate-Pfad die rohen empfangenen Wire-Bytes hasht (`compute_hash_c_raw`). | Nein (kein Guard nötig, spec-korrekt) |
| Reply-Wire-Property-Order (1,2) + ocsp_status weglassen | immer | Reply-Token-Paare in FastDDS-Wire-Order (parst by-name, harmlos); `ocsp_status` (spec-optional) nicht emittiert. | Nein |

**Hinweis:** Für FastDDS-Interop `ZERODDS_SECURE_SPDP=1` setzen. Für reine cyclone-/spec-
Deployments default lassen (aus) — der Secure-SPDP-Kanal ist FastDDS-spezifisch.

### GELÖST: rtps_protection=SIGN-Cluster (rtps-sign-data, all-sign, sros2-full)

Der rtps=SIGN-Cluster war zuletzt offen und ist jetzt **vollständig gelöst** — beide
Richtungen grün, **ohne** Config-Guard, regression-frei zu cyclone. Der Weg dahin korrigiert
eine frühere Fehldiagnose und ist als Lehrstück dokumentiert.

**Symptom:** zerodds-Writer `wait_for_matched timeout` gegen FastDDS unter rtps_protection=SIGN
(message-level SRTPS-GMAC). rtps=ENCRYPT-Profile liefen, SIGN nicht.

**Fehlspur (verworfen): SRTPS-GMAC-AAD-Vendor-Divergenz.** Ein vorläufiger Versuch
(`AAD = rtps_header‖body` statt body-only) ließ FastDDS scheinbar matchen und wurde hinter
einen Config-Guard `ZERODDS_SRTPS_GMAC_AAD` gelegt. Das war falsch: der Match kam über den
Discovery-Pfad zustande, nicht über die GMAC-Verifikation. Ein **In-Rust-Brute-Force** (bei
GMAC-Verify-Fail alle AAD-Kandidaten gegen FastDDS' tag gerechnet, mit der echten
Session-Key-Ableitung) ergab eindeutig **`AAD_MATCH variant=ct`** für alle Nachrichten:
**FastDDS nutzt body-only GMAC-AAD — exakt die cyclone-Konvention.** Der Guard war damit
gegenstandslos und wurde **entfernt**; body-only ist cross-vendor universell, kein Drift.

**Echte Ursache: `parse_srtps_body` schlug an FastDDS' SRTPS-Framing fehl.** Die GMAC-
SRTPS-Nachricht ist `RTPS-Header | SRTPS_PREFIX(0x33) | <cleartext-Submessages> |
SRTPS_POSTFIX(0x34) | …`. Der Parser nahm an, der `SRTPS_POSTFIX` seien die **letzten 24
Bytes**. cyclone erfüllt das; **FastDDS hängt nach dem POSTFIX noch eine vendor-spezifische
Submessage (`0x80`) an** (bzw. der POSTFIX trägt receiver-specific-MACs). Damit war
`r1[len-24]` nicht der POSTFIX-Header → `parse_srtps_message` → `Err` → `CryptoError` →
FastDDS' SRTPS (inkl. seiner SEDP/Daten) wurde nie entschlüsselt → kein Match. cyclone fiel
nie auf, weil es nichts anhängt.

**Fix (`crates/security-crypto/src/plugin.rs`, `parse_srtps_body`):** Fast-Path `last-24`
für cyclone/zerodds **unverändert**; bei ungültigem POSTFIX **vorwärts bis zum
`SRTPS_POSTFIX(0x34)` walken** und den common_mac (16 B) aus dessen Body nehmen. Die
ct-Submessages sind nicht message-final (POSTFIX folgt), tragen also kein `otn=0` → der Walk
terminiert deterministisch. Hybrid → cyclone exakt erhalten, FastDDS' Trailing-Submessage
toleriert.

**Diagnose-Kette (Trace + tshark, alle Traces danach entfernt):** RECV(22 FastDDS-0x33
empfangen) → SRTPS_IN=0 (decode nie erreicht) → CLASSIFY_033=crypto_error → Erkenntnis: Fehler
**vor** dem GMAC, in `parse_srtps_message` (SRTPS_IN steht danach) → tshark zeigt Trailing-
`0x80` → parse-Fix → SRTPS_IN 0→131 → GMAC-Fail → AAD-Brute-Force → `variant=ct` → Guard raus.

**Verifizierter Endstand (codepit Live-Matrix, sauberer Default, nur `ZERODDS_SECURE_SPDP=1`):**

| Vendor-Paar | Profile × Richtungen | Status |
|---|---|---|
| zerodds ↔ FastDDS | 13 × 2 = **26/26** | ✅ alle grün (rtps=SIGN: rtps-sign-data 80/117µs, all-sign 110/104µs, sros2-full 93/109µs) |
| zerodds ↔ cyclone | 13 × 2 = **26/26** | ✅ regression-frei |
| zerodds ↔ zerodds | rtps=SIGN-Cluster | ✅ 37/47/52µs |

Unit-Tests: security-crypto 87 ✓, security-runtime 222 ✓. **Kein Config-Guard für
rtps=SIGN** — body-only GMAC-AAD ist die universelle cyclone-/FastDDS-/OpenDDS-Konvention.


## NACHTRAG 2026-06-08 — OpenDDS secured Interop: Auth komplett, 3 Profile grün

OpenDDS (OCI, VendorId `0x0103`) war 0/13. Der **Auth-Handshake ist jetzt vollständig
gelöst** (4 OpenDDS-spezifische Cross-Vendor-Fixes, alle Source-verifiziert in
`/root/OpenDDS-src` Tag `DDS-3.34.0`), **3 Profile laufen end-to-end beide Richtungen**
(meta-data-enc, meta-sign-data, disc-meta-data = die metadata-/Submessage-Key-Schicht).
**cyclone 26/26 + FastDDS 26/26 bleiben regression-frei** (verifiziert).

### Auth-Fixes (alle Default-aktiv bzw. per-Vendor, kein Config-Guard nötig)

| Fix | OpenDDS-Erwartung (Source) | zerodds vorher | Lösung |
|---|---|---|---|
| **IdentityToken `dds.cert.sn`/`dds.ca.sn`** | `Certificate::subject_name_to_str` Default `XN_FLAG_ONELINE` (`CN = ..., emailAddress = ...`, DER-Order, Spaces um `=`); `validate_remote_identity` vergleicht `dds.ca.sn` per String-`!=` | RFC-4514 (`CN=...`, reversed, ohne Spaces) → `VALIDATION_FAILED` → erase → **stiller** Rediscover-Loop (Exception nicht geloggt) | `security-pki::identity_token::subject_oneline` (OpenSSL-ONELINE-Format). cyclone/FastDDS vergleichen `dds.ca.sn` nicht (Source-verifiziert lenient) → universell |
| **`c.kagree_algo`/`c.dsign_algo` NUL** | `DiffieHellman::factory` + `add_bin_property` via `sizeof`/`size()+1` → **MIT** trailing `\0` (`ECDH+prime256v1-CEUM\0`); no-NUL → `unknown kagree_algo` → **segfault** | ohne NUL (#3803-FastDDS-Fix) | **per-Vendor** `VendorId` (SPDP-Header) → `SecurityBuiltinStack::note_remote_vendor` → `AuthenticationPlugin::set_algo_nul_terminate` vor `begin_handshake_request/reply`. OpenDDS=mit NUL, FastDDS=ohne, cyclone=tolerant. Receive-Seite (`kx_suite_for_algo`/`check_dsign_matches`/kagree-Echo) NUL-tolerant; Hash nutzt die NUL-Form konsistent |
| **`c.id`-Cert trailing NUL** | `Certificate::load_cert_bytes`: `original_bytes.length(i + 1)` → Cert-PEM/DER **+ NUL-Byte** | webpki lehnt mit `TrailingData(SignedData)` ab | `cid_to_der` strippt trailing NULs (nur fuers Parsen; hash nutzt rohe Bytes weiter) |

Diagnose: OpenDDS' `OPENDDS_SECURITY_DEBUG=auth_debug,encdec_debug,showkeys` + Source-Lesen
(`Spdp::handle_participant_data` → `pre_check_auth` → `validate_remote_identity`;
`has_security_data()` = `dataKind ∈ {DPDK_ENHANCED, DPDK_SECURE}`). Schluessel: OpenDDS loggt
die `validate_remote_identity`-SecurityException NICHT → der Fehler war voellig still.

### GELÖST: data_protection-Schicht — Governance-FLOOR bei abwesendem EndpointSecurityInfo

**OpenDDS sendet KEINE `PID_ENDPOINT_SECURITY_INFO` (0x1004) im SEDP** (pcap-verifiziert; es
verlaesst sich auf die geteilte Domain-Governance). zerodds leitete daraus
`reader_protection = None` ab → `secure_outbound_for_target` sendete die User-DATA **plaintext**
→ OpenDDS `decode_serialized_payload failed [-3.1]: Crypto Key not found` (Plain als
CryptoHeader fehlinterpretiert). cyclone/FastDDS annoncieren die EndpointSecurityInfo → bei
ihnen korrektes ENCRYPT-Level.

**Fix** (`runtime.rs`, SEDP-Subscription-Processing): per-Reader-Level NUR bei **explizit**
annonciertem `security_info` in `reader_protection` setzen. Fehlt es, KEIN `None`-Override —
dann greift der Governance-`data_protection`-FLOOR in `secure_outbound_for_target`. Das ist
zugleich der Leak-Schutz-Intent (ein authentifizierter Peer im ENCRYPT-Domain erwartet
verschluesselte Payload). cyclone/FastDDS senden security_info → unveraendert (26/26 verifiziert).

→ **+3 Profile** (data-enc, common-subset, liv-data-enc, je beide Richtungen).

### Offen — kategorisiert per OpenDDS↔OpenDDS-Self-Beweislauf

Methode (wie bei FastDDS↔cyclone): **OpenDDS-Self** als Schiedsrichter. Self läuft = **unser**
Interop-Bug (fixbar). Self scheitert = OpenDDS-seitig (Limitation ODER Config).

| Profil | OpenDDS-Self | Kategorie + Beleg |
|---|---|---|
| **disc-data-enc** | ✅ p50=219µs | **UNSER Bug** — secure-SEDP-Reader-Registrierung unter `discovery_protection` (kein `locator_to_peer` → Plain-Pfad; OpenDDS `deliver_from_secure [-1.0]: Invalid Sending Participant`). Fixbar. |
| **rtps-enc** | ✅ p50=281µs | **UNSER Bug** — SRTPS-Schicht (rtps=ENCRYPT). Fixbar. |
| **rtps-sign-data** | ✅ p50=312µs | **UNSER Bug** — SRTPS + `handle_participant_crypto_tokens failed`. Fixbar. |
| **data-sign** | ❌ kein Match | **OpenDDS-LIMITATION (belegt):** `decode_serialized_payload [-3.3]: Auth-only payload transformation not supported (DDSSEC12-59)` + Source `CryptoBuiltInImpl.cpp:2310`. `data_protection_kind=SIGN` wird nicht unterstützt. |
| **all-sign** | ❌ kein Match | **OpenDDS-LIMITATION (belegt):** enthält `data_protection_kind=SIGN` → selbe DDSSEC12-59-Grenze. |
| **all-enc** | ❌ create-fail (OpenDDS-Self) | **OpenDDS-spezifische Governance-Striktheit (belegt):** OpenDDS' `check_create_participant` liest Table 63 **topology-only** (Source `AccessControlBuiltInImpl.cpp:281-347`): erlaubt nur, wenn ein Topic `read`/`write_access_control=FALSE` ODER `join_access_control=FALSE`. all-enc hat ALLES protected + `join_access_control=true` → FALSE → OpenDDS erstellt keinen Participant (auch OpenDDS-Self). **Das ist OpenDDS-eigen, nicht bindend:** §8.4.2.9.3 ist grant-basiert — zerodds/Cyclone/FastDDS joinen full-AC mit gültigem Grant. NICHT zerodds. |
| **sros2-full** | ❌ create-fail (OpenDDS-Self) | **OpenDDS-spezifisch:** wie all-enc (OpenDDS topology-only Table-63-Reject; zerodds/Cyclone/FastDDS joinen mit Grant). |

**Isolierter Blocker der 3 fixbaren (rtps-enc, repräsentativ):** unter `rtps_protection=ENCRYPT`
wrappt zerodds JEDE Message (inkl. secure-SPDP/SEDP) message-level-SRTPS. OpenDDS **dropt** sie
(`decode_rtps_message no remote participant crypto handle ... dropping`, 867×), BIS der
Participant-Crypto-Token ueber den Volatile-Kanal ankommt (`Spdp::handle_participant_crypto_
tokens`); danach **0 Drops** — OpenDDS decodet zerodds' SRTPS. Aber die VOR dem Token gedroppte
SRTPS-SEDP (zerodds' Publication-Announce) wird nicht re-delivered → OpenDDS lernt zerodds'
Writer nie → kein User-Match. = **SRTPS-Discovery-Konvergenz-Race** (Re-NACK/Re-Send nach
Participant-Key-Austausch). cyclone↔zerodds rtps-enc laeuft (26/26) → OpenDDS-Flow-spezifisches
Timing. Analog FastDDS-secure-SPDP-Arbeit, fuer OpenDDS' Volatile-Token-/SEDP-Reliable-Flow.

**Ehrliche Bilanz:** von 7 verbleibenden sind **3 unsere fixbaren Interop-Bugs** (disc-data-enc,
rtps-enc, rtps-sign-data — SRTPS/secure-SEDP-Konvergenz-Schicht), **4 OpenDDS-seitig**: 2 belegte
Limitationen (data-sign, all-sign = `data_protection=SIGN`/DDSSEC12-59) + 2 OpenDDS-eigene
Governance-Striktheit (all-enc, sros2-full = OpenDDS' topology-only Table-63-Reject; §8.4.2.9.3
ist grant-basiert, zerodds/Cyclone/FastDDS joinen full-AC mit Grant). Alle 4 OpenDDS-seitigen sind
OpenDDS-Self-reproduziert + source-belegt (wie FastDDS↔cyclone). Theoretisches zerodds-Maximum
**gegen OpenDDS**: **9/13** (die 3 fixbaren), Rest ist OpenDDS-Stance/Limitation — nicht zerodds.

**Stand:** **OpenDDS 6/13** (common-subset, data-enc, meta-data-enc, meta-sign-data,
liv-data-enc, disc-meta-data — je beide Richtungen, 12/26); Rest per Self-Lauf kategorisiert:
**2 belegte OpenDDS-Limitationen** (data-sign, all-sign = DDSSEC12-59), **3 fixbare unsere Bugs**
(disc-data-enc, rtps-enc, rtps-sign-data), **2 ungeklärte AC-Config-Fälle** (all-enc, sros2-full).
Auth- + data_protection-Schicht vollständig; **cyclone 26/26 + FastDDS 26/26 regression-frei**;
458 dcps + 193 security-pki + 116 discovery + 87 security-crypto Unit-Tests grün.
