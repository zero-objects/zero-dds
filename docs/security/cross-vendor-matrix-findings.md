# Cross-Vendor DDS-Security Interop — Matrix-Findings (Reevaluierungs-Doku)

**Zweck:** Pro Cross-Vendor-Klemme dokumentiert wo + warum es scheitert, ob es
**unser** Bug oder ein **Vendor-/Spec-Problem** ist, mit Web-Recherche + (wenn
ungelöst) verlinktem **aktuellem Upstream-Ticket**. Damit eine spätere
Reevaluierung (z.B. nach einem Vendor-Release) nicht stundenlang neu
recherchieren muss.

**Harness:** `bench-security/gen_profile.sh` (generiert pro Profil XSD-konforme
governance + signiert per-Vendor) + `bench-security/deep_matrix.sh` (Profile ×
4×4 Vendor-Paare, cyclone/fastdds/opendds/zerodds, beide Rollen, n=200/2000).
RTI = out-of-scope (Pro/LM-Lizenz). Bench-Host: codepit (LXC, Debian 13),
domain 200, FastDDS v3.6.1, Cyclone 11.0.x, OpenDDS (`/opt/opendds-secure`).

**Asset-Voraussetzungen (sonst SEC_FAIL bei JEDEM Vendor):**
- **CMS-Signer = Permissions-CA DIREKT** (self-signed CA als SignedData-Signer,
  OMG-Real-World-Muster). Mit einem EE-„authority"-Cert als Signer scheitert
  cyclone an `PKCS7_get0_signers: signer certificate not found`. `gen_profile.sh`
  signiert mit `permissions_ca.pem`/`permissions_ca_key.pem`.
- **Per-Vendor-MIME:** cyclone/FastDDS/ZeroDDS = `openssl smime -sign -text`
  (PKCS7_TEXT); OpenDDS = raw (siehe F-OPENDDS).
- governance XSD `…/20170901/…`, strikte `<xs:sequence>`-Reihenfolge, eingerückt.

---

## F-KEYPROT — IS_KEY_PROTECTED unter data=ENCRYPT — **ZeroDDS-Bug, GEFIXT**

- **Symptom (codepit, profile common-subset = data=ENCRYPT/metadata=NONE):**
  cyclone-Log `match_remote_writer … security_attributes mismatch:
  0x80000010 (zerodds) - 0x80000030 (cyclone)`. Differenz-Bit **0x20 =
  IS_KEY_PROTECTED**. zerodds→cyclone matchte nicht.
- **Wurzel:** `user_endpoint_security_info` band `IS_KEY_PROTECTED` an die
  **metadata**-Protection. Matchte zufällig bei meta=ENCRYPT+data=ENCRYPT (beide
  0x38), aber NICHT bei data-only.
- **Spec-Beleg (unser Bug, cyclone korrekt):** DDS-Security 1.2 §10.4.1.2.6 —
  data=NONE → key=F; data=SIGN → key=F; **data=ENCRYPT → is_key_protected=TRUE**.
  is_key_protected folgt der DATEN-, nicht der metadata-Protection.
- **Fix:** `d5227073` — `compute_user_endpoint_attrs`, KEY an `data==ENCRYPT`
  gebunden. Spec-Tabellen-Test `key_protected_follows_data_encrypt_per_spec_10_4_1_2_6`.
- **Status:** RESOLVED (unser Bug).

---

## F-CYC-FAST — cyclone ↔ FastDDS secured — **Vendor-Loch, NICHT unser Bug**

- **Symptom:** cyclone↔FastDDS secured matcht unter **keinem** Profil
  (`NO_MATCH`, pub=0/sub=0) — auch ohne ZeroDDS im Spiel. Auth-Handshake
  vollendet, aber User-Endpoints matchen nie.
- **Wurzel (Trace + Recherche):** (1) FastDDS tauscht über VolatileSecure nur
  ff0101-Tokens (participant + 1 r/w-Paar), **nicht ff0003/ff0004** (secure-SEDP)
  ohne per-topic `enable_discovery_protection`; (2) DH-Public-Key-Wire-Encoding
  raw `BN_bn2bin` (FastDDS) vs ASN.1 (cyclone) bei `DH+MODP-2048-256`.
- **Web-Recherche / aktuelle Tickets:**
  - **Cyclone #1547 — OFFEN, `needs-triage`** (geöffnet 2023-01-23, keine
    Resolution, Stand Juni 2026): „CycloneDDS publisher → FastDDS subscriber
    fails: *Failed to convert octet sequence to ASN1 integer*". Exakt unser
    Befund. https://github.com/eclipse-cyclonedds/cyclonedds/issues/1547
  - FastDDS #3802 (DH-Encoding raw vs ASN.1), #3803 (`\0` an `dsign_algo`/
    `kagree_algo` — ZeroDDS-seitig gefixt via `compute_hash_c_raw`/`297dc526`),
    #3804 (optionale Reply-Felder als Pflicht).
  - Cyclone #2184, #1477 (allg. cyclone↔FastDDS Interop).
- **Schlussfolgerung:** Genuines vendor-level Interop-Loch zwischen Cyclone und
  FastDDS, ungelöst upstream. ZeroDDS kann beide einzeln bedienen (mit dem
  jeweils passenden key_agreement/Profil), aber cyclone↔FastDDS selbst ist nicht
  unsere Verantwortung. **Reevaluieren wenn Cyclone #1547 / FastDDS #3802 schließen.**
- **Status:** EXTERNAL (vendor), tracked via Cyclone #1547 (open).

---

## F-OPENDDS — OpenDDS lehnt signierte Assets ab — **offen (Bench-Tooling)**

- **Symptom (codepit):** opendds→opendds (eigene Assets!) `participant create
  failed`, `WARNING: SMIME_read_PKCS7 failed: error:068000CE:asn1 … mime no
  content type` + `smime text error`.
- **Wurzel:** OpenDDS' `SMIME_read_PKCS7` akzeptiert das von `gen_profile.sh`
  erzeugte `raw`-p7s-Format noch nicht (Doc-Finding #2/#3 der 2026-05-27-Baseline:
  OpenDDS braucht ein spezifisches rohes Format, kein `-text`-Wrapper).
- **Klassifikation:** Bench-Asset-Tooling (kein ZeroDDS-Wire-Bug). Die
  2026-05-28-Baseline hatte opendds 15/16 mit einem funktionierenden raw-Asset —
  das Format muss in `gen_profile.sh` repliziert werden.
- **TODO:** OpenDDS-konformes raw-Signing/-Format im Generator; danach
  opendds-Zeilen der Matrix re-testen.
- **Status:** OPEN (bench tooling).

---

## F-RESPONDER — Fremd-Initiator → ZeroDDS-Responder: Auth-Handshake stockt — **ZeroDDS-Bug, GEFIXT 2026-06-05 (`9503baf5`)**

> **AUFLÖSUNG (echte Wurzel, widerlegt die frühere "Transport"-Vermutung):**
> Die `[HS]`-leer-Diagnose war ein Trace-Artefakt — instrumentierte man auch den
> DATA_FRAG-Pfad (`HSDFARM`/`HSFRAG`), zeigte sich: cyclones Stateless-Request
> ERREICHT den Dispatch sehr wohl, aber **RTPS-fragmentiert** (3956 B Cert+DH+
> Permissions, 3 Fragmente @ 1344 B). zerodds reassembliert sauber (`COMPLETE`),
> decodiert `class=dds.sec.auth` — und `begin_handshake_reply` wirft dann
> **`AuthenticationFailed: "missing binary property: hash_c1"`**.
> **Wurzel:** `parse_request_token` (security-pki/handshake_token.rs) machte
> `hash_c1` zur PFLICHT. Per **OMG DDS-Security §9.3.2.3.1 ist `hash_c1` OPTIONAL**
> (Optimierung) — cyclone/FastDDS senden es nicht; der Responder MUSS es aus
> (c.id, c.perm, c.pdata, c.dsign_algo, c.kagree_algo) selbst berechnen.
> **Fix:** optional — present → Tamper-Check gegen Recompute, absent → Recompute
> nutzen. +2 TDD-Tests (`parse_request_token_computes_hash_c1_when_absent`,
> `…rejects_tampered_hash_c1`). **Cross-vendor live-verifiziert:** cyclone erreicht
> jetzt `AUTHENTICATED` + `EVENT_VALIDATION_OK_FINAL_MESSAGE` + `pub=1` (vorher
> Deadlock pub=0/sub=0). Die GUID-Rollenlogik (`local<remote`=Initiator) war
> KORREKT und deckt sich mit cyclones FSM (`PENDING_HANDSHAKE_REQUEST` für die
> kleinere GUID) — kein Rollen-Bug.
> Die untenstehende historische Spur bleibt für Reevaluierung erhalten.

- **Symptom (codepit, common-subset):** `zerodds→cyclone` = 56µs ✓, aber
  `cyclone→zerodds` = NO_MATCH (`ping: match timeout pub=0 sub=0`). Betrifft
  **alle** Fremd-Initiatoren gegen ZeroDDS-als-Responder (cyclone/fastdds/opendds
  → zerodds).
- **Trace (cyclone finest, lguid=cyclone aefd18ce < rguid=zerodds b92af1c5):**
  cyclone (kleinere GUID = **Initiator**) hängt in `state_handshake_message_wait`
  → `EVENT_TIMEOUT` → `handshake resend` in Endlosschleife (4s lang). cyclone
  EMPFÄNGT zerodds vollständig: SPDP-Beacons #3/#4/#5 (identity_token PKI-DH,
  EC-prime256v1), alle SEDP-User-Endpoints (203 Echo-Writer + 104 Request-Reader,
  `endpoint_security_info=0x80000030:0x80000002` — IS_KEY_PROTECTED-Fix wirkt!),
  alle secure-Builtins (ff0101/ff0202/…). Aber **kein stateless HandshakeReply
  von zerodds (b92af1c5)** im cyclone-Trace.
- **Wurzel:** ZeroDDS-als-Responder (= das bereits laufende pong, das einen spät
  joinenden Initiator-Peer entdeckt) sendet keinen Auth-HandshakeReply. zerodds-
  als-Initiator (= late-joiner ping) funktioniert dagegen (→cyclone 56µs).
  Auffällig: zerodds-Beacon trägt `permissions_token=…:{}:{}` **LEER** — könnte
  cyclones AccessControl beim Reply-Path beeinflussen; primär aber sendet zerodds
  gar keine Reply.
- **Klassifikation: OUR bug** (ZeroDDS), **Regression** vs 2026-05-28-Baseline
  (damals cyclone→zerodds 53µs ✓ — siehe `2026-05-27-security-baseline.md`).
  Eingeführt irgendwo in feat/secured-user-data. Auth-Handshake läuft über
  ParticipantStatelessMessage (StatelessReader, NICHT von H-1 berührt) — H-1
  (ReliableReader-Demux) betrifft nur den VolatileSecure-Crypto-Token-Pfad, nicht
  den Auth-Handshake. Kandidaten: Discovery→`begin_handshake_with`-Trigger bzw.
  on_stateless_message-Reply-Pfad wenn ZeroDDS der früh-laufende Responder ist.
- **TODO:** ZeroDDS-seitige Instrumentierung (dylib mit Handshake-eprintln, da
  C-FFI keinen env_logger initialisiert → RUST_LOG wirkungslos) um zu trennen:
  empfängt zerodds cyclones HandshakeRequest auf dem Stateless-Reader, und warum
  sendet on_stateless_message keine Reply. Code: `runtime.rs:6501-6516` (Live-
  SPDP-Recv → begin_handshake_with) + der Stateless-Dispatch/on_stateless_message.
- **Status:** OPEN (unser Bug, höchste Prio für zerodds-Responder-Interop).
- **NACHTRAG (instrumentiert, ZERODDS_HS_TRACE auf wip/hs-trace 224883fd):** Mit
  gated eprintln im Stateless-Dispatch (`dispatch_security_builtin_datagram`,
  runtime.rs:7663) + on_stateless_message-Ergebnis: bei `cyclone→zerodds` bleibt
  der `[HS]`-Trace auf zerodds-Seite **LEER** → cyclones Stateless-HandshakeRequest
  erreicht den Security-Builtin-Dispatch von zerodds (frühes pong) **gar nicht**.
  → **Empfangs-/Transport-Pfad-Problem, NICHT Reply-Logik** (on_stateless_message
  ist nie aufgerufen). dylib war frisch + instrumentiert (strings-verifiziert,
  keine stale-dylib). Fix-Richtung: tcpdump/Locator — auf welche Adresse sendet
  cyclone den Request (zerodds' announced stateless-reader-Locator), und warum
  landet er nicht im recv_metatraffic_loop→dispatch_security_builtin_datagram von
  zerodds, wenn zerodds der früh-laufende Responder ist. Auth-Handshake ist
  plaintext (kein Decode-Gate). Eigene fokussierte Transport-Debug-Sitzung nötig.

## F-ECHO-WRITE — cyclone→ZeroDDS-Responder: voller Roundtrip flaky (sub=0) — **ZeroDDS-Bug, TEILWEISE GEFIXT, Determinismus-Followup OFFEN**

- **Symptom:** Nach dem F-RESPONDER-Fix completet der Auth-Handshake und cyclone
  matcht zerodds' Reader (`pub=1`), aber cyclones Reader matcht zerodds' Echo-
  Writer NICHT (`sub=0`) → kein Roundtrip. Plain (ohne Security) cyclone→zerodds-
  pong = **p50 33.9µs voller Roundtrip** → rein security-spezifisch.
- **Kette (HSREG/HSPUB/HSTOK + cyclone-finest):** zerodds erzeugt Echo-Writer
  (eid 0203) spät (typed echo, nach Match), announced ihn (SEDP-DATA(w) → cyclone),
  cyclone matcht ihn (`connect_proxy_writer_with_reader 203↔404`) aber
  **`waiting for approval by security`** — wartet auf zerodds' `datawriter_crypto_
  tokens` für 203. Das Token wird vorbereitet (HSTOK: matched_to_peer=1), geht
  über VolatileSecure — aber cyclone verwirft zerodds' Volatile-Submessages als
  **`clear submsg from protected src`**: zerodds sendete als Responder seine
  ParticipantVolatileMessageSecure-Submessages teils im KLARTEXT.
- **GEFIXT (`b79d485a`, §8.4.2.4):** (1) `protect_volatile_datagram` schützt jetzt
  ALLE Writer-Submessages (DATA/DATA_FRAG/**HEARTBEAT**/HEARTBEAT_FRAG/GAP), nicht
  nur DATA — der klare HEARTBEAT im write_with_heartbeat-Bundle ließ cyclone die
  ganze Token-Sample verwerfen. (2) Der Volatile-Protect-Tick nutzt die stabile
  `authenticated_peer_prefixes()` statt `completed_peer_prefixes()` (letztere wird
  nach Handshake-Completion GC't → späte Resends gingen klar raus).
  → cyclone→zerodds erreicht jetzt `tokens available` + **vollen Roundtrip
  p50=149µs** (bewiesen). Keine Regression: zero↔zero n=5000 p50=32.7µs,
  zerodds→cyclone n=5000 p50=46.5µs, 457/457 dcps-Tests grün.
- **OFFEN (Determinismus, ~0/6 grün):** zwei Followups:
  1. **Volatile-Reader-ACKNACK** wird noch klar gesendet (encode_kx_datawriter
     als Seal → cyclone `Invalid Crypto Handle`, weil cyclone den Reader-ACKNACK
     datareader-keyed mit eigener sender_key_id erwartet). Braucht korrektes
     kx-datareader-Encoding (verwandt mit FIX C Receiver-Specific-MAC).
  2. **Late-Token-Send:** das datawriter_crypto_token des spät erzeugten Echo-
     Writers sollte SOFORT bei `register_user_writer_kind` an authentifizierte
     Peers gehen, nicht erst per Tick — sonst Race gegen cyclones Match-Deadline
     (event-driven pong tickt evtl. nicht rechtzeitig). zerodds→cyclone geht
     deterministisch, weil dort der Writer früh existiert (Token im Completion-Batch).
- **Status:** Roundtrip ACHIEVABLE + spec-korrekte Volatile-Fixes gelandet;
  Determinismus für cyclone/fastdds/opendds→zerodds-Responder ist der nächste Schritt.

## F-GOV-FLAKY — data_protection_kind als SIGN statt ENCRYPT — **vermutlich durch IS_KEY_PROTECTED-Fix erledigt / Fehldiagnose**

> **NACHTRAG 2026-06-05:** In den frischen cyclone→zerodds-Läufen annoncierte
> zerodds **konsistent** `0x80000030:0x80000002` (data=ENCRYPT) für BEIDE
> Endpoints (Reader 104 + Writer 203, cyclone-finest verifiziert). Das frühere
> `0x80000010`-(SIGN)-Lesen war ein Vor-`d5227073`-Artefakt bzw. Mess-Substring-
> Fehler (`meta`**`data`** matcht `data_protection_kind`). Kein Flaky-Verhalten
> mehr beobachtet. Beobachten, falls es wiederkehrt.



- **Symptom:** Bei `cyclone→zerodds` common-subset annoncierte zerodds in einem
  Lauf `endpoint_security_attributes=0x80000010.0x80000000` (= PAYLOAD-protected,
  **KEIN** KEY, plugin OHNE PAYLOAD_ENCRYPTED) — das ist exakt das **data=SIGN**-
  Muster (§10.4.1.2.6). In einem FRÜHEREN Lauf (gleiche governance-Datei, Build
  d5227073) war es korrekt `0x80000030.0x80000002` (data=ENCRYPT).
- **Governance-Asset ist KORREKT** (`data_protection_kind=ENCRYPT` in .xml UND
  im CMS-`-text`-signierten .p7s, verifiziert) → zerodds liest/parst die data-
  Protection **flaky** als SIGN statt ENCRYPT.
- **Klassifikation: unser Bug** — Race/Timing im governance-Load vs. der
  user_endpoint_security_info-Berechnung beim Endpoint-Announce (Endpoint wird
  evtl. announced, bevor die governance voll geparst/aktiv ist), oder ein
  nicht-deterministischer Parse-Pfad in `gate.data_protection()`.
- **TODO:** governance-Load deterministisch VOR dem ersten Endpoint-Announce
  sicherstellen; `data_protection()`/`metadata_protection()`-Parse auf Determinismus
  prüfen. Reproduzieren via wiederholtem cyclone→zerodds + Vergleich der annoncten
  endpoint_security_attributes.
- **Status:** OPEN (unser Bug, flaky — verschärft F-RESPONDER-Diagnose).

## Matrix-Ergebnisse (werden pro Lauf hier angehängt)

### Common-Subset (data=ENCRYPT), n=200, HEAD d5227073 + opendds-text
```
              pong→  cyclone   fastdds   opendds   zerodds
cyclone              64µs      NO_MATCH  401µs     NO_MATCH(F-RESPONDER)
fastdds              NO_MATCH  111µs     NO_MATCH  NO_MATCH
opendds              SEC_FAIL  NO_MATCH  224µs     NO_MATCH
zerodds              56µs ✓    NO_MATCH  NO_MATCH  31µs ✓
```
Same-Vendor-Diagonale grün. zerodds→cyclone ✓ (IS_KEY_PROTECTED-Fix). Offen:
F-RESPONDER (alle →zerodds), zerodds↔fastdds (#30 + F-CYC-FAST-Klasse),
zerodds↔opendds, opendds-Initiator-Rolle.

<!-- deep_matrix.sh-Tabellen pro Profil; p50 / SEC_FAIL / NO_MATCH / FAIL -->
